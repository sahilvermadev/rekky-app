# Ask: from a loose intention to a useful decision

Status: approved product direction; first integrated ASK-0/1 pilot implemented 2026-09-27. Full slice/release gates remain open. See [implementation, evidence and limitations](ASK_AGENT_SLICE.md). This document expands the Ask sections of [the canonical product plan](REKKY_FLUTTER_PRODUCT_PLAN.md); the canonical plan owns access, consent, retention and release gates. ASK-0 through ASK-7 are implementation checkpoints within that plan, not a replacement phase system.

## 1. Outcome and principles

Ask helps a person recover a memory, discover an appropriate option, compare choices and act using their own and, when authorized, their friends' experiences. The signature moment is: “I hadn’t thought to look for that, but that is exactly why I saved it.”

The architecture is AI-native and agentic: an Ask agent owns the search strategy, adapts to evidence and chooses how to help the person decide. Application code supplies reliable tools, permission boundaries and native rendering.

People can express an occasion, partial memory, feeling, constraint or desired outcome without knowing an entity name or category. Rekky interprets the request, retrieves evidence, highlights useful connections and presents the appropriate native answer. It should anticipate the next useful step while making consequential assumptions easy to correct.

- One natural request produces useful material before optional refinement. A clarification is a tool for resolving an important ambiguity, not a compulsory questionnaire.
- Personal experience supplies the recommendation. Public facts can support a destination or current detail, with separate provenance; they cannot manufacture an endorsement.
- Show why a particular option fits this particular request. Preserve contrary evidence, caveats, attribution and unknowns.
- Give the task an appropriate answer shape while keeping controls, typography, actions and navigation predictable.
- Keep the person in control of constraints, durable preferences and external actions. Booking, contacting or sharing requires the corresponding explicit action.
- Every category can participate through supported observations. New categories must not require bespoke retrieval prompts or screen implementations.

## 2. Current starting point

Implemented: authenticated Rust/Postgres API, Flutter Ask and Library, one-tap voice recommendation capture, structured recommendations, editable content, category concepts/aliases, typed geographic projections, ratings with provenance, contacts, Maps actions and a curated Library.

The pre-agent baseline uses owner-scoped Postgres lexical search, exact-title priority and taxonomy matching. It returns item snippets and `answer: null`. It has no structured conversation, semantic retrieval, evidence-specific explanation, voice question input, comparison view or persistent personal preferences. The baseline Flutter client fetched every result page before returning the accumulated list, and opens results through the loaded Library list. The new pilot replaces both behaviors. Voice, semantic indexing, full conversation, comparison and durable preferences remain open.

The historical text seed is useful regression material, not a current benchmark or held-out evaluation. Its documented failures include Hindi retrieval, city constraints and same-name candidates. No complete delivery phase is declared passed.

## 3. Three signature journeys for the first Ask release

The following examples describe desired behavior, not factual claims about existing recommendations.

### Fuzzy recall

Request: “Who was that driver I used around Landour?”

Rekky uses role, geographic context, attribution and experience clues. A clear result leads with the person's identity, a short recognizable detail and an authorized Call action. If two drivers fit, present both and the distinguishing evidence. A useful clarification might concern the trip, without forcing the person to remember the driver's name.

Personal recall must not exclude a saved driver merely because current service coverage is unknown. Past-experience location and service area remain different fields.

### Finding something for an occasion

Request: “My parents are visiting. Somewhere nice where we can talk.”

Search supported qualities across relevant collections. Explain a fit using specific observations, such as a saved comment about conversation or atmosphere. Do not infer accessibility, cuisine, budget or older people's preferences from “parents.” If the choice between a meal and an activity would materially change the answer, offer that distinction alongside any useful preliminary results.

For “quiet Italian dinner in Delhi,” cuisine and explicit city constrain the answer; atmosphere must be supported, contradicted or unknown. A liked restaurant with a loud-music caveat must not be confidently recommended as quiet. Unknown atmosphere can appear as Worth checking, without pretending to meet the request.

