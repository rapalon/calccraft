//! The Android key vibration (Settings → Key vibration): the strength, style and duration the
//! card sets, the rule the Auto style follows, and what they mean for each way
//! `android::key_tap` can make the motor tap. Pure functions, so they are tested on every
//! platform; only the Android app uses them. The numbers come from the vibration lab's results
//! by feel on two phones (`docs/vibration-lab-results.md`).
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::sync::atomic::{AtomicU8, Ordering};

/// Storage keys, which Windows doesn't have (named like `CalcCraftLanguage`).
pub(super) const STORAGE_KEY: &str = "CalcCraftKeyVibration";
pub(super) const STYLE_KEY: &str = "CalcCraftKeyVibrationStyle";
pub(super) const DURATION_KEY: &str = "CalcCraftKeyVibrationDuration";
/// The strength slider's ends: 0 is Off, 100 the strongest tap.
pub(super) const MAX: u8 = 100;
/// The strength of a new install: clearly felt on every phone tried, still a click.
pub(super) const DEFAULT: u8 = 50;

/// The Pulse's duration slider, in milliseconds. Both phones tried buzz from 15 ms (the lab's
/// A4), so a tap never lasts longer than 12; the Pixel's best was 5 ms, the Samsung's 10.
pub(super) const MIN_MS: u8 = 5;
pub(super) const MAX_MS: u8 = 12;
pub(super) const DEFAULT_MS: u8 = 8;

/// The Pulse's amplitude at 1 %: 60 for 15 ms (the lab's A8) was weak but felt on both phones.
pub(super) const MIN_AMPLITUDE: i32 = 60;

/// Whether this build has the setting: only the Android app vibrates.
pub(super) const AVAILABLE: bool = cfg!(target_os = "android");

/// The card's Style drop-down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum Style {
    /// Click or Pulse, whichever suits the phone's vibrator (`auto`).
    #[default]
    Auto,
    Click,
    Pulse,
    HeavyClick,
}

impl Style {
    /// The drop-down's entries, in order.
    pub(super) const ALL: [Style; 4] = [Style::Auto, Style::Click, Style::Pulse, Style::HeavyClick];

    /// The stored value.
    pub(super) fn key(self) -> &'static str {
        match self {
            Style::Auto => "Auto",
            Style::Click => "Click",
            Style::Pulse => "Pulse",
            Style::HeavyClick => "HeavyClick",
        }
    }

    /// The name's string key in `locales/calccraft.txt`.
    pub(super) fn label_key(self) -> &'static str {
        match self {
            Style::Auto => "CalcCraft.KeyVibrationStyleAuto",
            Style::Click => "CalcCraft.KeyVibrationStyleClick",
            Style::Pulse => "CalcCraft.KeyVibrationStylePulse",
            Style::HeavyClick => "CalcCraft.KeyVibrationStyleHeavyClick",
        }
    }

    /// The stored style, or Auto for a missing or unknown value.
    pub(super) fn parse(stored: Option<&str>) -> Style {
        let stored = stored.map(str::trim);
        Style::ALL
            .into_iter()
            .find(|style| Some(style.key()) == stored)
            .unwrap_or_default()
    }
}

/// What the phone's vibrator can do, as Android reports it (`android::capabilities`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Capabilities {
    /// `Build.VERSION.SDK_INT`.
    pub sdk: i32,
    /// `Vibrator.hasAmplitudeControl` (Android 8+).
    pub amplitude_control: bool,
    /// `areAllPrimitivesSupported(PRIMITIVE_CLICK)` (Android 11+).
    pub click_primitive: bool,
    /// `Vibrator.getResonantFrequency` (Android 12+), NaN where the vibrator doesn't say.
    pub resonant_frequency: f32,
    /// `getPrimitiveDurations` of `PRIMITIVE_CLICK` and `PRIMITIVE_TICK` (Android 12+), 0 where
    /// unknown or unsupported.
    pub click_ms: i32,
    pub tick_ms: i32,
}

impl Capabilities {
    /// A vibrator that can do everything, for builds without one (the card's layout there).
    pub(super) const FULL: Capabilities = Capabilities {
        sdk: 36,
        amplitude_control: true,
        click_primitive: true,
        resonant_frequency: 150.0,
        click_ms: 26,
        tick_ms: 5,
    };

