import 'rekky_api.dart';

enum LibraryOrder { browse, recent, pinned }

enum LibraryShelf {
  places('Places', 'place'),
  people('People & services', 'person_service'),
  things('Things', 'thing'),
  activities('Activities & events', 'activity_event'),
  ideas('Ideas & tips', 'idea_tip'),
  notes('Notes', 'unspecified');

  const LibraryShelf(this.label, this.kind);
  final String label, kind;
  static LibraryShelf of(RekkyItem item) => values.firstWhere(
    (s) => s.kind == item.recommendation?.entityKind,
    orElse: () => notes,
  );
}

class LibraryArea {
  const LibraryArea(this.id, this.label);
  final String id, label;
}

// Browsing uses the geographic identity already supplied with the owner item.
// A past trip or contextual mention never establishes present coverage.
Iterable<RecommendationDetail> libraryLocations(RekkyItem item) =>
    item.recommendation?.locations.where(
      (l) => const {'venue', 'practice', 'service_area'}.contains(l.kind),
    ) ??
    const [];

List<LibraryArea> libraryAreas(List<RekkyItem> items) {
  final areas = <String, String>{};
  final kinds = <String, String>{};
  final direct = <String, String>{};
  for (final item in items) {
    for (final location in libraryLocations(item)) {
      final geo = location.geography;
      if (geo?['status'] != 'resolved') continue;
      final id = geo?['area_id'];
      if (id is String) direct[id] = location.displayText;
      final filterIds = (geo?['filter_ids'] as List? ?? const []).toSet();
      for (final parent in (geo?['hierarchy'] as List? ?? const [])) {
        if (parent is Map &&
            parent['id'] is String &&
            parent['name'] is String &&
            filterIds.contains(parent['id'])) {
          final id = parent['id'] as String;
          areas[id] = parent['name'] as String;
          kinds[id] = parent['kind'] as String? ?? '';
        }
      }
    }
  }
  // Prefer the resolved place's display label when it also occurs as a parent.
  areas.addAll(direct);
  final counts = <String, int>{};
  for (final label in areas.values) {
    counts[label] = (counts[label] ?? 0) + 1;
  }
  return areas.entries.map((e) {
    final kind = switch (kinds[e.key]) {
      'ADM1' => 'State / region',
      'ADM2' => 'District',
      'ADM3' => 'Subdistrict',
      'PCLI' => 'Country',
      _ => 'Place',
    };
    return LibraryArea(
      e.key,
      counts[e.value]! > 1 ? '${e.value} · $kind' : e.value,
    );
  }).toList()..sort((a, b) {
    final label = a.label.compareTo(b.label);
    return label == 0 ? a.id.compareTo(b.id) : label;
  });
}

bool libraryInArea(RekkyItem item, String area) {
  final resolved = libraryLocations(item)
      .where((l) => l.geography?['status'] == 'resolved');
  if (area == 'unresolved') return resolved.isEmpty;
  return resolved.any(
    (l) =>
        l.geography?['area_id'] == area ||
        (l.geography?['filter_ids'] as List? ?? const []).contains(area),
  );
}

bool libraryMatches(RekkyItem item, String query) {
  final rec = item.recommendation;
  final text = [
    item.subject,
    item.body,
    rec?.categoryLabel ?? '',
    rec?.contactSavedName ?? '',
    LibraryShelf.of(item).label,
    ...?rec?.locations.map((l) => l.displayText),
    ...?rec?.classification?.types.map((c) => c.label),
    ...?rec?.classification?.facets.map((c) => c.label),
  ].join(' ').toLowerCase();
  return query
      .toLowerCase()
      .trim()
      .split(RegExp(r'\s+'))
      .where((t) => t.isNotEmpty)
      .every(text.contains);
}

List<RekkyItem> librarySelection(
  List<RekkyItem> items, {
  String query = '',
  LibraryShelf? shelf,
  String? typeId,
  String? areaId,
  LibraryOrder order = LibraryOrder.browse,
}) =>
    items
        .where(
          (item) =>
              (shelf == null || LibraryShelf.of(item) == shelf) &&
              (typeId == null ||
                  (item.recommendation?.classification?.types.any(
                        (c) => c.id == typeId,
                      ) ??
                      false)) &&
              (areaId == null || libraryInArea(item, areaId)) &&
              (order != LibraryOrder.pinned || item.pinned) &&
              libraryMatches(item, query),
        )
        .toList()
      ..sort((a, b) {
        final ad = DateTime.tryParse(a.createdAt);
        final bd = DateTime.tryParse(b.createdAt);
        final date = ad == null || bd == null ? 0 : bd.compareTo(ad);
        return date == 0 ? b.id.compareTo(a.id) : date;
      });

String libraryMonth(RekkyItem item) {
  final date = DateTime.tryParse(item.createdAt)?.toLocal();
  if (date == null) return 'Saved earlier';
  const months = [
    'January',
    'February',
    'March',
    'April',
    'May',
    'June',
    'July',
    'August',
    'September',
    'October',
    'November',
    'December',
  ];
  return '${months[date.month - 1]} ${date.year}';
}
