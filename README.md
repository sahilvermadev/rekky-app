# Rekky

A fresh Flutter mobile application and Express/TypeScript/Postgres backend.

**Tell Rekky once. Find it when it matters. Pass it on effortlessly.**

## Source of truth

Follow [the Rekky Flutter product and delivery plan](docs/REKKY_FLUTTER_PRODUCT_PLAN.md). It owns the product requirements, delivery sequence and acceptance gates. The app opens on Ask, Library is secondary, and Remember is an always-visible action.

The user chose a standalone repository on 2026-09-25. That supersedes only the plan's earlier placement inside the legacy repository. Flutter and the new backend remain together here for coordinated API changes.

## Repository layout

- `apps/rekky_flutter/`: independent Flutter client.
- `apps/rekky_backend/`: independent API, workers and migrations.
- `contracts/rekky/`: language-neutral API schemas and shared wire fixtures.
- `docs/`: canonical product plan and reference provenance.

## Current status

Repository and documentation setup only. Application code, tooling, CI, contracts and infrastructure have not yet been scaffolded. F-00.1 remains in progress; no delivery gate is complete.

Start with F-00.1 foundations and contract fixtures, F-00.2 native design exploration, and F-00.3 recording/share feasibility. The first live product outcome is voice/text capture → useful organized knowledge → own Ask, including bounded temporary-media recovery and deletion.

## Isolation and configuration

Use a new database, database credentials, app identity, sessions, storage access, job state, indexes and deployment configuration. Everyone starts with fresh accounts and no legacy content or friendships.

The old app is a reference only. Do not copy its environment files, dependencies, history, accounts or database. Review provider settings individually; keep secrets outside Git and off the mobile client. See [legacy reference and provenance](docs/LEGACY_REFERENCE.md).
