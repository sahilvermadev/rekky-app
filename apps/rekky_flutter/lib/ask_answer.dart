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
