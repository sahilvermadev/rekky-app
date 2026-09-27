import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/ask_screen.dart';
import 'package:rekky_flutter/rekky_theme.dart';

void main() {
  testWidgets(
    'Ask remains scrollable with large text and keyboard; results open saved item',
    (tester) async {
      tester.view.physicalSize = const Size(320, 640);
      tester.view.devicePixelRatio = 1;
      tester.view.viewInsets = FakeViewPadding(bottom: 280);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetViewInsets);
      final controller = TextEditingController();
      addTearDown(controller.dispose);
      var searches = 0;
      String? opened;
      await tester.pumpWidget(
        MaterialApp(
          theme: RekkyTheme.build(Brightness.light),
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(2)),
            child: child!,
          ),
          home: Scaffold(
            body: AskScreen(
              controller: controller,
              searching: false,
              matches: const [
                {
                  'item_id': 'one',
                  'subject': 'A saved restaurant',
                  'body': 'A useful experience with a long detailed account.',
                },
              ],
              onAsk: () => searches++,
              onOpen: (id) => opened = id,
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.ensureVisible(find.byType(TextField));
      await tester.enterText(find.byType(TextField), 'restaurant');
      await tester.testTextInput.receiveAction(TextInputAction.search);
      expect(searches, 1);
      await tester.scrollUntilVisible(
        find.text('A saved restaurant'),
        150,
        scrollable: find
            .descendant(
              of: find.byType(CustomScrollView),
              matching: find.byType(Scrollable),
            )
            .first,
      );
      await tester.tap(find.text('A saved restaurant'));
      expect(opened, 'one');
      expect(tester.takeException(), isNull);
    },
  );
}
