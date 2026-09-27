import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/category_editor.dart';
import 'package:rekky_flutter/rekky_api.dart';

RekkyItem itemFixture() {
  final wire = jsonDecode(
    File('../../contracts/rekky/v1/fixtures/wire.json').readAsStringSync(),
  );
  final example = (wire['examples'] as List).firstWhere(
    (e) => e['name'] == 'categorized_recommendation',
  );
  return RekkyItem.fromJson(example['body']['item'] as Map<String, dynamic>);
}

List<CategoryConcept> concepts() {
  final vocabulary = jsonDecode(
    File('../../contracts/rekky/v1/taxonomy/vocabulary.json')
        .readAsStringSync(),
  );
  return (vocabulary['concepts'] as List)
      .map((c) => CategoryConcept.fromJson(c as Map<String, dynamic>))
      .toList();
}

void main() {
  testWidgets('learned category IDs render and save without an app update', (
    tester,
  ) async {
    final item = RekkyItem.fromJson(
      jsonDecode(
        File('../../contracts/rekky/v1/fixtures/learned_recommendation.json')
            .readAsStringSync(),
      ),
    );
    final concept = CategoryConcept.fromJson({
      'id': 'learned.ce7de4bb5e7563576b814de6',
      'label': 'Fountain pen restorer',
      'dimension': 'type',
      'dimension_label': 'Type',
      'entity_kinds': ['person_service'],
      'aliases': ['pen restoration specialist'],
      'definition': 'Repairs fountain pens.',
      'evidence_focus': ['work_performed'],
    });
    List<String>? saved;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: CategoryEditor(
            item: item,
            concepts: [...concepts(), concept],
            onSave: (types, _) async {
              saved = types;
            },
          ),
        ),
      ),
    );
    expect(item.recommendation!.categoryLabel, 'Fountain pen restorer');
    final chip = find.widgetWithText(FilterChip, 'Fountain pen restorer');
    expect(tester.widget<FilterChip>(chip).selected, isTrue);
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(saved, ['learned.ce7de4bb5e7563576b814de6']);
    expect(tester.takeException(), isNull);
  });
  testWidgets(
    'category edits send stable IDs and preserve choices on conflict',
    (tester) async {
      List<String>? savedTypes, savedFacets;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: CategoryEditor(
              item: itemFixture(),
              concepts: concepts(),
              onSave: (types, facets) async {
                savedTypes = types;
                savedFacets = facets;
                throw const ApiFailure(
                  'revision_conflict',
                  'Item changed; refresh before editing',
                  409,
                );
              },
            ),
          ),
        ),
      );
      expect(find.text('General doctor'), findsNothing);
      await tester.tap(find.text('Restaurant'));
      await tester.tap(find.text('Bar'));
      await tester.tap(find.text('Italian'));
      await tester.tap(find.text('Thai'));
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(savedTypes, ['place.bar']);
      expect(savedFacets, ['cuisine.thai']);
      expect(find.text('Item changed; refresh before editing'), findsOneWidget);
      final bar = tester.widget<FilterChip>(
        find.widgetWithText(FilterChip, 'Bar'),
      );
      expect(bar.selected, isTrue);
    },
  );
  for (final width in [320.0, 375.0, 414.0, 768.0]) {
    testWidgets('category editor scrolls at $width and 200% text size', (
      tester,
    ) async {
      tester.view.physicalSize = Size(width, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: MediaQuery(
              data: const MediaQueryData(textScaler: TextScaler.linear(2)),
              child: CategoryEditor(
                item: itemFixture(),
                concepts: concepts(),
                onSave: (_, _) async {},
              ),
            ),
          ),
        ),
      );
      expect(find.text('Save'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  }
}
