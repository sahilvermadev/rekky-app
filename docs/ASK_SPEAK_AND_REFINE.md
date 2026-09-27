# Ask: speak and refine pilot

Implemented 2026-09-28 as an ASK-2 development checkpoint. This extends the [first agentic Ask slice](ASK_AGENT_SLICE.md); it does not close ASK-0/1's evaluation/scale gates or the first-release comparison gate.

## Experience

Tap the microphone in Ask, speak a question, then Done. Rekky transcribes and automatically submits it. The question stays visible and can be tapped to edit; changing a pending request cancels the obsolete answer. The microphone never invokes Recommend, creates a voice draft, queues extraction or adds Library content.

After an answer, the follow-up field stays at the bottom of the Ask surface, above the navigation capsule. A request such as “We will be eight people” carries the original occasion and constraints. “Ask about this” explicitly selects one recommendation so “this one” has a precise referent. The exclude action removes an option for the conversation, without creating a durable preference. One consequential clarification still appears when the referent or need is ambiguous.

New question explicitly resets context. The agent also identifies clearly unrelated requests; those start fresh context while Previous answer can recover the earlier answer. Previous answer re-fetches/revalidates the prior snapshot without another AI call. Existing answers stay visible during work and recoverable failure. Editing the displayed question reuses its parent context rather than appending the old wording as another constraint.

## Context contract

`POST /v1/ask/agent` accepts optional `previous_request_id`, `selected_item_ids` (up to four) and `excluded_item_ids` (up to eight new exclusions). These participate in the idempotency hash. Only items in the preceding authorized answer can be explicitly selected or excluded. Missing/deleted selected references fail before a model call; no substitute is silently chosen.

The server carries up to eight actual user turns, current selections and session exclusions (at most 32), plus up to eight freshly fetched prior result projections. The agent sees prior clarification text only when its associated evidence has not changed. Every provider decision still checks session/permission generation and evidence revisions. The existing three-decision/four-read-tool/50-second/spend limits remain unchanged. The server filters explicit exclusions; the agent interprets natural-language refinements. Preserving semantic constraints is evaluated behavior, not a deterministic proof for arbitrary language.

The answer fixture adds `question`, `turn_count`, `selected_item_ids` and `excluded_item_ids`. The internal presentation tool adds `new_topic`, which resets inherited conversational constraints for an unrelated request. No hidden chain-of-thought or raw source archive is persisted. Context is contained in the same owner-only, 15-minute answer snapshots; children carry their own bounded context until their expiry. Expired context never silently falls back to treating a short follow-up as a standalone query. Start a new question after expiry or eight turns.

This is a bounded current conversation with local previous-answer navigation, not a history browser, durable preference system or synchronized multi-device session. Those remain ASK-4/5.

## Temporary dictation

`POST /v1/ask/dictations/{UUID}` takes `audio/mp4` bytes. It requires account disclosure and current voice-processing permission, validates an M4A container duration of 0.3–61 seconds (60-second recorder limit) and caps the file at 600,000 bytes. The parser handles ordinary and extended-size MP4 boxes. It uses the existing backend `gpt-transcribe` adapter and [official file-transcription API](https://developers.openai.com/api/docs/guides/speech-to-text); no additional transcription model or client credential is introduced.

Audio remains in request memory on the backend and is never inserted into a database or object store. On the phone it lives in a separate app-private cache directory, is deleted after reading for upload, and is discarded on cancel/background interruption. Startup purges recordings left by a process crash. Recorder startup and cancellation are ordered so a late native reply cannot leave the microphone running. Microphone access uses the existing OS permission, and the sheet explains OpenAI processing and that nothing will be saved to Library.

Migration 021 stores only the audio hash, operation status, owner/permission generation, short-lived text and usage receipt. Same ID/audio returns the accepted transcript without another paid call while still available. Changed input, cancellation or terminal failure does not restart paid work. The client clears accepted text with `DELETE /v1/ask/dictations/{UUID}` immediately; cancellation also clears text and fences late provider results. If deletion cannot reach the backend, text expires after ten minutes. Account deletion cascades; permission withdrawal blocks completion. Receipts expire after 30 days.

The operation has a 35-second provider deadline, one active dictation/account, 30 paid attempts/account/day and 300 globally/day. Admission and cancellation use a database lock. Bounded cancellation tombstones cannot consume the paid global allowance. Unknown paid outcomes remain counted; no automatic paid retry. Failed/oversized/empty transcriptions leave typing available, and typed Ask has its own allowance. These pilot call caps do not replace production project billing controls.

## Evidence and open work

- Deterministic API tests cover current evidence on refinement, owner-scoped parents/selections, expired context, temporary-text deletion, retry identity, cancellation racing admission/provider work, withdrawal and zero Library captures from dictation.
- Full regressions passed: 112 Rust tests (three live probes are opt-in) and 168 Flutter tests. The final additional stop/cancel race test and focused recorder suite also pass. Rust Clippy, Flutter analysis, release backend build and debug APK build pass.
- Flutter checks cover selection/parent propagation, previous-answer recovery without a new AI call, a clean new question, local file deletion, late recorder startup, cancellation during transcription, background interruption and 320/375/414/768 widths with 200% text.
- Five live-model synthetic follow-up checks pass: initial quiet dinner discovery, inherited constraints for eight people, ambiguous “this one,” an explicitly selected unsuitable option, and an unrelated taxi recall. The original six synthetic recall/discovery cases also pass after the prompt/schema change.
- A locally synthesized “A quiet Italian dinner for eight people” recording passed through the real API/transcription adapter in 1.48 seconds, returned the expected text, created zero captures, and had temporary text erased. This establishes media/API wiring, not noisy-microphone or multilingual speech accuracy.

The final APK is installed on the connected Android phone and migration 021 is applied to the local backend. On-device microphone readiness and Back cancellation were checked: the app returned to Ask, the question cache was empty, and the backend recorded zero paid dictations for that check. A user-spoken end-to-end question on the physical microphone remains to be tried; the live transcription evidence above uses synthesized audio.

The [development report](evaluation/ask_refinement_probe_2026-09-28.json) uses invented notes only; [dictation evidence](evaluation/ask_dictation_probe_2026-09-28.json) uses synthetic speech. These are not held-out quality estimates. Remaining: noisy/interrupted real speech across supported phones and languages, iOS runtime, full held-out retrieval/constraint evaluation, larger-library continuation, complete comparison UI (ASK-3) and later durable history/preferences. The plan's full gates remain unchecked.
