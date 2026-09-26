# Readable recommendations from voice

The canonical plan already requires this in sections 4 (flexible, evidence-backed shape), 5A (separate subjects) and 6 (capture-level understanding). The previous extractor saved source passages and checked text overlap; that was a preservation bridge, not the planned recommendation representation.

## Implemented scope

| Planned behavior | This implementation |
| --- | --- |
| A concise faithful account | A short attributed summary plus flexible observations; repetition and filler can disappear from display copy |
| Stable subject and one primary shelf | An exact source-backed subject and entity kind; server derives Places, People & services, Things, Activities & events, or Ideas & tips |
| Useful observations | Praise, concrete suggestions, suitability, cautions, attributed prices and open-ended context; absent information is omitted |
| Experience and attribution | Firsthand, secondhand, untried interest or unspecified; owner-facing labels and prose preserve supplied attribution |
| Distinct location roles | Venue, practice, stated service area, past experience and contextual mention; no implied service coverage or external place match |
| Evidence and coverage | Server-numbered transcript segments, checked references and stored private support; meaningful unaccounted material marks partial |
| Fast creation | One capture-wide understanding call for normal captures, using the existing model and budgeted worker; no research or separate category-classifier call |
| Source control | Evidence copies live in a separate table linked to source deletion by a cascading foreign key; item/search responses omit source support |

The item `body` remains an indexed projection of summary, observations, locations and use cases, so own lexical Ask can find details that do not fit the card preview. It is not an independent second model summary. The structured `recommendation` field drives Library/details. Cautions stay visible in compact cards; full detail is scrollable. Source text remains collapsed and owner-only.

A digit amount, currency symbol, external URL or location phrase absent from its cited units is rejected by deterministic checks. A broad summary citation does not count as complete detailed coverage of an entire long recording. Pure filler or verbatim duplicate units can be ignored; other missing units keep the result partial.

These checks establish references and conservative invariants, **not semantic entailment**. The model can still misinterpret a conditional or omit meaning within a cited segment. Structured Outputs guarantees a response shape, not factual correctness ([official documentation](https://developers.openai.com/api/docs/guides/structured-outputs)). The schema/prompt is versioned as understanding v2. All output remains private in the current pilot, and no quality gate is marked passed.

## Existing recommendations

`POST /v1/items/{id}/refine`, with the current `If-Match` revision, explicitly upgrades a legacy single-item voice capture. The mobile detail action is Update recommendation. There is no automatic library backfill.

The same item ID, audience and original creation time survive. A successful update increments its revision. Completed structured items return their existing result without another provider call. A changed/deleted item, removed/revised source or withdrawn permission fences late results. Failed or ambiguous extraction leaves the existing recommendation untouched. Multi-item captures are rejected by this upgrade path until a matching/edit-preservation workflow is implemented.

A separate refinement ledger preserves the old extraction receipt and attempt history. A refinement has at most three attempts, and new refinement operations count against the same daily 12/account and 100/global understanding-operation budget. This is an explicit new versioned operation, not a reset of an exhausted historical extraction. Withdrawal cancels unfinished refinement jobs; re-enabling does not revive them. The source/visibility controls remain independent of processing.

## What remains planned

Grouped shelf navigation, richer typed rating facts, persistent person/referrer identity, transcript correction, general multi-item reprocessing that preserves edits, independent completion/follow-up orchestration, external place/contact matching, semantic retrieval and friend access remain open. Prose and source support preserve supplied ratings/attribution now, without claiming a complete typed domain model or verified identity.

The initial four-case synthetic live probe is a failure-finding check, not the plan's 60-scenario labelled corpus or a held-out evaluation. It covers venue specifics, mixed subjects/negative experiences/untried advice, Hinglish conditions and personal professional experience. Prompt iterations on these same examples do not establish generalization.

The prompt-development probe produced four structurally valid outputs with six subjects in its third run (2.38–5.253 seconds for understanding only). Review found unsupported wording in optional use cases and variable named-referrer preservation. The final validator restricts use cases to cited source phrases and omits unsupported proposals; the prompt reinforces named attribution. Conservative English role checks downgrade unsupported practice/coverage claims, and common negative/limiting language is displayed as a caveat even if misclassified. These narrow safeguards do not establish multilingual semantic correctness. [Probe observations](./evaluation/understanding_v2_probe_2026-09-26.json) record the limits without any real user content.

Local verification: Rust formatting, Clippy with warnings denied, 14 API/database integration tests and 10 understanding-validator tests passed. Flutter analysis and all 22 tests passed, including shared-contract parsing and compact/expanded recommendation layouts at 200% text size. The release backend and Android debug APK build passed; the APK is installed on the connected Android phone and the new local backend is running. The update action was inspected on-device. Existing real notes have not been resent as part of this validation; their explicit upgrade is separately authorized. iOS runtime and the full labelled quality gate remain unverified.
