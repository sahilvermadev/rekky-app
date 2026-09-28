import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import 'ask_answer.dart';
import 'ask_view.dart';
import 'ask_explorer.dart';
import 'ask_comparison_view.dart';
import 'ask_voice_sheet.dart';
import 'ask_recorder.dart';
import 'rekky_api.dart';
import 'rekky_haptics.dart';
import 'library_style.dart';
import 'rekky_theme.dart';
import 'recommendation_maps_action.dart';
import 'contact_matching.dart';

// Hallmark · Ask: evidence-led answers · existing personal-field-guide tokens.
// Pre-emit critique: P5 H4 E4 S5 R5 V4. Native, accessible, stable answer layout.
class AskExperience extends StatefulWidget {
  const AskExperience({
    super.key,
    required this.api,
    required this.onOpen,
    this.resolveCity,
    this.onOverlayChanged,
    this.homeSignal = 0,
  });
  final RekkyApi api;
  final Future<void> Function(RekkyItem) onOpen;
  final Future<String?> Function()? resolveCity;
  final ValueChanged<bool>? onOverlayChanged;

  /// Increment when the Ask destination is tapped, including while selected.
  final int homeSignal;
  @override
  State<AskExperience> createState() => _AskExperienceState();
}

class _PausedAskThread {
  _PausedAskThread({
    required this.answer,
    required this.history,
    required this.asked,
    required this.requestId,
    required this.draft,
    required this.activeViewId,
    required this.explorerView,
    required this.explorerCreated,
    required this.explorerVersion,
    required this.selected,
    required this.excluded,
    required this.viewSelectionItems,
    required this.lastAnswerAt,
    required this.scrollOffset,
  });
  final AskAnswer? answer;
  final List<({AskAnswer answer, String question})> history;
  final String asked, requestId, draft;
  final String? activeViewId;
  final AskView? explorerView;
  final bool explorerCreated;
  final int explorerVersion;
  final List<String> selected, excluded;
  final List<RekkyItem> viewSelectionItems;
  final DateTime? lastAnswerAt;
  final double scrollOffset;
}

class _AskThinkingIndicator extends StatefulWidget {
  const _AskThinkingIndicator();

  @override
  State<_AskThinkingIndicator> createState() => _AskThinkingIndicatorState();
}

