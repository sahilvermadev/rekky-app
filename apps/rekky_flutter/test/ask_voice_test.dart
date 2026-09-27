import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/ask_recorder.dart';
import 'package:rekky_flutter/ask_voice_sheet.dart';
import 'package:rekky_flutter/rekky_api.dart';
import 'package:rekky_flutter/voice_drafts.dart';

class FileRecorder implements VoiceRecorder {
  String? path;
  Completer<void>? gate, stopGate;
  bool stopping = false;
  bool disposed = false;
  @override
  Future<bool> hasPermission() async => true;
  @override
  Future<void> start(String path) async {
    this.path = path;
    await File(path).writeAsBytes(List.filled(256, 1));
    await gate?.future;
  }

  @override
  Future<String?> stop() async {
    stopping = true;
    await stopGate?.future;
    return path;
  }

  @override
  Future<void> cancel() async {}
  @override
  Future<void> dispose() async {
    disposed = true;
  }
}

class FakeRecorder extends AskRecorder {
  bool closed = false;
  @override
  Future<void> start() async {}
  @override
  Future<Uint8List> finish() async => Uint8List(128);
  @override
  Future<void> close() async {
    closed = true;
  }
}

class FakeVoiceApi extends RekkyApi {
  FakeVoiceApi() : super('http://unused');
  final pending = Completer<String>();
  int calls = 0, cancellations = 0;
  @override
  Future<String> transcribeQuestion(String id, List<int> audio) {
    calls++;
    return pending.future;
  }

  @override
  Future<void> cancelDictation(String id) async {
    cancellations++;
  }
}

Future<void> open(
  WidgetTester t,
  FakeVoiceApi api,
  FakeRecorder recorder,
  void Function(String?) result,
) async {
  await t.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: Builder(
          builder: (context) => TextButton(
            onPressed: () async {
              result(
                await showModalBottomSheet<String>(
                  context: context,
                  isScrollControlled: true,
                  builder: (_) => AskVoiceSheet(
                    api: api,
                    requestId: 'synthetic',
                    recorder: recorder,
                  ),
                ),
              );
            },
            child: const Text('Speak'),
          ),
        ),
      ),
    ),
  );
  await t.tap(find.text('Speak'));
  await t.pumpAndSettle();
}

void main() {
  test('question audio is deleted after reading and stale files are purged on entry', () async {
    final root = await Directory.systemTemp.createTemp('rekky-ask-test');
    addTearDown(() => root.delete(recursive: true));
    final old = File('${root.path}/rekky_ask_voice_v1/old.m4a');
    await old.parent.create();
    await old.writeAsString('old');
    final device = FileRecorder();
    final recorder = AskRecorder(recorder: device, directory: () async => root);
    await recorder.start();
    expect(await old.exists(), isFalse);
    final bytes = await recorder.finish();
    expect(bytes.length, 256);
    expect(await File(device.path!).exists(), isFalse);
    await recorder.close();
    expect(device.disposed, isTrue);
  });
  test(
    'cancellation during recorder startup waits and removes the late file',
    () async {
      final root = await Directory.systemTemp.createTemp('rekky-ask-race');
      addTearDown(() => root.delete(recursive: true));
      final device = FileRecorder()..gate = Completer<void>();
      final recorder = AskRecorder(
        recorder: device,
        directory: () async => root,
      );
      final starting = recorder.start();
      while (device.path == null) {
        await Future<void>.delayed(const Duration(milliseconds: 1));
      }
      final closing = recorder.close();
      device.gate!.complete();
      await starting;
      await closing;
      expect(await File(device.path!).exists(), isFalse);
      expect(device.disposed, isTrue);
    },
  );
  test('cancel during native stop waits for the stop before disposing and removing audio', () async {
    final root = await Directory.systemTemp.createTemp('rekky-ask-stop');
    addTearDown(() => root.delete(recursive: true));
    final device = FileRecorder()..stopGate = Completer<void>();
    final recorder = AskRecorder(recorder: device, directory: () async => root);
    await recorder.start();
    final finishing = recorder.finish();
    while (!device.stopping) {
      await Future<void>.delayed(const Duration(milliseconds: 1));
    }
    final closing = recorder.close();
    expect(device.disposed, isFalse);
    device.stopGate!.complete();
    await finishing;
    await closing;
    expect(device.disposed, isTrue);
    expect(await File(device.path!).exists(), isFalse);
  });
  testWidgets(
    'Done returns only the question text and clears temporary server text',
    (t) async {
      final api = FakeVoiceApi();
      final recorder = FakeRecorder();
      String? result;
      await open(t, api, recorder, (value) => result = value);
      expect(find.text('Done'), findsOneWidget);
      await t.tap(find.text('Done'));
      await t.pump();
      expect(api.calls, 1);
      expect(recorder.closed, isTrue);
      api.pending.complete('Somewhere quiet for dinner?');
      await t.pumpAndSettle();
      expect(result, 'Somewhere quiet for dinner?');
      expect(api.cancellations, greaterThan(0));
    },
  );
  testWidgets('cancel during transcription ignores the late transcript', (
    t,
  ) async {
    final api = FakeVoiceApi();
    final recorder = FakeRecorder();
    String? result;
    await open(t, api, recorder, (value) => result = value);
    await t.tap(find.text('Done'));
    await t.pump();
    await t.tap(find.byTooltip('Cancel recording'));
    await t.pumpAndSettle();
    api.pending.complete('Must not submit this');
    await t.pumpAndSettle();
    expect(result, isNull);
    expect(recorder.closed, isTrue);
    expect(api.cancellations, greaterThan(0));
  });
  testWidgets(
    'background interruption discards a recording without uploading it',
    (t) async {
      final api = FakeVoiceApi();
      final recorder = FakeRecorder();
      await open(t, api, recorder, (_) {});
      t.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
      await t.pumpAndSettle();
      expect(api.calls, 0);
      expect(recorder.closed, isTrue);
      t.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    },
  );
}