### Comparing real options

Request: “Which of these two would work better for six people?”

Resolve “these two” from explicitly selected or unambiguous visible result IDs. Compare supported group suitability, location and relevant caveats in vertically stacked native rows. Missing capacity stays unknown. A conditional suggestion is allowed when supported: explain what makes one option preferable and what still needs checking. Estimated ratings cannot masquerade as explicit scores or automatically decide suitability.

The first release includes these three journeys, typed and spoken input, follow-up refinement and a current-answer history. Broad itinerary composition and durable personalization follow later.

## 4. The interaction

### Entry

Ask remains the home. Keep one prominent voice/text entry, the established navigation capsule and uninterrupted access to Recommend. An Ask microphone is explicitly for a question; it never creates a recommendation. No automatic listening on page entry. Allow dictation correction, stop and cancel. Submit the spoken question after the person finishes, with its transcript available to edit.

The empty state explains what the person can ask through a few short illustrative examples. Once real saved material exists, optional suggestions must be grounded in accessible knowledge. Do not fabricate personal context, infer the user's location silently or produce a daily feed of speculative recommendations. Recent sessions belong in a secondary entry.

### Answer surface

The request stays visible in a compact editable header. Under it, show a concise interpretation only when useful, plus editable constraints such as Delhi, Quiet or Own knowledge. Explicit constraints and suggestions must look distinguishable. A session preference override is removable without changing the person's saved preference.

Use one primary answer composition selected from:

| Shape | Contents and main action |
| --- | --- |
| Recall | Identity, recognition evidence, provenance, relevant Call/Maps/open action |
| Discovery | Brief orientation; strongest supported fit with a fuller preview; other qualifying results in a compact list |
| Comparison | Selected options with the same relevant dimensions, explicit unknowns and a conditional conclusion where supported |
| Clarification | One consequential choice with preliminary useful results where possible |
| Collection, later | An editable set for an occasion, with links to underlying recommendations and unresolved gaps |

Display each useful fact once. Avoid headings on every paragraph, generic “great choice” prose, inflated cards and routine confidence percentages. “Why this fits” can be one short sentence tied to evidence, expandable into the relevant saved passage and attribution. The private original transcript remains owner-only; network explanations use authorized recommendation evidence, not a friend's raw note.

All qualifying results remain reachable through pagination. The richer first preview is not a top-three display cap. Use an optional map only when the geographic arrangement helps; no map as default decoration. Provide equivalent accessible list information.

### Refinement

An always-reachable follow-up field accepts “closer,” “less expensive,” “keep this one,” or an entirely new question. At most three relevant refinement shortcuts can appear. They must change something meaningful; do not invent unsupported attributes to fill the row.

Refinement edits the current answer in place, with stable item IDs, scroll anchoring and a recoverable prior version. Keep the previous usable answer visible during work. A clearly separate question starts a new answer context. If “this one” is ambiguous, offer a choice rather than guessing. Explain a removed result briefly if a new hard constraint excludes an explicitly retained option; keeping an option does not override reality.

Do not silently loosen a hard constraint to avoid an empty result. Show the gap and offer an explicit alternative scope. If a user rejects an option, exclude it for this session; never convert that alone into a durable dislike.

### States and visual language

Use the existing neutral light/black surfaces, Fraunces/Manrope typography, blue Ask interaction accent and collection emblems. Preserve the red Recommend/yellow Library navigation roles. No new palette or chat-bubble template is required.

Opening Ask is immediate. During work, keep the input and existing answer usable, indicate the actual stage only when helpful, and show already validated results without fabricated reasoning or activity. New explanation text must not move a result being tapped. Provide cancel/retry and visible recoverable errors. For offline use, show cached own knowledge only when the later offline cache exists and mark its scope/age; until then, offer the retained current answer with an honest connectivity state. Do not imply offline search already works.

