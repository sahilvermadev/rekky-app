# Automatic category learning

An unfamiliar recommendation saves immediately. Its source-backed descriptive type remains readable and searchable while a separate worker considers a shared definition. No new capture choices, review prompts or category-specific UI are introduced.

## What is reused

The checked-in seed catalog and active database additions form one versioned registry. New flat types have stable `learned.<hash>` IDs, a generic definition and optional evidence reminders. An encountered synonym may instead map to an existing ID. Normal understanding receives the seed plus source-matching learned types; classification validation uses the same catalog snapshot. Taxonomy selection and own-library search use the combined catalog. Flutter already renders arbitrary IDs and labels.

This avoids repeating **category discovery**, not the normal transcription/understanding call for each voice note. A previously unseen spelling, synonym or language can need its own discovery cycle. Uncertain phrases retain the descriptive fallback. Categories do not grant credentials, establish identity or imply a recommendation's unstated capabilities.

## Automatic checks

1. Enqueue only an extracted, source-backed descriptive type with no canonical type and no owner override. Deduplicate by normalized phrase and entity kind.
2. Reserve a bounded attempt before sending only the short type phrase, broad kind and generic catalog to the proposal model. No transcript, subject name, saved contact, quote or locality is included in this extra request.
3. Validate shape, text lengths, kind compatibility, collisions and reserved identifiers. New labels keep the input words. New parent links and unobserved synonyms are forbidden. Evidence reminders come from a fixed optional whitelist and cannot execute code, introduce headings, call tools or change permissions.
4. Make a separate model call to assess generic/private identity, meaning, duplicate scope, equivalent alias, examples and safe optional reminders. All boolean checks must pass. This is an independent call to the same model, not a human review or a guarantee of semantic correctness.
5. Publish only with unchanged registry version, live attempt lease and eligible source/permission/item snapshot. Save a generic proposal/review receipt and increment the registry version. Update only eligible pending recommendations' classification, preserving prose, rating, contacts and audience.

Models use `gpt-4.1-mini`, strict Structured Outputs and `store:false`. Schema compliance does not establish semantic accuracy. A probe found that both calls accepted an inappropriate hierarchy and broad alias; the final implementation prohibits those model-generated expansions. Definitions still need ongoing quality evaluation. Do not treat the six-case smoke test as an external rollout gate.

Optional reminders preserve explicitly stated work, duration, price basis, compatibility, limitations, service area, venue, variant, accessibility or attribution. They are data within the existing understanding prompt, never mandatory fact slots or model-authored system instructions. General editorial rules continue to govern all recommendations.

## Failures, privacy and concurrency

The worker runs separately from recording/transcription. Accepted results are reused; rejected/provisional phrases are cached without repeat provider calls. Provider errors or a changed catalog allow one delayed retry. Source deletion, account deletion, withdrawal/re-enable generations, owner editing or changed classification invalidate private discovery pointers and late updates. Automatic contact attachment can advance item revision without invalidating an otherwise unchanged category. Accepted generic definitions remain shared independently of the source account; private discovery pointers cascade on deletion.

A review is skipped if eligibility has changed after proposal. Already-dispatched provider requests cannot be recalled. No transaction waits for model output. Claim/publication locks, attempt tokens, leases and revision checks handle competing workers. Registry events store only accepted generic proposals and suspension receipts; rejected model payloads and raw notes are not logged or persisted in shared tables.

Pilot limits:

| Limit | Value |
| --- | --- |
| Attempts per account / rolling day | 4 (at most 8 calls) |
| Attempts globally / rolling day | 12 (at most 24 calls) |
| Attempts per normalized phrase + kind | 2 total |
| Provider timeout / job lease / retry delay | 40 seconds / 3 minutes / 15 minutes |
| Private pending pointers per owner | 30 admission threshold |
| Pending jobs with source pointers | 1,000 admission threshold |
| Full-catalog review capacity | Fewer than 500 concepts |
| Understanding catalog | Seed plus matching learned types/ancestors, capped at 100 |

Admission thresholds are best-effort under simultaneous saves; provider-attempt budgets are transactionally enforced. When capacity or budgets are reached, the saved recommendation remains usable. Backlog processing does not block its save. Hash-only orphan jobs can remain after pointer removal but do not consume backlog admission capacity. There is no paid historical backfill, automatic merge, automatic reactivation or automatic assignment to a human reviewer. Broader scale needs indexed registry retrieval and measured quality/cost work.

## Operation

Apply backend migrations, then enable `CATEGORY_LEARNING_ENABLED=true` alongside the existing provider key. The account's extraction/processing permission must still allow the work. The flag defaults off. Restart the backend to change it. Disabling the flag stops the worker; previously accepted categories remain available.

Operator maintenance, from the backend directory with its database environment:

```sh
cargo run --example category_admin -- suspend learned.CATEGORY_ID
cargo run --example category_admin -- suspend-alias 'encountered synonym'
```

Suspend child categories first if any exist. Suspension bumps the registry revision, stops future use and reserves the old ID/alias against automatic recycling. Requests already using an earlier catalog snapshot may finish under that snapshot; suspension is not a recall of in-flight understanding. Historical recommendation snapshots are retained; the owner can keep an already-assigned retired type when editing unrelated fields. There is no automatic historical repair or reclassification. Correcting a meaning or retrying a provisional concept requires a deliberate reviewed registry change rather than deleting the budget ledger.

## Verification — 2026-09-27

Database tests use `rekky_category_learning_test`, never the running app database. They cover competing discoveries, cache reuse, seed synonyms, suspension, permission/source/item races, owner overrides, preserved contact/rating/audience, retry/lease limits, spending caps, dynamic extraction validation and signed-in taxonomy/search. Existing backend API tests run separately against `rekky_taxonomy_test`. Two Flutter tests use the shared learned-category fixture to verify card/detail rendering and stable-ID editing without a mobile code change.

[Full synthetic probe record](./evaluation/category_learning_probe_2026-09-27.json) includes failures, revisions and final outputs. The final six-case run accepted Clock repairer, Puzzle box and Birdwatching walk, reused Taxi service for cab provider, rejected airport pickup during review and deferred vague praise. Accepted proposal + review times were 2.843–3.469 seconds, outside the ordinary save path; queue delay is additional. No real user transcripts were submitted for these probes and no synthetic categories were installed in the live registry. Physical-device creation of a new learned category, representative multilingual quality and wider-scale load remain unverified.

Final checks: 71 Rust tests passed, including both isolated databases; Clippy with warnings denied and the release build passed. All 24 focused Flutter category/editor/reading tests and Flutter analysis passed. Live local migrations 013/014 were applied, the worker flag enabled and the backend restarted on port 3089. Health and Android forwarding checks passed; the live registry and discovery queue remained empty, confirming no historical backfill or probe publication. No mobile binary change was necessary.
