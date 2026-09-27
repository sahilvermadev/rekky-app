import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/library_collection.dart';
import 'package:rekky_flutter/library_screen.dart';
import 'package:rekky_flutter/rekky_api.dart';

RekkyItem entry(
  String id,
  String name, {
  String kind = 'place',
  String date = '2026-09-27T12:00:00Z',
  String role = 'venue',
  String area = 'Delhi',
  String? phone,
  bool pinned = false,
  bool resolved = true,
  String type = 'place.restaurant',
  bool caution = false,
  Map<String, dynamic>? geography,
}) => RekkyItem(
  id: id,
  captureId: 'capture-$id',
  subject: name,
  body: 'The handmade pasta was excellent.',
  visibility: 'private',
  revision: 3,
  createdAt: date,
  pinned: pinned,
  recommendation: RekkyRecommendation(
    summary: 'The handmade pasta was excellent.',
    shelf: 'Places',
    entityKind: kind,
    experience: 'firsthand',
    observations: caution
        ? [const RecommendationDetail('caution', 'Small portions.')]
        : [],
    locations: [
      RecommendationDetail(
        role,
        area,
        geography:
            geography ??
            (resolved
                ? {
                    'status': 'resolved',
                    'area_id': 'geo:$area',
                    'label': area,
                    'filter_ids': ['geo:$area', 'geo:India'],
                    'hierarchy': [
                      {'id': 'geo:India', 'name': 'India', 'kind': 'country'},
                    ],
                  }
                : {'status': 'unresolved'}),
      ),
    ],
    useCases: [],
    contactPhone: phone,
    rating: const RecommendationRating(8, 'inferred'),
    classification: RekkyClassification(
      types: [
        CategoryConcept(
          id: type,
          label: kind == 'place' ? 'Italian restaurant' : 'Taxi service',
          dimension: 'type',
        ),
      ],
      facets: [],
      descriptors: [],
      displayLabel: kind == 'place' ? 'Italian restaurant' : 'Taxi service',
    ),
  ),
);

final boundary = GlobalKey();
Widget app(
  List<RekkyItem> items, {
  double scale = 1,
  bool dark = false,
  ValueChanged<RekkyItem>? onOpen,
  Future<RekkyItem> Function(RekkyItem, bool)? onPin,
  Future<bool> Function(Uri)? openUrl,
  VoidCallback? onRemember,
}) => MaterialApp(
  theme: ThemeData(
    useMaterial3: true,
    colorScheme: ColorScheme.fromSeed(
      seedColor: const Color(0xFF74452F),
      surface: dark ? const Color(0xff171410) : const Color(0xFFFFFBF5),
      brightness: dark ? Brightness.dark : Brightness.light,
    ),
  ),
  builder: (context, child) => MediaQuery(
    data: MediaQuery.of(context).copyWith(textScaler: TextScaler.linear(scale)),
    child: child!,
  ),
  home: Scaffold(
    body: RepaintBoundary(
      key: boundary,
      child: ColoredBox(
        color: dark ? const Color(0xff171410) : const Color(0xFFFFFBF5),
        child: LibraryScreen(
          items: items,
          onOpen: onOpen ?? (_) {},
          onRefresh: () async {},
          onRemember: onRemember ?? () {},
          onPin:
              onPin ??
              (item, value) async => item.withPin(value, item.pinRevision + 1),
          openUrl: openUrl ?? (_) async => true,
        ),
      ),
    ),
  ),
);

