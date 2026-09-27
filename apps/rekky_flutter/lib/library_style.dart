import 'package:flutter/material.dart';

import 'library_collection.dart';
import 'rekky_api.dart';
import 'rekky_theme.dart';

// Hallmark · curated Library · neutral reading surfaces, confident accents.
// Pre-emit critique: P5 H5 E5 S5 R5 V4.
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
      LibraryShelf.places =>
        dark ? RekkyTheme.placeOnDark : RekkyTheme.placeOnLight,
      LibraryShelf.people =>
        dark ? RekkyTheme.peopleOnDark : RekkyTheme.peopleOnLight,
      LibraryShelf.things =>
        dark ? RekkyTheme.thingsOnDark : RekkyTheme.thingsOnLight,
      LibraryShelf.activities =>
        dark ? RekkyTheme.activitiesOnDark : RekkyTheme.activitiesOnLight,
      LibraryShelf.ideas =>
        dark ? RekkyTheme.ideasOnDark : RekkyTheme.ideasOnLight,
      LibraryShelf.notes =>
        dark ? RekkyTheme.notesOnDark : RekkyTheme.notesOnLight,
    };
  }

  static Color itemFill(BuildContext context, RekkyItem item) =>
      accent(context, LibraryShelf.of(item));

  static Color itemForeground(BuildContext context) =>
      Theme.of(context).brightness == Brightness.dark
      ? RekkyTheme.ink
      : RekkyTheme.onNavAsk;

  static Color libraryAccent(BuildContext context) =>
      Theme.of(context).brightness == Brightness.dark
      ? RekkyTheme.navLibrary
      : RekkyTheme.libraryControlLight;

  static Color libraryTint(BuildContext context) => Color.alphaBlend(
    libraryAccent(context).withValues(alpha: .14),
    Theme.of(context).colorScheme.surfaceContainerLow,
  );

  static Color searchFocus(BuildContext context) =>
      Theme.of(context).brightness == Brightness.dark
      ? RekkyTheme.askFocusDark
      : RekkyTheme.navAsk;

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
