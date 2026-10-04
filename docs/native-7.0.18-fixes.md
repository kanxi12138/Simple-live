# Android 7.0.18 Fixes

Version: `7.0.18 / 7000018`.

## Changes

- Follow entries display the stored live title, omitting empty titles.
- The chat list follows new messages until the user scrolls into history, and resumes following when the user returns to the latest messages.
- Temporary danmaku connection failures reconnect with bounded backoff. Playback and lifecycle changes cancel the previous connection and heartbeat.
- Douyu uses its existing application heartbeat without WebSocket Ping, which produced invalid control frames. Huya disables WebSocket compression negotiation, which stalled incoming messages after the initial batch.
- Content-free diagnostics track connection, dispatch and rendering counts to identify where updates stop.
- Douyin search preserves session generations and scoped cookie updates, initializes a visitor session when necessary, and uses platform pagination metadata.
- Required search requests retry HTTP 502, 503 and 504 once after one second. Optional search-page preparation can continue after these errors when the required identity is already available. Failed searches retain results for the same query.
- Douyin's `search_nil_info` can report `verify_check` with HTTP 200 and an empty list. This now produces a verification prompt instead of a misleading empty-result message.
- The search verification action opens the official page for the current query with the existing session. After the user confirms completion, the refreshed session is saved and an unchanged query is retried. The page supports fit-to-screen and zoom on phones.

## Verification

- Changed Kotlin and Gradle Kotlin files passed syntax parsing. Signed release builds succeeded with lint skipped; no full test suite or independent type check was run.
- Douyu and Huya each passed three phone rounds of fullscreen danmaku settings changes, followed by at least one minute of observation per round. Receive, dispatch and drawing counts continued to grow, and chat history updated after leaving fullscreen.
- Douyin account searches for the target's short name, full name and account identifier returned the target. Both name queries returned the target's active live room. The numeric live query returned a normal empty result, which does not imply that the account is missing.
- Account pagination returned nine parsed entries on the second page. Official session synchronization and automatic search retry were exercised on the phone.
- The APK retained the existing release signature and was installed as an update with application data preserved.
- Actual HTTP 502, 503 and 504 responses were not reproduced during phone verification. The platform can request verification again; the official flow remains necessary in that case.

The public platform interfaces, database schema and portable configuration format remain unchanged.
