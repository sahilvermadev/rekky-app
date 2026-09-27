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
        expect(action.overlaps(ask), isFalse);
        expect(action.overlaps(library), isFalse);
        expect(action.left, greaterThanOrEqualTo(0));
        expect(action.right, lessThanOrEqualTo(width));
        // The additional height must be tappable, not decorative padding.
        await tester.tapAt(Offset(action.center.dx, action.top + 5));
        expect(records, 2);
        await tester.tapAt(Offset(ask.center.dx, ask.bottom - 5));
        expect(selected, 0);
        await tester.tapAt(Offset(library.center.dx, library.top + 5));
        expect(selected, 1);
        expect(find.byType(Icon), findsNothing);
        if (scale == 1) {
          expect(action.center.dy, closeTo(ask.center.dy, 1));
          expect(action.center.dy, closeTo(library.center.dy, 1));
          expect(action.left, greaterThanOrEqualTo(ask.right));
          expect(action.right, lessThanOrEqualTo(library.left));
          expect(action.height, lessThanOrEqualTo(64));
          if (width <= 414) {
            expect(ask.left, closeTo(12, 1));
            expect(library.right, closeTo(width - 12, 1));
          }
        }
      });
    }
  }
}
