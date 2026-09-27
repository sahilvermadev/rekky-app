import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import 'rekky_haptics.dart';
import 'contact_matching.dart';
import 'library_collection.dart';
import 'library_style.dart';
import 'library_filters.dart';
import 'recommendation_maps_action.dart';
import 'recommendation_review.dart';
import 'rekky_api.dart';

Future<bool> _launchLibraryLink(Uri uri) =>
    launchUrl(uri, mode: LaunchMode.externalApplication);

class LibraryScreen extends StatefulWidget {
  const LibraryScreen({
    super.key,
    required this.items,
    required this.onOpen,
    required this.onRefresh,
    required this.onRemember,
    required this.onPin,
    this.processingMessage,
    this.headerAction,
    this.isActive = true,
    this.openUrl = _launchLibraryLink,
  });
  final List<RekkyItem> items;
  final ValueChanged<RekkyItem> onOpen;
  final Future<void> Function() onRefresh;
  final VoidCallback onRemember;
  final Future<RekkyItem> Function(RekkyItem, bool) onPin;
  final String? processingMessage;
  final Widget? headerAction;
  final bool isActive;
  final Future<bool> Function(Uri) openUrl;

  @override
  State<LibraryScreen> createState() => _LibraryScreenState();
}

class _LibraryScreenState extends State<LibraryScreen> {
  final search = TextEditingController();
  final scroll = ScrollController();
  LibraryOrder order = LibraryOrder.browse;
  LibraryShelf? shelf;
  String? typeId, areaId, areaLabel, neighbourhoodId, neighbourhoodLabel;
  final pendingPins = <String>{};
  double overviewOffset = 0;

  bool get atOverview =>
      order == LibraryOrder.browse &&
      shelf == null &&
      search.text.isEmpty &&
      areaId == null &&
      typeId == null;

  @override
  void dispose() {
    search.dispose();
    scroll.dispose();
    super.dispose();
  }

  void change(VoidCallback update) {
    setState(update);
    if (scroll.hasClients) scroll.jumpTo(0);
  }

  void openCollection(LibraryShelf value) {
    FocusScope.of(context).unfocus();
    if (scroll.hasClients) overviewOffset = scroll.offset;
    change(() {
      order = LibraryOrder.browse;
      shelf = value;
      typeId = null;
      areaId = null;
      areaLabel = null;
      neighbourhoodId = null;
      neighbourhoodLabel = null;
      search.clear();
    });
  }

  void openView(LibraryOrder value) {
    FocusScope.of(context).unfocus();
    if (scroll.hasClients) overviewOffset = scroll.offset;
    change(() {
      order = value;
      shelf = null;
      typeId = null;
      areaId = null;
      areaLabel = null;
      neighbourhoodId = null;
      neighbourhoodLabel = null;
      search.clear();
    });
  }

