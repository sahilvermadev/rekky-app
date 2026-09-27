# Rekky: personal field guide

Implementation brief — 2026-09-27. User-approved implementation direction, based on the screenshot critique. The code now follows this brief; `design.md` describes the implemented system. Verification evidence is recorded in the canonical product plan. This brief is a design target, not a claim of user acceptance or platform coverage.

## 1. What the next pass must achieve

The Library should feel like a collection of recommendations someone cares about. The current neutral palette is a usable base, but default Material controls, tiny metadata, repeated storefront tiles and full-width rules still dominate the appearance. A serif title alone does not create a distinctive product.

Design position: a contemporary personal field guide. Expressive typography, compact recommendation cards, colour used like collection tabs, and precise alignment. Keep the interface useful and restrained. Do not introduce scrapbook textures, fake paper folds, decorative quotations, generic stock photography, gradients, or a large introductory hero. Do not add summaries to Library cards.

The signature should be the combination of a distinctive Library title, small coloured collection tabs, restrained category emblems and an immediately recognisable microphone action. These elements must recur consistently across the product.

## 2. Shared design foundation

### Typography

Replace the current Liberation Serif/native-default pairing with bundled, licensed Fraunces Medium for display and Manrope for interface/reading. Use static font files and retain their licence notices. If these assets cannot be sourced, resolve that before declaring the design finished; do not silently substitute a system serif. Retain platform fallback for Hindi and other scripts; verify it on-device.

- Display title: 30 logical pixels, weight 500, line height 1.12, tracking -0.5. Roman only. Use for Library and recommendation detail titles, not every label.
- Recommendation name: 17, weight 600, line height 1.25, tracking -0.2. Give long names up to two lines at ordinary text size, natural height at large text.
- Section heading: 15, weight 650/nearest bundled weight, line height 1.3.
- Body/detail account: 16, weight 400, line height 1.55.
- Metadata: 13, weight 450/nearest bundled weight, line height 1.4. Do not shrink metadata to accommodate long text.
- Controls: 14, weight 600. Navigation labels: 12, weight 600.
- Ratings: 13–14 with tabular figures if the font supports them. Display the existing value and provenance; styling must not invent or relabel a rating.

### Palette and elevation

Use independently specified neutral surfaces in each mode. Initial token candidates: light canvas #F6F7F9, light card #FFFFFF, dark canvas #15181D, dark card #20242B. Use ink #222832 and muted text #626B78 in light mode, and #F0F2F5 / #ADB5C2 in dark mode. Final values must pass contrast checks in their actual pairings.

Collection families remain blue for Places, plum for People & services, jade for Things, apricot for Activities, gold for Ideas, and slate for Notes. Use pale washes with darker glyphs in light mode and restrained coloured surfaces with lighter glyphs in dark mode. Accent occupies small, deliberate areas: collection tab, category emblem, selected control, rating star. Most of every card stays neutral.

Do not use blue as the fill of every selected control. Active text tabs can use ink and an underline. Keep semantic errors separate from collection colours. Use visible icon/text distinctions in addition to colour.

No coloured shadow or glow in dark mode. Card elevation comes from surface lightness and a subtle neutral outline. In light mode, use either a faint outline or one restrained shadow, not both heavy treatments. Define all values in native tokens: surface roles, palette families, corner radii, spaces, type styles and durations. Do not create a CSS design system in this Flutter project.

### Shapes and icons

Use three main radii: 12 for inputs, 16 for recommendation cards, and capsule geometry for the microphone. Maintain one outline icon family, with consistent optical weight and 20–22-point utility icons.

Category emblems are a compact 36–40-point illustration area, not stock square tiles copied from Material defaults. Use a softly rounded rectangular wash and a carefully centred category glyph with consistent stroke weight. Add a small collection-tab motif to section headings: a short rounded horizontal colour mark followed by the heading and count. Do not add a thick colour stripe to every card.

Specific icons must use saved classification metadata. Never infer category solely from an entity name for decoration. Unknown and unfamiliar types get a deliberate generic emblem. This design pass does not reprocess private transcripts or silently change classifications.

## 3. Library composition

### Header and controls

Use 20-point horizontal gutters. Combine the display title, saved count and account menu. Align the count optically with the title, not arbitrarily vertically centred against the whole header. Keep the menu's 48-point target. The header may wrap gracefully at 200% text.

Below it, place a 48-point search control with a 20-point search icon, 14-point prompt and clear focus state. Put a separate Filters button beside the search field in the same row; do not place competing interactive elements on top of an editable field. Filters has its own 48-point target, a clear accessibility label and an active indicator.

