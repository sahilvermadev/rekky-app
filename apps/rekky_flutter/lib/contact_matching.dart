import 'package:flutter_contacts/flutter_contacts.dart';

import 'rekky_api.dart';

String? internationalPhone(String value) {
  if (value.length > 80 || RegExp(r'[^0-9+ ()\-.]').hasMatch(value)) {
    return null;
  }
  final number = value.replaceAll(RegExp(r'[ ()\-.]'), '');
  return RegExp(r'^\+[1-9][0-9]{7,14}$').hasMatch(number) ? number : null;
}

String normalizedContactName(String value) {
  final words = value
      .toLowerCase()
      .replaceAll(RegExp(r'[.,]'), ' ')
      .trim()
      .split(RegExp(r'\s+'));
  while (words.isNotEmpty &&
      const {'dr', 'doctor', 'mr', 'mrs', 'ms', 'prof'}.contains(words.first)) {
    words.removeAt(0);
  }
  return words.join(' ');
}

class LocalContact {
  const LocalContact(this.name, this.phones, {this.first = '', this.last = ''});
  final String name, first, last;
  final List<String> phones;
  factory LocalContact.fromNative(Contact contact) => LocalContact(
    contact.displayName ?? '',
    contact.phones
        .map((p) => internationalPhone(p.normalizedNumber ?? '') ?? p.number)
        .toSet()
        .toList(),
    first: contact.name?.first ?? '',
    last: contact.name?.last ?? '',
  );
}

// Small local vocabulary provides service equivalence without sending contacts
// to a language model. Unknown services still retain distinctive name matching.
const _serviceWords = <String, String>{
  'taxi': 'taxi',
  'taxis': 'taxi',
  'cab': 'taxi',
  'cabs': 'taxi',
  'driver': 'taxi',
  'chauffeur': 'taxi',
  'doctor': 'doctor',
  'dr': 'doctor',
  'physician': 'doctor',
  'gp': 'doctor',
  'dentist': 'dentist',
  'dental': 'dentist',
  'plumber': 'plumber',
  'plumbing': 'plumber',
  'electrician': 'electrician',
  'electrical': 'electrician',
  'carpenter': 'carpenter',
  'carpentry': 'carpenter',
  'tutor': 'tutor',
  'teacher': 'tutor',
  'tuition': 'tutor',
  'cleaner': 'cleaner',
  'cleaning': 'cleaner',
  'housekeeping': 'cleaner',
  'mechanic': 'mechanic',
  'garage': 'mechanic',
  'tailor': 'tailor',
  'tailoring': 'tailor',
  'salon': 'salon',
  'barber': 'salon',
  'hairdresser': 'salon',
  'lawyer': 'lawyer',
  'advocate': 'lawyer',
  'attorney': 'lawyer',
  'accountant': 'accountant',
  'ca': 'accountant',
  'physio': 'physio',
  'physiotherapist': 'physio',
  'physiotherapy': 'physio',
  'vet': 'vet',
  'veterinarian': 'vet',
  'painter': 'painter',
  'painting': 'painter',
  'photographer': 'photographer',
  'photography': 'photographer',
  'cook': 'cook',
  'chef': 'cook',
  'caterer': 'cook',
  'catering': 'cook',
  'guide': 'guide',
  'trainer': 'trainer',
  'coach': 'trainer',
};
const _genericWords = {
  'the',
  'and',
  'in',
  'at',
  'of',
  'my',
  'mr',
  'mrs',
  'ms',
  'prof',
  'general',
  'personal',
  'service',
  'services',
  'company',
  'co',
  'pvt',
  'ltd',
  'limited',
  'contact',
  'number',
};
List<String> _tokens(String text) => text
    .toLowerCase()
    .replaceAll(RegExp(r'[^\p{L}\p{N}]+', unicode: true), ' ')
    .trim()
    .split(RegExp(r'\s+'))
    .where((v) => v.length > 1)
    .toSet()
    .toList();
Set<String> _services(Iterable<String> tokens) =>
    tokens.map((t) => _serviceWords[t]).whereType<String>().toSet();
int _distance(String a, String b) {
  var previous = List<int>.generate(b.length + 1, (i) => i);
  for (var i = 0; i < a.length; i++) {
    final next = List<int>.filled(b.length + 1, 0)..[0] = i + 1;
    for (var j = 0; j < b.length; j++) {
      final values = [
        next[j] + 1,
        previous[j + 1] + 1,
        previous[j] + (a[i] == b[j] ? 0 : 1),
      ];
      next[j + 1] = values.reduce((a, b) => a < b ? a : b);
    }
    previous = next;
  }
  return previous.last;
}

