//! The Android app's Screen orientation setting (Settings → Screen orientation): Auto follows
//! the system (auto-rotate on: the app turns with the device; rotation lock on: it stays in the
//! orientation the system is locked in), Portrait and Landscape keep the app in that
//! orientation whatever the system's setting. `android::set_orientation` hands the choice to
//! `Activity.setRequestedOrientation`. Pure, so it is tested on every platform; only the Android
//! app shows the setting.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::sync::atomic::{AtomicU8, Ordering};

/// Storage key, which Windows doesn't have (named like `CalcCraftLanguage`).
pub(super) const STORAGE_KEY: &str = "CalcCraftScreenOrientation";

/// Whether this build has the setting: only the Android app can turn its screen. Desktop
/// windows and browser pages take the shape the user gives them.
pub(super) const AVAILABLE: bool = cfg!(target_os = "android");

/// `ActivityInfo.SCREEN_ORIENTATION_USER`: the orientation the user prefers, which is the
/// device's position with auto-rotate on and the locked orientation with rotation lock on.
const SCREEN_ORIENTATION_USER: i32 = 2;
/// `ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE` and `SENSOR_PORTRAIT`: that orientation
/// whatever the system's setting, upright or upside down as the device is held. Unlike
/// `USER_LANDSCAPE`, which keeps one side under rotation lock, the content is never upside down
/// for whoever turned the phone the other way, as in video players.
const SCREEN_ORIENTATION_SENSOR_LANDSCAPE: i32 = 6;
const SCREEN_ORIENTATION_SENSOR_PORTRAIT: i32 = 7;

/// The setting's choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum Orientation {
    /// Follows the system's auto-rotate and rotation lock.
    #[default]
    Auto,
    Portrait,
    Landscape,
}

impl Orientation {
    /// The drop-down's entries, in order.
    pub(super) const ALL: [Orientation; 3] = [
        Orientation::Auto,
        Orientation::Portrait,
        Orientation::Landscape,
    ];

    /// The stored value.
    pub(super) fn key(self) -> &'static str {
        match self {
            Orientation::Auto => "Auto",
            Orientation::Portrait => "Portrait",
            Orientation::Landscape => "Landscape",
        }
    }

    /// The name's string key in `locales/calccraft.txt`.
    pub(super) fn label_key(self) -> &'static str {
        match self {
            Orientation::Auto => "CalcCraft.ScreenOrientationAuto",
            Orientation::Portrait => "CalcCraft.ScreenOrientationPortrait",
            Orientation::Landscape => "CalcCraft.ScreenOrientationLandscape",
        }
    }

    /// The stored choice, or Auto for a missing or unknown value.
    pub(super) fn parse(stored: Option<&str>) -> Orientation {
        let stored = stored.map(str::trim);
        Orientation::ALL
            .into_iter()
            .find(|orientation| Some(orientation.key()) == stored)
            .unwrap_or_default()
    }

    /// The `ActivityInfo.SCREEN_ORIENTATION_*` value for `Activity.setRequestedOrientation`.
    pub(super) fn requested(self) -> i32 {
        match self {
            Orientation::Auto => SCREEN_ORIENTATION_USER,
            Orientation::Portrait => SCREEN_ORIENTATION_SENSOR_PORTRAIT,
            Orientation::Landscape => SCREEN_ORIENTATION_SENSOR_LANDSCAPE,
        }
    }
}

/// Whether Android honors an app's orientation: from Android 16 (API 36) it ignores it on
/// large screens, whose smallest width is 600 dp or more, for apps that target it, as this one
/// does. The setting is left out there: the app turns with the device whatever it asks.
pub(super) fn honored(sdk: i32, smallest_width_dp: i32) -> bool {
    sdk < 36 || smallest_width_dp < 600
}

/// The choice in effect, for the Settings card and `android::set_orientation`.
static CURRENT: AtomicU8 = AtomicU8::new(0);

pub(super) fn current() -> Orientation {
    Orientation::ALL
        .get(usize::from(CURRENT.load(Ordering::Relaxed)))
        .copied()
        .unwrap_or_default()
}

pub(super) fn set(orientation: Orientation) {
    let index = Orientation::ALL
        .iter()
        .position(|o| *o == orientation)
        .unwrap_or_default();
    CURRENT.store(index as u8, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_values() {
        for orientation in Orientation::ALL {
            assert_eq!(Orientation::parse(Some(orientation.key())), orientation);
        }
        // A new install, and anything a later version might store.
        assert_eq!(Orientation::parse(None), Orientation::Auto);
        assert_eq!(Orientation::parse(Some("Upside down")), Orientation::Auto);
        assert_eq!(
            Orientation::parse(Some(" Landscape ")),
            Orientation::Landscape
        );
    }

    #[test]
    fn requested_orientations() {
        // Auto is the user's own setting; the others ignore it but use both sides.
        assert_eq!(Orientation::Auto.requested(), 2);
        assert_eq!(Orientation::Portrait.requested(), 7);
        assert_eq!(Orientation::Landscape.requested(), 6);
    }

    #[test]
    fn android_16_ignores_it_on_large_screens() {
        assert!(honored(35, 800));
        assert!(honored(36, 412));
        assert!(!honored(36, 600));
        assert!(!honored(37, 800));
    }

    #[test]
    fn current_choice() {
        for orientation in Orientation::ALL {
            set(orientation);
            assert_eq!(current(), orientation);
        }
        set(Orientation::Auto);
    }
}