Respect 200% text, screen readers, keyboard navigation, portrait safe areas and reduced motion. Haptics use the shared vocabulary: deliberate selection and visible errors, no repeated buzzes as results arrive. Voice recording has an honest readiness cue and labelled controls.

## 5. Agentic Ask architecture

Use an evidence-seeking Ask agent with a bounded decision → tool → observation loop. It can choose a search strategy, pursue another clue, inspect contradictory evidence, change the answer composition, ask a useful clarification or stop. Query interpretation, retrieval and answer composition are capabilities it can use selectively, not an obligatory sequence of separate model calls or a fixed category workflow.

### Objective and working state

For each turn, give the agent the person's current request, acknowledged session constraints, selected result IDs, explicitly confirmed preferences when available, authorized scope, prior evidence references and remaining budget. Its objective is to produce a useful, supported decision or next step with minimal user effort. Keep tentative interpretations distinct from facts. The current request overrides preferences; neither can override access rules or manufacture a capability/location claim.

The working state records the current interpretation, unresolved questions, evidence examined, contradictions, excluded options, tool results and proposed answer. Persist structured operational state rather than hidden chain-of-thought. Resume a refinement from relevant state, invalidate stale references and do not resend the entire library or raw transcript archive on every turn.

### Tools the agent can choose

| Capability | Purpose and boundary |
| --- | --- |
| Search knowledge | Query exact names, lexical fields, category/facets, typed locations or multilingual semantic similarity inside the authorized scope; return paginated IDs, snippets, evidence references and matched/unknown constraints |
| Inspect evidence | Read relevant saved observations, caveats, attribution, confirmed destinations and field freshness for selected candidates; exclude raw friend transcripts and unnecessary contact values |
| Resolve a place expression | Resolve a user-mentioned city/area against available geographic identities; return ambiguity explicitly; device location requires permission and public calls count against the budget |
| Explore related knowledge | Find supported alternatives or complementary items across collections using the request and retrieved evidence; ordinary search projections suffice initially, with no speculative knowledge graph requirement |
| Propose clarification | Return one consequential question/choice and any already useful results; suspend until the user responds, with no live reasoning loop |
| Present or revise answer | Submit evidence-linked claims and a composition built from approved native components, result IDs, actions, constraints and stable section IDs; code validates before display |
| Propose remembered preference or saved collection, later | Produce a reviewable owner mutation that executes only after the corresponding explicit user action; normal search is read-only |
| Explore public information, later | Available only under the separate enabled scope, consent and provider gates; never silently invoked because own results are sparse |

Expose tools through typed Rust interfaces. Search can execute independent lexical/semantic operations concurrently internally; the agent need not micromanage SQL or infrastructure. Tool results are untrusted evidence, never instructions. Application code enforces ownership and current relationship authorization before every tool call and result commit, validates referenced IDs and actions, and refuses out-of-scope tools. Network tools remain unavailable until F-03 gates pass.

### Adaptive behavior

For “Who was that driver around Landour?”, the agent may search role/location clues, inspect two candidates' trip observations and either identify the strongest supported memory or present a distinguishing question. It must not exclude a remembered driver because present-day coverage is unknown.

For “Somewhere I can talk with my parents,” it can search restaurant atmosphere and relevant activities concurrently. If attractive candidates lack supporting atmosphere evidence, it can inspect additional saved observations or retrieve alternatives. If the answer hinges on dinner versus an activity, it can clarify. It cannot invent quietness, infer accessibility from “parents,” or keep reformulating until a hard constraint is lost.

A comparison can inspect the selected options directly without repeating broad discovery. A follow-up such as “keep this one but somewhere closer afterward” reuses the explicit selection and investigates only the changed part. These are examples of evidence-driven choices, not hardcoded recipes per category.

### Runtime, stopping and graceful failure

