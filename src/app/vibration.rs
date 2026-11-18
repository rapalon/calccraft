//! The Android key vibration's strength (Settings → Key vibration): the slider's value and
//! what it means for each way `android::key_tap` can make the motor tap. Pure functions, so
//! they are tested on every platform; only the Android app uses them.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::sync::atomic::{AtomicU8, Ordering};

/// Storage key of the strength, which Windows doesn't have (named like `CalcCraftLanguage`).
pub(super) const STORAGE_KEY: &str = "CalcCraftKeyVibration";
/// The slider's ends: 0 is Off, 100 the strongest tap.
pub(super) const MAX: u8 = 100;
/// The strength of a new install: clearly felt on every phone tried, still a click.
pub(super) const DEFAULT: u8 = 50;

/// The duration of a one-shot tap on a vibrator with amplitude control. A linear resonant
/// actuator (about 150–235 Hz) needs some two periods to reach the amplitude it is driven
/// at: 14 ms gives it that, and still ends before it reads as a buzz.
pub(super) const AMPLITUDE_MS: i64 = 14;
/// The shortest and longest one-shot on a vibrator without amplitude control, where the
/// duration is all there is: an eccentric motor barely spins up in 8 ms, and beyond 40 ms a
/// tap becomes a buzz.
pub(super) const MIN_MS: i64 = 8;
pub(super) const MAX_MS: i64 = 40;

/// Whether this build has the setting: only the Android app vibrates.
pub(super) const AVAILABLE: bool = cfg!(target_os = "android");

/// The strength in effect, read by every key press wherever it is drawn.
static STRENGTH: AtomicU8 = AtomicU8::new(DEFAULT);

/// The strength in effect (0 is Off).
pub(super) fn strength() -> u8 {
    STRENGTH.load(Ordering::Relaxed)
}

/// Sets the strength in effect, clamped to `MAX`.
pub(super) fn set_strength(strength: u8) {
    STRENGTH.store(strength.min(MAX), Ordering::Relaxed);
}

/// The stored strength, or the default for a missing or unreadable value.
pub(super) fn parse(stored: Option<&str>) -> u8 {
    stored
        .and_then(|text| text.trim().parse::<u8>().ok())
        .map_or(DEFAULT, |strength| strength.min(MAX))
}

/// The share of the full strength (0 to 1) the slider asks for, perceptually even. A tap's
/// felt intensity grows much faster than its drive at the bottom and flattens at the top,
/// so a linear mapping leaves the lower half of the slider all but numb (Samsung's
/// `KEYBOARD_TAP`, at a low fixed scale, is barely felt). The square root spreads it: 25 is
/// half the drive, 50 is 71 %, a firm click, and 1 is 10 %, the faintest tap still felt.
fn level(strength: u8) -> f32 {
    (f32::from(strength.min(MAX)) / f32::from(MAX)).sqrt()
}

/// The scale of `VibrationEffect.Composition.addPrimitive(PRIMITIVE_CLICK, scale)`, 0 to 1,
/// or `None` for Off. The click primitive is the phone maker's own crisp click waveform, so
/// even scale 1 stays a click; the scale only sets its strength.
pub(super) fn click_scale(strength: u8) -> Option<f32> {
    (strength > 0).then(|| level(strength))
}

/// The amplitude (1 to 255) of a `AMPLITUDE_MS` one-shot on a vibrator with amplitude
/// control, or `None` for Off: 26 at 1, 128 at 25, 180 at 50, 255 at 100.
pub(super) fn amplitude(strength: u8) -> Option<i32> {
    (strength > 0).then(|| ((level(strength) * 255.0).round() as i32).clamp(1, 255))
}

/// The duration of a one-shot on a vibrator without amplitude control, or `None` for Off:
/// `MIN_MS` at 1 up to `MAX_MS` at 100, linearly (8, 16, 24 and 40 ms at 1, 25, 50 and 100),
/// since an on/off motor's tap is felt in proportion to how long it spins.
pub(super) fn duration_ms(strength: u8) -> Option<i64> {
    (strength > 0).then(|| {
        let t = f32::from(strength.min(MAX) - 1) / f32::from(MAX - 1);
        MIN_MS + ((MAX_MS - MIN_MS) as f32 * t).round() as i64
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_strength() {
        assert_eq!(parse(None), 50);
        assert_eq!(parse(Some("0")), 0);
        assert_eq!(parse(Some("73")), 73);
        assert_eq!(parse(Some("250")), 100);
        assert_eq!(parse(Some("-3")), 50);
        assert_eq!(parse(Some("loud")), 50);
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
    fn one_shot_amplitude() {
        let amplitude = |s| amplitude(s).unwrap();
        assert_eq!(
            [1, 25, 50, 100].map(amplitude),
            [26, 128, 180, 255],
            "1, 25, 50, 100"
        );
        assert!((1..100).all(|s| amplitude(s) <= amplitude(s + 1)));
        assert!((1..=100).all(|s| (1..=255).contains(&amplitude(s))));
    }

    #[test]
    fn one_shot_duration() {
        let duration = |s| duration_ms(s).unwrap();
        assert_eq!([1, 25, 50, 100].map(duration), [8, 16, 24, 40]);
        assert!((1..100).all(|s| duration(s) <= duration(s + 1)));
        assert!((1..=100).all(|s| (MIN_MS..=MAX_MS).contains(&duration(s))));
        assert_eq!(duration_ms(200), Some(40));
    }
}
