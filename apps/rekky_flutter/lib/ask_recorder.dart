import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:path_provider/path_provider.dart';

import 'voice_drafts.dart';

/// A disposable question recording. No draft database or recommendation outbox.
class AskRecorder {
  AskRecorder({
    VoiceRecorder? recorder,
    Future<Directory> Function()? directory,
  }) : recorder = recorder ?? DeviceVoiceRecorder(),
       directory = directory ?? getApplicationCacheDirectory,
       customDirectory = directory != null;
  static Future<Directory>? _startup;
  static Future<Directory> _clean(
    Future<Directory> Function() directory,
  ) async {
    final root = Directory('${(await directory()).path}/rekky_ask_voice_v1');
    await root.create(recursive: true);
    await for (final entry in root.list()) {
      if (entry is File) await entry.delete();
    }
    return root;
  }

  static Future<Directory> _cachedDirectory() async {
    try {
      return await (_startup ??= _clean(getApplicationCacheDirectory));
    } catch (_) {
      _startup = null;
      rethrow;
    }
  }

  static Future<void> clearAbandoned() async {
    await _cachedDirectory();
  }

  final bool customDirectory;
  final VoiceRecorder recorder;
  final Future<Directory> Function() directory;
  File? _file;
  Future<void>? _starting;
  Future<void>? _closing;
  Future<Uint8List>? _finishing;
  bool _closed = false;

  Future<void> start() => _starting ??= _start();
  Future<void> _start() async {
    if (_closed) return;
    if (!await recorder.hasPermission()) {
      throw StateError(
        'Microphone access is needed to speak a question. You can still type.',
      );
    }
    if (_closed) return;
    final root = await (customDirectory
        ? _clean(directory)
        : _cachedDirectory());
    await root.create(recursive: true);
    if (_closed) return;
    _file = File(
      '${root.path}/${DateTime.now().microsecondsSinceEpoch}-${Random.secure().nextInt(1 << 30)}.m4a',
    );
    await recorder.start(_file!.path);
  }

  Future<Uint8List> finish() => _finishing ??= _finish();
  Future<Uint8List> _finish() async {
    await _starting;
    if (_closed || _file == null) throw StateError('Recording was cancelled.');
    await recorder.stop();
    try {
      final size = await _file!.length();
      if (size < 128 || size > 600000) {
        throw StateError('Try a short question of up to one minute.');
      }
      return await _file!.readAsBytes();
    } finally {
      await _delete();
    }
  }

  Future<void> close() {
    _closed = true;
    return _closing ??= _close();
  }

  Future<void> _close() async {
    try {
      await _starting;
    } catch (_) {
      /* Start failure still needs cleanup. */
    }
    try {
      await _finishing;
    } catch (_) {
      /* Cleanup must follow a failed stop too. */
    }
    try {
      await recorder.cancel();
    } finally {
      try {
        await recorder.dispose();
      } finally {
        await _delete();
      }
    }
  }

  Future<void> _delete() async {
    final file = _file;
    if (file != null && await file.exists()) await file.delete();
  }
}
