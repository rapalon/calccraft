//! Dark and light color palettes.
//!
//! Keypad colors follow the Windows Calculator's `App.xaml`: white at a fixed
//! opacity over the window background. The purple tint of the backgrounds comes
//! from the reference screenshots, and the accent shades are the purple
//! Windows accent palette those screenshots were taken with.

use super::color_theme::ColorTheme;
use eframe::egui::{self, Color32};

/// Every color the interface paints with, for one theme.
pub(crate) struct Palette {
    pub background: Color32,
    pub digit: Color32,
    pub digit_hover: Color32,
    pub digit_pressed: Color32,
    pub digit_disabled: Color32,
    pub operator: Color32,
    pub operator_hover: Color32,
    pub operator_pressed: Color32,
    pub operator_disabled: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_pressed: Color32,
    pub accent_disabled: Color32,
    /// The Scientific mode's second functions while `2nd` is on, hovered and pressed
    /// (`EmphasizedCalcButtonStyle`: the system accent color, at 90 % on hover), and their
    /// text then (`SystemControlForegroundBaseHighBrush`).
    pub emphasized_hover: Color32,
    pub emphasized_pressed: Color32,
    pub text_on_emphasized: Color32,
    /// Transparent buttons (memory row, header icons) on hover and press.
    pub subtle_hover: Color32,
    pub subtle_pressed: Color32,
    /// Outline of the keypad buttons.
    pub border: Color32,
    /// History and memory panels and the navigation pane.
    pub flyout: Color32,
    pub flyout_border: Color32,
    /// Darkens the page behind the navigation pane.
    pub backdrop: Color32,
    pub text: Color32,
    pub text_pressed: Color32,
    pub text_secondary: Color32,
    /// WinUI's tertiary text: the docked panel's tab headers while pressed.
    pub text_tertiary: Color32,
    pub text_disabled: Color32,
    pub text_on_accent: Color32,
    /// Cards on the Settings page.
    pub card: Color32,
    pub card_hover: Color32,
    pub card_pressed: Color32,
    pub card_border: Color32,
    pub radio_ring: Color32,
    pub radio_fill: Color32,
    pub radio_dot: Color32,
    pub link: Color32,
    pub link_hover: Color32,
    pub link_pressed: Color32,
    /// Drop-down lists of the date calculation mode.
    pub popup: Color32,
    pub popup_item: Color32,
    pub popup_item_pressed: Color32,
    pub popup_border: Color32,
    pub popup_shadow: Color32,
    /// The calendar of the date pickers.
    pub calendar: Color32,
    pub calendar_header: Color32,
    pub calendar_line: Color32,
    pub calendar_hover: Color32,
    pub calendar_pressed: Color32,
    pub calendar_arrow: Color32,
    pub other_month: Color32,
    /// The years, months and days boxes.
    pub control: Color32,
    pub control_hover: Color32,
    pub control_pressed: Color32,
    pub control_border: Color32,
    /// The unit pickers of the converters dim their text when hovered and pressed
    /// (WinUI's BaseMediumHigh and BaseMedium: text at 80 % and 60 %).
    pub unit_hover: Color32,
    pub unit_pressed: Color32,
    /// The line over the Programmer mode's keys and bits (`DividerStrokeColorDefault`).
    pub divider: Color32,
    /// The Programmer mode's bit toggling keypad: the bit numbers (WinUI's
    /// BaseMediumHigh), a 0 under the pointer and a bit being pressed (BaseMedium and
    /// BaseMediumLow), and bits beyond the word size (BaseLow): text at 80, 60, 40
    /// and 20 %.
    pub bit_label: Color32,
    pub bit_hover: Color32,
    pub bit_pressed: Color32,
    pub bit_disabled: Color32,
    /// The focus visual's outer and inner rings (WinUI's `FocusStrokeColorOuter` and
    /// `FocusStrokeColorInner`), drawn over the page.
    pub focus_outer: Color32,
    pub focus_inner: Color32,
    /// The Graphing mode's button panels over the graph (`GraphControlCommandPanel`):
    /// #303030 without a border in dark, `SolidBackgroundFillColorBase` with
    /// `CardStrokeColorDefaultSolid` in light, and their icons.
    pub graph_panel: Color32,
    pub graph_panel_border: Color32,
    pub graph_panel_text: Color32,
    /// An equation with an error: its box's border and fill (`EquationBoxErrorBorderBrush`
    /// and `EquationBoxErrorBackgroundBrush`), and the error icon
    /// (`SystemFillColorCritical`).
    pub error_border: Color32,
    pub error_fill: Color32,
    pub critical: Color32,
    /// A hidden equation's button and the empty last one's
    /// (`EquationButtonHideLineBackgroundBrush`, white or black at 40 %), and its text.
    pub hidden_equation: Color32,
    pub hidden_equation_text: Color32,
    /// The variable sliders: the rail (`ControlStrongFillColorDefault`), the thumb
    /// (`ControlSolidFillColorDefault`) and its border, and the line under the value boxes.
    pub slider_rail: Color32,
    pub slider_thumb: Color32,
    pub slider_thumb_border: Color32,
    pub value_underline: Color32,
    /// The track of the switch between the graph and the equations (`SwitchToggleBackground`:
    /// black at 25 % in dark, white at 38 % in light).
    pub switch_track: Color32,
    /// The dimmed window behind a dialog (`SmokeFillColorDefault`: black at 30 %).
    pub smoke: Color32,
    /// The `=` key's (every accent key's) metal finish in place of its flat `accent` colors
    /// (Gold).
    pub metal: Option<&'static Metal>,
    /// A panel behind the display and the expression line: fill and border (Retro LCD).
    pub display_panel: Option<(Color32, Color32)>,
    /// The graph's colors, whatever the Graph theme setting says (Night).
    pub graph: Option<&'static GraphPalette>,
    /// Whether egui's own widgets (tooltips, text boxes, scroll bars) take this palette's
    /// colors instead of egui's defaults, which Classic and Gold keep.
    pub restyle: bool,
    /// Night: no white anywhere, so what would be white (hover overlays, labels on an
    /// equation's color) takes the palette's text or ink instead.
    pub no_white: bool,
    /// The keys' outline width in points: 1, or 2 in High Contrast, which also outlines the
    /// accent keys then.
    pub key_border: f32,
    /// Key labels in semibold (High Contrast: they look larger without taking more room).
    pub bold_keys: bool,
}

/// A metal key: a vertical gradient (`stops`, top to bottom, evenly spaced), a soft
/// specular highlight near the top, a thin edge, and the label's color.
pub(crate) struct Metal {
    pub stops: [Color32; 4],
    pub highlight: Color32,
    pub edge: Color32,
    pub text: Color32,
}

/// The colors of the graph itself, which follow the "Graph theme" setting (always light by
/// default) rather than the app theme: `GraphingCalculator.xaml`'s graph theme colors, and
/// the `EquationBrush1`–`14` of that theme for the equations.
pub(crate) struct GraphPalette {
    pub background: Color32,
    pub axes: Color32,
    /// The labeled grid lines; the others are this at half opacity (#E3E3E3 in light).
    pub grid: Color32,
    pub minor_grid: Color32,
    pub equations: [Color32; 14],
}

