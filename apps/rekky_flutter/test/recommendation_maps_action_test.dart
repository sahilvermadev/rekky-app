import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/rekky_api.dart';
import 'package:rekky_flutter/recommendation_maps_action.dart';

RekkyItem place({
  String subject = 'Tea & Tales #2',
  String entityKind = 'place',
  List<RecommendationDetail> locations = const [
    RecommendationDetail('venue', 'पुणे'),
  ],
}) => RekkyItem(
  id: 'test-place',
  captureId: 'test-capture',
  subject: subject,
  body: 'Private opinions and referrer details must not leave in a URL.',
  visibility: 'private',
  revision: 1,
  createdAt: '',
  recommendation: RekkyRecommendation(
    summary: 'A private personal opinion.',
    shelf: 'Places',
    experience: 'firsthand',
    entityKind: entityKind,
    observations: [],
    locations: locations,
    useCases: [],
  ),
);

void main() {
  test('Maps search sends only encoded subject and venue, with a fixed destination', () {
    final item = place();
    final uri = recommendationMapsSearch(item)!;
    expect(uri.scheme, 'https');
    expect(uri.host, 'www.google.com');
    expect(uri.path, '/maps/search/');
    expect(uri.fragment, isEmpty);
    expect(uri.queryParameters, {'api': '1', 'query': 'Tea & Tales #2, पुणे'});
    expect(uri.toString(), isNot(contains('Private')));
    final hostile = recommendationMapsSearch(
      place(subject: 'https://evil.invalid/?api=2&query=x'),
    )!;
    expect(hostile.host, 'www.google.com');
    expect(hostile.queryParameters.length, 2);
    expect(hostile.queryParameters['api'], '1');
  });

  test('no invented venue from service areas, contextual cities or conflicting branches', () {
    for (final role in [
      'practice',
      'service_area',
      'past_experience',
      'context',
    ]) {
      expect(
        recommendationMapsSearch(
          place(locations: [RecommendationDetail(role, 'Pune')]),
        ),
        isNull,
      );
    }
    expect(
      recommendationMapsSearch(place(entityKind: 'person_service')),
      isNull,
    );
    expect(recommendationMapsSearch(place(locations: [])), isNull);
    expect(recommendationMapsSearch(place(subject: ' ')), isNull);
    expect(recommendationMapsSearch(place(subject: 'न' * 1000)), isNull);
    expect(
      recommendationMapsSearch(
        place(
          locations: const [
            RecommendationDetail('venue', 'Pune'),
            RecommendationDetail('venue', 'Mumbai'),
          ],
        ),
      ),
      isNull,
    );
  });

  testWidgets(
    'opens only on tap, suppresses double taps, and restores after success',
    (tester) async {
      final pending = Completer<bool>();
      var calls = 0;
      Uri? received;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: RecommendationMapsAction(
              item: place(),
              openUrl: (uri) {
                calls++;
                received = uri;
                return pending.future;
              },
            ),
          ),
        ),
      );
      expect(calls, 0);
      await tester.tap(find.text('Search Maps'));
      await tester.pump();
      await tester.tap(find.text('Opening Maps…'));
      expect(calls, 1);
      expect(received, recommendationMapsSearch(place()));
      pending.complete(true);
      await tester.pumpAndSettle();
      expect(find.text('Search Maps'), findsOneWidget);
      expect(find.textContaining('Couldn’t open'), findsNothing);
    },
  );

  for (final throws in [false, true]) {
    testWidgets(
      'launch failure is recoverable and does not expose queries: throws=$throws',
      (tester) async {
        var calls = 0;
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: RecommendationMapsAction(
                item: place(),
                openUrl: (_) async {
                  calls++;
                  if (calls > 1) return true;
                  if (throws) throw Exception('private provider query');
                  return false;
                },
              ),
            ),
          ),
        );
        await tester.tap(find.text('Search Maps'));
        await tester.pumpAndSettle();
        expect(find.text('Couldn’t open Maps. Try again.'), findsOneWidget);
        expect(find.textContaining('private provider query'), findsNothing);
        await tester.tap(find.text('Search Maps'));
        await tester.pumpAndSettle();
        expect(calls, 2);
        expect(find.textContaining('Couldn’t open'), findsNothing);
      },
    );
  }

  testWidgets('unsupported recommendations have no Maps action', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: RecommendationMapsAction(item: place(entityKind: 'idea_tip')),
        ),
      ),
    );
    expect(find.byType(FilledButton), findsNothing);
  });
}
