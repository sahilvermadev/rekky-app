import 'package:flutter/material.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/appearance.dart';
import 'package:rekky_flutter/rekky_theme.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() => FlutterSecureStorage.setMockInitialValues({}));

  test(
    'appearance defaults to System and restores each saved choice',
    () async {
      expect(await AppearancePreference.load(), ThemeMode.system);
      for (final mode in [ThemeMode.dark, ThemeMode.light, ThemeMode.system]) {
        await AppearancePreference.save(mode);
        expect(await AppearancePreference.load(), mode);
      }
    },
  );

  Future<void> openSheet(
    WidgetTester tester,
    Future<void> Function(ThemeMode) onChanged,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: RekkyTheme.build(Brightness.light),
        home: Builder(
          builder: (context) => Scaffold(
            body: TextButton(
              onPressed: () => showModalBottomSheet<void>(
                context: context,
                builder: (_) => AppearanceSheet(
                  mode: ThemeMode.system,
                  onChanged: onChanged,
                ),
              ),
              child: const Text('Open appearance'),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open appearance'));
    await tester.pumpAndSettle();
  }

  testWidgets('choosing Dark persists the choice and dismisses the sheet', (
    tester,
  ) async {
    await openSheet(tester, AppearancePreference.save);
    await tester.tap(find.text('Dark'));
    await tester.pumpAndSettle();
    expect(await AppearancePreference.load(), ThemeMode.dark);
    expect(find.byType(AppearanceSheet), findsNothing);
  });

  testWidgets('failed save keeps the sheet open with a retry', (tester) async {
    var attempts = 0;
    await openSheet(tester, (mode) async {
      attempts++;
      if (attempts == 1) throw StateError('storage unavailable');
      await AppearancePreference.save(mode);
    });
    await tester.tap(find.text('Light'));
    await tester.pumpAndSettle();
    expect(
      find.text('Couldn’t save your appearance. Try again.'),
      findsOneWidget,
    );
    expect(await AppearancePreference.load(), ThemeMode.system);
    await tester.tap(find.text('Light'));
    await tester.pumpAndSettle();
    expect(await AppearancePreference.load(), ThemeMode.light);
    expect(find.byType(AppearanceSheet), findsNothing);
  });
}
