# Legacy reference and plan provenance

The legacy repository is `sahilvermadev/mapx`, available locally at `/home/sahil/projects/mapx`. It remains separate and unchanged. This repository has independent Git history and no legacy runtime dependency.

## Canonical plan origin

- Source: `docs/REKKY_FLUTTER_PRODUCT_PLAN.md` in the legacy working tree.
- Imported on: 2026-09-25.
- Legacy HEAD at import: `ac0592f16b9ad8d3375129e55bbb1d9031af0308` (the source was read from the working tree and may include uncommitted planning changes).
- Source SHA-256: `23d85c70862c7387aa1bdd80e841c78a889d2ca4b1e3c7c79a71d6664ab6d9ff`.
- Initial import change: the user requested a standalone repository; that import preserved the then-current product requirements and acceptance gates.
- Subsequent approved review revision, 2026-09-25: mandatory sign-in replaces guest use; private text transcripts are retained while processed audio is deleted; Friends-by-default is reaffirmed. Consent withdrawal, offline acknowledgement, contact discovery, early evaluation and delivery checkpoints now follow the explicit new-product policies. The source hash above identifies the original import, not the revised plan.

The earlier `docs/REKKY_EXPERIENCE_REDESIGN_PLAN.md`, capture/voice plans, adapter filenames and architecture-review findings mentioned in the product plan refer to the legacy repository. They are historical or technical references, not files promised to exist here. Use them to inspect technical mechanisms and tests only. No consent, retention, guest, deletion or audience rule is inherited from them. The canonical Flutter plan defines the new-product rules explicitly; audit any reused adapter for compatibility, particularly legacy deletion of AI-derived items on opt-out.

## Selective reuse

Inspect candidate transcription, object-storage and lookup adapters, plus tested evidence, lease, retry and budget rules. Bring across only justified code with its relevant tests, provenance and applicable license notices. Implement new orchestration and persistence against the new domain.

Do not copy the old application, dependency tree, Git history, database migrations, content, account IDs or social graph. No legacy submodule or linked source directory is required.

## Environment configuration

Inspect the legacy example configuration to identify required services. Create the new backend's explicit configuration contract during F-00.1 rather than copying all existing variables.

Provider credentials may be reusable after checking their scope, restrictions, retention configuration and budget. Reuse of a provider does not authorize access to old product data. Use isolated new-product database credentials, storage access, sessions and deployment configuration. Do not put secrets in documentation, Git history or the Flutter application.