double _similarity(String a, String b) {
  if (a == b) return 1;
  if (a.length < 4 || b.length < 4 || a[0] != b[0]) return 0;
  final distance = _distance(a, b);
  if (distance == 1) return .9;
  // Common vowel/transcription variation: Lavnish / Lavneesh. Not sufficient
  // by itself for a single-name match; service and locality must corroborate.
  String consonants(String v) => v
      .replaceAll(RegExp('[aeiou]'), '')
      .replaceAllMapped(RegExp(r'(.)\1+'), (m) => m[1]!);
  final skeleton = consonants(a);
  if (skeleton.length >= 3 && skeleton == consonants(b) && distance <= 3) {
    return .86;
  }
  return 0;
}

class ContactMatch {
  const ContactMatch(this.contact, this.score);
  final LocalContact contact;
  final double score;
  String? get phone {
    final numbers = contact.phones
        .map((p) => internationalPhone(p) ?? p)
        .toSet();
    return numbers.length == 1 ? internationalPhone(numbers.single) : null;
  }
}

List<ContactMatch> rankContacts(
  String subject,
  List<LocalContact> contacts, {
  RekkyRecommendation? recommendation,
}) {
  final subjectWords = _tokens(subject);
  final contextServices = _services([
    ...subjectWords,
    ..._tokens(recommendation?.categoryLabel ?? ''),
    for (final type
        in recommendation?.classification?.types ?? <CategoryConcept>[])
      ..._tokens(type.label),
  ]);
  final locations = <String>{
    for (final l in recommendation?.locations ?? <RecommendationDetail>[])
      ..._tokens(l.text),
  };
  final identity = subjectWords
      .where(
        (w) =>
            !_genericWords.contains(w) &&
            !_serviceWords.containsKey(w) &&
            !locations.contains(w),
      )
      .toList();
  if (identity.isEmpty) return [];
  final ranked = <ContactMatch>[];
  for (final contact in contacts) {
    if (contact.phones.isEmpty ||
        contact.name.trim().isEmpty ||
        contact.name.runes.length > 200) {
      continue;
    }
    final words = _tokens(contact.name);
    final services = _services(words);
    // Explicitly conflicting occupations are not resolved by a matching name.
    if (contextServices.isNotEmpty &&
        services.isNotEmpty &&
        contextServices.intersection(services).isEmpty) {
      continue;
    }
    final available = words
        .where(
          (w) => !_genericWords.contains(w) && !_serviceWords.containsKey(w),
        )
        .toList();
    var total = 0.0;
    var matched = 0;
    for (final name in identity) {
      var best = 0.0, index = -1;
      for (var i = 0; i < available.length; i++) {
        final score = _similarity(name, available[i]);
        if (score > best) {
          best = score;
          index = i;
        }
      }
      if (index >= 0) {
        total += best;
        matched++;
        available.removeAt(index);
      }
    }
    if (matched == 0) continue;
    final coverage = total / identity.length;
    final service = contextServices.intersection(services).isNotEmpty;
    final locality = locations.any(
      (location) => words.any((word) => _similarity(location, word) >= .86),
    );
    double score;
    if (matched == identity.length && coverage >= .85) {
      if (identity.length >= 2) {
        score = coverage == 1 ? .93 : .88;
        if (service) score += .02;
        if (locality) score += .05;
      } else if (service && identity.single.length >= 4) {
        score = coverage == 1 ? .86 : .80;
        if (locality) score += .15;
      } else {
        score = .60;
      }
    } else {
      score = .5 * coverage;
    }
    ranked.add(ContactMatch(contact, score.clamp(0, 1).toDouble()));
  }
  ranked.sort((a, b) => b.score.compareTo(a.score));
  return ranked;
}

List<LocalContact> contactCandidates(
  String subject,
  List<LocalContact> contacts, {
  RekkyRecommendation? recommendation,
}) => rankContacts(
  subject,
  contacts,
  recommendation: recommendation,
).map((m) => m.contact).toList();