Below search, use an understated text tab row: All, Recent, Pinned. Map All to the existing Browse behavior. Use weight plus a short 2-point underline for the selected tab; remove the large filled Browse pill. Tab targets remain at least 48 points high even though the visual text/underline is small.

Remove the always-visible All collections / All locations row. The Filters sheet contains both dimensions in a single surface: collection choices and a searchable city/destination list. Selecting a city or collection applies immediately with no extra Apply step. Preserve optional neighbourhood narrowing and the Regions route, with their existing geographic semantics. Reopening the sheet shows current selection. Do not create a second nested chooser just to reach city options.

When filters are active, show removable, human-readable tokens under the tabs (for example, Restaurants and Bengaluru). Show only active constraints. Each token's remove target is accessible, labels never falsely imply a resolved city, and Clear all preserves the chosen All/Recent/Pinned view. Search and filter state still survive destination switching.

Target: at ordinary text size, the first recommendation begins about 190–220 logical pixels below the top safe area on a 375-point phone. Treat this as a visual budget, not a hard height or a reason to clip large text.

### Collection groups

Keep the useful group ordering and existing See all behavior for larger collections. Show a short collection-coloured tab, collection name and an accurate scoped count on a single baseline. Use 24 points before a group, 10–12 between group heading and first card, and 8 between cards. The collection tab is the distinctive recurring motif; do not make every heading large or serif.

Recent uses real dates and Pinned uses actual user pins. Do not manufacture a featured recommendation, completion score, streak, or inspirational statistic to fill space.

### Recommendation cards

Replace full-width separated rows with quiet, compact neutral cards. They should feel like individual collected entries, while maintaining list scanning and density. Avoid giant cards and do not insert the summary.

Card geometry: 16-point corner radius; 12-point internal padding; 10–12 gap between emblem and content; no fixed height. Normal short entries should be approximately 84–100 points; long metadata and interactive review controls may require more. At a 375 × 812 viewport with standard text, aim for four or five fully visible short recommendations after chrome, rather than achieving a density count by shrinking text.

Internal hierarchy:
1. Name dominates the card, in the interface face at 17/600.
2. Category and meaningful locality follow at 13 with readable contrast. Wrap naturally; avoid a chain of barely legible metadata. Generic shelf labels need not be repeated where the group already states them and no more specific category exists.
3. A compact metadata line carries the available rating, audience and optional review/pin/experience state. Do not leave placeholders for absent values.
4. Maps or Call is the trailing utility action. Use a recognisable map or phone icon, with tooltips/accessibility labels; do not use a generic northeast arrow for Maps. A custom web link retains an external-link icon.

The row body opens detail. Utility controls must have their own non-overlapping hit targets and must not also open the detail. Keep review explanations accessible through a quiet information icon near metadata. A 48-point target must not be achieved by extending invisibly over a neighbouring control or the next card. Do not shrink the target merely to force a uniform height. Long press may keep pinning, with pin also available through the detail menu.

Rating stars use muted gold; the score itself uses ink. Privacy icons are quieter than names and ratings but remain identifiable. Review is informational, not an error-coloured badge. Card pressed state is a slight neutral surface change; no scale bounce or shadow pop.

## 4. Bottom navigation

Preserve Ask, one-tap Remember and Library. Align Ask and Library icons and labels on matching baselines. Selected state uses icon/weight/colour, not a large pill around the entire destination. Keep the central microphone as the sole filled accent, approximately 56 × 36 visually, with a larger accessible enclosing control. Its label remains part of the tappable action.

Make the bar a coherent neutral surface with a very faint boundary. It should not look like three unrelated buttons placed on the bottom. Reserve the system gesture inset and final content padding. Do not let the final recommendation remain hidden under the bar. At 200% text, grow the bar naturally and keep labels readable on one line where feasible without suppressing system text scaling.

## 5. Apply the same identity to adjacent screens

Library is the first fully reviewed composition. Then propagate its shared tokens and visual vocabulary to the following existing screens; do not redesign their underlying data flows.

