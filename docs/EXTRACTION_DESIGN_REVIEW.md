# Recommendation extraction design review

2026-09-27. **Design review with implementation checkpoint below.** This reviews the current Rust pipeline against sections 4, 6 and 12 of the [canonical product plan](REKKY_FLUTTER_PRODUCT_PLAN.md). The baseline findings below describe the pre-change implementation. The canonical plan records the approved implementation and the deliberate change to per-item sharing readiness.

## Finding

The current implementation treats too many failures as rejection of an entire capture. It also uses literal checks and sentence-reference coverage as proxies for semantic fidelity. Those checks can reject harmless normalization while accepting changed meaning. Making the prompt longer or turning validation off would not solve this.

The product needs a useful, faithful saved recommendation with optional structured details. The plan already calls for a small stable spine, flexible observations, independently validated optional actions, selective continuation and measured semantic quality. The implementation has not fully realized that separation.

## Evidence from the implementation

- `src/extraction.rs::validate_with_catalog` returns one `Result<(Vec<ValidatedItem>, bool)>`. One bad subject, paragraph, location or evidence reference aborts every item, including earlier valid siblings.
- `src/extraction.rs::preserve_unresolved` then picks the first supported subject and saves the entire capture as its body with `recommendation: null`. Source text survives, but identity/category structure and separation between subjects do not.
- `src/app.rs::process_voice_capture` treats that fallback as a completed extraction job. There is no automatic field repair. `LibraryShelf.of` maps missing structure to Notes.
- All items inherit the capture's single `partial` flag. The UI cannot distinguish a possible missing warning from a conservative location-role downgrade or harmless repetition. The source-only fallback discards the rejected proposal; errors are reduced to `ExtractionError::Failed`.
- A location's display text must also be an exact contiguous phrase in the cited source. Number tokens and currency symbols must be literally present. These checks conflate source evidence with display normalization.
- Coverage counts sentence IDs, not preserved meaning. Every non-verbatim repetition needs representation, but merely citing a sentence can account for it even when a caveat inside it was omitted. English role/negative-word heuristics are not general multilingual understanding.
- Ratings, taxonomy assignments, retrieval phrases and readable-source corrections already have more useful behavior: unsupported optional output can be omitted or fall back locally. However, taxonomy validation runs after the prose checks, so it never gets to preserve a valid Restaurant classification when another field aborts the item.
- The prompt combines subject splitting, prose, sentence accounting, category IDs, location roles, ratings and copyediting. Several requirements address prior individual failures. Account paragraphs are constrained to three source-unit references each. This couples natural writing to sentence segmentation. The exact contribution of prompt complexity to model error has not been isolated.

A read-only local snapshot contained 13 extraction jobs, 6 marked partial; 5 of the 11 live voice items inherited a partial capture status, and one live item retained a source-only fallback. These include different pipeline revisions and explicit repairs. They are **not a model rejection rate**, and partial does not necessarily mean invalid output. Historic rejection reasons cannot be reconstructed reliably.

The earlier [category audit](CATEGORY_EXTRACTION_AUDIT.md) separately reproduced instruction/schema bias toward unrelated categories. The [ratings development check](RATINGS.md) recorded rejection after an unambiguous number-word-to-digit rewrite. These are different failure families; a larger category list does not address the current capture-level fallback.

## Offline validator probes

Six invented proposals were passed directly to the current Rust validator and fallback on 2026-09-27. No provider request or real source was used. These are deliberately constructed checks of validator behavior, not measurements of how often the model produces these outputs.

| Input / proposed output | Observed behavior | Implication |
| --- | --- | --- |
| Source names Lantern Cafe “in Delhi, in Malviya Nagar”; location output is “Malviya Nagar, Delhi” | Rejected; source-only fallback | Equivalent display ordering destroys otherwise usable structure |
| Source says “seven hundred rupees”; prose says “700 rupees” | Rejected; source-only fallback | Unambiguous numeric normalization is treated as invention |
| Source says “I did not like the pizza”; proposed prose says “Loved the pizza” with valid references | Accepted, not partial | Citations and literal-number checks do not establish semantic support |
| One source sentence praises pizza but says service was painfully slow; prose retains only the praise | Accepted, not partial | Sentence-ID coverage does not establish caveat coverage |
| Source separately says “I loved the pizza” and “The pizza was really good”; account compresses them into one statement without citing the third unit | Accepted but partial | Repetition can trigger a review flag without omitted useful meaning |
| First restaurant is valid; a second restaurant changes its price from 200 to 900 | Entire capture rejected; fallback contains one unstructured item | A sibling's failure removes a good item's structure and combines the source under one title |

