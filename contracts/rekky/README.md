# Rekky API contracts

`v1/fixtures/wire.json` is the versioned language-neutral example set consumed by both backend and Dart tests. It includes signed-out errors, disclosure, Friends/Private revisions, own Ask, idempotency, processing withdrawal, separate source deletion, and a future accepted-transcript/text-retained state. The voice state is a contract example, not a claim that the current API uploads or transcribes audio. Expand these fixtures with lifecycle requests and errors before voice workers or friends are connected.

Start with signed-in account sessions, captures, owner-only source transcripts, knowledge items, visibility acknowledgement and own-knowledge Ask. Include audio-deleted/text-retained states, source revisions, processing withdrawal without knowledge loss and pending offline privacy/deletion writes. No guest contract is required. Include success/error, optional/null, IDs, timestamps, large cursors, idempotency and revision-conflict examples. Verify the same fixtures in Dart and Rust before dependent implementation.

Legacy Zod definitions are reference material only. Domain models must remain independent of generated transport models.

The one-tap flow adds `POST/GET/DELETE /v1/remember/{draft_id}` and queued/saved/partial/cancelled receipt fixtures. POST returns 202 after durable upload acceptance, not after extraction. Poll GET for `transcribed` to delete local audio and `capture_status` completed/partial to refresh Library. Item lists include `needs_review` for partial captures. See [one-tap contract and limits](../../docs/ONE_TAP_REMEMBER.md).

Understanding v2 adds an optional `recommendation` item field (summary, entity kind/shelf, experience, observations, location roles and use cases); older/text items may have null/absent data. The `structured_recommendation` wire fixture is consumed by both clients. Raw source support is not part of this field. `POST /v1/items/{id}/refine` uses If-Match for an explicit in-place upgrade of a legacy single-item voice capture and returns the existing extraction-items envelope.

Personal ratings are additive inside recommendation v2. [Rating fixtures](v1/fixtures/ratings.json) distinguish an estimate, spoken score, owner-set zero and no score. Detail consumers retain origin; raw support remains private. Content edits accept optional `rating` with keep/none/set modes, defaulting to keep for older clients, subject to changed-content invalidation. See [contract and semantics](../../docs/RATINGS.md).

Provider contact snapshots and matching preferences are illustrated in `v1/fixtures/contact.json`. The number inherits the containing item's audience, with no separate visibility property. See [contact API and precedence](../../docs/CONTACTS.md).

Quota waiting is an additive receipt state: `waiting_reason: "processing_limit"` with UTC `retry_at` for a transcribed capture awaiting a new extraction slot. Both fields are null otherwise; old clients may ignore them. The time is earliest eligibility, not guaranteed completion. The `remember_waiting_limit` fixture is shared by Rust and Dart.

Locations add optional `name` and `geography` fields without changing `text` or `role`. `v1/fixtures/geographic_locations.json` distinguishes a resolved area and an ambiguous clean name. `GET /v1/items` accepts `area_id` and optional `location_role`, with filter-bound pagination; owner scope always applies. See [geographic locations](../../docs/GEOGRAPHIC_LOCATIONS.md).

`v1/fixtures/ask_ui.json` records the new `ask_ui.v1` native collection contract. The model proposes a collection reference; the backend supplies its authorized count, facets and paginated items. Rust and Dart parse the same fixture while the Ask UI rolls out. Own-data exploration ships before reciprocal-friend search, which remains behind F-03 access and privacy gates.