    /// Whether a `VibrationEffect` one-shot can set its amplitude.
    fn pulses(&self) -> bool {
        self.sdk >= 26 && self.amplitude_control
    }

    /// Whether a `PRIMITIVE_CLICK` composition plays.
    fn clicks(&self) -> bool {
        self.sdk >= 30 && self.click_primitive
    }

    /// Whether the vibrator looks calibrated: it knows its resonant frequency, and its click
    /// primitive is a waveform of its own. The Samsung S26 Ultra reports no frequency and
    /// 20 ms for CLICK, TICK and LOW_TICK alike (one generic waveform: its clicks were weak);
    /// the Pixel 7 Pro 146 Hz, a 26 ms CLICK and a 5 ms TICK (its click was the best tap).
    pub(super) fn calibrated(&self) -> bool {
        self.sdk >= 31
            && !self.resonant_frequency.is_nan()
            && self.click_ms > 0
            && self.click_ms != self.tick_ms
    }
}

/// How a key tap plays: the style in effect on this phone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tap {
    /// A `PRIMITIVE_CLICK` composition scaled by the strength (`click_scale`).
    Click,
    /// A one-shot of `ms` milliseconds whose amplitude follows the strength (`amplitude`).
    Pulse { ms: u8 },
    /// A one-shot whose duration follows the strength (`duration_ms`): a vibrator without
    /// amplitude control, or Android before 8.
    Duration,
    /// The predefined `EFFECT_HEAVY_CLICK`, at the phone's own strength.
    HeavyClick,
}

/// Auto's pick: Click on a calibrated vibrator, else Pulse at the default duration where the
/// amplitude can be set, else the duration fallback.
pub(super) fn auto(capabilities: &Capabilities) -> Tap {
    if capabilities.calibrated() && capabilities.clicks() {
        Tap::Click
    } else {
        pulse(capabilities, DEFAULT_MS)
    }
}

/// A Pulse of `ms`, or the duration fallback where the amplitude can't be set.
fn pulse(capabilities: &Capabilities, ms: u8) -> Tap {
    if capabilities.pulses() {
        Tap::Pulse { ms: clamp_ms(ms) }
    } else {
        Tap::Duration
    }
}

/// How `style` plays on this vibrator, with `ms` the Duration slider's value. A style the
/// phone can't play falls back to a Pulse (Click without the primitive, Heavy click before
/// Android 10, where `createPredefined` is missing).
pub(super) fn tap(style: Style, ms: u8, capabilities: &Capabilities) -> Tap {
    match style {
        Style::Auto => auto(capabilities),
        Style::Click if capabilities.clicks() => Tap::Click,
        Style::HeavyClick if capabilities.sdk >= 29 => Tap::HeavyClick,
        Style::Pulse => pulse(capabilities, ms),
        Style::Click | Style::HeavyClick => pulse(capabilities, DEFAULT_MS),
    }
}

/// A row of the Key vibration card, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Row {
    /// The title, the description and the strength slider.
    Strength,
    /// "Strength is set by the phone for this style" (Heavy click: the slider is only on/off).
    FixedStrength,
    /// The Style drop-down.
    Style,
    /// The Duration slider: only for a Pulse chosen explicitly (not Auto's), and only where it
    /// pulses (without amplitude control the strength sets the duration).
    Duration,
    /// "Touch feedback is turned off in your phone's settings".
    SystemOff,
}

/// The card's rows for `style` on this vibrator, with touch feedback on or off.
pub(super) fn card_rows(
    style: Style,
    capabilities: &Capabilities,
    touch_feedback: bool,
) -> Vec<Row> {
    let tap = tap(style, DEFAULT_MS, capabilities);
    let mut rows = vec![Row::Strength];
    if tap == Tap::HeavyClick {
        rows.push(Row::FixedStrength);
    }
    rows.push(Row::Style);
    if style == Style::Pulse && matches!(tap, Tap::Pulse { .. }) {
        rows.push(Row::Duration);
    }
    if !touch_feedback {
        rows.push(Row::SystemOff);
    }
    rows
}

