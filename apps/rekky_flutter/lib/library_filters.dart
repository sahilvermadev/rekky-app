import 'package:flutter/material.dart';

import 'library_collection.dart';
import 'library_style.dart';
import 'rekky_api.dart';

class LibraryFilters {
  const LibraryFilters({
    this.shelf,
    this.typeId,
    this.areaId,
    this.areaLabel,
    this.neighbourhoodId,
    this.neighbourhoodLabel,
  });
  final LibraryShelf? shelf;
  final String? typeId, areaId, areaLabel, neighbourhoodId, neighbourhoodLabel;
}

/// Changes apply immediately. The sheet only edits the collection's view.
class LibraryFilterSheet extends StatefulWidget {
  const LibraryFilterSheet({
    super.key,
    required this.items,
    required this.initial,
    required this.onChanged,
  });
  final List<RekkyItem> items;
  final LibraryFilters initial;
  final ValueChanged<LibraryFilters> onChanged;
  @override
  State<LibraryFilterSheet> createState() => _LibraryFilterSheetState();
}

class _LibraryFilterSheetState extends State<LibraryFilterSheet> {
  late LibraryFilters selection = widget.initial;
  String query = '';
  bool regions = false;
  void update(LibraryFilters value) {
    setState(() => selection = value);
    widget.onChanged(value);
  }

