import 'dart:async';

import 'package:flutter/material.dart';

import 'voice_drafts.dart';

class VoiceCaptureSheet extends StatefulWidget {
  const VoiceCaptureSheet({
    super.key,
    required this.ownerId,
    required this.store,
    required this.onChanged,
  });

  final String ownerId;
  final VoiceDraftStore store;
  final Future<void> Function() onChanged;

  @override
  State<VoiceCaptureSheet> createState() => _VoiceCaptureSheetState();
}

class _VoiceCaptureSheetState extends State<VoiceCaptureSheet>
    with WidgetsBindingObserver {
  bool starting = true, recording = false, finishing = false;
  VoiceDraft? saved;
  String? issue;
  Timer? limit;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    WidgetsBinding.instance.addPostFrameCallback((_) => _start());
  }

  @override
  void dispose() {
    limit?.cancel();
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.inactive ||
        state == AppLifecycleState.paused) {
      if (recording && !finishing) unawaited(_finish());
    }
  }

  Future<void> _start() async {
    if (!mounted) return;
    try {
      await widget.store.start(widget.ownerId);
      if (!mounted) {
        await widget.store.cancel(widget.ownerId);
        return;
      }
      setState(() {
        starting = false;
        recording = true;
      });
      limit = Timer(const Duration(minutes: 2), () => unawaited(_finish()));
    } catch (error) {
      if (mounted) {
        setState(() {
          starting = false;
          issue = '$error';
        });
      }
    }
  }

  Future<void> _finish() async {
    if (!recording || finishing) return;
    limit?.cancel();
    setState(() => finishing = true);
    try {
      final draft = await widget.store.finish(widget.ownerId);
      await widget.onChanged();
      if (mounted) {
        setState(() {
          recording = false;
          finishing = false;
          saved = draft;
        });
      }
    } catch (error) {
      if (mounted) {
        setState(() {
          recording = false;
          finishing = false;
          issue = '$error';
        });
      }
      await widget.onChanged();
    }
  }

  Future<void> _discard() async {
    if (finishing || starting) return;
    try {
      if (saved case final draft?) {
        await widget.store.delete(widget.ownerId, draft.id);
      } else {
        await widget.store.cancel(widget.ownerId);
      }
      await widget.onChanged();
      if (mounted) Navigator.pop(context, false);
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
    }
  }

  @override
  Widget build(BuildContext context) => PopScope(
    canPop: false,
    child: Padding(
      padding: const EdgeInsets.fromLTRB(24, 8, 24, 32),
      child: SafeArea(
        top: false,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              saved != null
                  ? 'Voice draft on this device'
                  : recording
                  ? 'Recording your thought'
                  : 'Record a thought',
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            const SizedBox(height: 12),
            Text(
              saved != null
                  ? 'The recording is local only. Transcription is not connected yet, so this is not a saved memory.'
                  : 'Audio stays on this device. It is not uploaded or transcribed in this build. Drafts become ineligible after seven days and are removed when the app next runs.',
            ),
            const SizedBox(height: 24),
            if (starting || finishing) const LinearProgressIndicator(),
            if (recording)
              const Center(
                child: Icon(Icons.mic, size: 72, semanticLabel: 'Recording'),
              ),
            if (issue != null)
              Text(
                issue!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            const SizedBox(height: 20),
            if (recording)
              FilledButton(
                onPressed: finishing ? null : _finish,
                child: const Text('Done'),
              ),
            if (saved != null)
              FilledButton(
                onPressed: () => Navigator.pop(context, true),
                child: const Text('Close'),
              ),
            if (!starting && !finishing)
              TextButton(
                onPressed: _discard,
                child: Text(
                  saved != null ? 'Delete draft' : 'Discard / Type instead',
                ),
              ),
          ],
        ),
      ),
    ),
  );
}

class VoiceDraftsSheet extends StatefulWidget {
  const VoiceDraftsSheet({
    super.key,
    required this.ownerId,
    required this.store,
    required this.onChanged,
  });

  final String ownerId;
  final VoiceDraftStore store;
  final Future<void> Function() onChanged;

  @override
  State<VoiceDraftsSheet> createState() => _VoiceDraftsSheetState();
}

class _VoiceDraftsSheetState extends State<VoiceDraftsSheet> {
  List<VoiceDraft> drafts = [];
  String? issue;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final loaded = await widget.store.forOwner(widget.ownerId);
      if (mounted) setState(() => drafts = loaded);
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
    }
  }

  Future<void> _delete(VoiceDraft draft) async {
    try {
      await widget.store.delete(widget.ownerId, draft.id);
      await widget.onChanged();
      await _load();
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
    }
  }

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.fromLTRB(24, 8, 24, 32),
    child: SafeArea(
      top: false,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            'Voice drafts',
            style: Theme.of(context).textTheme.headlineSmall,
          ),
          const SizedBox(height: 8),
          const Text(
            'These recordings are only on this device. They have not been transcribed or added to your Library.',
          ),
          const SizedBox(height: 16),
          if (issue != null)
            Text(
              issue!,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          if (drafts.isEmpty) const Text('No voice drafts on this device.'),
          for (final draft in drafts)
            ListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(switch (draft.status) {
                'ready' => 'Recording saved locally',
                'interrupted' => 'Recording interrupted',
                _ => 'Recording unavailable',
              }),
              subtitle: Text(
                'Created ${DateTime.fromMillisecondsSinceEpoch(draft.createdAtMs).toLocal()}',
              ),
              trailing: IconButton(
                tooltip: 'Delete voice draft',
                onPressed: () => _delete(draft),
                icon: const Icon(Icons.delete_outline),
              ),
            ),
          const SizedBox(height: 12),
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Close'),
          ),
        ],
      ),
    ),
  );
}
