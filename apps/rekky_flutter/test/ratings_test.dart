import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/rekky_api.dart';
import 'package:rekky_flutter/recommendation_view.dart';

import 'recommendation_editor_test.dart' show openEditor, select, enter;
import 'recommendation_view_test.dart' show fixture;

Map<String, dynamic> ratings() => jsonDecode(
  File('../../contracts/rekky/v1/fixtures/ratings.json').readAsStringSync(),
);
RekkyItem rated(String type) {
  final item = fixture();
  final r = item.recommendation!;
  return RekkyItem(
    id: item.id,
    captureId: item.captureId,
    subject: item.subject,
    body: item.body,
    visibility: item.visibility,
    revision: item.revision,
    createdAt: item.createdAt,
    recommendation: RekkyRecommendation(
      summary: r.summary,
      shelf: r.shelf,
      experience: r.experience,
      entityKind: r.entityKind,
      observations: r.observations,
      locations: r.locations,
      useCases: r.useCases,
      rating: RecommendationRating.parse(ratings()[type]),
    ),
  );
}

void main() {
  test('wire scores distinguish zero, absent, estimated and spoken', () {
    expect(rated('inferred').recommendation!.rating!.estimated, isTrue);
    expect(rated('spoken').recommendation!.rating!.label, '9');
    expect(rated('user').recommendation!.rating!.value, 0);
    expect(rated('unrated').recommendation!.rating, isNull);
    for (final invalid in [
      null,
      {},
      {'value': 11, 'origin': 'inferred'},
      {'value': 4, 'origin': 'unknown'},
    ]) {
      expect(RecommendationRating.parse(invalid), isNull);
    }
  });
  for (final type in ['inferred', 'spoken', 'user', 'unrated']) {
    testWidgets(
      'detail rating $type fits large text and compact card stays small',
      (tester) async {
        tester.view.physicalSize = const Size(320, 640);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        final item = rated(type);
        for (final expanded in [true, false]) {
          await tester.pumpWidget(
            MaterialApp(
              home: Scaffold(
                body: MediaQuery(
                  data: const MediaQueryData(textScaler: TextScaler.linear(2)),
                  child: SingleChildScrollView(
                    child: Padding(
                      padding: const EdgeInsets.all(20),
                      child: RecommendationView(item: item, expanded: expanded),
                    ),
                  ),
                ),
              ),
            ),
          );
          expect(
            find.text('Estimated'),
            expanded && type == 'inferred' ? findsOneWidget : findsNothing,
          );
          expect(
            find.byIcon(Icons.star_rounded),
            expanded && type != 'unrated' ? findsOneWidget : findsNothing,
          );
          if (expanded && type != 'unrated') {
            expect(
              find.text('${item.recommendation!.rating!.label}/10'),
              findsOneWidget,
            );
          }
          expect(tester.takeException(), isNull);
        }
      },
    );
  }
  testWidgets(
    'editing can set, remove and retain a score without silently confirming AI',
    (tester) async {
      for (final choice in [
        '4.5/10',
        'Remove rating',
        'Keep 7/10 · estimated',
      ]) {
        Map<String, dynamic>? sent;
        await openEditor(
          tester,
          item: rated('inferred'),
          save: (v) async {
            sent = v;
            return rated('inferred');
          },
        );
        if (choice.startsWith('Keep')) {
          await select(tester, 'Who can see this', 'Friends');
        } else {
          final control = find.widgetWithText(
            DropdownButtonFormField<String>,
            'Rating',
          );
          await tester.ensureVisible(control);
          await tester.tap(control);
          await tester.pumpAndSettle();
          await tester.scrollUntilVisible(
            find.text(choice),
            120,
            scrollable: find.byType(Scrollable).last,
          );
          await tester.tap(find.text(choice).last);
          await tester.pumpAndSettle();
        }
        await tester.tap(find.text('Save'));
        await tester.pumpAndSettle();
        expect(
          sent!['rating'],
          choice == '4.5/10'
              ? {'mode': 'set', 'value': 4.5}
              : choice == 'Remove rating'
              ? {'mode': 'none'}
              : {'mode': 'keep'},
        );
      }
    },
  );
  testWidgets('changing the opinion explains automatic rating invalidation', (
    tester,
  ) async {
    await openEditor(tester, item: rated('inferred'));
    await enter(tester, 'Recommendation', 'Disappointing overall.');
    expect(find.text('Clear previous rating'), findsOneWidget);
    expect(
      find.textContaining('This edit changes the experience.'),
      findsOneWidget,
    );
  });
}
