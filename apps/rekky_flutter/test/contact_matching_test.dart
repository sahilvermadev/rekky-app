import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/contact_matching.dart';
import 'package:rekky_flutter/rekky_api.dart';

const doctor = LocalContact(
  'Dr. Maya Rao',
  ['+919876543210'],
  first: 'Maya',
  last: 'Rao',
);
RekkyItem item({
  String? phone,
  bool removed = false,
  String kind = 'person_service',
  String? savedName = 'Dr. Maya Rao',
}) => RekkyItem(
  id: 'item',
  captureId: 'capture',
  subject: 'Dr. Maya Rao',
  body: 'Helpful doctor.',
  visibility: 'friends',
  revision: 1,
  createdAt: '',
  recommendation: RekkyRecommendation(
    summary: 'Helpful doctor.',
    shelf: 'People',
    experience: 'firsthand',
    observations: [],
    locations: [],
    useCases: [],
    entityKind: kind,
    contactPhone: phone,
    contactSavedName: phone == null ? null : savedName,
    contactMatchingOff: removed,
  ),
);

class Book implements ContactBook {
  bool allowed = true;
  int reads = 0;
  Completer<List<LocalContact>>? pending;
  @override
  Future<bool> hasAccess() async => allowed;
  @override
  Future<bool> requestAccess() async => allowed;
  @override
  Future<List<LocalContact>> read() async {
    reads++;
    return pending?.future ?? [doctor];
  }
}

class Api extends RekkyApi {
  Api() : super('http://127.0.0.1');
  bool enabled = true;
  final attached = <Map<String, dynamic>>[];
  @override
  Future<Map<String, dynamic>> contactPreference() async => {
    'enabled': enabled,
    'generation': 4,
  };
  @override
  Future<RekkyItem> attachContact(
    RekkyItem value,
    Map<String, dynamic> contact,
  ) async {
    attached.add(contact);
    return value;
  }
}