ContactMatch? automaticContactMatch(
  String subject,
  List<LocalContact> contacts, {
  RekkyRecommendation? recommendation,
}) {
  final ranked = rankContacts(
    subject,
    contacts,
    recommendation: recommendation,
  );
  if (ranked.isEmpty ||
      ranked.first.score < .85 ||
      ranked.first.phone == null) {
    return null;
  }
  // Never resolve a tie by phone-book order. Candidates without usable numbers
  // still compete, so filtering a number cannot create false certainty.
  if (ranked.length > 1 &&
      ranked.first.score - ranked[1].score < .08 - .000001) {
    return null;
  }
  return ranked.first;
}

String? automaticContactPhone(
  String subject,
  List<LocalContact> contacts, {
  RekkyRecommendation? recommendation,
}) => automaticContactMatch(
  subject,
  contacts,
  recommendation: recommendation,
)?.phone;

String? savedNameForPhone(String phone, List<LocalContact> contacts) {
  final names = contacts
      .where((c) => c.phones.any((p) => internationalPhone(p) == phone))
      .map((c) => c.name.trim())
      .where((n) => n.isNotEmpty && n.runes.length <= 200)
      .toSet();
  return names.length == 1 ? names.single : null;
}

abstract class ContactBook {
  Future<bool> hasAccess();
  Future<bool> requestAccess();
  Future<List<LocalContact>> read();
}

class DeviceContactBook implements ContactBook {
  @override
  Future<bool> hasAccess() =>
      FlutterContacts.permissions.has(PermissionType.read);
  @override
  Future<bool> requestAccess() async {
    final status = await FlutterContacts.permissions.request(
      PermissionType.read,
    );
    return status == PermissionStatus.granted ||
        status == PermissionStatus.limited;
  }

  @override
  Future<List<LocalContact>> read() async => (await FlutterContacts.getAll(
    properties: {ContactProperty.name, ContactProperty.phone},
  )).map(LocalContact.fromNative).toList();
}

class ContactMatchingCoordinator {
  ContactMatchingCoordinator({
    required this.api,
    required this.book,
    required this.isCurrent,
    required this.onSaved,
  });
  final RekkyApi api;
  final ContactBook book;
  final bool Function() isCurrent;
  final void Function(RekkyItem) onSaved;
  bool _running = false, _stopped = false;
  final _attempted = <String>{};
  List<RekkyItem>? _pending;
  void stop() => _stopped = true;
  bool get current => !_stopped && isCurrent();
  Future<void> process(List<RekkyItem> items) async {
    if (!current) return;
    if (_running) {
      _pending = items;
      return;
    }
    _running = true;
    try {
      final eligible = items
          .where(
            (item) =>
                item.recommendation?.entityKind == 'person_service' &&
                (item.recommendation?.contactPhone != null
                    ? item.recommendation?.contactSavedName == null
                    : item.recommendation?.contactMatchingOff != true) &&
                !_attempted.contains('${item.id}:${item.revision}'),
          )
          .toList();
      if (eligible.isEmpty) return;
      final preference = await api.contactPreference();
      if (!current ||
          preference['enabled'] != true ||
          !await book.hasAccess() ||
          !current) {
        return;
      }
      // In-memory for this batch only. No contact identifiers or address book
      // survive the operation, leave the device, or enter analytics/model input.
      final contacts = await book.read();
      if (!current) return;
      for (final item in eligible) {
        if (!current || !await book.hasAccess() || !current) return;
        final existingPhone = item.recommendation?.contactPhone;
        final match = existingPhone == null
            ? automaticContactMatch(
                item.subject,
                contacts,
                recommendation: item.recommendation,
              )
            : null;
        final phone = existingPhone ?? match?.phone;
        final savedName = existingPhone != null
            ? savedNameForPhone(existingPhone, contacts)
            : match?.contact.name.trim();
        if (phone == null || savedName == null) continue;
        try {
          final saved = await api.attachContact(item, {
            'mode': existingPhone == null ? 'automatic' : 'describe',
            'phone': phone,
            'saved_name': savedName,
            'generation': preference['generation'],
          });
          if (!current) return;
          _attempted.add('${item.id}:${item.revision}');
          onSaved(saved);
        } on ApiFailure catch (error) {
          if (error.status == 409 || error.status == 404) {
            _attempted.add('${item.id}:${item.revision}');
          } else {
            rethrow;
          }
        }
      }
    } catch (_) {
      // Optional enrichment never interrupts capture. Retry on next foreground
      // or Library refresh; manual attachment remains available.
    } finally {
      _running = false;
      final pending = _pending;
      _pending = null;
      if (pending != null && current) await process(pending);
    }
  }
}
