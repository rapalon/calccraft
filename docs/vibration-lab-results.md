# Vibration lab results

The capabilities dumps of the phones tried (`adb logcat -d -s CalcCraftVibLab` after opening
the lab page, docs/vibration-lab.md), from the test build `CalcCraft-Play-Lab-1.3.2-arm64.apk`.

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