const fn rgb(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

const GRAPH_LIGHT_BACKGROUND: Color32 = Color32::WHITE;
const GRAPH_DARK_BACKGROUND: Color32 = rgb(0x1F1F1F);

pub(super) const LIGHT_GRAPH: GraphPalette = GraphPalette {
    background: GRAPH_LIGHT_BACKGROUND,
    axes: Color32::BLACK,
    grid: rgb(0xC6C6C6),
    minor_grid: over(GRAPH_LIGHT_BACKGROUND, rgb(0xC6C6C6), 0.5),
    equations: [
        rgb(0x0063B1),
        rgb(0x00B7C3),
        rgb(0x6600CC),
        rgb(0x107C10),
        rgb(0x00CC6A),
        rgb(0x008055),
        rgb(0x58595B),
        rgb(0xE81123),
        rgb(0xE3008C),
        rgb(0xB31564),
        rgb(0xFFB900),
        rgb(0xF7630C),
        rgb(0x8E562E),
        rgb(0x000000),
    ],
};

pub(super) const DARK_GRAPH: GraphPalette = GraphPalette {
    background: GRAPH_DARK_BACKGROUND,
    axes: Color32::WHITE,
    grid: rgb(0x4F4F4F),
    minor_grid: over(GRAPH_DARK_BACKGROUND, rgb(0x4F4F4F), 0.5),
    equations: [
        rgb(0x4D92C8),
        rgb(0x4DCDD5),
        rgb(0xA366E0),
        rgb(0x58A358),
        rgb(0x4DDB97),
        rgb(0x4DA688),
        rgb(0x8A8B8C),
        rgb(0xEF5865),
        rgb(0xEB4DAF),
        rgb(0xCA5B93),
        rgb(0xFFCE4D),
        rgb(0xF99255),
        rgb(0xB0896D),
        rgb(0xFFFFFF),
    ],
};

/// `color` at `opacity` over `base`, as an opaque color.
const fn over(base: Color32, color: Color32, opacity: f32) -> Color32 {
    const fn channel(base: u8, top: u8, opacity: f32) -> u8 {
        (base as f32 + (top as f32 - base as f32) * opacity + 0.5) as u8
    }
    Color32::from_rgb(
        channel(base.r(), color.r(), opacity),
        channel(base.g(), color.g(), opacity),
        channel(base.b(), color.b(), opacity),
    )
}

const fn white(base: Color32, opacity: f32) -> Color32 {
    over(base, Color32::WHITE, opacity)
}

const fn black(base: Color32, opacity: f32) -> Color32 {
    over(base, Color32::BLACK, opacity)
}

// The purple Windows accent palette of the reference screenshots.
const ACCENT_LIGHT_3: Color32 = Color32::from_rgb(0xF0, 0xC0, 0xF4);
const ACCENT_LIGHT_2: Color32 = Color32::from_rgb(0xDB, 0x9E, 0xE5);
const ACCENT_DARK_1: Color32 = Color32::from_rgb(0x8E, 0x3A, 0xA7);
const ACCENT_DARK_2: Color32 = Color32::from_rgb(0x69, 0x27, 0x82);
/// The accent color itself (`SystemAccentColor`), between Light 1 and Dark 1: an
/// estimate, since the reference screenshots don't show it.
const ACCENT_BASE: Color32 = Color32::from_rgb(0xA8, 0x4C, 0xC0);

const DARK_BACKGROUND: Color32 = Color32::from_rgb(0x29, 0x1A, 0x29);
/// Not sampled: Mica in light mode with the same purple wallpaper tint.
const LIGHT_BACKGROUND: Color32 = Color32::from_rgb(0xF3, 0xEB, 0xF3);

pub(crate) const DARK: Palette = Palette {
    background: DARK_BACKGROUND,
    digit: white(DARK_BACKGROUND, 0.125),
    digit_hover: white(DARK_BACKGROUND, 0.0852),
    digit_pressed: white(DARK_BACKGROUND, 0.0359),
    digit_disabled: white(DARK_BACKGROUND, 0.0359),
    operator: white(DARK_BACKGROUND, 0.0852),
    operator_hover: white(DARK_BACKGROUND, 0.1256),
    operator_pressed: white(DARK_BACKGROUND, 0.0852),
    operator_disabled: white(DARK_BACKGROUND, 0.0359),
    accent: ACCENT_LIGHT_2,
    accent_hover: over(DARK_BACKGROUND, ACCENT_LIGHT_2, 0.9),
    accent_pressed: over(DARK_BACKGROUND, ACCENT_LIGHT_2, 0.8),
    accent_disabled: white(DARK_BACKGROUND, 0.1581),
    emphasized_hover: over(DARK_BACKGROUND, ACCENT_BASE, 0.9),
    emphasized_pressed: ACCENT_BASE,
    text_on_emphasized: Color32::WHITE,
    subtle_hover: white(DARK_BACKGROUND, 0.0605),
    subtle_pressed: white(DARK_BACKGROUND, 0.0419),
    border: Color32::from_rgb(0x38, 0x2A, 0x39),
    flyout: Color32::from_rgb(0x2F, 0x21, 0x2F),
    flyout_border: Color32::from_rgb(0x45, 0x37, 0x45),
    backdrop: Color32::from_black_alpha(0x6B),
    text: Color32::WHITE,
    text_pressed: Color32::from_rgb(0xCF, 0xC8, 0xCF),
    text_secondary: Color32::from_rgb(0xC5, 0xBE, 0xC5),
    text_tertiary: white(DARK_BACKGROUND, 0.53),
    text_disabled: Color32::from_rgb(0x6C, 0x62, 0x6D),
    text_on_accent: Color32::from_rgb(0x1A, 0x0F, 0x1B),
    card: Color32::from_rgb(0x34, 0x26, 0x35),
    card_hover: white(DARK_BACKGROUND, 0.0852),
    card_pressed: white(DARK_BACKGROUND, 0.0419),
    card_border: Color32::from_rgb(0x22, 0x18, 0x25),
    radio_ring: Color32::from_rgb(0x9D, 0x97, 0x9F),
    radio_fill: Color32::from_rgb(0x2C, 0x23, 0x33),
    radio_dot: Color32::BLACK,
    link: ACCENT_LIGHT_3,
    link_hover: over(DARK_BACKGROUND, ACCENT_LIGHT_3, 0.8),
    link_pressed: over(DARK_BACKGROUND, ACCENT_LIGHT_3, 0.6),
    // Sampled from screenshots of the Windows 11 date calculator.
    popup: Color32::from_rgb(0x2F, 0x2A, 0x34),
    popup_item: Color32::from_rgb(0x3C, 0x37, 0x40),
    popup_item_pressed: Color32::from_rgb(0x36, 0x31, 0x3A),
    popup_border: Color32::from_rgb(0x48, 0x3F, 0x4C),
    popup_shadow: Color32::from_black_alpha(100),
    calendar: Color32::from_rgb(0x3B, 0x31, 0x3F),
    calendar_header: Color32::from_rgb(0x32, 0x2A, 0x36),
    calendar_line: Color32::from_rgb(0x2C, 0x25, 0x30),
    calendar_hover: Color32::from_rgb(0x47, 0x3D, 0x4B),
    calendar_pressed: Color32::from_rgb(0x41, 0x37, 0x45),
    calendar_arrow: Color32::from_rgb(0xA1, 0x9D, 0xA2),
    other_month: Color32::from_rgb(0xA8, 0xA1, 0xA9),
    // WinUI control fills: white at 5.9 %, 8.2 % and 3.1 %.
    control: white(DARK_BACKGROUND, 0.059),
    control_hover: white(DARK_BACKGROUND, 0.082),
    control_pressed: white(DARK_BACKGROUND, 0.031),
    control_border: Color32::from_rgb(0x44, 0x36, 0x44),
    unit_hover: white(DARK_BACKGROUND, 0.8),
    unit_pressed: white(DARK_BACKGROUND, 0.6),
    divider: white(DARK_BACKGROUND, 0.0837),
    bit_label: white(DARK_BACKGROUND, 0.8),
    bit_hover: white(DARK_BACKGROUND, 0.6),
    bit_pressed: white(DARK_BACKGROUND, 0.4),
    bit_disabled: white(DARK_BACKGROUND, 0.2),
    focus_outer: Color32::WHITE,
    focus_inner: Color32::from_black_alpha(0xB3),
    graph_panel: rgb(0x303030),
    graph_panel_border: rgb(0x303030),
    graph_panel_text: Color32::WHITE,
    error_border: rgb(0xEB5757),
    error_fill: over(white(DARK_BACKGROUND, 0.059), rgb(0xEB5757), 0.2),
    critical: rgb(0xFF99A4),
    hidden_equation: white(DARK_BACKGROUND, 0.4),
    hidden_equation_text: white(white(DARK_BACKGROUND, 0.4), 0.6),
    slider_rail: white(DARK_BACKGROUND, 0.545),
    slider_thumb: rgb(0x454545),
    slider_thumb_border: white(DARK_BACKGROUND, 0.094),
    value_underline: white(DARK_BACKGROUND, 0.314),
    switch_track: black(DARK_BACKGROUND, 0.25),
    smoke: Color32::from_black_alpha(0x4D),
    metal: None,
    display_panel: None,
    graph: None,
    restyle: false,
    no_white: false,
    key_border: 1.0,
    bold_keys: false,
};

pub(crate) const LIGHT: Palette = Palette {
    background: LIGHT_BACKGROUND,
    digit: Color32::WHITE,
    digit_hover: white(LIGHT_BACKGROUND, 0.75),
    digit_pressed: white(LIGHT_BACKGROUND, 0.5),
    digit_disabled: white(LIGHT_BACKGROUND, 0.125),
    operator: white(LIGHT_BACKGROUND, 0.5),
    operator_hover: white(LIGHT_BACKGROUND, 0.25),
    operator_pressed: white(LIGHT_BACKGROUND, 0.125),
    operator_disabled: white(LIGHT_BACKGROUND, 0.125),
    accent: ACCENT_DARK_1,
    accent_hover: over(LIGHT_BACKGROUND, ACCENT_DARK_1, 0.9),
    accent_pressed: over(LIGHT_BACKGROUND, ACCENT_DARK_1, 0.8),
    accent_disabled: black(LIGHT_BACKGROUND, 0.2169),
    emphasized_hover: over(LIGHT_BACKGROUND, ACCENT_BASE, 0.9),
    emphasized_pressed: ACCENT_BASE,
    text_on_emphasized: Color32::BLACK,
    subtle_hover: black(LIGHT_BACKGROUND, 0.0373),
    subtle_pressed: black(LIGHT_BACKGROUND, 0.0241),
    border: black(LIGHT_BACKGROUND, 0.0578),
    flyout: white(LIGHT_BACKGROUND, 0.6),
    flyout_border: black(LIGHT_BACKGROUND, 0.0578),
    backdrop: Color32::from_black_alpha(0x4D),
    text: Color32::BLACK,
    text_pressed: black(LIGHT_BACKGROUND, 0.6063),
    text_secondary: black(LIGHT_BACKGROUND, 0.6063),
    text_tertiary: black(LIGHT_BACKGROUND, 0.4471),
    text_disabled: black(LIGHT_BACKGROUND, 0.3614),
    text_on_accent: Color32::WHITE,
    card: white(LIGHT_BACKGROUND, 0.7),
    card_hover: white(LIGHT_BACKGROUND, 0.5),
    card_pressed: black(LIGHT_BACKGROUND, 0.0241),
    card_border: black(LIGHT_BACKGROUND, 0.0578),
    radio_ring: black(LIGHT_BACKGROUND, 0.4458),
    radio_fill: black(LIGHT_BACKGROUND, 0.0241),
    radio_dot: Color32::WHITE,
    link: ACCENT_DARK_2,
    link_hover: over(LIGHT_BACKGROUND, ACCENT_DARK_2, 0.8),
    link_pressed: over(LIGHT_BACKGROUND, ACCENT_DARK_2, 0.6),
    // Not sampled: WinUI light flyouts, list items and controls over the light background.
    popup: white(LIGHT_BACKGROUND, 0.6),
    popup_item: black(LIGHT_BACKGROUND, 0.0373),
    popup_item_pressed: black(LIGHT_BACKGROUND, 0.0241),
    popup_border: black(LIGHT_BACKGROUND, 0.0578),
    popup_shadow: Color32::from_black_alpha(40),
    calendar: white(LIGHT_BACKGROUND, 0.7),
    calendar_header: white(LIGHT_BACKGROUND, 0.3),
    calendar_line: black(LIGHT_BACKGROUND, 0.0578),
    calendar_hover: black(LIGHT_BACKGROUND, 0.0373),
    calendar_pressed: black(LIGHT_BACKGROUND, 0.0241),
    calendar_arrow: black(LIGHT_BACKGROUND, 0.4458),
    other_month: black(LIGHT_BACKGROUND, 0.4458),
    // WinUI control fills: white at 70 %, 50 % and 30 %.
    control: white(LIGHT_BACKGROUND, 0.7),
    control_hover: white(LIGHT_BACKGROUND, 0.5),
    control_pressed: white(LIGHT_BACKGROUND, 0.3),
    control_border: black(LIGHT_BACKGROUND, 0.0578),
    unit_hover: black(LIGHT_BACKGROUND, 0.8),
    unit_pressed: black(LIGHT_BACKGROUND, 0.6),
    divider: black(LIGHT_BACKGROUND, 0.0803),
    bit_label: black(LIGHT_BACKGROUND, 0.8),
    bit_hover: black(LIGHT_BACKGROUND, 0.6),
    bit_pressed: black(LIGHT_BACKGROUND, 0.4),
    bit_disabled: black(LIGHT_BACKGROUND, 0.2),
    focus_outer: Color32::from_black_alpha(0xE4),
    // White at 70 %, premultiplied.
    focus_inner: Color32::from_rgba_premultiplied(0xB3, 0xB3, 0xB3, 0xB3),
    graph_panel: rgb(0xF3F3F3),
    graph_panel_border: rgb(0xEBEBEB),
    graph_panel_text: Color32::BLACK,
    error_border: rgb(0xEB5757),
    error_fill: over(white(LIGHT_BACKGROUND, 0.7), rgb(0xEB5757), 0.2),
    critical: rgb(0xC42B1C),
    hidden_equation: black(LIGHT_BACKGROUND, 0.4),
    hidden_equation_text: Color32::WHITE,
    slider_rail: black(LIGHT_BACKGROUND, 0.447),
    slider_thumb: Color32::WHITE,
    slider_thumb_border: black(LIGHT_BACKGROUND, 0.0578),
    value_underline: black(LIGHT_BACKGROUND, 0.2169),
    switch_track: white(LIGHT_BACKGROUND, 0.376),
    smoke: Color32::from_black_alpha(0x4D),
    metal: None,
    display_panel: None,
    graph: None,
    restyle: false,
    no_white: false,
    key_border: 1.0,
    bold_keys: false,
};

// ------------------------------------------------------------ colour themes

/// A colour theme (docs/color-themes.md): its palettes for dark and light, and the egui theme
/// it fixes, if any (then App theme's choice waits until another theme is picked).
pub(super) struct ThemeSpec {
    pub theme: ColorTheme,
    pub dark: &'static Palette,
    pub light: &'static Palette,
    pub fixed: Option<egui::Theme>,
}

impl ThemeSpec {
    /// Its palette for the theme egui resolved.
    pub fn palette(&self, theme: egui::Theme) -> &'static Palette {
        match self.fixed.unwrap_or(theme) {
            egui::Theme::Dark => self.dark,
            egui::Theme::Light => self.light,
        }
    }
}

