# Why unfamiliar-type discovery was skipped

A real caterer recommendation retained the broad People & services shelf. Its saved extraction proposal had unrelated type/cuisine assignments and an empty `type_description`. Evidence validation removed the unsupported assignments correctly. The discovery queue required a valid descriptive type, so no learning job ever existed. This was an extraction-to-discovery handoff failure, not a rejected new-category proposal. Private source details are not reproduced here.

## Request audit

The pre-fix request at commit `5ab18a5` used a fresh Responses API call with the intended transcript units, the generic vocabulary and `gpt-4.1-mini`. There was no previous conversation, cross-account source lookup or response-to-category remapping in this path. Stored proposal fields come from the parsed provider response before taxonomy validation. The actual historical provider response metadata was not retained, so we cannot reconstruct its exact returned snapshot/request ID retrospectively.

Two configuration defects were found:

- Instructions said the classification must use only the canonical vocabulary and appended “use canonical IDs, not invented labels.” This overbroad wording conflicted with the separate instruction to preserve unfamiliar explicit type phrases. The fallback was requested only if the model decided no known type fit.
- The strict schema independently enumerated every known concept ID and every alias in that dimension. It allowed a product ID with a cafe alias, or one cuisine ID with another cuisine's alias. It also offered aliases absent from the transcript. An unknown type could not be represented in those enum fields, although the correct empty-array plus descriptive-type path was structurally available.

The prompt did explicitly permit empty arrays; the schema did **not** literally require a wrong category. The hypothesis was that ambiguous instructions plus an over-constrained, unrelated menu biased the model toward a known choice. We tested that rather than assuming it.

## Controlled comparison

Twenty synthetic requests used two invented service descriptions, two repetitions and five variants. All returned `gpt-4.1-mini-2025-04-14`. The model, generation limit and source-unit construction stayed unchanged.

| Variant | Caterer results, two runs |
| --- | --- |
| Original prompt and independent enum schema | Both chose Carpenter and omitted the descriptive type; one also used the unspoken alias cafe |
| Original prompt, source phrase changed to free text | Both retained caterer, but both still proposed Carpenter with empty evidence |
| Revised prompt only | Both retained caterer with no unrelated canonical assignment |
| Revised schema only | Both retained caterer with no unrelated canonical assignment |
| Revised prompt and schema | Both retained caterer with no unrelated canonical assignment |

The bookbinder controls preserved that type in all variants; one free-source-phrase response additionally invented a second item and failed validation. The combined variant preserved both unfamiliar types across all four runs. This reproduces the failure family and supports a configuration cause; it does not prove which single instruction caused the historical response or establish general accuracy.

[Complete controlled outputs](./evaluation/category_prompt_audit.json). The executable `category_prompt_audit` reconstructs the old enum schema and accepts the baseline prompt file exported from the recorded commit. It reads no database. The prompt-only arm changes both the classification wording and the catalog wrapper; the schema-only arm changes both ID/alias pairing and source filtering. Those subcomponents were not individually isolated.

## Fix

- Limit the “canonical IDs only” rule to the actual ID arrays. Explicitly preserve an exact, cited subject-type phrase independently of those arrays, including when proposing a known ID. Leave it empty only when the source has no explicit type.
- Couple each selectable ID to its own aliases with a nested `anyOf` branch. Only aliases actually present as complete word sequences in the source are offered. If none are present, force that assignment array empty while keeping free, evidence-backed type descriptions available.
- Continue validating entity kind, exact alias, source unit and negation on the server. A phrase somewhere in a multi-subject note does not by itself establish which subject it belongs to.
- Preserve optional classification failure without losing the recommendation, and exercise the extraction-to-discovery queue handoff in a database test. This adds no extra model call on the ordinary capture path.

A further three synthetic whole-capture probes preserved caterer, bookbinder and violin repairer, while retaining Restaurant + Italian cuisine for a separate known place in the same recording. All three captures validated without partial status. [Outputs](./evaluation/category_capture_probe_v1.json). These are development probes, not a representative held-out gate; other extraction weaknesses, such as missing optional rating evidence, remain visible in the record.

For the affected item, a separate explicit maintenance command can restore only a missing, source-backed type under its current revision and permission, then enqueue the ordinary generic-term learning/review flow. It refuses existing category choices or owner edits, retains the original failed proposal for audit, and does not resubmit the transcript or modify recommendation prose, ratings, audience or partial status. This is targeted repair, not automatic historical backfill.

## Deployment and repair evidence

All 76 Rust tests passed, including source-constrained schema, invalid-assignment fallback, and extraction-to-discovery integration. Clippy with warnings denied and the release build passed. The local backend runs the corrected prompt/schema. The affected item was repaired from its already-saved exact type phrase; the ordinary learner accepted Caterer on its first attempt (one proposal plus one review) and stored a reusable generic ID. The phone Library and detail view were refreshed and verified to show Caterer. A before/after checksum excluding classification confirmed unchanged recommendation content, audience and partial status. The retained transcript was not sent again. No mobile code change was needed.
