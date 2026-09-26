import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;

class ApiFailure implements Exception {
  const ApiFailure(this.code, this.message, this.status);
  final String code;
  final String message;
  final int status;
  @override
  String toString() => message;
}

class RekkyItem {
  const RekkyItem({
    required this.id,
    required this.captureId,
    required this.subject,
    required this.body,
    required this.visibility,
    required this.revision,
    required this.createdAt,
  });
  final String id, captureId, subject, body, visibility, createdAt;
  final int revision;
  factory RekkyItem.fromJson(Map<String, dynamic> json) => RekkyItem(
    id: json['id'] as String,
    captureId: json['capture_id'] as String,
    subject: json['subject'] as String,
    body: json['body'] as String,
    visibility: json['visibility'] as String,
    revision: json['revision'] as int,
    createdAt: json['created_at'] as String,
  );
}

class RekkySource {
  const RekkySource({
    required this.text,
    required this.kind,
    required this.revision,
  });
  final String text, kind;
  final int revision;
  factory RekkySource.fromJson(Map<String, dynamic> json) => RekkySource(
    text: json['text'] as String,
    kind: json['kind'] as String,
    revision: json['revision'] as int,
  );
}

class VoiceCapture {
  const VoiceCapture({
    required this.id,
    required this.transcript,
    required this.sourceRevision,
    required this.createdAt,
    required this.itemCount,
    required this.extractionStatus,
    required this.partial,
  });
  final String id, transcript, createdAt;
  final int sourceRevision;
  final int itemCount;
  final String? extractionStatus;
  final bool? partial;
  factory VoiceCapture.fromJson(Map<String, dynamic> json) => VoiceCapture(
    id: json['id'] as String,
    transcript: json['transcript'] as String,
    sourceRevision: json['source_revision'] as int,
    createdAt: json['created_at'] as String,
    itemCount: json['item_count'] as int? ?? 0,
    extractionStatus: json['extraction_status'] as String?,
    partial: json['partial'] as bool?,
  );
}

class RekkyApi {
  RekkyApi(this.baseUrl, {http.Client? client})
    : _client = client ?? http.Client();
  final String baseUrl;
  final http.Client _client;
  String? token;

  Uri _uri(String path) {
    if (baseUrl.isEmpty) {
      throw const ApiFailure(
        'configuration',
        'Set API_BASE_URL to your Rekky backend.',
        0,
      );
    }
    final uri = Uri.parse('$baseUrl$path');
    if (uri.scheme != 'https' &&
        !(kDebugMode &&
            uri.scheme == 'http' &&
            ['10.0.2.2', '127.0.0.1', 'localhost'].contains(uri.host))) {
      throw const ApiFailure(
        'configuration',
        'Use an HTTPS Rekky backend URL.',
        0,
      );
    }
    return uri;
  }

  Map<String, dynamic> _decode(http.Response response) {
    if (response.statusCode == 204) return {};
    final decoded = jsonDecode(response.body) as Map<String, dynamic>;
    if (response.statusCode >= 400) {
      final error = decoded['error'] as Map<String, dynamic>? ?? {};
      throw ApiFailure(
        error['code'] as String? ?? 'request_failed',
        error['message'] as String? ?? 'Request failed',
        response.statusCode,
      );
    }
    return decoded;
  }

  Future<Map<String, dynamic>> request(
    String method,
    String path, {
    Object? body,
    Map<String, String> headers = const {},
    Duration timeout = const Duration(seconds: 20),
  }) async {
    final request = http.Request(method, _uri(path));
    request.headers.addAll({
      'accept': 'application/json',
      if (body != null) 'content-type': 'application/json',
      if (token != null) 'authorization': 'Bearer $token',
      ...headers,
    });
    if (body != null) request.body = jsonEncode(body);
    final streamed = await _client.send(request).timeout(timeout);
    final response = await http.Response.fromStream(streamed).timeout(timeout);
    return _decode(response);
  }

  Future<Map<String, dynamic>> voicePermission() =>
      request('GET', '/v1/me/voice-transcription-permission');

  Future<void> setVoicePermission(bool enabled) async {
    await request(
      'POST',
      '/v1/me/voice-transcription-permission',
      body: {'enabled': enabled, if (enabled) 'disclosure_version': 1},
    );
  }

  Future<List<VoiceCapture>> voiceCaptures() async {
    final response = await request('GET', '/v1/voice-captures');
    return (response['voice_captures'] as List<dynamic>)
        .map((value) => VoiceCapture.fromJson(value as Map<String, dynamic>))
        .toList();
  }

  Future<Map<String, dynamic>> extractionPermission() =>
      request('GET', '/v1/me/transcript-extraction-permission');

