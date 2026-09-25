# Rekky API contracts

Reserved for the new versioned, language-neutral API schema and shared wire fixtures. Initial contracts are pending F-00.1.

Start with signed-in account sessions, captures, owner-only source transcripts, knowledge items, visibility acknowledgement and own-knowledge Ask. Include audio-deleted/text-retained states, source revisions, processing withdrawal without knowledge loss and pending offline privacy/deletion writes. No guest contract is required. Include success/error, optional/null, IDs, timestamps, large cursors, idempotency and revision-conflict examples. Verify the same fixtures in Dart and TypeScript before dependent implementation.

Legacy Zod definitions are reference material only. Domain models must remain independent of generated transport models.
