import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/recommendation_editor.dart';
import 'package:rekky_flutter/recommendation_maps_action.dart';
import 'package:rekky_flutter/rekky_api.dart';

import 'recommendation_view_test.dart' show fixture;

Map<String, dynamic> editWire() => jsonDecode(
  File('../../contracts/rekky/v1/fixtures/recommendation_edit.json')
      .readAsStringSync(),
) as Map<String, dynamic>;
RekkyItem editedFixture({bool custom = false}) {
  final input = editWire();
  final base = fixture('categorized_recommendation');
  final old = base.recommendation!;
  return RekkyItem(
    id: base.id,
    captureId: base.captureId,
    subject: base.subject,
    body: base.body,
    visibility: 'private',
    revision: 2,
    createdAt: base.createdAt,
    recommendation: RekkyRecommendation.fromJson({
      ...input,
      'version': 2,
      'origin': 'user',
      'shelf': 'Places',
      'classification': {
        'types': old.classification!.types
            .map(
              (c) => {'id': c.id, 'label': c.label, 'dimension': c.dimension},
            )
            .toList(),
        'facets': old.classification!.facets
            .map(
              (c) => {'id': c.id, 'label': c.label, 'dimension': c.dimension},
            )
            .toList(),
        'descriptors': [],
        'display_label': 'Italian restaurant',
      },
      if (!custom) 'destination': {'mode': 'auto', 'url': '', 'label': ''},
    }),
  );
}