/// Every colour theme, in `ColorTheme::ALL`'s order.
pub(super) static THEMES: [ThemeSpec; 7] = [
    ThemeSpec {
        theme: ColorTheme::Classic,
        dark: &DARK,
        light: &LIGHT,
        fixed: None,
    },
    ThemeSpec {
        theme: ColorTheme::Gold,
        dark: &GOLD_DARK,
        light: &GOLD_LIGHT,
        fixed: None,
    },
    ThemeSpec {
        theme: ColorTheme::Night,
        dark: &NIGHT,
        light: &NIGHT,
        fixed: Some(egui::Theme::Dark),
    },
    ThemeSpec {
        theme: ColorTheme::RetroLcd,
        dark: &RETRO_LCD,
        light: &RETRO_LCD,
        fixed: Some(egui::Theme::Light),
    },
    ThemeSpec {
        theme: ColorTheme::Paper,
        dark: &PAPER,
        light: &PAPER,
        fixed: Some(egui::Theme::Light),
    },
    ThemeSpec {
        theme: ColorTheme::AmoledBlack,
        dark: &AMOLED_BLACK,
        light: &AMOLED_BLACK,
        fixed: Some(egui::Theme::Dark),
    },
    ThemeSpec {
        theme: ColorTheme::HighContrast,
        dark: &HIGH_CONTRAST,
        light: &HIGH_CONTRAST,
        fixed: Some(egui::Theme::Dark),
    },
];

