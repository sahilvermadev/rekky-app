import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

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
    this.openUrl = _launchLibraryLink,
  });
  final List<RekkyItem> items;
  final ValueChanged<RekkyItem> onOpen;
  final Future<void> Function() onRefresh;
  final VoidCallback onRemember;
  final Future<RekkyItem> Function(RekkyItem, bool) onPin;
  final String? processingMessage;
  final Widget? headerAction;
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

  Future<void> chooseFilters() => showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    showDragHandle: true,
    builder: (_) => LibraryFilterSheet(
      items: librarySelection(widget.items, query: search.text, order: order),
      initial: LibraryFilters(
        shelf: shelf,
        typeId: typeId,
        areaId: areaId,
        areaLabel: areaLabel,
        neighbourhoodId: neighbourhoodId,
        neighbourhoodLabel: neighbourhoodLabel,
      ),
      onChanged: (value) => change(() {
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
    } catch (error) {
      if (mounted) {
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
    shelf = null;
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
    final groups = <String, List<RekkyItem>>{};
    if (order == LibraryOrder.recent) {
      for (final item in selected) {
        groups.putIfAbsent(libraryMonth(item), () => []).add(item);
      }
    } else {
      for (final s in LibraryShelf.values) {
        final items = selected.where((i) => LibraryShelf.of(i) == s).toList();
        if (items.isNotEmpty) groups[s.label] = items;
      }
    }
    final filtered =
        shelf != null ||
        areaId != null ||
        search.text.isNotEmpty ||
        typeId != null;
    final filterActive = shelf != null || areaId != null || typeId != null;
    final libraryAccent = LibraryStyle.libraryAccent(context);
    Widget filterChip(String label, VoidCallback onDeleted) => InputChip(
      label: Text(label),
      onPressed: chooseFilters,
      onDeleted: onDeleted,
      backgroundColor: LibraryStyle.libraryTint(context),
      side: BorderSide(color: libraryAccent.withValues(alpha: .55)),
      deleteIconColor: libraryAccent,
    );
    int visibleCount(List<RekkyItem> items) =>
        order == LibraryOrder.browse &&
            shelf == null &&
            selected.length > 12 &&
            search.text.isEmpty &&
            areaId == null
        ? items.length.clamp(0, 4)
        : items.length;
    return RefreshIndicator(
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
                            final label = '${widget.items.length} saved';
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
                                measure('Your Library', titleStyle) +
                                    measure(label, countStyle) +
                                    12 <=
                                constraints.maxWidth;
                            final title = Text(
                              'Your Library',
                              style: titleStyle,
                            );
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
                          onSubmitted: (_) => FocusScope.of(context).unfocus(),
                          decoration: InputDecoration(
                            hintText: 'Search your library',
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
                  if (widget.items.isNotEmpty) ...[
                    const SizedBox(height: 8),
                    Wrap(
                      spacing: 20,
                      children: [
                        for (final mode in LibraryOrder.values)
                          Semantics(
                            selected: order == mode,
                            child: TextButton(
                              onPressed: () => change(() => order = mode),
                              style: TextButton.styleFrom(
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 4,
                                ),
                                minimumSize: const Size(48, 48),
                                foregroundColor: order == mode
                                    ? libraryAccent
                                    : colors.onSurfaceVariant,
                                shape: const RoundedRectangleBorder(),
                              ),
                              child: Container(
                                padding: const EdgeInsets.symmetric(
                                  vertical: 10,
                                ),
                                decoration: BoxDecoration(
                                  border: Border(
                                    bottom: BorderSide(
                                      color: order == mode
                                          ? libraryAccent
                                          : Colors.transparent,
                                      width: 2,
                                    ),
                                  ),
                                ),
                                child: Text(
                                  switch (mode) {
                                    LibraryOrder.browse => 'All',
                                    LibraryOrder.recent => 'Recent',
                                    LibraryOrder.pinned => 'Pinned',
                                  },
                                  style: TextStyle(
                                    fontWeight: order == mode
                                        ? FontWeight.w700
                                        : FontWeight.w500,
                                  ),
                                ),
                              ),
                            ),
                          ),
                      ],
                    ),
                    if (filtered)
                      Wrap(
                        spacing: 8,
                        runSpacing: 4,
                        children: [
                          if (shelf != null)
                            filterChip(
                              shelf!.label,
                              () => change(() {
                                shelf = null;
                                typeId = null;
                              }),
                            ),
                          if (typeId != null)
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
                        color: LibraryStyle.accent(
                          context,
                          LibraryShelf.of(group.value.first),
                        ),
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
                    Text(
                      '${group.value.length}',
                      style: TextStyle(
                        fontSize: 13,
                        color: colors.onSurfaceVariant,
                      ),
                    ),
                    if (order == LibraryOrder.browse &&
                        shelf == null &&
                        group.value.length > 4 &&
                        selected.length > 12)
                      TextButton(
                        onPressed: () => change(() {
                          shelf = LibraryShelf.of(group.value.first);
                          typeId = null;
                        }),
                        child: Text(
                          'See all (${group.value.length})',
                          softWrap: false,
                        ),
                      ),
                  ],
                ),
              ),
            ),
            SliverPadding(
              padding: const EdgeInsets.symmetric(horizontal: 20),
              sliver: SliverList.builder(
                itemCount: visibleCount(group.value),
                itemBuilder: (context, index) {
                  final item = group.value[index];
                  return _LibraryRow(
                    key: ValueKey(item.id),
                    item: item,
                    bottomGap: index == visibleCount(group.value) - 1 ? 0 : 8,
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