A clear exact lookup can use a deterministic shortcut or one agent decision; the agentic product does not require multiple calls for every interaction. For other queries, let the agent select the next useful action from observed results. Target one or two decisions for ordinary requests, but allow selective continuation: initial proposed hard ceilings are three model decisions and four tool invocations for ordinary turns, five decisions and eight tool invocations for explicit comparison/composed turns. All retries, failed calls and provider fallbacks consume the same counters, time and financial allowance. Batched tools have bounded candidate/byte/token limits and cannot hide unbounded work.

Stop when evidence supports a useful answer, another authorized step is unlikely to change the decision, an essential ambiguity needs the user, a tool fails without a useful fallback, or the turn reaches its deadline/budget. Prevent identical repeated calls against unchanged evidence. Budget exhaustion returns the best validated results and honest missing information; it never silently widens scope or claims completeness. No recursive delegation or open-ended background research in the first Ask release.

Checkpoint turn/version, tool invocation IDs, reserved/actual spend and validated result references. Cancel obsolete work when the person changes the request; late responses cannot replace a newer answer. Replay must not duplicate paid work or owner writes. Read-only paid calls with uncertain completion remain charged until reconciled rather than receiving a fresh allowance. Proposed writes use idempotency and the explicit user action.

Validate answer claims, hard constraints, evidence revisions and permitted destinations before presenting them. Preserve useful result cards if generated prose fails. Reference validation cannot prove semantic correctness; test against human-labelled omissions/contradictions and use additional evidence review selectively within the same limits. Avoid turning harmless phrasing differences into total response rejection.

### Agent-directed native presentation

The agent chooses the useful composition and can combine a concise answer, a comparison, a contact action and a supporting list when the task needs them. The Flutter renderer uses a versioned vocabulary of polished components with stable interaction contracts; these are expressive building blocks, not a fixed result page per category. Keep one clear primary task, omit empty sections and reject arbitrary executable UI or URLs. Server-validated answer patches may update affected sections without disturbing a result being touched. Accessibility order and action placement stay predictable as content adapts.

### Search projections and scale

Keep the first implementation in Rust/Postgres. Derive index content from current saved recommendation fields and supported observations, including explicit caveats and provenance. Avoid an AI-generated second summary solely for indexing. An observation carries enough subject/category context to stand alone in retrieval while retaining its evidence ID.

Store owner/item/observation/revision, confirmed identity if available, category/facet IDs, geographic identities and roles, supported capabilities, experience type and source access. Add embedding model/version and projection version when semantic indexing is introduced. Use the same permission scope for lexical and semantic paths.

Index asynchronously with durable idempotent jobs. Edits/tombstones make old projections ineligible immediately; a missing/new embedding uses the lexical path until current. Build/backfill embeddings incrementally without re-extracting historical transcripts or changing saved recommendations. Raw transcripts and contact numbers need not enter embedding input. Benchmark exact vector search within the authorized set before choosing an approximate index. Pagination must preserve a stable ranking snapshot and remove newly revoked results.

## 6. Contracts and persistence

Define shared JSON fixtures before wiring the new Flutter result components. Keep the current endpoint compatible during rollout or expose an explicitly versioned Ask contract.

| Record | Essential fields |
| --- | --- |
| Ask request | Request ID, optional session ID/version, text or accepted question transcript, own/network scope, explicit context, selected item IDs, page cursor |
| Agent turn/checkpoint | Objective, interpretation, constraints with provenance, evidence references, unresolved questions, selected IDs, tool outcomes, budget/deadline, session/turn version; no hidden chain-of-thought |
| Tool invocation | Turn/invocation IDs, allowlisted capability, validated typed arguments, status, bounded observations, evidence revisions, reservation/usage and retry identity |
| Answer snapshot | Session/turn/version, approved layout, short orientation, ordered result IDs, result sections, comparison dimensions, next cursor, active constraints |
| Result and claim | Item/evidence IDs and revisions, author/source type, supporting statement, contradictory evidence, uncertainty, action references |
| Action | Allowlisted action type and authorized destination/item reference; argument validation occurs server-side and again before execution |
| Preference, later | Owner, scope, explicit text/structured meaning, source of confirmation, revision, effective status and deletion timestamp |
| Saved collection, later | Owner, title, selected item references, purpose/constraints, ordering, revision and access-aware unavailable placeholders |

