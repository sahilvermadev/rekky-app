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

F-00.1, F-00.3 and the F-01.1 voice bridge are in progress. The repository contains a new Rust/Postgres API, a Flutter signed-in shell, shared v1 wire examples, local database Compose configuration, and Android/macOS CI definitions. Google sign-in → account disclosure → manually authored text memory → own-library Ask → deletion has been exercised on a physical Android phone against the isolated local backend. The user made one real local voice draft that survived an app restart and metadata upgrade. The [voice-draft spike](docs/VOICE_DRAFT_SPIKE.md) keeps audio in protected temporary storage, and the [transcription bridge](docs/VOICE_TRANSCRIPTION_SLICE.md) adds explicit OpenAI processing permission, a private transcript, and local audio deletion after server acknowledgement. The real draft was not uploaded during inspection; live provider transcription, Apple sign-in and iOS device behavior remain unverified. Offline sync and reciprocal-friend access are not implemented yet. No delivery phase has passed its full gate.

Continue F-00.3 physical-device recording/backup checks and F-00.2 design review, then verify F-01.1 with an explicitly consented live transcription and build transcript→knowledge extraction. The first full product outcome remains signed-in voice/text capture → private transcript → useful organized knowledge → own Ask, with prompt audio deletion and explicit source controls. Production text/URL share input arrives in early F-02; advanced capture polish remains F-06. Proposed limits and quality targets still require measurement.

## Isolation and configuration

Use a new database, database credentials, app identity, sessions, storage access, job state, indexes and deployment configuration. Everyone starts with fresh accounts and no legacy content or friendships.

The old app is a reference only. Do not copy its environment files, dependencies, history, accounts or database. Review provider settings individually; keep secrets outside Git and off the mobile client. See [legacy reference and provenance](docs/LEGACY_REFERENCE.md).