## Recommended design

### 1. Separate the source, the recommendation and enrichment

Keep the unchanged private transcript as the source of record. Produce a small item draft with a local item key, supported subject/subject references, broad kind, attributed reading account and independent optional fields. Unknown values are allowed; they are different from processing errors.

Keep three distinct representations:

- **Evidence:** unchanged source references, important qualifications and item association.
- **Reading:** one concise account, naturally worded, with references to its supporting material. No duplicated summary/fact checklist in the UI.
- **Structured facts and interpretation:** canonical category, typed location/amount when supported, optional rating and retrieval facets, each with provenance. Maps identity and saved-contact attachment are separate enrichment results.

The normal call can propose these together. Logical separation does not require a chain of category, writer and reviewer calls. Avoid a universal required fact template: an unfamiliar recommendation still needs only an identity and useful account. A new type can use a supported descriptive label immediately and enter the existing bounded category-learning process later.

Make category acceptance depend on the subject and relevant source, not on whether every prose paragraph passed. Derive compatible broad shelves from accepted canonical types where unambiguous; contradictions about which subject the type describes require review. Do not find a restaurant word anywhere in a multi-subject capture and attach it indiscriminately.

### 2. Return accepted content and specific issues, not all-or-nothing success

Replace the aggregate result with an assessment containing per-item accepted fields, rejected fields, unresolved source material and typed issues. An issue needs a code, item/field path, source references, materiality and recovery action. This is a design contract, not model-authored policy.

| Issue | Behavior |
| --- | --- |
| Invalid ownership, revoked permission, stale source/item revision or exhausted budget | Stop the operation; preserve the existing server-side fences |
| Malformed provider envelope / unusable JSON | Preserve the source and record a distinguishable provider/parse failure; bounded retry or deferred recovery |
| Cosmetic format or unambiguous normalization | Normalize deterministically; no user review flag |
| Unsupported optional inferred rating, unresolved canonical type or failed Maps enrichment | Omit/defer that field; retain the useful recommendation |
| Invalid evidence for a substantive price, caveat, attribution or other experience detail | Hold or repair the affected content; keep independently supported identity/category and other items |
| Unclear subject boundary, reversed sentiment, conflicting source or possibly missing material meaning | Keep the affected item private with a specific issue and attempt bounded repair |

An optional schema field can contain an essential user claim. Dropping a faulty *estimated* rating is different from losing an explicitly spoken score or warning. Reassess the remaining account after any field is removed. Do not display/publish its positive opening as complete if the rejected paragraph might contain the qualification that changes it. Where a safe item-specific source excerpt can preserve that meaning, use it provisionally; never dump the entire multi-subject transcript into the first item's shareable account.

Reserve Notes for genuinely unclassified content. A restaurant with a questionable price remains a restaurant. If identity itself is unresolved, show private recovery in Recently added rather than pretending classification succeeded.

### 3. Validate equivalent values separately from wording

For locations, retain original mentions, their role and source context separately from canonical geographic identity and display name. Independent mentions of Delhi and Malviya Nagar can resolve to a composed locality without requiring that exact composed phrase in the transcript. The resolver must establish the relationship; proximity or a city name elsewhere in the recording is insufficient. Failed resolution leaves a spoken locality, not a failed recommendation.

For amounts, retain the source phrase plus parsed value, currency if explicit, approximation, time and scope. Supported forms such as “seven hundred rupees” and “700 rupees” can compare equal. “700 bucks” does not alone establish INR; an unknown currency stays unknown. Ambiguous number/date formats retain their original wording. Normalizing a value must not lose “about,” “I think,” a historical qualifier, negation or a dish-versus-total distinction.

Length/format problems are local presentation issues, not reasons to erase categories. Use supported schema limits and local repair; do not truncate a sentence in a way that deletes its caveat. Evidence-reference bounds should protect resource use without forcing one paragraph per sentence.

### 4. Treat semantic fidelity as a measured language task

