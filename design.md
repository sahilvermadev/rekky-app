# Rekky design system

A curated personal collection: neutral surfaces, readable content, and purposeful colour. This replaces the warm brown Library palette. It applies to Flutter; executable tokens live in `apps/rekky_flutter/lib/rekky_theme.dart` and collection accents in `library_style.dart`.

## Structure

The Library is a compact, open list grouped into collections. Combine the title, saved count and account menu in one header; follow with search, location/collection filters, and Browse/Recent/Pins. Keep summaries in the recommendation detail. Rows prioritise the name, category/locality, then rating/audience/review state. Maps and Call are explicit separate actions. Preserve one-tap Remember in the bottom navigation.

## Colour

Light canvas: soft neutral #F7F8FA; primary text: ink #20252D. Dark canvas: charcoal #171A20 with neutral slate elevations. Neither mode inherits a brown surface tint. Collection accents have fixed meanings: Places blue, People & services plum, Things jade, Activities apricot, Ideas gold, Notes neutral. Use accent ink over pale or dark translucent washes for icons. Text labels and icon shapes always supplement colour. Functional actions use blue; errors retain their distinct semantic colour. Do not assign random colours to individual records or use full coloured card backgrounds.

Shared native tokens include surfaces, text, outline, primary/secondary states and button shapes. The palette is defined centrally rather than locally in feature screens. Collection accents have explicit light/dark variants. Icons are from the existing Material family. Recognised saved category labels choose a specific icon; unknown categories fall back to the shelf icon without changing stored data.

## Typography and spacing

Use bundled LibrarySerif in roman style for the Library title and empty-state invitation. Use the native sans-serif for controls, row titles and shelf headings. Library title 30, shelf/row titles 16, row metadata 13, navigation labels 12 logical pixels; respect system text scaling. Use a 4-point spacing rhythm, 20-point content gutters, 12-point row gaps, 44-point category tiles and 48-point interactive targets. Long content and large text wrap; avoid fixed row heights. Large-text layouts omit decorative category tiles to keep usable text width.

## Interaction

Rows open detail; long press exposes pinning. Keep privacy and caution states visible but quiet; the review icon opens the specific explanation. Remember starts the existing recording flow with one tap. Selected navigation uses icon/weight/colour instead of a second large filled pill. The central microphone has the sole filled navigation accent. Preserve native keyboard focus, pressed and disabled states. No decorative motion or background imagery.

## Verification

Check light/dark surfaces, 320/375/414/768 widths, 200% text, keyboard-visible filters, empty and long collections, launch failures, and the recording action. A native screenshot is the final visual check; widget previews do not prove device font fallback or iOS layout.
