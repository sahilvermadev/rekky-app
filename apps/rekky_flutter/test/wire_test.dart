import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:rekky_flutter/main.dart';
import 'package:rekky_flutter/rekky_api.dart';

void main() {
  test('Dart consumes the shared v1 wire examples', () {
    final wire = jsonDecode(
      File('../../contracts/rekky/v1/fixtures/wire.json').readAsStringSync(),
    ) as Map<String, dynamic>;
    expect(wire['version'], 1);
    final examples = {
      for (final value in wire['examples'] as List<dynamic>)
        (value as Map<String, dynamic>)['name'] as String: value,
    };
    final saved = RekkyItem.fromJson(
      (examples['saved_item'] as Map<String, dynamic>)['body']['item']
          as Map<String, dynamic>,
    );
    final private = RekkyItem.fromJson(
      (examples['private_override_ack'] as Map<String, dynamic>)['body']['item']
          as Map<String, dynamic>,
    );
    expect(examples['remember_queued']!['status'], 202);
    expect(
      examples['remember_queued']!['body']['remember']['transcript_saved'],
      false,
    );
    expect(
      examples['remember_saved']!['body']['remember']['capture_status'],
      'completed',
    );
    expect(
      examples['remember_partial']!['body']['remember']['capture_status'],
      'partial',
    );
    expect(
      examples['remember_cancelled']!['body']['remember']['status'],
      'cancelled',
    );
    expect(saved.visibility, 'friends');
    expect(private.visibility, 'private');
    expect(private.revision, saved.revision + 1);
    expect(
      ((examples['text_retained_audio_deleted']
          as Map<String, dynamic>)['body']['capture']['source']['kind']),
      'transcript',
    );
    final source = RekkySource.fromJson(
      (examples['text_retained_audio_deleted']
              as Map<String, dynamic>)['body']['capture']['source']
          as Map<String, dynamic>,
    );
    expect(source.revision, 1);
    expect(
      ((examples['signed_out']
          as Map<String, dynamic>)['body']['error']['code']),
      'unauthorized',
    );
    expect(
      ((examples['ask_result'] as Map<String, dynamic>)['body']['results']
              as List<dynamic>)
          .length,
      1,
    );
    expect(
      (examples['processing_withdrawal_ack']
          as Map<String, dynamic>)['body']['processing']['acknowledged'],
      true,
    );
    expect(
      (examples['source_deleted_item_retained']
          as Map<String, dynamic>)['body']['capture']['source'],
      null,
    );
    expect(
      (examples['voice_transcript_ready']
          as Map<String, dynamic>)['body']['server_audio_retained'],
      false,
    );
    final extracted = RekkyItem.fromJson(
      ((examples['voice_knowledge_saved']
                      as Map<String, dynamic>)['body']['items']
                  as List<dynamic>)
              .single
          as Map<String, dynamic>,
    );
    expect(extracted.visibility, 'private');
    expect(extracted.captureId, '44444444-4444-4444-8444-444444444444');
  });

  test('the API client accepts saved-item and error wire examples', () async {
    final wire = jsonDecode(
      File('../../contracts/rekky/v1/fixtures/wire.json').readAsStringSync(),
    ) as Map<String, dynamic>;
    final examples = {
      for (final value in wire['examples'] as List<dynamic>)
        (value as Map<String, dynamic>)['name'] as String: value,
    };
    final client = MockClient((request) async {
      if (request.url.path == '/v1/items') {
        expect(request.headers['authorization'], 'Bearer session-token');
        expect(request.headers['idempotency-key'], 'request-key-123');
        return http.Response(
          jsonEncode((examples['saved_item'] as Map<String, dynamic>)['body']),
          201,
        );
      }
      return http.Response(
        jsonEncode((examples['signed_out'] as Map<String, dynamic>)['body']),
        401,
      );
    });
    final api = RekkyApi('https://api.example.test', client: client)
      ..token = 'session-token';
    final item = await api.save(
      'Raju',
      'Fixed our tap',
      'friends',
      'request-key-123',
    );
    expect(item.subject, 'Raju');
    await expectLater(
      api.me(),
      throwsA(
        isA<ApiFailure>().having((error) => error.code, 'code', 'unauthorized'),
      ),
    );
    await expectLater(
      RekkyApi('http://other-host.test', client: client).me(),
      throwsA(
        isA<ApiFailure>().having(
          (error) => error.code,
          'code',
          'configuration',
        ),
      ),
    );
  });

  testWidgets('signed-out shell requires identity before Ask or Library', (
    tester,
  ) async {
    FlutterSecureStorage.setMockInitialValues({});
    await tester.pumpWidget(const RekkyApp());
    await tester.pumpAndSettle();
    expect(find.text('Continue with Google'), findsOneWidget);
    expect(find.byType(NavigationBar), findsNothing);
  });

  test(
    'voice upload uses binary M4A and waits for a transcript acknowledgement',
    () async {
      final root = await Directory.systemTemp.createTemp('rekky-api-voice-');
      try {
        final file = File('${root.path}/voice.m4a');
        final audio = List<int>.filled(256, 0)
          ..setRange(4, 8, 'ftyp'.codeUnits);
        await file.writeAsBytes(audio);
        final client = MockClient((request) async {
          expect(
            request.url.path,
            '/v1/voice-drafts/draft-1234567890123456/transcribe',
          );
          expect(request.headers['authorization'], 'Bearer session-token');
          expect(request.headers['content-type'], 'audio/mp4');
          expect(request.headers['x-captured-at-ms'], '1234');
          expect(request.bodyBytes, audio);
          return http.Response(
            jsonEncode({
              'capture': {
                'id': '44444444-4444-4444-8444-444444444444',
                'status': 'transcript_ready',
                'transcript': 'Ravi fixed the kitchen tap.',
              },
              'server_audio_retained': false,
            }),
            201,
          );
        });
        final api = RekkyApi('https://api.example.test', client: client)
          ..token = 'session-token';
        expect(
          await api.transcribeVoice('draft-1234567890123456', 1234, file),
          '44444444-4444-4444-8444-444444444444',
        );
      } finally {
        await root.delete(recursive: true);
      }
    },
  );

  test(
    'private transcript can become a saved item without a second upload',
    () async {
      final client = MockClient((request) async {
        expect(request.headers['authorization'], 'Bearer session-token');
        if (request.url.path == '/v1/me/transcript-extraction-permission') {
          expect(request.method, 'POST');
          expect(jsonDecode(request.body)['disclosure_version'], 1);
          return http.Response('{}', 200);
        }
        expect(request.url.path, '/v1/voice-captures/capture-1/extract');
        expect(request.method, 'POST');
        expect(request.body, isEmpty);
        return http.Response(
          jsonEncode({
            'capture_id': 'capture-1',
            'items': [
              {'id': 'item-1', 'subject': 'Ravi', 'visibility': 'private'},
            ],
            'partial': false,
          }),
          200,
        );
      });
      final api = RekkyApi('https://api.example.test', client: client)
        ..token = 'session-token';
      await api.setExtractionPermission(true);
      final result = await api.extractVoiceCapture('capture-1');
      expect(result['items'][0]['visibility'], 'private');
    },
  );
}