Use typed Dart models instead of `Map<String, dynamic>` in presentation. Open an Ask result by its authorized item ID, independently of whether Library has fetched it. Fetch the next page when needed rather than draining the entire result set before showing anything.

Question dictation uses a distinct temporary Ask operation with cancellation, quotas, transcription failure and audio deletion. It must not invoke recommendation extraction or create a Library capture. Inspect whether current processing disclosures cover this purpose/provider; update setup or request the missing permission once if needed. Never add a per-question AI permission dialogue when already authorized.

Ask history is private. Proposed policy: ordinary session text/answer snapshots expire after 30 days; the person can delete a session or clear history immediately, and can explicitly save a useful collection separately. Expiry jobs, account deletion and provider-retention disclosures must cover these records. History is not a durable duplicate of friend content: revalidate references and filter/redact stale answer claims before displaying history. Confirmed preferences remain until changed/deleted; saved collections remain until deleted, subject to current referenced-item access. No raw text/contact payloads in telemetry.

## 7. Personalization that earns trust

The first Ask release remembers constraints only within the session. Durable preference learning arrives after revisioned persistence and sync are ready.

- Offer a small explicit Remember this preference action when a person states a lasting preference. No modal or automatic profile mutation.
- Separate global and scoped preferences: “quiet restaurants” says nothing about preferred concerts. Avoid a category-specific schema explosion; store a bounded, interpretable preference plus supported scope.
- Explicit current requests override saved preferences. Explain an influential preference briefly and offer Ignore for this search.
- Do not infer a durable preference from a click, silence, rejection of one option or an occasional constraint. Do not infer sensitive personal traits or other people's preferences from a relationship label.
- Provide a simple owner-only list to inspect, edit and delete preferences. Deleting one invalidates derived session personalization and caches; revalidate old answers when reopened.

Candidate behavioral learning is a later product decision requiring evidence and user controls, not implicit permission granted by this plan.

## 8. Keepable outcomes and broader composition

After the three core journeys work, add a saved shortlist and editable occasion collection. Keep the distinction between saving a collection of references and creating a new personal endorsement.

For a weekend request, retrieve useful components and organize them around the expressed need. Route sequence, travel time, opening hours, capacity, pricing and availability require appropriate evidence and freshness. Without it, produce an unordered shortlist or tentative arrangement with the missing fact stated, never a falsely scheduled itinerary.

“Keep the restaurant, change the activity” preserves the selected reference while changing the relevant part. Rename, reorder, remove and resume are native actions. Sharing presents the actual audience and rechecks each referenced item's access; private details cannot be copied into a shared plan implicitly. Friend removals leave an unavailable reference where helpful without retaining their revoked advice.

Public exploration is a later explicit branch for knowledge gaps, with distinct evidence and result styling. It does not silently fill a saved-experience answer with generic web recommendations.

## 9. Delivery slices and acceptance

