import 'dart:io';

import 'package:flutter/services.dart';

/// Native, app-private temporary storage outside the system's evictable cache.
/// The native side excludes this directory and each completed file from backup.
class ProtectedVoiceStorage {
  static const _channel = MethodChannel('app.rekky/voice_storage');

  Future<Directory> directory() async {
    final path = await _channel.invokeMethod<String>('directory');
    if (path == null || path.isEmpty) {
      throw StateError('Protected voice storage is unavailable.');
    }
    return Directory(path);
  }

  Future<void> protectFile(String path) async {
    await _channel.invokeMethod<void>('protectFile', {'path': path});
  }
}