Future<void> openEditor(
  WidgetTester tester, {
  RekkyItem? item,
  Future<RekkyItem> Function(Map<String, dynamic>)? save,
  double scale = 1,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(scale)),
        child: child!,
      ),
      home: Builder(
        builder: (context) => Scaffold(
          body: TextButton(
            child: const Text('Open editor'),
            onPressed: () => Navigator.push<RekkyItem>(
              context,
              MaterialPageRoute(
                builder: (_) => RecommendationEditor(
                  item: item ?? editedFixture(),
                  loadConcepts: () async {
                    final catalog = jsonDecode(
                      File('../../contracts/rekky/v1/taxonomy/vocabulary.json')
                          .readAsStringSync(),
                    );
                    return (catalog['concepts'] as List)
                        .map((c) => CategoryConcept.fromJson(c))
                        .toList();
                  },
                  onSave: save ?? (_) async => editedFixture(),
                ),
              ),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('Open editor'));
  await tester.pumpAndSettle();
}

Finder field(String label) => find.widgetWithText(TextFormField, label);
Future<void> enter(WidgetTester tester, String label, String text) async {
  await tester.ensureVisible(field(label));
  await tester.enterText(field(label), text);
  await tester.pump();
}

Future<void> select(WidgetTester tester, String label, String value) async {
  final control = find.widgetWithText(DropdownButtonFormField<String>, label);
  await tester.ensureVisible(control);
  await tester.tap(control);
  await tester.pumpAndSettle();
  await tester.tap(find.text(value).last);
  await tester.pumpAndSettle();
}

void main() {
  test(
    'manual destination and attribution consume the shared edit contract',
    () {
      final saved = editedFixture(custom: true);
      expect(saved.recommendation!.experienceLabel, 'Heard from Priya');
      expect(saved.recommendation!.origin, 'user');
      expect(
        recommendationDestination(saved).toString(),
        editWire()['destination']['url'],
      );
      expect(recommendationMapsSearch(saved), isNull);
    },
  );
  testWidgets(
    'save sends one complete staged edit including visibility and preserves wording',
    (tester) async {
      Map<String, dynamic>? received;
      var calls = 0;
      final pending = Completer<RekkyItem>();
      await openEditor(
        tester,
        save: (value) {
          calls++;
          received = value;
          return pending.future;
        },
      );
      expect(
        tester
            .widget<TextButton>(find.widgetWithText(TextButton, 'Save'))
            .onPressed,
        isNull,
      );
      await enter(tester, 'Name', 'Lantern Annex');
      await enter(tester, 'Recommendation', 'My exact wording — keep it.');
      await select(tester, 'Who can see this', 'Friends');
      expect(calls, 0);
      await tester.tap(find.text('Save'));
      await tester.pump();
      expect(calls, 1);
      expect(received!['subject'], 'Lantern Annex');
      expect(received!['summary'], 'My exact wording — keep it.');
      expect(received!['visibility'], 'friends');
      expect(received!['observations'], editWire()['observations']);
      expect(received!['locations'], editWire()['locations']);
      expect(received!['attribution'], 'Priya');
      await tester.tap(find.text('Saving…'));
      expect(calls, 1);
      pending.complete(editedFixture());
      await tester.pumpAndSettle();
      expect(find.byType(RecommendationEditor), findsNothing);
    },
  );
  testWidgets(
    'discard prompt protects changes and conflict preserves the draft',
    (tester) async {
      await openEditor(
        tester,
        save: (_) async => throw const ApiFailure(
          'revision_conflict',
          'This recommendation changed. Reopen it before editing; your draft has not been saved.',
          409,
        ),
      );
      await enter(tester, 'Name', 'Keep this draft');
      await tester.tap(find.byTooltip('Close editor'));
      await tester.pumpAndSettle();
      expect(find.text('Discard changes?'), findsOneWidget);
      await tester.tap(find.text('Keep editing'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(
        find.textContaining('your draft has not been saved'),
        findsOneWidget,
      );
      expect(find.text('Keep this draft'), findsOneWidget);
      await tester.tap(find.byTooltip('Close editor'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Discard'));
      await tester.pumpAndSettle();
      expect(find.byType(RecommendationEditor), findsNothing);
    },
  );
  testWidgets(
    'existing link requires confirmation when identity changes; no link is a deliberate override',
    (tester) async {
      Map<String, dynamic>? received;
      await openEditor(
        tester,
        item: editedFixture(custom: true),
        save: (value) async {
          received = value;
          return editedFixture();
        },
      );
      await enter(tester, 'Name', 'Another branch');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(received, isNull);
      expect(
        find.text('Check the existing link below, or remove it.'),
        findsOneWidget,
      );
      await select(tester, 'Link', 'No link');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(received!['destination'], {
        'mode': 'none',
        'url': '',
        'label': '',
      });
    },
  );
  testWidgets('category Apply stages IDs without saving recommendation', (
    tester,
  ) async {
    Map<String, dynamic>? received;
    await openEditor(
      tester,
      save: (value) async {
        received = value;
        return editedFixture();
      },
    );
    await tester.ensureVisible(find.text('Type and attributes'));
    await tester.tap(find.text('Type and attributes'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilterChip, 'Restaurant'));
    await tester.tap(find.widgetWithText(FilterChip, 'Bar'));
    await tester.tap(find.text('Apply'));
    await tester.pumpAndSettle();
    expect(received, isNull);
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(received!['types'], ['place.bar']);
  });
  testWidgets(
    'removing a detail preserves sibling values and allows adding a plain detail',
    (tester) async {
      Map<String, dynamic>? received;
      await openEditor(
        tester,
        save: (value) async {
          received = value;
          return editedFixture();
        },
      );
      await tester.ensureVisible(find.byTooltip('Remove Detail 1'));
      await tester.tap(find.byTooltip('Remove Detail 1'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Add detail'));
      await tester.tap(find.text('Add detail'));
      await tester.pumpAndSettle();
      final last = field('Detail').last;
      await tester.ensureVisible(last);
      await tester.enterText(last, 'Takeaway is available.');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(received!['observations'], [
        editWire()['observations'][1],
        {'kind': 'context', 'text': 'Takeaway is available.'},
      ]);
    },
  );
  for (final width in [320.0, 375.0, 414.0, 768.0]) {
    for (final scale in [1.0, 2.0]) {
      testWidgets(
        'editor fits $width at $scale with keyboard and footer controls',
        (tester) async {
          tester.view.physicalSize = Size(width, 800);
          tester.view.devicePixelRatio = 1;
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          await openEditor(tester, scale: scale);
          await enter(tester, 'Recommendation', 'A readable correction.');
          tester.view.viewInsets = const FakeViewPadding(bottom: 300);
          await tester.pump();
          expect(tester.takeException(), isNull);
          await tester.ensureVisible(find.text('Useful link'));
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          tester.view.resetViewInsets();
          await tester.pump();
          await tester.tap(find.byTooltip('Close editor'));
          await tester.pumpAndSettle();
          await tester.tap(find.text('Discard'));
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
        },
      );
    }
  }
}