/// The settings in effect, read by every key press wherever it is drawn.
static STRENGTH: AtomicU8 = AtomicU8::new(DEFAULT);
static STYLE: AtomicU8 = AtomicU8::new(0);
static DURATION: AtomicU8 = AtomicU8::new(DEFAULT_MS);

/// The strength in effect (0 is Off).
pub(super) fn strength() -> u8 {
    STRENGTH.load(Ordering::Relaxed)
}

/// Sets the strength in effect, clamped to `MAX`.
pub(super) fn set_strength(strength: u8) {
    STRENGTH.store(strength.min(MAX), Ordering::Relaxed);
}

/// The style in effect.
pub(super) fn style() -> Style {
    Style::ALL[usize::from(STYLE.load(Ordering::Relaxed)).min(Style::ALL.len() - 1)]
}

/// Sets the style in effect.
pub(super) fn set_style(style: Style) {
    let index = Style::ALL.iter().position(|s| *s == style).unwrap_or(0);
    STYLE.store(index as u8, Ordering::Relaxed);
}

/// The Pulse's duration in effect, in milliseconds.
pub(super) fn duration() -> u8 {
    DURATION.load(Ordering::Relaxed)
}

/// Sets the Pulse's duration in effect, clamped to `MIN_MS..=MAX_MS`.
pub(super) fn set_duration(ms: u8) {
    DURATION.store(clamp_ms(ms), Ordering::Relaxed);
}

fn clamp_ms(ms: u8) -> u8 {
    ms.clamp(MIN_MS, MAX_MS)
}

/// The stored strength, or the default for a missing or unreadable value.
pub(super) fn parse(stored: Option<&str>) -> u8 {
    stored
        .and_then(|text| text.trim().parse::<u8>().ok())
        .map_or(DEFAULT, |strength| strength.min(MAX))
}

/// The stored duration, clamped to `MIN_MS..=MAX_MS`, or the default for a missing or
/// unreadable value.
pub(super) fn parse_duration(stored: Option<&str>) -> u8 {
    stored
        .and_then(|text| text.trim().parse::<u8>().ok())
        .map_or(DEFAULT_MS, clamp_ms)
}

/// The share of the full strength (0 to 1) the slider asks for, perceptually even. A tap's
/// felt intensity grows much faster than its drive at the bottom and flattens at the top,
/// so a linear mapping leaves the lower half of the slider all but numb. The square root
/// spreads it: 25 is half the drive, 50 is 71 %, a firm click, and 1 is 10 %.
fn level(strength: u8) -> f32 {
    (f32::from(strength.min(MAX)) / f32::from(MAX)).sqrt()
}

/// Where `strength` (1 to 100) lies between 1 and 100, from 0 to 1.
fn linear(strength: u8) -> f32 {
    f32::from(strength.clamp(1, MAX) - 1) / f32::from(MAX - 1)
}

/// The scale of `VibrationEffect.Composition.addPrimitive(PRIMITIVE_CLICK, scale)`, 0 to 1,
/// or `None` for Off. The click primitive is the phone maker's own click waveform, so even
/// scale 1 stays a click; the scale only sets its strength.
pub(super) fn click_scale(strength: u8) -> Option<f32> {
    (strength > 0).then(|| level(strength))
}

/// The amplitude (60 to 255) of a Pulse, or `None` for Off: linear from `MIN_AMPLITUDE` at 1,
/// the faintest tap felt on both phones tried, to 255 at 100 (157 at 50). The floor is felt
/// already, so a square root as for the click's scale isn't needed.
pub(super) fn amplitude(strength: u8) -> Option<i32> {
    (strength > 0).then(|| {
        let span = (255 - MIN_AMPLITUDE) as f32;
        MIN_AMPLITUDE + (span * linear(strength)).round() as i32
    })
}

