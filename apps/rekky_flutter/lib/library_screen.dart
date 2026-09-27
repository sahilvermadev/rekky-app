import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import 'contact_matching.dart';
import 'library_collection.dart';
import 'library_style.dart';
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
    this.openUrl = _launchLibraryLink,
  });
  final List<RekkyItem> items;
  final ValueChanged<RekkyItem> onOpen;
  final Future<void> Function() onRefresh;
  final VoidCallback onRemember;
  final Future<RekkyItem> Function(RekkyItem, bool) onPin;
  final String? processingMessage;
  final Future<bool> Function(Uri) openUrl;

  @override
  State<LibraryScreen> createState() => _LibraryScreenState();
}

class _LibraryScreenState extends State<LibraryScreen> {
  final search = TextEditingController();
  final scroll = ScrollController();
  LibraryOrder order = LibraryOrder.browse;
  LibraryShelf? shelf;
  String? typeId, areaId, areaLabel;
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

  Future<void> chooseShelf() async {
    final available = LibraryShelf.values.where(
      (s) => widget.items.any((i) => LibraryShelf.of(i) == s),
    );
    final choice = await _choose(context, 'Collections', [
      const LibraryArea('all', 'All collections'),
      ...available.map((s) => LibraryArea(s.name, s.label)),
    ], shelf?.name ?? 'all');
    if (!mounted || choice == null) return;
    change(() {
      shelf = LibraryShelf.values.where((s) => s.name == choice).firstOrNull;
      typeId = null;
    });
  }