/// The spec of `theme`.
pub(super) fn spec(theme: ColorTheme) -> &'static ThemeSpec {
    THEMES
        .iter()
        .find(|spec| spec.theme == theme)
        .unwrap_or(&THEMES[0])
}

/// Gold's `=`: the gold key of the Pro icon (android/icon-source/pro/calccraft-pro-icon-512.png),
/// sampled from it: the key's mean colour from its upper to its lower bands (#E6C980 to
/// #B99148), its sheen (#FFF3B8, the lightest pixel) as the highlight and its darkest pixel
/// (#795421) as the rim. The label is a dark brown that keeps 4.5:1 on every stop (5.8:1 on the
/// darkest).
pub(crate) const GOLD: Metal = Metal {
    stops: [rgb(0xE8CC84), rgb(0xDAB96E), rgb(0xC8A257), rgb(0xB99148)],
    // #FFF3B8 at 60 %, premultiplied.
    highlight: Color32::from_rgba_premultiplied(0x99, 0x92, 0x6E, 0x99),
    edge: rgb(0x795421),
    text: rgb(0x2A1A00),
};

pub(crate) const GOLD_DARK: Palette = Palette {
    metal: Some(&GOLD),
    ..DARK
};

pub(crate) const GOLD_LIGHT: Palette = Palette {
    metal: Some(&GOLD),
    ..LIGHT
};

const NIGHT_BACKGROUND: Color32 = rgb(0x0A0505);
/// Night's label red: 5:1 on its keys, and dim enough for a dark room.
const NIGHT_RED: Color32 = rgb(0xE8503F);
const NIGHT_RED_2: Color32 = rgb(0xDC4A3C);
const NIGHT_RED_3: Color32 = rgb(0xC24034);
const NIGHT_RED_4: Color32 = rgb(0xA83A2E);
const NIGHT_INK: Color32 = rgb(0x0A0303);

/// Night's graph: near-black, dark red lines, and equations from red to amber, never blue.
pub(crate) const NIGHT_GRAPH: GraphPalette = GraphPalette {
    background: NIGHT_BACKGROUND,
    axes: NIGHT_RED_3,
    grid: rgb(0x3A1712),
    minor_grid: over(NIGHT_BACKGROUND, rgb(0x3A1712), 0.5),
    equations: [
        NIGHT_RED,
        rgb(0xFF9A3C),
        rgb(0xD4A017),
        rgb(0xA0522D),
        rgb(0xFF5E8A),
        rgb(0xC2187A),
        rgb(0x8B2A1A),
        rgb(0xC89A5A),
        rgb(0xFF2A1A),
        rgb(0x7A5A18),
        rgb(0xF59A8A),
        rgb(0x8A2050),
        rgb(0xD97A2A),
        rgb(0xB8B040),
    ],
};

/// Night: dim red on near-black, for use in the dark. No white and no blue anywhere.
pub(crate) const NIGHT: Palette = Palette {
    background: NIGHT_BACKGROUND,
    digit: rgb(0x1C0D0B),
    digit_hover: rgb(0x241210),
    digit_pressed: rgb(0x140807),
    digit_disabled: rgb(0x120807),
    operator: rgb(0x150908),
    operator_hover: rgb(0x1C0D0B),
    operator_pressed: rgb(0x100606),
    operator_disabled: rgb(0x120807),
    accent: NIGHT_RED,
    accent_hover: NIGHT_RED_2,
    accent_pressed: NIGHT_RED_3,
    accent_disabled: rgb(0x2A1210),
    emphasized_hover: NIGHT_RED_3,
    emphasized_pressed: NIGHT_RED,
    text_on_emphasized: NIGHT_INK,
    subtle_hover: rgb(0x1A0C0A),
    subtle_pressed: rgb(0x140907),
    border: rgb(0x2A1411),
    flyout: rgb(0x120807),
    flyout_border: rgb(0x3A1712),
    backdrop: Color32::from_black_alpha(0x8C),
    text: NIGHT_RED,
    text_pressed: NIGHT_RED_3,
    text_secondary: NIGHT_RED_2,
    text_tertiary: NIGHT_RED_4,
    text_disabled: rgb(0x6A2A22),
    text_on_accent: NIGHT_INK,
    card: rgb(0x140908),
    card_hover: rgb(0x1C0D0B),
    card_pressed: rgb(0x100606),
    card_border: rgb(0x2A1411),
    radio_ring: NIGHT_RED_4,
    radio_fill: rgb(0x140908),
    radio_dot: NIGHT_INK,
    link: rgb(0xF0603F),
    link_hover: NIGHT_RED_2,
    link_pressed: NIGHT_RED_3,
    popup: rgb(0x140908),
    popup_item: rgb(0x241210),
    popup_item_pressed: rgb(0x1C0D0B),
    popup_border: rgb(0x3A1712),
    popup_shadow: Color32::from_black_alpha(120),
    calendar: rgb(0x140908),
    calendar_header: rgb(0x100606),
    calendar_line: rgb(0x241210),
    calendar_hover: rgb(0x2A1411),
    calendar_pressed: rgb(0x241210),
    calendar_arrow: NIGHT_RED_4,
    other_month: NIGHT_RED_4,
    control: rgb(0x140908),
    control_hover: rgb(0x1C0D0B),
    control_pressed: rgb(0x100606),
    control_border: rgb(0x3A1712),
    unit_hover: NIGHT_RED_3,
    unit_pressed: NIGHT_RED_4,
    divider: rgb(0x2A1411),
    bit_label: NIGHT_RED_2,
    bit_hover: NIGHT_RED_3,
    bit_pressed: NIGHT_RED_4,
    bit_disabled: rgb(0x4A1E18),
    focus_outer: NIGHT_RED,
    focus_inner: Color32::from_black_alpha(0xB3),
    graph_panel: rgb(0x140908),
    graph_panel_border: rgb(0x2A1411),
    graph_panel_text: NIGHT_RED,
    error_border: NIGHT_RED,
    error_fill: rgb(0x2A0E0B),
    critical: rgb(0xF0603F),
    hidden_equation: rgb(0x3A1712),
    hidden_equation_text: NIGHT_RED_3,
    slider_rail: NIGHT_RED_4,
    slider_thumb: rgb(0x3A1712),
    slider_thumb_border: rgb(0x5A2620),
    value_underline: rgb(0x6A2A22),
    switch_track: rgb(0x050202),
    smoke: Color32::from_black_alpha(0x4D),
    metal: None,
    display_panel: None,
    graph: Some(&NIGHT_GRAPH),
    restyle: true,
    no_white: true,
    key_border: 1.0,
    bold_keys: false,
};

const LCD_BODY: Color32 = rgb(0xB4BCA2);
const LCD_INK: Color32 = rgb(0x1C2414);
const LCD_INK_2: Color32 = rgb(0x3A4530);
const LCD_INK_3: Color32 = rgb(0x4E5A42);
const LCD_LINE: Color32 = rgb(0x8E977C);
/// The LCD glass behind the display.
const LCD_GLASS: Color32 = rgb(0xC4CCAE);