Keep references for traceability, but stop calling referenced-sentence coverage proof of completeness. Track important source assertions and qualifiers against the account, including contradictions, attribution and multi-subject boundaries. Allow repeated statements to map to the same represented meaning.

Use a compact evidence map for material meaning, not a second verbose generated account or an exhaustive category-specific form. The model may propose the map in the same call; that remains a model judgment. Deterministic checks should verify references and known normalized values and flag contradictions they can actually recognize. They cannot establish general entailment or multilingual semantic equivalence.

Use targeted semantic review/repair for materially uncertain cases. Evaluate false negatives with independently labelled examples and separately consented quality review, including apparently successful outputs. A second model's approval or self-reported confidence is not proof. Compare the normal one-call path with a separate semantic-check variant before fixing the production policy; if a second check demonstrably improves fidelity enough to justify its latency, use it for the measured scope. Do not assume either that every capture needs two calls or that one call is sufficient simply because it is cheaper.

### 5. Recover automatically, with a small shared budget

Keep one normal understanding request as the initial target. Try deterministic normalization first. If a material problem remains, allow one automatic repair call for the capture in the initial rollout, counted inside the existing attempt/spend ceilings. Batch affected fields/items into that call; do not multiply retries by category or optional field.

Send the issue codes, rejected candidate, relevant unchanged source context and accepted item identity. Include neighbouring qualifiers and unresolved material when needed; positive excerpts alone cannot resolve a missing warning. Revalidate the returned patch and its dependencies. Rewriting an opinion can invalidate a carried inferred rating; changing subject association can invalidate contact or place matching. Background repairs must preserve owner edits, item IDs, source revisions and visibility overrides.

If meaning is missing from the transcript, more calls cannot recover it. After the bounded repair, preserve a useful private item plus a specific unresolved issue; do not loop, invent certainty or ask the user to manage internal pipeline stages. Provider failures, malformed output and semantic repair share a visible operation ledger and do not get independent unlimited retry budgets.

### 6. Separate processing status, content quality and optional enrichment

Internally distinguish the state of the job from the readiness of each item and the status of its optional enrichments. A completed HTTP/model request is not equivalent to a faithful saved recommendation. A missing Maps match or absent rating is not incomplete understanding.

Track unassigned source material at capture level. Independent, clearly separated good items can remain usable when a sibling needs review; ambiguous boundaries can still hold the affected set. Changing publication from today's capture-wide hold to item-level readiness is a deliberate product-plan change and must be covered by sibling/source-isolation tests.

Friends remains the intended default for ready new recommendations. Materially unresolved content stays private. Optional-field absence alone should not create a private hold. Preserve both intended audience and any explicit owner override; an automatic repair cannot overwrite a Private choice. Keep existing partial items private during rollout, since their historic flags do not record enough detail for safe automatic promotion.

The user still gets one-tap capture, a useful Library item and optional later enrichment. Only a material, unresolved issue earns the small review icon, whose explanation names the actual concern. Filler compression, formatting and optional fields should not generate alarming review chores.

### 7. Make failures diagnosable

Persist prompt/schema/validator/model versions, provider request/outcome metadata, issue codes, repair count, stage timings, token/cost data and final disposition. Separate timeout, quota, truncated output, parse failure and content failure. General logs contain codes and IDs, not notes.

Keep any rejected candidate needed for debugging in bounded, access-controlled source-linked storage with deletion/expiry behavior, rather than retaining private content indefinitely or publishing it in evaluation files. Store concise reasons even when raw candidates are no longer available. A future “why did this become Notes?” investigation should not require guessing.

## Implementation order and acceptance evidence

1. **Diagnostics and a replayable offline baseline.** Introduce typed validation reasons first; establish labelled cases and capture current outcomes before changing behavior.
2. **Contain failures.** Assess items and fields independently; preserve identity/category, prevent mixed-subject fallback, and separate optional absence from material incompleteness. Add numeric/location normalization with explicit provenance.
3. **Simplify the generation contract.** Align prompt/schema with the new behavior, remove instructions compensating for literal display comparisons, keep one reading account and introduce only the evidence mapping needed for material fidelity.
4. **Bounded repair and states.** Add automatic targeted recovery, version/revision fences and specific review reasons. Keep old saved items unchanged by default; separately repair known affected items through a controlled path.
5. **Compare on the plan's quality set.** Evaluate the new one-call path and targeted-review variant, including latency and cost. Expand beyond restaurant examples before calling this solved.

