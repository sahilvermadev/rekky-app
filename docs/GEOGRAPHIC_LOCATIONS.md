# Structured geographic locations

Implemented 2026-09-27. A recommendation location now separates:

- `role`: venue, practice/base, service area, past experience or context.
- `text`: the original saved location phrase. Private transcript evidence remains separate and unchanged.
- `name`: a clean extracted name, without conversational framing.
- `geography`: resolved identity, canonical display label, country, administrative hierarchy, filter IDs, source and matching method; or `status: unresolved`.

The understanding prompt requests geographic names, with source units carrying the relationship evidence. Nearby/outside references cannot establish exact practice or coverage. A local background worker enriches saved location fields without reading transcripts, sending provider requests or using the phone's location. Existing recommendations also receive this additive enrichment. Owner edits, item locks and revisions prevent overwriting an edit; source text, bodies, ratings, contacts and visibility are preserved. Each record is evaluated once per imported catalog revision, and re-evaluated after an explicit content edit. Geography is derived metadata, not a new endorsement or proof of coverage.

## Matching and display

Known aliases resolve to stable `geonames:<id>` identities. Multiple explicit location components provide context, e.g. Dwarka + Delhi. A single globally dominant city may be selected over tiny namesakes (population at least one million and over 100 times the next candidate); this is an explicit heuristic, not a calibrated confidence score. Ordinary ambiguous towns abstain. No fuzzy spelling substitution occurs: Landor and Landour are distinct names in the dataset. Missing/ambiguous matches keep a clean name and remain absent from strict area filters.

Cards and sheets show the canonical label, or the clean unresolved name, without adding “Serves” to the primary location. The underlying role remains available for filtering; secondary locations retain role labels to avoid confusing a past trip with coverage. Google venue-address lookup and Maps destinations remain separate and may display an exact matched venue address. An area match never establishes a business's exact address. Edit recommendation retains editable role/text; adding an explicit city/region can disambiguate a name. This slice does not add a geographic candidate picker.

## Filtering contract

`GET /v1/items?area_id=geonames:1265106` filters the signed-in owner's library by geographic identity and supported ancestor/explicit scopes. Optional `location_role=service_area` restricts to that relationship. Without a role, only venue/practice/service-area records qualify. Past-experience/context records require an explicit role; they cannot establish current service coverage. City/state scopes can include supported child locations, but a service in one neighbourhood never implies availability throughout the city. Unknown locations remain available in unfiltered Library.

Location filters run before pagination, bind cursors to the filter, enforce owner/deletion checks and reject invalid role/ID inputs. The index is usable only if its location snapshot matches current content and its catalog revision is current. Changes therefore invalidate stale matches immediately, before asynchronous rebuilding. The mobile Library now uses the city-first browsing projection described below; natural-language Ask location routing remains a later slice.

## Licensed data and coverage

Persistent geographic data comes from [GeoNames downloads](https://download.geonames.org/export/dump/) under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Data is combined, normalized and filtered to geographic features; it is not a complete or error-free global boundary database. About Rekky provides attribution. The committed import manifest records hashes and dataset coverage, not user data.

The local pilot imports worldwide cities500, country/admin data and hierarchy, plus India's detailed country dump: **836,636 geographic records**. Small localities outside that coverage and missing parent relationships can remain unresolved. Hierarchy comes from published administrative codes and explicit hierarchy edges; never infer city containment from proximity. An explicitly named city may supply an item-local filter scope when its administrative context is compatible; that does not add a global parent edge. City membership and service-area correctness still require held-out evaluation before wider release.

Google Maps content is not copied into this permanent gazetteer. Existing ephemeral Google venue details retain their existing storage/attribution rules.

## Reproducing the import

Download `cities500.zip`, `countryInfo.txt`, `admin1CodesASCII.txt`, `admin2Codes.txt`, `hierarchy.zip` and desired country archives (initially `IN.zip`) from GeoNames. Extract the text files into ignored `work/geography`. Additional country files extend detailed coverage without code changes. Then:

```sh
python3 tools/prepare_geography.py work/geography
# In apps/rekky_backend, with the intended DATABASE_URL:
cargo run --locked -- migrate
cargo run --locked --example import_geography -- ../../work/geography/areas.jsonl
```

The import publishes data and a new catalog revision atomically. Reimport the complete intended dataset: records missing from the replacement are removed. Store the generated manifest for provenance. No automatic download, provider call, new credential or extra AI job is part of recording a recommendation.

Validation includes alias/context matching, ambiguous and similarly spelled names, parent closure/cycles, clean labels, source/role preservation, nearby-area exclusion, owner isolation, parent-area filtering, edit invalidation and deletion. See the shared `geographic_locations.json` fixture and Rust/Flutter/importer tests. Device and local backfill results are recorded after deployment below.

Deployment validation: 82 Rust tests, all 111 Flutter tests and the importer hierarchy/cycle regression test pass; Clippy, Flutter analysis and release backend/debug Android builds pass. Migration 016 and the open-data catalog are installed locally. Ten existing location fields were evaluated: eight resolve, while bare Dwarka and Malviya Nagar remain unresolved. Sanskriti displays Delhi; Lavnish Taxi Cabs displays Landour, Uttarakhand. Phone Library labels were verified after refresh. A checksum over every existing recommendation's subject/body/audience, rating/contact/other content and original location text/role is identical before and after enrichment. No real transcript was submitted to a provider for this change. Region-filter controls, correction candidate selection, boundary-quality evaluation and iOS runtime validation remain open.


## Library browsing projection

`geography.browse` v1 separates a destination (usually a city/town), an optional neighbourhood, and broader regions. It is derived exclusively from already supported geographic scope IDs. Explicit city context can link an item-local neighbourhood with a city even where GeoNames types that neighbourhood as PPL; city-seat settlement types are preferred when uniquely supported. Published settlement parent edges are also respected. A district is never treated as the boundary of its namesake city. Missing/ambiguous city membership retains the explicit locality, and unresolved geography remains unresolved.

The mobile picker exposes unique destinations and alias search, optional neighbourhood refinement and a secondary Regions view. It does not dump the administrative hierarchy. Familiar display names omit standard administrative prefixes while IDs remain stable. Counts are unique items within the current collection/search/Pins scope. Neighbourhood results include explicit city-wide service areas but never imply that a city-only practice is located in that neighbourhood. See `contracts/rekky/v1/fixtures/location_browsing.json` for the wire example. Migration 018 requests a local derived-data rebuild; no stored note or transcript is reprocessed through AI.

Country picker names come from the existing licensed GeoNames countryInfo snapshot, bundled as `apps/rekky_backend/data/country_names.json`; country names are not shortened by blindly stripping political prefixes.

The worker also checks the browse-format version on resolved fields, so an old worker's catalog-current record without browse metadata is repaired once. The obsolete local test backend on port 3089 was stopped during this deployment after it was found competing for the same derived-data jobs.
