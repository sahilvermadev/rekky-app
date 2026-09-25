# Local voice-draft spike (F-00.3)

This checkpoint proves the app can prepare account-scoped temporary audio without sending it to a processor. It is **not** the product's voice-to-memory journey: there is no upload, transcription, accepted-transcript acknowledgement or server audio deletion yet. A draft is never shown as a saved Library item.

## Current flow

From the signed-in Remember sheet, **Record voice draft** requests microphone access and starts a local AAC/M4A recording. **Done** stops the recorder, verifies a nonempty file, copies it into protected temporary ownership, and only then marks the draft ready in SQLite. The receipt says the audio is on this device only. The Ask home shows a separate voice-draft entry with a Delete action. **Discard / Type instead** removes an active draft. Recording stops after two minutes or when the app becomes inactive. Nothing starts listening on launch.

The SQLite row is committed before recording starts. It contains the fresh account ID, opaque draft ID, status, local path, byte count and creation time; no audio bytes or transcript. Recording starts in app-private cache. On Done, the app copies the bytes to a temporary file, flushes and renames it in Android's `noBackupFilesDir` or iOS Application Support with the backup-exclusion flag, then commits the new path and ready status; the staging file is removed. Android excludes `noBackupFilesDir` from Auto Backup. iOS backup exclusion is a system hint, not a guarantee of every backup pathway, and is applied to the directory and completed file. The app does not call a draft “queued for processing” because there is no upload yet.

On a later signed-in launch, the store migrates older ready cache drafts into protected storage, marks unfinished files **interrupted**, marks absent files **unavailable**, removes orphan files from both locations and deletes drafts older than seven days. A signed-out or different account cannot list or delete another account's drafts through the store. The same account can see remaining drafts after reauthentication. An interrupted recording is not assumed complete merely because some bytes exist.

The recorder uses the [record package](https://pub.dev/packages/record) with AAC-LC, mono, 16 kHz and 64 kb/s settings; actual hardware output may vary. The local metadata uses [sqflite](https://pub.dev/packages/sqflite), and the app directories come from [path_provider](https://pub.dev/packages/path_provider). Android declares `RECORD_AUDIO`; iOS declares its microphone purpose string. No Mapx capture code or data is imported.

## Evidence and remaining checks

Flutter lifecycle tests cover row-before-record, owner isolation, protected-copy/deletion, cache-era migration, interrupted restart, missing files, orphan cleanup, expiry and permission denial. Flutter analysis and an Android debug build pass. The earlier build was installed on a physical Android phone and its signed-in Remember sheet was inspected; the microphone was not activated during that checkpoint. Physical-device recording, permission denial, app kill/restart with real media, backup inspection, full-storage behavior and iOS recording still need to be tested. Android and iOS CI builds check compilation, not microphone behavior.

The current app also needs the local backend to restore a session on launch. Offline capture after a prior sign-in, source-text acknowledgement, processing permission, transcription, and post-transcription deletion belong to the next integrated slice. Before any audio upload, audit and disclose the selected provider's actual retention/training settings, record independent transcription consent on the backend, and test withdrawal fencing. This draft spike grants no processing permission.
