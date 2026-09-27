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
    expect(contact.keys.toSet(), {'phone', 'origin'});
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
        isNull,
      );
      expect(
        automaticContactPhone('Maya Rao', [
          const LocalContact(
            'Maya Rao dentist',
            ['+919876543210'],
            first: 'Maya',
            last: 'Rao',
          ),
        ]),
        isNull,
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
    'batch sends only attached phone with generation; no address book fields',
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
        {'mode': 'automatic', 'phone': '+919876543210', 'generation': 4},
      ]);
      expect(saved, hasLength(1));
      await coordinator.process([item()]);
      expect(api.attached, hasLength(1));
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
