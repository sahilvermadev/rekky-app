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
  const LibraryArea(
    this.id,
    this.label, {
    this.aliases = const [],
    this.count,
    this.kind = 'city',
    this.context = '',
  });
  final String id, label, kind, context;
  final List<String> aliases;
  final int? count;
  bool matches(String query) => [
    label,
    context,
    ...aliases,
  ].join(' ').toLowerCase().contains(query.trim().toLowerCase());
}

// Past trips and contextual mentions never establish a location or coverage.
Iterable<RecommendationDetail> libraryLocations(RekkyItem item) =>
    item.recommendation?.locations.where(
      (l) => const {'venue', 'practice', 'service_area'}.contains(l.kind),
    ) ??
    const [];

Map<String, dynamic>? _browse(RecommendationDetail l) =>
    l.geography?['browse'] as Map<String, dynamic>?;

List<LibraryArea> libraryAreas(
  List<RekkyItem> items, {
  bool regions = false,
  String? cityId,
}) {
  final areas = <String, Map<String, dynamic>>{};
  for (final item in items) {
    for (final location in libraryLocations(item)) {
      final geo = location.geography;
      if (geo?['status'] != 'resolved') continue;
      final browse = _browse(location);
      final candidates = <Map<String, dynamic>>[];
      if (cityId != null) {
        if (browse?['destination']?['id'] == cityId &&
            browse?['neighbourhood'] is Map<String, dynamic>) {
          candidates.add(browse!['neighbourhood'] as Map<String, dynamic>);
        }
      } else if (regions) {
        candidates.addAll(
          (browse?['regions'] as List? ?? const [])
              .whereType<Map<String, dynamic>>(),
        );
      } else if (browse?['destination'] is Map<String, dynamic>) {
        candidates.add(browse!['destination'] as Map<String, dynamic>);
      } else if (geo?['area_id'] is String) {
        // Older backend: retain the explicit place without exposing all ancestors.
        candidates.add({
          'id': geo!['area_id'],
          'label': location.displayText,
          'kind': 'locality',
        });
      }
      for (final candidate in candidates) {
        final id = candidate['id'] as String;
        areas[id] = candidate;
      }
    }
  }
  return areas.entries.map((e) {
    final value = e.value;
    return LibraryArea(
      e.key,
      value['label'] as String,
      aliases: (value['aliases'] as List? ?? const [])
          .whereType<String>()
          .toList(),
      count: cityId == null
          ? items.where((i) => libraryInArea(i, e.key)).length
          : items
                .where((i) => libraryInArea(i, cityId, neighbourhoodId: e.key))
                .length,
      kind: value['kind'] as String? ?? 'locality',
      context: value['country_code'] as String? ?? '',
    );
  }).toList()..sort((a, b) => a.label.compareTo(b.label));
}

bool libraryInArea(RekkyItem item, String area, {String? neighbourhoodId}) {
  final resolved = libraryLocations(item)
      .where((l) => l.geography?['status'] == 'resolved');
  if (area == 'unresolved') return resolved.isEmpty;
  return resolved.any((l) {
    final geo = l.geography!;
    final browse = _browse(l);
    final matchesArea =
        geo['area_id'] == area ||
        (geo['filter_ids'] as List? ?? const []).contains(area);
    if (!matchesArea) return false;
    if (neighbourhoodId == null) return true;
    if (browse?['destination']?['id'] != area) return false;
    return browse?['neighbourhood']?['id'] == neighbourhoodId ||
        // Explicit city-wide service coverage is useful within the city. A city-only
        // practice/venue must never be presented as a neighbourhood match.
        (l.kind == 'service_area' && geo['area_id'] == area);
  });
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
  String? neighbourhoodId,
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
              (areaId == null ||
                  libraryInArea(
                    item,
                    areaId,
                    neighbourhoodId: neighbourhoodId,
                  )) &&
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
