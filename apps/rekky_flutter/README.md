# Flutter application

Fresh Flutter application with mandatory sign-in, Friends disclosure, Ask home, Library and an explicit one-item text Remember action. Its backend calls are real; there is no guest or demo account. Voice, offline storage and network friend retrieval are still pending.

Use Flutter 3.47.0. Run `flutter pub get`, `flutter analyze`, `flutter test` and `flutter build apk --debug`. For an Android emulator, launch with `--dart-define=API_BASE_URL=http://10.0.2.2:3088 --dart-define=GOOGLE_SERVER_CLIENT_ID=<web OAuth client ID>`. The backend's `GOOGLE_CLIENT_IDS` must include that token audience. For physical devices or production, use an HTTPS API URL. Debug Android alone permits cleartext local HTTP; release builds require HTTPS.

Set up a distinct Android OAuth client for `app.rekky.rekky_flutter` with the signing certificate fingerprint, and an iOS client for bundle ID `app.rekky.rekkyFlutter`. On iOS also pass `--dart-define=GOOGLE_CLIENT_ID=<iOS OAuth client ID>` and configure its URL scheme plus the Sign in with Apple capability before device sign-in. The iOS build and provider flows need macOS/physical-device validation. No provider secret belongs in `--dart-define` or the mobile bundle; client IDs are public configuration.
