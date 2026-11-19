# The Android app (prototype)

CalcCraft as a native Android app: Android's `NativeActivity` loads the Rust library
(`libcalculator.so`, see `src/lib.rs`), and eframe draws the same egui interface as on the
desktop with OpenGL ES. The only Java code is the graph share's small `ContentProvider` (see
"How it works"). TalkBack and other accessibility services (and the automation agents built on
them) get every control's name and the display's value from the same AccessKit tree as Narrator
does on Windows (see "Accessibility"). `docs/mobile-apps-research.md` (on
the `research/mobile-apps` branch) explains the choices and what a store release still needs.

**Status:** builds for 64-bit ARM (`arm64-v8a`), Android 7.0 (API 24) and later, targeting
Android 16 (API 36). Tried on a Samsung Galaxy with Android 17: the layout clears the status
bar and the gesture bar, back closes the menu and then leaves the app, and the app comes back
after it.

## Install

```bash
adb install -r target/android/CalcCraft-1.2.0-arm64.apk
```

The APK is signed with the debug key (`~/.android/debug.keystore`). A build signed with
another key can't update it: uninstall first with `adb uninstall com.rapalon.calccraft`.
Warnings and panics are in `adb logcat -s CalcCraft`.

## Build

```bash
pwsh android/build-apk.ps1
```

It writes `target/android/CalcCraft-<version>-arm64.apk`, with the version from `Cargo.toml`
(1.2.0 is version code 10200). It runs in PowerShell 7 on Windows, Linux and macOS, and needs:

- the Rust target and cargo-ndk: `rustup target add aarch64-linux-android`,
  `cargo install cargo-ndk`;
- the Android SDK's `platforms;android-36` and `build-tools;36.0.0`, and the NDK
  `29.0.14206865` (in `ANDROID_HOME`, or by default `%LOCALAPPDATA%\Android\Sdk`,
  `~/Library/Android/sdk` or `~/Android/Sdk`);
- a JDK 17 or later for `apksigner` and `keytool` (`JAVA_HOME`, or by default
  `%LOCALAPPDATA%\Android\jdk-21` on Windows and the `PATH` elsewhere).

The application ID (package name) defaults to `com.rapalon.calccraft`. `-ApplicationId` (or the
`APPLICATION_ID` environment variable) builds with another one. Google Play never allows a free
app to become paid and the package name is permanent after first upload, so the ID must be
chosen before publishing.

To try a build on a phone that has the Play-signed app, give it another ID: it installs next to
that app, which then stays (the debug key can't update it). `-Debuggable` lets
`adb shell run-as <id>` look into the app's files, such as the graph share's single
`cache/share/graph.png`:

```powershell
pwsh -File android/build-apk.ps1 -ApplicationId com.rapalon.calccraft.sharetest -Debuggable
adb install -r target/android/CalcCraft-<version>-arm64.apk
adb shell run-as com.rapalon.calccraft.sharetest ls -la cache/share
adb uninstall com.rapalon.calccraft.sharetest
```

### Channels: GitHub, Google Play free, Google Play paid

Google Play's Payments policy forbids leading users to payment methods other than Play's, so
the Play builds have no Donate link (docs/support-prompt.md, section 8). `-Channel` (or
`CALCCRAFT_CHANNEL`) picks the build's distribution channel, a Cargo feature:

| `-Channel` | For | Application ID | Support prompt | Files |
| --- | --- | --- | --- | --- |
| `default` | GitHub releases, F-Droid, `adb install` | `-ApplicationId` (`com.rapalon.calccraft`) | Donate opens rapalon.com/calccraft/donate | `CalcCraft-<version>-arm64.*` |
| `play-free` | the free app on Google Play | `-ApplicationId` (`com.rapalon.calccraft`) | Without `-PaidApplicationId`: none (no dot, popup or menu button). With it: the same reminders, whose "Get CalcCraft Pro" opens the paid app's listing (`market://details?id=<id>`, else `https://play.google.com/store/apps/details?id=<id>`, no other parameters) | `CalcCraft-Play-<version>-arm64.*` |
| `play-paid` | the paid app, CalcCraft Pro | `-PaidApplicationId` (required) | None: the menu always shows "Supported by you" | `CalcCraft-Pro-<version>-arm64.*` |

`-PaidApplicationId` (or `CALCCRAFT_PAID_ID`) is the paid app's ID, unset by default. Konst chose
**`com.rapalon.calccraft.pro`** on 2026-11-17; it stays a build parameter, not a constant in the
code. The two Google Play bundles:

```bash
# The free app. The first release ships without -PaidApplicationId: no support prompt.
pwsh android/build-apk.ps1 -Channel play-free -Bundle
# Once CalcCraft Pro is on Google Play, the free app's popup leads to it:
pwsh android/build-apk.ps1 -Channel play-free -PaidApplicationId com.rapalon.calccraft.pro -Bundle
# The paid app:
pwsh android/build-apk.ps1 -Channel play-paid -PaidApplicationId com.rapalon.calccraft.pro -Bundle
```

A Play build fails if its library contains `rapalon.com/calccraft/donate` or `stripe.com`. CI
builds all three Play variants with the debug key and checks the paid bundle's package. The
release workflow builds the free bundle (and, when the repository variable `CALCCRAFT_PAID_ID`
is set, the paid one) with the upload key and keeps them as the run's artifact
`CalcCraft-google-play-<tag>`; the GitHub release's APK and bundle stay in the default channel.
Both Play apps can use the same upload key: Play App Signing keeps a separate app signing key
for each.

The script doesn't use Gradle. cargo-ndk builds the library (a `cdylib` only for Android: the
desktop and web builds use the binary), `llvm-strip` drops its symbol table, `javac` and `d8`
compile `java/` into `classes.dex`, `aapt2` links the manifest and resources, the library goes
in uncompressed, `zipalign -P 16` aligns it to 16 KB
pages (as Google Play requires), and `apksigner` signs the APK (v2 and v3).

