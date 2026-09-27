import 'recommendation_review.dart';

import 'package:flutter/material.dart';

import 'rekky_api.dart';
import 'contact_action.dart';
import 'recommendation_maps_action.dart';
import 'library_collection.dart';
import 'library_style.dart';

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
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (RecommendationReviewButton.needed(item))
            RecommendationReviewButton(item: item),
          Icon(
            item.visibility == 'private'
                ? Icons.lock_outline
                : Icons.people_outline,
            size: 18,
            semanticLabel: item.visibility == 'private' ? 'Only me' : 'Friends',
          ),
        ],
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
    this.place,
    this.onManageContact,
  });
  final RekkyItem item;
  final bool expanded;
  final ResolvedPlace? place;
  final VoidCallback? onManageContact;

  @override
  Widget build(BuildContext context) {
    final recommendation = item.recommendation;
    final theme = Theme.of(context);
    if (!expanded) {
      final signals = [
        if (recommendation?.experience == 'secondhand') 'Heard from others',
        if (recommendation?.experience == 'interest') 'Not tried yet',
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
          if (recommendation?.rating != null || signals.isNotEmpty) ...[
            const SizedBox(height: 4),
            Wrap(
              crossAxisAlignment: WrapCrossAlignment.center,
              spacing: 10,
              runSpacing: 4,
              children: [
                if (recommendation?.rating case final rating?)
                  _RatingView(rating: rating, compact: true),
                if (signals.isNotEmpty)
                  Text(signals.join(' · '), style: theme.textTheme.labelSmall),
              ],
            ),
          ],
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
    // Retrieval metadata is not a second account of the experience. Keep every
    // distinct observation; only verbatim sentences/duplicates are suppressed.
    final paragraphs = <RecommendationDetail>[];
    final seen = <String>{};
    String normalize(String text) =>
        text.trim().toLowerCase().replaceAll(RegExp(r'\s+'), ' ');
    final summarySentences = recommendation.summary
        .split(RegExp(r'(?<=[.!?])\s+'))
        .map(normalize)
        .toSet();
    for (final observation in recommendation.observations) {
      final text = normalize(observation.text);
      if (text.isNotEmpty &&
          seen.add(text) &&
          text != normalize(recommendation.summary) &&
          !summarySentences.contains(text)) {
        paragraphs.add(observation);
      }
    }
    final metadata = <String>{
      for (final type
          in recommendation.classification?.types.skip(1) ??
              <CategoryConcept>[])
        type.label,
      for (final facet
          in recommendation.classification?.facets ?? <CategoryConcept>[])
        if (!recommendation.categoryLabel.toLowerCase().contains(
          facet.label.toLowerCase(),
        ))
          facet.label,
    };
    final otherLocations = recommendation.locations.where(
      (l) => l.displayText != recommendation.primaryLocation,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(recommendation.categoryLabel, style: secondary),
        if (recommendation.rating case final rating?) ...[
          const SizedBox(height: 8),
          _RatingView(rating: rating),
        ],
        if (place != null || recommendation.primaryLocation != null) ...[
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
                child: Text(
                  place?.address ?? recommendation.primaryLocationLabel!,
                  style: secondary,
                ),
              ),
            ],
          ),
        ],
        if (recommendation.entityKind == 'person_service')
          ContactAction(
            phone: recommendation.contactPhone,
            savedName: recommendation.contactSavedName,
            onManage: onManageContact,
          ),
        RecommendationMapsAction(item: item, place: place),
        const SizedBox(height: 24),
        if (recommendation.experience == 'secondhand' ||
            recommendation.experience == 'interest') ...[
          Text(
            recommendation.experienceLabel,
            style: theme.textTheme.labelLarge?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: 8),
        ],
        if (recommendation.attribution.isNotEmpty &&
            recommendation.experience != 'secondhand') ...[
          Text('Source: ${recommendation.attribution}', style: secondary),
          const SizedBox(height: 8),
        ],
        Text(
          recommendation.summary,
          style: theme.textTheme.bodyLarge?.copyWith(
            height: 1.55,
            letterSpacing: 0,
          ),
        ),
        for (final paragraph in paragraphs)
          Padding(
            padding: const EdgeInsets.only(top: 14),
            child: Container(
              padding: paragraph.kind == 'caution'
                  ? const EdgeInsets.only(left: 12)
                  : null,
              decoration: paragraph.kind == 'caution'
                  ? BoxDecoration(
                      border: Border(
                        left: BorderSide(
                          color: theme.colorScheme.outlineVariant,
                          width: 2,
                        ),
                      ),
                    )
                  : null,
              child: Text(
                paragraph.text,
                style: theme.textTheme.bodyLarge?.copyWith(
                  height: 1.5,
                  letterSpacing: 0,
                ),
              ),
            ),
          ),
        for (final location in otherLocations)
          Padding(
            padding: const EdgeInsets.only(top: 12),
            child: Text(
              '${switch (location.kind) {
                'service_area' => 'Serves',
                'practice' => 'Practices in',
                'past_experience' => 'Experience in',
                'venue' when recommendation.entityKind == 'place' => 'Also in',
                _ => 'Mentioned',
              }}: ${location.displayText}',
              style: secondary,
            ),
          ),
        if (metadata.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(top: 12),
            child: Text(metadata.join(' · '), style: secondary),
          ),
      ],
    );
  }
}

class _RatingView extends StatelessWidget {
  const _RatingView({required this.rating, this.compact = false});
  final bool compact;
  final RecommendationRating rating;
  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final explanation = rating.estimated
        ? 'AI estimate of the author’s opinion in the note. Can be changed in Edit.'
        : rating.origin == 'spoken'
        ? rating.spokenScale == 5
              ? 'The author gave ${rating.spokenValue}/5 in the note, shown out of 10.'
              : 'The author’s rating from the note.'
        : 'Rating set by the author.';
    return Tooltip(
      message: explanation,
      child: Semantics(
        label:
            '${rating.estimated ? 'Estimated rating' : 'Author rating'}: ${rating.label} out of 10. $explanation',
        child: ExcludeSemantics(
          child: Wrap(
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: compact ? 4 : 8,
            children: [
              Icon(
                Icons.star_rounded,
                size: compact ? 16 : 20,
                color: LibraryStyle.accent(context, LibraryShelf.ideas),
              ),
              Text(
                '${rating.label}/10',
                style: compact
                    ? theme.textTheme.labelMedium
                    : theme.textTheme.titleSmall,
              ),
              if (rating.estimated)
                Text(
                  'Estimated',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
