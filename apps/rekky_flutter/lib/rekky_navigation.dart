import 'package:flutter/material.dart';

import 'rekky_theme.dart';

// Hallmark · user-sketched three-part capsule · flat, text-only navigation.
// Pre-emit critique: P5 H5 E4 S5 R5 V4.
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
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final labelStyle = theme.textTheme.labelLarge!.copyWith(
      fontSize: 15,
      fontWeight: FontWeight.w600,
      letterSpacing: 0,
    );
    Widget tab(int index, String label) => Semantics(
      selected: destination == index,
      child: TextButton(
        onPressed: () => onSelect(index),
        style: TextButton.styleFrom(
          foregroundColor: destination == index
              ? colors.onSurface
              : colors.onSurfaceVariant,
          textStyle: labelStyle.copyWith(
            fontWeight: destination == index
                ? FontWeight.w700
                : FontWeight.w500,
          ),
          minimumSize: const Size(48, 60),
          padding: const EdgeInsets.symmetric(horizontal: 8),
          shape: const StadiumBorder(),
        ),
        child: Text(label, maxLines: 1),
      ),
    );
    Widget recommend() => Semantics(
      hint: 'Start recording a recommendation',
      child: FilledButton(
        onPressed: onRemember,
        style: FilledButton.styleFrom(
          backgroundColor: RekkyTheme.capture,
          foregroundColor: RekkyTheme.onCapture,
          textStyle: labelStyle.copyWith(fontWeight: FontWeight.w700),
          minimumSize: const Size(48, 60),
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          shape: const StadiumBorder(),
        ),
        child: const Text('Recommend', maxLines: 1),
      ),
    );
    return ColoredBox(
      color: colors.surface,
      child: SafeArea(
        top: false,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 4, 12, 4),
          child: Center(
            heightFactor: 1,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 560),
              child: LayoutBuilder(
                builder: (context, constraints) {
                  double measure(String text) {
                    final painter = TextPainter(
                      text: TextSpan(
                        text: text,
                        style: labelStyle.copyWith(fontWeight: FontWeight.w700),
                      ),
                      textDirection: Directionality.of(context),
                      textScaler: MediaQuery.textScalerOf(context),
                    )..layout();
                    final width = painter.width;
                    painter.dispose();
                    return width;
                  }

                  final sideWidth = (measure('Library') + 16).clamp(
                    constraints.maxWidth / 4,
                    double.infinity,
                  );
                  final centreWidth = measure('Recommend') + 32;
                  final fits =
                      sideWidth * 2 + centreWidth <= constraints.maxWidth;
                  return DecoratedBox(
                    decoration: ShapeDecoration(
                      color: colors.surfaceContainerLow,
                      shape: StadiumBorder(
                        side: BorderSide(
                          color: colors.outlineVariant.withValues(alpha: .7),
                        ),
                      ),
                    ),
                    child: fits
                        ? Row(
                            children: [
                              SizedBox(width: sideWidth, child: tab(0, 'Ask')),
                              Expanded(child: recommend()),
                              SizedBox(
                                width: sideWidth,
                                child: tab(1, 'Library'),
                              ),
                            ],
                          )
                        : Padding(
                            // Reflow only when the scaled labels cannot fit in one row.
                            padding: const EdgeInsets.all(8),
                            child: Column(
                              mainAxisSize: MainAxisSize.min,
                              crossAxisAlignment: CrossAxisAlignment.stretch,
                              children: [
                                recommend(),
                                Row(
                                  children: [
                                    Expanded(child: tab(0, 'Ask')),
                                    Expanded(child: tab(1, 'Library')),
                                  ],
                                ),
                              ],
                            ),
                          ),
                  );
                },
              ),
            ),
          ),
        ),
      ),
    );
  }
}