### The App Bundle for Google Play

Google Play takes no APK but an Android App Bundle (`.aab`), from which it builds the APKs
itself. `-Bundle` writes one next to the APK, from the same library and manifest:

```bash
pwsh android/build-apk.ps1 -Bundle
```

It writes `target/android/CalcCraft-<version>-arm64.aab`. The bundle is the APK's contents in
bundletool's layout: `aapt2 link --proto-format` writes the manifest and the resources as
protocol buffers, the script lays out the base module (`manifest/`, `res/`, `resources.pb`,
`lib/arm64-v8a/`), `bundletool build-bundle` makes the bundle with `android/BundleConfig.json`
(the library stays uncompressed, aligned to 16 KB pages, as in the APK), and `jarsigner` signs it
(SHA256withRSA, the signature Play expects on a bundle) with the same key as the APK. Then
`bundletool validate` and `jarsigner -verify` check it. The one switch, instead of a second
script, keeps the Rust build, the manifest and the signing in one place: the bundle only
takes another `aapt2 link` and the bundletool step.

bundletool is Google's own tool, a Java program: the first `-Bundle` build downloads the pinned
release (`bundletool-all-1.18.3.jar`) into `.tools/` (not in Git) and checks its SHA-256 against
the one in the script, as every later build does; `-Bundletool <path>` uses another copy.

Without a release key the bundle is signed with the debug key, like the APK, which is enough to
try it (`bundletool build-apks --mode=universal` makes an installable APK from it) but useless
for Play, which only takes bundles signed with the upload key (see below).

## CI and releases

- **CI** (the `android` job in `.github/workflows/ci.yml`) runs clippy for Android and builds
  the APK and the App Bundle for every push and pull request, both signed with a new debug key.
  It then checks the bundle's manifest (package name, version code and name, minimum and target
  SDK) and builds the universal APK from the bundle, with the arm64 library on 16 KB pages. The
  APK and the bundle
  are the run's `CalcCraft-android-arm64` artifact; uninstall the app before installing an APK
  from another run (another debug key).
- **Releases** (`.github/workflows/release.yml`) attach
  `calculator-<tag>-android-arm64.apk` and `calculator-<tag>-android-arm64.aab`, each with its
  SHA-256 checksum, signed with the upload key from the repository's secrets. Without the
  secrets the release still gets an APK, signed with a debug key, and the run warns; the bundle
  is skipped, since Play wouldn't take it.

## Releasing on Google Play: the signing runbook

The steps from no key to a bundle on Play, in order. Everything that is a secret stays out of
the repository, off command lines (shell history) and out of chats.

