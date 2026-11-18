# Vibration lab results

What the Settings → Key vibration card (FEATURES.md `FEAT-SHELL-008`, `src/app/vibration.rs`)
is based on. The vibration lab was a test-only page (task a0f59c25, branch `mc/a0f59c25`, never
merged) that played every way Android can make a key tap and dumped what the vibrator reports.
Below: what it played, the capabilities dumps of the two phones tried (`adb logcat -d -s
CalcCraftVibLab` after opening the lab page, test build `CalcCraft-Play-Lab-1.3.2-arm64.apk`),
Konst's results by feel, and the rule the card takes from them.

## The lab's entries

| Id | What it played |
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

## Samsung Galaxy S26 Ultra (SM-S948U1), 2026-11-17

All 25 entries play there: nothing is greyed.

```
SDK 37 (Android 17)
Device: samsung SM-S948U1 (m3q)
hasVibrator: true
hasAmplitudeControl: true
VibratorManager ids: [0]
getId: -1
getResonantFrequency: NaN
getQFactor: NaN
VibratorInfo (hidden getInfo): VibratorInfo{mId=-1, mCapabilities=[COMPOSE_EFFECTS, AMPLITUDE_CONTROL, EXTERNAL_CONTROL, EXTERNAL_AMPLITUDE_CONTROL], mCapabilities flags=111100, mSupportedEffects=[CLICK, DOUBLE_CLICK, TICK, HEAVY_CLICK], mSupportedPrimitives=[CLICK(20ms), THUD(300ms), SPIN(130ms), QUICK_RISE(150ms), SLOW_RISE(500ms), QUICK_FALL(100ms), TICK(20ms), LOW_TICK(20ms)], mPrimitiveDelayMax=1000, mCompositionSizeMax=10, mQFactor=NaN, mFrequencyProfileLegacy=FrequencyProfileLegacy{mFrequencyRange=null, mMinFrequency=NaN, mResonantFrequency=NaN, mFrequencyResolution=NaN, mMaxAmplitudes count=0}, mFrequencyProfile=FrequencyProfile{mResonantFrequency=NaN, mFrequenciesHz=null, mOutputAccelerationsGs=null, mMinFrequencyHz=NaN, mMaxFrequencyHz=NaN, mMaxOutputAccelerationGs=NaN}, mMaxEnvelopeEffectSize=0, mMinEnvelopeEffectControlPointDurationMillis=0, mMaxEnvelopeEffectControlPointDurationMillis=0}
Primitive CLICK: supported, 20 ms
Primitive THUD: supported, 300 ms
Primitive SPIN: supported, 130 ms
Primitive QUICK_RISE: supported, 150 ms
Primitive SLOW_RISE: supported, 500 ms
Primitive QUICK_FALL: supported, 100 ms
Primitive TICK: supported, 20 ms
Primitive LOW_TICK: supported, 20 ms
Effects supported: CLICK yes, TICK yes, HEAVY_CLICK yes, DOUBLE_CLICK yes
areEnvelopeEffectsSupported: false
getEnvelopeEffectInfo: missing: java.lang.NoSuchMethodError: no non-static method "Landroid/os/SystemVibrator;.getEnvelopeEffectInfo()Landroid/os/vibrator/EnvelopeEffectInfo;"
getFrequencyProfile: null
Settings.System haptic_feedback_enabled: 1
Settings.System haptic_feedback_intensity: unset
Settings.System hardware_haptic_feedback_intensity: not readable: java.lang.SecurityException: Settings key: <hardware_haptic_feedback_intensity> is not readable. From S+, settings keys annotated with @hide are restricted to system_server and system apps only, unless they are annotated with @Readable.
Settings.System keyboard_vibration_enabled: 1
Settings.System VIB_FEEDBACK_MAGNITUDE: 2
Settings.System SEM_VIBRATION_FORCE_TOUCH_INTENSITY: not readable: java.lang.SecurityException: Settings key: <SEM_VIBRATION_FORCE_TOUCH_INTENSITY> is not readable. From S+, settings keys annotated with @hide are restricted to system_server and system apps only, unless they are annotated with @Readable.
```

Notes: the HAL reports no resonant frequency, Q factor or frequency profile (all NaN, `null`),
so no frequency or acceleration based calibration is possible there. Samsung's own touch
intensity, `VIB_FEEDBACK_MAGNITUDE`, is readable (2 here).

## Google Pixel 7 Pro (cheetah), 2026-11-17

All 25 entries play there too: nothing is greyed.