/// Retro LCD: a greenish-grey LCD display with dark digits and muted grey-green keys.
pub(crate) const RETRO_LCD: Palette = Palette {
    background: LCD_BODY,
    digit: rgb(0xA9B196),
    digit_hover: rgb(0xB2BAA0),
    digit_pressed: rgb(0x9DA58A),
    digit_disabled: rgb(0xAEB69C),
    operator: rgb(0x9DA58A),
    operator_hover: rgb(0xA6AE93),
    operator_pressed: rgb(0x939B80),
    operator_disabled: rgb(0xAEB69C),
    accent: rgb(0x3E4A30),
    accent_hover: rgb(0x4A5839),
    accent_pressed: rgb(0x334027),
    accent_disabled: rgb(0x9DA58A),
    emphasized_hover: rgb(0x4A5839),
    emphasized_pressed: rgb(0x3E4A30),
    text_on_emphasized: rgb(0xDDE3CC),
    subtle_hover: rgb(0xA9B196),
    subtle_pressed: rgb(0x9DA58A),
    border: LCD_LINE,
    flyout: LCD_GLASS,
    flyout_border: LCD_LINE,
    backdrop: Color32::from_black_alpha(0x4D),
    text: LCD_INK,
    text_pressed: LCD_INK_2,
    text_secondary: LCD_INK_2,
    text_tertiary: LCD_INK_3,
    text_disabled: rgb(0x7C866C),
    text_on_accent: rgb(0xDDE3CC),
    card: rgb(0xBEC6A8),
    card_hover: LCD_GLASS,
    card_pressed: LCD_BODY,
    card_border: LCD_LINE,
    radio_ring: LCD_INK_2,
    radio_fill: LCD_BODY,
    radio_dot: rgb(0xDDE3CC),
    link: rgb(0x2E4A1E),
    link_hover: rgb(0x3A5A28),
    link_pressed: rgb(0x4E6A3A),
    popup: LCD_GLASS,
    popup_item: LCD_BODY,
    popup_item_pressed: rgb(0xA9B196),
    popup_border: LCD_LINE,
    popup_shadow: Color32::from_black_alpha(40),
    calendar: LCD_GLASS,
    calendar_header: rgb(0xBEC6A8),
    calendar_line: rgb(0xA9B196),
    calendar_hover: LCD_BODY,
    calendar_pressed: rgb(0xA9B196),
    calendar_arrow: LCD_INK_2,
    other_month: LCD_INK_3,
    control: rgb(0xBEC6A8),
    control_hover: LCD_GLASS,
    control_pressed: LCD_BODY,
    control_border: LCD_LINE,
    unit_hover: LCD_INK_2,
    unit_pressed: LCD_INK_3,
    divider: LCD_LINE,
    bit_label: rgb(0x2A3420),
    bit_hover: LCD_INK_2,
    bit_pressed: LCD_INK_3,
    bit_disabled: LCD_LINE,
    focus_outer: LCD_INK,
    focus_inner: Color32::from_rgba_premultiplied(0xB3, 0xB3, 0xB3, 0xB3),
    graph_panel: LCD_GLASS,
    graph_panel_border: LCD_LINE,
    graph_panel_text: LCD_INK,
    error_border: rgb(0xA8322A),
    error_fill: rgb(0xD0C0A8),
    critical: rgb(0xA8322A),
    hidden_equation: LCD_LINE,
    hidden_equation_text: LCD_INK,
    slider_rail: LCD_INK_2,
    slider_thumb: rgb(0xDDE3CC),
    slider_thumb_border: LCD_LINE,
    value_underline: LCD_INK_3,
    switch_track: LCD_GLASS,
    smoke: Color32::from_black_alpha(0x4D),
    metal: None,
    display_panel: Some((LCD_GLASS, LCD_LINE)),
    graph: None,
    restyle: true,
    no_white: false,
    key_border: 1.0,
    bold_keys: false,
};

const PAPER_BACKGROUND: Color32 = rgb(0xF6F0E1);
const PAPER_SHEET: Color32 = rgb(0xFBF6EA);
const PAPER_INK: Color32 = rgb(0x1F1A14);
const PAPER_INK_2: Color32 = rgb(0x5A5046);
const PAPER_INK_3: Color32 = rgb(0x7A6E60);
const PAPER_LINE: Color32 = rgb(0xE2D9C4);

/// Paper: warm cream and ink, with almost no accent: the `=` is ink.
pub(crate) const PAPER: Palette = Palette {
    background: PAPER_BACKGROUND,
    digit: rgb(0xFFFBF2),
    digit_hover: PAPER_SHEET,
    digit_pressed: rgb(0xF1EADA),
    digit_disabled: rgb(0xF3EDDF),
    operator: rgb(0xEFE7D4),
    operator_hover: rgb(0xF3ECDC),
    operator_pressed: rgb(0xE8DFCA),
    operator_disabled: rgb(0xF3EDDF),
    accent: rgb(0x2B2620),
    accent_hover: rgb(0x3A332B),
    accent_pressed: rgb(0x4A4238),
    accent_disabled: rgb(0xDDD4C0),
    emphasized_hover: rgb(0x3A332B),
    emphasized_pressed: rgb(0x2B2620),
    text_on_emphasized: PAPER_BACKGROUND,
    subtle_hover: rgb(0xEDE5D2),
    subtle_pressed: rgb(0xE6DDC8),
    border: PAPER_LINE,
    flyout: PAPER_SHEET,
    flyout_border: rgb(0xDDD3BD),
    backdrop: Color32::from_black_alpha(0x4D),
    text: PAPER_INK,
    text_pressed: PAPER_INK_2,
    text_secondary: PAPER_INK_2,
    text_tertiary: PAPER_INK_3,
    text_disabled: rgb(0xA89C8A),
    text_on_accent: PAPER_BACKGROUND,
    card: PAPER_SHEET,
    card_hover: rgb(0xFFFBF2),
    card_pressed: rgb(0xF1EADA),
    card_border: PAPER_LINE,
    radio_ring: PAPER_INK_2,
    radio_fill: rgb(0xF1EADA),
    radio_dot: rgb(0xFFFBF2),
    link: rgb(0x7A3B12),
    link_hover: rgb(0x8E4C20),
    link_pressed: rgb(0xA0602E),
    popup: PAPER_SHEET,
    popup_item: rgb(0xEFE7D4),
    popup_item_pressed: rgb(0xE8DFCA),
    popup_border: rgb(0xDDD3BD),
    popup_shadow: Color32::from_black_alpha(40),
    calendar: PAPER_SHEET,
    calendar_header: rgb(0xF3ECDC),
    calendar_line: PAPER_LINE,
    calendar_hover: rgb(0xEFE7D4),
    calendar_pressed: rgb(0xE8DFCA),
    calendar_arrow: PAPER_INK_2,
    other_month: PAPER_INK_3,
    control: PAPER_SHEET,
    control_hover: rgb(0xFFFBF2),
    control_pressed: rgb(0xF1EADA),
    control_border: rgb(0xDDD3BD),
    unit_hover: rgb(0x3A332B),
    unit_pressed: PAPER_INK_2,
    divider: PAPER_LINE,
    bit_label: rgb(0x3A332B),
    bit_hover: PAPER_INK_2,
    bit_pressed: PAPER_INK_3,
    bit_disabled: rgb(0xC8BEAA),
    focus_outer: PAPER_INK,
    focus_inner: Color32::from_rgba_premultiplied(0xB3, 0xB3, 0xB3, 0xB3),
    graph_panel: PAPER_SHEET,
    graph_panel_border: PAPER_LINE,
    graph_panel_text: PAPER_INK,
    error_border: rgb(0xB03A2E),
    error_fill: rgb(0xF2D9CF),
    critical: rgb(0xB03A2E),
    hidden_equation: rgb(0xA89C8A),
    hidden_equation_text: rgb(0xFFFBF2),
    slider_rail: PAPER_INK_2,
    slider_thumb: rgb(0xFFFBF2),
    slider_thumb_border: rgb(0xDDD3BD),
    value_underline: rgb(0xA89C8A),
    switch_track: rgb(0xFFFBF2),
    smoke: Color32::from_black_alpha(0x4D),
    metal: None,
    display_panel: None,
    graph: None,
    restyle: true,
    no_white: false,
    key_border: 1.0,
    bold_keys: false,
};

