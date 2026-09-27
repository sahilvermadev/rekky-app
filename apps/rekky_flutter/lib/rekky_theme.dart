import 'package:flutter/material.dart';

// Hallmark · curated native collection · neutral canvas, purposeful colour.
// Pre-emit critique: P5 H5 E4 S5 R5 V4. Shared tokens for both brightness modes.
abstract final class RekkyTheme {
  static const paper = Color(0xfff6f7f9);
  static const ink = Color(0xff222832);
  static const charcoal = Color(0xff15181d);
  static const blue = Color(0xff355bbb);
  static const plum = Color(0xff854b91);
  static const jade = Color(0xff24765f);
  static const apricot = Color(0xffa55125);
  static const gold = Color(0xff89650c);

  static ThemeData build(Brightness brightness) {
    final dark = brightness == Brightness.dark;
    final colors = ColorScheme.fromSeed(seedColor: blue, brightness: brightness)
        .copyWith(
          surface: dark ? charcoal : paper,
          onSurface: dark ? const Color(0xffedf0f5) : ink,
          onSurfaceVariant: dark
              ? const Color(0xffb6becb)
              : const Color(0xff596271),
          surfaceContainerLowest: dark
              ? const Color(0xff12151a)
              : const Color(0xffffffff),
          surfaceContainerLow: dark
              ? const Color(0xff20242b)
              : const Color(0xffffffff),
          surfaceContainer: dark
              ? const Color(0xff272d37)
              : const Color(0xffe8ebf1),
          surfaceContainerHigh: dark
              ? const Color(0xff303846)
              : const Color(0xffe2e6ed),
          surfaceContainerHighest: dark
              ? const Color(0xff394250)
              : const Color(0xffdce1ea),
          primary: dark ? const Color(0xffadc3ff) : blue,
          onPrimary: dark ? const Color(0xff172d61) : const Color(0xffffffff),
          primaryContainer: dark
              ? const Color(0xff293b65)
              : const Color(0xffe1e9ff),
          onPrimaryContainer: dark
              ? const Color(0xffdce6ff)
              : const Color(0xff253f7c),
          secondaryContainer: dark
              ? const Color(0xff303846)
              : const Color(0xffe5e9f0),
          onSecondaryContainer: dark ? const Color(0xffedf0f5) : ink,
          outline: dark ? const Color(0xff818c9e) : const Color(0xff727e90),
          outlineVariant: dark
              ? const Color(0xff39414e)
              : const Color(0xffdce1e8),
          surfaceTint: Colors.transparent,
        );
    return ThemeData(
      useMaterial3: true,
      fontFamily: 'Manrope',
      textTheme: TextTheme(
        headlineLarge: TextStyle(
          fontFamily: 'Fraunces',
          fontWeight: FontWeight.w500,
          fontSize: 32,
          height: 1.12,
          letterSpacing: -.5,
          color: colors.onSurface,
        ),
        headlineMedium: TextStyle(
          fontFamily: 'Fraunces',
          fontWeight: FontWeight.w500,
          fontSize: 30,
          height: 1.12,
          letterSpacing: -.5,
          color: colors.onSurface,
        ),
        headlineSmall: TextStyle(
          fontFamily: 'Fraunces',
          fontWeight: FontWeight.w500,
          fontSize: 26,
          height: 1.2,
          color: colors.onSurface,
        ),
        titleMedium: const TextStyle(
          fontSize: 17,
          fontWeight: FontWeight.w600,
          height: 1.25,
          letterSpacing: -.2,
        ),
        bodyLarge: const TextStyle(fontSize: 16, height: 1.55),
        bodyMedium: const TextStyle(fontSize: 14, height: 1.45),
        bodySmall: const TextStyle(fontSize: 13, height: 1.4),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: colors.surfaceContainerLow,
        contentPadding: const EdgeInsets.symmetric(
          horizontal: 16,
          vertical: 14,
        ),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: BorderSide(color: colors.outlineVariant),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: BorderSide(
            color: colors.outlineVariant.withValues(alpha: .5),
          ),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(12),
          borderSide: BorderSide(color: colors.primary, width: 2),
        ),
      ),
      colorScheme: colors,
      scaffoldBackgroundColor: colors.surface,
      appBarTheme: AppBarTheme(
        backgroundColor: colors.surface,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
      ),
      bottomSheetTheme: BottomSheetThemeData(
        backgroundColor: colors.surface,
        surfaceTintColor: Colors.transparent,
      ),
      dividerTheme: DividerThemeData(
        color: colors.outlineVariant,
        thickness: 1,
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(16),
          ),
        ),
      ),
    );
  }
}
