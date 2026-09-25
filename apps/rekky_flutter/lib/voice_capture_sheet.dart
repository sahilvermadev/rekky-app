import 'dart:async';

import 'package:flutter/material.dart';

import 'rekky_api.dart';
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
                  ? 'The recording is saved on this device. Open Voice drafts to choose whether to transcribe it. It is not a Library item yet.'
                  : 'Audio stays on this device until you choose to transcribe it. Drafts become ineligible after seven days and are removed when the app next runs.',
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
    required this.api,
    required this.onChanged,
  });

  final String ownerId;
  final VoiceDraftStore store;
  final RekkyApi api;
  final Future<void> Function() onChanged;

  @override
  State<VoiceDraftsSheet> createState() => _VoiceDraftsSheetState();
}

class _VoiceDraftsSheetState extends State<VoiceDraftsSheet> {
  List<VoiceDraft> drafts = [];
  List<VoiceCapture> captures = [];
  bool working = false, permissionEnabled = false, providerAvailable = false;
  String? issue;
  String? receipt;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final loaded = await widget.store.forOwner(widget.ownerId);
      if (mounted) setState(() => drafts = loaded);
      final permission = await widget.api.voicePermission();
      final remoteCaptures = await widget.api.voiceCaptures();
      final data = permission['voice_transcription'] as Map<String, dynamic>;
      if (mounted) {
        setState(() {
          captures = remoteCaptures;
          permissionEnabled = data['enabled'] as bool;
          providerAvailable = data['provider_available'] as bool;
          issue = null;
        });
      }
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
    }
  }

  Future<bool> _confirmVoiceProcessing() async =>
      await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: const Text('Allow voice transcription?'),
          content: const SingleChildScrollView(
            child: Text(
              'Rekky will send the selected recording to OpenAI’s gpt-transcribe service to create text. OpenAI says API data is not used to train models by default unless the API account opts in. Its default abuse-monitoring logs may include content for up to 30 days, or longer when required by law or to protect services. Rekky saves the transcript privately and removes its audio copies after a usable transcript is saved. This permission is separate from microphone access and sharing with friends. You can turn it off here later; already completed private transcripts remain until you delete them.',
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context, false),
              child: const Text('Not now'),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(context, true),
              child: const Text('Allow and transcribe'),
            ),
          ],
        ),
      ) ??
      false;

  Future<void> _transcribe(VoiceDraft draft) async {
    if (working) return;
    setState(() {
      working = true;
      issue = null;
      receipt = null;
    });
    try {
      final permission = await widget.api.voicePermission();
      final data = permission['voice_transcription'] as Map<String, dynamic>;
      if (data['provider_available'] != true) {
        throw StateError(
          'Voice transcription is not configured on this backend.',
        );
      }
      if (data['enabled'] != true) {
        if (!await _confirmVoiceProcessing()) return;
        await widget.api.setVoicePermission(true);
      }
      final file = await widget.store.readyFile(widget.ownerId, draft);
      await widget.api.transcribeVoice(draft.id, draft.createdAtMs, file);
      await widget.store.acknowledgeTranscript(widget.ownerId, draft.id);
      await widget.onChanged();
      await _load();
      if (mounted) {
        setState(
          () => receipt = 'Private transcript saved. The local audio was queued for deletion. It is not a Library item yet.',
        );
      }
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
    } finally {
      if (mounted) setState(() => working = false);
    }
  }

  Future<void> _withdraw() async {
    if (working) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Turn off voice transcription?'),
        content: const Text(
          'New voice processing will stop after the server confirms this change. Unprocessed voice drafts on this device will be deleted. Saved private transcripts will remain.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Keep it on'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Turn off and delete drafts'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    setState(() => working = true);
    try {
      await widget.api.setVoicePermission(false);
      await widget.store.deleteAll(widget.ownerId);
      await widget.onChanged();
      await _load();
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
    } finally {
      if (mounted) setState(() => working = false);
    }
  }

  Future<void> _deleteTranscript(VoiceCapture capture) async {
    if (working) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Delete private transcript?'),
        content: const Text(
          'This removes the saved text. The audio has already been removed from Rekky.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Delete transcript'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    setState(() => working = true);
    try {
      await widget.api.deleteVoiceTranscript(capture);
      await _load();
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
    } finally {
      if (mounted) setState(() => working = false);
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
  Widget build(BuildContext context) => PopScope(
    canPop: !working,
    child: Padding(
      padding: const EdgeInsets.fromLTRB(24, 8, 24, 32),
      child: SafeArea(
        top: false,
        child: ConstrainedBox(
          constraints: BoxConstraints(
            maxHeight: MediaQuery.sizeOf(context).height * 0.75,
          ),
          child: SingleChildScrollView(
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
                  'Draft audio stays on this device until you choose Transcribe. Transcripts are private and separate from Library.',
                ),
                const SizedBox(height: 16),
                if (working) const LinearProgressIndicator(),
                if (issue != null)
                  Text(
                    issue!,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                if (receipt != null) Text(receipt!),
                if (!providerAvailable && drafts.isNotEmpty)
                  const Text(
                    'Transcription is paused while the provider settings are reviewed. Your drafts remain on this device.',
                  ),
                if (drafts.isEmpty)
                  const Text('No voice drafts on this device.'),
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
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        if (draft.status == 'ready')
                          TextButton(
                            onPressed: working || !providerAvailable
                                ? null
                                : () => _transcribe(draft),
                            child: const Text('Transcribe'),
                          ),
                        IconButton(
                          tooltip: 'Delete voice draft',
                          onPressed: working ? null : () => _delete(draft),
                          icon: const Icon(Icons.delete_outline),
                        ),
                      ],
                    ),
                  ),
                const Divider(),
                Text(
                  'Private transcripts',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                if (captures.isEmpty) const Text('No private transcripts yet.'),
                for (final capture in captures)
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    title: Text(
                      capture.transcript,
                      maxLines: 3,
                      overflow: TextOverflow.ellipsis,
                    ),
                    subtitle: Text('Saved ${capture.createdAt}'),
                    onTap: () => showDialog<void>(
                      context: context,
                      builder: (context) => AlertDialog(
                        title: const Text('Private transcript'),
                        content: SingleChildScrollView(
                          child: SelectableText(capture.transcript),
                        ),
                        actions: [
                          TextButton(
                            onPressed: () => Navigator.pop(context),
                            child: const Text('Close'),
                          ),
                        ],
                      ),
                    ),
                    trailing: IconButton(
                      tooltip: 'Delete private transcript',
                      onPressed: working
                          ? null
                          : () => _deleteTranscript(capture),
                      icon: const Icon(Icons.delete_outline),
                    ),
                  ),
                if (permissionEnabled)
                  TextButton(
                    onPressed: working ? null : _withdraw,
                    child: const Text('Turn off voice transcription'),
                  )
                else if (providerAvailable)
                  const Text(
                    'Transcription permission will be requested when you choose a draft.',
                  ),
                const SizedBox(height: 12),
                TextButton(
                  onPressed: working ? null : () => Navigator.pop(context),
                  child: const Text('Close'),
                ),
              ],
            ),
          ),
        ),
      ),
    ),
  );
}
