# Rekky API contracts

`v1/fixtures/wire.json` is the versioned language-neutral example set consumed by both backend and Dart tests. It includes signed-out errors, disclosure, Friends/Private revisions, own Ask, idempotency, processing withdrawal, separate source deletion, and a future accepted-transcript/text-retained state. The voice state is a contract example, not a claim that the current API uploads or transcribes audio. Expand these fixtures with lifecycle requests and errors before voice workers or friends are connected.

Start with signed-in account sessions, captures, owner-only source transcripts, knowledge items, visibility acknowledgement and own-knowledge Ask. Include audio-deleted/text-retained states, source revisions, processing withdrawal without knowledge loss and pending offline privacy/deletion writes. No guest contract is required. Include success/error, optional/null, IDs, timestamps, large cursors, idempotency and revision-conflict examples. Verify the same fixtures in Dart and Rust before dependent implementation.

Legacy Zod definitions are reference material only. Domain models must remain independent of generated transport models.
