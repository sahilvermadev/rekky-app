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

Light canvas #F7F7F5, white card; dark canvas #0C0C0C, card #1B1B1B. Surfaces, text, outlines and ordinary controls are achromatic. Coral Places, orchid People & services, mint Things, citron Activities, gold Ideas, neutral Notes. Specific saved categories may use consistent accents within collections: restaurants in Places/coral, bars/orchid, cafés/gold, fitness/mint and stays/orchid. Unclassified entries retain their collection accent. Explicit light/dark accent pairs and subtle washes are in LibraryStyle. Ratings use a gold star and neutral number. Colour always accompanies an icon or label.

Inputs use 12-point corners, cards 16, category emblems 10–14. Use one Material outline icon family. Existing category metadata selects the emblem; unknown types have a shelf fallback. Do not guess classification from the entity name. Dark elevation uses surface lightness without coloured glow. Error, disabled and focus styles remain functional semantic states.

## Shared surfaces

Detail: collection emblem beside the expressive title; category/location/rating and Maps/Call precede the reading account. Original text remains the collapsed neutral quotation view. Ask: matching display, input and result cards, with a scrollable layout for keyboard and large text. Recording: expressive prompt, honest static microphone/elapsed-time state in the coral capture colour, clear finish action, automatic processing unchanged. Never display fabricated waveform activity.

Bottom navigation is an inset rounded neutral dock with two quiet destinations flanking a raised coral microphone action labelled Recommend. Its entire button, including the label, starts recording in one tap. The button rises above the dock without painting interactive content outside its actual layout bounds. At large text sizes the action spans the dock above the two destinations; labels scale naturally. Selection uses neutral ink, weight and a small indicator. Safe areas and end-of-list padding keep content reachable. Native pressed/focus states; no decorative motion. Coral belongs to the capture action and selected category emblems, not every control.

## Acceptance

Inspect native light and dark screens and compare to the previous version at the same viewport. Verify Library top/end, filters, detail, Ask, menu, empty/long lists, 320/375/414/768 widths, 200% text and keyboard-visible filters/search. Touch targets must not overlap. Contrast floors: 4.5:1 ordinary text and 3:1 meaningful icons/focus. Report functional tests and visual inspection separately; neither proves the other's quality. No backend data changes are part of this design system.
