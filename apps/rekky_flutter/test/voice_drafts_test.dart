import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/voice_drafts.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';

class _FakeRecorder implements VoiceRecorder {
  String? path;
  bool permission = true;
  bool failStart = false;

  @override
  Future<bool> hasPermission() async => permission;

  @override
  Future<void> start(String path) async {
    this.path = path;
    await File(path).writeAsBytes([1, 2, 3, 4]);
    if (failStart) throw StateError('Recorder failed after opening the file.');
  }

  @override
  Future<String?> stop() async => path;

  @override
  Future<void> cancel() async {
    final current = path;
    if (current != null && await File(current).exists()) {
      await File(current).delete();
    }
    path = null;
  }

  @override
  Future<void> dispose() async {}
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  sqfliteFfiInit();

  late Directory root, support, cache;
  late _FakeRecorder recorder;
  late VoiceDraftStore store;

  VoiceDraftStore createStore(_FakeRecorder recorder) => VoiceDraftStore(
    recorder: recorder,
    factory: databaseFactoryFfi,
    supportDirectory: () async => support,
    cacheDirectory: () async => cache,
  );

  setUp(() async {
    root = await Directory.systemTemp.createTemp('rekky-voice-test-');
    support = await Directory('${root.path}/support').create();
    cache = await Directory('${root.path}/cache').create();
    recorder = _FakeRecorder();
    store = createStore(recorder);
  });

  tearDown(() async {
    await store.dispose();
    await root.delete(recursive: true);
  });

  test(
    'draft is recorded before capture and stays scoped to its owner',
    () async {
      await store.start('owner-a');
      final pending = await store.forOwner('owner-a');
      expect(pending.single.status, 'recording');
      expect(pending.single.ownerId, 'owner-a');

      final ready = await store.finish('owner-a');
      expect(ready.status, 'ready');
      expect(ready.bytes, 4);
      expect(await store.forOwner('owner-b'), isEmpty);
      expect((await store.forOwner('owner-a')).single.id, ready.id);

      await store.delete('owner-b', ready.id);
      expect(await File(recorder.path!).exists(), isTrue);
      await store.delete('owner-a', ready.id);
      expect(await File(recorder.path!).exists(), isFalse);
      expect(await store.forOwner('owner-a'), isEmpty);
    },
  );

  test('restart marks an unfinished recording as interrupted', () async {
    await store.start('owner-a');
    await store.dispose();
    store = createStore(_FakeRecorder());

    final recovered = await store.forOwner('owner-a');
    expect(recovered.single.status, 'interrupted');
    await store.delete('owner-a', recovered.single.id);
    expect(await File(recorder.path!).exists(), isFalse);
  });

  test('missing, expired and orphaned audio are reconciled', () async {
    await store.start('owner-a');
    final ready = await store.finish('owner-a');
    await File(recorder.path!).delete();
    expect((await store.forOwner('owner-a')).single.status, 'missing');

    final orphan = File('${recorder.path!}.m4a');
    await orphan.writeAsBytes([1]);
    await store.forOwner('owner-a');
    expect(await orphan.exists(), isFalse);

    final db = await databaseFactoryFfi.openDatabase(
      '${support.path}/voice_drafts_v1.db',
      options: OpenDatabaseOptions(singleInstance: false),
    );
    await db.update(
      'voice_drafts',
      {
        'created_at_ms': DateTime.now()
            .subtract(const Duration(days: 8))
            .millisecondsSinceEpoch,
      },
      where: 'id = ?',
      whereArgs: [ready.id],
    );
    await db.close();
    expect(await store.forOwner('owner-a'), isEmpty);
  });

  test('microphone denial leaves no pending record', () async {
    recorder.permission = false;
    await expectLater(store.start('owner-a'), throwsStateError);
    expect(await store.forOwner('owner-a'), isEmpty);
  });

  test('partial recorder startup deletes its audio and pending row', () async {
    recorder.failStart = true;
    await expectLater(store.start('owner-a'), throwsStateError);
    expect(
      await Directory('${cache.path}/rekky_voice_drafts_v1')
          .list()
          .where((entry) => entry is File)
          .toList(),
      isEmpty,
    );
    expect(await store.forOwner('owner-a'), isEmpty);
  });
}
