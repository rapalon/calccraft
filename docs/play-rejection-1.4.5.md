# Google Play's rejection of 1.4.5: Broken Functionality

Google Play rejected CalcCraft 1.4.5 (versionCode 10405) under the Broken Functionality policy:
"UI elements are causing crashes: tapping certain buttons or icons causes your app to crash"
(in-app experience). The evidence was two screenshots: the Standard mode (dark theme, display
0, empty memory) and Android's crash dialog "CalcCraft closed because this app has a bug …
try clearing the app's cache first" (Cancel / Clear cache). Google didn't say which button.

This is the investigation (2026-11-24, Mission Control task 3ec17022) on Konst's Pixel 7 Pro
(cheetah, Android 16, build CP1A.260405.005, arm64).

## The build tested

The Play app is `com.rapalon.calccraft` (`-Channel play-free`). It isn't installed on the Pixel;
`com.rapalon.calccraft.test` is the same code: 1.4.5 / 10405, `play-free`, not debuggable,
arm64-v8a, built by `android/build-apk.ps1` like the release (the library is always
`cargo ndk … --release`; there is no R8 or ProGuard, the only Java is `ShareProvider` through
`d8`, so a release build has no shrinking step that a test build lacks). Nothing found below
depends on the build type: they come from the manifest and from how winit and android-activity
handle the activity's lifecycle.

Both findings were then reproduced on `com.rapalon.calccraft` itself: 1.4.5 rebuilt from
`c197fba` (main at 1.4.5) with the release's command, `build-apk.ps1 -Channel play-free
-PaidApplicationId com.rapalon.calccraft.pro`, signed with the debug key (the signing key
changes nothing in how the app runs). bundletool couldn't be downloaded on this runner for the
universal APK of the bundle, but that APK has the same library and manifest. `am start -n`
while it ran ended the process (`EventLoop can't be recreated`, `has died: fg TOP`); Bold text
then a tap gave "CalcCraft isn't responding" on exactly the screen of Google's first evidence
(`IN_APP_EXPERIENCE-8641.png`). Two such ANRs in a row, each closed with "Close app" and the app
started again, didn't bring up the "Clear cache for CalcCraft?" dialog of the second evidence
(`IN_APP_EXPERIENCE-3527.png`) on the Pixel.

## Findings

Neither failure needs an unusual button: both are lifecycle events that a crawler (Robo,
monkey) or a reviewer's device produces while someone taps the keys, and each makes CalcCraft
vanish or freeze in the Standard mode.

### 1. A second activity in the process ends the app

- **Repro:** start CalcCraft, then, while it runs:
  `adb shell am start -n com.rapalon.calccraft.test/android.app.NativeActivity`. Monkey does the
  same with its app-switch events (`--pct-appswitch`, a launcher intent during a recents swipe):
  seeds 3, 4, 5 and 6 of `monkey -p com.rapalon.calccraft.test -s <seed> --throttle 30
  --pct-syskeys 2 --pct-appswitch 1 --pct-rotation 3 -v 5000` all hit it.
- **What happens:** the manifest has the default `launchMode="standard"`, so Android stacks a
  second `NativeActivity` in the task (`dumpsys activity activities`: `sz=2`, `LAUNCH_MULTIPLE`).
  android-activity calls `android_main` again in the same process; eframe asks winit for an
  event loop, which winit allows once per process:

  ```
  I android-activity: Creating: Activity = 0x3742, saved state size = 0
  E CalcCraft: calculator: CalcCraft couldn't start: winit EventLoopError: EventLoop can't be recreated
  I Zygote  : Process 15348 exited cleanly (0)
  I ActivityManager: Process com.rapalon.calccraft.test (pid 15348) has died: vis TOP
  W ActivityTaskManager: Force removing ActivityRecord{…}: app died, no saved state
  ```

  `android_main` then ends the process (`src/lib.rs`), so the app disappears while the user is
  in it; Android's exit records show it as `EXIT_SELF`. Under monkey it repeated every few
  seconds, and several runs ended in ANRs ("Application does not have a focused window") while
  the restarted process came up.
- **Root cause:** `launchMode="standard"` plus winit's one event loop per process.
- **Release-only?** No: every build, every Android version.

### 2. A destroyed activity freezes the app (ANR)

- **Repro:** start CalcCraft, then change a configuration that the manifest's `configChanges`
  doesn't name. On the Pixel: Settings → Display → Bold text
  (`adb shell settings put secure font_weight_adjustment 300`; `settings delete secure
  font_weight_adjustment` turns it off again). The system's dynamic colour overlays changing
  (new wallpaper colours: `AppCompatResourceOverlayPolicy: doResourceOverlayChangesAffectActivity:
  result: true … overlayPathsAdded`, which happened during a monkey run) does the same, and no
  app can declare that one.
