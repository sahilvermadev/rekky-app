import 'package:flutter/material.dart';

import 'rekky_theme.dart';

// Hallmark · playful capture dock · neutral chrome, coral contribution action.
// Pre-emit critique: P5 H5 E4 S5 R5 V5.
class RekkyNavigation extends StatelessWidget {
  const RekkyNavigation({
    super.key,
    required this.destination,
    required this.onSelect,
    required this.onRemember,
  });
  final int destination;
  final ValueChanged<int> onSelect;
  final VoidCallback onRemember;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    Widget tab(int index, String label, IconData icon) => Expanded(
      child: Semantics(
        selected: destination == index,
        child: TextButton(
          onPressed: () => onSelect(index),
          style: TextButton.styleFrom(
            foregroundColor: destination == index
                ? colors.onSurface
                : colors.onSurfaceVariant,
            padding: const EdgeInsets.symmetric(vertical: 10, horizontal: 4),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(22),
            ),
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 22),
              const SizedBox(height: 4),
              Text(
                label,
                maxLines: 1,
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: destination == index
                      ? FontWeight.w700
                      : FontWeight.w500,
                ),
              ),
              const SizedBox(height: 5),
              Container(
                width: 14,
                height: 3,
                decoration: BoxDecoration(
                  color: destination == index
                      ? colors.onSurface
                      : Colors.transparent,
                  borderRadius: BorderRadius.circular(2),
                ),
              ),
            ],
          ),
        ),
      ),
    );
    Widget recommend({required bool wide}) => Semantics(
      hint: 'Start recording a recommendation',
      child: FilledButton(
        onPressed: onRemember,
        style: FilledButton.styleFrom(
          backgroundColor: RekkyTheme.capture,
          foregroundColor: RekkyTheme.onCapture,
          padding: EdgeInsets.symmetric(
            horizontal: 12,
            vertical: wide ? 16 : 12,
          ),
          minimumSize: const Size(48, 72),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(24),
          ),
        ),
        child: const Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.mic_none_rounded, size: 26),
            SizedBox(height: 4),
            Text(
              'Recommend',
              maxLines: 1,
              style: TextStyle(fontSize: 13, fontWeight: FontWeight.w700),
            ),
          ],
        ),
      ),
    );
    return ColoredBox(
      color: colors.surface,
      child: SafeArea(
        top: false,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 6, 12, 8),
          child: LayoutBuilder(
            builder: (context, constraints) {
              final label = TextPainter(
                text: TextSpan(
                  text: 'Recommend',
                  style: Theme.of(context).textTheme.labelLarge
                      ?.copyWith(fontSize: 13, fontWeight: FontWeight.w700),
                ),
                textDirection: Directionality.of(context),
                textScaler: MediaQuery.textScalerOf(context),
              )..layout();
              final wide =
                  MediaQuery.textScalerOf(context).scale(13) > 18 ||
                  label.width > (constraints.maxWidth - 16) / 2 - 36;
              label.dispose();
              final decoration = BoxDecoration(
                color: colors.surfaceContainerLow,
                border: Border.all(
                  color: colors.outlineVariant.withValues(alpha: .65),
                ),
                borderRadius: BorderRadius.circular(30),
              );
              if (wide) {
                return DecoratedBox(
                  decoration: decoration,
                  child: Padding(
                    padding: const EdgeInsets.all(8),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        recommend(wide: true),
                        Row(
                          children: [
                            tab(0, 'Ask', Icons.search_rounded),
                            tab(
                              1,
                              'Library',
                              destination == 1
                                  ? Icons.bookmarks_rounded
                                  : Icons.bookmarks_outlined,
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                );
              }
              return Stack(
                children: [
                  Positioned(
                    top: 12,
                    left: 0,
                    right: 0,
                    bottom: 0,
                    child: DecoratedBox(decoration: decoration),
                  ),
                  Padding(
                    padding: const EdgeInsets.fromLTRB(8, 0, 8, 6),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.end,
                      children: [
                        tab(0, 'Ask', Icons.search_rounded),
                        Expanded(
                          flex: 2,
                          child: Padding(
                            padding: const EdgeInsets.fromLTRB(6, 0, 6, 8),
                            child: recommend(wide: false),
                          ),
                        ),
                        tab(
                          1,
                          'Library',
                          destination == 1
                              ? Icons.bookmarks_rounded
                              : Icons.bookmarks_outlined,
                        ),
                      ],
                    ),
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );
  }
}
