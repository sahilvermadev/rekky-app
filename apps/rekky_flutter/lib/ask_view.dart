import 'rekky_api.dart';

class AskBrowseSpec {
  const AskBrowseSpec({
    this.title = '',
    this.location = '',
    this.categoryIds = const [],
    this.kind = '',
    this.query = '',
    this.locationRole = 'relevant',
    this.sort = 'saved_newest',
  });
  final String title, location, kind, query, locationRole, sort;
  final List<String> categoryIds;
  factory AskBrowseSpec.fromJson(Map<String, dynamic> j) => AskBrowseSpec(
    title: j['title'] as String? ?? '',
    location: j['location'] as String? ?? '',
    categoryIds: (j['category_ids'] as List? ?? []).cast<String>(),
    kind: j['kind'] as String? ?? '',
    query: j['query'] as String? ?? '',
    locationRole: j['location_role'] as String? ?? 'relevant',
    sort: j['sort'] as String? ?? 'saved_newest',
  );
  Map<String, dynamic> toJson() => {
    'title': title,
    'location': location,
    'category_ids': categoryIds,
    'kind': kind,
    'query': query,
    'location_role': locationRole,
    'sort': sort,
    'source': 'mine',
  };
  AskBrowseSpec copyWith({
    String? location,
    List<String>? categoryIds,
    String? kind,
    String? query,
    String? sort,
  }) => AskBrowseSpec(
    location: location ?? this.location,
    categoryIds: categoryIds ?? this.categoryIds,
    kind: kind ?? this.kind,
    query: query ?? this.query,
    sort: sort ?? this.sort,
    locationRole: locationRole,
  );
}

class AskFacet {
  const AskFacet(this.id, this.label, this.count);
  final String id, label;
  final int count;
  factory AskFacet.fromJson(Map<String, dynamic> j) =>
      AskFacet(j['id'] as String, j['label'] as String, j['count'] as int);
}

class AskView {
  const AskView({
    required this.id,
    required this.title,
    required this.total,
    required this.spec,
    required this.items,
    this.categories = const [],
    this.kinds = const [],
    this.areas = const [],
    this.areaLabel = '',
    this.nextOffset,
  });
  final String id, title, areaLabel;
  final int total;
  final AskBrowseSpec spec;
  final List<RekkyItem> items;
  final List<AskFacet> categories, kinds, areas;
  final int? nextOffset;
  factory AskView.fromJson(Map<String, dynamic> j) {
    List<AskFacet> facets(String key) =>
        ((j['facets'] as Map<String, dynamic>?)?[key] as List? ?? [])
            .map((e) => AskFacet.fromJson(e as Map<String, dynamic>))
            .toList();
    return AskView(
      id: j['view_id'] as String,
      title: j['title'] as String,
      total: j['total'] as int,
      spec: AskBrowseSpec.fromJson(j['spec'] as Map<String, dynamic>),
      items: (j['items'] as List)
          .map((e) => RekkyItem.fromJson(e as Map<String, dynamic>))
          .toList(),
      categories: facets('categories'),
      kinds: facets('kinds'),
      areas: facets('areas'),
      areaLabel:
          (j['area'] as Map<String, dynamic>?)?['label'] as String? ?? '',
      nextOffset: j['next_offset'] as int?,
    );
  }
  AskView append(AskView page) => AskView(
    id: id,
    title: title,
    total: page.total,
    spec: spec,
    items: [
      ...items,
      ...page.items.where((e) => !items.any((old) => old.id == e.id)),
    ],
    categories: page.categories,
    kinds: page.kinds,
    areas: page.areas,
    areaLabel: page.areaLabel,
    nextOffset: page.nextOffset,
  );
}