/// The duration of a one-shot on a vibrator without amplitude control, or `None` for Off:
/// `MIN_MS` at 1 up to `MAX_MS` at 100, linearly (5, 7, 8 and 12 ms at 1, 25, 50 and 100), since
/// an on/off motor's tap is felt in proportion to how long it runs; never longer than a
/// Pulse, which buzzes from 15 ms.
pub(super) fn duration_ms(strength: u8) -> Option<i64> {
    (strength > 0).then(|| {
        let span = f32::from(MAX_MS - MIN_MS);
        i64::from(MIN_MS) + (span * linear(strength)).round() as i64
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Pixel 7 Pro's dump: 146 Hz, CLICK 26 ms, TICK 5 ms (`docs/vibration-lab-results.md`).
    const PIXEL: Capabilities = Capabilities {
        sdk: 36,
        amplitude_control: true,
        click_primitive: true,
        resonant_frequency: 145.95691,
        click_ms: 26,
        tick_ms: 5,
    };
    /// The Samsung S26 Ultra's: no resonant frequency, CLICK and TICK both 20 ms.
    const SAMSUNG: Capabilities = Capabilities {
        sdk: 37,
        amplitude_control: true,
        click_primitive: true,
        resonant_frequency: f32::NAN,
        click_ms: 20,
        tick_ms: 20,
    };

    #[test]
    fn stored_settings() {
        assert_eq!(parse(None), 50);
        assert_eq!(parse(Some("0")), 0);
        assert_eq!(parse(Some("73")), 73);
        assert_eq!(parse(Some("250")), 100);
        assert_eq!(parse(Some("-3")), 50);
        assert_eq!(parse(Some("loud")), 50);
        assert_eq!(parse_duration(None), 8);
        assert_eq!(parse_duration(Some("10")), 10);
        assert_eq!(parse_duration(Some("3")), 5);
        assert_eq!(parse_duration(Some("40")), 12);
        assert_eq!(parse_duration(Some("long")), 8);
        for style in Style::ALL {
            assert_eq!(Style::parse(Some(style.key())), style);
        }
        assert_eq!(Style::parse(None), Style::Auto);
        assert_eq!(Style::parse(Some("Buzz")), Style::Auto);
        assert_eq!(Style::parse(Some("HeavyClick")), Style::HeavyClick);
    }

    #[test]
    fn settings_in_effect() {
        set_style(Style::Pulse);
        assert_eq!(style(), Style::Pulse);
        set_style(Style::Auto);
        assert_eq!(style(), Style::Auto);
        set_duration(30);
        assert_eq!(duration(), MAX_MS);
        set_duration(DEFAULT_MS);
        assert_eq!(duration(), 8);
    }

    #[test]
    fn auto_picks_by_the_vibrator() {
        // Pixel-like: a calibrated actuator with its own click.
        assert_eq!(auto(&PIXEL), Tap::Click);
        // Samsung-like: no resonant frequency, one generic waveform.
        assert_eq!(auto(&SAMSUNG), Tap::Pulse { ms: 8 });
        // A resonant frequency alone isn't enough, nor different durations alone.
        let same_waveform = Capabilities {
            tick_ms: 26,
            ..PIXEL
        };
        assert_eq!(auto(&same_waveform), Tap::Pulse { ms: 8 });
        let no_frequency = Capabilities {
            resonant_frequency: f32::NAN,
            ..PIXEL
        };
        assert_eq!(auto(&no_frequency), Tap::Pulse { ms: 8 });
        // Calibrated, but the click primitive isn't supported.
        let no_click = Capabilities {
            click_primitive: false,
            click_ms: 0,
            ..PIXEL
        };
        assert_eq!(auto(&no_click), Tap::Pulse { ms: 8 });
        // No amplitude control: the duration fallback.
        let on_off = Capabilities {
            amplitude_control: false,
            ..SAMSUNG
        };
        assert_eq!(auto(&on_off), Tap::Duration);
        // Before Android 11 (API 30) there are no primitives and no VibratorInfo: Pulse, or
        // the duration fallback without amplitude control or before Android 8.
        let api_29 = Capabilities {
            sdk: 29,
            click_primitive: false,
            resonant_frequency: f32::NAN,
            click_ms: 0,
            tick_ms: 0,
            ..PIXEL
        };
        assert_eq!(auto(&api_29), Tap::Pulse { ms: 8 });
        // Even if a value leaked through, API 30 has no getResonantFrequency.
        assert_eq!(
            auto(&Capabilities { sdk: 30, ..PIXEL }),
            Tap::Pulse { ms: 8 }
        );
        let api_29_on_off = Capabilities {
            amplitude_control: false,
            ..api_29
        };
        assert_eq!(auto(&api_29_on_off), Tap::Duration);
        assert_eq!(auto(&Capabilities { sdk: 25, ..api_29 }), Tap::Duration);
    }

    #[test]
    fn styles_play_as_chosen_or_fall_back() {
        assert_eq!(tap(Style::Click, 8, &SAMSUNG), Tap::Click);
        assert_eq!(tap(Style::Pulse, 10, &PIXEL), Tap::Pulse { ms: 10 });
        assert_eq!(tap(Style::Pulse, 30, &PIXEL), Tap::Pulse { ms: 12 });
        assert_eq!(tap(Style::HeavyClick, 8, &SAMSUNG), Tap::HeavyClick);
        // Auto ignores the Duration slider, which it hides.
        assert_eq!(tap(Style::Auto, 12, &SAMSUNG), Tap::Pulse { ms: 8 });
        let old = Capabilities {
            sdk: 28,
            click_primitive: false,
            ..SAMSUNG
        };
        assert_eq!(tap(Style::Click, 10, &old), Tap::Pulse { ms: 8 });
        assert_eq!(tap(Style::HeavyClick, 10, &old), Tap::Pulse { ms: 8 });
        let on_off = Capabilities {
            amplitude_control: false,
            ..SAMSUNG
        };
        assert_eq!(tap(Style::Pulse, 10, &on_off), Tap::Duration);
    }

    #[test]
    fn card_rows_by_style() {
        use super::Style as S;
        use Row::*;
        assert_eq!(card_rows(S::Auto, &PIXEL, true), [Strength, Style]);
        // Auto picking Pulse doesn't show the duration either.
        assert_eq!(card_rows(S::Auto, &SAMSUNG, true), [Strength, Style]);
        assert_eq!(card_rows(S::Click, &PIXEL, true), [Strength, Style]);
        assert_eq!(
            card_rows(S::Pulse, &SAMSUNG, true),
            [Strength, Style, Duration]
        );
        assert_eq!(
            card_rows(S::Pulse, &PIXEL, false),
            [Strength, Style, Duration, SystemOff]
        );
        // Without amplitude control the strength sets the duration: no Duration slider.
        let on_off = Capabilities {
            amplitude_control: false,
            ..SAMSUNG
        };
        assert_eq!(card_rows(S::Pulse, &on_off, true), [Strength, Style]);
        assert_eq!(
            card_rows(S::HeavyClick, &SAMSUNG, true),
            [Strength, FixedStrength, Style]
        );
        assert_eq!(
            card_rows(S::HeavyClick, &SAMSUNG, false),
            [Strength, FixedStrength, Style, SystemOff]
        );
        // Heavy click falling back to a Pulse has a strength again, but no Duration slider.
        let old = Capabilities { sdk: 28, ..SAMSUNG };
        assert_eq!(card_rows(S::HeavyClick, &old, true), [Strength, Style]);
    }

    #[test]
    fn off_never_vibrates() {
        assert_eq!(click_scale(0), None);
        assert_eq!(amplitude(0), None);
        assert_eq!(duration_ms(0), None);
    }

    #[test]
    fn click_primitive_scale() {
        let scale = |s| click_scale(s).unwrap();
        assert!((scale(1) - 0.1).abs() < 1e-6);
        assert!((scale(25) - 0.5).abs() < 1e-6);
        assert!((scale(50) - 0.70710677).abs() < 1e-6);
        assert_eq!(scale(100), 1.0);
        assert!((1..100).all(|s| scale(s) < scale(s + 1)));
    }

    #[test]
    fn pulse_amplitude() {
        let amplitude = |s| amplitude(s).unwrap();
        assert_eq!([1, 25, 50, 100].map(amplitude), [60, 107, 157, 255]);
        assert!((1..100).all(|s| amplitude(s) <= amplitude(s + 1)));
        assert!((1..=100).all(|s| (60..=255).contains(&amplitude(s))));
        assert_eq!(amplitude(200), 255);
    }

    #[test]
    fn fallback_duration() {
        let duration = |s| duration_ms(s).unwrap();
        assert_eq!([1, 25, 50, 100].map(duration), [5, 7, 8, 12]);
        assert!((1..100).all(|s| duration(s) <= duration(s + 1)));
        assert!((1..=100).all(|s| (5..=12).contains(&duration(s))));
        assert_eq!(duration_ms(200), Some(12));
    }
}
