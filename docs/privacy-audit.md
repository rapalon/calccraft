# Privacy audit (2026-11-17)

The code audit behind [`PRIVACY.md`](../PRIVACY.md) and [`play-data-safety.md`](play-data-safety.md),
made on `main` at `a7310d2` (version 1.3.2). Line numbers refer to that commit. When the app
starts handling data differently, update this file, `PRIVACY.md` and the Data safety answers
together.

## Android manifest

- One `<uses-permission>`, `android.permission.VIBRATE` (added 2026-11-17 for the Key
  vibration setting): a normal permission, granted at install without a prompt, that lets the
  app run the vibration motor (`src/app/android.rs`, `key_tap`). It reads and sends nothing.
  There is no `INTERNET` permission, so the Android app can't open a network connection itself
  (`android/AndroidManifest.xml`).
- The key vibration reads Android's touch feedback setting (`Settings.System`
  `haptic_feedback_enabled`) so that keys stay still when the user turned it off; it is used on
  the device only. The chosen strength is stored as `CalcCraftKeyVibration` in `app.ron`.
- `android:hasCode="false"` (`AndroidManifest.xml:15`): no Java/Kotlin code, so no Google Play
  services, Firebase or other SDKs. The activity is Android's `NativeActivity`.
  Since 2026-11-18 (version 1.4.1) `hasCode` is `true`, for one class of the app's own,
  `android/java/com/rapalon/calccraft/ShareProvider.java` (no library, no SDK): a
  `ContentProvider` (`android:exported="false"`, `android:grantUriPermissions="true"`, authority
  `<application id>.share`) that serves one file read-only, `cache/share/graph.png`, to the app
  the user picks in the graph's share sheet. Nothing else can read it: the provider isn't
  exported, and Android grants the chosen app read access to that one URI only.
- Since 2026-11-19 the library also carries AccessKit's Android adapter (`accesskit_android`,
  Apache-2.0, through the patched `vendor/accesskit_winit`): one Java class, loaded from a dex
  embedded in `libcalculator.so`, that describes the app's controls to TalkBack and other
  accessibility services the user has turned on, on the device (`android/README.md`,
  "Accessibility"). It reads nothing, stores nothing and sends nothing; no SDK, no network.
- `android:allowBackup="true"` (`AndroidManifest.xml:12`): Android Auto Backup may copy the
  app's internal files (the settings file below) to the user's Google account backup and use
  them in a device-to-device transfer. That is Google's backup under the user's account; the
  developer has no access to it. `PRIVACY.md` says so. See "Open decisions" in
  `play-data-safety.md`.

## Stored locally

eframe's `persistence` saves one key-value file, `app.ron`:

| Platform | Location | Evidence |
| --- | --- | --- |
| Windows | `%APPDATA%\CalcCraft\data\app.ron` | `src/lib.rs:42-44` (app name `CalcCraft`), AGENTS.md "Pitfalls" |
| macOS | `~/Library/Application Support/CalcCraft/app.ron` | eframe's `storage_dir` |
| Linux | `$XDG_DATA_HOME/calccraft/app.ron`, else `~/.local/share/calccraft/app.ron` | eframe's `storage_dir` |
| Android | the app's internal storage, `files/app.ron` | `src/lib.rs:100-103` (`internal_data_path`) |
| Web | the browser's `localStorage` for the page's origin | `src/lib.rs:50-51`, eframe's web storage |

What goes in it (`CalculatorApp::save`, `src/app.rs:2770-2789`; keys at `src/app.rs:93-105`):

- `SelectedAppTheme`, `Mode`, `UnitConverterPreferences` (category and units),
  `CalcCraftLanguage`, `CalcCraftRegionalFormat`, the graph theme (`graphing_view::THEME_KEY`),
  the keep-on-top window size (`calculatorAlwaysOnTopLastWidth`/`Height`, `src/app.rs:454-457`);
