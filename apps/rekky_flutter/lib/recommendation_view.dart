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
                    recommendation.categoryLabel,
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
        children: [Text(item.body)],
      );
    }
    final secondary = theme.textTheme.bodyMedium?.copyWith(
      color: theme.colorScheme.onSurfaceVariant,
      height: 1.4,
    );
    final groups = <String, List<String>>{};
    void add(String heading, String text) {
      final values = groups.putIfAbsent(heading, () => []);
      if (text.trim().isNotEmpty && !values.contains(text)) values.add(text);
    }

    final supporting = <String, List<String>>{};
    for (final observation in recommendation.observations) {
      if (observation.kind == 'caution') continue;
      final heading = switch (observation.kind) {
        'suggestion' => 'Recommended',
        'suitability' => 'Good for',
        'price' => 'Price mentioned · current price unverified',
        'praise' => 'What stood out',
        _ => 'More context',
      };
      // Praise often paraphrases the summary. Retain it on demand rather than
      // guessing semantic equivalence or deleting evidence from the model.
      if (observation.kind == 'praise') {
        final values = supporting.putIfAbsent(heading, () => []);
        if (!values.contains(observation.text)) values.add(observation.text);
      } else if (observation.text.trim() != recommendation.summary.trim()) {
        add(heading, observation.text);
      }
    }
    for (final location in recommendation.locations) {
      if (location.text == recommendation.primaryLocation &&
          (location.kind == 'venue' || location.kind == 'practice')) {
        continue;
      }
      add(switch (location.kind) {
        'venue' => 'Other location mentioned',
        'practice' => 'Also practices in',
        'service_area' => 'Stated service area',
        'past_experience' => 'Location of the experience',
        _ => 'Location in context',
      }, location.text);
    }
    for (final type
        in recommendation.classification?.types.skip(1) ??
            <CategoryConcept>[]) {
      add('Also', type.label);
    }
    for (final facet
        in recommendation.classification?.facets ?? <CategoryConcept>[]) {
      // The compound title already says Italian restaurant, for example.
      if (!recommendation.categoryLabel.toLowerCase().contains(
        facet.label.toLowerCase(),
      )) {
        add(facet.dimensionLabel, facet.label);
      }
    }
    for (final descriptor
        in recommendation.classification?.descriptors ?? <String>[]) {
      add('More about it', descriptor);
    }
    for (final useCase in recommendation.useCases) {
      add('Related needs', useCase);
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(recommendation.categoryLabel, style: secondary),
        if (recommendation.primaryLocation != null) ...[
          const SizedBox(height: 6),
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(
                Icons.location_on_outlined,
                size: 18,
                color: theme.colorScheme.onSurfaceVariant,
              ),
              const SizedBox(width: 6),
              Expanded(
                child: Text(recommendation.primaryLocation!, style: secondary),
              ),
            ],
          ),
        ],
        RecommendationMapsAction(item: item),
        const SizedBox(height: 24),
        Text(
          recommendation.experienceLabel,
          style: theme.textTheme.labelLarge?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        if (recommendation.attribution.isNotEmpty &&
            recommendation.experience != 'secondhand')
          Text('Source: ${recommendation.attribution}', style: secondary),
        const SizedBox(height: 8),
        Text(
          recommendation.summary,
          style: theme.textTheme.bodyLarge?.copyWith(
            height: 1.55,
            letterSpacing: 0,
          ),
        ),
        if (recommendation.cautions.isNotEmpty)
          _ReadingSection(
            title: 'Worth knowing',
            lines: recommendation.cautions.map((c) => c.text).toSet().toList(),
          ),
        for (final group in groups.entries)
          _ReadingSection(title: group.key, lines: group.value),
        if (supporting.isNotEmpty) ...[
          const SizedBox(height: 16),
          ExpansionTile(
            tilePadding: EdgeInsets.zero,
            childrenPadding: const EdgeInsets.only(bottom: 8),
            shape: const Border(),
            collapsedShape: const Border(),
            title: const Text('More from your note'),
            children: [
              for (final group in supporting.entries)
                _ReadingSection(title: group.key, lines: group.value),
            ],
          ),
        ],
      ],
    );
  }
}

class _ReadingSection extends StatelessWidget {
  const _ReadingSection({required this.title, required this.lines});
  final String title;
  final List<String> lines;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(top: 20),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title,
          style: Theme.of(context).textTheme.titleSmall
              ?.copyWith(fontWeight: FontWeight.w600),
        ),
        const SizedBox(height: 8),
        for (final line in lines)
          Padding(
            padding: EdgeInsets.only(bottom: lines.length > 1 ? 8 : 0),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (lines.length > 1) ...[
                  const Text('•'),
                  const SizedBox(width: 10),
                ],
                Expanded(
                  child: Text(
                    line,
                    style: Theme.of(context).textTheme.bodyLarge
                        ?.copyWith(height: 1.5, letterSpacing: 0),
                  ),
                ),
              ],
            ),
          ),
      ],
    ),
  );
}
