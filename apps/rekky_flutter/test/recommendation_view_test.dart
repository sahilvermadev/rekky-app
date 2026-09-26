import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/rekky_api.dart';
import 'package:rekky_flutter/recommendation_view.dart';

RekkyItem fixture() {
  final wire = jsonDecode(
    File('../../contracts/rekky/v1/fixtures/wire.json').readAsStringSync(),
  );
  final example = (wire['examples'] as List).firstWhere(
    (e) => e['name'] == 'structured_recommendation',
  );
  return RekkyItem.fromJson(example['body']['item'] as Map<String, dynamic>);
}

void main() {
  test('structured recommendation consumes the shared wire fixture', () {
    final item = fixture();
    expect(item.recommendation!.primaryLocation, 'Pune');
    expect(
      item.recommendation!.cautions.single.text,
      'Small portions; order a few plates.',
    );
    expect(item.recommendation!.experienceLabel, 'Your experience');
  });
  for (final expanded in [false, true]) {
    testWidgets(
      'caveats remain visible in ${expanded ? 'detail' : 'preview'} at large text sizes',
      (tester) async {
        tester.view.physicalSize = const Size(360, 640);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: MediaQuery(
                data: const MediaQueryData(textScaler: TextScaler.linear(2)),
                child: SingleChildScrollView(
                  child: Padding(
                    padding: const EdgeInsets.all(16),
                    child: RecommendationView(
                      item: fixture(),
                      expanded: expanded,
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
        expect(
          find.text('Small portions; order a few plates.'),
          findsOneWidget,
        );
        expect(
          find.textContaining('current price unverified'),
          expanded ? findsWidgets : findsNothing,
        );
        expect(tester.takeException(), isNull);
      },
    );
  }
  test('untried and hearsay are not labelled as firsthand', () {
    for (final (experience, label) in [
      ('interest', 'Not tried yet'),
      ('secondhand', 'Heard from others'),
    ]) {
      final r = RekkyRecommendation(
        summary: 'Saved for later.',
        shelf: 'People & services',
        experience: experience,
        observations: [],
        locations: [],
        useCases: [],
      );
      expect(r.experienceLabel, label);
    }
  });
}
