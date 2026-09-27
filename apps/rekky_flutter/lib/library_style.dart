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
      LibraryShelf.places => dark ? RekkyTheme.coralLight : RekkyTheme.coral,
      LibraryShelf.people => dark ? RekkyTheme.plumLight : RekkyTheme.plum,
      LibraryShelf.things => dark ? RekkyTheme.jadeLight : RekkyTheme.jade,
      LibraryShelf.activities =>
        dark ? RekkyTheme.citronLight : RekkyTheme.citron,
      LibraryShelf.ideas => dark ? RekkyTheme.goldLight : RekkyTheme.gold,
      LibraryShelf.notes => Theme.of(context).colorScheme.onSurfaceVariant,
    };
  }

  static Color tint(BuildContext context, LibraryShelf shelf) =>
      Color.alphaBlend(
        accent(context, shelf).withValues(alpha: .12),
        Theme.of(context).colorScheme.surface,
      );

  static Color itemAccent(BuildContext context, RekkyItem item) {
    final glyph = itemIcon(item);
    final palette = switch (glyph) {
      Icons.local_bar_outlined || Icons.bed_outlined => LibraryShelf.people,
      Icons.local_cafe_outlined => LibraryShelf.ideas,
      Icons.fitness_center => LibraryShelf.things,
      _ => LibraryShelf.of(item),
    };
    return accent(context, palette);
  }

  static Color itemTint(BuildContext context, RekkyItem item) =>
      Color.alphaBlend(
        itemAccent(context, item).withValues(alpha: .12),
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