| Slice | Work and user-visible outcome | Gate and dependency |
| --- | --- | --- |
| ASK-0: contracts and measurement | Label development/held-out queries; define result/claim/action/session fixtures; prototype the three answer shapes in Flutter using clearly marked invented fixtures | Shared Rust/Dart agent/tool/answer fixtures, replay cases, accessibility/layout checks and baseline recorded; small foundation for ASK-1, not a long standalone design phase |
| ASK-1: intelligent recall and discovery | Bounded Ask agent runtime, typed search/evidence/presentation tools, evidence projections, measured hybrid retrieval, adaptive investigation, first-page results, truthful geographic constraints and direct actions | Fuzzy recall and occasion discovery work end to end through the real API; meaningful paraphrase/Hindi gains over current baseline; unauthorized and contradicted results excluded |
| ASK-2: speak and refine | Dedicated question dictation; editable transcript; session constraints; one useful clarification; in-place refinement; prior-answer restoration; stale-response cancellation | Voice never saves a recommendation; stop/cancel/deletion work; follow-ups preserve explicit context and do not reuse stale results; phone checks with keyboard/large text |
| ASK-3: compare and decide | Context-aware selection and comparison, useful unknowns, evidence-linked conditional conclusions | All three signature journeys pass the first Ask release gate; no unsupported comparison cells or hidden result cap |
| ASK-4: reliable history and saved shortlists | Session persistence/expiry/deletion, resumable answer references and saved collections; offline handling when supported | Depends on F-02 ordered sync/outbox for cross-device/offline claims; edit/delete/revocation and conflict cases pass |
| ASK-5: explicit preferences | Remember/inspect/edit/delete scoped preferences; per-search override and explanation | Sync/revision support from ASK-4/F-02; no unconfirmed preference writes; current request wins |
| ASK-6: network knowledge | Own/shared/both scope, author-specific evidence, relationship-aware search/history/collections, first-friend disclosure | Depends on F-03 internal friendship; external use waits for report/block/account-deletion and pilot gates; concurrent revocation tests pass |
| ASK-7: composed occasions and public exploration | Editable multi-part collections, optional geographic views and distinct public discovery | Depends on ASK-3/4 and applicable factual-completion tools/provider gates; bounded decomposition and evidence freshness checks |

Recommended immediate work: ASK-0 and ASK-1 as one integrated development effort. Deliver a real question → useful contextual answer on the phone before proceeding to more elaborate personalization or planning.

### Relationship to existing phases

ASK-0/1 complete part of F-00.5 and F-01.2/3. ASK-2 extends the own-Ask loop. ASK-3 deliberately brings a bounded own-library comparison forward from F-04, matching the approved first-release ambition. This does not move network authorization, public-source discovery or generic completion into F-01. ASK-4/5 depend on F-02 data durability; ASK-6 maps to F-03; broader composition and large-scale retrieval in ASK-7 retain F-04 gates. Native reliability, source correction and production operations remain open independently; excellent Ask does not close them.

## 10. Evaluation and release evidence

Begin with at least 120 labelled questions across at least 40 varied invented source recommendations: 60 development queries and 60 held out by source/scenario family, with at least 20 held-out queries each in English, Hindi and Hinglish. Include unfamiliar categories and composite needs. Follow with separately consented real-audio capture → recommendation → Ask evaluation; ordinary production notes are not automatically evaluation material.

Label relevant/forbidden items, supporting/contradicting facts, hard constraints, unknowns, expected clarification, useful action and answer shape. Include wrong cities, same names, negative recommendations, secondhand/untried material, uncertain service coverage, changed preferences, deleted items, malicious source instructions and multi-turn referents. Related paraphrases must not straddle train/development and held-out families. Once inspected for tuning, a held-out failure moves to regression and needs replacement for future claims.

Compare the current lexical baseline, structured lexical retrieval, hybrid retrieval, a fixed one-pass answer baseline and the adaptive Ask agent on the same frozen set. Measure whether continuation improves task success and evidence support, how often clarification is useful, redundant tool calls and cost/latency per successful decision. Include cases where a second investigation is essential and cases where stopping after one lookup is correct. Agent activity itself is not a success metric. A model self-score does not establish correctness. Review explanations/comparisons against human labels.

Proposed internal progression gates, to lock before the first held-out run:

