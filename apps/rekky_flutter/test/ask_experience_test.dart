import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/ask_answer.dart';
import 'package:rekky_flutter/ask_experience.dart';
import 'package:rekky_flutter/ask_view.dart';
import 'package:rekky_flutter/rekky_api.dart';
import 'package:rekky_flutter/rekky_theme.dart';

AskAnswer fixture({int turn = 1}) => AskAnswer.fromJson({
  ...(jsonDecode(
    File('../../contracts/rekky/v1/fixtures/ask_answer.json')
        .readAsStringSync(),
  ) as Map<String, dynamic>),
  'turn_count': turn,
  if (turn > 1) 'request_id': '44444444-4444-4444-8444-444444444444',
});

AskAnswer comparisonFixture() => AskAnswer.fromJson(
  jsonDecode(
    File('../../contracts/rekky/v1/fixtures/ask_comparison.json')
        .readAsStringSync(),
  ) as Map<String, dynamic>,
);
AskAnswer twoOptions() {
  final c = comparisonFixture();
  return AskAnswer(
    requestId: fixture().requestId,
    title: 'Dinner options',
    intent: 'discovery',
    mode: 'agent',
    clarification: '',
    choices: [],
    location: '',
    results: c.items
        .map(
          (item) => AskResult(
            item: item,
            section: 'supported',
            reason: item.body,
            caveat: '',
            evidence: const [],
          ),
        )
        .toList(),
  );
}

AskAnswer browsableAnswer() {
  final answer = fixture();
  final browse = jsonDecode(
    File('../../contracts/rekky/v1/fixtures/ask_ui.json').readAsStringSync(),
  ) as Map<String, dynamic>;
  return AskAnswer(
    requestId: answer.requestId,
    title: answer.title,
    intent: answer.intent,
    mode: answer.mode,
    clarification: '',
    choices: const [],
    location: '',
    results: answer.results,
    views: [AskView.fromJson(browse['view'] as Map<String, dynamic>)],
  );
}

class FakeAsk extends RekkyApi {
  FakeAsk() : super('http://unused');
  final contexts = <Map<String, dynamic>>[];
  final requestIds = <String>[];
  final pending = <Completer<AskAnswer>>[];
  int cancelled = 0, reads = 0, pages = 0;
  void Function(String)? emitText;
  void Function()? resetText;
  @override
  Future<AskAnswer> askAgentStream(
    String question,
    String requestId, {
    String? scopeCity,
    String? activeViewId,
    String? previousRequestId,
    List<String> selectedItemIds = const [],
    List<String> excludedItemIds = const [],
    required void Function(String) onText,
    required void Function() onReset,
  }) {
    emitText = onText;
    resetText = onReset;
    return askAgent(
      question,
      requestId,
      scopeCity: scopeCity,
      activeViewId: activeViewId,
      previousRequestId: previousRequestId,
      selectedItemIds: selectedItemIds,
      excludedItemIds: excludedItemIds,
    );
  }

  final viewRequests = <AskBrowseSpec>[];
  @override
  Future<AskView> createAskView(AskBrowseSpec spec) async {
    viewRequests.add(spec);
    final fixture = jsonDecode(
      File('../../contracts/rekky/v1/fixtures/ask_ui.json').readAsStringSync(),
    ) as Map<String, dynamic>;
    final view = Map<String, dynamic>.from(
      fixture['view'] as Map<String, dynamic>,
    );
    view['spec'] = spec.toJson();
    return AskView.fromJson(view);
  }

