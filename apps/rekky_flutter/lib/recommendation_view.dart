import 'package:flutter/material.dart';

import 'rekky_api.dart';
import 'recommendation_maps_action.dart';

// Hallmark · component scope; existing warm Material tokens, compact list/detail.
// Pre-emit critique: Philosophy 4, Hierarchy 5, Execution 4,
// Specificity 4, Restraint 5, Variety 3.
class RecommendationCard extends StatelessWidget {
  const RecommendationCard({
    super.key,
    required this.item,
    required this.onTap,
  });

  final RekkyItem item;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => Card(
    child: ListTile(
      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
      title: Text(item.subject, maxLines: 2, overflow: TextOverflow.ellipsis),
      subtitle: RecommendationView(item: item),
      trailing: Icon(
        item.visibility == 'private'
            ? Icons.lock_outline
            : Icons.people_outline,
        size: 18,
        semanticLabel: item.visibility == 'private' ? 'Only me' : 'Friends',
      ),
      onTap: onTap,
    ),
  );
}

/// The preview identifies a memory; the detail explains it.
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
    if (!expanded) {
      final signals = [
        if (recommendation?.experience == 'secondhand') 'Heard from others',
        if (recommendation?.experience == 'interest') 'Not tried yet',
        if (recommendation?.cautions.isNotEmpty ?? false) 'Caution',
        if (item.needsReview) 'Needs review',
      ];
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            recommendation == null
                ? 'Saved note'
                : [
                    recommendation.shelf,
                    if (recommendation.primaryLocation != null)
                      recommendation.primaryLocation!,
                  ].join(' · '),
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          if (signals.isNotEmpty)
            Text(signals.join(' · '), style: theme.textTheme.labelSmall),
        ],
      );
    }
    if (recommendation == null) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (item.needsReview)
            Text('Needs review', style: theme.textTheme.labelMedium),
          Text(item.body),
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
        RecommendationMapsAction(item: item),
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
