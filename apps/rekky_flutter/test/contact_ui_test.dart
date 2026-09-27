import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/contact_action.dart';
import 'package:rekky_flutter/contact_sheet.dart';

import 'contact_matching_test.dart' show item, Book;

void main() {
  testWidgets(
    'manual contact works with permission denied; validates before saving',
    (tester) async {
      final book = Book()..allowed = false;
      Map<String, dynamic>? sent;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ContactSheet(
              item: item(),
              book: book,
              enableMatching: () async => null,
              save: (value) async {
                sent = value;
                return item();
              },
            ),
          ),
        ),
      );
      expect(
        find.text('Friends can see and use the attached number.'),
        findsOneWidget,
      );
      await tester.enterText(find.byType(TextField), '9876543210');
      await tester.tap(find.text('Save'));
      await tester.pump();
      expect(sent, isNull);
      expect(find.textContaining('Include +'), findsOneWidget);
      await tester.enterText(find.byType(TextField), '+1 (202) 555-0123');
      await tester.tap(find.text('Save'));
      await tester.pump();
      expect(sent, {'mode': 'set', 'phone': '+12025550123'});
      expect(book.reads, 0);
    },
  );
  testWidgets('clear match completes using the current permission generation', (
    tester,
  ) async {
    Map<String, dynamic>? sent;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ContactSheet(
            item: item(),
            book: Book(),
            enableMatching: () async => 7,
            save: (value) async {
              sent = value;
              return item();
            },
          ),
        ),
      ),
    );
    await tester.tap(find.text('Find a saved contact'));
    await tester.pumpAndSettle();
    expect(sent, {
      'mode': 'automatic',
      'phone': '+919876543210',
      'generation': 7,
    });
  });
  testWidgets('removal is explicit and sends no phone', (tester) async {
    Map<String, dynamic>? sent;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: ContactSheet(
            item: item(phone: '+12025550123'),
            book: Book(),
            enableMatching: () async => null,
            save: (value) async {
              sent = value;
              return item();
            },
          ),
        ),
      ),
    );
    await tester.tap(find.text('Remove number'));
    await tester.pump();
    expect(sent, {'mode': 'none'});
  });
  testWidgets('contact action wraps at narrow width and large text', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(320, 640);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: MediaQuery(
            data: const MediaQueryData(textScaler: TextScaler.linear(2)),
            child: ContactAction(phone: '+12025550123', onManage: () {}),
          ),
        ),
      ),
    );
    expect(find.text('Call'), findsOneWidget);
    expect(find.byTooltip('Change contact'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
