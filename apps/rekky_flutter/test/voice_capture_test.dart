import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/voice_capture_sheet.dart';
import 'package:rekky_flutter/rekky_theme.dart';
import 'package:rekky_flutter/voice_drafts.dart';

class CaptureStore extends VoiceDraftStore {
  int starts = 0, finishes = 0, cancels = 0;
  Completer<void>? startGate, finishGate;
  Object? startError;
  @override
  Future<void> start(String ownerId) async {
    starts++;
    if (startGate != null) await startGate!.future;
    if (startError != null) throw startError!;
  }

  @override
  Future<VoiceDraft> finish(String ownerId) async {
    finishes++;
    if (finishGate != null) await finishGate!.future;
    return draft(ownerId, 'ready');
  }

  @override
  Future<void> cancel(String ownerId) async {
    cancels++;
  }

  VoiceDraft draft(String owner, String status) => VoiceDraft(
    id: 'new-recording-0001',
    ownerId: owner,
    status: status,
    createdAtMs: 123,
    bytes: 256,
    autoProcess: true,
  );
}

void main() {
  final haptics = <String>[];
  setUp(() {
    haptics.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, (call) async {
          if (call.method == 'HapticFeedback.vibrate') {
            haptics.add(call.arguments as String);
          }
          return null;
        });
  });

  Future<void> open(WidgetTester tester, CaptureStore store) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Builder(
          builder: (context) => Scaffold(
            body: TextButton(
              onPressed: () => Navigator.push<bool>(
                context,
                MaterialPageRoute(
                  builder: (_) => VoiceCaptureSheet(
                    ownerId: 'owner',
                    store: store,
                    onChanged: () async {},
                    onCaptured: () {},
                  ),
                ),
              ),
              child: const Text('Open'),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open'));
    await tester.pumpAndSettle();
  }

  testWidgets(
    'stable controls, truthful readiness and acknowledgement timing',
    (tester) async {
      final store = CaptureStore()
        ..startGate = Completer<void>()
        ..finishGate = Completer<void>();
      await open(tester, store);
      final before = tester.getRect(find.widgetWithText(FilledButton, 'Done'));
      expect(find.text('Getting ready'), findsNothing);
      expect(find.byType(LinearProgressIndicator), findsNothing);
      expect(find.text('Starting microphone…'), findsOneWidget);
      expect(
        tester
            .widget<FilledButton>(find.widgetWithText(FilledButton, 'Done'))
            .onPressed,
        isNull,
      );
      expect(haptics, isEmpty);
      store.startGate!.complete();
      await tester.pumpAndSettle();
      expect(find.text('Recording'), findsOneWidget);
      expect(tester.getRect(find.widgetWithText(FilledButton, 'Done')), before);
      expect(haptics, ['HapticFeedbackType.lightImpact']);
      await tester.tap(find.text('Done'));
      await tester.pump();
      expect(find.text('Saving recording…'), findsOneWidget);
      expect(haptics.length, 1);
      store.finishGate!.complete();
      await tester.pumpAndSettle();
      expect(haptics, [
        'HapticFeedbackType.lightImpact',
        'HapticFeedbackType.lightImpact',
      ]);
    },
  );

  testWidgets(
    'cancel during startup cleans up late recorder without saving or buzzing',
    (tester) async {
      final store = CaptureStore()..startGate = Completer<void>();
      await open(tester, store);
      await tester.tap(find.text('Cancel'));
      await tester.pump();
      expect(find.text('Closing…'), findsOneWidget);
      store.startGate!.complete();
      await tester.pumpAndSettle();
      expect(store.cancels, 1);
      expect(store.finishes, 0);
      expect(find.text('Open'), findsOneWidget);
      expect(haptics, isEmpty);
    },
  );

  testWidgets(
    'denied microphone gives visible failure and warning with close available',
    (tester) async {
      final store = CaptureStore()
        ..startError = StateError('Microphone access is needed to record.');
      await open(tester, store);
      expect(find.text('Couldn’t record'), findsOneWidget);
      expect(find.text('Microphone off'), findsOneWidget);
      expect(haptics, ['HapticFeedbackType.mediumImpact']);
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      expect(find.text('Open'), findsOneWidget);
      expect(store.finishes, 0);
    },
  );

  testWidgets(
    'backgrounding during startup cancels instead of recording unattended',
    (tester) async {
      final store = CaptureStore()..startGate = Completer<void>();
      await open(tester, store);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
      store.startGate!.complete();
      await tester.pumpAndSettle();
      expect(store.cancels, 1);
      expect(store.finishes, 0);
      expect(haptics, isEmpty);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    },
  );

  for (final scale in [1.0, 2.0]) {
    testWidgets(
      'one tap records; Done queues and returns at text scale $scale',
      (tester) async {
        tester.view.physicalSize = const Size(360, 640);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        final store = CaptureStore();
        var queued = 0;
        bool? saved;
        await tester.pumpWidget(
          MaterialApp(
            theme: RekkyTheme.build(Brightness.light),
            builder: (context, child) => MediaQuery(
              data: MediaQuery.of(context)
                  .copyWith(textScaler: TextScaler.linear(scale)),
              child: child!,
            ),
            home: Builder(
              builder: (context) => Scaffold(
                body: Center(
                  child: FilledButton(
                    child: const Text('Recommend'),
                    onPressed: () async {
                      saved = await Navigator.push<bool>(
                        context,
                        MaterialPageRoute(
                          builder: (_) => VoiceCaptureSheet(
                            ownerId: 'owner',
                            store: store,
                            onChanged: () async {},
                            onCaptured: () {
                              queued++;
                            },
                          ),
                        ),
                      );
                    },
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.tap(find.text('Recommend'));
        await tester.pumpAndSettle();
        expect(store.starts, 1);
        expect(find.byType(TextField), findsNothing);
        expect(find.text('Tell us about your experience'), findsOneWidget);
        expect(tester.takeException(), isNull);
        await tester.ensureVisible(find.text('Done'));
        await tester.tap(find.text('Done'));
        await tester.pumpAndSettle();
        expect(store.finishes, 1);
        expect(queued, 1);
        expect(saved, true);
        expect(find.text('Recommend'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );
  }
}
