# Rekky API contracts

`v1/fixtures/wire.json` is the versioned language-neutral example set consumed by both backend and Dart tests. It includes signed-out errors, disclosure, Friends/Private revisions, own Ask, idempotency, processing withdrawal, separate source deletion, and a future accepted-transcript/text-retained state. The voice state is a contract example, not a claim that the current API uploads or transcribes audio. Expand these fixtures with lifecycle requests and errors before voice workers or friends are connected.

Start with signed-in account sessions, captures, owner-only source transcripts, knowledge items, visibility acknowledgement and own-knowledge Ask. Include audio-deleted/text-retained states, source revisions, processing withdrawal without knowledge loss and pending offline privacy/deletion writes. No guest contract is required. Include success/error, optional/null, IDs, timestamps, large cursors, idempotency and revision-conflict examples. Verify the same fixtures in Dart and Rust before dependent implementation.

Legacy Zod definitions are reference material only. Domain models must remain independent of generated transport models.

The one-tap flow adds `POST/GET/DELETE /v1/remember/{draft_id}` and queued/saved/partial/cancelled receipt fixtures. POST returns 202 after durable upload acceptance, not after extraction. Poll GET for `transcribed` to delete local audio and `capture_status` completed/partial to refresh Library. Item lists include `needs_review` for partial captures. See [one-tap contract and limits](../../docs/ONE_TAP_REMEMBER.md).

Understanding v2 adds an optional `recommendation` item field (summary, entity kind/shelf, experience, observations, location roles and use cases); older/text items may have null/absent data. The `structured_recommendation` wire fixture is consumed by both clients. Raw source support is not part of this field. `POST /v1/items/{id}/refine` uses If-Match for an explicit in-place upgrade of a legacy single-item voice capture and returns the existing extraction-items envelope.

Personal ratings are additive inside recommendation v2. [Rating fixtures](v1/fixtures/ratings.json) distinguish an estimate, spoken score, owner-set zero and no score. Detail consumers retain origin; raw support remains private. Content edits accept optional `rating` with keep/none/set modes, defaulting to keep for older clients, subject to changed-content invalidation. See [contract and semantics](../../docs/RATINGS.md).