- **What happens:** Android relaunches the activity (`ActivityRecord{…} is relaunching`).
  `NativeActivity.onDestroy` calls android-activity's `on_destroy`, which sends `Destroy` to the
  app's thread and waits until `android_main` returns. winit 0.30's Android backend ignores
  `MainEvent::Destroy` (`warn!("TODO: forward onDestroy notification to application")`), so it
  never returns: the main thread waits forever, input stops, and Android reports an ANR. The
  ANR's trace of the main thread:

  ```
  "main" prio=5 tid=1 Native
    native: #00 pc 0009f7dc  /apex/com.android.runtime/lib64/bionic/libc.so (syscall+28)
    native: #01 pc 00d92be4  …/base.apk (offset 50000) (???)          ← libcalculator.so
    native: #02 pc 00af4f40  …/base.apk (offset 50000) (???)
    native: #03 pc 00afa930  …/base.apk (offset 50000) (???)
    native: #04 pc 00af9120  …/base.apk (offset 50000) (???)
    native: #05 pc 00154b20  /system/lib64/libandroid_runtime.so (android::NativeCode::~NativeCode+48)
    native: #06 pc 00154358  /system/lib64/libandroid_runtime.so (android::unloadNativeCode_native+24)
    at android.app.NativeActivity.unloadNativeCode(Native method)
    at android.app.NativeActivity.onDestroy(NativeActivity.java:198)
    at android.app.Activity.performDestroy(Activity.java:9612)
    at android.app.ActivityThread.performDestroyActivity(ActivityThread.java:6554)
    at android.app.ActivityThread.handleDestroyActivity(ActivityThread.java:6599)
    at android.app.ActivityThread.handleRelaunchActivityInner(ActivityThread.java:6884)
    at android.app.ActivityThread.handleRelaunchActivity(ActivityThread.java:6799)
  ```

  (frames #01–#04 are in the stripped `libcalculator.so`; by the call chain they are
  android-activity's `on_destroy` → `notify_destroyed`, waiting on its condition variable for
  the app's thread to stop).
- **What the user sees:** the Standard mode as in Google's first screenshot (dark, display 0,
  MC and MR disabled), keys that don't react, then Android's "CalcCraft isn't responding"
  (Close app / Wait). Reproduced deterministically: Bold text toggled while CalcCraft is in
  front (`Config changes=10000000`, `fontWeightAdjustment`), one tap on a key, an ANR 10 s later
  (`Input dispatching timed out … Waited 5006ms for MotionEvent`). Even `adb shell input tap`
  blocks then.
- **Root cause:** `configChanges` lacked `mcc`, `mnc`, `touchscreen`, `colorMode`,
  `fontWeightAdjustment` and `grammaticalGender`; and nothing handled a destroy that can't be
  avoided.
- **Release-only?** No.

### What was not found

- No Rust panic, Java exception or native signal (`Fatal signal`, `FATAL EXCEPTION`, tombstones)
  in any run, and Android's exit records (`dumpsys activity exit-info`) list no `CRASH` or
  `CRASH_NATIVE` for any CalcCraft package: only `EXIT_SELF` (finding 1) and force stops after
  ANRs (finding 2). So the exact dialog of Google's second screenshot wasn't reproduced on the
  Pixel. Both findings make the app vanish or freeze, which is what a crawler counts as a crash.
- Monkey runs that hit neither (seeds 1, 2, 7, 8, 9, 12, 13, 14; 3,000 to 5,000 events each,
  with and without an accessibility client querying the tree in a loop) tapped every mode,
  Settings and the menus without a failure.
- JNI calls (`with_activity` in `src/app/android.rs`) log a failure and clear the Java exception
  instead of panicking, so a device without a browser or a Play Store doesn't crash on About's
  links or the PRO pill.

## The fixes

`android/README.md`, "One activity, one process":

1. `configChanges` names every configuration change an app can declare.
2. `launchMode="singleTask"`, so an intent for the running activity goes to it.
3. `android.app.func_name` points NativeActivity at `calccraft_on_create` (`src/lib.rs`), which
   runs android-activity's `ANativeActivity_onCreate` and wraps `onDestroy`: a destroy (an
   overlay change, Don't keep activities) ends the process at once instead of freezing it.

## Verified on the Pixel

The fixed build, installed as `com.rapalon.calccraft.lifecycle` (`play-free`, debug key; the
installed `.test` has another signature and wasn't removed):

| Case | 1.4.5 (`.test`) | Fixed |
| --- | --- | --- |
| `am start -n …/android.app.NativeActivity` while running | second activity, process ends (`EXIT_SELF`) | "intent has been delivered to currently running top-most instance", same pid, `sz=1` |
| Bold text on while in front | relaunch, ANR dialog | no relaunch (handled config change), same pid, keys work |
| Colour style changed (an overlay change) | relaunch, ANR (seen under monkey) | `CalcCraft ends: Android destroyed the activity`, process exits in 30 ms, no ANR |
| Monkey seeds 3, 4, 5 (ANRs before) | ANR, monkey aborted | finished |

Seed 6 left the app for the launcher and opened the old 1.4.5 `.pro.test`, which froze on its
pending relaunch (finding 2 again), so it says nothing about the fix.

A limit that stays: on a destroy that can't be avoided (overlay change, Don't keep activities),
the fixed app closes instead of freezing, and Android shows the home screen ("app died, no
saved state"); the next start restores the saved state. Coming back as it was would need
winit to allow a second event loop per process (a patched winit, or upstream support for
`onDestroy`), which is more than this fix.

## Not covered

- Other devices and Android versions (an emulator, a tablet, Android 7–15): the emulator wasn't
  available to this run; see the task's report.
- The exact crash dialog: if Google's report names a stack trace later (Play Console → Android
  vitals, or the pre-launch report's crash details), compare it with the two findings.
