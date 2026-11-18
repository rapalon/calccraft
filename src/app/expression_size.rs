//! The Expression size setting (Settings → Expression size, CalcCraft's own): how large the
//! expression line above the result is drawn in the Standard, Scientific and Programmer
//! modes. Windows has one size; Small is it, and the default. The larger sizes make the
//! line's numbers and operators easier to tap on a phone, where a tap edits them
//! (docs/research/editable-expression.md). Pure, so it is tested on every platform.

/// Storage key, which Windows doesn't have (named like `CalcCraftLanguage`).
pub(super) const STORAGE_KEY: &str = "CalcCraftExpressionSize";

/// The setting's choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum ExpressionSize {
    /// Windows' 14 px (16 px while a part of the line is edited).
    #[default]
    Small,
    Medium,
    Large,
    ExtraLarge,
}

/// The size of the line in pixels while a part of it is edited or selected, relative to its
/// size otherwise: 16 / 14, the default's.
pub(super) const EDIT_SCALE: f32 = 16.0 / 14.0;

/// The least height a tap on the line can land in, at the sizes for fingers: Material's 48 dp
/// touch target (egui's points are Android's dp).
pub(super) const TOUCH_HEIGHT: f32 = 48.0;

impl ExpressionSize {
    /// The drop-down's entries, in order.
    pub(super) const ALL: [ExpressionSize; 4] = [
        ExpressionSize::Small,
        ExpressionSize::Medium,
        ExpressionSize::Large,
        ExpressionSize::ExtraLarge,
    ];

    /// The stored value.
    pub(super) fn key(self) -> &'static str {
        match self {
            ExpressionSize::Small => "Small",
            ExpressionSize::Medium => "Medium",
            ExpressionSize::Large => "Large",
            ExpressionSize::ExtraLarge => "ExtraLarge",
        }
    }

    /// The name's string key in `locales/calccraft.txt`.
    pub(super) fn label_key(self) -> &'static str {
        match self {
            ExpressionSize::Small => "CalcCraft.ExpressionSizeSmall",
            ExpressionSize::Medium => "CalcCraft.ExpressionSizeMedium",
            ExpressionSize::Large => "CalcCraft.ExpressionSizeLarge",
            ExpressionSize::ExtraLarge => "CalcCraft.ExpressionSizeExtraLarge",
        }
    }

    /// The stored choice, or Small for a missing or unknown value.
    pub(super) fn parse(stored: Option<&str>) -> ExpressionSize {
        let stored = stored.map(str::trim);
        ExpressionSize::ALL
            .into_iter()
            .find(|size| Some(size.key()) == stored)
            .unwrap_or_default()
    }

    /// The line's font size in pixels, while no part of it is edited.
    pub(super) fn font_size(self) -> f32 {
        match self {
            ExpressionSize::Small => 14.0,
            ExpressionSize::Medium => 18.0,
            ExpressionSize::Large => 24.0,
            ExpressionSize::ExtraLarge => 30.0,
        }
    }

    /// The font size while a part of the line is edited or selected.
    pub(super) fn edit_font_size(self) -> f32 {
        self.font_size() * EDIT_SCALE
    }

    /// The least height of the line's tap area: none at the desktop's sizes (the line's own,
    /// as before), a finger's at Large and Extra large.
    pub(super) fn min_tap_height(self) -> f32 {
        match self {
            ExpressionSize::Small | ExpressionSize::Medium => 0.0,
            ExpressionSize::Large | ExpressionSize::ExtraLarge => TOUCH_HEIGHT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_values() {
        for size in ExpressionSize::ALL {
            assert_eq!(ExpressionSize::parse(Some(size.key())), size);
        }
        // A new install, and anything a later version might store.
        assert_eq!(ExpressionSize::parse(None), ExpressionSize::Small);
        assert_eq!(ExpressionSize::parse(Some("Huge")), ExpressionSize::Small);
        assert_eq!(
            ExpressionSize::parse(Some(" Large ")),
            ExpressionSize::Large
        );
    }

    #[test]
    fn the_default_is_windows_size() {
        let size = ExpressionSize::default();
        assert_eq!(size, ExpressionSize::Small);
        assert_eq!(size.font_size(), 14.0);
        assert_eq!(size.edit_font_size(), 16.0);
        assert_eq!(size.min_tap_height(), 0.0);
    }

    #[test]
    fn sizes_grow() {
        let sizes: Vec<f32> = ExpressionSize::ALL.iter().map(|s| s.font_size()).collect();
        assert!(sizes.windows(2).all(|pair| pair[0] < pair[1]), "{sizes:?}");
        // The sizes for fingers: at least the touch target's height in all.
        assert_eq!(ExpressionSize::Large.min_tap_height(), TOUCH_HEIGHT);
        assert_eq!(ExpressionSize::ExtraLarge.min_tap_height(), TOUCH_HEIGHT);
    }
}
