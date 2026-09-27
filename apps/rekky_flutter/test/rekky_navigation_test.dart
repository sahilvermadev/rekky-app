import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/rekky_navigation.dart';
import 'package:rekky_flutter/rekky_theme.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    final fonts = FontLoader('Manrope');
    for (final weight in ['Medium', 'SemiBold', 'Bold']) {
      fonts.addFont(rootBundle.load('assets/fonts/Manrope-$weight.ttf'));
    }
    await fonts.load();
  });
  testWidgets('only destination changes tick; Recommend waits for microphone', (
    tester,
  ) async {
    final haptics = <String>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async {
        if (call.method == 'HapticFeedback.vibrate') {
          haptics.add(call.arguments as String);
        }
        return null;
      },
    );
    var destination = 1;
    await tester.pumpWidget(
      MaterialApp(
        home: StatefulBuilder(
          builder: (context, setState) => Scaffold(
            bottomNavigationBar: RekkyNavigation(
              destination: destination,
              onSelect: (value) => setState(() => destination = value),
              onRemember: () {},
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Library'));
    await tester.tap(find.text('Recommend'));
    expect(haptics, isEmpty);
    await tester.tap(find.text('Ask'));
    await tester.pumpAndSettle();
    expect(haptics, ['HapticFeedbackType.selectionClick']);
    await tester.tap(find.text('Ask'));
    expect(haptics.length, 1);
    await tester.tap(find.text('Library'));
    await tester.pumpAndSettle();
    expect(haptics.length, 2);
  });

  for (final width in [320.0, 375.0, 414.0, 768.0]) {
    for (final scale in [1.0, 2.0]) {
      testWidgets('navigation stays readable and one-tap at $width / $scale', (
        tester,
      ) async {
        tester.view.physicalSize = Size(width, 800);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        var records = 0;
        var selected = -1;
        await tester.pumpWidget(
          MaterialApp(
            theme: RekkyTheme.build(Brightness.light),
            builder: (context, child) => MediaQuery(
              data: MediaQuery.of(context)
                  .copyWith(textScaler: TextScaler.linear(scale)),
              child: child!,
            ),
            home: Scaffold(
              bottomNavigationBar: RekkyNavigation(
                destination: 1,
                onSelect: (i) => selected = i,
                onRemember: () => records++,
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        await tester.tap(find.text('Recommend'));
        expect(records, 1);
        await tester.tap(find.text('Ask'));
        expect(selected, 0);
        await tester.tap(find.text('Library'));
        expect(selected, 1);
        final action = tester.getRect(
          find.widgetWithText(FilledButton, 'Recommend'),
        );
        final ask = tester.getRect(find.widgetWithText(TextButton, 'Ask'));
        final library = tester.getRect(
          find.widgetWithText(TextButton, 'Library'),
        );
        expect(action.width, greaterThanOrEqualTo(48));
        expect(action.height, greaterThanOrEqualTo(60));
        expect(ask.height, greaterThanOrEqualTo(60));
        expect(library.height, greaterThanOrEqualTo(60));
        expect(action.left, greaterThanOrEqualTo(0));
        expect(action.right, lessThanOrEqualTo(width));
        // The additional height must be tappable, not decorative padding.
        await tester.tapAt(Offset(action.center.dx, action.top + 5));
        expect(records, 2);
        await tester.tapAt(
          Offset(tester.getCenter(find.text('Ask')).dx, ask.bottom - 5),
        );
        expect(selected, 0);
        await tester.tapAt(
          Offset(tester.getCenter(find.text('Library')).dx, library.top + 5),
        );
        expect(selected, 1);
        expect(find.byType(Icon), findsNothing);
        if (scale == 1) {
          expect(action.center.dy, closeTo(ask.center.dy, 1));
          expect(action.center.dy, closeTo(library.center.dy, 1));
          // The side buttons extend behind the centre's rounded ends.
          expect(action.left, lessThan(ask.right));
          expect(action.right, greaterThan(library.left));
          expect(action.height, lessThanOrEqualTo(64));
          await tester.tapAt(Offset(action.left + 2, action.top + 2));
          expect(selected, 0);
          await tester.tapAt(Offset(action.right - 2, action.top + 2));
          expect(selected, 1);
          await tester.tapAt(Offset(ask.left + 2, ask.top + 2));
          expect(selected, 1);
          expect(records, 2);
          if (width <= 414) {
            expect(ask.left, closeTo(12, 1));
            expect(library.right, closeTo(width - 12, 1));
          }
        }
        await tester.pumpAndSettle();
      });
    }
  }

  testWidgets(
    'side press animates the whole colour and respects reduced motion',
    (tester) async {
      Widget app({bool reduceMotion = false}) => MaterialApp(
        theme: RekkyTheme.build(Brightness.dark),
        builder: (context, child) => MediaQuery(
          data: MediaQuery.of(context)
              .copyWith(disableAnimations: reduceMotion),
          child: child!,
        ),
        home: Scaffold(
          bottomNavigationBar: RekkyNavigation(
            destination: 1,
            onSelect: (_) {},
            onRemember: () {},
          ),
        ),
      );

      await tester.pumpWidget(app());
      final side = find.byKey(const ValueKey('nav-side-1'));
      final before =
          (tester.widget<AnimatedContainer>(side).decoration! as BoxDecoration)
              .color;
      final press = await tester.startGesture(
        tester.getCenter(find.text('Library')),
      );
      await tester.pump();
      expect(
        (tester.widget<AnimatedContainer>(side).decoration! as BoxDecoration)
            .color,
        isNot(before),
      );
      expect(
        tester.widget<AnimatedContainer>(side).duration,
        const Duration(milliseconds: 220),
      );
      await press.up();
      await tester.pumpAndSettle();

      await tester.pumpWidget(app(reduceMotion: true));
      await tester.pumpAndSettle();
      expect(tester.widget<AnimatedContainer>(side).duration, Duration.zero);
      expect(
        tester.widget<AnimatedScale>(find.byType(AnimatedScale)).duration,
        Duration.zero,
      );
      final still = await tester.startGesture(
        tester.getCenter(find.text('Recommend')),
      );
      await tester.pump();
      expect(tester.widget<AnimatedScale>(find.byType(AnimatedScale)).scale, 1);
      await still.up();
      await tester.pumpAndSettle();
    },
  );
}
