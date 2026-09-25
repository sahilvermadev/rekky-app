# Working on Rekky

Read `docs/REKKY_FLUTTER_PRODUCT_PLAN.md` before implementation. It is the canonical product and delivery specification; record deliberate scope changes there rather than inventing a competing plan.

- This is a standalone, fresh product. Keep the Flutter client, new backend and versioned contracts in this repository.
- Treat legacy `mapx` code as reference. Do not import its domain packages, routes, workers, database migrations, accounts or user data. Adapt small audited integrations behind new interfaces with focused verification.
- Never copy a legacy `.env` wholesale or commit credentials. New-product infrastructure and sessions must be isolated. Provider credentials and paid orchestration belong on the backend.
- Follow the F-00 through F-07 tracker. Build integrated mobile/API slices and agree on wire fixtures before dependent implementation diverges.
- Require sign-in; do not implement guest capture, anonymous owners or guest import. Preserve Ask home, secondary Library, always-visible Remember and Friends-by-default completed items with Private overrides.
- Delete audio after accepted transcription is durably stored; retain owner-only text sources until explicit deletion. Processing withdrawal preserves completed knowledge/transcripts. Apply the plan's server-acknowledgement rules to offline privacy changes.
- New-product consent, source retention and deletion rules are explicit in the plan. Do not inherit legacy opt-out cascades or expose transcripts through network evidence links.
- Mark a delivery gate complete only with evidence. Distinguish previews, automated checks and physical-device validation; record remaining gaps honestly.
- Keep the old application and infrastructure unchanged as part of this build.
