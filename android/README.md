# The Android app (prototype)

CalcCraft as a native Android app: Android's `NativeActivity` loads the Rust library
(`libcalculator.so`, see `src/lib.rs`), and eframe draws the same egui interface as on the
desktop with OpenGL ES. There is no Java or Kotlin code. `docs/mobile-apps-research.md` (on
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
another key can't update it: uninstall first with `adb uninstall io.github.rapalon.calccraft`.
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

The script doesn't use Gradle. cargo-ndk builds the library (a `cdylib` only for Android: the
desktop and web builds use the binary), `llvm-strip` drops its symbol table, `aapt2` links the
manifest and resources, the library goes in uncompressed, `zipalign -P 16` aligns it to 16 KB
pages (as Google Play requires), and `apksigner` signs the APK (v2 and v3).

## CI and releases

- **CI** (the `android` job in `.github/workflows/ci.yml`) runs clippy for Android and builds
  the APK for every push and pull request. The APK is the run's `CalcCraft-android-arm64`
  artifact. Each run signs it with a new debug key, so uninstall the app before installing
  one from another run.
- **Releases** (`.github/workflows/release.yml`) attach
  `calculator-<tag>-android-arm64.apk` and its SHA-256 checksum, signed with the release key
  from the repository's secrets. Without them the release still gets an APK, signed with a
  debug key, and the run warns.

### The release key

Every update of an Android app has to be signed with the same key as the version it
replaces, so releases need a key of their own, kept out of the repository. Make it once:

```bash
keytool -genkeypair -keystore calccraft-release.keystore -storetype PKCS12 -alias calccraft -keyalg RSA -keysize 4096 -validity 10000 -dname "CN=CalcCraft"
```

keytool asks for a password. Keep the keystore and its password somewhere safe, with a
backup: without the key, users have to uninstall the app to install an update (and Google
Play would have to reset its upload key). Then add four secrets in GitHub (Settings → Secrets
and variables → Actions → New repository secret):

| Secret | Value |
| --- | --- |
| `ANDROID_KEYSTORE_BASE64` | The keystore file in Base64: `[Convert]::ToBase64String([IO.File]::ReadAllBytes("calccraft-release.keystore"))` in PowerShell, or `base64 -w0 calccraft-release.keystore` on Linux |
| `ANDROID_KEYSTORE_PASSWORD` | The keystore's password |
| `ANDROID_KEY_ALIAS` | `calccraft` |
| `ANDROID_KEY_PASSWORD` | Only if the key has a password of its own (a PKCS12 keystore uses the keystore's) |

To sign a local build with it, set `ANDROID_KEYSTORE` to the keystore's path and the other
three variables before running `build-apk.ps1`. An APK signed with the release key can't
update one signed with a debug key: uninstall the debug build first.

## How it works

- `Cargo.toml` gives Android its own eframe: NativeActivity (`android-native-activity`) and
  glow, as in egui's `hello_android` example, without wgpu and `arboard`.
- `AndroidManifest.xml`: the app handles every configuration change itself, since winit allows
  one event loop per process; phones stay upright, because the layouts need some 500 points of
  height; the keyboard pans the screen instead of shrinking it; and the app opts out of
  predictive back (`enableOnBackInvokedCallback="false"`), without which Android 16 and later
  handle the back button themselves and the app never sees it.
- `res/`: the launcher icon (adaptive from Android 8; `cargo test write_android_icons --
  --ignored` writes the PNGs from `src/icon.rs`) and a light and a dark theme for the system
  bars and the start-up background.
- `src/app/android.rs` (JNI on the NativeActivity that `android_main` gets; ndk-context's
  context is the Application, which has no window): the system bars' and camera cutout's part
  of the screen becomes egui's safe area, which the layout keeps clear (NativeActivity always
  draws under them); the back button closes the open menu, popup, flyout or Settings page as
  Esc does, and with nothing open moves the app to the background (Esc would clear the
  calculator, as in Windows); the interface follows the phone's languages; and a finger
  landing on a key gives the system keyboard's light tap (`performHapticFeedback` with
  `KEYBOARD_TAP`), which Android tunes for each phone and leaves out when touch feedback is
  turned off in its settings.

## Not done yet

- Copy and Paste (no clipboard: egui has none on Android, and `arboard` doesn't build there).
- Share (an `ACTION_SEND` intent).
- Regional formats: numbers and dates are en-US, whatever the phone's region.
- TalkBack: eframe's screen reader support needs GameActivity on Android.
- The status bar's icons follow the phone's dark mode, not the app's own theme setting.
- The graph's hover-only features (its buttons, tracing) have no touch alternative yet.
- Google Play: an App Bundle (AAB); the release key above becomes its upload key.