  Future<void> setExtractionPermission(bool enabled) async {
    await request(
      'POST',
      '/v1/me/transcript-extraction-permission',
      body: {'enabled': enabled, if (enabled) 'disclosure_version': 1},
    );
  }

  Future<Map<String, dynamic>> extractVoiceCapture(String captureId) => request(
    'POST',
    '/v1/voice-captures/$captureId/extract',
    timeout: const Duration(seconds: 90),
  );

  Future<String> transcribeVoice(
    String draftId,
    int capturedAtMs,
    File audioFile,
  ) async {
    final request = http.Request(
      'POST',
      _uri('/v1/voice-drafts/$draftId/transcribe'),
    );
    request.headers.addAll({
      'accept': 'application/json',
      'content-type': 'audio/mp4',
      'x-captured-at-ms': '$capturedAtMs',
      if (token != null) 'authorization': 'Bearer $token',
    });
    request.bodyBytes = await audioFile.readAsBytes();
    final streamed = await _client
        .send(request)
        .timeout(const Duration(seconds: 90));
    final response = await http.Response.fromStream(streamed)
        .timeout(const Duration(seconds: 90));
    final result = _decode(response);
    final capture = result['capture'] as Map<String, dynamic>?;
    if (result['server_audio_retained'] != false ||
        capture?['status'] != 'transcript_ready' ||
        (capture?['transcript'] as String?)?.trim().isEmpty != false) {
      throw const ApiFailure(
        'invalid_response',
        'Server did not confirm a usable private transcript.',
        0,
      );
    }
    return capture?['id'] as String;
  }

  Future<void> deleteVoiceTranscript(VoiceCapture capture) async {
    await request(
      'DELETE',
      '/v1/captures/${capture.id}/source',
      headers: {'if-match': '${capture.sourceRevision}'},
    );
  }

  Future<Map<String, dynamic>> exchange(String provider, String idToken) =>
      request(
        'POST',
        '/v1/session/exchange',
        body: {'provider': provider, 'id_token': idToken},
      );
  Future<Map<String, dynamic>> me() => request('GET', '/v1/me');
  Future<void> acceptDisclosure() async {
    await request(
      'POST',
      '/v1/me/visibility-disclosure',
      body: {'accept': true},
    );
  }

  Future<void> signOut() async {
    await request('DELETE', '/v1/session');
  }

  Future<List<RekkyItem>> items() async {
    final all = <RekkyItem>[];
    String? cursor;
    do {
      final page = await request(
        'GET',
        '/v1/items${cursor == null ? '' : '?cursor=${Uri.encodeQueryComponent(cursor)}'}',
      );
      all.addAll(
        (page['items'] as List<dynamic>).map(
          (value) => RekkyItem.fromJson(value as Map<String, dynamic>),
        ),
      );
      cursor = page['next_cursor'] as String?;
    } while (cursor != null);
    return all;
  }

  Future<RekkyItem> save(
    String subject,
    String body,
    String visibility,
    String idempotencyKey,
  ) async {
    final response = await request(
      'POST',
      '/v1/items',
      body: {'subject': subject, 'body': body, 'visibility': visibility},
      headers: {'idempotency-key': idempotencyKey},
    );
    return RekkyItem.fromJson(response['item'] as Map<String, dynamic>);
  }

  Future<RekkyItem> changeVisibility(RekkyItem item, String visibility) async {
    final response = await request(
      'PATCH',
      '/v1/items/${item.id}',
      body: {'visibility': visibility},
      headers: {'if-match': '${item.revision}'},
    );
    return RekkyItem.fromJson(response['item'] as Map<String, dynamic>);
  }

  Future<void> delete(RekkyItem item) async {
    await request(
      'DELETE',
      '/v1/items/${item.id}',
      headers: {'if-match': '${item.revision}'},
    );
  }

  Future<RekkySource?> source(RekkyItem item) async {
    final response = await request('GET', '/v1/captures/${item.captureId}');
    final data =
        (response['capture'] as Map<String, dynamic>)['source']
            as Map<String, dynamic>?;
    return data == null ? null : RekkySource.fromJson(data);
  }

  Future<void> deleteSource(RekkyItem item, RekkySource source) async {
    await request(
      'DELETE',
      '/v1/captures/${item.captureId}/source',
      headers: {'if-match': '${source.revision}'},
    );
  }

  Future<List<Map<String, dynamic>>> ask(String question) async {
    final results = <Map<String, dynamic>>[];
    String? cursor;
    do {
      final input = <String, String>{'question': question};
      if (cursor != null) input['cursor'] = cursor;
      final page = await request('POST', '/v1/ask', body: input);
      results.addAll(
        (page['results'] as List<dynamic>).cast<Map<String, dynamic>>(),
      );
      cursor = page['next_cursor'] as String?;
    } while (cursor != null);
    return results;
  }
}
