//! The icons, drawn from the font embedded in the app.
//!
//! `assets/fonts/CalcCraftIcons.ttf` is a subset of Microsoft's Fluent System Icons, built by
//! `tools/icon-font.py`. The tool puts every icon at the code point of the Segoe Fluent Icons
//! glyph that the Windows Calculator uses for it, so the code points here are Windows' own. A
//! new icon needs its line in the tool's `ICONS` and a run of the tool; a test checks that the
//! font has every glyph named in this file.

use eframe::egui::{FontFamily, text::LayoutJob};

use crate::converter::Category;

/// The icon font (MIT license, see `assets/fonts/README.md`).
pub(super) const FONT: &[u8] = include_bytes!("../../assets/fonts/CalcCraftIcons.ttf");

pub(super) const ICON_MENU: char = '\u{E700}';
pub(super) const ICON_CHEVRON_DOWN: char = '\u{E70D}';
pub(super) const ICON_CHEVRON_UP: char = '\u{E70E}';
pub(super) const ICON_CHEVRON_LEFT: char = '\u{E76B}';
pub(super) const ICON_CHEVRON_RIGHT: char = '\u{E76C}';
pub(super) const ICON_SETTINGS: char = '\u{E713}';
pub(super) const ICON_BACK: char = '\u{E72B}';
pub(crate) const ICON_SHARE: char = '\u{E72D}';
pub(super) const ICON_DELETE: char = '\u{E74D}';
pub(super) const ICON_RETURN: char = '\u{E751}';
pub(super) const ICON_PASTE: char = '\u{E77F}';
pub(super) const ICON_THEME: char = '\u{E790}';
pub(super) const ICON_UNDO: char = '\u{E7A7}';
pub(super) const ICON_WARNING: char = '\u{E7BA}';
pub(super) const ICON_HISTORY: char = '\u{E81C}';
pub(super) const ICON_CALENDAR: char = '\u{E787}';
pub(super) const ICON_VIEW: char = '\u{E890}';
pub(super) const ICON_HIDE: char = '\u{ED1A}';
pub(super) const ICON_CUT: char = '\u{E8C6}';
pub(super) const ICON_COPY: char = '\u{E8C8}';
pub(crate) const ICON_REMOVE: char = '\u{ECC9}';
/// Dismiss / close cross icon ('X') from Fluent System Icons ("dismiss").
#[cfg_attr(not(feature = "support-prompt"), allow(dead_code))]
pub(crate) const ICON_CLOSE: char = '\u{E947}';
/// HeartFill, the support prompt's thank-you line for supporters.
#[cfg_attr(not(feature = "support-prompt"), allow(dead_code))]
pub(crate) const ICON_HEART: char = '\u{EB52}';
pub(super) const ICON_CALCULATOR: char = '\u{E8EF}';
/// The Programmer mode in the menu, `</>`.
pub(super) const ICON_CODE: char = '\u{E943}';
/// The Scientific mode in the menu, a flask.
pub(super) const ICON_SCIENTIFIC: char = '\u{F196}';
pub(super) const ICON_LIGHTNING: char = '\u{E945}';
pub(super) const ICON_MULTIPLY: char = '\u{E947}';
pub(super) const ICON_PLUS: char = '\u{E948}';
pub(super) const ICON_MINUS: char = '\u{E949}';
pub(super) const ICON_DIVIDE: char = '\u{E94A}';
pub(super) const ICON_EQUALS: char = '\u{E94E}';
pub(super) const ICON_BACKSPACE: char = '\u{E94F}';
/// The calendar's arrows (WinUI's `CaretUpSolid8` and `CaretDownSolid8`).
pub(super) const ICON_CARET_UP: char = '\u{EDDB}';
pub(super) const ICON_CARET_DOWN: char = '\u{EDDC}';
pub(super) const ICON_KEEP_ON_TOP: char = '\u{EE49}';
pub(super) const ICON_BACK_TO_FULL: char = '\u{EE47}';
pub(super) const ICON_LANGUAGE: char = '\u{F2B7}';
pub(super) const ICON_REGION: char = '\u{E909}';

/// The converters' icons in the navigation menu. Windows draws Volume, Weight, Temperature,
/// Speed, Data and Angle from the Calculator's own icon font, which may only be used inside
/// Windows, so those are similar Fluent icons.
pub(super) fn category_icon(category: Category) -> char {
    match category {
        Category::Volume => '\u{F158}',
        Category::Length => '\u{ECC6}',
        Category::Weight => '\u{E821}',
        Category::Temperature => '\u{E9CA}',
        Category::Energy => '\u{ECAD}',
        Category::Area => '\u{E809}',
        Category::Speed => '\u{E805}',
        Category::Time => '\u{E917}',
        Category::Power => '\u{E945}',
        Category::Data => '\u{EDA2}',
        Category::Pressure => '\u{EC4A}',
        Category::Angle => '\u{F461}',
    }
}

/// The font family of the icon font.
pub(super) fn family() -> FontFamily {
    FontFamily::Name("icons".into())
}

/// A label made of one icon.
pub(crate) fn icon_label(glyph: char, size: f32) -> LayoutJob {
    super::text(&glyph.to_string(), size, family())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use eframe::egui::{FontData, FontDefinitions, FontId, epaint::text::Fonts};

    use super::*;

    /// Every code point written in this file, so a new constant can't miss the font.
    fn code_points() -> Vec<char> {
        let source = include_str!("icons.rs");
        source
            .match_indices("'\\u{")
            .map(|(start, _)| {
                let hex = &source[start + 4..];
                let hex = &hex[..hex.find('}').expect("a closing brace")];
                char::from_u32(u32::from_str_radix(hex, 16).expect("hex digits"))
                    .expect("a valid code point")
            })
            .collect()
    }

    #[test]
    fn the_font_has_every_icon() {
        let mut definitions = FontDefinitions::empty();
        definitions
            .font_data
            .insert("icons".into(), Arc::new(FontData::from_static(FONT)));
        definitions.families.insert(family(), vec!["icons".into()]);
        let mut fonts = Fonts::new(Default::default(), definitions);
        let font = FontId::new(16.0, family());

        let glyphs = code_points();
        assert!(glyphs.len() > 40, "found only {} code points", glyphs.len());
        for glyph in glyphs {
            assert!(
                fonts.has_glyph(&font, glyph),
                "U+{:04X} is missing from the icon font",
                glyph as u32
            );
        }
        for category in Category::ALL {
            assert!(fonts.has_glyph(&font, category_icon(category)));
        }
    }
}
