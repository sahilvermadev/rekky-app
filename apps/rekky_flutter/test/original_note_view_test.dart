import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/original_note_view.dart';
import 'package:rekky_flutter/rekky_api.dart';

void main() {
  final source = RekkySource.fromJson(
    jsonDecode(
      File('../../contracts/rekky/v1/fixtures/readable_source.json')
          .readAsStringSync(),
    ) as Map<String, dynamic>,
  );
  Future<void> show(
    WidgetTester tester,
    RekkySource value, {
    double scale = 1,
  }) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: MediaQuery(
            data: MediaQueryData(textScaler: TextScaler.linear(scale)),
            child: SingleChildScrollView(
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: OriginalNoteView(source: value),
              ),
            ),
          ),
        ),
      ),
    );
  }

  testWidgets('readable quote toggles to exact raw transcript and back', (
    tester,
  ) async {
    await show(tester, source);
    expect(find.text('Lightly edited · your words'), findsNothing);
    expect(find.text(source.readableText!), findsOneWidget);
    expect(find.text('“'), findsOneWidget);
    expect(find.text('”'), findsOneWidget);
    await tester.tap(find.text('Original transcript'));
    await tester.pump();
    expect(find.text(source.text), findsOneWidget);
    expect(find.text('Raw transcript'), findsNothing);
    await tester.tap(find.text('Back to note'));
    await tester.pump();
    expect(find.text(source.readableText!), findsOneWidget);
  });
  testWidgets('legacy and typed sources do not pretend to be edited', (
    tester,
  ) async {
    await show(
      tester,
      const RekkySource(text: 'Unchanged.', kind: 'transcript', revision: 1),
    );
    expect(find.text('Raw transcript'), findsNothing);
    expect(find.byType(TextButton), findsNothing);
    await show(
      tester,
      const RekkySource(text: 'Typed.', kind: 'typed', revision: 1),
    );
    expect(find.text('Typed.'), findsOneWidget);
  });
  for (final width in [320.0, 375.0, 414.0, 768.0]) {
    testWidgets('quote fits width $width at double text scale', (tester) async {
      tester.view.physicalSize = Size(width, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await show(tester, source, scale: 2);
      expect(tester.takeException(), isNull);
    });
  }
}