const OLED: Color32 = Color32::BLACK;
const AMOLED_TEXT: Color32 = rgb(0xF2F2F2);
/// AMOLED Black's one light accent: a soft blue, with a black label.
const AMOLED_ACCENT: Color32 = rgb(0x8AB4F8);
/// The thin outlines that tell the black keys and panels apart.
const AMOLED_LINE: Color32 = rgb(0x262626);
const AMOLED_LINE_2: Color32 = rgb(0x333333);

/// AMOLED Black's graph: black, with the dark graph's equation colours.
pub(crate) const AMOLED_GRAPH: GraphPalette = GraphPalette {
    background: OLED,
    axes: rgb(0xBFBFBF),
    grid: AMOLED_LINE_2,
    minor_grid: rgb(0x1A1A1A),
    equations: DARK_GRAPH.equations,
};

/// AMOLED Black: pure black behind everything (an OLED screen leaves those pixels off), near-white
/// text, keys told apart by thin dim outlines rather than grey fills, and one light accent.
/// Hover and pressed states are the only near-black greys, and only while touched.
pub(crate) const AMOLED_BLACK: Palette = Palette {
    background: OLED,
    digit: OLED,
    digit_hover: rgb(0x121212),
    digit_pressed: rgb(0x1E1E1E),
    digit_disabled: OLED,
    operator: OLED,
    operator_hover: rgb(0x121212),
    operator_pressed: rgb(0x1E1E1E),
    operator_disabled: OLED,
    accent: AMOLED_ACCENT,
    accent_hover: rgb(0xA8C7FA),
    accent_pressed: rgb(0x6E9EF0),
    accent_disabled: rgb(0x1E1E1E),
    emphasized_hover: rgb(0xA8C7FA),
    emphasized_pressed: AMOLED_ACCENT,
    text_on_emphasized: OLED,
    subtle_hover: rgb(0x121212),
    subtle_pressed: rgb(0x1E1E1E),
    border: AMOLED_LINE,
    flyout: OLED,
    flyout_border: AMOLED_LINE_2,
    backdrop: Color32::from_black_alpha(0xB3),
    text: AMOLED_TEXT,
    text_pressed: rgb(0xBFBFBF),
    text_secondary: rgb(0xB3B3B3),
    text_tertiary: rgb(0x8C8C8C),
    text_disabled: rgb(0x5C5C5C),
    text_on_accent: OLED,
    card: OLED,
    card_hover: rgb(0x121212),
    card_pressed: rgb(0x1A1A1A),
    card_border: AMOLED_LINE,
    radio_ring: rgb(0x8C8C8C),
    radio_fill: OLED,
    radio_dot: OLED,
    link: AMOLED_ACCENT,
    link_hover: rgb(0xA8C7FA),
    link_pressed: rgb(0x6E9EF0),
    popup: OLED,
    popup_item: rgb(0x1A1A1A),
    popup_item_pressed: rgb(0x121212),
    popup_border: AMOLED_LINE_2,
    popup_shadow: Color32::TRANSPARENT,
    calendar: OLED,
    calendar_header: OLED,
    calendar_line: AMOLED_LINE,
    calendar_hover: rgb(0x1A1A1A),
    calendar_pressed: rgb(0x121212),
    calendar_arrow: rgb(0xB3B3B3),
    other_month: rgb(0x8C8C8C),
    control: OLED,
    control_hover: rgb(0x121212),
    control_pressed: rgb(0x1A1A1A),
    control_border: AMOLED_LINE_2,
    unit_hover: rgb(0xCCCCCC),
    unit_pressed: rgb(0x999999),
    divider: AMOLED_LINE,
    bit_label: rgb(0xCCCCCC),
    bit_hover: rgb(0x999999),
    bit_pressed: rgb(0x737373),
    bit_disabled: rgb(0x404040),
    focus_outer: AMOLED_TEXT,
    focus_inner: OLED,
    graph_panel: OLED,
    graph_panel_border: AMOLED_LINE_2,
    graph_panel_text: AMOLED_TEXT,
    error_border: rgb(0xEB5757),
    error_fill: rgb(0x1A0606),
    critical: rgb(0xFF99A4),
    hidden_equation: AMOLED_LINE_2,
    hidden_equation_text: AMOLED_TEXT,
    slider_rail: rgb(0x8C8C8C),
    slider_thumb: AMOLED_ACCENT,
    slider_thumb_border: OLED,
    value_underline: rgb(0x595959),
    switch_track: OLED,
    smoke: Color32::from_black_alpha(0x80),
    metal: None,
    display_panel: None,
    graph: Some(&AMOLED_GRAPH),
    restyle: true,
    no_white: false,
    key_border: 1.0,
    bold_keys: false,
};

const HC_BLACK: Color32 = Color32::BLACK;
const HC_WHITE: Color32 = Color32::WHITE;
/// Windows' high-contrast yellow: the accent, links, hover text and pressed keys.
const HC_YELLOW: Color32 = rgb(0xFFFF00);
/// The keyboard focus, apart from hover and pressed (yellow).
const HC_CYAN: Color32 = rgb(0x00FFFF);
/// Disabled text: Windows' Night sky theme's GrayText, 8.4:1 on black.
const HC_GRAY: Color32 = rgb(0xA6A6A6);
/// The only fills besides black and yellow: hovered items, under white text (13:1).
const HC_HOVER: Color32 = rgb(0x262626);

/// High Contrast's graph: black, white axes, and 14 bright equation colours.
pub(crate) const HIGH_CONTRAST_GRAPH: GraphPalette = GraphPalette {
    background: HC_BLACK,
    axes: HC_WHITE,
    grid: rgb(0x8C8C8C),
    minor_grid: rgb(0x404040),
    equations: [
        HC_YELLOW,
        HC_CYAN,
        rgb(0xFF66FF),
        rgb(0x33FF33),
        rgb(0xFF9900),
        HC_WHITE,
        rgb(0x66B3FF),
        rgb(0xFF6666),
        rgb(0xB399FF),
        rgb(0x99FFCC),
        rgb(0xCCCC00),
        rgb(0xFFB3D9),
        rgb(0x00CC99),
        rgb(0xFFCC99),
    ],
};

