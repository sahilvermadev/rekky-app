import 'package:flutter/material.dart';

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
                ? colors.primary
                : colors.onSurfaceVariant,
            padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 4),
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
            ],
          ),
        ),
      ),
    );
    return DecoratedBox(
      decoration: BoxDecoration(
        color: colors.surface,
        border: Border(top: BorderSide(color: colors.outlineVariant)),
      ),
      child: SafeArea(
        top: false,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
          child: Row(
            children: [
              tab(0, 'Ask', Icons.search),
              Expanded(
                flex: 2,
                child: TextButton(
                  onPressed: onRemember,
                  style: TextButton.styleFrom(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 4,
                      vertical: 4,
                    ),
                  ),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Container(
                        width: 64,
                        height: 36,
                        decoration: BoxDecoration(
                          color: colors.primary,
                          borderRadius: BorderRadius.circular(14),
                        ),
                        child: Icon(
                          Icons.mic_none_rounded,
                          size: 22,
                          color: colors.onPrimary,
                        ),
                      ),
                      const SizedBox(height: 4),
                      const Text(
                        'Remember',
                        maxLines: 1,
                        style: TextStyle(
                          fontSize: 12,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ],
                  ),
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
      ),
    );
  }
}