### Two keys: the app signing key and the upload key

Every update of an Android app has to be signed with the same key as the version it replaces.
With **Play App Signing**, which is mandatory for new apps, that key is the **app signing key**:
Google generates it, keeps it and signs the APKs it serves to users with it. We never see it, and
can't lose it. Our own key is the **upload key**: it signs the bundles we upload, and Play checks
each upload against the upload certificate it has on file. Choose the Google-generated app signing
key when the app is created in the Play Console (Test and release → Setup → App signing); the key
of the first bundle uploaded becomes the upload key. Nothing in the build changes between the two
keys: the bundle is always signed with ours.

### 1. Generate the upload key

Once, on a trusted computer, with the JDK's keytool (the build script's JDK has one):

```bash
keytool -genkeypair -keystore calccraft-upload.keystore -storetype PKCS12 -alias calccraft -keyalg RSA -keysize 4096 -validity 10000 -dname "CN=CalcCraft, O=Rapalon Labs"
```

keytool asks for the password itself; don't pass `-storepass`, which would put it in the shell
history. Play requires the upload key to be valid until at least 2033-10-22 (RSA, 2048 bits or
more); 10000 days from today is well past that. A PKCS12 keystore has one password for the store
and the key, so `ANDROID_KEY_PASSWORD` stays unset.

Don't commit the keystore: it belongs outside the repository, in the two places of step 2.

### 2. Keep the keystore and the password in SOPS, with an offline backup

The keystore goes into the SOPS repository as Base64, with its password, in one encrypted file:

```bash
cd <SOPS repository>        # fill in: the repository that holds the other secrets
base64 -w0 calccraft-upload.keystore > /tmp/keystore.b64
EDITOR=nano sops calccraft/android-upload-key.yaml
```

In the editor, write three keys and save:

```yaml
keystore_base64: <the one line of /tmp/keystore.b64>
keystore_password: <the password, typed in the editor>
key_alias: calccraft
```

Then `shred -u /tmp/keystore.b64` (`rm -P` on macOS). The password is typed in the editor and
never on a command line, so it's in no history. A quick way to see that the password typed in
matches the keystore, without echoing it:

```bash
read -rs -p 'Keystore password: ' P && keytool -list -keystore calccraft-upload.keystore -storepass:env P; unset P
```

Offline backup: copy the keystore file and the password to a medium that isn't the SOPS
repository (a password manager entry with the file attached, or an encrypted USB stick kept
elsewhere). Lose both the SOPS copy and the backup and the upload key has to be reset (see 6).

### 3. The upload certificate (only if Play asks)

Play usually learns the upload certificate from the first bundle. It asks for the certificate
itself, as a PEM file, when registering an upload key for an existing app or after a reset:

```bash
keytool -export -rfc -keystore calccraft-upload.keystore -alias calccraft -file upload_certificate.pem
```

The certificate is public: it can go anywhere, including the Play Console's upload form.

### 4. The four GitHub secrets

The release workflow reads the key from the repository's secrets (Settings → Secrets and
variables → Actions). Set them from files and prompts, never with the value on the command line:

```bash
gh secret set ANDROID_KEYSTORE_BASE64 < /tmp/keystore.b64     # or straight from SOPS:
sops -d --extract '["keystore_base64"]' calccraft/android-upload-key.yaml | gh secret set ANDROID_KEYSTORE_BASE64
gh secret set ANDROID_KEYSTORE_PASSWORD                        # prompts; paste the password
gh secret set ANDROID_KEY_ALIAS --body calccraft
```

