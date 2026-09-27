import 'package:flutter/material.dart';

import 'rekky_api.dart';

/// Quiet affordance for processing uncertainty and source-stated caveats.
/// Reading it never clears the item-level review state.
class RecommendationReviewButton extends StatelessWidget {
  const RecommendationReviewButton({
    super.key,
    required this.item,
    this.onEdit,
  });

  final RekkyItem item;
  final VoidCallback? onEdit;

  static String? explanation(String code) => switch (code) {
    'unsupported_amount' || 'unsupported_currency' =>
      'A price or amount needs checking. We kept the wording from your note.',
    'possible_missing_caveat' => 'A warning or limitation may not have been captured accurately. We kept the relevant wording from your note.',
    'possible_missing_qualifier' =>
      'An uncertain detail needs checking. We kept its original wording.',
    'location_unresolved' ||
    'location_role_uncertain' => 'The location or service area is unclear.',
    'subject_boundary' || 'subject_unresolved' => 'We could not confidently separate who or what part of the recording refers to.',
    'unrepresented_source' || 'unresolved_meaning' => 'Some details still need organizing. The available original wording has been retained.',
    'invalid_evidence' ||
    'unsupported_link' => 'A detail could not be supported by the recording.',
    'presentation_length' =>
      'Part of this note still needs shortening without losing its meaning.',
    'category_unresolved' => 'The type of recommendation is unclear.',
    _ => null,
  };

  static bool needed(RekkyItem item) =>
      item.needsReview || (item.recommendation?.cautions.isNotEmpty ?? false);

  @override
  Widget build(BuildContext context) {
    if (!needed(item)) return const SizedBox.shrink();
    final label = item.needsReview ? 'Needs review' : 'Caution';
    return IconButton(
      tooltip: label,
      constraints: const BoxConstraints(minWidth: 48, minHeight: 48),
      iconSize: 18,
      color: Theme.of(context).colorScheme.onSurfaceVariant,
      icon: const Icon(Icons.info_outline),
      onPressed: () async {
        final reasons = {
          for (final code in item.recommendation?.reviewIssues ?? <String>[])
            if (explanation(code) case final String reason) reason,
        };
        final cautions = {
          for (final caution
              in item.recommendation?.cautions ?? <RecommendationDetail>[])
            if (caution.text.trim().isNotEmpty) caution.text.trim(),
        };
        final edit = await showDialog<bool>(
          context: context,
          builder: (dialogContext) => AlertDialog(
            title: Text(label),
            content: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  if (item.needsReview && reasons.isEmpty)
                    Text(
                      item.recommendation == null
                          ? 'We saved the recording but couldn’t organize it into a recommendation.'
                          : 'We may have missed something from the recording, but can’t pinpoint which detail.',
                    ),
                  if (item.needsReview)
                    for (final reason in reasons)
                      Padding(
                        padding: EdgeInsets.only(
                          top: reason == reasons.first ? 0 : 12,
                        ),
                        child: Text(reason),
                      ),
                  for (final caution in cautions)
                    Padding(
                      padding: EdgeInsets.only(
                        top: item.needsReview || caution != cautions.first
                            ? 12
                            : 0,
                      ),
                      child: Text(caution),
                    ),
                ],
              ),
            ),
            actions: [
              if (onEdit != null)
                TextButton(
                  onPressed: () => Navigator.pop(dialogContext, true),
                  child: const Text('Edit recommendation'),
                ),
              TextButton(
                onPressed: () => Navigator.pop(dialogContext, false),
                child: const Text('Close'),
              ),
            ],
          ),
        );
        if (edit == true && context.mounted) onEdit?.call();
      },
    );
  }
}
