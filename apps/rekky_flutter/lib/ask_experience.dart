import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import 'ask_answer.dart';
import 'rekky_api.dart';
import 'rekky_haptics.dart';
import 'library_style.dart';
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
  AskAnswer? answer;
  String asked = '', pending = '', requestId = '', openingId = '';
  String? error;
  bool working = false, paging = false;
  int generation = 0;
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
      paging = false;
      error = null;
      pending = question;
      if (!retry) requestId = newId();
    });
    try {
      if (previousRequest != null) await cancelRequest(previousRequest);
      if (!mounted || turn != generation) return;
      final result = await widget.api.askAgent(question, requestId);
      if (!mounted || turn != generation) return;
      setState(() {
        answer = result;
        asked = question;
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

  Future<void> open(AskResult result, {bool action = false}) async {
    if (openingId.isNotEmpty) return;
    setState(() => openingId = result.item.id);
    try {
      final item = await widget.api.item(result.item.id);
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
    return CustomScrollView(
      key: const PageStorageKey('intelligent-ask'),
      keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
      slivers: [
        SliverPadding(
          padding: const EdgeInsets.fromLTRB(20, 12, 20, 16),
          sliver: SliverToBoxAdapter(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  current == null ? 'What do you have in mind?' : 'Ask Rekky',
                  style: LibraryStyle.heading(
                    context,
                    current == null ? 32 : 27,
                  ),
                ),
                const SizedBox(height: 12),
                if (current == null)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 24),
                    child: Text(
                      'A half-remembered name. A plan for tonight.\nStart with what you know.',
                      style: theme.textTheme.bodyMedium?.copyWith(
                        color: colors.onSurfaceVariant,
                      ),
                    ),
                  ),
                if (current == null)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 14),
                    child: Text(
                      'Ask uses OpenAI with your question and relevant saved recommendations.',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: colors.onSurfaceVariant,
                      ),
                    ),
                  ),
                TextField(
                  controller: input,
                  minLines: 1,
                  maxLines: 3,
                  maxLength: 500,
                  textInputAction: TextInputAction.search,
                  onSubmitted: (_) => submit(),
                  decoration: InputDecoration(
                    hintText: 'Somewhere we can sit and talk…',
                    counterText: '',
                    prefixIcon: const Icon(Icons.search_rounded),
                    suffixIcon: IconButton(
                      tooltip: working ? 'Ask a new question' : 'Ask',
                      onPressed: () => submit(),
                      icon: const Icon(Icons.arrow_forward_rounded),
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(12),
                      borderSide: BorderSide(
                        color: LibraryStyle.searchFocus(context),
                        width: 2,
                      ),
                    ),
                  ),
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
                          child: Text(
                            error!,
                            style: TextStyle(color: colors.error),
                          ),
                        ),
                        TextButton(
                          onPressed: working ? null : () => submit(retry: true),
                          child: const Text('Try again'),
                        ),
                      ],
                    ),
                  ),
                if (current == null && !working && error == null) ...[
                  const SizedBox(height: 28),
                  for (final example in [
                    'Who was that taxi driver?',
                    'Something fun to try this weekend',
                    'A place for a quiet dinner',
                  ])
                    Padding(
                      padding: const EdgeInsets.only(bottom: 8),
                      child: TextButton(
                        onPressed: () {
                          input.text = example;
                          input.selection = TextSelection.collapsed(
                            offset: example.length,
                          );
                        },
                        style: TextButton.styleFrom(
                          foregroundColor: colors.onSurface,
                          alignment: Alignment.centerLeft,
                          padding: const EdgeInsets.symmetric(
                            vertical: 14,
                            horizontal: 12,
                          ),
                        ),
                        child: Row(
                          children: [
                            Expanded(child: Text(example)),
                            Icon(
                              Icons.north_west_rounded,
                              size: 16,
                              color: LibraryStyle.searchFocus(context),
                            ),
                          ],
                        ),
                      ),
                    ),
                ],
                if (current != null) ...[
                  const SizedBox(height: 24),
                  Text(
                    asked,
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: colors.onSurfaceVariant,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    current.title.isEmpty
                        ? 'Your saved knowledge'
                        : current.title,
                    style: LibraryStyle.heading(context, 26),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    current.location.isEmpty
                        ? 'From your Library'
                        : 'From your Library · ${current.location}',
                    style: theme.textTheme.labelMedium?.copyWith(
                      color: LibraryStyle.searchFocus(context),
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
                    Text(
                      current.clarification,
                      style: theme.textTheme.titleMedium,
                    ),
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
                                    input.text = '$asked — $choice';
                                    submit();
                                  },
                                ),
                              )
                              .toList(),
                        ),
                      ),
                  ],
                  if (current.results.isEmpty && current.clarification.isEmpty)
                    const Padding(
                      padding: EdgeInsets.only(top: 20),
                      child: Text(
                        'There isn’t enough in your Library to answer this yet. Try another detail or a broader question.',
                      ),
                    ),
                ],
              ],
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
            onTap: openingId.isEmpty ? () => open(result) : null,
            child: Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 12),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Container(
                    width: 36,
                    height: 40,
                    decoration: BoxDecoration(
                      color: LibraryStyle.itemFill(context, item),
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Icon(
                      LibraryStyle.itemIcon(item),
                      color: LibraryStyle.itemForeground(context),
                      size: 21,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          item.subject,
                          style: theme.textTheme.titleMedium?.copyWith(
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                        if (location != null)
                          Text(
                            location,
                            style: theme.textTheme.bodySmall?.copyWith(
                              color: colors.onSurfaceVariant,
                            ),
                          ),
                      ],
                    ),
                  ),
                  const SizedBox(width: 8),
                  const Icon(Icons.arrow_outward_rounded, size: 18),
                ],
              ),
            ),
          ),
          if (result.reason.isNotEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
              child: Text(result.reason, style: theme.textTheme.bodyMedium),
            ),
          if (result.caveat.isNotEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
              child: Text(
                result.caveat,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: colors.onSurfaceVariant,
                ),
              ),
            ),
          Padding(
            padding: const EdgeInsets.fromLTRB(8, 0, 8, 4),
            child: Wrap(
              spacing: 8,
              children: [
                if (phone != null || maps != null)
                  TextButton.icon(
                    onPressed: openingId.isEmpty
                        ? () => open(result, action: true)
                        : null,
                    icon: Icon(
                      phone != null ? Icons.call_outlined : Icons.map_outlined,
                      size: 18,
                    ),
                    label: Text(phone != null ? 'Call' : 'Maps'),
                  ),
                TextButton(
                  onPressed: () => showModalBottomSheet<void>(
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
                                item.subject,
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
                              Text(
                                'From your saved recommendation',
                                style: theme.textTheme.bodySmall,
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                  child: const Text('Why this fits'),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