| Secret | Value |
| --- | --- |
| `ANDROID_KEYSTORE_BASE64` | The keystore file in Base64 (`base64 -w0 calccraft-upload.keystore`, or `[Convert]::ToBase64String([IO.File]::ReadAllBytes("calccraft-upload.keystore"))` in PowerShell) |
| `ANDROID_KEYSTORE_PASSWORD` | The keystore's password |
| `ANDROID_KEY_ALIAS` | `calccraft` |
| `ANDROID_KEY_PASSWORD` | Only if the key has a password of its own (a PKCS12 keystore uses the keystore's) |

To sign a local build with the key, set `ANDROID_KEYSTORE` to the keystore's path and the other
three variables (`read -rs ANDROID_KEYSTORE_PASSWORD; export ANDROID_KEYSTORE_PASSWORD`) before
running `build-apk.ps1 -Bundle`. An APK signed with the upload key can't update one signed with
a debug key: uninstall the debug build first.

### 5. Version codes, and the release

The bundle's `versionName` is `version` in `Cargo.toml`, and its `versionCode` is
`major × 10000 + minor × 100 + patch` (1.4.1 is 10401), computed by `build-apk.ps1`. Play
refuses a bundle whose version code isn't higher than every one uploaded before, to any track,
so every Play upload needs a new version in `Cargo.toml` first: a patch bump is enough (the
code may skip numbers but never repeat). The code must stay below 2100000000; the scheme
allows for version 209.

A release: bump `version`, merge, push the tag `v<version>` (see the README's "Releasing"),
download `calculator-v<version>-android-arm64.aab` from the GitHub release (check its
`.sha256`), and upload it in the Play Console (Test and release → Testing → Internal testing
first, then promote). The release workflow warns and attaches no bundle when the secrets are
missing: a debug-signed bundle is of no use to Play.

To try a release on a phone before the tag, run the release workflow by hand on the branch:
`gh workflow run release.yml --ref <branch>` (or Actions → Release → Run workflow; the `ref`
input builds another branch, tag or commit). Run that way, only its Android job runs: no draft,
no release, nothing published. It needs the upload key and leaves one artifact,
`CalcCraft-google-play-test-<version>-<commit>`, with a `SHA256SUMS` file and:

- `CalcCraft-Play-<version>-arm64.aab`: the play-free bundle for the Play Console, signed with
  the upload key.
- `CalcCraft-Play-<version>-arm64-universal.apk`: the universal APK `bundletool build-apks
  --mode=universal` makes from that bundle (`build-apk.ps1 -Bundle -UniversalApk`), signed with
  the upload key: the code Play installs. It has the Play app's ID but not Play's signature, so
  installing it needs the Play-installed CalcCraft uninstalled first.
- `CalcCraft-Play-<version>-arm64-test.apk`: the play-free app with the ID
  `com.rapalon.calccraft.test`, which installs next to the Play app.

### 6. If the upload key is lost or leaked

With Play App Signing the app survives: users keep getting updates signed with Google's app
signing key. In the Play Console, under Test and release → Setup → App signing, request an
upload key reset: generate a new keystore as in step 1, export its certificate as in step 3,
and upload the PEM there. Google confirms by e-mail (it takes a couple of days), and the new key
is valid from then on. Then redo steps 2 and 4 with the new keystore. Without Play App Signing
(an app enrolled before 2021) a lost key is final: that's why the Google-generated key.

## How it works

- `Cargo.toml` gives Android its own eframe: NativeActivity (`android-native-activity`), glow,
  as in egui's `hello_android` example, and the screen reader support (`accesskit`, see
  "Accessibility" below), without wgpu and `arboard`.
- `AndroidManifest.xml`: the app handles every configuration change itself, since winit allows
  one event loop per process (so a rotation only resizes the window, and the input, history and
  mode stay); it fixes no screen orientation (Settings → Screen orientation picks it, see
  below); the keyboard pans the screen instead of shrinking it; and the app opts out of
  predictive back (`enableOnBackInvokedCallback="false"`), without which Android 16 and later
  handle the back button themselves and the app never sees it.
- Screen orientation (1.4.1): Settings → Screen orientation offers Auto (the default:
  `SCREEN_ORIENTATION_USER`, which follows the system's auto-rotate, or stays in the
  orientation it's locked in), Portrait and Landscape (`SENSOR_PORTRAIT` and
  `SENSOR_LANDSCAPE`: that orientation whatever the system's setting, on whichever side the
  device is held). `android.rs` `set_orientation` calls `Activity.setRequestedOrientation` at
  start-up, before the first frame, and when the choice changes; it works on the activity
  pointer, so it needs no change for GameActivity. Android 16 and later ignore an app's
  orientation on large screens (smallest width 600 dp or more), so the card is left out there.
  A phone on its side gets a landscape layout (the keys in a column of their own, see
  `FEATURES.md`, `FEAT-SHELL-002`); `cargo test layout_tests` checks every mode at phone and
  tablet sizes in both orientations.
- `res/`: the launcher icon (adaptive from Android 8; `cargo test write_android_icons --
  --ignored` writes the PNGs from `src/icon.rs`) and a light and a dark theme for the system
  bars and the start-up background.
- `src/app/android.rs` (JNI on the NativeActivity that `android_main` gets; ndk-context's
  context is the Application, which has no window): the system bars' and camera cutout's part
  of the screen becomes egui's safe area, which the layout keeps clear (NativeActivity always
  draws under them); the back button closes the open menu, popup, flyout or Settings page as
  Esc does, and with nothing open moves the app to the background (Esc would clear the
  calculator, as in Windows); the interface follows the phone's languages; and a finger
  landing on a key gives a tap of the vibration motor with the strength, style and duration of
  Settings → Key vibration (`key_tap`, with the mappings and the Auto rule in
  `src/app/vibration.rs`, measured in `docs/vibration-lab-results.md`). The style Off turns
  the taps off (the strength slider runs from 1 to 100 %; a strength of 0 saved by an older
  build loads as Off with 50 %). Click is a
  `PRIMITIVE_CLICK` composition (Android 11+), Pulse a 5 to 12 ms one-shot with an amplitude
  (or, without amplitude control, a one-shot whose duration follows the strength), Heavy click
  the predefined `EFFECT_HEAVY_CLICK` (Android 10+). Auto is Click where the vibrator reports a
  resonant frequency and a click primitive of its own (CLICK's duration isn't TICK's, from
  `VibratorInfo`, Android 12+), else Pulse; it logs its choice once with the tag `CalcCraft`
  (`adb logcat -s CalcCraft`: `Key vibration: Auto plays Click …`). The Pixel 7 Pro gets Click,
  the Samsung S26 Ultra Pulse. It goes through `Vibrator` (`VibratorManager` from Android 12),
  not `performHapticFeedback(KEYBOARD_TAP)`, whose strength each phone maker sets (barely
  noticeable on a Samsung S26 Ultra), and needs the `VIBRATE` permission, a normal permission
  that Android grants without asking. It stays off when touch feedback is turned off in
  Android's settings (`Settings.System.HAPTIC_FEEDBACK_ENABLED`).