  Future<void> chooseArea() async {
    final areas = [
      const LibraryArea('all', 'All locations'),
      ...libraryAreas(widget.items),
      const LibraryArea('unresolved', 'No confirmed location'),
    ];
    final choice = await _choose(
      context,
      'Saved locations',
      areas,
      areaId ?? 'all',
      description: 'Places, practice locations and stated service areas.',
    );
    if (!mounted || choice == null) return;
    change(() {
      areaId = choice == 'all' ? null : choice;
      areaLabel = areas.firstWhere((a) => a.id == choice).label;
    });
  }

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
    order = LibraryOrder.browse;
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
    final types = <String, String>{};
    if (shelf != null) {
      for (final item in widget.items.where(
        (i) => LibraryShelf.of(i) == shelf,
      )) {
        for (final type
            in item.recommendation?.classification?.types ??
                <CategoryConcept>[]) {
          types[type.id] = type.label;
        }
      }
    }
    final filtered =
        shelf != null ||
        areaId != null ||
        search.text.isNotEmpty ||
        typeId != null;
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
                  Text(
                    'Your Library',
                    style: LibraryStyle.heading(context, 36),
                  ),
                  const SizedBox(height: 6),
                  Text(
                    '${widget.items.length} saved',
                    style: TextStyle(color: colors.onSurfaceVariant),
                  ),
                  const SizedBox(height: 20),
                  TextField(
                    controller: search,
                    onChanged: (_) => setState(() {}),
                    textInputAction: TextInputAction.search,
                    onSubmitted: (_) => FocusScope.of(context).unfocus(),
                    decoration: InputDecoration(
                      hintText: 'Search your library',
                      prefixIcon: const Icon(Icons.search, size: 22),
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
                        vertical: 14,
                      ),
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(
                          LibraryStyle.radius,
                        ),
                        borderSide: BorderSide.none,
                      ),
                    ),
                  ),
                  if (widget.items.isNotEmpty) ...[
                    const SizedBox(height: 8),
                    Wrap(
                      spacing: 8,
                      children: [
                        _Filter(
                          label: shelf?.label ?? 'All collections',
                          icon: Icons.grid_view_outlined,
                          active: shelf != null,
                          onTap: chooseShelf,
                        ),
                        _Filter(
                          label: areaId == null
                              ? 'All locations'
                              : areaLabel ?? 'Saved location',
                          icon: Icons.location_on_outlined,
                          active: areaId != null,
                          onTap: chooseArea,
                        ),
                      ],
                    ),
                    const SizedBox(height: 8),
                    Wrap(
                      spacing: 8,
                      children: [
                        for (final mode in LibraryOrder.values)
                          Semantics(
                            selected: order == mode,
                            child: TextButton(
                              onPressed: () => change(() => order = mode),
                              style: TextButton.styleFrom(
                                foregroundColor: order == mode
                                    ? colors.onSurface
                                    : colors.onSurfaceVariant,
                                backgroundColor: order == mode
                                    ? colors.secondaryContainer
                                    : null,
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 16,
                                ),
                              ),
                              child: Text(switch (mode) {
                                LibraryOrder.browse => 'Browse',
                                LibraryOrder.recent => 'Recent',
                                LibraryOrder.pinned => 'Pins',
                              }, softWrap: false),
                            ),
                          ),
                      ],
                    ),
                    if (shelf != null && types.length > 1) ...[
                      const SizedBox(height: 8),
                      Wrap(
                        spacing: 8,
                        runSpacing: 4,
                        children: [
                          ChoiceChip(
                            label: const Text('All types'),
                            selected: typeId == null,
                            onSelected: (_) => change(() => typeId = null),
                          ),
                          for (final type in types.entries)
                            ChoiceChip(
                              label: Text(type.value),
                              selected: typeId == type.key,
                              onSelected: (_) =>
                                  change(() => typeId = type.key),
                            ),
                        ],
                      ),
                    ],
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
                        label: const Text('Remember'),
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
              padding: const EdgeInsets.fromLTRB(20, 24, 12, 4),
              sliver: SliverToBoxAdapter(
                child: Row(
                  children: [
                    Expanded(
                      child: Semantics(
                        header: true,
                        child: Text(
                          order == LibraryOrder.recent
                              ? (group.key == 'Saved earlier'
                                    ? group.key
                                    : 'Saved in ${group.key}')
                              : group.key,
                          style: LibraryStyle.heading(context, 25),
                        ),
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
                itemCount:
                    order == LibraryOrder.browse &&
                        shelf == null &&
                        selected.length > 12 &&
                        search.text.isEmpty &&
                        areaId == null
                    ? group.value.length.clamp(0, 4)
                    : group.value.length,
                itemBuilder: (context, index) {
                  final item = group.value[index];
                  return _LibraryRow(
                    key: ValueKey(item.id),
                    item: item,
                    onOpen: () => widget.onOpen(item),
                    onPin: () => pinMenu(item),
                    pinBusy: pendingPins.contains(item.id),
                    openUrl: widget.openUrl,
                  );
                },
              ),
            ),
          ],
          const SliverToBoxAdapter(child: SizedBox(height: 32)),
        ],
      ),
    );
  }
}

class _Filter extends StatelessWidget {
  const _Filter({
    required this.label,
    required this.icon,
    required this.active,
    required this.onTap,
  });
  final String label;
  final IconData icon;
  final bool active;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => TextButton.icon(
    onPressed: onTap,
    icon: Icon(icon, size: 16),
    label: Text(label, maxLines: 1, overflow: TextOverflow.ellipsis),
    style: TextButton.styleFrom(
      foregroundColor: active
          ? Theme.of(context).colorScheme.primary
          : Theme.of(context).colorScheme.onSurfaceVariant,
      padding: const EdgeInsets.symmetric(horizontal: 8),
    ),
  );
}

Future<String?> _choose(
  BuildContext context,
  String title,
  List<LibraryArea> options,
  String selected, {
  String? description,
}) => showModalBottomSheet<String>(
  context: context,
  isScrollControlled: true,
  useSafeArea: true,
  showDragHandle: true,
  builder: (_) => _Choices(
    title: title,
    options: options,
    selected: selected,
    description: description,
  ),
);

class _Choices extends StatefulWidget {
  const _Choices({
    required this.title,
    required this.options,
    required this.selected,
    this.description,
  });
  final String title, selected;
  final String? description;
  final List<LibraryArea> options;
  @override
  State<_Choices> createState() => _ChoicesState();
}

