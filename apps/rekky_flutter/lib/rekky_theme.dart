import 'package:flutter/material.dart';

// Hallmark · curated native collection · neutral canvas, purposeful colour.
// Pre-emit critique: P5 H5 E4 S5 R5 V4. Shared tokens for both brightness modes.
abstract final class RekkyTheme {
  static const paper = Color(0xfff7f7f5);
  static const ink = Color(0xff202020);
  static const charcoal = Color(0xff0c0c0c);
  static const coral = Color(0xffac452e);
  static const plum = Color(0xff854886);
  static const jade = Color(0xff227057);
  static const citron = Color(0xff686719);
  static const gold = Color(0xff89650c);
  static const capture = Color(0xfff34f47);
  static const onCapture = Color(0xff17110f);
  static const navAsk = Color(0xff2856d8);
  static const onNavAsk = Color(0xffffffff);
  static const navLibrary = Color(0xffffd43b);
  static const onNavLibrary = Color(0xff1b170a);
  static const coralLight = Color(0xffffa58c);
  static const plumLight = Color(0xffdda6db);
  static const jadeLight = Color(0xff9bd4b8);
  static const citronLight = Color(0xffdcdf85);
  static const goldLight = Color(0xffe7cc7e);

  static ThemeData build(Brightness brightness) {
    final dark = brightness == Brightness.dark;
    // Neutral material roles keep menus, fields and selection free of colour casts.
    final colors = ColorScheme.fromSeed(seedColor: ink, brightness: brightness)
        .copyWith(
          surface: dark ? charcoal : paper,
          onSurface: dark ? const Color(0xfff2f2f0) : ink,
          onSurfaceVariant: dark
              ? const Color(0xffb9b9b5)
              : const Color(0xff62625e),
          surfaceContainerLowest: dark
              ? const Color(0xff080808)
              : const Color(0xffffffff),
          surfaceContainerLow: dark
              ? const Color(0xff1b1b1b)
              : const Color(0xffffffff),
          surfaceContainer: dark
              ? const Color(0xff232323)
              : const Color(0xffededea),
          surfaceContainerHigh: dark
              ? const Color(0xff2b2b2b)
              : const Color(0xffe6e6e2),
          surfaceContainerHighest: dark
              ? const Color(0xff343434)
              : const Color(0xffdededa),
          primary: dark ? const Color(0xfff2f2f0) : ink,
          onPrimary: dark ? ink : const Color(0xffffffff),
          primaryContainer: dark
              ? const Color(0xff353535)
              : const Color(0xffe6e6e2),
          onPrimaryContainer: dark ? const Color(0xfff2f2f0) : ink,
          secondary: dark ? const Color(0xffc8c8c4) : const Color(0xff565652),
          onSecondary: dark ? ink : const Color(0xffffffff),
          secondaryContainer: dark
              ? const Color(0xff2b2b2b)
              : const Color(0xffe6e6e2),
          onSecondaryContainer: dark ? const Color(0xfff2f2f0) : ink,
          tertiary: dark ? const Color(0xffc8c8c4) : const Color(0xff565652),
          onTertiary: dark ? ink : const Color(0xffffffff),
          tertiaryContainer: dark
              ? const Color(0xff2b2b2b)
              : const Color(0xffe6e6e2),
          onTertiaryContainer: dark ? const Color(0xfff2f2f0) : ink,
          inverseSurface: dark
              ? const Color(0xffededeb)
              : const Color(0xff252525),
          onInverseSurface: dark ? ink : const Color(0xfff2f2f0),
          inversePrimary: dark ? ink : const Color(0xfff2f2f0),
          outline: dark ? const Color(0xff898985) : const Color(0xff787874),
          outlineVariant: dark
              ? const Color(0xff363636)
              : const Color(0xffdeded9),
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
