# Legacy reference and plan provenance

The legacy repository is `sahilvermadev/mapx`, available locally at `/home/sahil/projects/mapx`. It remains separate and unchanged. This repository has independent Git history and no legacy runtime dependency.

## Canonical plan origin

- Source: `docs/REKKY_FLUTTER_PRODUCT_PLAN.md` in the legacy working tree.
- Imported on: 2026-09-25.
- Legacy HEAD at import: `ac0592f16b9ad8d3375129e55bbb1d9031af0308` (the source was read from the working tree and may include uncommitted planning changes).
- Source SHA-256: `23d85c70862c7387aa1bdd80e841c78a889d2ca4b1e3c7c79a71d6664ab6d9ff`.
- Approved change: the user requested a new repository for independent GitHub management. The imported plan now places both fresh applications and their contracts here. Product requirements and acceptance gates are preserved.

The earlier `docs/REKKY_EXPERIENCE_REDESIGN_PLAN.md`, capture/voice plans, adapter filenames and architecture-review findings mentioned in the product plan refer to the legacy repository. They are historical or technical references, not files promised to exist here. Resolve compatible consent/evidence/enrichment requirements against those references when adapting an integration; the canonical Flutter plan governs conflicts.

## Selective reuse

Inspect candidate transcription, object-storage and lookup adapters, plus tested evidence, lease, retry and budget rules. Bring across only justified code with its relevant tests, provenance and applicable license notices. Implement new orchestration and persistence against the new domain.

Do not copy the old application, dependency tree, Git history, database migrations, content, account IDs or social graph. No legacy submodule or linked source directory is required.

## Environment configuration

Inspect the legacy example configuration to identify required services. Create the new backend's explicit configuration contract during F-00.1 rather than copying all existing variables.

Provider credentials may be reusable after checking their scope, restrictions, retention configuration and budget. Reuse of a provider does not authorize access to old product data. Use isolated new-product database credentials, storage access, sessions and deployment configuration. Do not put secrets in documentation, Git history or the Flutter application.
