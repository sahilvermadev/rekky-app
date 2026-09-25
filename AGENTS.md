# Working on Rekky

Read `docs/REKKY_FLUTTER_PRODUCT_PLAN.md` before implementation. It is the canonical product and delivery specification; record deliberate scope changes there rather than inventing a competing plan.

- This is a standalone, fresh product. Keep the Flutter client, new backend and versioned contracts in this repository.
- Treat legacy `mapx` code as reference. Do not import its domain packages, routes, workers, database migrations, accounts or user data. Adapt small audited integrations behind new interfaces with focused verification.
- Never copy a legacy `.env` wholesale or commit credentials. New-product infrastructure and sessions must be isolated. Provider credentials and paid orchestration belong on the backend.
- Follow the F-00 through F-07 tracker. Build integrated mobile/API slices and agree on wire fixtures before dependent implementation diverges.
- Preserve Ask home, secondary Library and always-visible Remember; preserve the plan's temporary-audio policy and reciprocal-friend access rules.
- Mark a delivery gate complete only with evidence. Distinguish previews, automated checks and physical-device validation; record remaining gaps honestly.
- Keep the old application and infrastructure unchanged as part of this build.