Use the plan's 60 independently authored scenarios and held-out split, including English/Hindi/Hinglish, unfamiliar types, loose wording, numbers, repeated praise, mixed sentiment, same-name subjects, ambiguous locations and transcription errors. Keep production recordings outside the corpus without separate permission. Include audio-to-transcript evaluation because perfect extraction cannot correct a misheard name by itself.

Measure useful structured-save rate, unnecessary rejection/review rate, category retention, material-claim/caveat retention, unsupported assertions, item association, repair success, one-call completion rate, and end-to-end p50/p95 latency and cost. The existing 95% material-retention progression target and zero critical warning reversals/wrong-person/access failures in the regression suite still apply; small samples are not production guarantees. Establish numerical false-review and latency targets from the baseline rather than inventing an achieved threshold.

Mandatory regressions include every probe above; unknown types saving immediately; an omitted rating not hiding a restaurant; a rejected price not deleting a sibling; a lost caution remaining a material issue; ambiguity staying unresolved; and stale repair results never overwriting owner edits or changing audience. Merely increasing the acceptance rate is not success.

During the initial investigation, no production code or saved data was changed. The six probes ran offline against the existing validator. The exact cause of the latest historical rejection remains unavailable because its rejected output and reason were not retained.


## Implementation checkpoint

Normal extraction and explicit refinement now use `understanding::assess`; the former strict v2 validator remains only for historical comparisons/tests. The new assessment preserves supported fields, typed reasons and safe source-scoped recovery, accepts bounded number/location equivalence, and keeps distinct items separate. Per-item `quality` controls automatic sharing and the existing quiet review explanation. Unassigned source holds affected output privately instead of being appended to an arbitrary subject. One durably reserved automatic repair can fix affected content or recover a missing distinct subject; it counts within the existing three-attempt limit and cannot overwrite completed owner edits.

The v3 prompt removes the narrow paragraph-citation cap, separates optional classification from prose and clarifies provider-versus-offering categories. It does not create a universal category-specific form. Missing ratings remain optional; unsupported category assignments can retain their independently supported descriptive label.

The deterministic checks remain deliberately limited. Related paragraphs can collectively represent a source unit; pure metadata sentences need not be repeated in prose. Narrow warning/uncertainty signals can trigger review, but they are not a semantic entailment engine. A full claim-level semantic map, independent checker comparison and the planned held-out corpus are still evaluation work. The final implementation must not be described as proving complete understanding or multilingual fidelity.

Private diagnostic candidates are source-linked, excluded from item/search responses and purged by the running worker after seven days; issue metadata remains until source deletion. Provider parse, timeout, rate-limit and incomplete-response errors have separate codes. Existing historical rejection reasons cannot be reconstructed and old items are not silently backfilled.

Development verification used six fabricated direct-validator cases during the audit, additional automated assessment/API regressions and synthetic provider probes. Live probes exposed false local caveat flags, metadata repetition, and a repair that found omitted subjects but could not merge them. Those failures were retained for offline replay and informed regression fixes. Final replay of the six captured synthetic examples produced eight structured items: five captures needed no repair and the three-subject capture recovered through one recorded repair response. This is offline replay of known development outputs, not six new final-prompt calls or a held-out accuracy measure. One captured repair also classified a named lesson provider as an activity; the final prompt clarifies that boundary. Broader category/attribution correctness still needs the independent evaluation described above.

A fresh final-prompt call for the three-subject synthetic case returned all three subjects without repair, with Arun and Meena both classified as people/services. Its visible account preserved the leaking-tap warning and the secondhand, untried lesson recommendation. This is one additional development check, not a general category or semantic-quality result.

Final verification: 103 Rust tests passed with the isolated PostgreSQL database configured, including 29 API integration cases; 134 Flutter tests passed. Clippy with warnings denied, Flutter analysis and format checks passed. Release backend and Android debug APK built; migration 019 applied to the local Rekky database, the backend restarted healthy, and the app installed and launched on the connected physical phone. Saved recommendation and source checksums matched before/after migration and restart. No historical transcript was resubmitted and no existing recommendation was backfilled. The new extraction behavior was exercised with automated API tests and synthetic text provider calls, not a fresh real phone recording; representative audio, iOS and production latency/cost remain unverified.
