import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:path_provider/path_provider.dart';
import 'package:record/record.dart';
import 'package:sqflite/sqflite.dart';

const voiceDraftLifetime = Duration(days: 7);

abstract class VoiceRecorder {
  Future<bool> hasPermission();
  Future<void> start(String path);
  Future<String?> stop();
  Future<void> cancel();
  Future<void> dispose();
}

class DeviceVoiceRecorder implements VoiceRecorder {
  AudioRecorder? _active;
  AudioRecorder get _record => _active ??= AudioRecorder();

  @override
  Future<bool> hasPermission() => _record.hasPermission();

  @override
  Future<void> start(String path) => _record.start(
    const RecordConfig(
      encoder: AudioEncoder.aacLc,
      bitRate: 64000,
      sampleRate: 16000,
      numChannels: 1,
    ),
    path: path,
  );

  @override
  Future<String?> stop() => _record.stop();

  @override
  Future<void> cancel() => _record.cancel();

  @override
  Future<void> dispose() async {
    await _active?.dispose();
  }
}

class VoiceDraft {
  const VoiceDraft({
    required this.id,
    required this.ownerId,
    required this.status,
    required this.createdAtMs,
    required this.bytes,
  });

  final String id, ownerId, status;
  final int createdAtMs, bytes;

  factory VoiceDraft.fromRow(Map<String, Object?> row) => VoiceDraft(
    id: row['id'] as String,
    ownerId: row['owner_id'] as String,
    status: row['status'] as String,
    createdAtMs: row['created_at_ms'] as int,
    bytes: row['bytes'] as int,
  );
}

/// Account-scoped, local-only recording drafts. No audio reaches the API here.
class VoiceDraftStore {
  VoiceDraftStore({
    VoiceRecorder? recorder,
    this.factory,
    Future<Directory> Function()? supportDirectory,
    Future<Directory> Function()? cacheDirectory,
  }) : _recorder = recorder ?? DeviceVoiceRecorder(),
       _supportDirectory = supportDirectory ?? getApplicationSupportDirectory,
       _cacheDirectory = cacheDirectory ?? getApplicationCacheDirectory;

  final VoiceRecorder _recorder;
  final DatabaseFactory? factory;
  final Future<Directory> Function() _supportDirectory, _cacheDirectory;
  Future<Database>? _databaseFuture;
  Future<Directory>? _audioDirectoryFuture;
  String? _activeId;

  Future<Database> get _database => _databaseFuture ??= _openDatabase();
  Future<Directory> get _audioDirectory =>
      _audioDirectoryFuture ??= _openAudioDirectory();

  Future<Database> _openDatabase() async {
    final support = await _supportDirectory();
    return (factory ?? databaseFactory).openDatabase(
      '${support.path}/voice_drafts_v1.db',
      options: OpenDatabaseOptions(
        version: 1,
        onCreate: (db, _) async {
          await db.execute('''
          CREATE TABLE voice_drafts (
            id TEXT PRIMARY KEY,
            owner_id TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('recording','ready','interrupted','missing')),
            path TEXT NOT NULL,
            bytes INTEGER NOT NULL DEFAULT 0,
            created_at_ms INTEGER NOT NULL
          )
        ''');
          await db.execute(
            'CREATE INDEX voice_drafts_owner_idx ON voice_drafts(owner_id, created_at_ms DESC)',
          );
        },
      ),
    );
  }

  Future<Directory> _openAudioDirectory() async {
    // App cache is excluded from normal device backups. The OS may evict it;
    // reconciliation reports that loss rather than claiming durable capture.
    final cache = await _cacheDirectory();
    final directory = Directory('${cache.path}/rekky_voice_drafts_v1');
    await directory.create(recursive: true);
    return directory;
  }

  String _newId() => base64UrlEncode(
    List<int>.generate(24, (_) => Random.secure().nextInt(256)),
  ).replaceAll('=', '');

  Future<void> start(String ownerId) async {
    if (_activeId != null) throw StateError('A recording is already active.');
    if (!await _recorder.hasPermission()) {
      throw StateError('Microphone access is needed to record.');
    }
    final id = _newId();
    final directory = await _audioDirectory;
    final path = '${directory.path}/$id.m4a';
    final db = await _database;
    await db.insert('voice_drafts', {
      'id': id,
      'owner_id': ownerId,
      'status': 'recording',
      'path': path,
      'bytes': 0,
      'created_at_ms': DateTime.now().millisecondsSinceEpoch,
    });
    _activeId = id;
    try {
      await _recorder.start(path);
    } catch (_) {
      _activeId = null;
      try {
        await _recorder.cancel();
      } finally {
        await _deleteRow(db, id, path);
      }
      rethrow;
    }
  }