  @override
  Future<AskAnswer> askAgent(
    String question,
    String requestId, {
    String? scopeCity,
    String? activeViewId,
    String? previousRequestId,
    List<String> selectedItemIds = const [],
    List<String> excludedItemIds = const [],
  }) {
    contexts.add({
      'parent': previousRequestId,
      'selected': List.of(selectedItemIds),
      'excluded': List.of(excludedItemIds),
      'question': question,
      'scopeCity': scopeCity,
      'activeViewId': activeViewId,
    });
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
  testWidgets('streams the reply before showing recommendation cards', (
    t,
  ) async {
    final semantics = t.ensureSemantics();
    final api = FakeAsk();
    await t.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AskExperience(api: api, onOpen: (_) async {}),
        ),
      ),
    );
    await submit(t, 'dinner');
    api.emitText!('Here is a dinner option.');
    await t.pump();
    expect(find.text('Here is a dinner option.'), findsOneWidget);
    expect(find.text('Lantern Kitchen'), findsNothing);
    api.pending.single.complete(fixture());
    await t.pumpAndSettle();
    expect(find.text('Lantern Kitchen'), findsOneWidget);
    semantics.dispose();
  });

  testWidgets('Explore filters a native collection and carries it into Ask', (
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
    await submit(t, 'Show me saved services');
    api.pending.single.complete(browsableAnswer());
    await t.pumpAndSettle();
    await t.tap(find.text('Saved services'));
    await t.pumpAndSettle();
    expect(find.text('Saved services'), findsOneWidget);
    expect(find.text('1 saved · Your Library'), findsOneWidget);
    await t.tap(find.text('All types'));
    await t.pumpAndSettle();
    await t.tap(find.text('Fountain pen restorer').last);
    await t.pumpAndSettle();
    expect(api.viewRequests.last.categoryIds, [
      'learned.ce7de4bb5e7563576b814de6',
    ]);
    await t.enterText(
      find.byType(TextField).last,
      'Would this suit a gift repair?',
    );
    await t.testTextInput.receiveAction(TextInputAction.send);
    await t.pump();
    expect(
      api.contexts.last['activeViewId'],
      'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
    );
    expect(api.contexts.last['question'], 'Would this suit a gift repair?');
    api.pending.last.complete(fixture());
    await t.pumpAndSettle();
    expect(find.text('Would this suit a gift repair?'), findsOneWidget);
  });
  testWidgets('fresh Ask home contains only the question entry', (t) async {
    final api = FakeAsk();
    final overlays = <bool>[];
    await t.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AskExperience(
            api: api,
            onOpen: (_) async {},
            onOverlayChanged: overlays.add,
          ),
        ),
      ),
    );
    expect(find.text('Ask Rekky…'), findsOneWidget);
    expect(find.byType(TextField), findsOneWidget);
    expect(find.text('What do you have in mind?'), findsNothing);
    expect(find.text('Explore your saved recommendations'), findsNothing);
    expect(find.text('Who was that taxi driver?'), findsNothing);
    expect(find.textContaining('Ask sends your question'), findsNothing);
    await submit(t, 'Who was that taxi driver?');
    await t.pump();
    expect(overlays.last, true);
    expect(find.byTooltip('Back to Ask'), findsOneWidget);
    expect(find.text('New question'), findsNothing);
    expect(find.byTooltip('Conversation options'), findsNothing);
    expect(find.text('Who was that taxi driver?'), findsOneWidget);
    expect(
      t.widget<TextField>(find.byType(TextField)).controller!.text,
      isEmpty,
    );
    expect(api.contexts.single['question'], 'Who was that taxi driver?');
    api.pending.single.complete(fixture());
    await t.pumpAndSettle();
    await t.tap(find.byTooltip('Back to Ask'));
    await t.pumpAndSettle();
    expect(overlays.last, false);
  });

  testWidgets(
    'reduced motion keeps the response indicator still and labelled',
    (t) async {
      final api = FakeAsk();
      await t.pumpWidget(
        MaterialApp(
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context).copyWith(disableAnimations: true),
            child: child!,
          ),
          home: Scaffold(
            body: AskExperience(api: api, onOpen: (_) async {}),
          ),
        ),
      );
      await submit(t, 'quiet dinner');
      await t.pumpAndSettle();
      expect(find.bySemanticsLabel('Rekky is thinking'), findsOneWidget);
      expect(find.byTooltip('Stop response'), findsOneWidget);
      await t.tap(find.byTooltip('Stop response'));
      await t.pumpAndSettle();
      expect(find.bySemanticsLabel('Rekky is thinking'), findsNothing);
    },
  );

  testWidgets('a new home question drops a cancelled unanswered turn', (
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
    await submit(t, 'first question');
    await t.tap(find.byTooltip('Back to Ask'));
    await t.pump();
    await submit(t, 'second question');
    await t.pump();
    expect(api.cancelled, 1);
    await t.tap(find.byTooltip('Back to Ask'));
    await t.pump();
    expect(find.text('Continue previous conversation'), findsNothing);
    api.pending.last.complete(fixture());
    await t.pumpAndSettle();
  });

  for (final width in [320.0, 375.0, 414.0, 768.0]) {
    testWidgets('centred Ask entry at $width with 200% text fits', (t) async {
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
      expect(t.takeException(), isNull);
      expect(find.byTooltip('Speak a question'), findsOneWidget);
      expect(find.byTooltip('Ask'), findsOneWidget);
    });
  }

  testWidgets(
    'routine availability caveat stays in evidence; exclusion is clear',
    (t) async {
      final api = FakeAsk();
      final original = fixture();
      const caveat =
          'The note is about a past visit and does not establish current availability.';
      final answer = AskAnswer(
        requestId: original.requestId,
        question: 'Something fun to try this weekend',
        title: original.title,
        intent: original.intent,
        mode: original.mode,
        clarification: '',
        choices: const [],
        location: '',
        results: [
          AskResult(
            item: original.results.single.item,
            section: 'supported',
            reason: 'We could talk easily.',
            caveat: caveat,
            evidence: original.results.single.evidence,
          ),
        ],
      );
      await t.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: AskExperience(api: api, onOpen: (_) async {}),
          ),
        ),
      );
      await submit(t, answer.question);
      api.pending.single.complete(answer);
      await t.pumpAndSettle();
      expect(find.text(caveat), findsNothing);
      await t.ensureVisible(find.text('Saved note'));
      await t.pumpAndSettle();
      await t.tap(find.text('Saved note'));
      await t.pumpAndSettle();
      expect(find.text(caveat), findsOneWidget);
      Navigator.of(t.element(find.text('From your saved recommendation')))
          .pop();
      await t.pumpAndSettle();
      await t.ensureVisible(find.byTooltip('More options for Lantern Kitchen'));
      await t.pumpAndSettle();
      await t.tap(find.byTooltip('More options for Lantern Kitchen'));
      await t.pumpAndSettle();
      await t.tap(find.text('Leave this out'));
      await t.pump();
      expect(api.contexts.last['excluded'], [original.results.single.item.id]);
      api.pending.last.complete(fixture(turn: 2));
      await t.pumpAndSettle();
    },
  );

  test(
    'comparison contract keeps cells, unknowns and named source citations',
    () {
      final a = comparisonFixture();
      expect(a.comparison!.items.length, 2);
      expect(
        a.comparison!.dimensions[1].cells.every(
          (c) => c.text == 'Not saved' && c.evidence.isEmpty,
        ),
        isTrue,
      );
      expect(a.comparison!.citations.length, 2);
      expect(a.append(a).comparison!.items.length, 2);
    },
  );
  testWidgets(
    'select two results, compare, inspect evidence and ask a follow-up',
    (t) async {
      final api = FakeAsk();
      await t.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: AskExperience(api: api, onOpen: (_) async {}),
          ),
        ),
      );
      await submit(t, 'dinner options');
      api.pending.single.complete(twoOptions());
      await t.pumpAndSettle();
      await t.tap(find.text('Compare options'));
      await t.pumpAndSettle();
      for (var i = 0; i < 2; i++) {
        await t.ensureVisible(find.byType(Checkbox).at(i));
        await t.pumpAndSettle();
        await t.tap(find.byType(Checkbox).at(i));
        await t.pumpAndSettle();
      }
      await t.tap(find.text('Compare 2'));
      await t.pump();
      expect(
        api.contexts.last['selected'],
        comparisonFixture().items.map((i) => i.id).toList(),
      );
      expect(api.contexts.last['parent'], fixture().requestId);
      api.pending.last.complete(comparisonFixture());
      await t.pumpAndSettle();
      await t.ensureVisible(find.text('Saved evidence'));
      await t.pumpAndSettle();
      await t.tap(find.text('Saved evidence'));
      await t.pumpAndSettle();
      expect(
        find.text('“We could talk easily. The tables are small.”'),
        findsOneWidget,
      );
      Navigator.of(t.element(find.text('From your saved recommendations')))
          .pop();
      await t.pumpAndSettle();
      await t.ensureVisible(find.widgetWithText(FilterChip, 'Ask about').first);
      await t.pumpAndSettle();
      await t.tap(find.widgetWithText(FilterChip, 'Ask about').first);
      await t.pumpAndSettle();
      await submit(t, 'What is missing for this one?');
      expect(api.contexts.last['selected'], [
        comparisonFixture().items.first.id,
      ]);
      api.pending.last.complete(fixture(turn: 3));
      await t.pumpAndSettle();
    },
  );
  for (final width in [320.0, 375.0, 414.0, 768.0]) {
    testWidgets(
      'comparison at $width and 200% text has vertical readable cells',
      (t) async {
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
        await submit(t, 'compare dinner options');
        api.pending.single.complete(comparisonFixture());
        await t.pumpAndSettle();
        await t.ensureVisible(find.text('Not saved').last);
        await t.pumpAndSettle();
        expect(t.takeException(), isNull);
        for (var i = 0; i < 2; i++) {
          await t.ensureVisible(
            find.widgetWithText(FilterChip, 'Ask about').at(i),
          );
          await t.pumpAndSettle();
          await t.tap(find.widgetWithText(FilterChip, 'Ask about').at(i));
          await t.pumpAndSettle();
        }
        await t.enterText(find.byType(TextField), 'Price matters most');
        await t.pump();
        expect(find.text('Compare 2'), findsOneWidget);
        expect(t.takeException(), isNull);
      },
    );
  }
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
      expect(find.bySemanticsLabel('Rekky is thinking'), findsOneWidget);
      expect(find.text('Looking through your recommendations…'), findsNothing);
      await t.tap(find.byTooltip('Stop response'));
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
      await t.pumpAndSettle();
      await t.tap(find.text('Lantern Kitchen'));
      await t.pumpAndSettle();
      expect(api.reads, 1);
      expect(opened, 1);
      expect(api.pages, 1);
      await t.ensureVisible(find.text('Saved note'));
      await t.pumpAndSettle();
      await t.tap(find.text('Saved note'));
      await t.pumpAndSettle();
      expect(
        find.text('“We could talk easily. The tables are small.”'),
        findsOneWidget,
      );
    },
  );
  testWidgets(
    'follow-up carries selection, keeps prior turn visible and resets context',
    (t) async {
      final api = FakeAsk();
      await t.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: AskExperience(api: api, onOpen: (_) async {}),
          ),
        ),
      );
      await submit(t, 'quiet dinner in Delhi');
      api.pending[0].complete(fixture());
      await t.pumpAndSettle();
      await t.ensureVisible(find.byTooltip('More options for Lantern Kitchen'));
      await t.pumpAndSettle();
      await t.tap(find.byTooltip('More options for Lantern Kitchen'));
      await t.pumpAndSettle();
      await t.tap(find.text('Ask about this'));
      await t.pump();
      await submit(t, 'Can this one fit six people?');
      expect(api.contexts[1]['parent'], fixture().requestId);
      expect(api.contexts[1]['selected'], [fixture().results.single.item.id]);
      api.pending[1].complete(fixture(turn: 2));
      await t.pumpAndSettle();
      expect(find.text('quiet dinner in Delhi'), findsOneWidget);
      expect(find.text('Can this one fit six people?'), findsOneWidget);
      expect(api.pending.length, 2);
      await t.tap(find.byTooltip('Back to Ask'));
      await t.pumpAndSettle();
      expect(find.text('Ask Rekky…'), findsOneWidget);
      expect(find.text('Continue conversation'), findsOneWidget);
      await submit(t, 'A doctor');
      expect(api.contexts.last['parent'], isNull);
      expect(api.contexts.last['selected'], isEmpty);
      api.pending.last.complete(fixture());
      await t.pumpAndSettle();
      await t.tap(find.byTooltip('Back to Ask'));
      await t.pumpAndSettle();
      await t.tap(find.text('Continue previous conversation'));
      await t.pumpAndSettle();
      expect(find.text('Can this one fit six people?'), findsOneWidget);
      await submit(t, 'And its location?');
      expect(api.contexts.last['parent'], fixture(turn: 2).requestId);
      api.pending.last.complete(fixture(turn: 2));
      await t.pumpAndSettle();
    },
  );

  testWidgets(
    'Back to Ask preserves a thread and its draft; tab returns home',
    (t) async {
      final api = FakeAsk();
      Widget screen(int signal) => MaterialApp(
        home: Scaffold(
          body: AskExperience(
            api: api,
            onOpen: (_) async {},
            homeSignal: signal,
          ),
        ),
      );
      await t.pumpWidget(screen(0));
      await submit(t, 'dinner in Delhi');
      api.pending.single.complete(fixture());
      await t.pumpAndSettle();
      await t.enterText(find.byType(TextField), 'Something with a garden');
      await t.tap(find.byTooltip('Back to Ask'));
      await t.pumpAndSettle();
      expect(find.text('Ask Rekky…'), findsOneWidget);
      await t.tap(find.text('Continue conversation'));
      await t.pumpAndSettle();
      expect(find.text('dinner in Delhi'), findsOneWidget);
      expect(find.text('Something with a garden'), findsOneWidget);
      await t.pumpWidget(screen(1));
      await t.pumpAndSettle();
      expect(find.text('Ask Rekky…'), findsOneWidget);
      expect(find.text('Continue conversation'), findsOneWidget);
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
      await t.ensureVisible(find.text('Saved note'));
      await t.pumpAndSettle();
      await t.ensureVisible(find.byTooltip('More options for Lantern Kitchen'));
      await t.pumpAndSettle();
      await t.tap(find.byTooltip('More options for Lantern Kitchen'));
      await t.pumpAndSettle();
      await t.tap(find.text('Ask about this'));
      await t.pump();
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
  testWidgets(
    'failed synthesis keeps prior answer and retries with a new run',
    (t) async {
      final api = FakeAsk();
      await t.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: AskExperience(
              api: api,
              onOpen: (_) async {},
              resolveCity: () async => 'Delhi',
            ),
          ),
        ),
      );
      await submit(t, 'something fun this weekend');
      expect(api.contexts[0]['scopeCity'], 'Delhi');
      api.pending[0].complete(fixture());
      await t.pumpAndSettle();
      await submit(t, 'in Delhi');
      api.pending[1].completeError(
        const ApiFailure(
          'ask_failed',
          'Couldn’t finish checking your saved recommendations. Try again.',
          503,
        ),
      );
      await t.pumpAndSettle();
      expect(find.text('Lantern Kitchen'), findsOneWidget);
      expect(
        find.textContaining('There isn’t enough in your Library'),
        findsNothing,
      );
      await t.tap(find.text('Try again'));
      await t.pump();
      expect(api.requestIds[1], isNot(api.requestIds[2]));
      expect(api.contexts[1]['parent'], api.contexts[2]['parent']);
      expect(api.contexts[2]['question'], 'in Delhi');
      expect(api.contexts[2]['scopeCity'], 'Delhi');
      api.pending[2].complete(fixture(turn: 2));
      await t.pumpAndSettle();
    },
  );
  testWidgets('denied location still allows an explicit city in Ask', (
    t,
  ) async {
    final api = FakeAsk();
    await t.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: AskExperience(
            api: api,
            onOpen: (_) async {},
            resolveCity: () async => null,
          ),
        ),
      ),
    );
    await submit(t, 'dinner nearby');
    expect(api.contexts.single['scopeCity'], isNull);
    api.pending.single.complete(fixture());
    await t.pumpAndSettle();
    await submit(t, 'a quiet place in Delhi');
    expect(api.contexts.last['scopeCity'], isNull);
    expect(api.contexts.last['question'], 'a quiet place in Delhi');
    api.pending.last.complete(fixture(turn: 2));
    await t.pumpAndSettle();
  });
}
