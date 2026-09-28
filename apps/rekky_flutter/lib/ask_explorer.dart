import 'package:flutter/material.dart';

import 'ask_view.dart';
import 'rekky_api.dart';
import 'library_style.dart';
import 'library_collection.dart';
import 'rekky_theme.dart';

// One native collection, configured by either the agent or direct user filters.
class AskExplorer extends StatefulWidget {
  const AskExplorer({
    super.key,
    required this.api,
    required this.onBack,
    required this.onOpen,
    required this.onAsk,
    this.initialView,
  });
  final RekkyApi api;
  final VoidCallback onBack;
  final Future<void> Function(RekkyItem) onOpen;
  final void Function(AskView, List<String>, String) onAsk;
  final AskView? initialView;
  @override
  State<AskExplorer> createState() => _AskExplorerState();
}

class _AskExplorerState extends State<AskExplorer> {
  AskView? view;
  AskBrowseSpec spec = const AskBrowseSpec();
  final scroll = ScrollController(), question = TextEditingController();
  final selected = <String>[];
  bool loading = false, paging = false;
  String? error;
  int generation = 0;
  @override
  void initState() {
    super.initState();
    view = widget.initialView;
    if (view != null) {
      spec = view!.spec;
    } else {
      load(spec);
    }
  }

  @override
  void dispose() {
    generation++;
    scroll.dispose();
    question.dispose();
    super.dispose();
  }

  Future<void> load(AskBrowseSpec next, {bool preserve = false}) async {
    final turn = ++generation;
    setState(() {
      loading = true;
      error = null;
    });
    try {
      final result = await widget.api.createAskView(next);
      if (!mounted || turn != generation) return;
      setState(() {
        view = result;
        spec = result.spec;
        // A new filter changes what the selection refers to. Refreshing the
        // same query retains it, including selected items beyond page one.
        if (!preserve) selected.clear();
      });
      if (!preserve && scroll.hasClients) scroll.jumpTo(0);
    } catch (e) {
      if (mounted && turn == generation) {
        setState(
          () => error = e is ApiFailure
              ? e.message
              : 'Couldn’t load this collection. Try again.',
        );
      }
    } finally {
      if (mounted && turn == generation) setState(() => loading = false);
    }
  }