/// High Contrast: for low vision, after Windows' high-contrast themes. White on black, every
/// text at least 7:1 (WCAG AAA), yellow for the accent keys and links and for a pressed key, a
/// cyan focus ring, 2 px white key outlines and semibold labels.
pub(crate) const HIGH_CONTRAST: Palette = Palette {
    background: HC_BLACK,
    digit: HC_BLACK,
    digit_hover: HC_HOVER,
    digit_pressed: HC_YELLOW,
    digit_disabled: HC_BLACK,
    operator: HC_BLACK,
    operator_hover: HC_HOVER,
    operator_pressed: HC_YELLOW,
    operator_disabled: HC_BLACK,
    accent: HC_YELLOW,
    accent_hover: rgb(0xFFFF80),
    accent_pressed: HC_WHITE,
    accent_disabled: HC_BLACK,
    emphasized_hover: HC_YELLOW,
    emphasized_pressed: HC_WHITE,
    text_on_emphasized: HC_BLACK,
    subtle_hover: HC_HOVER,
    subtle_pressed: HC_YELLOW,
    border: HC_WHITE,
    flyout: HC_BLACK,
    flyout_border: HC_WHITE,
    backdrop: Color32::from_black_alpha(0xCC),
    text: HC_WHITE,
    // On the yellow of a pressed key.
    text_pressed: HC_BLACK,
    text_secondary: rgb(0xE6E6E6),
    text_tertiary: rgb(0xCCCCCC),
    text_disabled: HC_GRAY,
    text_on_accent: HC_BLACK,
    card: HC_BLACK,
    card_hover: HC_HOVER,
    card_pressed: rgb(0x333333),
    card_border: HC_WHITE,
    radio_ring: HC_WHITE,
    radio_fill: HC_BLACK,
    radio_dot: HC_BLACK,
    link: HC_YELLOW,
    link_hover: rgb(0xFFFF80),
    link_pressed: HC_WHITE,
    popup: HC_BLACK,
    popup_item: HC_HOVER,
    popup_item_pressed: rgb(0x333333),
    popup_border: HC_WHITE,
    popup_shadow: Color32::TRANSPARENT,
    calendar: HC_BLACK,
    calendar_header: HC_BLACK,
    calendar_line: rgb(0x8C8C8C),
    calendar_hover: HC_HOVER,
    calendar_pressed: rgb(0x333333),
    calendar_arrow: HC_WHITE,
    other_month: HC_GRAY,
    control: HC_BLACK,
    control_hover: HC_HOVER,
    control_pressed: rgb(0x333333),
    control_border: HC_WHITE,
    unit_hover: HC_YELLOW,
    unit_pressed: rgb(0xE6E600),
    divider: HC_WHITE,
    bit_label: HC_WHITE,
    bit_hover: HC_YELLOW,
    bit_pressed: rgb(0xE6E600),
    bit_disabled: HC_GRAY,
    focus_outer: HC_CYAN,
    focus_inner: HC_BLACK,
    graph_panel: HC_BLACK,
    graph_panel_border: HC_WHITE,
    graph_panel_text: HC_WHITE,
    error_border: rgb(0xFF6666),
    error_fill: rgb(0x330000),
    critical: rgb(0xFF8080),
    hidden_equation: rgb(0x333333),
    hidden_equation_text: HC_WHITE,
    slider_rail: HC_WHITE,
    slider_thumb: HC_YELLOW,
    slider_thumb_border: HC_BLACK,
    value_underline: HC_WHITE,
    switch_track: HC_BLACK,
    smoke: Color32::from_black_alpha(0x99),
    metal: None,
    display_panel: None,
    graph: Some(&HIGH_CONTRAST_GRAPH),
    restyle: true,
    no_white: false,
    key_border: 2.0,
    bold_keys: true,
};

/// The color of the gradient `stops` (evenly spaced, top to bottom) at `t` in 0..=1.
pub(crate) fn gradient_at(stops: &[Color32], t: f32) -> Color32 {
    let last = stops.len() - 1;
    let x = t.clamp(0.0, 1.0) * last as f32;
    let i = (x.floor() as usize).min(last.saturating_sub(1));
    let f = x - i as f32;
    let (a, b) = (stops[i], stops[(i + 1).min(last)]);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * f).round() as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// WCAG's relative luminance of an opaque color.
