# Personal ratings

Decision: 2026-09-27. Automatic ratings describe an author's opinion, not an external review average. The existing `gpt-4.1-mini` understanding request proposes the score alongside extraction. There is no additional request or interactive capture field. Only the expanded sheet shows it, near the category and before locality/Maps; an inferred rating has a small **Estimated** marker with accessible explanatory text. Owners can change or remove it in Edit recommendation.

## Rubric and provenance

| Score | Overall firsthand opinion |
| --- | --- |
| 1 | Awful; extreme dissatisfaction, strong warning to avoid |
| 2 | Very bad; little redeeming value |
| 3 | Bad; clearly disliked, substantial problems |
| 4 | Disappointing; negatives outweigh positives |
| 5 | Mixed; middling overall |
| 6 | Okay; acceptable, limited enthusiasm |
| 7 | Good; ordinary positive recommendation |
| 8 | Very good; strong positive experience |
| 9 | Excellent; outstanding, very strong recommendation |
| 10 | Exceptional personal favourite; highest enthusiasm |

Abstain for unknown/neutral opinion, hearsay, untried interest, unclear attribution, unresolved sarcasm, conflicting/negated scores and aspect-only ratings. A 5 is not an uncertainty bucket. Weigh the whole experience and material caveats; do not use repetition, category, price or length as score signals. Never assign a single recording's score to every subject.

An explicitly stated personal overall score takes precedence, with `origin=spoken`; inferred whole-number scores have `origin=inferred` and `rubric_version=1`. Owner-selected values use `origin=user`. The initial spoken parser recognizes English digit/number-word `out of five`, `out of ten`, numeric `/5` and `/10` formats, with explicit personal/overall attribution. Scores out of five are doubled for display out of ten, with the original value and scale retained in `spoken`; /10 is unchanged. Other scales and unsupported phrasing remain prose without an inferred replacement. Hindi/Hinglish sentiment can be interpreted by the model, but spoken-score normalization is not yet multilingual or comprehensive.

## Contract and lifecycle

`recommendation.rating` is absent/null or `{value: number, scale: 10, origin: "spoken" | "inferred" | "user", rubric_version?: 1, spoken?: {value: number, scale: 5 | 10}}`. Range is 0–10; zero is a real score and never a missing sentinel. No database migration: this is additive v2 JSON. Shared examples live in `contracts/rekky/v1/fixtures/ratings.json`. Source phrases/evidence remain exclusively in existing owner-only source support, not the public recommendation or search body.

The model's optional proposal also includes stance, literal source phrase and item-scoped evidence. Invalid fields, unknown origins/stances, out-of-scope support or unsupported numeric scales drop the score without failing extraction. Rating evidence does not count as coverage of missing facts. Checks on explicit indecision and invalid score scope catch known failures, but literal evidence and English patterns cannot prove semantic correctness.

`PATCH /v1/items/{id}/content` accepts `rating: {mode:"keep"}`, `{mode:"none"}` or `{mode:"set",value:number}`. Omitted rating defaults to keep for older clients. Server validation rejects out-of-range values and spoofed origins. Keep retains provenance, unless subject/kind/experience changed; an automatic rating is also cleared when summary/observations/attribution changed. The editor explains that clearing before save. A deliberate set wins and uses user origin. Removal and edits inherit existing owner/revision checks and protection from automatic refinement. Unrelated audience, category, link or locality edits do not confirm an AI estimate as the owner's score. All rating derivation commits with its item under existing source/permission/job fences.

Older notes remain unrated until the owner sets a rating or a separately requested reprocessing flow updates them. Opening a sheet never processes a transcript. No historical user transcripts were sent for this implementation. Ratings are not added to current Ask ranking or aggregated across people.

## Provider choice and verification

Jev's [Score primitive](https://docs.typesafe.ai/primitives/score) can judge ordered descriptive criteria and return probabilities/confidence. It is a reasonable candidate for a later comparison. Its raw score uses zero-based criterion indices, so it is not a ready-made ten-point rating; define abstention and an explicit mapping. Model confidence is not measured correctness. No Jev key/provider integration or new data recipient is introduced here.

The current baseline avoids a second network request and independent transcript disclosure. Total token cost and latency still depend on the enlarged output; no measured speed advantage over Jev is claimed. The synthetic probe is reproducible with `cargo run --example understanding_probe -- --ratings`; optional `PROBE_CASES` selects comma-separated case IDs. Human-labelled real audio, held-out Hindi/Hinglish cases, sarcasm, rating scope and user correction rates are still needed before a broader quality gate.


Development evidence (2026-09-27): thirteen invented transcripts cover fourteen subjects, with expected score bands/abstentions recorded before the /10 probe. Twelve transcripts validated initially; the negated-score case was rejected because generated prose converted spoken number words into digits. A targeted prompt correction and one recheck saved that note without a rating. Accepted scores fell within the seed bands and the neutral/hearsay/undecided cases remained unrated. These are development examples, not held-out accuracy or a production quality gate. Full understanding requests in the initial /10 probe took 2.737–4.207 seconds; the targeted recheck took 3.579 seconds. This measures neither incremental rating overhead nor audio-to-library latency. Earlier /5 development probes exposed uncertainty/aspect-scope errors and informed the abstention guards. [Synthetic output and recheck](evaluation/ratings_probe_2026-09-27.json).

Validation: the 55-test Rust suite passed against the isolated database; after adding explicit /10 normalization coverage, all 16 affected rating/understanding tests passed (56 tests total in the suite). Clippy with warnings denied passed. Flutter analysis and all 88 tests passed, including invalid/absent/zero scores, estimated provenance, score editing/removal/retention, large text and compact previews. The Android debug APK was built and installed on the connected phone. The final backend release build passed and the updated local server was restarted successfully; no real transcripts or saved ratings were changed by these checks. iOS runtime and held-out human preference calibration remain unverified.