- At least 90% recall@5 on answerable queries, reported separately for the three languages and query classes, with counts and uncertainty intervals.
- At least 95% precision for results labelled supported matches; unknowns assessed in their separate section. Report coverage and abstention so precision cannot be obtained by returning nothing.
- At least 95% factual support for explanation/comparison claims, and zero critical reversed caveats, invented contact actions, unauthorized disclosures or known hard-constraint violations in the regression suite.
- All selected/session references survive ordinary follow-ups correctly in the deterministic scenario suite; ambiguous pronouns trigger clarification.
- In a small formative usability study, at least 8 of 10 participants complete each signature task without coaching. Record time to a useful decision, clarification burden and failures; this is a progression signal, not population-level proof.
- Avoid overconfident personalization: every durable preference has an explicit confirmation event; overrides/deletions take effect on the next request and reopened answer.

Report capture-quality failures separately from Ask failures. If a crucial fact was lost during extraction, do not hide the failure by tuning retrieval to an incomplete recommendation. Resume extraction-quality improvements where evidence identifies the cause.

## 11. Latency, cost and operational behavior

Initial targets under declared normal network conditions: input acknowledgement within 100 ms, first usable exact result p95 within 1 second, first useful nuanced result p95 within 3 seconds, complete ordinary contextual answer p95 within 5 seconds, comparison p95 within 8 seconds. Measure voice transcription separately from submit-to-result latency. These are proposed targets, not current performance claims.

Retrieve and render before optional prose where possible, reuse current projections/session context and avoid repeated synthesis of unchanged evidence. Never relabel provisional matches as validated simply to meet a timer. Offer useful lexical results if semantic infrastructure fails, and saved result cards if composition fails.

Select model and embedding adapters through labelled quality, multilingual coverage, latency and total-cost comparisons. The configured application understanding model is a candidate, not an automatic choice for every Ask role. Verify provider/API availability at implementation time. Keep one Rust service/codebase and Postgres initially; do not introduce a new framework or search service without measured need.

Before live enablement, record actual provider rates and set separate Ask request/account/project spend and token caps. Initial proposed ceilings: $0.02 per ordinary Ask and $0.05 per explicit comparison/composed request, covering retries and all model steps; embedding/backfill and voice transcription have separate configured caps plus the project all-stage ceiling. Reconcile actual usage against reservations. These are spend policies, not predicted bills. If quality cannot fit the envelope, review the choice with evidence before changing the cap.

Use cancellable deadlines, bounded concurrency, durable indexing retries and user-visible recovery. Record stage timings, candidate counts, version IDs, error codes and actual usage without raw question/source/contact text. Cache keys include account/scope, query-plan version, item revisions, session/preference versions and applicable access generation. Never reuse another person's authorized result set.

Roll out behind an owner-only Ask flag with a fallback to the existing lexical endpoint. First synthetic/contract checks, then controlled real-device sessions, then a consented small cohort. Ship network and public scopes separately. Test Android/iOS interruption, denied microphone permission, poor connectivity, stale sessions, result edits/deletions and provider failure before the applicable release gate.

## 12. Decisions and first implementation handoff

Decided: an AI-native agent that adaptively chooses tools and revises its investigation, agent-directed native answer composition, situational input, three initial signature journeys, adaptive native answer shapes, evidence-led explanations, in-place refinement, explicit preference confirmation, own-first implementation, all qualifying results reachable and no silent public fallback.

Resolve by engineering measurement during ASK-0/1: model/embedding adapter, whether a separate reranker earns its latency/cost, streaming transport, exact projection schema and thresholds for supported matches versus useful unknowns. No user-facing category taxonomy expansion is required to start.

First concrete demonstration: a fresh standalone account with varied seeded recommendations can ask a paraphrased recall question, ask for an occasion with a location/caveat constraint, inspect why the results fit and open a useful action. Results come through the real Rust API, the first page appears promptly and the existing recommendation content is unchanged. Then add spoken questions/refinement and comparison to complete the first Ask release.