```
SDK 36 (Android 16)
Device: Google Pixel 7 Pro (cheetah)
hasVibrator: true
hasAmplitudeControl: true
VibratorManager ids: [0]
getId: -1
getResonantFrequency: 145.95691
getQFactor: 18.77275
VibratorInfo (hidden getInfo): VibratorInfo{mId=-1, mCapabilities=[ON_CALLBACK, PERFORM_CALLBACK, COMPOSE_EFFECTS, AMPLITUDE_CONTROL, EXTERNAL_CONTROL], mCapabilities flags=110101111, mSupportedEffects=[CLICK, DOUBLE_CLICK, TICK, HEAVY_CLICK, TEXTURE_TICK], mSupportedBraking=[], mSupportedPrimitives=[NOOP(0ms), CLICK(26ms), THUD(300ms), SPIN(133ms), QUICK_RISE(150ms), SLOW_RISE(500ms), QUICK_FALL(100ms), TICK(5ms), LOW_TICK(12ms)], mPrimitiveDelayMax=10000, mCompositionSizeMax=254, mPwlePrimitiveDurationMax=0, mPwleSizeMax=0, mQFactor=18.77275, mFrequencyProfileLegacy=FrequencyProfileLegacy{mFrequencyRange=null, mMinFrequency=NaN, mResonantFrequency=145.95691, mFrequencyResolution=NaN, mMaxAmplitudes count=0}, mFrequencyProfile=FrequencyProfile{mResonantFrequency=145.95691, mFrequenciesHz=null, mOutputAccelerationsGs=null, mMinFrequencyHz=NaN, mMaxFrequencyHz=NaN, mMaxOutputAccelerationGs=NaN}, mMaxEnvelopeEffectSize=0, mMinEnvelopeEffectControlPointDurationMillis=0, mMaxEnvelopeEffectControlPointDurationMillis=0}
Primitive CLICK: supported, 26 ms
Primitive THUD: supported, 300 ms
Primitive SPIN: supported, 133 ms
Primitive QUICK_RISE: supported, 150 ms
Primitive SLOW_RISE: supported, 500 ms
Primitive QUICK_FALL: supported, 100 ms
Primitive TICK: supported, 5 ms
Primitive LOW_TICK: supported, 12 ms
Effects supported: CLICK yes, TICK yes, HEAVY_CLICK yes, DOUBLE_CLICK yes
areEnvelopeEffectsSupported: false
getEnvelopeEffectInfo: missing: java.lang.NoSuchMethodError: no non-static method "Landroid/os/SystemVibrator;.getEnvelopeEffectInfo()Landroid/os/vibrator/EnvelopeEffectInfo;"
getFrequencyProfile: null
Settings.System haptic_feedback_enabled: 1
Settings.System haptic_feedback_intensity: unset
Settings.System hardware_haptic_feedback_intensity: not readable: java.lang.SecurityException: Settings key: <hardware_haptic_feedback_intensity> is not readable. From S+, settings keys annotated with @hide are restricted to system_server and system apps only, unless they are annotated with @Readable.
Settings.System keyboard_vibration_enabled: 1
Settings.System VIB_FEEDBACK_MAGNITUDE: unset
Settings.System SEM_VIBRATION_FORCE_TOUCH_INTENSITY: unset
```

## Comparison

- Both phones support every primitive and effect the lab plays, with amplitude control, so the
  button list is identical and nothing is greyed on either.
- The click primitive is 20 ms on the Samsung and 26 ms on the Pixel; TICK 20 ms against 5 ms,
  LOW_TICK 20 ms against 12 ms. The Samsung's equal 20 ms for CLICK, TICK and LOW_TICK suggests
  one generic waveform behind all three.
- Only the Pixel reports its actuator: resonant frequency 146 Hz, Q factor 18.8. Neither has a
  frequency profile or envelope effects (`areEnvelopeEffectsSupported` false); the public
  `getEnvelopeEffectInfo` getter isn't on `SystemVibrator` on either phone (the hidden
  `VibratorInfo` shows envelope size 0 anyway).
- Touch feedback is on on both; Samsung's `VIB_FEEDBACK_MAGNITUDE` (2) exists only there.

## By feel (Konst, 2026-11-17)

Samsung S26 Ultra (SM-S948U1, SDK 37; no resonant frequency or Q factor; CLICK, TICK and
LOW_TICK all 20 ms):

- Not felt: E3, P2, P3.
- Weak but felt: P1, H1–H4 (H1 the weakest), P4.
- A3 (255 for 10 ms) is the best; the buzz starts at A4 (15 ms). A7 (128 for 15 ms) is clearly
  stronger than A8 (60 for 15 ms), and both are felt.
- E2 (heavy click) is very good.
- Between D1 and D2 is good; D3 buzzes. P5 and P6 buzz.

Pixel 7 Pro (SDK 36; resonant frequency 146 Hz, Q factor 18.8; CLICK 26 ms, TICK 5 ms,
LOW_TICK 12 ms):

- Everything is felt.
- P1 is better than A3; E2 is good and competes with P1.
- Among the A entries A1 (5 ms) is the best; the buzz starts at A4 (15 ms). A7 is better than A8.

## What the card takes from it

- **Click** (`PRIMITIVE_CLICK`, scaled by the strength) is the best tap on a calibrated
  actuator (the Pixel: P1), and weak on the Samsung, whose primitives look like one generic
  20 ms waveform.
- **Pulse** (an amplitude one-shot) is the best tap on the Samsung (A3) and good on the Pixel.
  Both phones buzz from 15 ms, so its duration goes from 5 to 12 ms (8 by default). Amplitude
  60 (A8) is weak but felt on both, so strength 1 % is amplitude 60 and 100 % is 255.
- **Heavy click** (`EFFECT_HEAVY_CLICK`, E2) is very good on the Samsung and good on the Pixel,
  but Android has no strength for a predefined effect: the phone sets it.
- **Auto** tells the two kinds of vibrator apart by what they report: a calibrated actuator
  has a resonant frequency (`VibratorInfo.getResonantFrequency` not NaN) and a click primitive
  of its own (`getPrimitiveDurations`: CLICK's duration differs from TICK's). Then Auto is
  Click; otherwise Pulse where the vibrator has amplitude control, otherwise a one-shot whose
  duration follows the strength. So the Pixel gets Click and the Samsung Pulse.