#[cfg(test)]
fn luminance(c: Color32) -> f64 {
    let lin = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG's contrast ratio of two opaque colors, 1 to 21.
#[cfg(test)]
pub(crate) fn contrast(a: Color32, b: Color32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every palette of every theme, with its name.
    fn palettes() -> Vec<(String, &'static Palette)> {
        THEMES
            .iter()
            .flat_map(|spec| {
                let name = spec.theme.key();
                [
                    (format!("{name} dark"), spec.dark),
                    (format!("{name} light"), spec.light),
                ]
            })
            .collect()
    }

    /// Labels keep 4.5:1 (WCAG AA) on everything they are drawn on, in every theme.
    #[test]
    fn labels_have_contrast_in_every_theme() {
        const MIN: f64 = 4.5;
        for (name, p) in palettes() {
            let mut grounds = vec![
                ("background", p.background),
                ("digit", p.digit),
                ("digit_hover", p.digit_hover),
                ("operator", p.operator),
                ("card", p.card),
                ("flyout", p.flyout),
                ("popup", p.popup),
            ];
            if let Some((panel, _)) = p.display_panel {
                grounds.push(("display_panel", panel));
            }
            for (ground, color) in &grounds {
                let ratio = contrast(p.text, *color);
                assert!(ratio >= MIN, "{name}: text on {ground} is {ratio:.2}:1");
            }
            for (ground, color) in [("background", p.background), ("card", p.card)] {
                let ratio = contrast(p.text_secondary, color);
                assert!(
                    ratio >= MIN,
                    "{name}: secondary on {ground} is {ratio:.2}:1"
                );
            }
            match p.metal {
                Some(metal) => {
                    for (i, stop) in metal.stops.iter().enumerate() {
                        let ratio = contrast(metal.text, *stop);
                        assert!(ratio >= MIN, "{name}: = label on stop {i} is {ratio:.2}:1");
                    }
                }
                None => {
                    for (state, color) in [("normal", p.accent), ("hover", p.accent_hover)] {
                        let ratio = contrast(p.text_on_accent, color);
                        assert!(ratio >= MIN, "{name}: = label ({state}) is {ratio:.2}:1");
                    }
                }
            }
        }
    }

    /// Redmean color distance, a cheap stand-in for perceived difference.
    fn distance(a: Color32, b: Color32) -> f64 {
        let r = (a.r() as f64 + b.r() as f64) / 2.0;
        let dr = a.r() as f64 - b.r() as f64;
        let dg = a.g() as f64 - b.g() as f64;
        let db = a.b() as f64 - b.b() as f64;
        ((2.0 + r / 256.0) * dr * dr + 4.0 * dg * dg + (2.0 + (255.0 - r) / 256.0) * db * db).sqrt()
    }

    /// The 14 equation colors of every graph palette stay apart, and visible on the graph.
    #[test]
    fn graph_colors_are_distinguishable() {
        for (name, graph) in [
            ("light", &LIGHT_GRAPH),
            ("dark", &DARK_GRAPH),
            ("night", &NIGHT_GRAPH),
            ("amoled", &AMOLED_GRAPH),
            ("high contrast", &HIGH_CONTRAST_GRAPH),
        ] {
            let colors = graph.equations;
            for (i, a) in colors.iter().enumerate() {
                assert!(
                    contrast(*a, graph.background) >= 1.5,
                    "{name}: equation {i} on the graph"
                );
                for (j, b) in colors.iter().enumerate().skip(i + 1) {
                    let d = distance(*a, *b);
                    assert!(d >= 60.0, "{name}: equations {i} and {j} are {d:.0} apart");
                }
            }
            assert!(
                contrast(graph.axes, graph.background) >= 3.0,
                "{name}: axes"
            );
        }
    }

    /// Night has no blue (no color with more blue than red) and nothing near white.
    #[test]
    fn night_is_red() {
        let p = &NIGHT;
        let mut colors = vec![
            p.background,
            p.digit,
            p.operator,
            p.accent,
            p.accent_hover,
            p.accent_pressed,
            p.emphasized_hover,
            p.emphasized_pressed,
            p.text,
            p.text_pressed,
            p.text_secondary,
            p.text_tertiary,
            p.text_disabled,
            p.text_on_accent,
            p.card,
            p.flyout,
            p.border,
            p.link,
            p.link_hover,
            p.popup,
            p.popup_item,
            p.calendar_arrow,
            p.other_month,
            p.unit_hover,
            p.bit_label,
            p.focus_outer,
            p.graph_panel,
            p.graph_panel_text,
            p.critical,
            p.slider_rail,
            p.slider_thumb,
        ];
        colors.extend(NIGHT_GRAPH.equations);
        colors.extend([NIGHT_GRAPH.axes, NIGHT_GRAPH.grid]);
        for c in colors {
            assert!(c.b() <= c.r(), "{c:?} is blue");
            assert!(luminance(c) < 0.5, "{c:?} is too bright for Night");
        }
        assert_eq!(spec(ColorTheme::Night).fixed, Some(egui::Theme::Dark));
        assert!(NIGHT.graph.is_some());
    }

    /// AMOLED Black lights no pixel where nothing is drawn: every surface, the graph included,
    /// is pure black, and so are egui's own panels and windows (it restyles them).
    #[test]
    fn amoled_black_is_pure_black() {
        let p = &AMOLED_BLACK;
        for (name, color) in [
            ("background", p.background),
            ("digit", p.digit),
            ("digit_disabled", p.digit_disabled),
            ("operator", p.operator),
            ("operator_disabled", p.operator_disabled),
            ("flyout", p.flyout),
            ("card", p.card),
            ("radio_fill", p.radio_fill),
            ("popup", p.popup),
            ("calendar", p.calendar),
            ("calendar_header", p.calendar_header),
            ("control", p.control),
            ("graph_panel", p.graph_panel),
            ("switch_track", p.switch_track),
            ("graph", AMOLED_GRAPH.background),
        ] {
            assert_eq!(color, Color32::BLACK, "{name}");
        }
        assert!(p.restyle && p.display_panel.is_none() && p.metal.is_none());
        assert!(std::ptr::eq(p.graph.unwrap(), &AMOLED_GRAPH));
        assert_eq!(spec(ColorTheme::AmoledBlack).fixed, Some(egui::Theme::Dark));
        // Its text is near-white, its labels and accent label at least 7:1 too.
        assert!(contrast(p.text, Color32::BLACK) >= 7.0);
        assert!(contrast(p.text_on_accent, p.accent) >= 7.0);
    }

    /// High Contrast keeps 7:1 (WCAG AAA) for every text on every surface it is drawn on, in
    /// every key state, and its graph colours 4.5:1 on the graph.
    #[test]
    fn high_contrast_is_aaa() {
        const AAA: f64 = 7.0;
        let p = &HIGH_CONTRAST;
        let check = |what: &str, a: Color32, b: Color32| {
            let ratio = contrast(a, b);
            assert!(ratio >= AAA, "High Contrast: {what} is {ratio:.2}:1");
        };
        let surfaces = [
            ("background", p.background),
            ("digit", p.digit),
            ("digit_hover", p.digit_hover),
            ("operator", p.operator),
            ("operator_hover", p.operator_hover),
            ("subtle_hover", p.subtle_hover),
            ("card", p.card),
            ("card_hover", p.card_hover),
            ("card_pressed", p.card_pressed),
            ("flyout", p.flyout),
            ("popup", p.popup),
            ("popup_item", p.popup_item),
            ("popup_item_pressed", p.popup_item_pressed),
            ("calendar", p.calendar),
            ("calendar_hover", p.calendar_hover),
            ("control", p.control),
            ("control_hover", p.control_hover),
            ("graph_panel", p.graph_panel),
            ("hidden_equation", p.hidden_equation),
        ];
        for (ground, color) in surfaces {
            check(&format!("text on {ground}"), p.text, color);
            check(&format!("secondary on {ground}"), p.text_secondary, color);
            check(&format!("tertiary on {ground}"), p.text_tertiary, color);
        }
        for (ground, color) in [
            ("background", p.background),
            ("digit_disabled", p.digit_disabled),
            ("operator_disabled", p.operator_disabled),
            ("accent_disabled", p.accent_disabled),
        ] {
            check(&format!("disabled on {ground}"), p.text_disabled, color);
        }
        // Every key state: pressed keys are yellow with a black label.
        for (state, fill) in [
            ("digit pressed", p.digit_pressed),
            ("operator pressed", p.operator_pressed),
            ("subtle pressed", p.subtle_pressed),
        ] {
            check(state, p.text_pressed, fill);
        }
        for (state, fill) in [
            ("accent", p.accent),
            ("accent hover", p.accent_hover),
            ("accent pressed", p.accent_pressed),
        ] {
            check(state, p.text_on_accent, fill);
        }
        for (state, fill) in [
            ("emphasized hover", p.emphasized_hover),
            ("emphasized pressed", p.emphasized_pressed),
        ] {
            check(state, p.text_on_emphasized, fill);
        }
        for (what, color) in [
            ("link", p.link),
            ("link hover", p.link_hover),
            ("link pressed", p.link_pressed),
            ("bit label", p.bit_label),
            ("bit hover", p.bit_hover),
            ("bit pressed", p.bit_pressed),
            ("bit disabled", p.bit_disabled),
            ("unit hover", p.unit_hover),
            ("unit pressed", p.unit_pressed),
            ("other month", p.other_month),
            ("critical", p.critical),
            ("graph panel text", p.graph_panel_text),
            ("focus ring", p.focus_outer),
            ("key border", p.border),
        ] {
            check(what, color, p.background);
        }
        check(
            "hidden equation text",
            p.hidden_equation_text,
            p.hidden_equation,
        );
        // Focus, hover and pressed never look alike.
        assert_ne!(p.focus_outer, p.digit_pressed);
        assert_ne!(p.focus_outer, p.accent);
        assert_ne!(p.digit_hover, p.digit_pressed);
        let graph = &HIGH_CONTRAST_GRAPH;
        for (i, color) in graph.equations.iter().enumerate() {
            let ratio = contrast(*color, graph.background);
            assert!(ratio >= 4.5, "High Contrast: equation {i} is {ratio:.2}:1");
        }
        assert!(contrast(graph.axes, graph.background) >= AAA);
        assert!(p.key_border >= 2.0 && p.bold_keys);
        assert_eq!(
            spec(ColorTheme::HighContrast).fixed,
            Some(egui::Theme::Dark)
        );
    }

    /// Only High Contrast changes the keys' outlines and label weight: the others keep the
    /// old look.
    #[test]
    fn key_outlines_change_only_in_high_contrast() {
        for spec in &THEMES {
            for p in [spec.dark, spec.light] {
                let high = spec.theme == ColorTheme::HighContrast;
                assert_eq!(p.key_border, if high { 2.0 } else { 1.0 });
                assert_eq!(p.bold_keys, high);
            }
        }
    }

    #[test]
    fn themes_are_in_the_settings_order() {
        for (spec, theme) in THEMES.iter().zip(ColorTheme::ALL) {
            assert_eq!(spec.theme, theme);
            assert!(std::ptr::eq(super::spec(theme), spec));
        }
        assert!(GOLD_DARK.metal.is_some() && GOLD_LIGHT.metal.is_some());
        assert!(DARK.metal.is_none() && !DARK.restyle && !LIGHT.restyle);
        assert_eq!(spec(ColorTheme::Paper).fixed, Some(egui::Theme::Light));
    }

    #[test]
    fn gradients_interpolate() {
        let stops = [Color32::BLACK, Color32::WHITE];
        assert_eq!(gradient_at(&stops, 0.0), Color32::BLACK);
        assert_eq!(gradient_at(&stops, 1.0), Color32::WHITE);
        assert_eq!(gradient_at(&stops, 0.5), Color32::from_rgb(128, 128, 128));
        assert_eq!(gradient_at(&GOLD.stops, 1.0 / 3.0), GOLD.stops[1]);
        assert_eq!(gradient_at(&GOLD.stops, 2.0), GOLD.stops[3]);
    }

    /// The opacities from `App.xaml` reproduce the colors sampled from the
    /// dark reference screenshot (`#433644` and `#3B2D3C`) to within one step.
    #[test]
    fn opacities_match_sampled_dark_buttons() {
        let close = |a: Color32, b: Color32| {
            let [ar, ag, ab, _] = a.to_array();
            let [br, bg, bb, _] = b.to_array();
            ar.abs_diff(br) <= 1 && ag.abs_diff(bg) <= 1 && ab.abs_diff(bb) <= 1
        };
        assert!(close(DARK.digit, Color32::from_rgb(0x43, 0x36, 0x44)));
        assert!(close(DARK.operator, Color32::from_rgb(0x3B, 0x2D, 0x3C)));
        assert!(close(
            DARK.operator_hover,
            Color32::from_rgb(0x43, 0x36, 0x44)
        ));
        assert!(close(
            DARK.subtle_hover,
            Color32::from_rgb(0x35, 0x27, 0x35)
        ));
    }

    #[test]
    fn over_blends_channels() {
        assert_eq!(white(Color32::BLACK, 0.5), Color32::from_rgb(128, 128, 128));
        assert_eq!(black(Color32::WHITE, 0.0), Color32::WHITE);
        assert_eq!(white(Color32::BLACK, 1.0), Color32::WHITE);
    }
}
