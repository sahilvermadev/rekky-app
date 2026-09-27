# Provider phone completion

A provider's phone belongs to the recommendation. Friends-visible items carry it automatically; Private items keep it owner-only. There is no second audience field, contact sharing toggle or per-match sharing approval. The current pilot still has owner-only retrieval; actual friend viewing is an F-03 dependency.

## Mobile flow

- Account menu → Contact matching → Enable matching explains the behavior once, then requests native Contacts access. Android uses READ_CONTACTS only; iOS supports its native limited/full permission. Denial never prevents recording or manual phone entry.
- After foreground Library loading (including completed recordings), eligible people/service recommendations can acquire one clear saved number. Existing recommendations are eligible too. The app does not run this device operation while closed or guarantee background execution.
- Matching uses names and phones only, in memory for each batch. One exact normalized full name, structured first and last names, one contact and one international number qualifies. Honorifics and basic punctuation/case are ignored. Single names, duplicates, businesses with no structured person name, fuzzy matches, multiple phones and missing country codes need user selection/input. A match is not proof of current ownership of a number.
- Add contact → Find a saved contact enables matching if needed and completes a clear match; otherwise it presents local candidates. Choose from contacts opens the native picker. Manual entry is always available. Select the provider, never a referrer.
- Detail shows Call, a selectable number and Change contact beside the destination actions. Call opens the OS dialer on user gesture. Full Edit supports adding/changing/clearing the number. Contact removal suppresses future automatic matching for that item.
- Disabling matching requires a server acknowledgement and cancels pending automatic writes by generation. It does not erase saved snapshots. Phone-book changes do not silently replace published numbers. A rename/kind change clears an inherited number; an explicit replacement in that edit is retained.

No address book, candidate names, native IDs or unrelated details go to the backend, AI providers, public lookup or analytics. Phone numbers are not added to recommendation prose or search projections. Matching uses only the recommendation subject, never names in its source/attribution. Native access does not enable account-level automatic matching by itself.

## API and persistence

`GET /v1/me/contact-matching` returns `{ "enabled": false, "generation": 0 }` before setup. `POST` accepts `{ "enabled": true|false }` and increments the generation on every change. Migration 012 stores this account-specific preference. Automatic attachment locks that preference while committing; an acknowledged disable cannot be followed by a write using the old generation.

`PATCH /v1/items/{id}/contact` requires a signed-in disclosed owner and `If-Match: <revision>`:

- `{ "mode": "automatic", "phone": "+919876543210", "generation": 1 }`: enabled current generation, person/service item, no existing contact or removal override.
- `{ "mode": "set", "phone": "+919876543210" }`: explicit manual/picker attachment or replacement; does not require global matching permission.
- `{ "mode": "none" }`: removes the snapshot and persists `contact_matching: "off"` to prevent resurrection.

Saved v2 JSON adds `contact: { "phone": "+919876543210", "origin": "contacts"|"user" }`. No phone-specific visibility exists. Each successful mutation increments the item revision and preserves content/audience/source. Normalize harmless phone separators but require `+` and 8–15 digits, no leading zero, extensions or dial-control characters. Country is never inferred from a device locale.

Full content edits accept optional `contact: {mode: keep|set|none, phone?: string}`; omission means keep for old clients. Unrelated edits preserve the snapshot/removal override; subject/kind changes clear inherited contact. Current AI refinement skips contact-bearing/explicitly suppressed legacy recommendations and revisions fence in-flight refinement. Future refinement versions must deliberately preserve this precedence. Source deletion leaves the independently attached snapshot; item deletion removes authorized access. Future Friends APIs must apply item authorization before returning the complete snapshot, including previews/actions/caches.

## Validation and remaining limits

Validation on 2026-09-27: Rust formatting, Clippy, all 59 unit/integration tests against the isolated PostgreSQL test database, and release build passed. The expanded contact integration case additionally passed after adding full-edit preservation, identity-change clearing and shared fixture checks. Flutter analysis and the full 95-test suite passed; the 4 new contact UI tests and the additional shared-fixture test passed separately (100 unique tests across these runs). Coverage includes deterministic matching/abstention, permission denial, cancellation/account change, number validation, owner isolation, revision conflicts, removal suppression, permission generations and audience inheritance.

Built and installed the Android debug APK on the connected physical Android phone, applied migration 012 to the isolated development database, and started the updated Rust backend. Verified signed-in Library loading, Add contact on the existing doctor item, the manual form, and optional matching disclosure on-device. No real phone contact was attached or address book read in this check; native permission grant, real matching/picker selection and dialer handoff still need user/device exercise. Matching remains off until the user enables it. iOS runtime was not available. No real address-book data or device screenshots were committed.

The contact feature does not implement friend discovery, social graph, public phone lookup, referrer routes, email contact routes, server-side address-book matching or live contact synchronization. Those remain separate planned work.
