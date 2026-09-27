import 'package:flutter/material.dart';

import 'ask_answer.dart';
import 'rekky_api.dart';
import 'library_style.dart';
import 'contact_matching.dart';
import 'recommendation_maps_action.dart';

/// Shared dimensions stay vertical, including at large accessibility text sizes.
class AskComparisonView extends StatelessWidget {
  const AskComparisonView({
    super.key,
    required this.comparison,
    required this.onOpen,
    required this.onAction,
    required this.onSelect,
    required this.selected,
  });
  final AskComparison comparison;
  final ValueChanged<RekkyItem> onOpen, onAction;
  final ValueChanged<RekkyItem>? onSelect;
  final List<String> selected;

  String name(String id) =>
      comparison.items.firstWhere((i) => i.id == id).subject;

  void evidence(BuildContext context, List<AskComparisonCell> citations) {
    showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      useSafeArea: true,
      isScrollControlled: true,
      builder: (context) => SafeArea(
        child: SingleChildScrollView(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(24, 8, 24, 32),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                for (final citation in citations) ...[
                  Text(
                    name(citation.itemId),
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                  const SizedBox(height: 8),
                  for (final e in citation.evidence)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 16),
                      child: Text('“${e.text}”'),
                    ),
                ],
                Text(
                  'From your saved recommendations',
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final item in comparison.items)
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Material(
              color: colors.surfaceContainerLow,
              borderRadius: BorderRadius.circular(12),
              clipBehavior: Clip.antiAlias,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(12, 4, 12, 4),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    TextButton(
                      onPressed: () => onOpen(item),
                      style: TextButton.styleFrom(
                        alignment: Alignment.centerLeft,
                      ),
                      child: Row(
                        children: [
                          Expanded(
                            child: Text(
                              item.subject,
                              style: theme.textTheme.titleMedium,
                            ),
                          ),
                          const SizedBox(width: 8),
                          const Icon(Icons.arrow_outward_rounded, size: 18),
                        ],
                      ),
                    ),
                    Wrap(
                      spacing: 8,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: [
                        FilterChip(
                          label: const Text('Ask about'),
                          selected: selected.contains(item.id),
                          onSelected: onSelect == null
                              ? null
                              : (_) => onSelect!(item),
                        ),
                        if (internationalPhone(
                                  item.recommendation?.contactPhone ?? '',
                                ) !=
                                null ||
                            recommendationDestination(item) != null)
                          TextButton(
                            onPressed: () => onAction(item),
                            child: Text(
                              internationalPhone(
                                        item.recommendation?.contactPhone ?? '',
                                      ) !=
                                      null
                                  ? 'Call'
                                  : 'Maps',
                            ),
                          ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
          ),
        if (comparison.conclusion.isNotEmpty) ...[
          const SizedBox(height: 12),
          Text(
            comparison.conclusion,
            style: theme.textTheme.bodyLarge?.copyWith(height: 1.5),
          ),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              onPressed: () => evidence(context, comparison.citations),
              icon: const Icon(Icons.format_quote_rounded, size: 18),
              label: const Text('Saved evidence'),
            ),
          ),
        ],
        for (final dimension in comparison.dimensions) ...[
          const SizedBox(height: 20),
          Text(
            dimension.label,
            style: theme.textTheme.titleMedium?.copyWith(
              color: LibraryStyle.searchFocus(context),
              fontWeight: FontWeight.w600,
            ),
          ),
          const SizedBox(height: 8),
          Material(
            color: colors.surfaceContainerLow,
            borderRadius: BorderRadius.circular(16),
            clipBehavior: Clip.antiAlias,
            child: Column(
              children: [
                for (var i = 0; i < dimension.cells.length; i++) ...[
                  if (i > 0)
                    Divider(
                      height: 1,
                      indent: 16,
                      endIndent: 16,
                      color: colors.outlineVariant.withValues(alpha: .4),
                    ),
                  _cell(context, dimension.cells[i]),
                ],
              ],
            ),
          ),
        ],
      ],
    );
  }

  Widget _cell(BuildContext context, AskComparisonCell cell) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 12, 8, 12),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  name(cell.itemId),
                  style: theme.textTheme.labelMedium?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
                const SizedBox(height: 4),
                Text(
                  cell.text,
                  style: theme.textTheme.bodyMedium?.copyWith(
                    height: 1.45,
                    color: cell.evidence.isEmpty
                        ? theme.colorScheme.onSurfaceVariant
                        : null,
                  ),
                ),
              ],
            ),
          ),
          if (cell.evidence.isNotEmpty)
            IconButton(
              tooltip: 'Saved evidence for ${name(cell.itemId)}',
              onPressed: () => evidence(context, [cell]),
              icon: const Icon(Icons.format_quote_rounded, size: 19),
            )
          else
            const SizedBox(width: 8),
        ],
      ),
    );
  }
}
