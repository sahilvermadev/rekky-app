import 'rekky_api.dart';

class AskEvidence {
  const AskEvidence(this.id, this.kind, this.text);
  final String id, kind, text;
  factory AskEvidence.fromJson(Map<String, dynamic> value) => AskEvidence(
    value['id'] as String,
    value['kind'] as String,
    value['text'] as String,
  );
}

class AskResult {
  const AskResult({
    required this.item,
    required this.section,
    required this.reason,
    required this.caveat,
    required this.evidence,
  });
  final RekkyItem item;
  final String section, reason, caveat;
  final List<AskEvidence> evidence;
  factory AskResult.fromJson(Map<String, dynamic> value) => AskResult(
    item: RekkyItem.fromJson(value['item'] as Map<String, dynamic>),
    section: value['section'] as String,
    reason: value['reason'] as String,
    caveat: value['caveat'] as String,
    evidence: (value['evidence'] as List)
        .map((v) => AskEvidence.fromJson(v as Map<String, dynamic>))
        .toList(),
  );
}

class AskComparisonCell {
  const AskComparisonCell({
    required this.itemId,
    required this.text,
    required this.evidence,
  });
  final String itemId, text;
  final List<AskEvidence> evidence;
  factory AskComparisonCell.fromJson(Map<String, dynamic> value) =>
      AskComparisonCell(
        itemId: value['item_id'] as String,
        text: value['text'] as String? ?? '',
        evidence: (value['evidence'] as List)
            .map((e) => AskEvidence.fromJson(e as Map<String, dynamic>))
            .toList(),
      );
}

class AskComparisonDimension {
  const AskComparisonDimension(this.label, this.cells);
  final String label;
  final List<AskComparisonCell> cells;
  factory AskComparisonDimension.fromJson(Map<String, dynamic> value) =>
      AskComparisonDimension(
        value['label'] as String,
        (value['cells'] as List)
            .map((e) => AskComparisonCell.fromJson(e as Map<String, dynamic>))
            .toList(),
      );
}

class AskComparison {
  const AskComparison({
    required this.items,
    required this.dimensions,
    required this.conclusion,
    required this.citations,
  });
  final List<RekkyItem> items;
  final List<AskComparisonDimension> dimensions;
  final String conclusion;
  final List<AskComparisonCell> citations;
  factory AskComparison.fromJson(Map<String, dynamic> value) => AskComparison(
    items: (value['items'] as List)
        .map((e) => RekkyItem.fromJson(e as Map<String, dynamic>))
        .toList(),
    dimensions: (value['dimensions'] as List)
        .map((e) => AskComparisonDimension.fromJson(e as Map<String, dynamic>))
        .toList(),
    conclusion: value['conclusion'] as String,
    citations: (value['citations'] as List)
        .map((e) => AskComparisonCell.fromJson(e as Map<String, dynamic>))
        .toList(),
  );
}

class AskAnswer {
  const AskAnswer({
    required this.requestId,
    required this.title,
    required this.intent,
    required this.mode,
    required this.clarification,
    required this.choices,
    required this.location,
    required this.results,
    this.nextOffset,
    this.comparison,
    this.changed = false,
    this.searchIncomplete = false,
    this.question = '',
    this.turnCount = 1,
    this.selectedItemIds = const [],
    this.excludedItemIds = const [],
  });
  final String requestId, title, intent, mode, clarification, location;
  final List<String> choices;
  final List<AskResult> results;
  final int? nextOffset;
  final AskComparison? comparison;
  List<RekkyItem> get items =>
      comparison?.items ?? results.map((r) => r.item).toList();
  final bool changed, searchIncomplete;
  final String question;
  final int turnCount;
  final List<String> selectedItemIds, excludedItemIds;
  factory AskAnswer.fromJson(Map<String, dynamic> value) => AskAnswer(
    requestId: value['request_id'] as String,
    title: value['title'] as String,
    intent: value['intent'] as String,
    mode: value['mode'] as String,
    clarification: value['clarification'] as String,
    location: value['location'] as String,
    choices: (value['choices'] as List).cast<String>(),
    results: (value['results'] as List)
        .map((v) => AskResult.fromJson(v as Map<String, dynamic>))
        .toList(),
    nextOffset: value['next_offset'] as int?,
    comparison: value['comparison'] == null
        ? null
        : AskComparison.fromJson(value['comparison'] as Map<String, dynamic>),
    changed: value['changed'] as bool? ?? false,
    searchIncomplete: value['search_incomplete'] as bool? ?? false,
    question: value['question'] as String? ?? '',
    turnCount: value['turn_count'] as int? ?? 1,
    selectedItemIds: (value['selected_item_ids'] as List? ?? []).cast<String>(),
    excludedItemIds: (value['excluded_item_ids'] as List? ?? []).cast<String>(),
  );
  AskAnswer append(AskAnswer page) => page.changed
      ? page
      : AskAnswer(
          requestId: requestId,
          comparison: comparison,
          question: question,
          turnCount: turnCount,
          selectedItemIds: selectedItemIds,
          excludedItemIds: excludedItemIds,
          title: title,
          intent: intent,
          mode: mode,
          clarification: clarification,
          choices: choices,
          location: location,
          results: [
            ...results,
            ...page.results.where(
              (r) => !results.any((old) => old.item.id == r.item.id),
            ),
          ],
          nextOffset: page.nextOffset,
          changed: changed || page.changed,
          searchIncomplete: searchIncomplete || page.searchIncomplete,
        );
}