- Recommendation detail: a compact header with the category emblem/tab, expressive title, clear location and rating, then one comfortable reading account. Place Maps/Call immediately after identity/location. Keep the original note in the existing collapsed quotation treatment with a neutral or lightly tinted surface. Do not reintroduce Your experience, Original note, repeated headings or source-management clutter.
- Ask: use the same display/interface pairing, a confident search field and restrained results. Keep the user's question primary. Do not add fake suggested answers or a dashboard of unsupported capabilities. Existing Ask scope remains unchanged.
- Recording: one dominant live recording state, elapsed time and a clear finish action. Use the shared microphone shape and one recording accent. Any waveform must reflect actual input; otherwise use a simple honest recording indicator. Respect reduced motion. Keep transcription/extraction automatic and backgrounded.
- Menus, filters, editor and empty states: share corner geometry, type, surface roles, spacing, focus and error treatment. Avoid leaving a default brown or oversized Material component among the custom surfaces.

## 6. Native implementation map

- `lib/rekky_theme.dart`: shared typography, neutral surfaces, interaction state styles, radii and colours. Eliminate incidental colour generation where it changes the intended neutral surfaces.
- `lib/library_style.dart`: collection accent pairs, emblem geometry and classification-to-glyph presentation mapping. Retain semantic fallbacks.
- `lib/library_screen.dart`: header composition, consolidated filter entry, All/Recent/Pinned tabs, group headers and compact cards. Extract focused widgets if it improves ownership/readability, rather than adding more nested styling to the existing build method.
- `lib/rekky_navigation.dart`: baseline alignment, central microphone, selection, safe area and scaling.
- `lib/recommendation_detail.dart`, `recommendation_view.dart`, `original_note_view.dart`, `voice_capture_sheet.dart`, and the Ask composition in `main.dart`: apply the same visual language after the Library is settled.
- `pubspec.yaml` / `assets/fonts`: bundled font registration and licence assets. Update runtime licence registration in `main.dart`.
- `design.md` and canonical product plan: record the implemented system and any deliberate control-layout changes. Do not mark the proposal as shipped beforehand.

No backend migration, source reprocessing or category mutation is required for these visual changes. Preserve authentication, server-acknowledged privacy, source permissions, existing action launching, pin failure behavior and processing limits.

## 7. Delivery sequence and acceptance

1. Build a native preview using fabricated examples that include a restaurant, bar, unfamiliar service, doctor, book, activity and unclassified note. Cover missing rating/location and long names. Use the real components and fonts, not a separate attractive mockup whose implementation drifts.
2. Settle one Library composition in light and dark mode. Inspect actual rendered output before applying it to every surface. Check the title/counter baseline, search/filter alignment, card density, icon consistency and action alignment. Do not treat a palette swap as completion.
3. Integrate existing callbacks/state, build the consolidated filters with preserved city/neighbourhood behavior, and propagate the visual language to the adjacent surfaces.
4. Verify 320/375/414/768 widths, 200% text, a small-height keyboard-visible view, empty/one-item/long collections and both brightness modes. Check non-Latin font fallback on-device. Use natural wrapping and reflow rather than arbitrary font reduction.
5. Run relevant widget tests and analysis. Include navigation dispatch, filter persistence/clear semantics, distinct card versus Maps/Call/review taps, loading/error states, and safe-area behavior. Avoid tests that merely assert colour constants.
6. Install and inspect on the physical phone: Library top and end, active filters, detail, Ask, recording entrance, menu and back navigation. Do not create a real recording merely for a visual check. Restore device-wide appearance settings after temporary checks.
7. Compare before/after screenshots at the same viewport and text scale. Verify that ordinary content is easier to recognise, primary actions are obvious, and the app has a coherent identity. Report visual evidence separately from functional test counts.

Contrast floors: 4.5:1 for ordinary text and 3:1 for meaningful icons/focus indicators. No button overlap, clipped labels, hidden last card or accidental two-action tap. Avoid decorative animation; use brief native state transitions and reduced-motion behavior. Accessibility and content density are constraints of the design, not reasons to settle for generic styling.

Completion means the real native surfaces match the intended composition with the user's variable content. Passing layout tests alone is not a visual-quality judgement.


## Subsequent palette and navigation revision — 2026-09-27

The user's review supersedes the blue/slate palette and compact Remember treatment above. Use the neutral black/charcoal surfaces, varied category accents and raised coral **Recommend** dock documented in `design.md` and the canonical plan. The layout, typography, accessibility and interaction requirements still apply. Remember remains an internal callback/API term only.


## User sketch refinement — 2026-09-27

The user's drawn capsule supersedes the raised dock: text-only Ask / Recommend / Library on one horizontal centreline, with an approximately half-width coral centre pill and equal neutral ends. No navigation icons or elevation. Follow `design.md` for dimensions and large-text reflow.
