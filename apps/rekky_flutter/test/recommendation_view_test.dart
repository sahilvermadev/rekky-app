import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/rekky_api.dart';
import 'package:rekky_flutter/recommendation_view.dart';

RekkyItem fixture([String name = 'structured_recommendation']) {
  final wire = jsonDecode(
    File('../../contracts/rekky/v1/fixtures/wire.json').readAsStringSync(),
  );
  final example = (wire['examples'] as List).firstWhere(
    (e) => e['name'] == name,
  );
  return RekkyItem.fromJson(example['body']['item'] as Map<String, dynamic>);
}

void main() {
  testWidgets(
    'canonical type and cuisine replace broad shelf on compact cards',
    (tester) async {
      final item = fixture('categorized_recommendation');
      expect(item.recommendation!.categoryLabel, 'Italian restaurant');
      expect(
        item.recommendation!.classification!.types.single.id,
        'place.restaurant',
      );
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: RecommendationCard(item: item, onTap: () {}),
          ),
        ),
      );
      expect(find.text('Italian restaurant · Pune'), findsOneWidget);
      expect(find.text('Places · Pune'), findsNothing);
      expect(find.text(item.recommendation!.summary), findsNothing);
    },
  );
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
      'detail preserves caveats and preview signals them at large text sizes: $expanded',
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
          expanded ? findsOneWidget : findsNothing,
        );
        expect(find.text('Caution'), expanded ? findsNothing : findsOneWidget);
        expect(
          find.text(fixture().recommendation!.summary),
          expanded ? findsOneWidget : findsNothing,
        );
        expect(
          find.textContaining('current price unverified'),
          expanded ? findsWidgets : findsNothing,
        );
        expect(tester.takeException(), isNull);
      },
    );
  }
  for (final width in [320.0, 375.0, 414.0, 768.0]) {
    for (final scale in [1.0, 2.0]) {
      testWidgets('compact card and detail fit width $width at scale $scale', (
        tester,
      ) async {
        tester.view.physicalSize = Size(width, 800);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        var opened = false;
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: MediaQuery(
                data: MediaQueryData(textScaler: TextScaler.linear(scale)),
                child: SingleChildScrollView(
                  padding: const EdgeInsets.all(16),
                  child: RecommendationCard(
                    item: fixture(),
                    onTap: () => opened = true,
                  ),
                ),
              ),
            ),
          ),
        );
        expect(find.text(fixture().recommendation!.summary), findsNothing);
        if (scale == 1) {
          expect(
            tester.getSize(find.byType(RecommendationCard)).height,
            lessThan(140),
          );
        }
        await tester.tap(find.text(fixture().subject));
        expect(opened, isTrue);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: MediaQuery(
                data: MediaQueryData(textScaler: TextScaler.linear(scale)),
                child: SingleChildScrollView(
                  padding: const EdgeInsets.all(24),
                  child: RecommendationView(item: fixture(), expanded: true),
                ),
              ),
            ),
          ),
        );
        expect(find.text('Search Maps'), findsOneWidget);
        expect(tester.takeException(), isNull);
      });
    }
  }
  testWidgets('legacy source passages never appear in compact cards', (
    tester,
  ) async {
    const item = RekkyItem(
      id: 'legacy',
      captureId: 'capture',
      subject: 'Saved subject',
      body: 'A long private source passage',
      visibility: 'private',
      revision: 1,
      createdAt: '',
      needsReview: true,
    );
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: RecommendationCard(item: item, onTap: () {}),
        ),
      ),
    );
    expect(find.text(item.body), findsNothing);
    expect(find.text('Needs review'), findsOneWidget);
    expect(find.text('Saved note'), findsOneWidget);
  });
  testWidgets('praise is grouped on demand while caveats stay visible', (
    tester,
  ) async {
    const item = RekkyItem(
      id: 'grouped',
      captureId: 'capture',
      subject: 'A useful place',
      body: 'Stored evidence',
      visibility: 'private',
      revision: 1,
      createdAt: '',
      recommendation: RekkyRecommendation(
        summary: 'A comfortable place for an afternoon.',
        shelf: 'Places',
        experience: 'secondhand',
        entityKind: 'place',
        observations: [
          RecommendationDetail('praise', 'Comfortable seats.'),
          RecommendationDetail('praise', 'Friendly staff.'),
          RecommendationDetail('praise', 'Friendly staff.'),
          RecommendationDetail(
            'caution',
            'Avoid the stairs if mobility is limited.',
          ),
        ],
        locations: [],
        useCases: [],
      ),
    );
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(
            child: RecommendationView(item: item, expanded: true),
          ),
        ),
      ),
    );
    expect(find.text('Heard from others'), findsOneWidget);
    expect(
      find.text('Avoid the stairs if mobility is limited.'),
      findsOneWidget,
    );
    expect(find.text('What stood out'), findsNothing);
    await tester.tap(find.text('More from your note'));
    await tester.pumpAndSettle();
    expect(find.text('What stood out'), findsOneWidget);
    expect(find.text('Comfortable seats.'), findsOneWidget);
    expect(find.text('Friendly staff.'), findsOneWidget);
  });
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
