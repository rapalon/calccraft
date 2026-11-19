//! The Colour theme setting (CalcCraft's own, docs/color-themes.md): the themes, their stored
//! keys, and which of them a build unlocks. Pure: the palettes are in `palette.rs`, the card in
//! `app.rs`.

use crate::channel::Channel;

/// Storage key of the choice: the theme's `key`.
pub(super) const STORAGE_KEY: &str = "CalcCraftColorTheme";

/// A colour theme. The order is the card's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum ColorTheme {
    /// Today's look, in every build.
    Classic,
    /// Classic with a metallic gold `=`: CalcCraft Pro's default.
    Gold,
    /// Dim red on near-black, for the dark.
    Night,
    /// A greenish-grey LCD display with dark digits.
    RetroLcd,
    /// Cream and ink.
    Paper,
}

/// Whether a build lets the user pick a theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Access {
    Unlocked,
    /// Shown with a PRO pill; a tap opens the paid app's listing.
    Locked,
    /// Not shown at all.
    Hidden,
}

impl ColorTheme {
    pub(super) const ALL: [ColorTheme; 5] = [
        ColorTheme::Classic,
        ColorTheme::Gold,
        ColorTheme::Night,
        ColorTheme::RetroLcd,
        ColorTheme::Paper,
    ];

    /// The stored value.
    pub(super) fn key(self) -> &'static str {
        match self {
            ColorTheme::Classic => "Classic",
            ColorTheme::Gold => "Gold",
            ColorTheme::Night => "Night",
            ColorTheme::RetroLcd => "RetroLcd",
            ColorTheme::Paper => "Paper",
        }
    }

    pub(super) fn from_key(key: &str) -> Option<ColorTheme> {
        Self::ALL.into_iter().find(|theme| theme.key() == key)
    }

    /// The resource key of its name.
    pub(super) fn label_key(self) -> &'static str {
        match self {
            ColorTheme::Classic => "CalcCraft.ThemeClassic",
            ColorTheme::Gold => "CalcCraft.ThemeGold",
            ColorTheme::Night => "CalcCraft.ThemeNight",
            ColorTheme::RetroLcd => "CalcCraft.ThemeRetroLcd",
            ColorTheme::Paper => "CalcCraft.ThemePaper",
        }
    }

    /// Whether it is one of CalcCraft Pro's themes.
    pub(super) fn is_pro(self) -> bool {
        self != ColorTheme::Classic
    }

    /// Whether a build of `channel`, whose paid app is `paid_id`, offers it.
    pub(super) fn access(self, channel: Channel, paid_id: Option<&str>) -> Access {
        match channel {
            _ if !self.is_pro() => Access::Unlocked,
            // Open source with Donate, and the paid app: everything.
            Channel::Donate | Channel::PlayPaid => Access::Unlocked,
            Channel::PlayFree if paid_id.is_some() => Access::Locked,
            // Nothing to lead to: the Pro themes aren't mentioned.
            Channel::PlayFree => Access::Hidden,
        }
    }

    /// The theme a build of `channel` starts with.
    pub(super) fn default_for(channel: Channel) -> ColorTheme {
        match channel {
            Channel::PlayPaid => ColorTheme::Gold,
            Channel::Donate | Channel::PlayFree => ColorTheme::Classic,
        }
    }

    /// The theme for a `stored` key: the stored theme if the build unlocks it, else the
    /// channel's default (a free-app user never ends up on a Pro theme).
    pub(super) fn load(stored: Option<&str>, channel: Channel, paid_id: Option<&str>) -> Self {
        stored
            .and_then(ColorTheme::from_key)
            .filter(|theme| theme.access(channel, paid_id) == Access::Unlocked)
            .unwrap_or(ColorTheme::default_for(channel))
    }
}

/// This build's access to `theme`.
pub(super) fn access(theme: ColorTheme) -> Access {
    theme.access(crate::channel::CHANNEL, crate::channel::paid_id())
}

/// This build's theme for a stored key.
pub(super) fn load(stored: Option<&str>) -> ColorTheme {
    ColorTheme::load(stored, crate::channel::CHANNEL, crate::channel::paid_id())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRO: Option<&str> = Some("com.rapalon.calccraft.pro");

    #[test]
    fn keys_round_trip() {
        for theme in ColorTheme::ALL {
            assert_eq!(ColorTheme::from_key(theme.key()), Some(theme));
        }
        assert_eq!(ColorTheme::from_key("Sepia"), None);
        assert!(!ColorTheme::Classic.is_pro());
        assert!(ColorTheme::ALL[1..].iter().all(|theme| theme.is_pro()));
    }

    #[test]
    fn access_by_channel() {
        for theme in ColorTheme::ALL {
            assert_eq!(theme.access(Channel::Donate, None), Access::Unlocked);
            assert_eq!(theme.access(Channel::PlayPaid, PRO), Access::Unlocked);
            let (free, free_without) = if theme.is_pro() {
                (Access::Locked, Access::Hidden)
            } else {
                (Access::Unlocked, Access::Unlocked)
            };
            assert_eq!(theme.access(Channel::PlayFree, PRO), free);
            assert_eq!(theme.access(Channel::PlayFree, None), free_without);
        }
    }

    #[test]
    fn defaults_and_fallbacks() {
        assert_eq!(
            ColorTheme::load(None, Channel::Donate, None),
            ColorTheme::Classic
        );
        assert_eq!(
            ColorTheme::load(None, Channel::PlayFree, PRO),
            ColorTheme::Classic
        );
        assert_eq!(
            ColorTheme::load(None, Channel::PlayPaid, PRO),
            ColorTheme::Gold
        );
        assert_eq!(
            ColorTheme::load(Some("Night"), Channel::Donate, None),
            ColorTheme::Night
        );
        assert_eq!(
            ColorTheme::load(Some("Classic"), Channel::PlayPaid, PRO),
            ColorTheme::Classic
        );
        // A free-app user can't end up on a Pro theme, whatever is stored.
        for theme in ColorTheme::ALL {
            for paid in [PRO, None] {
                assert_eq!(
                    ColorTheme::load(Some(theme.key()), Channel::PlayFree, paid),
                    ColorTheme::Classic
                );
            }
        }
        assert_eq!(
            ColorTheme::load(Some("garbage"), Channel::PlayPaid, PRO),
            ColorTheme::Gold
        );
    }

    #[test]
    fn this_build() {
        let expected = if cfg!(feature = "play-paid") {
            ColorTheme::Gold
        } else {
            ColorTheme::Classic
        };
        assert_eq!(load(None), expected);
        assert_eq!(access(ColorTheme::Classic), Access::Unlocked);
        if !cfg!(feature = "play-free") {
            assert_eq!(access(ColorTheme::Night), Access::Unlocked);
        }
    }
}