- The Graphing mode's Share sends the graph as a PNG through Android's share sheet. The
  picture is the one Windows and the desktop share: the graph drawn alone, at the screen's
  resolution, with its current view, colors, grid and axes (a `ViewportCommand::Screenshot`
  cropped to the graph, `graph_canvas.rs`). `share::write_single_file` writes it as
  `cache/share/graph.png` in the app's private cache, the only file kept there: each share first
  deletes everything else in `cache/share/`, then overwrites the picture, so shares never pile
  up. `android.rs` (`share_image`) then starts `Intent.createChooser` around an `ACTION_SEND`
  intent (`image/png`, `EXTRA_STREAM` and `ClipData` with the URI,
  `FLAG_GRANT_READ_URI_PERMISSION`). The URI is `content://<application id>.share/graph.png`,
  served by the app's one Java class, `java/com/rapalon/calccraft/ShareProvider.java`: a
  `ContentProvider` without androidx that serves only that file, read-only (`openFile`,
  `getType` `image/png`, and `query` for `OpenableColumns`' name and size). It isn't exported
  (`android:exported="false"`); `android:grantUriPermissions="true"` lets the app the user picks
  read that one URI. The authority follows the application ID (the manifest says
  `${applicationId}.share`, which `build-apk.ps1` fills in), so the free and paid apps don't
  clash. No permission is needed, and nothing goes to the gallery or shared storage. If the
  file or the chooser fails, the mode shows the "unable to share" dialog, as on Windows.
- `java/`: that class. `build-apk.ps1` compiles it with the JDK's `javac` against the
  platform's `android.jar` (Java 8 bytecode), turns it into `classes.dex` with `d8` from the
  pinned build-tools, and puts the dex into the APK and the bundle (`dex/`), so the manifest
  says `android:hasCode="true"`. No library, no download: both tools come with the JDK and
  the SDK the build already needs.

## Accessibility: TalkBack and accessibility services

The app's controls are described to screen readers through AccessKit on every platform
(`src/app/access.rs`: Windows' `AutomationProperties.Name` for every key, the display as a live
region, and so on). On Android, eframe's `accesskit` feature hands that tree to
`accesskit_winit`, whose Android adapter (`accesskit_android`) installs a
`View.AccessibilityDelegate` with an `AccessibilityNodeProvider` on the activity's view: TalkBack
reads the display and the keys by name, swipes between them and double-taps to press, and any
accessibility service (an automation agent such as Marionet, `uiautomator dump`) sees the
same tree and can click a control by its name. Nothing is drawn differently, and nothing leaves
the device: the accessibility services run on the phone.

- **Why a patched `accesskit_winit`.** Upstream's Android adapter only knows
  [GameActivity](https://developer.android.com/games/agdk/game-activity): it reads the activity's
  `mSurfaceView` field. GameActivity itself extends `AppCompatActivity`, so an APK built on it
  needs the whole AppCompat and Jetpack chain (about 25 libraries, their resources, a
  `Theme.AppCompat` theme), which this Gradle-less build doesn't have. CalcCraft keeps
  `NativeActivity` and ships a copy of `accesskit_winit` 0.32.2 in `vendor/accesskit_winit`
  (Apache-2.0, its `LICENSE-APACHE` kept), which `[patch.crates-io]` in `Cargo.toml` puts in
  place of the crates.io release for every platform. The only change is in
  `src/platform_impl/android.rs`, whose header names the upstream version and the change: the
  adapter's host is NativeActivity's content view (the window's decor view →
  `android.R.id.content` → its one child), found through JNI. Everything else, and the desktop
  builds, are the unchanged upstream code.
- **Android 8.0 and later.** `accesskit_android` loads its Java delegate class from a dex it
  embeds, through `dalvik.system.InMemoryDexClassLoader`, which exists from API 26. Upstream
  creates the adapter unconditionally; the patched copy asks `Build.VERSION.SDK_INT` first
  and, below 26, logs one line (`adb logcat -s CalcCraft`) and leaves the app without an
  accessibility tree instead of crashing. That's why the minimum SDK stays 24.
- **The direct dependency.** eframe's `accesskit` feature alone gives Android a no-op adapter:
  the real one is behind `accesskit_winit`'s own `accesskit_android` feature, which egui-winit
  doesn't pass on, so `Cargo.toml` names `accesskit_winit` with that feature for Android.
- **Upgrading eframe.** A new eframe (egui-winit) wants a new `accesskit_winit`, and Cargo then
  warns that the patch is unused (`cargo test` has a test, `vendored_accesskit_winit_matches_the_lockfile`,
  that fails too). Re-apply the patch: copy the new release from
  `~/.cargo/registry/src/*/accesskit_winit-<version>/` over `vendor/accesskit_winit` (its
  `Cargo.toml` without the examples and dev-dependencies, which the copy doesn't carry), put
  the same change into `src/platform_impl/android.rs` again, and update the version in the
  file's header.
- **Checking it on a phone.** `adb shell uiautomator dump` writes the tree Android sees
  (`/sdcard/window_dump.xml`): the keys appear as `android.view.View` nodes with their names
  as `content-desc` and `clickable="true"`, the display with its value as `text`. With
  TalkBack on, swipe right through the keys, double-tap `1`, `2`, `×`, `3`, `=` and hear "Display
  is 36"; drag a finger over the keypad (explore by touch) to hear the key under it. Hover
  events reach the adapter only if NativeActivity passes them on to its view: that's the one
  part of TalkBack that wasn't verified on a device when the patch was written.

## Not done yet

- Copy and Paste (no clipboard: egui has none on Android, and `arboard` doesn't build there).
- Regional formats: numbers and dates are en-US, whatever the phone's region, and Settings has
  no Regional format card (its list would be empty).
- TalkBack on Android 7 (API 24 and 25): the accessibility tree needs Android 8.0's
  `InMemoryDexClassLoader` (see "Accessibility"); the app runs there without it.
- The status bar's icons follow the phone's dark mode, not the app's own theme setting.
- The graph's hover-only features (its buttons, tracing) have no touch alternative yet.
- Google Play: the bundle builds (`-Bundle`), but the upload key, the Play Console listing
  (privacy policy, data safety form, content rating) and the first upload are still to do; see
  the runbook above. The bundle has no native debug symbols (the library is stripped), so Play
  warns on upload and shows native crashes without symbols.