  Future<void> more() async {
    final v = view;
    final turn = generation;
    if (v == null || v.nextOffset == null || paging || loading) return;
    setState(() {
      paging = true;
      error = null;
    });
    try {
      final page = await widget.api.askViewPage(v.id, v.nextOffset!);
      if (mounted && turn == generation) setState(() => view = v.append(page));
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

  String kindLabel(String id) =>
      LibraryShelf.values.where((s) => s.kind == id).firstOrNull?.label ?? id;
  Future<void> pick(
    String title,
    List<AskFacet> choices,
    String current,
    void Function(String) choose, {
    bool kinds = false,
  }) async {
    final result = await showModalBottomSheet<String>(
      context: context,
      useSafeArea: true,
      showDragHandle: true,
      isScrollControlled: true,
      builder: (context) => DraggableScrollableSheet(
        expand: false,
        initialChildSize: .55,
        builder: (context, controller) => ListView(
          controller: controller,
          children: [
            Padding(
              padding: const EdgeInsets.all(20),
              child: Text(title, style: LibraryStyle.heading(context, 26)),
            ),
            ListTile(
              title: const Text('All'),
              selected: current.isEmpty,
              onTap: () => Navigator.pop(context, ''),
            ),
            for (final f in choices)
              ListTile(
                title: Text(kinds ? kindLabel(f.id) : f.label),
                trailing: Text('${f.count}'),
                selected: current == f.id,
                onTap: () => Navigator.pop(context, f.id),
              ),
          ],
        ),
      ),
    );
    if (result != null && mounted) choose(result);
  }

  void ask() {
    final v = view;
    if (v == null || loading) return;
    final text = question.text.trim();
    question.clear();
    widget.onAsk(
      v,
      List.of(selected),
      text.isNotEmpty
          ? text
          : selected.length >= 2
          ? 'Compare these options.'
          : 'Help me choose from this collection.',
    );
  }

  @override
  Widget build(BuildContext context) {
    final v = view, colors = Theme.of(context).colorScheme;
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(8, 0, 12, 4),
          child: Row(
            children: [
              IconButton(
                tooltip: 'Back to conversation',
                onPressed: widget.onBack,
                icon: const Icon(Icons.arrow_back_rounded),
              ),
              const Expanded(child: Text('Explore')),
              IconButton(
                tooltip: 'Refresh collection',
                onPressed: loading ? null : () => load(spec, preserve: true),
                icon: const Icon(Icons.refresh_rounded),
              ),
            ],
          ),
        ),
        Expanded(
          child: CustomScrollView(
            controller: scroll,
            keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
            slivers: [
              SliverPadding(
                padding: const EdgeInsets.fromLTRB(20, 8, 20, 16),
                sliver: SliverToBoxAdapter(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        v?.title ?? 'Your saved recommendations',
                        style: LibraryStyle.heading(context, 30),
                      ),
                      const SizedBox(height: 8),
                      Text(
                        v == null
                            ? 'Your Library, open to possibilities'
                            : '${v.total} saved · Your Library',
                        style: TextStyle(color: colors.onSurfaceVariant),
                      ),
                      const SizedBox(height: 16),
                      if (v != null)
                        Wrap(
                          spacing: 8,
                          runSpacing: 4,
                          children: [
                            ActionChip(
                              label: Text(
                                spec.location.isEmpty
                                    ? 'All locations'
                                    : v.areaLabel.isEmpty
                                    ? spec.location
                                    : v.areaLabel,
                              ),
                              onPressed: loading
                                  ? null
                                  : () => pick(
                                      'Where?',
                                      v.areas,
                                      spec.location,
                                      (id) => load(spec.copyWith(location: id)),
                                    ),
                            ),
                            ActionChip(
                              label: Text(
                                spec.kind.isEmpty
                                    ? 'All collections'
                                    : kindLabel(spec.kind),
                              ),
                              onPressed: loading
                                  ? null
                                  : () => pick(
                                      'Collections',
                                      v.kinds,
                                      spec.kind,
                                      (id) => load(
                                        spec.copyWith(
                                          kind: id,
                                          categoryIds: [],
                                        ),
                                      ),
                                      kinds: true,
                                    ),
                            ),
                            ActionChip(
                              label: Text(
                                spec.categoryIds.isEmpty
                                    ? 'All types'
                                    : v.categories
                                              .where(
                                                (f) =>
                                                    f.id ==
                                                    spec.categoryIds.first,
                                              )
                                              .firstOrNull
                                              ?.label ??
                                          'Selected type',
                              ),
                              onPressed: loading
                                  ? null
                                  : () => pick(
                                      'Types',
                                      v.categories,
                                      spec.categoryIds.firstOrNull ?? '',
                                      (id) => load(
                                        spec.copyWith(
                                          categoryIds: id.isEmpty ? [] : [id],
                                        ),
                                      ),
                                    ),
                            ),
                            ActionChip(
                              label: Text(
                                spec.sort == 'name' ? 'A–Z' : 'Newest',
                              ),
                              onPressed: loading
                                  ? null
                                  : () => load(
                                      spec.copyWith(
                                        sort: spec.sort == 'name'
                                            ? 'saved_newest'
                                            : 'name',
                                      ),
                                    ),
                            ),
                          ],
                        ),
                      if (loading)
                        const Padding(
                          padding: EdgeInsets.only(top: 12),
                          child: LinearProgressIndicator(minHeight: 2),
                        ),
                      if (error != null)
                        Padding(
                          padding: const EdgeInsets.only(top: 12),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                error!,
                                style: TextStyle(color: colors.error),
                              ),
                              TextButton(
                                onPressed: () => load(spec, preserve: true),
                                child: const Text('Refresh and try again'),
                              ),
                            ],
                          ),
                        ),
                      if (v != null && v.total == 0 && !loading)
                        const Padding(
                          padding: EdgeInsets.only(top: 24),
                          child: Text(
                            'No saved recommendations match these filters. Try another area or type.',
                          ),
                        ),
                    ],
                  ),
                ),
              ),
              if (v != null)
                SliverPadding(
                  padding: const EdgeInsets.symmetric(horizontal: 20),
                  sliver: SliverList.builder(
                    itemCount: v.items.length,
                    itemBuilder: (context, index) {
                      final item = v.items[index];
                      final location = item
                          .recommendation
                          ?.locations
                          .firstOrNull
                          ?.displayText;
                      return Padding(
                        padding: const EdgeInsets.only(bottom: 10),
                        child: Material(
                          color: colors.surfaceContainerLow,
                          borderRadius: BorderRadius.circular(16),
                          clipBehavior: Clip.antiAlias,
                          child: ListTile(
                            contentPadding: const EdgeInsets.symmetric(
                              horizontal: 12,
                              vertical: 8,
                            ),
                            leading: Container(
                              width: 40,
                              height: 40,
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
                            title: Text(
                              item.subject,
                              style: const TextStyle(
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                            subtitle: Text(
                              [item.recommendation?.categoryLabel, location]
                                  .whereType<String>()
                                  .where((part) => part.isNotEmpty)
                                  .join(' · '),
                            ),
                            trailing: Checkbox(
                              value: selected.contains(item.id),
                              activeColor: RekkyTheme.navAsk,
                              checkColor: RekkyTheme.onNavAsk,
                              semanticLabel: 'Select ${item.subject}',
                              onChanged: loading
                                  ? null
                                  : (value) => setState(() {
                                      if (!selected.remove(item.id) &&
                                          selected.length < 4) {
                                        selected.add(item.id);
                                      }
                                    }),
                            ),
                            onTap: () async {
                              try {
                                final current = await widget.api.item(item.id);
                                if (!mounted) return;
                                await widget.onOpen(current);
                              } catch (e) {
                                if (mounted) {
                                  setState(
                                    () => error = 'This recommendation changed. Refresh the collection.',
                                  );
                                }
                              }
                            },
                          ),
                        ),
                      );
                    },
                  ),
                ),
              if (v?.nextOffset != null)
                SliverToBoxAdapter(
                  child: Padding(
                    padding: const EdgeInsets.all(20),
                    child: OutlinedButton(
                      onPressed: paging || loading ? null : more,
                      child: Text(paging ? 'Loading…' : 'Load more'),
                    ),
                  ),
                ),
              const SliverToBoxAdapter(child: SizedBox(height: 12)),
            ],
          ),
        ),
        if (v != null && v.total > 0)
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (selected.isNotEmpty)
                  Align(
                    alignment: Alignment.centerLeft,
                    child: TextButton(
                      onPressed: () => setState(selected.clear),
                      child: Text('${selected.length} selected · Clear'),
                    ),
                  ),
                TextField(
                  controller: question,
                  maxLines: 2,
                  minLines: 1,
                  maxLength: 500,
                  textInputAction: TextInputAction.send,
                  onSubmitted: (_) => ask(),
                  decoration: InputDecoration(
                    counterText: '',
                    hintText: selected.isEmpty
                        ? 'Ask about these…'
                        : 'Compare or ask about your selection…',
                    suffixIcon: IconButton(
                      tooltip: 'Ask about collection',
                      onPressed: loading ? null : ask,
                      icon: const Icon(Icons.arrow_upward_rounded),
                    ),
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }
}
