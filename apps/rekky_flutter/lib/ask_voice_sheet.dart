import 'dart:async';

import 'package:flutter/material.dart';

import 'ask_recorder.dart';
import 'library_style.dart';
import 'rekky_api.dart';
import 'rekky_haptics.dart';

class AskVoiceSheet extends StatefulWidget {
  const AskVoiceSheet({
    super.key,
    required this.api,
    required this.requestId,
    this.recorder,
  });
  final RekkyApi api;
  final String requestId;
  final AskRecorder? recorder;
  @override
  State<AskVoiceSheet> createState() => _AskVoiceSheetState();
}

class _AskVoiceSheetState extends State<AskVoiceSheet>
    with WidgetsBindingObserver {
  late final AskRecorder recorder = widget.recorder ?? AskRecorder();
  bool starting = true,
      recording = false,
      transcribing = false,
      cancelled = false;
  String? error;
  Timer? timer;
  final elapsed = Stopwatch();
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    unawaited(start());
  }

  Future<void> clearServer() async {
    try {
      await widget.api.cancelDictation(widget.requestId);
    } catch (_) {
      /* Ten-minute server expiry bounds interrupted cleanup. */
    }
  }

  Future<void> start() async {
    try {
      await recorder.start();
      if (!mounted || cancelled) return;
      setState(() {
        starting = false;
        recording = true;
      });
      elapsed.start();
      RekkyHaptics.confirm();
      timer = Timer.periodic(const Duration(seconds: 1), (_) {
        if (!mounted || cancelled) return;
        if (elapsed.elapsed.inSeconds >= 60) {
          unawaited(finish());
        } else {
          setState(() {});
        }
      });
    } catch (e) {
      if (mounted && !cancelled) {
        setState(() {
          starting = false;
          error = e is StateError
              ? e.message.toString()
              : 'Microphone unavailable. You can type your question instead.';
        });
      }
      await recorder.close();
    }
  }

  Future<void> finish() async {
    if (!recording || transcribing || cancelled) return;
    timer?.cancel();
    elapsed.stop();
    setState(() {
      recording = false;
      transcribing = true;
    });
    RekkyHaptics.confirm();
    try {
      final bytes = await recorder.finish();
      await recorder.close();
      if (!mounted || cancelled) return;
      final text = await widget.api.transcribeQuestion(widget.requestId, bytes);
      unawaited(clearServer());
      if (mounted && !cancelled) Navigator.pop(context, text);
    } catch (e) {
      await recorder.close();
      unawaited(clearServer());
      if (mounted && !cancelled) {
        RekkyHaptics.warning();
        setState(() {
          transcribing = false;
          error = e is ApiFailure ? e.message : 'Couldn’t transcribe your question. You can type it or record again.';
        });
      }
    }
  }

  Future<void> cancel() async {
    if (cancelled) return;
    cancelled = true;
    timer?.cancel();
    unawaited(clearServer());
    await recorder.close();
    if (mounted) Navigator.pop(context);
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.hidden) {
      unawaited(cancel());
    }
  }

  @override
  void dispose() {
    cancelled = true;
    timer?.cancel();
    WidgetsBinding.instance.removeObserver(this);
    unawaited(recorder.close());
    unawaited(clearServer());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => PopScope(
    canPop: false,
    onPopInvokedWithResult: (didPop, _) {
      if (!didPop) unawaited(cancel());
    },
    child: SafeArea(
      child: SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(24, 12, 24, 24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    'Ask out loud',
                    style: LibraryStyle.heading(context, 28),
                  ),
                ),
                IconButton(
                  tooltip: 'Cancel recording',
                  onPressed: cancel,
                  icon: const Icon(Icons.close_rounded),
                ),
              ],
            ),
            const SizedBox(height: 12),
            Text(
              error ??
                  (transcribing
                      ? 'Turning your question into text…'
                      : starting
                      ? 'Getting your microphone ready…'
                      : 'Listening · ${elapsed.elapsed.inSeconds}s'),
            ),
            const SizedBox(height: 20),
            if (transcribing) const LinearProgressIndicator(),
            if (error == null && !transcribing)
              FilledButton.icon(
                style: FilledButton.styleFrom(
                  backgroundColor: LibraryStyle.searchFocus(context),
                  minimumSize: const Size(0, 56),
                ),
                onPressed: recording ? finish : null,
                icon: const Icon(Icons.stop_rounded),
                label: const Text('Done'),
              ),
            const SizedBox(height: 12),
            Text(
              'Your question goes to OpenAI for transcription. The recording is discarded; nothing is added to your Library.',
              style: Theme.of(context).textTheme.bodySmall,
            ),
            if (error != null)
              TextButton(
                onPressed: cancel,
                child: const Text('Back to typing'),
              ),
          ],
        ),
      ),
    ),
  );
}
