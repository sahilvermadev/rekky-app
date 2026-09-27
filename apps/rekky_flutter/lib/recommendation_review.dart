import 'package:flutter/material.dart';

import 'rekky_api.dart';

/// Quiet affordance for processing uncertainty and source-stated caveats.
/// Reading it never clears the recording-level review flag.
class RecommendationReviewButton extends StatelessWidget {
  const RecommendationReviewButton({
    super.key,
    required this.item,
    this.onEdit,
  });

  final RekkyItem item;
  final VoidCallback? onEdit;

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
                  if (item.needsReview)
                    Text(
                      item.recommendation == null
                          ? 'We saved the recording but couldn’t organize it into a recommendation.'
                          : 'We may have missed something from the recording, but can’t pinpoint which detail.',
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
