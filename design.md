# Rekky design system — personal field guide

A contemporary personal collection, with expressive display typography, compact neutral cards, and small collection-coloured emblems and heading markers. Implemented from `docs/CURATED_LIBRARY_DESIGN_BRIEF.md`. Native tokens live in `lib/rekky_theme.dart` and `lib/library_style.dart` under the Flutter app.

## Composition

Library combines its title, count and menu in one header. The count shares the title baseline when they fit; they stack naturally at large text sizes. Search and a separate Filters action share the next row. All / Recent / Pinned use ink, weight and a short underline. Active category, type, city and neighbourhood filters appear as removable tokens.

Filters share one sheet. Collections, optional specific types, destination search, broader Regions and optional neighbourhood choices apply immediately. Closing dismisses the sheet without a save/apply step. Clear all resets filtering while preserving the current Library view and query. Geographic identity and coverage semantics remain unchanged.

Collections have a short rounded colour marker, a small sans-serif heading and scoped count. Individual entries use neutral cards with a faint outline, 16-point corners, 12-point inset and 8-point separation. Names dominate; category/locality and rating/audience follow. Omit duplicate broad shelf text where a group already communicates it. Maps, Call and review actions retain separate accessible targets. No summaries in Library cards. Natural height takes priority over a fixed density target.

## Typography

Bundled static Fraunces Medium (500, optical size 30, SOFT 20, WONK 1) is the display face. Bundled Manrope 400/500/600/700 is the interface and reading face. Licences and source/instance details are under `assets/fonts`. No runtime font downloads. Preserve platform fallback for other scripts.

Display 30/1.12, recommendation title 17/1.25 at weight 600, shelf heading 14–15 bold, body 16/1.55, metadata 13/1.4, navigation label 12. Display text is roman. Respect system text scaling; wrap or reflow instead of reducing its effect.

## Colour and shape

Light canvas #F7F7F5, white card; dark canvas #0C0C0C, card #1B1B1B. Surfaces, text and card outlines remain neutral. Red identifies recording, blue identifies Ask and focused search, and yellow identifies active Library controls and rating stars. Collections use a separate fixed six-colour palette: vermilion Places, violet People & services, turquoise Things, leaf green Activities, amber Ideas, and rose Notes. Each broad collection keeps its accent across section markers, solid icon tiles, filter choices, and detail emblems. Specific saved categories change the icon, not the collection colour; unfamiliar categories inherit their broad collection's accent. Dark mode uses bright flat fills with dark glyphs, while light mode uses deeper fills with white glyphs. The light/dark pairs and interaction tokens live in RekkyTheme and LibraryStyle. Colour always accompanies an icon or label.

Inputs use 12-point corners, cards 16, category emblems 10–14. Use one Material outline icon family. Existing category metadata selects the emblem; unknown types have a shelf fallback. Do not guess classification from the entity name. Dark elevation uses surface lightness without coloured glow. Error, disabled and focus styles remain functional semantic states.

## Shared surfaces

Detail: collection emblem beside the expressive title; category/location/rating and Maps/Call precede the reading account. Original text remains the collapsed neutral quotation view. Library: selected tabs and active filters use yellow, and search takes a blue focus outline. Icon tiles are small saturated objects within neutral cards. Ask: matching display, input and result cards, with a scrollable layout for keyboard and large text. Recording: expressive prompt, honest static microphone/elapsed-time state in the red capture colour, clear finish action, automatic processing unchanged. Never display fabricated waveform activity.

Bottom navigation follows the user's drawn three-part capsule: Ask at the left, a longer red Recommend capsule in the centre, Library at the right. Text only, no raised microphone, stacked icon labels or selection underline. Use flat primary fills in this control: red Recommend, blue Ask, yellow Library. These colours recur as functional accents on their respective screens. Ask and Library are full-colour button surfaces with rounded outer ends and straight inner edges; their colour continues underneath the rounded ends of Recommend, so there are no empty seams. The visible curved edges also govern hit testing: taps in the exposed blue or yellow corners belong to that side, not Recommend. Selection and press subtly tint the whole side surface over 220 ms, while the selected label gains weight and lifts slightly over 200 ms. Recommend compresses to 97.5% on press and returns over 190 ms. Use ease-out motion, no looping effects, and zero-duration transitions when the system requests reduced motion. All segments share a centreline and at least 60-point touch height with 15-point labels. The outer capsule fills the phone width with 12-point side insets and is capped at 560 points on larger screens. It has no top padding and only 4 points below it before the system safe area. The Library's final visible card has no trailing list gap or separate end spacer, so its bottom edge meets the navigation area when scrolled to the end. The entire centre label/button starts recording in one tap. Scaled label measurement triggers a taller reflow only when all three labels cannot fit, preserving accessibility without shrinking text. Safe areas keep content reachable.

## Acceptance

Inspect native light and dark screens and compare to the previous version at the same viewport. Verify Library top/end, filters, detail, Ask, menu, empty/long lists, 320/375/414/768 widths, 200% text and keyboard-visible filters/search. Touch targets must not overlap. Contrast floors: 4.5:1 ordinary text and 3:1 meaningful icons/focus. Report functional tests and visual inspection separately; neither proves the other's quality. No backend data changes are part of this design system.