- on Android, the Screen orientation choice `CalcCraftScreenOrientation` (`Auto`, `Portrait` or
  `Landscape`, `src/app/orientation.rs`, added 2026-11-18) and the key vibration's
  `CalcCraftKeyVibration`, `CalcCraftKeyVibrationStyle` and `CalcCraftKeyVibrationDuration`;
- egui's memory (UI state such as scroll positions), which eframe saves with it;
- with the `support-prompt` feature (on by default), `SupportStatsV2` (`src/support/stats.rs:6`,
  fields at `stats.rs:17-47`, written by `SupportState::save`, `src/support/view.rs:199`):
  launches, active days, calculations, reminder cycles, the interval ladder step, day numbers
  (first day, last active day, last/next cycle, dot since, pause until, supporter until),
  `donate_clicked_ever`, shares, popups shown, and the demo-mode flag. Counters and local day
  numbers only: no identifier (`stats.rs:14-15`).

Not stored: the calculators' history and memory, the graphing equations and variables, the
date calculation's dates and the converters' values. `save` writes none of them, so they're gone
when the app closes.

## Device information read (never sent)

- Language: Windows' preferred UI languages (`GetUserPreferredUILanguages`,
  `src/app/strings.rs:329`), Android's (`src/app/android.rs:178`, via JNI),
  the browser's `navigator.languages` (`src/app/web.rs:26`).
- Regional formats: `GetUserDefaultLocaleName` and `GetLocaleInfoEx` (`src/app/region.rs:373`),
  the browser's `Intl` (`src/app/web.rs:63-216`).
- Country, for the converters' default units: `GetUserDefaultGeoName`
  (`src/app/converter_view.rs:180-184`), the region tag in the browser (`src/app/web.rs:51`).
- The system theme (egui/winit), the Android SDK version and the system bars' insets
  (`src/app/android.rs:52`, `:90`), the page address's `?lang=`/`?region=` (`src/app/web.rs:17`).
- The screen's smallest width in dp (Android, `Configuration.smallestScreenWidthDp`,
  `android::orientation_honored`), only to leave out the Screen orientation setting where
  Android 16 ignores an app's orientation (large screens). It sets the activity's requested
  orientation (`setRequestedOrientation`), which needs no permission.

## Network

- No HTTP or telemetry crate in `Cargo.lock`: no `reqwest`, `hyper`, `ureq`, `curl`, `rustls`,
  `native-tls`, `openssl`, `sentry` or `opentelemetry`. `tokio` comes only through
  `wasm-bindgen-futures`/`winit`, `zbus` only through AccessKit's Linux screen reader bridge
  (local D-Bus), `webbrowser` only opens the system browser (`cargo tree -i`).
- The app's code makes no request. The web version's service worker fetches only the page's
  own files from its own origin (`assets/web/sw.js:22-32`). `index.html` loads no third-party
  script, font or analytics.
- Panics and warnings go to the local console or `adb logcat` only (`src/lib.rs:56-57`, `:93-98`).

## Outbound links (opened only when the user taps)

All go through `open_link` (`src/app.rs:3050-3057`): the system browser on desktops (egui's
`open_url`), a new tab on the web, an `ACTION_VIEW` intent on Android
(`src/app/android.rs:248`).

| Link | URL | Parameters | Evidence |
| --- | --- | --- | --- |
| Rapalon Labs (About card) | `https://rapalon.com` | none | `src/app.rs:2502`, `src/brand.rs:13` |
| Rapalon Labs (support popup) | `https://rapalon.com` | none | `src/support/view.rs:397` |
| Privacy policy (Settings) | `https://rapalon.com/calccraft/privacy` | none | added after the audit: `PRIVACY_URL`, the link under Send feedback on the Settings page (`src/app.rs`) |
| Send feedback | `https://github.com/rapalon/calccraft/issues` | none | `src/app.rs:91`, `:2520` |
| GitHub | `https://github.com/rapalon/calccraft` | none | `src/app.rs:90`, `:2538` |
| Donate | `https://rapalon.com/calccraft/donate` (redirects to a Stripe Payment Link, keeping the query) | see below | `src/support/donate.rs:9`, `:88-106`; `src/support/view.rs:351-369` |

