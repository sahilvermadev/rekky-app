# Bundled typography

Fraunces and Manrope are bundled under the SIL Open Font License; the full notices are alongside the fonts. No font downloads occur at runtime.

Source: the Google Fonts repository, `ofl/fraunces` and `ofl/manrope`, retrieved 2026-09-27. Static instances were generated with fontTools 4.65.0:

- Fraunces: weight 500, optical size 30, SOFT 20, WONK 1.
- Manrope: weights 400, 500, 600 and 700.

Source files: `Fraunces[SOFT,WONK,opsz,wght].ttf` and `Manrope[wght].ttf`. Family registration is in `pubspec.yaml`; platform fallback handles scripts outside the fonts' coverage. Verify non-Latin text on-device. The older Liberation Serif asset is retained for compatibility with existing development fixtures.
