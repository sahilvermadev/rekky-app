import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import 'ask_answer.dart';
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
  const AskExperience({super.key, required this.api, required this.onOpen});
  final RekkyApi api;
  final Future<void> Function(RekkyItem) onOpen;
  @override
  State<AskExperience> createState() => _AskExperienceState();
}

class _AskExperienceState extends State<AskExperience> {
  final input = TextEditingController();
  final inputFocus = FocusNode();
  final history = <({AskAnswer answer, String question})>[];
  final selected = <String>[];
  final excluded = <String>[];
  List<String> pendingSelected = [], pendingExcluded = [];
  String? pendingParent, editParent;
  bool speaking = false, restoring = false, editing = false, comparing = false;

  AskAnswer? answer;
  String asked = '', pending = '', requestId = '', openingId = '';
  String? error;
  bool working = false, paging = false;
  int generation = 0;
  @override
  void initState() {
    super.initState();
    inputFocus.addListener(_focusChanged);
    unawaited(AskRecorder.clearAbandoned().catchError((_) {}));
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

  @override
  void dispose() {
    if (working) unawaited(cancelRequest(requestId));
    generation++;
    input.dispose();
    inputFocus.removeListener(_focusChanged);
    inputFocus.dispose();
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
    FocusScope.of(context).unfocus();
    final previousRequest = working ? requestId : null;
    final turn = ++generation;
    setState(() {
      working = true;
      restoring = false;
      paging = false;
      error = null;
      pending = question;
      if (!retry) {
        requestId = newId();
        pendingParent = editing ? editParent : answer?.requestId;
        pendingSelected = List.of(selected);
        pendingExcluded = List.of(excluded);
      }
    });
    try {
      if (previousRequest != null) await cancelRequest(previousRequest);
      if (!mounted || turn != generation) return;
      final result = await widget.api.askAgent(
        question,
        requestId,
        previousRequestId: pendingParent,
        selectedItemIds: pendingSelected,
        excludedItemIds: pendingExcluded,
      );
      if (!mounted || turn != generation) return;
      setState(() {
        if (answer != null) history.add((answer: answer!, question: asked));
        if (history.length > 8) history.removeAt(0);
        answer = result;
        asked = question;
        selected.clear();
        excluded.clear();
        editing = false;
        comparing = false;
        input.clear();
        working = false;
      });
    } catch (e) {
      if (!mounted || turn != generation) return;
      RekkyHaptics.warning();
      setState(() {
        working = false;
        if (e is ApiFailure && e.code != 'ask_running') requestId = newId();
        error = e is ApiFailure
            ? e.message
            : 'Couldn’t connect. Your previous answer is still here.';
      });
    }
  }

  void newQuestion() {
    if (working) unawaited(cancelRequest(requestId));
    generation++;
    setState(() {
      answer = null;
      asked = '';
      pending = '';
      requestId = '';
      working = false;
      restoring = false;
      paging = false;
      error = null;
      editing = false;
      comparing = false;
      history.clear();
      selected.clear();
      excluded.clear();
      input.clear();
    });
    inputFocus.requestFocus();
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

  Future<void> speak() async {
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
      input.text = text;
      await submit();
    }
  }

  Widget composer(BuildContext context) {
    final current = answer;
    final colors = Theme.of(context).colorScheme;
    final askBlue = LibraryStyle.searchFocus(context);
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (selected.isNotEmpty) ...[
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
                          current?.items
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
              color: inputFocus.hasFocus ? askBlue : colors.outlineVariant,
            ),
          ),
          child: TextField(
            controller: input,
            focusNode: inputFocus,
            minLines: 1,
            maxLines: 3,
            maxLength: 500,
            textInputAction: TextInputAction.search,
            style: Theme.of(context).textTheme.bodyLarge,
            onSubmitted: (_) => submit(),
            decoration: InputDecoration(
              filled: false,
              border: InputBorder.none,
              enabledBorder: InputBorder.none,
              focusedBorder: InputBorder.none,
              errorBorder: InputBorder.none,
              focusedErrorBorder: InputBorder.none,
              contentPadding: const EdgeInsets.symmetric(vertical: 17),
              hintText: current == null
                  ? 'Ask what you have saved…'
                  : editing
                  ? 'Edit your question…'
                  : 'Ask a follow-up…',
              hintStyle: TextStyle(color: colors.onSurfaceVariant),
              counterText: '',
              prefixIconConstraints: const BoxConstraints(minWidth: 52),
              prefixIcon: IconButton(
                tooltip: 'Speak a question',
                onPressed: speaking || restoring ? null : speak,
                icon: Icon(Icons.mic_none_rounded, color: askBlue),
              ),
              suffixIconConstraints: const BoxConstraints(minWidth: 56),
              suffixIcon: Padding(
                padding: const EdgeInsets.only(right: 5),
                child: IconButton.filled(
                  tooltip: current == null ? 'Ask' : 'Ask follow-up',
                  onPressed: speaking || restoring ? null : () => submit(),
                  style: IconButton.styleFrom(
                    backgroundColor: RekkyTheme.navAsk,
                    foregroundColor: RekkyTheme.onNavAsk,
                    minimumSize: const Size(48, 48),
                  ),
                  icon: const Icon(Icons.arrow_upward_rounded),
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

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final current = answer;
    final supported =
        current?.results.where((r) => r.section == 'supported').toList() ??
        <AskResult>[];
    final uncertain =
        current?.results.where((r) => r.section != 'supported').toList() ??
        <AskResult>[];
    final centerHome =
        current == null &&
        MediaQuery.textScalerOf(context).scale(16) <= 22 &&
        MediaQuery.sizeOf(context).height >= 720 &&
        MediaQuery.viewInsetsOf(context).bottom == 0;
    final intro = Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (current == null) ...[
          Text(
            'What do you have in mind?',
            style: LibraryStyle.heading(context, 32),
          ),
          const SizedBox(height: 10),
          Text(
            'Find something you saved, or ask what fits.',
            style: theme.textTheme.bodyLarge?.copyWith(
              color: colors.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: 26),
          composer(context),
        ],
        if (current != null)
          Wrap(
            spacing: 8,
            runSpacing: 2,
            children: [
              if (history.isNotEmpty)
                TextButton.icon(
                  onPressed: restoring ? null : previousAnswer,
                  icon: const Icon(Icons.arrow_back_rounded, size: 18),
                  label: Text(restoring ? 'Restoring…' : 'Previous answer'),
                ),
              TextButton.icon(
                onPressed: newQuestion,
                icon: const Icon(Icons.add_rounded, size: 18),
                label: const Text('New question'),
              ),
            ],
          ),
        if (working)
          Padding(
            padding: const EdgeInsets.only(top: 14),
            child: Row(
              children: [
                const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Semantics(
                    liveRegion: true,
                    child: Text('Finding useful connections…'),
                  ),
                ),
                TextButton(
                  onPressed: () {
                    unawaited(cancelRequest(requestId));
                    setState(() {
                      generation++;
                      working = false;
                    });
                  },
                  child: const Text('Cancel'),
                ),
              ],
            ),
          ),
        if (error != null)
          Padding(
            padding: const EdgeInsets.only(top: 12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Semantics(
                  liveRegion: true,
                  child: Text(error!, style: TextStyle(color: colors.error)),
                ),
                TextButton(
                  onPressed: working ? null : () => submit(retry: true),
                  child: const Text('Try again'),
                ),
              ],
            ),
          ),
        if (current == null && !working && error == null) ...[
          const SizedBox(height: 18),
          for (final example in [
            'Who was that taxi driver?',
            'Something fun to try this weekend',
            'A place for a quiet dinner',
          ])
            Padding(
              padding: const EdgeInsets.only(bottom: 4),
              child: InkWell(
                onTap: () {
                  input.text = example;
                  submit();
                },
                borderRadius: BorderRadius.circular(12),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    vertical: 13,
                    horizontal: 10,
                  ),
                  child: Row(
                    children: [
                      Expanded(
                        child: Text(example, style: theme.textTheme.bodyMedium),
                      ),
                      const SizedBox(width: 12),
                      Icon(
                        Icons.arrow_upward_rounded,
                        size: 18,
                        color: LibraryStyle.searchFocus(context),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          const SizedBox(height: 16),
          Text(
            'Ask sends your question and relevant saved recommendations to OpenAI.',
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.onSurfaceVariant,
            ),
          ),
        ],
        if (current != null) ...[
          const SizedBox(height: 14),
          InkWell(
            onTap: working
                ? null
                : () {
                    setState(() {
                      editing = true;
                      editParent = answer?.turnCount == 1
                          ? null
                          : history.lastOrNull?.answer.requestId;
                      input.text = asked;
                    });
                    inputFocus.requestFocus();
                  },
            child: Padding(
              padding: const EdgeInsets.symmetric(vertical: 10),
              child: Semantics(
                button: true,
                label: 'Edit question: $asked',
                child: Row(
                  children: [
                    Expanded(
                      child: Text(
                        asked,
                        style: LibraryStyle.heading(context, 24),
                      ),
                    ),
                    const SizedBox(width: 8),
                    const Icon(Icons.edit_outlined, size: 20),
                  ],
                ),
              ),
            ),
          ),
          if (current.results.length >= 2 && current.comparison == null)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: TextButton.icon(
                onPressed: working
                    ? null
                    : () => setState(() {
                        comparing = !comparing;
                        selected.clear();
                      }),
                icon: Icon(
                  comparing
                      ? Icons.close_rounded
                      : Icons.compare_arrows_rounded,
                  size: 19,
                ),
                label: Text(comparing ? 'Done comparing' : 'Compare options'),
              ),
            ),
          if (current.mode == 'limited')
            const Padding(
              padding: EdgeInsets.only(top: 8),
              child: Text(
                'The full answer couldn’t finish. These saved matches may help.',
              ),
            ),
          if (current.searchIncomplete && current.mode != 'limited')
            const Padding(
              padding: EdgeInsets.only(top: 8),
              child: Text(
                'There may be more matches. Add another detail to narrow your search.',
              ),
            ),
          if (current.changed)
            const Padding(
              padding: EdgeInsets.only(top: 8),
              child: Text(
                'Some recommendations changed. Ask again for an updated answer.',
              ),
            ),
          if (current.clarification.isNotEmpty) ...[
            const SizedBox(height: 16),
            Text(current.clarification, style: theme.textTheme.titleMedium),
            if (current.choices.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: current.choices
                      .map(
                        (choice) => ActionChip(
                          label: Text(choice),
                          onPressed: () {
                            input.text = choice;
                            submit();
                          },
                        ),
                      )
                      .toList(),
                ),
              ),
          ],
          if (current.results.isEmpty &&
              current.comparison == null &&
              !current.changed &&
              current.clarification.isEmpty)
            const Padding(
              padding: EdgeInsets.only(top: 20),
              child: Text(
                'There isn’t enough in your Library to answer this yet. Try another detail or a broader question.',
              ),
            ),
        ],
      ],
    );
    final content = CustomScrollView(
      key: const PageStorageKey('intelligent-ask'),
      keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
      slivers: [
        if (centerHome)
          SliverFillRemaining(
            hasScrollBody: false,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
              child: Center(
                child: SizedBox(width: double.infinity, child: intro),
              ),
            ),
          )
        else
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(20, 12, 20, 16),
            sliver: SliverToBoxAdapter(child: intro),
          ),
        if (current?.comparison != null)
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(20, 0, 20, 16),
            sliver: SliverToBoxAdapter(
              child: AskComparisonView(
                comparison: current!.comparison!,
                onOpen: (item) => open(item),
                onAction: (item) => open(item, action: true),
                onSelect: working ? null : toggleSelection,
                selected: selected,
              ),
            ),
          ),
        for (final result in supported)
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(20, 0, 20, 12),
            sliver: SliverToBoxAdapter(child: resultCard(context, result)),
          ),
        if (uncertain.isNotEmpty)
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(20, 12, 20, 12),
            sliver: SliverToBoxAdapter(
              child: Text('Worth checking', style: theme.textTheme.titleMedium),
            ),
          ),
        for (final result in uncertain)
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(20, 0, 20, 12),
            sliver: SliverToBoxAdapter(child: resultCard(context, result)),
          ),
        if (current?.nextOffset != null)
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(20, 0, 20, 20),
            sliver: SliverToBoxAdapter(
              child: OutlinedButton(
                onPressed: paging || working ? null : more,
                child: Text(paging ? 'Loading…' : 'More recommendations'),
              ),
            ),
          ),
      ],
    );
    return LayoutBuilder(
      builder: (context, constraints) => Column(
        children: [
          Expanded(child: content),
          if (current != null)
            ConstrainedBox(
              constraints: BoxConstraints(
                maxHeight: constraints.maxHeight * .5,
              ),
              child: SingleChildScrollView(
                reverse: true,
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
                  child: composer(context),
                ),
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

  Widget resultCard(BuildContext context, AskResult result) {
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
                  if (comparing)
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
                  enabled: !working,
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
