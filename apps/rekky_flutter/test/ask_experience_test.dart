import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/ask_answer.dart';
import 'package:rekky_flutter/ask_experience.dart';
import 'package:rekky_flutter/rekky_api.dart';
import 'package:rekky_flutter/rekky_theme.dart';

AskAnswer fixture() => AskAnswer.fromJson(
  jsonDecode(
    File('../../contracts/rekky/v1/fixtures/ask_answer.json')
        .readAsStringSync(),
  ) as Map<String, dynamic>,
);

class FakeAsk extends RekkyApi {
  FakeAsk() : super('http://unused');
  final requestIds = <String>[];
  final pending = <Completer<AskAnswer>>[];
  int cancelled = 0, reads = 0, pages = 0;
  @override
  Future<AskAnswer> askAgent(String question, String requestId) {
    requestIds.add(requestId);
    final c = Completer<AskAnswer>();
    pending.add(c);
    return c.future;
  }

  @override
  Future<void> cancelAsk(String id) async {
    cancelled++;
  }

  @override
  Future<RekkyItem> item(String id) async {
    reads++;
    return fixture().results.first.item;
  }

  @override
  Future<AskAnswer> askPage(String id, int offset) async {
    pages++;
    return fixture();
  }
}

Future<void> submit(WidgetTester t, String text) async {
  await t.enterText(find.byType(TextField), text);
  await t.testTextInput.receiveAction(TextInputAction.search);
  await t.pump();
}

void main() {
  test('shared Ask contract preserves evidence and caveats', () {
    final a = fixture();
    expect(a.results.single.caveat, 'The tables are small.');
    expect(a.results.single.evidence.single.id, 'summary');
    expect(a.nextOffset, isNull);
  });
  testWidgets(
    'cancel and replacement ignore late results; open fetches current item',
    (t) async {
      final api = FakeAsk();
      var opened = 0;
      await t.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: AskExperience(
              api: api,
              onOpen: (_) async {
                opened++;
              },
            ),
          ),
        ),
      );
      await submit(t, 'quiet dinner');
      expect(api.pending.length, 1);
      await t.tap(find.text('Cancel'));
      await t.pump();
      expect(api.cancelled, 1);
      api.pending[0].complete(fixture());
      await t.pumpAndSettle();
      expect(find.text('Lantern Kitchen'), findsNothing);
      await submit(t, 'somewhere to talk');
      api.pending[1].complete(fixture());
      await t.pumpAndSettle();
      expect(find.text('Lantern Kitchen'), findsOneWidget);
      expect(find.text('The tables are small.'), findsOneWidget);
      await t.ensureVisible(find.text('Lantern Kitchen'));
      await t.tap(find.text('Lantern Kitchen'));
      await t.pumpAndSettle();
      expect(api.reads, 1);
      expect(opened, 1);
      expect(api.pages, 1);
      await t.ensureVisible(find.text('Why this fits'));
      await t.tap(find.text('Why this fits'));
      await t.pumpAndSettle();
      expect(
        find.text('“We could talk easily. The tables are small.”'),
        findsOneWidget,
      );
    },
  );
  for (final width in [320.0, 375.0, 414.0, 768.0]) {
    testWidgets('answer at $width with large text stays scrollable', (t) async {
      t.view.physicalSize = Size(width, 800);
      t.view.devicePixelRatio = 1;
      addTearDown(t.view.resetPhysicalSize);
      addTearDown(t.view.resetDevicePixelRatio);
      final api = FakeAsk();
      await t.pumpWidget(
        MaterialApp(
          theme: RekkyTheme.build(Brightness.dark),
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: const TextScaler.linear(2)),
            child: child!,
          ),
          home: Scaffold(
            body: AskExperience(api: api, onOpen: (_) async {}),
          ),
        ),
      );
      await submit(t, 'quiet dinner');
      api.pending.single.complete(fixture());
      await t.pumpAndSettle();
      await t.ensureVisible(find.text('Why this fits'));
      expect(t.takeException(), isNull);
    });
  }
  testWidgets('failure leaves previous answer and retry uses same operation', (
    t,
  ) async {
    final api = FakeAsk();
    await t.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AskExperience(api: api, onOpen: (_) async {}),
        ),
      ),
    );
    await submit(t, 'dinner');
    api.pending[0].complete(fixture());
    await t.pumpAndSettle();
    await submit(t, 'another request');
    api.pending[1].completeError(TimeoutException('network response lost'));
    await t.pumpAndSettle();
    expect(find.text('Lantern Kitchen'), findsOneWidget);
    expect(
      find.text('Couldn’t connect. Your previous answer is still here.'),
      findsOneWidget,
    );
    await t.tap(find.text('Try again'));
    await t.pump();
    expect(api.requestIds[1], api.requestIds[2]);
    api.pending[2].complete(fixture());
    await t.pumpAndSettle();
  });
}
