import 'dart:async';

import 'rekky_api.dart';
import 'voice_drafts.dart';

/// Uploads new account-owned recordings and observes the durable server job.
/// Unaccepted uploads resume when the app is running; accepted jobs run on the
/// server even when this coordinator or its screen no longer exists.
class VoiceProcessingCoordinator {
  VoiceProcessingCoordinator({
    required this.ownerId,
    required this.store,
    required this.api,
    required this.onChanged,
    required this.onStatus,
    this.pollInterval = const Duration(seconds: 4),
  });

  final String ownerId;
  final VoiceDraftStore store;
  final RekkyApi api;
  final Future<void> Function() onChanged;
  final void Function(String?) onStatus;
  final Duration pollInterval;
  bool _running = false, _again = false, _stopped = false, _paused = false;
  Timer? _timer;

  void stop() {
    _stopped = true;
    _timer?.cancel();
  }

  void pause() {
    _paused = true;
    _timer?.cancel();
  }

  void resume() {
    _paused = false;
    unawaited(process());
  }

  Future<void> process() async {
    if (_stopped || _paused) return;
    if (_running) {
      _again = true;
      return;
    }
    _timer?.cancel();
    _running = true;
    var pending = false;
    String? issue;
    try {
      do {
        _again = false;
        final drafts = (await store.forOwner(ownerId))
            .where((draft) => draft.autoProcess && draft.status == 'ready')
            .toList();
        final tracked = await store.pendingRemembers(ownerId);
        final ids = {...tracked, ...drafts.map((draft) => draft.id)};
        if (ids.isNotEmpty && !_stopped) {
          onStatus('Adding your recommendation…');
        }
        for (final id in ids) {
          if (_stopped || _paused) break;
          final draft = drafts.where((value) => value.id == id).firstOrNull;
          try {
            Map<String, dynamic> receipt;
            try {
              receipt = await api.rememberStatus(id);
            } on ApiFailure catch (failure) {
              if (failure.status != 404 || draft == null) rethrow;
              final file = await store.readyFile(ownerId, draft);
              if (_stopped || _paused) break;
              receipt = await api.rememberVoice(id, draft.createdAtMs, file);
            }
            if (_stopped) break;
            await store.queueRemember(ownerId, id);
            final status = receipt['status'];
            final captureStatus = receipt['capture_status'];
            if (status == 'transcribed') {
              // A transcribed receipt is durable even if its source is later
              // deleted; accepted audio must never be resurrected or re-sent.
              if (draft != null) await store.acknowledgeTranscript(ownerId, id);
              if (captureStatus == 'completed' || captureStatus == 'partial') {
                await store.acknowledgeRemember(ownerId, id);
                if (!_stopped) await onChanged();
              } else if (captureStatus == 'failed' ||
                  captureStatus == 'cancelled') {
                issue = 'A recording needs attention. Open Pending recordings in settings.';
              } else {
                pending = true;
              }
            } else if (['failed', 'cancelled', 'expired'].contains(status)) {
              issue = 'A recording could not be saved. Open Pending recordings in settings.';
            } else {
              pending = true;
            }
          } on ApiFailure catch (failure) {
            if (failure.status == 403 || failure.status == 401) {
              issue = failure.status == 401
                  ? 'Sign in again to finish saving your recording.'
                  : 'Processing is off. Turn it on in settings to save new recordings.';
            } else {
              pending = true;
              issue = 'Your recording is safe. We’ll try again shortly.';
            }
          } catch (_) {
            pending = true;
            issue = 'Saved on this phone. We’ll finish when connected.';
          }
        }
      } while (_again && !_stopped && !_paused);
    } catch (_) {
      pending = true;
      issue = 'Saved on this phone. We’ll finish when connected.';
    } finally {
      _running = false;
      if (!_stopped) {
        onStatus(issue ?? (pending ? 'Adding your recommendation…' : null));
        if (pending && !_paused) {
          _timer = Timer(
            issue == null ? pollInterval : const Duration(seconds: 30),
            () => unawaited(process()),
          );
        }
      }
    }
  }
}
