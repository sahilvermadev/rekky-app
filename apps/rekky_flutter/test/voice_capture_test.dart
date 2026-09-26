import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rekky_flutter/voice_capture_sheet.dart';
import 'package:rekky_flutter/voice_drafts.dart';

class CaptureStore extends VoiceDraftStore {
  int starts = 0, finishes = 0, cancels = 0;
  @override
  Future<void> start(String ownerId) async {
    starts++;
  }

  @override
  Future<VoiceDraft> finish(String ownerId) async {
    finishes++;
    return draft(ownerId, 'ready');
  }

  @override
  Future<void> cancel(String ownerId) async {
    cancels++;
  }

  VoiceDraft draft(String owner, String status) => VoiceDraft(
    id: 'new-recording-0001',
    ownerId: owner,
    status: status,
    createdAtMs: 123,
    bytes: 256,
    autoProcess: true,
  );
}

void main() {
  for (final scale in [1.0, 2.0]) {
    testWidgets(
      'one tap records; Done queues and returns at text scale $scale',
      (tester) async {
        tester.view.physicalSize = const Size(360, 640);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        final store = CaptureStore();
        var queued = 0;
        bool? saved;
        await tester.pumpWidget(
          MaterialApp(
            builder: (context, child) => MediaQuery(
              data: MediaQuery.of(context)
                  .copyWith(textScaler: TextScaler.linear(scale)),
              child: child!,
            ),
            home: Builder(
              builder: (context) => Scaffold(
                body: Center(
                  child: FilledButton(
                    child: const Text('Remember'),
                    onPressed: () async {
                      saved = await Navigator.push<bool>(
                        context,
                        MaterialPageRoute(
                          builder: (_) => VoiceCaptureSheet(
                            ownerId: 'owner',
                            store: store,
                            onChanged: () async {},
                            onCaptured: () {
                              queued++;
                            },
                          ),
                        ),
                      );
                    },
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.tap(find.text('Remember'));
        await tester.pumpAndSettle();
        expect(store.starts, 1);
        expect(find.byType(TextField), findsNothing);
        expect(find.text('Tell us about your experience'), findsOneWidget);
        expect(tester.takeException(), isNull);
        await tester.ensureVisible(find.text('Done'));
        await tester.tap(find.text('Done'));
        await tester.pumpAndSettle();
        expect(store.finishes, 1);
        expect(queued, 1);
        expect(saved, true);
        expect(find.text('Remember'), findsOneWidget);
        expect(tester.takeException(), isNull);
      },
    );
  }
}
