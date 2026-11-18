# Vibration lab (test builds only)

The key vibration slider (Settings → Key vibration, `src/app/vibration.rs`) feels right on a
Pixel 7 Pro, but on a Samsung S26 Ultra even 100 % is barely felt: that phone takes the
`PRIMITIVE_CLICK` composition path, whose strength Samsung sets. Before the strength is
calibrated per phone, the lab lets us feel every way Android can make a key tap, on each phone,
and dumps what the phone's vibrator reports.

## Never in a release

- The lab is behind the Cargo feature `vibration-lab`. It is not in `default`, and no Play or
  release build enables it: `build-apk.ps1` takes it only through `-Features vibration-lab`,
  which `release.yml` and the Play bundle commands in `ci.yml` never pass.
- A test (`vibration_lab_stays_out_of_releases` in `src/app/vibration_lab.rs`) reads
  `release.yml`, `ci.yml` and `build-apk.ps1` and fails if a release or Play command names the
  feature, or if `Cargo.toml`'s `default`, `play-free` or `play-paid` features include it.
- Its texts are English literals, not `locales/` strings: it is a measuring tool for the team,
  never shown to users.

Build it for a phone with

```
pwsh -File android/build-apk.ps1 -Channel play-free -Features vibration-lab
```

## The page

With the feature on, the Settings page ends with a "Vibration lab" link (the Android app only;
desktops have no vibrator). The lab page has a back button (and Android's back and Esc go back
to Settings), a "Capabilities" block, and one big button per entry. A tap plays the entry once.

The entries, always all of them in this order, so the list is the same on every phone. An
entry the phone can't play is greyed and disabled, with the reason under its label (for
example "primitive not supported", "needs API 31", "no amplitude control"). Predefined effects
the vibrator reports as unsupported (`areEffectsSupported` = NO) still play, Android's fallback
waveform, so they stay enabled with a note.

| Id | What it plays |
| --- | --- |
| P1 | `Composition.addPrimitive(PRIMITIVE_CLICK, 1.0)` (API 30+) |
| P2 | `PRIMITIVE_CLICK`, scale 0.5 |
| P3 | `PRIMITIVE_TICK`, 1.0 |
| P4 | `PRIMITIVE_LOW_TICK`, 1.0 (API 31+) |
| P5 | `PRIMITIVE_THUD`, 1.0 |
| P6 | `PRIMITIVE_QUICK_RISE`, 1.0 |
| E1 | `createPredefined(EFFECT_CLICK)` (API 29+) |
| E2 | `EFFECT_HEAVY_CLICK` |
| E3 | `EFFECT_TICK` |
| E4 | `EFFECT_DOUBLE_CLICK` |
| H1 | `View.performHapticFeedback(KEYBOARD_TAP)`, the app's old key tap |
| H2 | `VIRTUAL_KEY` |
| H3 | `CONFIRM` (API 30+) |
| H4 | `KEYBOARD_PRESS` (the same constant, 3, as `KEYBOARD_TAP`) |
| A1–A6 | `createOneShot` at amplitude 255 for 5, 8, 10, 15, 20, 30 ms (`hasAmplitudeControl`) |
| A7 | amplitude 128, 15 ms |
| A8 | amplitude 60, 15 ms |
| D1–D3 | `createOneShot` at `DEFAULT_AMPLITUDE` for 10, 20, 40 ms |

P, E, A and D go through `Vibrator.vibrate(VibrationEffect)` like the key tap does; H goes
through the activity's decor view. The lab ignores the app's own Key vibration setting, but
Android's own settings (touch feedback, vibration intensity) still apply as the system decides.

## Capabilities

Gathered once when the page opens, shown on the page and written line by line to logcat with
the tag `CalcCraftVibLab` (`adb logcat -s CalcCraftVibLab`):

- SDK level and release, manufacturer, model, device.
- `hasVibrator`, `hasAmplitudeControl`; the vibrator ids (`VibratorManager`, API 31+).
- API 31+: `getId`, `getResonantFrequency`, `getQFactor`; every primitive's support and its
  `getPrimitiveDurations`; `areEffectsSupported` for CLICK, TICK, HEAVY_CLICK, DOUBLE_CLICK
  (API 30+).
- The hidden `getInfo()` (`VibratorInfo.toString()`, which includes the capabilities), when
  Android lets an app call it; otherwise the error.
- API 36: `areEnvelopeEffectsSupported`, `getEnvelopeEffectInfo`, `getFrequencyProfile`
  (minimum and maximum frequency, maximum output acceleration). A method the phone lacks is
  reported as missing, with the error.
- `Settings.System` `haptic_feedback_enabled`, `haptic_feedback_intensity` and a few vendor
  keys, each with its value or why it can't be read.