class _AskThinkingIndicatorState extends State<_AskThinkingIndicator>
    with SingleTickerProviderStateMixin {
  late final AnimationController pulse = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 1350),
  );

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (MediaQuery.disableAnimationsOf(context)) {
      pulse.stop();
    } else if (!pulse.isAnimating) {
      pulse.repeat();
    }
  }

  @override
  void dispose() {
    pulse.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final color = Theme.of(context).colorScheme.onSurface;
    return Semantics(
      label: 'Rekky is thinking',
      liveRegion: true,
      child: AnimatedBuilder(
        animation: pulse,
        builder: (context, _) => SizedBox(
          width: 54,
          height: 24,
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              for (var index = 0; index < 3; index++)
                Builder(
                  builder: (context) {
                    final wave =
                        (sin(pulse.value * 2 * pi - index * .9) + 1) / 2;
                    return Transform.translate(
                      offset: Offset(0, -2 * wave),
                      child: Transform.scale(
                        scale: .78 + .22 * wave,
                        child: Container(
                          width: 8,
                          height: 8,
                          decoration: BoxDecoration(
                            shape: BoxShape.circle,
                            color: color.withValues(alpha: .28 + .62 * wave),
                          ),
                        ),
                      ),
                    );
                  },
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _AskExperienceState extends State<AskExperience> {
  final input = TextEditingController();
  final homeInput = TextEditingController();
  final threadScroll = ScrollController();
  bool onHome = true, explorerFromHome = true;
  _PausedAskThread? pausedThread;
  DateTime? lastAnswerAt;
  AskView? explorerView;
  bool showExplorer = false, explorerCreated = false;
  String? activeViewId, pendingViewId;
  List<RekkyItem> viewSelectionItems = [];
  int explorerVersion = 0;
  final inputFocus = FocusNode();
  final homeFocus = FocusNode();
  final history = <({AskAnswer answer, String question})>[];
  final selected = <String>[];
  final excluded = <String>[];
  List<String> pendingSelected = [], pendingExcluded = [];
  String? pendingParent, editParent;
  bool speaking = false, restoring = false, editing = false, comparing = false;

  AskAnswer? answer;
  String asked = '', pending = '', requestId = '', openingId = '';
  String? deviceCity, pendingCity;
  bool cityChecked = false, checkingCity = false;
  String? error;
  bool working = false, paging = false;
  String streamedReply = '';
  bool cardsPending = false;
  int generation = 0;
  bool? reportedOverlay;
  @override
  void initState() {
    super.initState();
    inputFocus.addListener(_focusChanged);
    homeFocus.addListener(_focusChanged);
    unawaited(AskRecorder.clearAbandoned().catchError((_) {}));
  }

  @override
  void didUpdateWidget(covariant AskExperience oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.homeSignal != oldWidget.homeSignal) {
      FocusManager.instance.primaryFocus?.unfocus();
      setState(() {
        showExplorer = false;
        onHome = true;
      });
    }
  }

  void _focusChanged() {
    if (mounted) setState(() {});
  }

  Future<void> cancelRequest(String id) async {
    if (id.isEmpty) return;
    try {
      await widget.api.cancelAsk(id);
    } catch (_) {
      /* Backend deadlines still bound uncertain work. */
    }
  }

  void stopResponse() {
    if (!working) return;
    final id = requestId;
    setState(() {
      generation++;
      working = false;
      streamedReply = '';
      cardsPending = false;
      checkingCity = false;
      input.text = pending;
    });
    unawaited(cancelRequest(id));
  }

  @override
  void dispose() {
    if (working) unawaited(cancelRequest(requestId));
    generation++;
    input.dispose();
    homeInput.dispose();
    threadScroll.dispose();
    inputFocus.removeListener(_focusChanged);
    inputFocus.dispose();
    homeFocus.removeListener(_focusChanged);
    homeFocus.dispose();
    super.dispose();
  }

  String newId() {
    final bytes = List.generate(16, (_) => Random.secure().nextInt(256));
    bytes[6] = (bytes[6] & 15) | 64;
    bytes[8] = (bytes[8] & 63) | 128;
    final hex = bytes.map((v) => v.toRadixString(16).padLeft(2, '0')).join();
    return '${hex.substring(0, 8)}-${hex.substring(8, 12)}-${hex.substring(12, 16)}-${hex.substring(16, 20)}-${hex.substring(20)}';
  }

  Future<void> submit({bool retry = false}) async {
    final question = retry ? pending : input.text.trim();
    if (question.isEmpty) return;
    if (answer != null && !_threadIsValid(lastAnswerAt)) {
      setState(
        () => error = 'This conversation expired. Start a new question.',
      );
      return;
    }
    FocusScope.of(context).unfocus();
    final previousRequest = working ? requestId : null;
    final turn = ++generation;
    setState(() {
      onHome = false;
      working = true;
      streamedReply = '';
      cardsPending = false;
      restoring = false;
      paging = false;
      error = null;
      pending = question;
      input.clear();
      if (!retry) {
        requestId = newId();
        pendingParent = editing ? editParent : answer?.requestId;
        pendingViewId = activeViewId;
        pendingSelected = List.of(selected);
        pendingExcluded = List.of(excluded);
      }
    });
    try {
      if (previousRequest != null) await cancelRequest(previousRequest);
      if (!mounted || turn != generation) return;
      if (!retry && !cityChecked && widget.resolveCity != null) {
        setState(() => checkingCity = true);
        final city = await widget.resolveCity!();
        if (!mounted || turn != generation) return;
        setState(() {
          deviceCity = city;
          cityChecked = true;
          checkingCity = false;
        });
      }
      if (!retry) pendingCity = deviceCity;
      final result = await widget.api.askAgentStream(
        question,
        requestId,
        scopeCity: pendingCity,
        activeViewId: pendingViewId,
        previousRequestId: pendingParent,
        selectedItemIds: pendingSelected,
        excludedItemIds: pendingExcluded,
        onText: (chunk) {
          if (mounted && turn == generation) {
            final follow =
                !threadScroll.hasClients ||
                threadScroll.position.maxScrollExtent - threadScroll.offset <
                    120;
            setState(() => streamedReply += chunk);
            if (follow) {
              WidgetsBinding.instance.addPostFrameCallback((_) {
                if (mounted && turn == generation && threadScroll.hasClients) {
                  threadScroll.jumpTo(threadScroll.position.maxScrollExtent);
                }
              });
            }
          }
        },
        onReset: () {
          if (mounted && turn == generation) {
            setState(() {
              streamedReply = '';
              cardsPending = false;
            });
          }
        },
        onCardsPending: () {
          if (mounted && turn == generation) {
            setState(() => cardsPending = true);
          }
        },
      );
      if (!mounted || turn != generation) return;
      if (result.mode == 'limited') {
        throw StateError(
          'Ask could not finish checking your saved recommendations. Try again.',
        );
      }
      setState(() {
        if (answer != null) history.add((answer: answer!, question: asked));
        if (history.length > 8) history.removeAt(0);
        answer = result;
        lastAnswerAt = DateTime.now();
        asked = question;
        selected.clear();
        excluded.clear();
        editing = false;
        comparing = false;
        input.clear();
        working = false;
        streamedReply = '';
        cardsPending = false;
      });
    } catch (e) {
      if (!mounted || turn != generation) return;
      RekkyHaptics.warning();
      setState(() {
        working = false;
        streamedReply = '';
        cardsPending = false;
        checkingCity = false;
        input.text = question;
        if (e is ApiFailure && e.code != 'ask_running') requestId = newId();
        error = e is ApiFailure
            ? e.message
            : e is StateError
            ? e.message
            : 'Couldn’t connect. Your previous answer is still here.';
      });
    }
  }

  bool _threadIsValid(DateTime? lastAnswer) =>
      lastAnswer != null &&
      DateTime.now().difference(lastAnswer) < const Duration(minutes: 14);

  bool get _hasCurrentThread =>
      answer != null || history.isNotEmpty || working || input.text.isNotEmpty;

  bool get _canResumeCurrent =>
      _hasCurrentThread &&
      (answer == null ? true : _threadIsValid(lastAnswerAt));

  bool get _canResumePaused =>
      pausedThread != null &&
      (pausedThread!.answer == null ||
          _threadIsValid(pausedThread!.lastAnswerAt));

  _PausedAskThread _snapshotThread() => _PausedAskThread(
    answer: answer,
    history: List.of(history),
    asked: asked,
    requestId: requestId,
    draft: input.text,
    activeViewId: activeViewId,
    explorerView: explorerView,
    explorerCreated: explorerCreated,
    explorerVersion: explorerVersion,
    selected: List.of(selected),
    excluded: List.of(excluded),
    viewSelectionItems: List.of(viewSelectionItems),
    lastAnswerAt: lastAnswerAt,
    scrollOffset: threadScroll.hasClients ? threadScroll.offset : 0,
  );

  void _resumePausedThread() {
    final previous = pausedThread;
    if (previous == null || !_canResumePaused) return;
    final current = _hasCurrentThread ? _snapshotThread() : null;
    setState(() {
      pausedThread = current;
      answer = previous.answer;
      history
        ..clear()
        ..addAll(previous.history);
      asked = previous.asked;
      requestId = previous.requestId;
      input.text = previous.draft;
      activeViewId = previous.activeViewId;
      explorerView = previous.explorerView;
      explorerCreated = previous.explorerCreated;
      explorerVersion = previous.explorerVersion;
      selected
        ..clear()
        ..addAll(previous.selected);
      excluded
        ..clear()
        ..addAll(previous.excluded);
      viewSelectionItems = previous.viewSelectionItems;
      lastAnswerAt = previous.lastAnswerAt;
      pending = '';
      error = null;
      editing = false;
      comparing = false;
      showExplorer = false;
      onHome = false;
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && threadScroll.hasClients) {
        threadScroll.jumpTo(
          previous.scrollOffset.clamp(
            0.0,
            threadScroll.position.maxScrollExtent,
          ),
        );
      }
    });
  }

  void _askFromHome() {
    final question = homeInput.text.trim();
    if (question.isEmpty) return;
    if (working) unawaited(cancelRequest(requestId));
    if (answer != null ||
        history.isNotEmpty ||
        (!working && input.text.isNotEmpty)) {
      pausedThread = _snapshotThread();
    }
    _clearThread();
    input.text = question;
    homeInput.clear();
    submit();
  }

  void _clearThread() {
    generation++;
    answer = null;
    activeViewId = null;
    explorerCreated = false;
    showExplorer = false;
    asked = '';
    pending = '';
    requestId = '';
    working = false;
    restoring = false;
    paging = false;
    error = null;
    editing = false;
    comparing = false;
    lastAnswerAt = null;
    history.clear();
    selected.clear();
    excluded.clear();
    input.clear();
    cityChecked = false;
  }

  Future<void> previousAnswer() async {
    if (history.isEmpty || restoring) return;
    if (working) unawaited(cancelRequest(requestId));
    final turn = ++generation;
    final previous = history.last;
    setState(() {
      restoring = true;
      working = false;
      error = null;
    });
    try {
      final fresh = await widget.api.askPage(previous.answer.requestId, 0);
      if (!mounted || turn != generation) return;
      setState(() {
        history.removeLast();
        answer = fresh;
        asked = previous.question;
        input.clear();
        selected.clear();
        excluded.clear();
        editing = false;
        comparing = false;
      });
    } catch (e) {
      if (mounted && turn == generation) {
        setState(
          () => error = e is ApiFailure ? e.message : 'Couldn’t restore that answer. Your current answer is still here.',
        );
      }
    } finally {
      if (mounted && turn == generation) setState(() => restoring = false);
    }
  }

  Future<void> speak({bool fromHome = false}) async {
    if (speaking || restoring) return;
    if (working) {
      unawaited(cancelRequest(requestId));
      generation++;
      setState(() => working = false);
    }
    inputFocus.unfocus();
    setState(() => speaking = true);
    // Bind the sheet to the current session so an account switch cannot rebind audio.
    final scoped = RekkyApi(widget.api.baseUrl)..token = widget.api.token;
    final text = await showModalBottomSheet<String>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      isDismissible: false,
      enableDrag: false,
      showDragHandle: false,
      builder: (_) => AskVoiceSheet(api: scoped, requestId: newId()),
    );
    if (!mounted) return;
    setState(() => speaking = false);
    if (text != null && text.trim().isNotEmpty) {
      if (fromHome) {
        homeInput.text = text;
        _askFromHome();
      } else {
        input.text = text;
        await submit();
      }
    }
  }

  Widget composer(BuildContext context, {bool fromHome = false}) {
    final current = fromHome ? null : answer;
    final controller = fromHome ? homeInput : input;
    final focusNode = fromHome ? homeFocus : inputFocus;
    void send() => fromHome ? _askFromHome() : submit();
    final colors = Theme.of(context).colorScheme;
    final askBlue = LibraryStyle.searchFocus(context);
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (!fromHome && selected.isNotEmpty) ...[
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: [
                for (final id in selected)
                  Padding(
                    padding: const EdgeInsets.only(right: 6),
                    child: InputChip(
                      label: ConstrainedBox(
                        constraints: const BoxConstraints(maxWidth: 160),
                        child: Text(
                          [...viewSelectionItems, ...?current?.items]
                                  .where((item) => item.id == id)
                                  .firstOrNull
                                  ?.subject ??
                              'Selected recommendation',
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      onDeleted: working
                          ? null
                          : () => setState(() => selected.remove(id)),
                    ),
                  ),
              ],
            ),
          ),
          if (comparing && selected.length >= 2)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: FilledButton.icon(
                onPressed: working || restoring || speaking
                    ? null
                    : () {
                        if (input.text.trim().isEmpty) input.text = 'Compare these options for what I asked about. Show the important differences and what is not saved.';
                        submit();
                      },
                icon: const Icon(Icons.compare_arrows_rounded, size: 20),
                label: Text('Compare ${selected.length}'),
                style: FilledButton.styleFrom(
                  backgroundColor: RekkyTheme.navAsk,
                  foregroundColor: RekkyTheme.onNavAsk,
                  minimumSize: const Size.fromHeight(48),
                ),
              ),
            ),
        ],
        AnimatedContainer(
          duration: MediaQuery.disableAnimationsOf(context)
              ? Duration.zero
              : const Duration(milliseconds: 150),
          decoration: BoxDecoration(
            color: colors.surfaceContainerLow,
            borderRadius: BorderRadius.circular(20),
            border: Border.all(
              color: focusNode.hasFocus ? askBlue : colors.outlineVariant,
            ),
          ),
          child: TextField(
            controller: controller,
            focusNode: focusNode,
            readOnly: !fromHome && working,
            minLines: 1,
            maxLines: 3,
            maxLength: 500,
            textInputAction: TextInputAction.search,
            style: Theme.of(context).textTheme.bodyLarge,
            onSubmitted: (_) {
              if (!working || fromHome) send();
            },
            decoration: InputDecoration(
              filled: false,
              border: InputBorder.none,
              enabledBorder: InputBorder.none,
              focusedBorder: InputBorder.none,
              errorBorder: InputBorder.none,
              focusedErrorBorder: InputBorder.none,
              contentPadding: const EdgeInsets.symmetric(vertical: 17),
              hintText: fromHome
                  ? 'Ask Rekky…'
                  : current == null
                  ? 'Ask what you have saved…'
                  : editing
                  ? 'Edit your question…'
                  : 'Ask a follow-up…',
              hintStyle: TextStyle(color: colors.onSurfaceVariant),
              counterText: '',
              prefixIconConstraints: const BoxConstraints(minWidth: 52),
              prefixIcon: IconButton(
                tooltip: 'Speak a question',
                onPressed: speaking || restoring || working
                    ? null
                    : () => speak(fromHome: fromHome),
                icon: Icon(Icons.mic_none_rounded, color: askBlue),
              ),
              suffixIconConstraints: const BoxConstraints(minWidth: 56),
              suffixIcon: Padding(
                padding: const EdgeInsets.only(right: 5),
                child: IconButton.filled(
                  tooltip: !fromHome && working
                      ? 'Stop response'
                      : current == null
                      ? 'Ask'
                      : 'Ask follow-up',
                  onPressed: speaking || restoring
                      ? null
                      : !fromHome && working
                      ? stopResponse
                      : send,
                  style: IconButton.styleFrom(
                    backgroundColor: RekkyTheme.navAsk,
                    foregroundColor: RekkyTheme.onNavAsk,
                    minimumSize: const Size(48, 48),
                  ),
                  icon: Icon(
                    !fromHome && working
                        ? Icons.stop_rounded
                        : Icons.arrow_upward_rounded,
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }

  Future<void> more() async {
    final current = answer;
    final turn = generation;
    if (current == null || current.nextOffset == null || paging || working) {
      return;
    }
    setState(() {
      paging = true;
      error = null;
    });
    try {
      final page = await widget.api.askPage(
        current.requestId,
        current.nextOffset!,
      );
      if (mounted && turn == generation) {
        setState(() => answer = current.append(page));
      }
    } catch (e) {
      if (mounted && turn == generation) {
        setState(
          () => error = e is ApiFailure
              ? e.message
              : 'Couldn’t load more. Try again.',
        );
      }
    } finally {
      if (mounted && turn == generation) setState(() => paging = false);
    }
  }

  Future<void> open(RekkyItem source, {bool action = false}) async {
    if (openingId.isNotEmpty) return;
    setState(() => openingId = source.id);
    try {
      final item = await widget.api.item(source.id);
      if (!mounted) return;
      if (action) {
        final number = internationalPhone(
          item.recommendation?.contactPhone ?? '',
        );
        final destination = recommendationDestination(item);
        final uri = number == null
            ? destination
            : Uri(scheme: 'tel', path: number);
        if (uri == null ||
            !await launchUrl(uri, mode: LaunchMode.externalApplication)) {
          throw StateError('Action unavailable');
        }
      } else {
        final turn = generation;
        await widget.onOpen(item);
        if (mounted && turn == generation && answer != null) {
          final refreshed = await widget.api.askPage(answer!.requestId, 0);
          if (mounted && turn == generation) setState(() => answer = refreshed);
        }
      }
    } catch (e) {
      if (mounted) {
        RekkyHaptics.warning();
        setState(
          () => error = e is ApiFailure ? e.message : 'This action isn’t available right now. Try opening the recommendation.',
        );
      }
    } finally {
      if (mounted) setState(() => openingId = '');
    }
  }

  void explore([AskView? view]) {
    setState(() {
      explorerFromHome = onHome;
      if (!explorerCreated || view?.id != explorerView?.id) {
        explorerView = view;
        explorerVersion++;
      }
      explorerCreated = true;
      showExplorer = true;
    });
  }

  Widget questionBubble(BuildContext context, String question) {
    final colors = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.only(top: 20, bottom: 18),
      child: Align(
        alignment: Alignment.centerRight,
        child: Container(
          constraints: const BoxConstraints(maxWidth: 330),
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          decoration: BoxDecoration(
            color: colors.surfaceContainerLow,
            borderRadius: BorderRadius.circular(18),
          ),
          child: Text(question, style: Theme.of(context).textTheme.bodyLarge),
        ),
      ),
    );
  }

  Widget replyView(
    BuildContext context,
    AskAnswer reply,
    String question, {
    bool latest = false,
  }) {
    final colors = Theme.of(context).colorScheme;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        questionBubble(context, question),
        if (reply.reply.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(bottom: 16),
            child: Text(
              reply.reply,
              style: Theme.of(context).textTheme.bodyLarge
                  ?.copyWith(height: 1.5),
            ),
          ),
        if (reply.changed)
          const Padding(
            padding: EdgeInsets.only(bottom: 12),
            child: Text(
              'Some recommendations changed. Ask again for an updated answer.',
            ),
          ),
        for (final view in reply.views)
          Padding(
            padding: const EdgeInsets.only(bottom: 16),
            child: Material(
              color: colors.surfaceContainerLow,
              borderRadius: BorderRadius.circular(18),
              clipBehavior: Clip.antiAlias,
              child: InkWell(
                onTap: () => explore(view),
                child: Padding(
                  padding: const EdgeInsets.all(18),
                  child: Row(
                    children: [
                      Icon(
                        Icons.view_agenda_outlined,
                        color: LibraryStyle.searchFocus(context),
                      ),
                      const SizedBox(width: 14),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              view.title,
                              style: LibraryStyle.heading(context, 23),
                            ),
                            const SizedBox(height: 6),
                            Text('${view.total} saved · Browse matches'),
                          ],
                        ),
                      ),
                      const Icon(Icons.arrow_forward_rounded),
                    ],
                  ),
                ),
              ),
            ),
          ),
        if (latest && reply.results.length >= 2 && reply.comparison == null)
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              onPressed: working
                  ? null
                  : () => setState(() {
                      comparing = !comparing;
                      selected.clear();
                    }),
              icon: const Icon(Icons.compare_arrows_rounded, size: 19),
              label: Text(comparing ? 'Done comparing' : 'Compare options'),
            ),
          ),
        if (reply.comparison != null)
          AskComparisonView(
            comparison: reply.comparison!,
            onOpen: (item) => open(item),
            onAction: (item) => open(item, action: true),
            onSelect: latest && !working ? toggleSelection : null,
            selected: latest ? selected : const [],
          ),
        for (final result in reply.results)
          _answerResultCard(context, result, latest: latest),
        if (reply.clarification.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 8, bottom: 12),
            child: Text(
              reply.clarification,
              style: Theme.of(context).textTheme.bodyLarge,
            ),
          ),
        if (latest && reply.choices.isNotEmpty)
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: reply.choices
                .map(
                  (choice) => ActionChip(
                    label: Text(choice),
                    onPressed: working
                        ? null
                        : () {
                            input.text = choice;
                            submit();
                          },
                  ),
                )
                .toList(),
          ),
        if (reply.results.isEmpty &&
            reply.views.isEmpty &&
            reply.comparison == null &&
            reply.clarification.isEmpty &&
            reply.reply.isEmpty &&
            !reply.changed)
          const Text(
            'No supported match found for this question. Try another detail or a broader scope.',
          ),
        if (latest && reply.nextOffset != null)
          OutlinedButton(
            onPressed: working || paging ? null : more,
            child: Text(paging ? 'Loading…' : 'More recommendations'),
          ),
        const SizedBox(height: 12),
      ],
    );
  }

  Widget _answerResultCard(
    BuildContext context,
    AskResult result, {
    required bool latest,
  }) {
    final card = Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: resultCard(context, result, interactive: latest),
    );
    if (!latest || MediaQuery.disableAnimationsOf(context)) return card;
    return TweenAnimationBuilder<double>(
      key: ValueKey('arrival-${result.item.id}'),
      tween: Tween(begin: 0, end: 1),
      duration: const Duration(milliseconds: 220),
      curve: Curves.easeOutCubic,
      builder: (context, progress, child) => Opacity(
        opacity: progress,
        child: Transform.translate(
          offset: Offset(0, 8 * (1 - progress)),
          child: child,
        ),
      ),
      child: card,
    );
  }

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final overlay = !onHome || showExplorer;
    if (reportedOverlay != overlay) {
      reportedOverlay = overlay;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && (!onHome || showExplorer) == overlay) {
          widget.onOverlayChanged?.call(overlay);
        }
      });
    }
    final home = Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (_canResumeCurrent)
                Align(
                  alignment: Alignment.centerLeft,
                  child: TextButton.icon(
                    onPressed: () => setState(() => onHome = false),
                    icon: const Icon(Icons.chat_bubble_outline_rounded),
                    label: const Text('Continue conversation'),
                  ),
                ),
              if (_canResumePaused)
                Align(
                  alignment: Alignment.centerLeft,
                  child: TextButton.icon(
                    onPressed: working ? null : _resumePausedThread,
                    icon: const Icon(Icons.history_rounded),
                    label: const Text('Continue previous conversation'),
                  ),
                ),
              composer(context, fromHome: true),
            ],
          ),
        ),
      ),
    );
    final conversation = Column(
      children: [
        Align(
          alignment: Alignment.centerLeft,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(8, 4, 0, 0),
            child: IconButton(
              tooltip: 'Back to Ask',
              onPressed: () {
                FocusManager.instance.primaryFocus?.unfocus();
                setState(() => onHome = true);
              },
              icon: const Icon(Icons.arrow_back_rounded),
            ),
          ),
        ),
        Expanded(
          child: ListView(
            controller: threadScroll,
            key: const PageStorageKey('ask-conversation'),
            keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
            padding: const EdgeInsets.fromLTRB(20, 8, 20, 20),
            children: [
              for (final turn in history)
                replyView(context, turn.answer, turn.question),
              if (answer != null)
                replyView(context, answer!, asked, latest: true),
              if (working || error != null) ...[
                if (working) ...[
                  questionBubble(context, pending),
                  if (streamedReply.isEmpty)
                    const Padding(
                      padding: EdgeInsets.only(left: 4, top: 2, bottom: 18),
                      child: Align(
                        alignment: Alignment.centerLeft,
                        child: _AskThinkingIndicator(),
                      ),
                    )
                  else
                    Padding(
                      padding: const EdgeInsets.only(bottom: 16),
                      child: Text(
                        streamedReply,
                        style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                          height: 1.5,
                          color: colors.onSurfaceVariant,
                        ),
                      ),
                    ),
                  if (cardsPending && streamedReply.isNotEmpty)
                    Semantics(
                      label: 'Recommendations are loading',
                      child: Container(
                        height: 72,
                        margin: const EdgeInsets.only(bottom: 16),
                        decoration: BoxDecoration(
                          color: colors.surfaceContainerLow,
                          borderRadius: BorderRadius.circular(16),
                        ),
                        child: const Align(
                          alignment: Alignment.centerLeft,
                          child: Padding(
                            padding: EdgeInsets.only(left: 16),
                            child: ExcludeSemantics(
                              child: _AskThinkingIndicator(),
                            ),
                          ),
                        ),
                      ),
                    ),
                ],
                if (error != null) ...[
                  Semantics(
                    liveRegion: true,
                    child: Text(error!, style: TextStyle(color: colors.error)),
                  ),
                  Align(
                    alignment: Alignment.centerLeft,
                    child: TextButton(
                      onPressed: working ? null : () => submit(retry: true),
                      child: const Text('Try again'),
                    ),
                  ),
                ],
              ],
            ],
          ),
        ),
        ConstrainedBox(
          constraints: BoxConstraints(
            maxHeight: MediaQuery.sizeOf(context).height * .35,
          ),
          child: SingleChildScrollView(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
              child: composer(context),
            ),
          ),
        ),
      ],
    );
    return PopScope(
      canPop: onHome && !showExplorer,
      onPopInvokedWithResult: (didPop, result) {
        if (!didPop) {
          setState(() {
            if (showExplorer) {
              showExplorer = false;
              onHome = explorerFromHome;
            } else {
              onHome = true;
            }
          });
        }
      },
      child: Stack(
        children: [
          Offstage(offstage: showExplorer || !onHome, child: home),
          Offstage(
            offstage: showExplorer || onHome,
            child: TickerMode(
              enabled: !showExplorer && !onHome,
              child: conversation,
            ),
          ),
          if (explorerCreated)
            Offstage(
              offstage: !showExplorer,
              child: AskExplorer(
                key: ValueKey(explorerVersion),
                api: widget.api,
                initialView: explorerView,
                onBack: () => setState(() {
                  showExplorer = false;
                  onHome = explorerFromHome;
                }),
                onOpen: widget.onOpen,
                onAsk: (view, ids, text) {
                  setState(() {
                    activeViewId = view.id;
                    viewSelectionItems = view.items;
                    selected
                      ..clear()
                      ..addAll(ids);
                    showExplorer = false;
                    onHome = false;
                  });
                  input.text = text;
                  submit();
                },
              ),
            ),
        ],
      ),
    );
  }

  void toggleSelection(RekkyItem item) {
    if (working) return;
    if (!selected.contains(item.id) && selected.length >= 4) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('Compare up to four recommendations at a time.'),
        ),
      );
      return;
    }
    RekkyHaptics.selection();
    setState(() {
      editing = false;
      if (!selected.remove(item.id)) selected.add(item.id);
      if (answer?.comparison != null) comparing = selected.length >= 2;
    });
  }

  bool _showCaveatOnCard(String caveat) {
    final lower = caveat.toLowerCase();
    final genericAvailability =
        lower.contains('past visit') && lower.contains('current availability');
    final availabilityAsked = RegExp(
      r'\b(open|opening|hours|available|availability|booking|book|reservation|reserve)\b',
    ).hasMatch(asked.toLowerCase());
    return !genericAvailability || availabilityAsked;
  }

  void _showSavedEvidence(BuildContext context, AskResult result) {
    final theme = Theme.of(context);
    showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (context) => SafeArea(
        child: SingleChildScrollView(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(24, 8, 24, 32),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  result.item.subject,
                  style: LibraryStyle.heading(context, 24),
                ),
                const SizedBox(height: 16),
                for (final evidence in result.evidence)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 16),
                    child: Text(
                      '“${evidence.text}”',
                      style: theme.textTheme.bodyLarge,
                    ),
                  ),
                if (result.caveat.isNotEmpty)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 12),
                    child: Text(
                      result.caveat,
                      style: theme.textTheme.bodyMedium,
                    ),
                  ),
                Text(
                  'From your saved recommendation',
                  style: theme.textTheme.bodySmall,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  void _resultMenuAction(String action, RekkyItem item) {
    if (action == 'ask') {
      setState(() {
        comparing = false;
        selected.clear();
        selected.add(item.id);
        editing = false;
      });
      inputFocus.requestFocus();
    } else if (action == 'exclude') {
      setState(() {
        comparing = false;
        selected.clear();
        excluded.add(item.id);
      });
      input.text = 'Leave this option out and show other possibilities.';
      submit();
    }
  }

  Widget resultCard(
    BuildContext context,
    AskResult result, {
    bool interactive = true,
  }) {
    final item = result.item;
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final location = item.recommendation?.locations
        .map((l) => l.displayText)
        .firstOrNull;
    final phone = internationalPhone(item.recommendation?.contactPhone ?? '');
    final maps = recommendationDestination(item);
    return Material(
      key: ValueKey(item.id),
      color: colors.surfaceContainerLow,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: colors.outlineVariant.withValues(alpha: .35)),
      ),
      clipBehavior: Clip.antiAlias,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          InkWell(
            onTap: openingId.isEmpty ? () => open(result.item) : null,
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 12, 10),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Container(
                    width: 42,
                    height: 42,
                    decoration: BoxDecoration(
                      color: LibraryStyle.itemFill(context, item),
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Icon(
                      LibraryStyle.itemIcon(item),
                      color: LibraryStyle.itemForeground(context),
                      size: 22,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          item.subject,
                          style: theme.textTheme.titleLarge?.copyWith(
                            fontSize: 19,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                        if (location != null)
                          Text(
                            location,
                            style: theme.textTheme.bodyMedium?.copyWith(
                              color: colors.onSurfaceVariant,
                            ),
                          ),
                      ],
                    ),
                  ),
                  const SizedBox(width: 8),
                  if (comparing && interactive)
                    Semantics(
                      label: 'Select ${item.subject} for comparison',
                      child: Checkbox(
                        value: selected.contains(item.id),
                        onChanged: working
                            ? null
                            : (_) => toggleSelection(item),
                        activeColor: RekkyTheme.navAsk,
                        checkColor: RekkyTheme.onNavAsk,
                      ),
                    )
                  else
                    const Padding(
                      padding: EdgeInsets.only(top: 8),
                      child: Icon(Icons.chevron_right_rounded, size: 24),
                    ),
                ],
              ),
            ),
          ),
          if (result.reason.isNotEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 10),
              child: Text(result.reason, style: theme.textTheme.bodyLarge),
            ),
          if (result.caveat.isNotEmpty && _showCaveatOnCard(result.caveat))
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 10),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Icon(
                    Icons.info_outline_rounded,
                    size: 17,
                    color: colors.onSurfaceVariant,
                  ),
                  const SizedBox(width: 7),
                  Expanded(
                    child: Text(
                      result.caveat,
                      style: theme.textTheme.bodyMedium?.copyWith(
                        color: colors.onSurfaceVariant,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          Padding(
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 4),
            child: Wrap(
              spacing: 8,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                if (phone != null || maps != null)
                  TextButton.icon(
                    onPressed: openingId.isEmpty
                        ? () => open(result.item, action: true)
                        : null,
                    icon: Icon(
                      phone != null ? Icons.call_outlined : Icons.map_outlined,
                      size: 18,
                    ),
                    label: Text(phone != null ? 'Call' : 'Maps'),
                  ),
                if (result.evidence.isNotEmpty)
                  TextButton.icon(
                    onPressed: () => _showSavedEvidence(context, result),
                    icon: const Icon(Icons.format_quote_rounded, size: 18),
                    label: const Text('Saved note'),
                  ),
                PopupMenuButton<String>(
                  tooltip: 'More options for ${item.subject}',
                  enabled: !working && interactive,
                  onSelected: (action) => _resultMenuAction(action, item),
                  itemBuilder: (_) => const [
                    PopupMenuItem(value: 'ask', child: Text('Ask about this')),
                    PopupMenuItem(
                      value: 'exclude',
                      child: Text('Leave this out'),
                    ),
                  ],
                  icon: const Icon(Icons.more_horiz_rounded),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