Donate parameters (`donate_url`, `src/support/donate.rs:88-106`):

| Parameter | Value | Reveals |
| --- | --- | --- |
| `utm_source` | `calccraft` | nothing (static) |
| `utm_medium` | `popup` or `menu` | whether the reminder opened by itself or from the menu |
| `utm_campaign` | `support` | nothing (static) |
| `utm_content` | `<platform>_<version>`, e.g. `android_1-3-2` | platform (`win`, `mac`, `linux`, `android`, `web`, `other`) and app version |
| `client_reference_id` | `cc_<platform>_<version>_<medium>_c<cycles>_<age>`, e.g. `cc_win_1-3-2_popup_c3_d30-90` | the above, plus completed reminder cycles capped at 10 (`c0`–`c10`) and days since the first launch in 5 buckets (`d0-7`, `d7-30`, `d30-90`, `d90-365`, `d365p`) |

No identifier, no counts beyond these buckets, nothing typed. Stripe stores
`client_reference_id` with a Checkout Session paid through the link; the UTM values show in
Stripe's and the website's statistics.

## Share and clipboard

- Tell a friend (`src/support/view.rs:373-388`): the text "CalcCraft — a free, precise
  calculator: https://rapalon.com/calccraft?utm_source=calccraft&utm_medium=share&utm_content=<platform>_<version>"
  (`src/support/share.rs:22-33`) goes to the clipboard on desktops (`share.rs:66-73`), the
  browser's `navigator.share` sheet (`src/app/web.rs:259`), or Android's `ACTION_SEND` chooser
  (`src/app/android.rs:273`), falling back to `ClipboardManager.setPrimaryClip`
  (`android.rs:307-333`). The recipient is picked by the user in the system's sheet.
- The graph's Share (Windows): Windows' share sheet with the PNG, the equations and variables as
  HTML (`src/app/share.rs:85`, `:150-194`); in the browser a PNG download (`src/app/web.rs:291`).
  On Android (since 1.4.1) the PNG alone goes to Android's `ACTION_SEND` chooser as a
  `content://<application id>.share/graph.png` URI with a read grant (`src/app/android.rs`,
  `share_image`). Before that it's written to the app's private cache, `cache/share/graph.png`,
  the only file kept there: every share deletes whatever else is in `cache/share/` and
  overwrites the picture (`src/app/share.rs`, `write_single_file`). It never goes to the gallery
  or shared storage, and Android may delete the cache at any time.
- Copy writes the clipboard on the user's command: results (`src/app.rs:923-924`, `:1684`),
  converter values (`src/app/converter_view.rs:396`), dates (`src/app/date_view.rs:256`),
  Programmer values (`src/app/programmer_view.rs:330`), the graph and equation text
  (`src/app/graphing_view.rs:2562-2566`).
- Paste reads the clipboard on the user's command: keyboard pastes arrive as egui events, the
  equation boxes' Paste menu item uses `arboard` (`src/app/graphing_view.rs:2897-2898`), and the
  web version prefetches the text when a menu with Paste opens (`src/app/web.rs:232-252`).

## Outside the app

- Google Play: distribution, installs and reviews; Android vitals (crash and ANR reports that
  Android sends to Google for users who share usage and diagnostics), shown to the developer
  in aggregate.
- F-Droid: planned distribution of the open-source build (`docs/package-id-unification-plan.md:11`).
- GitHub: Releases (the APK and desktop builds, `.github/workflows/release.yml`), the
  repository and issues, and GitHub Pages serving the web version at
  `rapalon.github.io/calccraft` (`.github/workflows/pages.yml:4`); GitHub logs visitors' IP
  addresses.
- Stripe: the hosted payment page; Rapalon Labs receives the payment's details from Stripe.
- Cloudflare: delivers rapalon.com, where the donate, share and privacy pages are.
- The calccraft-mcp server lives on branch `mc/b2bca5e0` and isn't part of `main`: out of scope.