class _ChoicesState extends State<_Choices> {
  String query = '';
  @override
  Widget build(BuildContext context) {
    final options = widget.options
        .where((o) => o.label.toLowerCase().contains(query.toLowerCase()))
        .toList();
    return Padding(
      padding: EdgeInsets.only(bottom: MediaQuery.viewInsetsOf(context).bottom),
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * .65,
        child: CustomScrollView(
          slivers: [
            SliverToBoxAdapter(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 24),
                    child: Row(
                      children: [
                        Expanded(
                          child: Text(
                            widget.title,
                            style: LibraryStyle.heading(context, 28),
                          ),
                        ),
                        IconButton(
                          tooltip: 'Close',
                          onPressed: () => Navigator.pop(context),
                          icon: const Icon(Icons.close),
                        ),
                      ],
                    ),
                  ),
                  if (widget.description != null)
                    Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 24,
                        vertical: 8,
                      ),
                      child: Text(widget.description!),
                    ),
                  if (widget.options.length > 8)
                    Padding(
                      padding: const EdgeInsets.all(16),
                      child: TextField(
                        onChanged: (value) => setState(() => query = value),
                        decoration: const InputDecoration(
                          hintText: 'Find a location',
                          prefixIcon: Icon(Icons.search),
                        ),
                      ),
                    ),
                ],
              ),
            ),
            SliverList.builder(
              itemCount: options.length,
              itemBuilder: (context, index) {
                final option = options[index];
                return ListTile(
                  title: Text(option.label),
                  selected: option.id == widget.selected,
                  trailing: option.id == widget.selected
                      ? const Icon(Icons.check)
                      : null,
                  onTap: () => Navigator.pop(context, option.id),
                );
              },
            ),
            if (options.isEmpty)
              const SliverToBoxAdapter(
                child: Padding(
                  padding: EdgeInsets.all(24),
                  child: Text('No matching locations.'),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _LibraryRow extends StatefulWidget {
  const _LibraryRow({
    super.key,
    required this.item,
    required this.onOpen,
    required this.onPin,
    required this.pinBusy,
    required this.openUrl,
  });
  final RekkyItem item;
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
      rec?.categoryLabel ?? 'Saved note',
      if (rec?.primaryLocation != null) rec!.primaryLocation!,
    ].join(' · ');
    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border(
          bottom: BorderSide(
            color: colors.outlineVariant.withValues(alpha: .45),
          ),
        ),
      ),
      child: InkWell(
        onTap: widget.onOpen,
        onLongPress: widget.pinBusy ? null : widget.onPin,
        borderRadius: BorderRadius.circular(12),
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 14),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (!large) ...[
                Container(
                  width: 44,
                  height: 48,
                  alignment: Alignment.center,
                  decoration: BoxDecoration(
                    color: LibraryStyle.tint(context, shelf),
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Icon(
                    LibraryStyle.icon(shelf),
                    size: 22,
                    color: colors.onSurfaceVariant,
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
                          ?.copyWith(fontWeight: FontWeight.w600, height: 1.25),
                    ),
                    const SizedBox(height: 4),
                    Text(
                      metadata,
                      maxLines: large ? null : 2,
                      overflow: large ? null : TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: colors.onSurfaceVariant,
                        height: 1.4,
                      ),
                    ),
                    const SizedBox(height: 6),
                    Wrap(
                      spacing: 10,
                      runSpacing: 4,
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
                                      size: 13,
                                      color: colors.primary,
                                    ),
                                    const SizedBox(width: 4),
                                    Text(
                                      '${rating.label}/10',
                                      style: Theme.of(context)
                                          .textTheme
                                          .labelSmall
                                          ?.copyWith(color: colors.primary),
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
                            color: colors.primary,
                            semanticLabel: 'Pinned',
                          ),
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
                  if (RecommendationReviewButton.needed(item))
                    RecommendationReviewButton(item: item),
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
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : Icon(
                              phone != null
                                  ? Icons.call_outlined
                                  : Icons.north_east,
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
    );
  }
}
