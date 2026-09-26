# Rekky

A fresh Flutter mobile application and Rust/Postgres backend.

**Tell Rekky once. Find it when it matters. Pass it on effortlessly.**

## Source of truth

Follow [the Rekky Flutter product and delivery plan](docs/REKKY_FLUTTER_PRODUCT_PLAN.md). It owns the product requirements, delivery sequence and acceptance gates. After mandatory sign-in, the app opens on Ask, Library is secondary, and Remember is an always-visible action. Completed items default to Friends with a simple Private override. Audio is deleted after a usable transcript is durably saved; retained text transcripts stay owner-only. Turning off future AI processing preserves completed knowledge and transcripts.

The user chose a standalone repository and a Rust backend on 2026-09-25. Flutter and the new backend remain together here for coordinated API changes.

## Repository layout

- `apps/rekky_flutter/`: independent Flutter client.
- `apps/rekky_backend/`: independent API, workers and migrations.
- `contracts/rekky/`: language-neutral API schemas and shared wire fixtures.
- `docs/`: canonical product plan and reference provenance.

## Current status

F-00.1, F-00.3, F-01.1 and the first F-01.2 extraction path are in progress. The repository contains a new Rust/Postgres API, a Flutter signed-in shell, shared v1 wire examples, local database Compose configuration, and Android/macOS CI definitions. Google sign-in → account disclosure → manually authored text memory → own-library Ask → deletion has been exercised on a physical Android phone against the isolated local backend. The user made one real local voice draft that survived an app restart and metadata upgrade. The [voice-draft spike](docs/VOICE_DRAFT_SPIKE.md) keeps audio in protected temporary storage, and the [transcription bridge](docs/VOICE_TRANSCRIPTION_SLICE.md) adds explicit OpenAI processing permission, a private transcript, and local audio deletion after server acknowledgement. One live transcription completed. The new extraction path can turn an owner-only transcript into evidence-grounded private items and own Ask. For the first real transcript, strict extraction failed twice; a user-approved final attempt saved one private partial source-preserving review draft, not yet a distilled recommendation. Apple sign-in, iOS device behavior, offline sync and reciprocal-friend access remain unverified. No delivery phase has passed its full gate.

The [readable recommendation update](docs/RECOMMENDATION_UNDERSTANDING.md) implements concise summaries, useful observations, visible caveats, attribution and typed locations from source-linked voice understanding. Older single-item voice recommendations have an explicit Update recommendation action in details.

The [one-tap Remember update](docs/ONE_TAP_REMEMBER.md) removes the typed capture form and draft panels from Ask: tap Remember, speak, tap Done, and new recordings upload, transcribe and save automatically. Server-accepted processing survives app closure; uploads not yet accepted resume when the app runs. The pilot saves private items and labels partial extraction as needing review. Validate a new recording on-device, then improve extraction quality and measure latency before starting further slices. Text/URL share entry remains planned for F-02.

## Isolation and configuration

Use a new database, database credentials, app identity, sessions, storage access, job state, indexes and deployment configuration. Everyone starts with fresh accounts and no legacy content or friendships.

The old app is a reference only. Do not copy its environment files, dependencies, history, accounts or database. Review provider settings individually; keep secrets outside Git and off the mobile client. See [legacy reference and provenance](docs/LEGACY_REFERENCE.md).
