import 'package:flutter/material.dart';

import 'rekky_api.dart';

/// One saved representation, with a compact preview and richer detail view.
/// No generated markup, contact links, or inferred public destinations.
class RecommendationView extends StatelessWidget {
  const RecommendationView({
    super.key,
    required this.item,
    this.expanded = false,
  });
  final RekkyItem item;
  final bool expanded;

  @override
  Widget build(BuildContext context) {
    final recommendation = item.recommendation;
    final theme = Theme.of(context);
    if (recommendation == null) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (item.needsReview)
            Text('Needs review', style: theme.textTheme.labelMedium),
          Text(
            item.body,
            maxLines: expanded ? null : 3,
            overflow: expanded ? null : TextOverflow.ellipsis,
          ),
        ],
      );
    }
    Widget detail(String title, String text) => Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: theme.textTheme.labelLarge),
          const SizedBox(height: 3),
          Text(text),
        ],
      ),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          [
            recommendation.shelf,
            if (recommendation.primaryLocation != null)
              recommendation.primaryLocation!,
          ].join(' · '),
          style: theme.textTheme.labelMedium?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 6),
        Text(
          recommendation.experienceLabel,
          style: theme.textTheme.labelMedium,
        ),
        if (item.needsReview) const Text('Some details need review'),
        const SizedBox(height: 8),
        Text(recommendation.summary, style: theme.textTheme.bodyLarge),
        // Caveats stay visible even in previews and at large text sizes.
        for (final caution in recommendation.cautions)
          detail('Worth knowing', caution.text),
        if (expanded) ...[
          for (final observation in recommendation.observations.where(
            (o) => o.kind != 'caution',
          ))
            detail(switch (observation.kind) {
              'suggestion' => 'Recommended',
              'suitability' => 'Good for',
              'price' => 'Price mentioned · current price unverified',
              'praise' => 'What stood out',
              _ => 'More context',
            }, observation.text),
          for (final location in recommendation.locations)
            detail(switch (location.kind) {
              'venue' => 'Location mentioned',
              'practice' => 'Practices in',
              'service_area' => 'Stated service area',
              'past_experience' => 'Location of the experience',
              _ => 'Location in context',
            }, location.text),
          if (recommendation.useCases.isNotEmpty)
            detail('Related needs', recommendation.useCases.join(' · ')),
        ],
      ],
    );
  }
}
