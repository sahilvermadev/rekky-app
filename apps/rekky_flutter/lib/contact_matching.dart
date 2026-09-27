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

List<LocalContact> contactCandidates(
  String subject,
  List<LocalContact> contacts,
) {
  final name = normalizedContactName(subject);
  final tokens = name.split(' ').where((word) => word.length > 1).toSet();
  return contacts.where((c) {
    final candidate = normalizedContactName(c.name).split(' ').toSet();
    return c.phones.isNotEmpty && tokens.intersection(candidate).isNotEmpty;
  }).toList()..sort((a, b) {
    final exactA = normalizedContactName(a.name) == name;
    final exactB = normalizedContactName(b.name) == name;
    return exactA != exactB ? (exactA ? -1 : 1) : a.name.compareTo(b.name);
  });
}

/// Deliberately abstain on single names, display-only/business names, duplicate
/// identities, extra qualifiers, or multiple numbers. Fuzzy matches are choices.
String? automaticContactPhone(String subject, List<LocalContact> contacts) {
  final name = normalizedContactName(subject);
  if (name.split(' ').where((w) => w.length > 1).length < 2) return null;
  final exact = contacts
      .where((c) => normalizedContactName(c.name) == name)
      .toList();
  if (exact.length != 1) return null;
  final contact = exact.single;
  if (contact.first.trim().isEmpty || contact.last.trim().isEmpty) return null;
  final words = name.split(' ');
  if (!words.contains(normalizedContactName(contact.first)) ||
      !words.contains(normalizedContactName(contact.last))) {
    return null;
  }
  final numbers = contact.phones.toSet();
  if (numbers.length != 1) return null;
  return internationalPhone(numbers.single);
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
  void stop() => _stopped = true;
  bool get current => !_stopped && isCurrent();
  Future<void> process(List<RekkyItem> items) async {
    if (_running || !current) return;
    _running = true;
    try {
      final eligible = items
          .where(
            (item) =>
                item.recommendation?.entityKind == 'person_service' &&
                item.recommendation?.contactPhone == null &&
                item.recommendation?.contactMatchingOff != true &&
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
        final phone = automaticContactPhone(item.subject, contacts);
        if (phone == null) continue;
        try {
          final saved = await api.attachContact(item, {
            'mode': 'automatic',
            'phone': phone,
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
    }
  }
}