  @override
  void initState() {
    super.initState();
    regions =
        selection.areaId != null &&
        libraryAreas(
          widget.items,
          regions: true,
        ).any((a) => a.id == selection.areaId) &&
        !libraryAreas(widget.items).any((a) => a.id == selection.areaId);
  }

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final scope = librarySelection(
      widget.items,
      shelf: selection.shelf,
      typeId: selection.typeId,
    );
    final types = <String, String>{};
    if (selection.shelf != null) {
      for (final item in widget.items.where(
        (i) => LibraryShelf.of(i) == selection.shelf,
      )) {
        for (final t
            in item.recommendation?.classification?.types ??
                <CategoryConcept>[]) {
          types[t.id] = t.label;
        }
      }
    }
    final areas = [
      LibraryArea('all', 'All locations', count: scope.length),
      ...libraryAreas(scope, regions: regions),
      if (!regions && scope.any((i) => libraryInArea(i, 'unresolved')))
        LibraryArea(
          'unresolved',
          'No confirmed location',
          count: scope.where((i) => libraryInArea(i, 'unresolved')).length,
        ),
    ].where((a) => a.matches(query)).toList();
    final neighbourhoods = selection.areaId == null
        ? <LibraryArea>[]
        : libraryAreas(scope, cityId: selection.areaId);
    Widget heading(String text) => Padding(
      padding: const EdgeInsets.only(top: 20, bottom: 8),
      child: Text(
        text,
        style: Theme.of(context).textTheme.titleSmall
            ?.copyWith(fontWeight: FontWeight.w700),
      ),
    );
    final libraryAccent = LibraryStyle.libraryAccent(context);
    return Padding(
      padding: EdgeInsets.only(bottom: MediaQuery.viewInsetsOf(context).bottom),
      child: SizedBox(
        height: MediaQuery.sizeOf(context).height * .82,
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 0, 8, 4),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      'Filters',
                      style: LibraryStyle.heading(context, 28),
                    ),
                  ),
                  if (MediaQuery.textScalerOf(context).scale(14) <= 20)
                    TextButton(
                      onPressed: () => update(const LibraryFilters()),
                      style: TextButton.styleFrom(
                        foregroundColor: libraryAccent,
                      ),
                      child: const Text('Clear all'),
                    )
                  else
                    IconButton(
                      tooltip: 'Clear all filters',
                      onPressed: () => update(const LibraryFilters()),
                      icon: const Icon(Icons.filter_alt_off_outlined),
                    ),
                  IconButton(
                    tooltip: 'Close filters',
                    onPressed: () => Navigator.pop(context),
                    icon: const Icon(Icons.close),
                  ),
                ],
              ),
            ),
            Expanded(
              child: ListView(
                padding: const EdgeInsets.fromLTRB(20, 0, 20, 24),
                keyboardDismissBehavior:
                    ScrollViewKeyboardDismissBehavior.onDrag,
                children: [
                  heading('Collections'),
                  Wrap(
                    spacing: 8,
                    runSpacing: 4,
                    children: [
                      ChoiceChip(
                        label: const Text('All collections'),
                        selected: selection.shelf == null,
                        selectedColor: LibraryStyle.libraryTint(context),
                        onSelected: (_) => update(
                          LibraryFilters(
                            areaId: selection.areaId,
                            areaLabel: selection.areaLabel,
                            neighbourhoodId: selection.neighbourhoodId,
                            neighbourhoodLabel: selection.neighbourhoodLabel,
                          ),
                        ),
                      ),
                      for (final shelf in LibraryShelf.values.where(
                        (s) => widget.items.any((i) => LibraryShelf.of(i) == s),
                      ))
                        ChoiceChip(
                          avatar: Icon(
                            LibraryStyle.icon(shelf),
                            size: 18,
                            color: LibraryStyle.accent(context, shelf),
                          ),
                          label: Text(shelf.label),
                          selected: selection.shelf == shelf,
                          selectedColor: LibraryStyle.libraryTint(context),
                          onSelected: (_) => update(
                            LibraryFilters(
                              shelf: shelf,
                              areaId: selection.areaId,
                              areaLabel: selection.areaLabel,
                              neighbourhoodId: selection.neighbourhoodId,
                              neighbourhoodLabel: selection.neighbourhoodLabel,
                            ),
                          ),
                        ),
                    ],
                  ),
                  if (types.length > 1) ...[
                    heading('Type'),
                    Wrap(
                      spacing: 8,
                      runSpacing: 4,
                      children: [
                        for (final entry in {'': 'All types', ...types}.entries)
                          ChoiceChip(
                            label: Text(entry.value),
                            selected: (selection.typeId ?? '') == entry.key,
                            selectedColor: LibraryStyle.libraryTint(context),
                            onSelected: (_) => update(
                              LibraryFilters(
                                shelf: selection.shelf,
                                typeId: entry.key.isEmpty ? null : entry.key,
                                areaId: selection.areaId,
                                areaLabel: selection.areaLabel,
                                neighbourhoodId: selection.neighbourhoodId,
                                neighbourhoodLabel:
                                    selection.neighbourhoodLabel,
                              ),
                            ),
                          ),
                      ],
                    ),
                  ],
                  if (neighbourhoods.isNotEmpty) ...[
                    heading('Within ${selection.areaLabel ?? 'this location'}'),
                    Wrap(
                      spacing: 8,
                      runSpacing: 4,
                      children: [
                        for (final a in [
                          const LibraryArea('all', 'All neighbourhoods'),
                          ...neighbourhoods,
                        ])
                          ChoiceChip(
                            label: Text(a.label),
                            selected:
                                (selection.neighbourhoodId ?? 'all') == a.id,
                            selectedColor: LibraryStyle.libraryTint(context),
                            onSelected: (_) => update(
                              LibraryFilters(
                                shelf: selection.shelf,
                                typeId: selection.typeId,
                                areaId: selection.areaId,
                                areaLabel: selection.areaLabel,
                                neighbourhoodId: a.id == 'all' ? null : a.id,
                                neighbourhoodLabel: a.id == 'all'
                                    ? null
                                    : a.label,
                              ),
                            ),
                          ),
                      ],
                    ),
                  ],
                  heading('Location'),
                  TextField(
                    decoration: InputDecoration(
                      hintText: 'Search locations',
                      prefixIcon: const Icon(Icons.search),
                      focusedBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(12),
                        borderSide: BorderSide(
                          color: LibraryStyle.searchFocus(context),
                          width: 1.5,
                        ),
                      ),
                    ),
                    onChanged: (v) => setState(() => query = v),
                  ),
                  const SizedBox(height: 8),
                  Wrap(
                    spacing: 8,
                    children: [
                      TextButton(
                        onPressed: () => setState(() => regions = false),
                        child: Text(
                          'Destinations',
                          style: TextStyle(
                            color: !regions
                                ? libraryAccent
                                : colors.onSurfaceVariant,
                          ),
                        ),
                      ),
                      TextButton(
                        onPressed: () => setState(() => regions = true),
                        child: Text(
                          'Regions',
                          style: TextStyle(
                            color: regions
                                ? libraryAccent
                                : colors.onSurfaceVariant,
                          ),
                        ),
                      ),
                    ],
                  ),
                  if (areas.isEmpty)
                    const Padding(
                      padding: EdgeInsets.all(16),
                      child: Text('No matching locations'),
                    ),
                  for (final a in areas)
                    ListTile(
                      contentPadding: const EdgeInsets.symmetric(horizontal: 4),
                      title: Text(a.label),
                      subtitle: a.context.isEmpty ? null : Text(a.context),
                      trailing: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          if (a.count != null)
                            Text(
                              '${a.count}',
                              style: TextStyle(color: colors.onSurfaceVariant),
                            ),
                          if ((selection.areaId ?? 'all') == a.id)
                            Padding(
                              padding: const EdgeInsets.only(left: 8),
                              child: Icon(Icons.check, color: libraryAccent),
                            ),
                        ],
                      ),
                      onTap: () => update(
                        LibraryFilters(
                          shelf: selection.shelf,
                          typeId: selection.typeId,
                          areaId: a.id == 'all' ? null : a.id,
                          areaLabel: a.id == 'all' ? null : a.label,
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
