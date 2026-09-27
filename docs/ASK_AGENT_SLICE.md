# Intelligent Ask: first integrated pilot

Implemented 2026-09-27. This is a working portion of ASK-0/1, not completion of the first Ask release or its evaluation gate. The [Ask plan](ASK_EXPERIENCE_PLAN.md) and [canonical product plan](REKKY_FLUTTER_PRODUCT_PLAN.md) remain authoritative.

## What works

A typed question reaches an owner-scoped Rust agent. `gpt-6-luna` chooses native function calls to search, inspect saved evidence, investigate again or present a structured answer. Flutter renders useful candidates, a brief explanation, relevant caveats, a separate Worth checking section and a source-evidence sheet. One optional clarification can accompany results. Clicking a clarification submits the original question plus that choice; this is not yet full conversational state.

The answer can recover a partial memory or find options for an occasion across categories. It opens the current recommendation independently of the loaded Library, and derives Call/Maps actions from stored fields after re-fetching the item. The model cannot supply an action URL or phone number. The old `/v1/ask` endpoint remains for baseline regression; the app uses the new agent route.

Native layout preserves the existing typography, neutral surfaces, blue Ask accents and navigation capsule. A new question keeps the previous answer visible until replacement succeeds. Cancellation ignores late responses and prevents subsequent agent decisions. Retry after a lost response reuses the request UUID; a definitive failed operation starts a new one. Results page eight at a time rather than draining every page before rendering.

## Retrieval and reasoning boundaries

The current retrieval tool tokenizes the model's English/Hindi/Hinglish search alternatives and ranks owner-only lexical matches, including taxonomy search projections. The model judges those candidates against saved evidence. There is **no embedding index or separate reranker yet**. Early probes exposed a real defect: phrase-as-substring search missed useful records even when the model chose sensible clues. Splitting phrases into retrieval clues fixed those development examples.

`search_knowledge` returns eight candidates plus an explicit next page; `inspect_evidence` accepts only seen IDs; `present_answer` cites seen item IDs and evidence IDs. Up to three decisions and four read calls bound each operation. The agent chooses when another search is useful. A larger library can exceed that investigation budget: `search_incomplete` conservatively signals any truncated candidate page, and the UI suggests narrowing the question. There is no claim of exhaustive retrieval. Resumable investigation across that boundary remains open before the ASK-1 gate.

Server validation removes unknown IDs/evidence, duplicates and invalid result entries without discarding valid siblings. This validates references, not the semantic truth of arbitrary prose. The prompt distinguishes suitability from evidence supporting a negative statement: a known mismatch must be excluded, not displayed as an option with a warning. The live-model regression exercises that distinction; broader semantic quality is still a release gate.

Explicit discovery locations resolve through the local geographic catalog, then filter against saved geographic IDs with venue/practice/service-area roles. Unknown geography becomes Worth checking; known mismatches are excluded. Fuzzy recall does not invent a service-area constraint from a past journey. No device location or public-place facts are silently inferred.

## API, state and privacy

- `POST /v1/ask/agent`: `{request_id: UUID, question: string}`; trim and bound question to 500 characters. Same owner/UUID/question returns the original answer without another paid run. Changed input conflicts.
- `GET /v1/ask/answers/{id}?offset=8`: current, owner-authorized page; offsets are multiples of eight. Answer references expire after 15 minutes.
- `DELETE /v1/ask/answers/{id}`: cancellation, including a bounded tombstone for cancellation racing admission. An in-flight provider request may finish, but no late result is committed and no later decision starts.
- `GET /v1/items/{id}`: current owner-only recommendation, used for opening and action revalidation.

[Shared answer fixture](../contracts/rekky/v1/fixtures/ask_answer.json) defines the mobile-facing shape. Results contain an existing item projection, section, reason, caveat and cited saved evidence. `mode: limited` explicitly marks an interrupted/incomplete agent answer. `changed` removes stale item revisions; the client replaces prior pages when invalidated.

Tool access is owner-only SQL. Friends' notes, raw retained transcripts, source-support payloads and structured contact fields are not agent inputs. Relevant derived saved content and the question are sent to OpenAI; the entry screen and processing settings disclose this. This pilot reuses the account's existing understanding permission and global withdrawal fence. There is no per-question permission dialog. Phone/contact text already embedded in an ordinary saved body is not independently redacted by this adapter.

Session validity and permission generation are checked before every provider decision and before saving. Item revisions are checked before redispatch and on answer hydration; deletion/edits invalidate corresponding evidence. Account deletion cascades the answer records. Response payloads use `store:false`. No private question/tool/provider bodies are logged. Answer content is swept after 15 minutes; usage receipts survive up to 30 days. This is temporary answer recovery, not durable session history or offline support.

`OPENAI_ASK_ENABLED=false` is the default in the example configuration. Local testing enables it explicitly. Admission is serialized with a Postgres advisory lock: one active run/account, 30 runs/account/24h and 500 globally/24h. Each reserves $0.02; cancellation tombstones reserve no spend and cannot consume the global paid allowance. The overall deadline is 50 seconds, each provider request 18 seconds, each response at most 2,000 output tokens, and request bodies at most 47,000 UTF-8 bytes. Three such calls fit within the reservation at the model's current standard input/output prices. Abandoned runs become failed after one minute; unknown provider outcomes retain their reservation. No paid automatic retries. Revisit budgets if pricing/model changes.

Provider implementation follows the official [Responses function-calling contract](https://developers.openai.com/api/docs/guides/function-calling) and [gpt-6-luna documentation](https://developers.openai.com/api/docs/models/gpt-6-luna). Reasoning effort is `none`; future model/effort changes must earn their cost in the same evaluation.

## Development evidence and remaining gates

The checked-in [development probe](evaluation/ask_agent_probe_2026-09-27.json) contains invented notes only. Six questions cover occasion discovery, Hindi fuzzy recall, Hinglish city restriction, no suitable professional, unknown catering capacity and an outdoor activity. The final development run found all four specified positive targets, rejected all four explicitly excluded targets, and kept unknown larger-event capacity out of supported results. Timings were 3.28–5.17 seconds. These are tuned development cases, not held-out accuracy or production latency statistics. Earlier runs exposed phrase retrieval failures, a synthetic geographic-ID fixture error, and a model returning an excluded provider as a supported negative statement; the final prompt/tool changes address those observed cases without proving universal correctness.

Deterministic checks cover cross-owner denial, idempotency, revision invalidation, permission withdrawal, cancellation racing admission/in-flight work, forged evidence, geographic roles and mobile stale-response handling. Full regressions passed: 108 Rust tests (the explicit live-model test is normally ignored) and 162 Flutter tests. Final Ask-specific tests, Rust Clippy with warnings denied, Flutter analysis, release backend build and debug APK build also passed. Flutter layout checks cover 320/375/414/768 widths with large text. The build was installed on the connected Android phone, the local backend migrated/restarted with Ask enabled, and the dark-mode entry/navigation inspected on device; the live answer checks use the isolated test database, not the owner's private library.

Still required: the planned 120 labelled queries/40 varied source families with truly held-out English/Hindi/Hinglish partitions; same-set baseline/fixed-pass/adaptive comparison; semantic retrieval measurement; continuation through larger libraries; prompt-injection/adversarial and ambiguous-geography coverage; source-to-Ask real-audio evaluation with separate consent; voice questions and full refinement (ASK-2); comparison (ASK-3); durable history, shortlists, preferences and friend search in their later slices. No complete delivery phase is marked passed.
