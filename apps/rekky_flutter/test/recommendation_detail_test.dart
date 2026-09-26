import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/recommendation_detail.dart';
import 'package:rekky_flutter/rekky_api.dart';

import 'recommendation_view_test.dart' show fixture;

const original = RekkySource(
  text: 'The private original note.',
  kind: 'transcript',
  revision: 3,
);

RekkyItem revised(RekkyItem item, String visibility) => RekkyItem(
  id: item.id,
  captureId: item.captureId,
  subject: item.subject,
  body: item.body,
  visibility: visibility,
  revision: item.revision + 1,
  createdAt: item.createdAt,
  recommendation: item.recommendation,
  needsReview: item.needsReview,
);

Future<void> openSheet(
  WidgetTester tester, {
  Future<RekkySource?> Function()? source,
  Future<RekkyItem> Function(RekkyItem, String)? audience,
  Future<ResolvedPlace?> Function()? place,
  Future<void> Function(RekkyItem)? deleteItem,
  double scale = 1,
  RekkyItem? item,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      theme: ThemeData(useMaterial3: true),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(scale)),
        child: child!,
      ),
      home: Builder(
        builder: (context) => Scaffold(
          body: TextButton(
            child: const Text('Open'),
            onPressed: () => showModalBottomSheet<void>(
              context: context,
              isScrollControlled: true,
              showDragHandle: true,
              builder: (context) => ConstrainedBox(
                constraints: BoxConstraints(
                  maxHeight: MediaQuery.sizeOf(context).height * .9,
                ),
                child: RecommendationDetailSheet(
                  item: item ?? fixture('categorized_recommendation'),
                  loadSource: source ?? () async => original,
                  changeAudience:
                      audience ?? (item, value) async => revised(item, value),
                  loadPlace: place,
                  deleteItem: deleteItem ?? (_) async {},
                  editRecommendation: (_) {},
                  refineItem: (_) {},
                ),
              ),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.text('Open'));
  await tester.pump(const Duration(milliseconds: 500));
  await tester.pump(const Duration(milliseconds: 500));
}

void main() {
  testWidgets(
    'opens immediately while the quote loads once and can finish after close',
    (tester) async {
      var calls = 0;
      final pending = Completer<RekkySource?>();
      await openSheet(
        tester,
        source: () {
          calls++;
          return pending.future;
        },
      );
      expect(calls, 1);
      expect(
        find.text(
          fixture('categorized_recommendation').recommendation!.summary,
        ),
        findsOneWidget,
      );
      expect(calls, 1);
      expect(find.byType(LinearProgressIndicator), findsOneWidget);
      await tester.tap(find.byTooltip('Close recommendation'));
      await tester.pumpAndSettle();
      pending.complete(original);
      await tester.pump();
      expect(tester.takeException(), isNull);
      expect(find.text(original.text), findsNothing);
    },
  );

  testWidgets(
    'source failure stays below the recommendation and retry recovers',
    (tester) async {
      var calls = 0;
      await openSheet(
        tester,
        source: () async {
          if (++calls == 1) throw Exception('private internal detail');
          return original;
        },
      );
      await tester.pumpAndSettle();
      expect(
        find.textContaining('Couldn’t load your original'),
        findsOneWidget,
      );
      expect(find.textContaining('private internal'), findsNothing);
      expect(
        find.text(fixture('categorized_recommendation').subject),
        findsOneWidget,
      );
      await tester.ensureVisible(find.text('Retry'));
      await tester.tap(find.text('Retry'));
      await tester.pumpAndSettle();
      expect(find.text(original.text), findsOneWidget);
      expect(calls, 2);
      await tester.pumpAndSettle();
      expect(calls, 2);
    },
  );

  testWidgets(
    'audience waits for acknowledgement, then uses returned revision for deletion',
    (tester) async {
      final pending = Completer<RekkyItem>();
      final item = fixture('categorized_recommendation');
      int? deletedRevision;
      await openSheet(
        tester,
        audience: (_, _) => pending.future,
        deleteItem: (current) async {
          deletedRevision = current.revision;
        },
      );
      await tester.tap(find.byTooltip('Who can see this'));
      await tester.pumpAndSettle();
      await tester.tap(
        find.byWidgetPredicate(
          (widget) =>
              widget is CheckedPopupMenuItem<String> &&
              widget.value == 'friends',
        ),
      );
      await tester.pump(const Duration(milliseconds: 500));
      await tester.pump(const Duration(milliseconds: 500));
      expect(find.text('Only me'), findsOneWidget);
      pending.complete(revised(item, 'friends'));
      await tester.pumpAndSettle();
      expect(find.text('Friends'), findsOneWidget);
      expect(find.text('Delete recommendation'), findsNothing);
      await tester.tap(find.byTooltip('Recommendation options'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Delete recommendation'));
      await tester.pumpAndSettle();
      expect(deletedRevision, isNull);
      await tester.tap(find.text('Delete').last);
      await tester.pumpAndSettle();
      expect(deletedRevision, item.revision + 1);
      expect(find.byType(RecommendationDetailSheet), findsNothing);
    },
  );

  testWidgets(
    'failed audience update keeps the current audience and local error',
    (tester) async {
      await openSheet(
        tester,
        audience: (_, _) async => throw const ApiFailure(
          'revision_conflict',
          'This recommendation changed. Reopen it and try again.',
          409,
        ),
      );
      await tester.tap(find.byTooltip('Who can see this'));
      await tester.pumpAndSettle();
      await tester.tap(
        find.byWidgetPredicate(
          (widget) =>
              widget is CheckedPopupMenuItem<String> &&
              widget.value == 'friends',
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Only me'), findsOneWidget);
      expect(find.textContaining('Reopen it'), findsOneWidget);
    },
  );

  testWidgets('quote is visible without headings or source deletion controls', (
    tester,
  ) async {
    await openSheet(tester);
    expect(find.text(original.text), findsOneWidget);
    for (final label in [
      'Your experience',
      'More from your note',
      'Original note',
      'Private · only you',
      'Delete original note',
    ]) {
      expect(find.text(label), findsNothing);
    }
  });

  testWidgets(
    'missing source leaves no empty heading or removed-note message',
    (tester) async {
      await openSheet(tester, source: () async => null);
      expect(find.text('Original note removed.'), findsNothing);
      expect(find.text('Original note'), findsNothing);
    },
  );

  testWidgets(
    'resolved address replaces spoken locality without blocking the quote',
    (tester) async {
      final pending = Completer<ResolvedPlace?>();
      await openSheet(tester, place: () => pending.future);
      expect(find.text('Pune'), findsOneWidget);
      expect(find.text(original.text), findsOneWidget);
      pending.complete(
        const ResolvedPlace(id: 'sample-id', address: '12 Sample Road, Pune'),
      );
      await tester.pumpAndSettle();
      expect(find.text('12 Sample Road, Pune'), findsOneWidget);
      expect(find.text('Pune'), findsNothing);
      expect(find.text('Google Maps'), findsOneWidget);
      expect(find.text('Search Maps'), findsNothing);
    },
  );

  testWidgets(
    'review notice opens the original without claiming a specific error',
    (tester) async {
      final saved = fixture('categorized_recommendation');
      final review = RekkyItem(
        id: saved.id,
        captureId: saved.captureId,
        subject: saved.subject,
        body: saved.body,
        visibility: saved.visibility,
        revision: saved.revision,
        createdAt: saved.createdAt,
        recommendation: saved.recommendation,
        needsReview: true,
      );
      var calls = 0;
      await openSheet(
        tester,
        item: review,
        source: () async {
          calls++;
          return original;
        },
      );
      expect(find.text('This saved note may be incomplete.'), findsOneWidget);
      expect(calls, 1);
      await tester.ensureVisible(find.text('View original note'));
      await tester.tap(find.text('View original note'));
      await tester.pumpAndSettle();
      expect(calls, 1);
      expect(find.text(original.text), findsOneWidget);
      expect(find.text('This saved note may be incomplete.'), findsOneWidget);
    },
  );

  for (final width in [320.0, 375.0, 414.0, 768.0]) {
    for (final scale in [1.0, 2.0]) {
      testWidgets(
        'full reading sheet at $width / $scale remains scrollable with reachable controls',
        (tester) async {
          tester.view.physicalSize = Size(width, 800);
          tester.view.devicePixelRatio = 1;
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          await openSheet(tester, scale: scale);
          expect(tester.takeException(), isNull);
          expect(find.text('Cuisine'), findsNothing);
          expect(find.text('Pune'), findsOneWidget);
          final map = tester.getTopLeft(find.text('Search Maps')).dy;
          final summary = tester
              .getTopLeft(
                find.text(
                  fixture('categorized_recommendation').recommendation!.summary,
                ),
              )
              .dy;
          expect(map, lessThan(summary));
          await tester.pumpAndSettle();
          await tester.ensureVisible(find.text(original.text));
          expect(tester.takeException(), isNull);
          await tester.tap(find.byTooltip('Close recommendation'));
          await tester.pumpAndSettle();
          expect(find.byType(RecommendationDetailSheet), findsNothing);
        },
      );
    }
  }
}
