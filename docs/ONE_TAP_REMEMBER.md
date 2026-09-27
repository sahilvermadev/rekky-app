# One-tap Remember

Implemented 2026-09-26, following the canonical [product plan](REKKY_FLUTTER_PRODUCT_PLAN.md). This replaces the capture form and draft panels in the normal mobile journey.

## Normal journey

Sign in and accept the concise setup once. Ask is home; Library is a stable destination. The central Remember action opens the recording screen and starts the microphone immediately (subject to the operating system's microphone permission). The screen prompts for the experience and shows elapsed time, Done and Discard. There is no typing, title, category, audience or transcription decision.

Done finalizes the account-owned recording on the phone and acknowledges it locally. Upload, transcription and extraction proceed automatically. Library displays a quiet pending state and refreshes when the saved item arrives. Partial/source-preserving results say Needs review. The source transcript remains owner-only; long quoted notes preview three lines inside details. New completed voice recommendations default to Friends, following the account disclosure. Partial/fallback results initially remain Only me; transcripts remain owner-only. Already accepted legacy work and saved items retain their audience. The Friends marker does not enable cross-account reads: friendship access and external-pilot quality gates remain unfinished.

Account settings retain processing opt-out and Pending recordings recovery. New recordings are marked for automatic processing; migrated older drafts are not uploaded automatically. Older retained transcripts are not reprocessed by this upgrade. No production transcript is used as a test fixture.

## Persistence and API

`POST /v1/remember/{draft_id}` accepts `audio/mp4` bytes with `X-Captured-At-Ms`, authenticated to the recording owner. It requires the separate recorded voice and extraction permissions. A new upload returns 202; an identical retry returns its current receipt. Changed audio under an existing ID conflicts. GET returns only the owner's status, capture ID/status and whether a transcript exists. DELETE cancels pending processing; its tombstone prevents a delayed upload from reviving the recording. Completed saved items and source transcripts have separate delete controls.

SQLite schema v3 stores an automatic-processing marker and durable pending-receipt IDs per account. It saves the receipt tracker before deleting accepted local audio. The coordinator polls while the app is active, pauses with the app and resumes after restart. It never waits for extraction before deleting acknowledged audio. It captures the session token for the current owner, and stops on account changes.

Migration 007 adds `voice_uploads` and the capture's automatic-processing state. The Rust worker runs in the API process, claims bounded leased work and resumes after process restart. It does not persist or require a user's session token. Each accepted transcript and removal of its active queued audio commit in the same database transaction. Extraction consumes only the retained private text. Completed job receipts make retries idempotent. Permission generations, source revisions and attempt IDs fence stale/withdrawn results.

Quota-blocked extraction receipts add `waiting_reason: "processing_limit"` and UTC `retry_at`, otherwise both are null. The reset estimate includes extraction and refinement history, respects both account/global limits and handles a lowered limit. Existing extraction-job retries retain their own attempt rules. The app shows the wait and local reset time, keeps the pending receipt after deleting accepted audio, and checks again automatically. The backend rechecks pending captures at three-minute intervals, so the time is earliest eligibility, not a completion promise. No provider request or attempt is consumed while waiting for a new-job quota slot.

## Operational limits

- New recordings: two-minute mobile cap, 5 MiB upload cap, 50 MiB local pending-audio cap, seven-day eligibility. Quotas default to 12 per account per rolling 24 hours, configurable with `PILOT_DAILY_ACCOUNT_LIMIT` (1–100). The local testing instance uses 30 with owner approval. The account cap applies at upload, transcription and combined extraction/refinement stages; each global cap remains 100. Category-learning limits are separate; at most three attempts per upload/extraction. These are pilot limits, not performance measurements.
- Before the server accepts an upload, the recording is safe locally and retries when the app runs. Native Android/iOS scheduled background upload is not implemented; force-quitting before acceptance does not guarantee upload completion.
- After acceptance, the running backend finishes work without the phone. It scans every two seconds while idle, processes one upload and extraction per tick and uses bounded leases after interruption. This single-worker pilot is not a throughput or latency benchmark.
- Temporary server audio currently lives in the isolated development Postgres database. Clearing the active row does not erase WAL, volume snapshots or backups. Production must move it to excluded ephemeral storage, or establish and verify an equivalent backup/expiry design before claiming production audio-deletion guarantees. Local physical deletion occurs on the next permitted app execution; server expiry requires the worker to run.
- The UI marks saving complete only after the local recording is durable; “Adding your recommendation” means remote processing remains. Permanent failures are routed to recovery. Extraction quality, organization, source correction, friends, offline Library synchronization and iOS behavior retain their existing open gates.

## Validation

Synthetic backend checks cover queue acceptance/idempotency, cross-account denial, processing after logout, atomic audio removal, single saved item and own Ask, opt-out/opt-in without revival, cancellation before upload and cancellation during transcription. Existing privacy, shared-source deletion, quota and extraction checks remain.

Flutter checks cover account-scoped durable audio, legacy migration without automatic upload, server acknowledgement cleanup, pending extraction recovery across a store restart, one-tap recording and immediate Done return at normal and 200% text sizes on a 360×640 viewport. These tests use fake recorders/providers and do not establish live provider latency or real-world extraction quality.

Local verification: Rust formatting, Clippy with warnings denied, 5 unit tests and 12 integration tests passed; release backend build passed. Flutter analysis and all 18 tests passed; Android debug APK built and installed on the connected Android phone. Physical-device inspection verified the simplified home navigation, Library's existing partial-review label and successful SQLite upgrade. The new backend is running on the isolated local phone connection. No new live voice recording or provider request was performed during this validation; the fresh-recording latency/quality check remains for the user's next capture. iOS runtime validation remains open.

Migration 015 snapshots the intended audience on voice uploads and transcription jobs. Existing rows get Private; new rows default to Friends. Transcription copies the job audience into the capture, and complete extraction copies the capture audience into its items. Retries do not choose a new audience. Known-partial output always initially saves privately, regardless of the intended audience.

2026-09-27 quota recovery verification: 77 Rust tests, 16 focused Flutter tests, Clippy, Flutter analysis and both builds pass. Regression coverage verifies combined extraction/refinement accounting, lowered limits, natural reset and increased-limit resumption without retranscription. The updated backend and APK are installed locally; the blocked real capture completed on its first extraction attempt after the approved increase to 30.


2026-09-27 understanding revision: new captures retain structured information when an individual field fails and reserve at most one automatic repair within the same three extraction-attempt ceiling. Review state and automatic sharing are per item; unresolved subject boundaries retain a private hold. Diagnostic issue codes drive the quiet review explanation. Optional missing ratings or enrichment do not require review. See the canonical plan's resilient-understanding checkpoint. Previously saved items and source transcripts are not silently reprocessed.