void main() {
  test('shared contact wire fixture carries no separate audience', () {
    final wire = jsonDecode(
      File('../../contracts/rekky/v1/fixtures/contact.json').readAsStringSync(),
    ) as Map;
    final contact = wire['saved_contact'] as Map;
    expect(internationalPhone(contact['phone'] as String), contact['phone']);
    expect(contact.keys.toSet(), {'phone', 'origin', 'saved_name'});
    expect(wire['removal'], {'mode': 'none'});
  });
  test('one exact full name with a single international phone attaches', () {
    expect(automaticContactPhone('Maya Rao', [doctor]), '+919876543210');
    expect(automaticContactPhone('Doctor MAYA RAO', [doctor]), '+919876543210');
  });
  test(
    'ambiguous, partial, role-only and country-unknown contacts abstain',
    () {
      expect(automaticContactPhone('Maya', [doctor]), isNull);
      expect(automaticContactPhone('Maya Rao', [doctor, doctor]), isNull);
      expect(
        automaticContactPhone('Maya Rao', [
          const LocalContact(
            'Maya Rao',
            ['+919876543210', '+919876543211'],
            first: 'Maya',
            last: 'Rao',
          ),
        ]),
        isNull,
      );
      expect(
        automaticContactPhone('Maya Rao', [
          const LocalContact(
            'Maya Rao',
            ['9876543210'],
            first: 'Maya',
            last: 'Rao',
          ),
        ]),
        isNull,
      );
      expect(
        automaticContactPhone('Maya Rao', [
          const LocalContact('Maya Rao', ['+919876543210']),
        ]),
        '+919876543210',
      );
      expect(
        contactCandidates('Maya Rao', [
          doctor,
          const LocalContact('Priya', ['+919876543211']),
        ]),
        [doctor],
      );
    },
  );
  test(
    'phone parsing rejects extensions, extra plus signs and missing country',
    () {
      expect(internationalPhone('+91 (98765) 43210'), '+919876543210');
      for (final value in [
        '9876543210',
        '+019876543210',
        '++919876543210',
        '+919876543210;123',
        '+919876543210 ext 2',
        '+1',
      ]) {
        expect(internationalPhone(value), isNull);
      }
    },
  );
  test(
    'batch sends only attached phone and saved name with generation',
    () async {
      final api = Api(), book = Book();
      final saved = <RekkyItem>[];
      final coordinator = ContactMatchingCoordinator(
        api: api,
        book: book,
        isCurrent: () => true,
        onSaved: saved.add,
      );
      await coordinator.process([item()]);
      expect(api.attached, [
        {
          'mode': 'automatic',
          'phone': '+919876543210',
          'saved_name': 'Dr. Maya Rao',
          'generation': 4,
        },
      ]);
      expect(saved, hasLength(1));
      await coordinator.process([item()]);
      expect(api.attached, hasLength(1));
    },
  );
  test('ranked match uses taxi synonyms, spelling variants and locality without exact names', () {
    final rec = RekkyRecommendation(
      summary: '',
      shelf: 'People & services',
      experience: 'firsthand',
      observations: [],
      locations: [const RecommendationDetail('context', 'Landour')],
      useCases: [],
      entityKind: 'person_service',
    );
    const selected = LocalContact('Lavneesh Landor Taxi', ['+12025550123']);
    const unrelated = LocalContact('Rohit Taxi', ['+12025550124']);
    const lessSpecific = LocalContact('Lavneesh Delhi Taxi', ['+12025550125']);
    final result = automaticContactMatch('Lavnish Taxi Cabs', [
      unrelated,
      lessSpecific,
      selected,
    ], recommendation: rec);
    expect(result?.contact.name, selected.name);
    expect(result?.phone, '+12025550123');
    expect(
      contactCandidates('Lavnish Taxi Cabs', [
        unrelated,
        lessSpecific,
        selected,
      ], recommendation: rec).first.name,
      selected.name,
    );
    expect(
      automaticContactMatch('Lavneesh Taxi Cabs', [selected])?.phone,
      '+12025550123',
    );
    expect(
      automaticContactMatch('Lavnish Taxi Cabs', [selected]),
      isNull,
    ); // A spelling-only first name needs corroboration.
  });
  test('ranking rejects tied providers, profession-only matches, conflicting roles and multiple phones', () {
    const first = LocalContact('Lavneesh Taxi', ['+12025550123']);
    const second = LocalContact('Lavneesh Cabs', ['+12025550124']);
    expect(
      automaticContactMatch('Lavneesh Taxi Cabs', [first, second]),
      isNull,
    );
    expect(automaticContactMatch('Taxi Cabs', [first]), isNull);
    expect(automaticContactMatch('Priya Taxi', [first]), isNull);
    expect(
      automaticContactMatch('Dr Maya Rao', [
        const LocalContact('Maya Rao Dentist', ['+12025550123']),
      ]),
      isNull,
    );
    expect(
      automaticContactMatch('Lavneesh Taxi', [
        const LocalContact('Lavneesh Taxi', ['+12025550123', '+12025550124']),
      ]),
      isNull,
    );
    expect(
      automaticContactMatch('Maya Rao', [
        const LocalContact('Maya Sharma', ['+12025550123']),
      ]),
      isNull,
    );
  });
  test('an existing chosen number gets a label by number, never a new provider by name', () async {
    final api = Api(), book = Book();
    await ContactMatchingCoordinator(
      api: api,
      book: book,
      isCurrent: () => true,
      onSaved: (_) {},
    ).process([item(phone: '+919876543210', savedName: null, removed: true)]);
    expect(api.attached, [
      {
        'mode': 'describe',
        'phone': '+919876543210',
        'saved_name': 'Dr. Maya Rao',
        'generation': 4,
      },
    ]);
    expect(
      savedNameForPhone('+919876543210', [
        doctor,
        const LocalContact('Someone Else', ['+919876543210']),
      ]),
      isNull,
    );
  });
  test(
    'newly saved recommendations arriving during a lookup are not dropped',
    () async {
      final api = Api(),
          book = Book()..pending = Completer<List<LocalContact>>();
      final coordinator = ContactMatchingCoordinator(
        api: api,
        book: book,
        isCurrent: () => true,
        onSaved: (_) {},
      );
      final task = coordinator.process([
        item(phone: '+919876543211', savedName: null),
      ]);
      await Future<void>.delayed(Duration.zero);
      await coordinator.process([item()]);
      book.pending!.complete([doctor]);
      await task;
      expect(api.attached, hasLength(1));
      expect(api.attached.single['mode'], 'automatic');
    },
  );
  test('permission denial and account opt-out never read contacts', () async {
    for (final disabled in [true, false]) {
      final api = Api()..enabled = !disabled;
      final book = Book()..allowed = false;
      await ContactMatchingCoordinator(
        api: api,
        book: book,
        isCurrent: () => true,
        onSaved: (_) {},
      ).process([item()]);
      expect(book.reads, 0);
      expect(api.attached, isEmpty);
    }
  });
  test(
    'removal, existing number and other kinds never invoke contact lookup',
    () async {
      final api = Api(), book = Book();
      await ContactMatchingCoordinator(
        api: api,
        book: book,
        isCurrent: () => true,
        onSaved: (_) {},
      ).process([
        item(removed: true),
        item(phone: '+919876543211'),
        item(kind: 'place'),
      ]);
      expect(book.reads, 0);
      expect(api.attached, isEmpty);
    },
  );
  test('stop, account change or revoked permission while reading prevents attachment', () async {
    for (final change in ['stop', 'account', 'permission']) {
      final api = Api(),
          book = Book()..pending = Completer<List<LocalContact>>();
      var current = true;
      final coordinator = ContactMatchingCoordinator(
        api: api,
        book: book,
        isCurrent: () => current,
        onSaved: (_) => fail('Late attachment'),
      );
      final task = coordinator.process([item()]);
      await Future<void>.delayed(Duration.zero);
      expect(book.reads, 1);
      if (change == 'stop') coordinator.stop();
      if (change == 'account') current = false;
      if (change == 'permission') book.allowed = false;
      book.pending!.complete([doctor]);
      await task;
      expect(api.attached, isEmpty);
    }
  });
}