  Future<VoiceDraft> finish(String ownerId) async {
    final id = _activeId;
    if (id == null) throw StateError('No recording is active.');
    final db = await _database;
    final rows = await db.query(
      'voice_drafts',
      where: 'id = ? AND owner_id = ?',
      whereArgs: [id, ownerId],
      limit: 1,
    );
    if (rows.isEmpty) throw StateError('Recording owner changed.');
    final path = rows.single['path'] as String;
    try {
      final stoppedPath = await _recorder.stop();
      _activeId = null;
      if (stoppedPath != path) {
        await db.update(
          'voice_drafts',
          {'status': 'interrupted'},
          where: 'id = ?',
          whereArgs: [id],
        );
        throw StateError('The recorder did not return the expected file.');
      }
      final file = File(path);
      final bytes = await file.exists() ? await file.length() : 0;
      if (bytes == 0) {
        await db.update(
          'voice_drafts',
          {'status': 'missing'},
          where: 'id = ?',
          whereArgs: [id],
        );
        throw StateError('The recording was empty or unavailable.');
      }
      await db.update(
        'voice_drafts',
        {'status': 'ready', 'bytes': bytes},
        where: 'id = ? AND owner_id = ?',
        whereArgs: [id, ownerId],
      );
      return VoiceDraft.fromRow({
        ...rows.single,
        'status': 'ready',
        'bytes': bytes,
      });
    } catch (_) {
      _activeId = null;
      rethrow;
    }
  }

  Future<void> cancel(String ownerId) async {
    final id = _activeId;
    if (id == null) return;
    final db = await _database;
    final rows = await db.query(
      'voice_drafts',
      where: 'id = ? AND owner_id = ?',
      whereArgs: [id, ownerId],
      limit: 1,
    );
    await _recorder.cancel();
    _activeId = null;
    if (rows.isNotEmpty) {
      await _deleteRow(db, id, rows.single['path'] as String);
    }
  }

  Future<void> _deleteRow(Database db, String id, String path) async {
    final file = File(path);
    if (await file.exists()) await file.delete();
    await db.delete('voice_drafts', where: 'id = ?', whereArgs: [id]);
  }

  Future<void> delete(String ownerId, String id) async {
    if (_activeId == id) throw StateError('Stop recording before deletion.');
    final db = await _database;
    final rows = await db.query(
      'voice_drafts',
      where: 'id = ? AND owner_id = ?',
      whereArgs: [id, ownerId],
      limit: 1,
    );
    if (rows.isEmpty) return;
    await _deleteRow(db, id, rows.single['path'] as String);
  }

  Future<List<VoiceDraft>> forOwner(String ownerId) async {
    final db = await _database;
    final directory = await _audioDirectory;
    final all = await db.query('voice_drafts');
    final knownPaths = <String>{};
    final cutoff = DateTime.now()
        .subtract(voiceDraftLifetime)
        .millisecondsSinceEpoch;
    for (final row in all) {
      final id = row['id'] as String;
      final path = row['path'] as String;
      if ((row['created_at_ms'] as int) < cutoff && id != _activeId) {
        await _deleteRow(db, id, path);
        continue;
      }
      knownPaths.add(path);
      if (id == _activeId) continue;
      final exists = await File(path).exists();
      final status = row['status'] as String;
      if (!exists && status != 'missing') {
        await db.update(
          'voice_drafts',
          {'status': 'missing', 'bytes': 0},
          where: 'id = ?',
          whereArgs: [id],
        );
      } else if (status == 'recording') {
        await db.update(
          'voice_drafts',
          {'status': 'interrupted'},
          where: 'id = ?',
          whereArgs: [id],
        );
      }
    }
    await for (final entry in directory.list()) {
      if (entry is File &&
          entry.path.endsWith('.m4a') &&
          !knownPaths.contains(entry.path)) {
        await entry.delete();
      }
    }
    final rows = await db.query(
      'voice_drafts',
      columns: ['id', 'owner_id', 'status', 'bytes', 'created_at_ms'],
      where: 'owner_id = ?',
      whereArgs: [ownerId],
      orderBy: 'created_at_ms DESC',
    );
    return rows.map(VoiceDraft.fromRow).toList();
  }

  Future<void> dispose() async {
    await _recorder.dispose();
    final future = _databaseFuture;
    if (future != null) {
      final db = await future;
      await db.close();
    }
  }
}
