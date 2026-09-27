import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/rekky_navigation.dart';
import 'package:rekky_flutter/rekky_theme.dart';

void main() {
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
        await tester.tap(find.text('Remember'));
        expect(records, 1);
        await tester.tap(find.text('Ask'));
        expect(selected, 0);
        await tester.tap(find.text('Library'));
        expect(selected, 1);
        expect(
          tester.getRect(find.text('Remember')).width,
          lessThan(width / 2),
        );
      });
    }
  }
}