void main() {
  setUpAll(() async {
    if (Platform.environment['LIBRARY_SCREENSHOTS'] == 'true') {
      TestWidgetsFlutterBinding.ensureInitialized();
      await (FontLoader('LibrarySerif')
            ..addFont(rootBundle.load('assets/fonts/LibrarySerif-Regular.ttf')))
          .load();
      await (FontLoader('MaterialIcons')..addFont(
            File(
              '../../work/flutter-sdk/bin/cache/artifacts/material_fonts/MaterialIcons-Regular.otf',
            ).readAsBytes().then((b) => ByteData.sublistView(b)),
          ))
          .load();
      await (FontLoader('Roboto')..addFont(
            File(
              '../../work/flutter-sdk/bin/cache/artifacts/material_fonts/Roboto-Regular.ttf',
            ).readAsBytes().then((b) => ByteData.sublistView(b)),
          ))
          .load();
    }
  });
  test('filters use identity and roles, preserve unresolved and search useful content', () {
    final items = [
      entry('1', 'Lantern'),
      entry('2', 'Ravi', kind: 'person_service', role: 'past_experience'),
      entry('3', 'Mira', kind: 'person_service', role: 'service_area'),
      entry('4', 'Unresolved', resolved: false),
      entry('5', 'Elsewhere', area: 'Pune'),
    ];
    expect(librarySelection(items, areaId: 'geo:Delhi').map((i) => i.id), [
      '3',
      '1',
    ]);
    expect(librarySelection(items, areaId: 'unresolved').map((i) => i.id), [
      '4',
      '2',
    ]);
    expect(librarySelection(items, query: 'handmade pasta').length, 5);
    expect(librarySelection(items, query: 'unmentioned').isEmpty, isTrue);
    expect(librarySelection(items, shelf: LibraryShelf.people).length, 2);
    expect(librarySelection(items).length, 5);
    expect(libraryAreas(items).where((a) => a.label == 'Delhi').length, 1);
  });

  test('location choices disambiguate administrative areas and omit unsupported parents', () {
    final areas = libraryAreas([
      entry(
        '1',
        'Service',
        geography: {
          'status': 'resolved',
          'area_id': 'local',
          'label': 'Landour',
          'filter_ids': ['local', 'district', 'subdistrict'],
          'hierarchy': [
            {'id': 'district', 'name': 'Dehradun', 'kind': 'ADM2'},
            {'id': 'subdistrict', 'name': 'Dehradun', 'kind': 'ADM3'},
            {'id': 'unsupported', 'name': 'Elsewhere', 'kind': 'PPLA'},
          ],
        },
      ),
    ]);
    expect(
      areas.map((a) => a.label),
      containsAll(['Landour', 'Dehradun · District', 'Dehradun · Subdistrict']),
    );
    expect(areas.any((a) => a.id == 'unsupported'), isFalse);
  });

  testWidgets('empty Library invites recording without blank categories', (
    tester,
  ) async {
    var recorded = false;
    await tester.pumpWidget(app([], onRemember: () => recorded = true));
    expect(find.text('Good things start here.'), findsOneWidget);
    expect(find.text('Places'), findsNothing);
    await tester.tap(find.text('Remember'));
    expect(recorded, isTrue);
  });

  testWidgets(
    'search, locations, pins and recent preserve their scoped meaning',
    (tester) async {
      final items = [
        entry('1', 'Lantern', pinned: true),
        entry('2', 'Fern', area: 'Pune', date: '2026-08-03T12:00:00Z'),
      ];
      await tester.pumpWidget(app(items));
      expect(find.text('The handmade pasta was excellent.'), findsNothing);
      await tester.enterText(find.byType(TextField), 'Fern');
      await tester.pump();
      expect(find.text('Lantern'), findsNothing);
      expect(
        find.byWidgetPredicate((w) => w is Text && w.data == 'Fern'),
        findsOneWidget,
      );
      await tester.tap(find.byTooltip('Clear search'));
      await tester.pump();
      await tester.tap(find.text('All locations'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Delhi').last);
      await tester.pumpAndSettle();
      expect(find.text('Lantern'), findsOneWidget);
      expect(find.text('Fern'), findsNothing);
      await tester.tap(find.text('Delhi').first);
      await tester.pumpAndSettle();
      await tester.tap(find.text('All locations'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Pins'));
      await tester.pump();
      expect(find.text('Fern'), findsNothing);
      await tester.tap(find.text('Recent'));
      await tester.pump();
      expect(find.text('Saved in September 2026'), findsOneWidget);
      expect(find.text('Saved in August 2026'), findsOneWidget);
    },
  );

  testWidgets('pins wait for acknowledgement and a failure stays visible', (
    tester,
  ) async {
    final result = Completer<RekkyItem>();
    await tester.pumpWidget(
      app(
        [entry('1', 'Lantern')],
        onPin: (_, value) {
          expect(value, isTrue);
          return result.future;
        },
      ),
    );
    await tester.longPress(find.text('Lantern'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Pin in Library'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.bySemanticsLabel('Pinned'), findsNothing);
    result.completeError(StateError('offline'));
    await tester.pumpAndSettle();
    expect(find.text('Couldn’t save the pin. Try again.'), findsOneWidget);
  });

  testWidgets(
    'Maps is explicit, does not open detail, and handles launcher failure',
    (tester) async {
      var details = 0;
      final opened = <Uri>[];
      await tester.pumpWidget(
        app(
          [entry('1', 'Lantern')],
          onOpen: (_) => details++,
          openUrl: (uri) async {
            opened.add(uri);
            return false;
          },
        ),
      );
      expect(opened, isEmpty);
      await tester.tap(find.byTooltip('Search Maps for Lantern'));
      await tester.pumpAndSettle();
      expect(opened.single.queryParameters['query'], 'Lantern, Delhi');
      expect(details, 0);
      expect(
        find.text('Couldn’t open this action. Try again.'),
        findsOneWidget,
      );
    },
  );

  testWidgets('large collection exposes all items through See all', (
    tester,
  ) async {
    await tester.pumpWidget(
      app(List.generate(18, (i) => entry('$i', 'Place $i'))),
    );
    await tester.tap(find.text('See all (18)'));
    await tester.pumpAndSettle();
    await tester.scrollUntilVisible(
      find.text('Place 0'),
      300,
      scrollable: find.byType(Scrollable).first,
    );
    expect(find.text('Place 0'), findsOneWidget);
  });

  for (final width in [320.0, 375.0, 414.0, 768.0]) {
    for (final scale in [1.0, 2.0]) {
      testWidgets('Library layout $width at $scale text scale', (tester) async {
        tester.view.physicalSize = Size(width, 1000);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        await tester.pumpWidget(
          app([
            entry('1', 'The Paper Lantern Cafe', caution: true),
            entry('2', 'Bramble Kitchen', area: 'Connaught Place, Delhi'),
            entry(
              '3',
              'Ravi Taxi Service',
              kind: 'person_service',
              role: 'service_area',
              phone: '+919876543210',
            ),
            entry(
              '4',
              'डॉ. मीरा शर्मा — आपकी पारिवारिक चिकित्सक',
              kind: 'person_service',
              role: 'practice',
            ),
          ], scale: scale),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        if (Platform.environment['LIBRARY_SCREENSHOTS'] == 'true' &&
            scale == 1 &&
            width == 375) {
          await tester.runAsync(() async {
            final render =
                boundary.currentContext!.findRenderObject()!
                    as RenderRepaintBoundary;
            final image = await render.toImage(pixelRatio: 2);
            final bytes = await image.toByteData(
              format: ui.ImageByteFormat.png,
            );
            await File('/tmp/rekky-library-preview.png')
                .writeAsBytes(bytes!.buffer.asUint8List());
          });
        }
        await tester.tap(find.text('All locations'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
      });
    }
  }
  testWidgets('location chooser remains usable with keyboard and large text', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(320, 640);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetViewInsets);
    await tester.pumpWidget(
      app(
        List.generate(12, (i) => entry('$i', 'Place $i', area: 'Area $i')),
        scale: 2,
      ),
    );
    await tester.ensureVisible(find.text('All locations'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('All locations'));
    await tester.pumpAndSettle();
    expect(find.text('Saved locations'), findsOneWidget);
    tester.view.viewInsets = FakeViewPadding(bottom: 280);
    await tester.pumpAndSettle();
    await tester.enterText(
      find.widgetWithText(TextField, 'Find a location'),
      'Area 7',
    );
    await tester.pump();
    expect(tester.takeException(), isNull);
    final choice = find.byWidgetPredicate(
      (w) => w is Text && w.data == 'Area 7',
    );
    await tester.scrollUntilVisible(
      choice,
      100,
      scrollable: find
          .descendant(
            of: find.byType(CustomScrollView).last,
            matching: find.byType(Scrollable),
          )
          .first,
    );
    await tester.pumpAndSettle();
    await tester.tap(choice);
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('Saved locations'), findsNothing);
    expect(find.text('Area 7'), findsOneWidget);
  });
  testWidgets('dark Library has no layout errors', (tester) async {
    await tester.pumpWidget(app([entry('1', 'Lantern')], dark: true));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });
}
