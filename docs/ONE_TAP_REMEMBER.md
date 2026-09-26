# One-tap Remember

Implemented 2026-09-26, following the canonical [product plan](REKKY_FLUTTER_PRODUCT_PLAN.md). This replaces the capture form and draft panels in the normal mobile journey.

## Normal journey

Sign in and accept the concise setup once. Ask is home; Library is a stable destination. The central Remember action opens the recording screen and starts the microphone immediately (subject to the operating system's microphone permission). The screen prompts for the experience and shows elapsed time, Done and Discard. There is no typing, title, category, audience or transcription decision.

Done finalizes the account-owned recording on the phone and acknowledges it locally. Upload, transcription and extraction proceed automatically. Library displays a quiet pending state and refreshes when the saved item arrives. Partial/source-preserving results say Needs review. The source transcript remains owner-only and is collapsed inside details. The current extraction pilot saves all output privately; it has not passed the gates for automatic Friends publication or faithful distilled recommendations.

Account settings retain processing opt-out and Pending recordings recovery. New recordings are marked for automatic processing; migrated older drafts are not uploaded automatically. Older retained transcripts are not reprocessed by this upgrade. No production transcript is used as a test fixture.

## Persistence and API

`POST /v1/remember/{draft_id}` accepts `audio/mp4` bytes with `X-Captured-At-Ms`, authenticated to the recording owner. It requires the separate recorded voice and extraction permissions. A new upload returns 202; an identical retry returns its current receipt. Changed audio under an existing ID conflicts. GET returns only the owner's status, capture ID/status and whether a transcript exists. DELETE cancels pending processing; its tombstone prevents a delayed upload from reviving the recording. Completed saved items and source transcripts have separate delete controls.

SQLite schema v3 stores an automatic-processing marker and durable pending-receipt IDs per account. It saves the receipt tracker before deleting accepted local audio. The coordinator polls while the app is active, pauses with the app and resumes after restart. It never waits for extraction before deleting acknowledged audio. It captures the session token for the current owner, and stops on account changes.

Migration 007 adds `voice_uploads` and the capture's automatic-processing state. The Rust worker runs in the API process, claims bounded leased work and resumes after process restart. It does not persist or require a user's session token. Each accepted transcript and removal of its active queued audio commit in the same database transaction. Extraction consumes only the retained private text. Completed job receipts make retries idempotent. Permission generations, source revisions and attempt IDs fence stale/withdrawn results.

## Operational limits

- New recordings: two-minute mobile cap, 5 MiB upload cap, 50 MiB local pending-audio cap, seven-day eligibility. Current quotas: 12 new uploads per account and 100 globally per rolling day; at most three attempts per upload/extraction. These are pilot limits, not performance measurements.
- Before the server accepts an upload, the recording is safe locally and retries when the app runs. Native Android/iOS scheduled background upload is not implemented; force-quitting before acceptance does not guarantee upload completion.
- After acceptance, the running backend finishes work without the phone. It scans every two seconds while idle, processes one upload and extraction per tick and uses bounded leases after interruption. This single-worker pilot is not a throughput or latency benchmark.
- Temporary server audio currently lives in the isolated development Postgres database. Clearing the active row does not erase WAL, volume snapshots or backups. Production must move it to excluded ephemeral storage, or establish and verify an equivalent backup/expiry design before claiming production audio-deletion guarantees. Local physical deletion occurs on the next permitted app execution; server expiry requires the worker to run.
- The UI marks saving complete only after the local recording is durable; “Adding your recommendation” means remote processing remains. Permanent failures are routed to recovery. Extraction quality, organization, source correction, friends, offline Library synchronization and iOS behavior retain their existing open gates.

## Validation

Synthetic backend checks cover queue acceptance/idempotency, cross-account denial, processing after logout, atomic audio removal, single saved item and own Ask, opt-out/opt-in without revival, cancellation before upload and cancellation during transcription. Existing privacy, shared-source deletion, quota and extraction checks remain.

Flutter checks cover account-scoped durable audio, legacy migration without automatic upload, server acknowledgement cleanup, pending extraction recovery across a store restart, one-tap recording and immediate Done return at normal and 200% text sizes on a 360×640 viewport. These tests use fake recorders/providers and do not establish live provider latency or real-world extraction quality.

Local verification: Rust formatting, Clippy with warnings denied, 5 unit tests and 12 integration tests passed; release backend build passed. Flutter analysis and all 18 tests passed; Android debug APK built and installed on the connected Android phone. Physical-device inspection verified the simplified home navigation, Library's existing partial-review label and successful SQLite upgrade. The new backend is running on the isolated local phone connection. No new live voice recording or provider request was performed during this validation; the fresh-recording latency/quality check remains for the user's next capture. iOS runtime validation remains open.