  void returnToOverview() {
    FocusScope.of(context).unfocus();
    setState(() {
      order = LibraryOrder.browse;
      shelf = null;
      typeId = null;
      areaId = null;
      areaLabel = null;
      neighbourhoodId = null;
      neighbourhoodLabel = null;
      search.clear();
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && scroll.hasClients) {
        scroll.jumpTo(overviewOffset.clamp(0, scroll.position.maxScrollExtent));
      }
    });
  }

  Future<void> chooseFilters() => showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    showDragHandle: true,
    builder: (_) => LibraryFilterSheet(
      items: librarySelection(widget.items, query: search.text, order: order),
      fixedShelf: shelf,
      initial: LibraryFilters(
        shelf: shelf,
        typeId: typeId,
        areaId: areaId,
        areaLabel: areaLabel,
        neighbourhoodId: neighbourhoodId,
        neighbourhoodLabel: neighbourhoodLabel,
      ),
      onChanged: (value) => change(() {
        if (atOverview && scroll.hasClients) {
          overviewOffset = scroll.offset;
        }
        shelf = value.shelf;
        typeId = value.typeId;
        areaId = value.areaId;
        areaLabel = value.areaLabel;
        neighbourhoodId = value.neighbourhoodId;
        neighbourhoodLabel = value.neighbourhoodLabel;
      }),
    ),
  );

  Future<void> pinMenu(RekkyItem item) async {
    RekkyHaptics.confirm();
    final pin = await showModalBottomSheet<bool>(
      context: context,
      showDragHandle: true,
      useSafeArea: true,
      builder: (context) => SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(8, 0, 8, 16),
          child: ListTile(
            leading: Icon(
              item.pinned ? Icons.push_pin : Icons.push_pin_outlined,
            ),
            title: Text(item.pinned ? 'Unpin from Library' : 'Pin in Library'),
            subtitle: Text(
              item.subject,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
            onTap: () => Navigator.pop(context, !item.pinned),
          ),
        ),
      ),
    );
    if (pin == null || !mounted || pendingPins.contains(item.id)) return;
    setState(() => pendingPins.add(item.id));
    try {
      await widget.onPin(item, pin);
      if (mounted) RekkyHaptics.selection();
    } catch (error) {
      if (mounted) {
        RekkyHaptics.warning();
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              error is ApiFailure
                  ? error.message
                  : 'Couldn’t save the pin. Try again.',
            ),
          ),
        );
      }
    } finally {
      if (mounted) setState(() => pendingPins.remove(item.id));
    }
  }

  void reset() => change(() {
    search.clear();
    typeId = null;
    areaId = null;
    areaLabel = null;
    neighbourhoodId = null;
    neighbourhoodLabel = null;
  });

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final selected = librarySelection(
      widget.items,
      query: search.text,
      shelf: shelf,
      typeId: typeId,
      areaId: areaId,
      neighbourhoodId: neighbourhoodId,
      order: order,
    );
    final populatedShelves = LibraryShelf.values
        .where((s) => widget.items.any((i) => LibraryShelf.of(i) == s))
        .toList();
    final constrained =
        search.text.isNotEmpty || areaId != null || typeId != null;
    final overview =
        order == LibraryOrder.browse && shelf == null && !constrained;
    final sparse =
        overview && widget.items.length <= 3 && populatedShelves.length == 1;
    final collectionIndex = overview && !sparse && widget.items.isNotEmpty;
    final collectionCount = shelf == null
        ? 0
        : widget.items.where((i) => LibraryShelf.of(i) == shelf).length;
    final pinnedCount = widget.items.where((i) => i.pinned).length;
    final pageTitle =
        shelf?.label ??
        switch (order) {
          LibraryOrder.browse => 'Your Library',
          LibraryOrder.recent => 'Recently added',
          LibraryOrder.pinned => 'Pinned',
        };
    final countLabel = shelf != null
        ? '$collectionCount saved'
        : order == LibraryOrder.pinned
        ? '$pinnedCount saved'
        : '${widget.items.length} saved';
    final groups = <String, List<RekkyItem>>{};
    if (collectionIndex) {
      groups['Recently added'] = selected.take(4).toList();
    } else if (order == LibraryOrder.recent) {
      for (final item in selected) {
        groups.putIfAbsent(libraryMonth(item), () => []).add(item);
      }
    } else if (sparse) {
      groups[populatedShelves.single.label] = selected;
    } else if (selected.isNotEmpty) {
      groups['Results'] = selected;
    }
    final filtered = constrained;
    final filterActive = areaId != null || typeId != null;
    final libraryAccent = LibraryStyle.libraryAccent(context);
    Widget filterChip(String label, VoidCallback onDeleted) => InputChip(
      label: Text(label),
      onPressed: chooseFilters,
      onDeleted: onDeleted,
      backgroundColor: LibraryStyle.libraryTint(context),
      side: BorderSide(color: libraryAccent.withValues(alpha: .55)),
      deleteIconColor: libraryAccent,
    );
    final shelfTypes = <String, (String, int)>{};
    if (shelf != null) {
      for (final item in widget.items.where(
        (i) => LibraryShelf.of(i) == shelf,
      )) {
        for (final type
            in item.recommendation?.classification?.types ??
                <CategoryConcept>[]) {
          final previous = shelfTypes[type.id];
          shelfTypes[type.id] = (type.label, (previous?.$2 ?? 0) + 1);
        }
      }
    }
    final leadingTypes = shelfTypes.entries.toList()
      ..sort((a, b) {
        final count = b.value.$2.compareTo(a.value.$2);
        return count != 0 ? count : a.value.$1.compareTo(b.value.$1);
      });
    if (typeId != null &&
        leadingTypes.length > 4 &&
        !leadingTypes.take(4).any((entry) => entry.key == typeId)) {
      final current = leadingTypes
          .where((entry) => entry.key == typeId)
          .firstOrNull;
      if (current != null) {
        leadingTypes.remove(current);
        leadingTypes.insert(0, current);
      }
    }
    return PopScope(
      canPop: !widget.isActive || overview,
      onPopInvokedWithResult: (didPop, result) {
        if (!didPop && widget.isActive && !atOverview) returnToOverview();
      },
      child: RefreshIndicator(
        onRefresh: widget.onRefresh,
        child: CustomScrollView(
          controller: scroll,
          keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
          physics: const AlwaysScrollableScrollPhysics(),
          slivers: [
            SliverPadding(
              padding: const EdgeInsets.fromLTRB(20, 12, 20, 0),
              sliver: SliverToBoxAdapter(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    if (shelf != null || order != LibraryOrder.browse)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 10),
                        child: TextButton.icon(
                          onPressed: returnToOverview,
                          icon: const Icon(Icons.arrow_back, size: 18),
                          label: const Text('Your Library'),
                          style: TextButton.styleFrom(
                            foregroundColor: libraryAccent,
                            padding: const EdgeInsets.symmetric(horizontal: 4),
                            minimumSize: const Size(48, 48),
                          ),
                        ),
                      ),
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.center,
                      children: [
                        Expanded(
                          child: LayoutBuilder(
                            builder: (context, constraints) {
                              final titleStyle = LibraryStyle.heading(
                                context,
                                30,
                              );
                              final countStyle = Theme.of(context)
                                  .textTheme
                                  .bodySmall!
                                  .copyWith(color: colors.onSurfaceVariant);
                              final label = countLabel;
                              double measure(String text, TextStyle style) {
                                final painter = TextPainter(
                                  text: TextSpan(text: text, style: style),
                                  textDirection: Directionality.of(context),
                                  textScaler: MediaQuery.textScalerOf(context),
                                )..layout();
                                final width = painter.width;
                                painter.dispose();
                                return width;
                              }

                              final fits =
                                  measure(pageTitle, titleStyle) +
                                      measure(label, countStyle) +
                                      12 <=
                                  constraints.maxWidth;
                              final title = Text(pageTitle, style: titleStyle);
                              final count = Text(label, style: countStyle);
                              return fits
                                  ? Row(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.baseline,
                                      textBaseline: TextBaseline.alphabetic,
                                      children: [
                                        title,
                                        const SizedBox(width: 12),
                                        count,
                                      ],
                                    )
                                  : Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        title,
                                        const SizedBox(height: 4),
                                        count,
                                      ],
                                    );
                            },
                          ),
                        ),
                        if (widget.headerAction != null) widget.headerAction!,
                      ],
                    ),
                    const SizedBox(height: 16),
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Expanded(
                          child: TextField(
                            controller: search,
                            onChanged: (_) => setState(() {}),
                            textInputAction: TextInputAction.search,
                            onSubmitted: (_) =>
                                FocusScope.of(context).unfocus(),
                            decoration: InputDecoration(
                              hintText: shelf == null
                                  ? 'Search your library'
                                  : 'Search ${shelf!.label}',
                              prefixIcon: const Icon(Icons.search, size: 22),
                              prefixIconColor: WidgetStateColor.resolveWith(
                                (states) => states.contains(WidgetState.focused)
                                    ? LibraryStyle.searchFocus(context)
                                    : colors.onSurfaceVariant,
                              ),
                              suffixIcon: search.text.isEmpty
                                  ? null
                                  : IconButton(
                                      tooltip: 'Clear search',
                                      icon: const Icon(Icons.close),
                                      onPressed: () => setState(search.clear),
                                    ),
                              filled: true,
                              fillColor: colors.surfaceContainerLow,
                              contentPadding: const EdgeInsets.symmetric(
                                horizontal: 16,
                                vertical: 12,
                              ),
                              border: OutlineInputBorder(
                                borderRadius: BorderRadius.circular(12),
                                borderSide: BorderSide.none,
                              ),
                              focusedBorder: OutlineInputBorder(
                                borderRadius: BorderRadius.circular(12),
                                borderSide: BorderSide(
                                  color: LibraryStyle.searchFocus(context),
                                  width: 1.5,
                                ),
                              ),
                            ),
                          ),
                        ),
                        const SizedBox(width: 8),
                        SizedBox(
                          width: 48,
                          height: 48,
                          child: IconButton.filledTonal(
                            tooltip: 'Filters',
                            onPressed: chooseFilters,
                            icon: Badge(
                              backgroundColor: libraryAccent,
                              isLabelVisible: filterActive,
                              child: const Icon(Icons.tune, size: 22),
                            ),
                            style: IconButton.styleFrom(
                              backgroundColor: filterActive
                                  ? LibraryStyle.libraryTint(context)
                                  : colors.surfaceContainerLow,
                              foregroundColor: filterActive
                                  ? libraryAccent
                                  : colors.onSurface,
                              shape: RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(12),
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                    if (overview && pinnedCount > 0) ...[
                      const SizedBox(height: 8),
                      TextButton.icon(
                        onPressed: () => openView(LibraryOrder.pinned),
                        icon: const Icon(Icons.push_pin_outlined, size: 18),
                        label: Text('Pinned · $pinnedCount'),
                        style: TextButton.styleFrom(
                          foregroundColor: libraryAccent,
                          padding: const EdgeInsets.symmetric(horizontal: 4),
                          minimumSize: const Size(48, 48),
                        ),
                      ),
                    ],
                    if (widget.items.isNotEmpty) ...[
                      if (filtered)
                        Wrap(
                          spacing: 8,
                          runSpacing: 4,
                          children: [
                            if (typeId != null && shelfTypes.length <= 1)
                              filterChip(
                                widget.items
                                        .expand(
                                          (i) =>
                                              i
                                                  .recommendation
                                                  ?.classification
                                                  ?.types ??
                                              <CategoryConcept>[],
                                        )
                                        .where((t) => t.id == typeId)
                                        .firstOrNull
                                        ?.label ??
                                    'Selected type',
                                () => change(() => typeId = null),
                              ),
                            if (areaId != null)
                              filterChip(
                                areaLabel ?? 'Selected location',
                                () => change(() {
                                  areaId = null;
                                  areaLabel = null;
                                  neighbourhoodId = null;
                                  neighbourhoodLabel = null;
                                }),
                              ),
                            if (neighbourhoodId != null)
                              filterChip(
                                neighbourhoodLabel ?? 'Selected neighbourhood',
                                () => change(() {
                                  neighbourhoodId = null;
                                  neighbourhoodLabel = null;
                                }),
                              ),
                          ],
                        ),
                    ],
                    if (widget.processingMessage case final message?)
                      Padding(
                        padding: const EdgeInsets.only(top: 12),
                        child: Semantics(
                          liveRegion: true,
                          child: Row(
                            children: [
                              const Icon(Icons.schedule, size: 18),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Text(
                                  message,
                                  style: Theme.of(context).textTheme.bodySmall,
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
            if (collectionIndex)
              SliverPadding(
                padding: const EdgeInsets.fromLTRB(20, 24, 20, 0),
                sliver: SliverToBoxAdapter(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Semantics(
                        header: true,
                        child: Text(
                          'Collections',
                          style: Theme.of(context).textTheme.titleSmall
                              ?.copyWith(fontWeight: FontWeight.w700),
                        ),
                      ),
                      const SizedBox(height: 12),
                      LayoutBuilder(
                        builder: (context, constraints) {
                          final columns =
                              constraints.maxWidth >= 300 &&
                                  MediaQuery.textScalerOf(context).scale(14) <=
                                      21
                              ? 2
                              : 1;
                          final width = columns == 2
                              ? (constraints.maxWidth - 10) / 2
                              : constraints.maxWidth;
                          return Wrap(
                            spacing: 10,
                            runSpacing: 10,
                            children: [
                              for (final destination in populatedShelves)
                                SizedBox(
                                  width: width,
                                  child: _CollectionTile(
                                    shelf: destination,
                                    count: widget.items
                                        .where(
                                          (i) =>
                                              LibraryShelf.of(i) == destination,
                                        )
                                        .length,
                                    onTap: () => openCollection(destination),
                                  ),
                                ),
                            ],
                          );
                        },
                      ),
                    ],
                  ),
                ),
              ),
            if (shelf != null && shelfTypes.length > 1)
              SliverPadding(
                padding: const EdgeInsets.fromLTRB(20, 16, 20, 0),
                sliver: SliverToBoxAdapter(
                  child: Wrap(
                    spacing: 8,
                    runSpacing: 4,
                    children: [
                      ChoiceChip(
                        label: const Text('All types'),
                        selected: typeId == null,
                        selectedColor: LibraryStyle.libraryTint(context),
                        onSelected: (_) => change(() => typeId = null),
                      ),
                      for (final entry in leadingTypes.take(4))
                        ChoiceChip(
                          label: Text(entry.value.$1),
                          selected: typeId == entry.key,
                          selectedColor: LibraryStyle.libraryTint(context),
                          onSelected: (_) => change(() => typeId = entry.key),
                        ),
                      if (shelfTypes.length > 4)
                        TextButton(
                          onPressed: chooseFilters,
                          child: const Text('More types'),
                        ),
                    ],
                  ),
                ),
              ),
            if (selected.isEmpty)
              SliverToBoxAdapter(
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(28, 48, 28, 40),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Icon(
                        widget.items.isEmpty
                            ? Icons.bookmarks_outlined
                            : order == LibraryOrder.pinned && !filtered
                            ? Icons.push_pin_outlined
                            : Icons.search_off,
                        size: 32,
                        color: colors.primary,
                      ),
                      const SizedBox(height: 20),
                      Text(
                        widget.items.isEmpty
                            ? 'Good things start here.'
                            : order == LibraryOrder.pinned && !filtered
                            ? 'Keep your go-to things close.'
                            : 'Nothing here just yet.',
                        style: LibraryStyle.heading(context, 28),
                      ),
                      const SizedBox(height: 12),
                      Text(
                        widget.items.isEmpty
                            ? 'A place you loved. Someone you trust. Something worth passing on.'
                            : order == LibraryOrder.pinned && !filtered
                            ? 'Touch and hold a recommendation to pin it. You can also pin it from its menu.'
                            : 'Try another search or clear the filters.',
                        style: TextStyle(
                          color: colors.onSurfaceVariant,
                          height: 1.5,
                        ),
                      ),
                      const SizedBox(height: 20),
                      if (widget.items.isEmpty)
                        FilledButton.icon(
                          onPressed: widget.onRemember,
                          icon: const Icon(Icons.mic),
                          label: const Text('Recommend'),
                        )
                      else if (filtered)
                        TextButton(
                          onPressed: reset,
                          child: const Text('Clear filters'),
                        ),
                    ],
                  ),
                ),
              ),
            for (final group in groups.entries) ...[
              if (collectionIndex ||
                  sparse ||
                  order == LibraryOrder.recent ||
                  (shelf == null && constrained))
                SliverPadding(
                  padding: const EdgeInsets.fromLTRB(20, 20, 20, 12),
                  sliver: SliverToBoxAdapter(
                    child: Row(
                      children: [
                        Container(
                          width: 16,
                          height: 6,
                          margin: const EdgeInsets.only(right: 8),
                          decoration: BoxDecoration(
                            color: sparse
                                ? LibraryStyle.accent(
                                    context,
                                    LibraryShelf.of(group.value.first),
                                  )
                                : libraryAccent,
                            borderRadius: BorderRadius.circular(3),
                          ),
                        ),
                        Expanded(
                          child: Semantics(
                            header: true,
                            child: Text(
                              order == LibraryOrder.recent
                                  ? (group.key == 'Saved earlier'
                                        ? group.key
                                        : 'Saved in ${group.key}')
                                  : group.key,
                              style: Theme.of(context).textTheme.titleSmall
                                  ?.copyWith(fontWeight: FontWeight.w700),
                            ),
                          ),
                        ),
                        if (!collectionIndex)
                          Text(
                            '${group.value.length}',
                            style: TextStyle(
                              fontSize: 13,
                              color: colors.onSurfaceVariant,
                            ),
                          ),
                        if (collectionIndex && selected.length > 4)
                          TextButton(
                            onPressed: () => openView(LibraryOrder.recent),
                            child: const Text('See all'),
                          ),
                      ],
                    ),
                  ),
                ),
              SliverPadding(
                padding: EdgeInsets.fromLTRB(
                  20,
                  group.key == 'Results' && (shelf != null || !constrained)
                      ? 16
                      : 0,
                  20,
                  0,
                ),
                sliver: SliverList.builder(
                  itemCount: group.value.length,
                  itemBuilder: (context, index) {
                    final item = group.value[index];
                    return _LibraryRow(
                      key: ValueKey(item.id),
                      item: item,
                      bottomGap: index == group.value.length - 1 ? 0 : 8,
                      onOpen: () => widget.onOpen(item),
                      onPin: () => pinMenu(item),
                      pinBusy: pendingPins.contains(item.id),
                      openUrl: widget.openUrl,
                    );
                  },
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _CollectionTile extends StatelessWidget {
  const _CollectionTile({
    required this.shelf,
    required this.count,
    required this.onTap,
  });

  final LibraryShelf shelf;
  final int count;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final accent = LibraryStyle.accent(context, shelf);
    return Material(
      color: colors.surfaceContainerLow,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: colors.outlineVariant.withValues(alpha: .35)),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: ConstrainedBox(
          constraints: const BoxConstraints(minHeight: 108),
          child: Padding(
            padding: const EdgeInsets.all(14),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Row(
                  children: [
                    Container(
                      width: 34,
                      height: 34,
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                        color: accent,
                        borderRadius: BorderRadius.circular(9),
                      ),
                      child: Icon(
                        LibraryStyle.icon(shelf),
                        size: 19,
                        color: LibraryStyle.itemForeground(context),
                      ),
                    ),
                    const Spacer(),
                    Icon(
                      Icons.arrow_outward,
                      size: 17,
                      color: colors.onSurfaceVariant,
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                Text(
                  shelf.label,
                  style: Theme.of(context).textTheme.titleSmall
                      ?.copyWith(fontWeight: FontWeight.w700),
                ),
                const SizedBox(height: 2),
                Text(
                  '$count saved',
                  style: Theme.of(context).textTheme.bodySmall
                      ?.copyWith(color: colors.onSurfaceVariant),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _LibraryRow extends StatefulWidget {
  const _LibraryRow({
    super.key,
    required this.item,
    required this.bottomGap,
    required this.onOpen,
    required this.onPin,
    required this.pinBusy,
    required this.openUrl,
  });
  final RekkyItem item;
  final double bottomGap;
  final VoidCallback onOpen, onPin;
  final bool pinBusy;
  final Future<bool> Function(Uri) openUrl;
  @override
  State<_LibraryRow> createState() => _LibraryRowState();
}

class _LibraryRowState extends State<_LibraryRow> {
  bool opening = false;
  Future<void> open(Uri uri) async {
    if (opening) return;
    setState(() => opening = true);
    var success = false;
    try {
      success = await widget.openUrl(uri).timeout(const Duration(seconds: 10));
    } catch (_) {}
    if (!mounted) return;
    setState(() => opening = false);
    if (!success) {
      RekkyHaptics.warning();
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Couldn’t open this action. Try again.')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final item = widget.item;
    final rec = item.recommendation;
    final shelf = LibraryShelf.of(item);
    final colors = Theme.of(context).colorScheme;
    final phone = rec?.contactPhone == null
        ? null
        : internationalPhone(rec!.contactPhone!);
    final destination = recommendationDestination(item);
    final action = phone != null
        ? Uri(scheme: 'tel', path: phone)
        : destination;
    final large = MediaQuery.textScalerOf(context).scale(14) > 20;
    final rating = rec?.rating;
    final metadata = [
      if (rec?.categoryLabel != null && rec!.categoryLabel != shelf.label)
        rec.categoryLabel,
      if (rec?.primaryLocation != null) rec!.primaryLocation!,
    ].join(' · ');
    return Padding(
      padding: EdgeInsets.only(bottom: widget.bottomGap),
      child: Material(
        color: colors.surfaceContainerLow,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(16),
          side: BorderSide(color: colors.outlineVariant.withValues(alpha: .35)),
        ),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: widget.onOpen,
          onLongPress: widget.pinBusy ? null : widget.onPin,
          enableFeedback: false,
          borderRadius: BorderRadius.circular(12),
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (!large) ...[
                  Container(
                    width: 36,
                    height: 40,
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      color: LibraryStyle.itemFill(context, item),
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Icon(
                      LibraryStyle.itemIcon(item),
                      size: 22,
                      color: LibraryStyle.itemForeground(context),
                    ),
                  ),
                  const SizedBox(width: 12),
                ],
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        item.subject,
                        maxLines: large ? null : 2,
                        overflow: large ? null : TextOverflow.ellipsis,
                        style: Theme.of(context).textTheme.titleMedium
                            ?.copyWith(
                              fontWeight: FontWeight.w600,
                              height: 1.25,
                            ),
                      ),
                      if (metadata.isNotEmpty) ...[
                        const SizedBox(height: 4),
                        Text(
                          metadata,
                          maxLines: large ? null : 2,
                          overflow: large ? null : TextOverflow.ellipsis,
                          style: Theme.of(context).textTheme.bodySmall
                              ?.copyWith(
                                fontSize: 13,
                                color: colors.onSurfaceVariant,
                                height: 1.4,
                              ),
                        ),
                      ],
                      const SizedBox(height: 4),
                      Wrap(
                        spacing: 8,
                        runSpacing: 0,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: [
                          if (rating != null)
                            Tooltip(
                              message: rating.estimated
                                  ? 'Estimated from your note'
                                  : 'Your rating',
                              child: Semantics(
                                label:
                                    '${rating.label} out of 10${rating.estimated ? ', estimated from your note' : ''}',
                                child: ExcludeSemantics(
                                  child: Row(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      Icon(
                                        Icons.star,
                                        size: 15,
                                        color: LibraryStyle.libraryAccent(
                                          context,
                                        ),
                                      ),
                                      const SizedBox(width: 4),
                                      Text(
                                        '${rating.label}/10',
                                        style: Theme.of(context)
                                            .textTheme
                                            .labelMedium
                                            ?.copyWith(
                                              color: colors.onSurface,
                                              fontWeight: FontWeight.w600,
                                            ),
                                      ),
                                    ],
                                  ),
                                ),
                              ),
                            ),
                          Icon(
                            item.visibility == 'private'
                                ? Icons.lock_outline
                                : Icons.people_outline,
                            size: 14,
                            color: colors.onSurfaceVariant,
                            semanticLabel: item.visibility == 'private'
                                ? 'Only me'
                                : 'Friends',
                          ),
                          if (item.pinned)
                            Icon(
                              Icons.push_pin,
                              size: 14,
                              color: LibraryStyle.libraryAccent(context),
                              semanticLabel: 'Pinned',
                            ),
                          if (RecommendationReviewButton.needed(item))
                            RecommendationReviewButton(item: item),
                          if (rec?.experience == 'interest')
                            Text(
                              'Not tried yet',
                              style: Theme.of(context).textTheme.labelSmall,
                            ),
                          if (rec?.experience == 'secondhand')
                            Text(
                              'Heard from others',
                              style: Theme.of(context).textTheme.labelSmall,
                            ),
                        ],
                      ),
                    ],
                  ),
                ),
                Column(
                  children: [
                    if (action != null)
                      IconButton(
                        tooltip: phone != null
                            ? 'Call ${item.subject}'
                            : rec?.destinationMode == 'custom'
                            ? 'Open link for ${item.subject}'
                            : 'Search Maps for ${item.subject}',
                        onPressed: opening ? null : () => open(action),
                        icon: opening
                            ? const SizedBox.square(
                                dimension: 18,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : Icon(
                                phone != null
                                    ? Icons.call_outlined
                                    : rec?.destinationMode == 'custom'
                                    ? Icons.open_in_new
                                    : Icons.map_outlined,
                                size: 20,
                              ),
                      ),
                    if (widget.pinBusy)
                      const SizedBox.square(
                        dimension: 18,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
