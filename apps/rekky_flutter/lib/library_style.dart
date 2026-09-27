import 'package:flutter/material.dart';

import 'library_collection.dart';
import 'rekky_api.dart';
import 'rekky_theme.dart';

// Hallmark · curated Library · neutral surfaces and stable collection colours.
// Pre-emit critique: P5 H5 E4 S5 R5 V4.
abstract final class LibraryStyle {
  static const gutter = 20.0;
  static const gap = 12.0;
  static const radius = 16.0;
  static TextStyle heading(BuildContext context, double size) => TextStyle(
    fontFamily: 'Fraunces',
    fontWeight: FontWeight.w500,
    letterSpacing: -.5,
    fontSize: size,
    height: 1.15,
    color: Theme.of(context).colorScheme.onSurface,
  );
  static Color accent(BuildContext context, LibraryShelf shelf) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    return switch (shelf) {
      LibraryShelf.places => dark ? const Color(0xffadc3ff) : RekkyTheme.blue,
      LibraryShelf.people => dark ? const Color(0xffdbb1e5) : RekkyTheme.plum,
      LibraryShelf.things => dark ? const Color(0xff8bd7b9) : RekkyTheme.jade,
      LibraryShelf.activities =>
        dark ? const Color(0xffffbc92) : RekkyTheme.apricot,
      LibraryShelf.ideas => dark ? const Color(0xffe7cc7e) : RekkyTheme.gold,
      LibraryShelf.notes => Theme.of(context).colorScheme.onSurfaceVariant,
    };
  }

  static Color tint(BuildContext context, LibraryShelf shelf) =>
      Color.alphaBlend(
        accent(context, shelf).withValues(alpha: .12),
        Theme.of(context).colorScheme.surface,
      );

  // Only existing category metadata chooses an icon; this never reclassifies data.
  static IconData itemIcon(RekkyItem item) {
    final classification = item.recommendation?.classification;
    final labels = [
      item.recommendation?.categoryLabel ?? '',
      ...?classification?.types.map((t) => t.label),
    ].join(' ').toLowerCase();
    bool has(String pattern) => RegExp(pattern).hasMatch(labels);
    if (has(r'\b(bar|pub)\b')) return Icons.local_bar_outlined;
    if (has(r'\b(cafe|coffee|café)\b')) return Icons.local_cafe_outlined;
    if (has(r'\b(restaurant|dining|food|caterer|catering)\b')) {
      return Icons.restaurant_outlined;
    }
    if (has(r'\b(doctor|physician|dentist|clinic)\b')) {
      return Icons.medical_services_outlined;
    }
    if (has(r'\b(taxi|cab|driver|transport)\b')) {
      return Icons.local_taxi_outlined;
    }
    if (has(r'\b(climbing|bouldering|gym|fitness)\b')) {
      return Icons.fitness_center;
    }
    if (has(r'\b(book|novel)\b')) return Icons.menu_book_outlined;
    if (has(r'\b(hotel|stay|accommodation)\b')) return Icons.bed_outlined;
    if (has(r'\b(teacher|tutor|class|lesson)\b')) return Icons.school_outlined;
    return icon(LibraryShelf.of(item));
  }

  static IconData icon(LibraryShelf shelf) => switch (shelf) {
    LibraryShelf.places => Icons.storefront_outlined,
    LibraryShelf.people => Icons.handshake_outlined,
    LibraryShelf.things => Icons.book_outlined,
    LibraryShelf.activities => Icons.explore_outlined,
    LibraryShelf.ideas => Icons.lightbulb_outline,
    LibraryShelf.notes => Icons.notes_outlined,
  };
}
