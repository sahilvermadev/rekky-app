import 'package:flutter/material.dart';

import 'library_collection.dart';

// Hallmark · native Library · warm editorial collection, open rows and shelves.
// Pre-emit critique: P5 H4 E4 S5 R5 V4. Tokens follow the existing Rekky palette.
abstract final class LibraryStyle {
  static const gutter = 20.0;
  static const gap = 12.0;
  static const radius = 16.0;
  static TextStyle heading(BuildContext context, double size) => TextStyle(
    fontFamily: 'LibrarySerif',
    fontSize: size,
    height: 1.15,
    color: Theme.of(context).colorScheme.onSurface,
  );
  static Color tint(BuildContext context, LibraryShelf shelf) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    return switch (shelf) {
      LibraryShelf.places =>
        dark ? const Color(0xff3b382d) : const Color(0xffeee9da),
      LibraryShelf.people =>
        dark ? const Color(0xff293d36) : const Color(0xffe6eee7),
      LibraryShelf.things =>
        dark ? const Color(0xff3f3436) : const Color(0xfff1e5e1),
      LibraryShelf.activities =>
        dark ? const Color(0xff2b3b42) : const Color(0xffe4ecee),
      _ => dark ? const Color(0xff383342) : const Color(0xffeee8f0),
    };
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
