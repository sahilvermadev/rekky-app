import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

/// Small, semantic cues for deliberate actions; background work stays silent.
abstract final class RekkyHaptics {
  static void selection() => _play(HapticFeedback.selectionClick);
  static void confirm() => _play(HapticFeedback.lightImpact);
  static void warning() => _play(HapticFeedback.mediumImpact);

  static void _play(Future<void> Function() feedback) {
    final state = WidgetsBinding.instance.lifecycleState;
    if (state != null && state != AppLifecycleState.resumed) return;
    unawaited(_tryFeedback(feedback));
  }

  static Future<void> _tryFeedback(Future<void> Function() feedback) async {
    try {
      await feedback();
    } catch (_) {
      // Unsupported hardware must never interrupt an action.
    }
  }
}
