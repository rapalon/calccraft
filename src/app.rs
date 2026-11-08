//! User interface styled after the Windows 11 calculator, in a dark or light theme.

pub(crate) mod access;
#[cfg(target_os = "android")]
pub(crate) mod android;
mod converter_view;
mod date_view;
mod graph_canvas;
mod graphing_keypad;
mod graphing_view;
pub(crate) mod icons;
pub(crate) mod palette;
mod programmer_view;
mod region;
mod scientific_view;
mod share;
pub(crate) mod strings;
#[cfg(target_arch = "wasm32")]
mod web;

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, Color32, Context, CornerRadius, CursorIcon, Event, FontFamily, FontId,
    Galley, Id, Key, Modifiers, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    TextFormat, TextureHandle, TextureOptions, Ui, UiBuilder, ViewportCommand, WidgetInfo,
    WidgetType, WindowLevel, pos2, text::LayoutJob, vec2,
};

use crate::converter::Category;
use crate::engine::{Angle, Calculator, Command, Func, HistoryEntry, Op, Radix};
use icons::{
    ICON_BACK, ICON_BACK_TO_FULL, ICON_BACKSPACE, ICON_CALCULATOR, ICON_CALENDAR,
    ICON_CHEVRON_DOWN, ICON_CHEVRON_UP, ICON_CODE, ICON_COPY, ICON_DELETE, ICON_DIVIDE,
    ICON_EQUALS, ICON_HISTORY, ICON_KEEP_ON_TOP, ICON_LANGUAGE, ICON_MENU, ICON_MINUS,
    ICON_MULTIPLY, ICON_PLUS, ICON_REGION, ICON_SCIENTIFIC, ICON_SETTINGS, ICON_THEME,
    category_icon, icon_label,
};
use palette::Palette;
use strings::{tr, tr_format};

/// A converter's name, from its resource key in Windows (`NavCategory`'s
/// `NameResourceKey` with "Text" added).
fn category_name(category: Category) -> &'static str {
    tr(match category {
        Category::Volume => "CategoryName_VolumeText",
        Category::Length => "CategoryName_LengthText",
        Category::Weight => "CategoryName_WeightText",
        Category::Temperature => "CategoryName_TemperatureText",
        Category::Energy => "CategoryName_EnergyText",
        Category::Area => "CategoryName_AreaText",
        Category::Speed => "CategoryName_SpeedText",
        Category::Time => "CategoryName_TimeText",
        Category::Power => "CategoryName_PowerText",
        Category::Data => "CategoryName_DataText",
        Category::Pressure => "CategoryName_PressureText",
        Category::Angle => "CategoryName_AngleText",
    })
}

const HEADER_HEIGHT: f32 = 48.0;
const COMPACT_HEADER_HEIGHT: f32 = 40.0;
const MEMORY_ROW_HEIGHT: f32 = 30.0;
const MARGIN: f32 = 4.0;
const GAP: f32 = 2.0;
const RADIUS: f32 = 4.0;
const COMPACT_SIZE: egui::Vec2 = vec2(320.0, 394.0);
/// The smallest window, Windows' `AppMinWindowWidth` and `AppMinWindowHeight`: every
/// mode fits without clipping.
pub const MIN_SIZE: egui::Vec2 = vec2(320.0, 500.0);
/// Keep-on-top mode shows only the display and the keypad, so it can be smaller.
const COMPACT_MIN_SIZE: egui::Vec2 = vec2(280.0, 360.0);
/// From this window width on, the Standard mode docks history and memory in a side
/// panel (`DockVisible` in `Calculator.xaml`). The panel takes 240 of every 560 px
/// (`ColumnHistory` is `240*` next to `320*`), up to 320 px.
const DOCK_WIDTH: f32 = 560.0;
const DOCK_MAX_WIDTH: f32 = 320.0;
/// The memory buttons are at most 80 px wide (`MaxWidth="80"`).
const MEMORY_BUTTON_MAX_WIDTH: f32 = 80.0;
/// How long (in seconds) a button looks pressed after its keyboard key is pressed.
const KEY_FLASH: f64 = 0.1;
/// The menu button's names. Windows uses WinUI's own `NavigationView` strings, which
/// aren't in the Calculator's resources, so these stay English.
const NAVIGATION_OPEN: &str = "Open Navigation";
const NAVIGATION_CLOSE: &str = "Close Navigation";

const REPOSITORY_URL: &str = "https://github.com/rapalon/calccraft";
const FEEDBACK_URL: &str = "https://github.com/rapalon/calccraft/issues";
/// Storage key and values of the theme choice, the same as Windows uses.
const THEME_KEY: &str = "SelectedAppTheme";
/// Storage key of the mode the app was closed in, the same as Windows uses
/// (`ApplicationViewModel.Mode`); the value is `Mode::serialization_id`.
const MODE_KEY: &str = "Mode";
/// Storage key of the last converter and its units, the same as Windows uses.
const CONVERTER_KEY: &str = "UnitConverterPreferences";
/// Storage keys of the keep-on-top window's last size, the same as Windows uses.
const COMPACT_WIDTH_KEY: &str = "calculatorAlwaysOnTopLastWidth";
const COMPACT_HEIGHT_KEY: &str = "calculatorAlwaysOnTopLastHeight";
/// Storage keys of the Language and Regional format choices, which Windows doesn't have:
/// a language or locale name, or `Default` for the Windows setting (like the theme's).
const LANGUAGE_KEY: &str = "CalcCraftLanguage";
const REGION_KEY: &str = "CalcCraftRegionalFormat";
const SYSTEM_CHOICE: &str = "Default";

/// The Settings page's drop-down lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SettingsList {
    Language,
    Region,
}

impl SettingsList {
    /// The id of the list's box, which gets the focus back when the list closes.
    fn box_id(self) -> Id {
        Id::new("settings-combo").with(self)
    }

    /// The choice it shows.
    fn choice(self, app: &CalculatorApp) -> Option<&str> {
        match self {
            SettingsList::Language => app.language_choice.as_deref(),
            SettingsList::Region => app.region_choice.as_deref(),
        }
    }
}

/// A Settings drop-down's entries: the Windows setting (`None`) first, then the languages or
/// regional formats by their own names (`Deutsch (Deutschland)`), sorted.
fn settings_entries(list: SettingsList) -> Vec<(Option<&'static str>, &'static str)> {
    static LANGUAGES: OnceLock<Vec<(&'static str, String)>> = OnceLock::new();
    let mut entries = vec![(None, tr("SystemThemeRadioButton.Content"))];
    match list {
        SettingsList::Language => {
            let languages = LANGUAGES.get_or_init(|| {
                let mut languages: Vec<_> = strings::languages()
                    .map(|tag| (tag, region::native_name(tag)))
                    .collect();
                languages.sort_by_cached_key(|(_, name)| name.to_lowercase());
                languages
            });
            entries.extend(
                languages
                    .iter()
                    .map(|(tag, name)| (Some(*tag), name.as_str())),
            );
        }
        SettingsList::Region => entries.extend(
            region::regions()
                .iter()
                .map(|region| (Some(region.tag.as_str()), region.name.as_str())),
        ),
    }
    entries
}

/// A Settings card with a drop-down, like WinUI's `SettingsCard` with a `ComboBox`: icon,
/// title and description on the left, the box on the right, or under the text in cards
/// narrower than 476 px (`SettingsCardWrapThreshold`).
struct ComboCard {
    list: SettingsList,
    title_text: &'static str,
    value_text: String,
    title: Arc<Galley>,
    description: Arc<Galley>,
    /// The value in the box, cut with "…" if it doesn't fit.
    value: Arc<Galley>,
    wide: bool,
    box_width: f32,
    height: f32,
}

impl ComboCard {
    /// From a card's left edge to the center of its icon and to its text, in every
    /// Settings card.
    const ICON_X: f32 = 28.0;
    const TEXT_X: f32 = 58.0;
    const PADDING: f32 = 16.0;
    const BOX_HEIGHT: f32 = 32.0;
    /// The box's room for its chevron, right of the value.
    const CHEVRON_ROOM: f32 = 40.0;

    #[allow(clippy::too_many_arguments)]
    fn new(
        painter: &Painter,
        p: &Palette,
        width: f32,
        list: SettingsList,
        title: &'static str,
        description: &str,
        value: &str,
    ) -> Self {
        let wide = width >= 476.0;
        let font = FontId::proportional(14.0);
        let room = width - Self::TEXT_X - Self::PADDING;
        let max_box = if wide { room / 2.0 } else { room };
        let natural = painter
            .layout_no_wrap(value.to_string(), font.clone(), p.text)
            .size()
            .x
            + 12.0
            + Self::CHEVRON_ROOM;
        let box_width = natural.clamp(120.0_f32.min(max_box), max_box);
        let text_width = if wide {
            room - box_width - Self::PADDING
        } else {
            room
        };
        let title_galley = painter.layout(title.to_string(), font.clone(), p.text, text_width);
        let description_galley = painter.layout(
            description.to_string(),
            FontId::proportional(12.0),
            p.text_secondary,
            text_width,
        );
        let mut job = LayoutJob::simple_singleline(value.to_string(), font, p.text);
        job.wrap =
            egui::text::TextWrapping::truncate_at_width(box_width - 12.0 - Self::CHEVRON_ROOM);
        let value_galley = painter.layout_job(job);
        let text_height = title_galley.size().y + description_galley.size().y;
        let height = if wide {
            (text_height + 2.0 * Self::PADDING).max(68.0)
        } else {
            Self::PADDING + text_height + 12.0 + Self::BOX_HEIGHT + Self::PADDING
        };
        Self {
            list,
            title_text: title,
            value_text: value.to_string(),
            title: title_galley,
            description: description_galley,
            value: value_galley,
            wide,
            box_width,
            height,
        }
    }
}

/// An open Settings drop-down: which, and the box it opened from.
#[derive(Clone, Copy)]
struct SettingsPopup {
    list: SettingsList,
    anchor: Rect,
    /// Scroll the list so the selected item sits over the box (the first frame).
    scroll_pending: bool,
    /// An item typed-ahead to, to scroll to and focus.
    jump: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flyout {
    None,
    History,
    Memory,
    Navigation,
}

/// The two lists a flyout or the docked panel shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    History,
    Memory,
}

/// Calculator mode, picked in the navigation menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Standard,
    Scientific,
    Graphing,
    Programmer,
    DateCalculation,
    Converter(Category),
}

impl Mode {
    /// The modes of the menu's "Calculator" group, in Windows' order; the converters
    /// follow.
    const CALCULATORS: [Mode; 5] = [
        Mode::Standard,
        Mode::Scientific,
        Mode::Graphing,
        Mode::Programmer,
        Mode::DateCalculation,
    ];

    fn title(self) -> &'static str {
        match self {
            Mode::Standard => tr("StandardModeText"),
            Mode::Scientific => tr("ScientificModeText"),
            Mode::Graphing => tr("GraphingCalculatorModeText"),
            Mode::Programmer => tr("ProgrammerModeText"),
            Mode::DateCalculation => tr("DateCalculationModeText"),
            Mode::Converter(category) => category_name(category),
        }
    }

    /// The mode an `Alt` + digit shortcut opens. Windows' `NavCategory` gives the
    /// calculators `Alt+1` (Standard) to `Alt+5` (Date calculation) and the converters
    /// none.
    fn for_alt_key(key: Key) -> Option<Mode> {
        match key {
            Key::Num1 => Some(Mode::Standard),
            Key::Num2 => Some(Mode::Scientific),
            Key::Num3 => Some(Mode::Graphing),
            Key::Num4 => Some(Mode::Programmer),
            Key::Num5 => Some(Mode::DateCalculation),
            _ => None,
        }
    }

    /// The modes with a keypad engine. Windows' calculator view model switches its
    /// engine between them, and they share the memory.
    fn is_calculator(self) -> bool {
        matches!(self, Mode::Standard | Mode::Scientific | Mode::Programmer)
    }

    /// The modes with a history (Windows gives the Programmer engine none).
    fn has_history(self) -> bool {
        matches!(self, Mode::Standard | Mode::Scientific)
    }

    /// Windows' id of the mode, `NavCategory`'s `SerializationId`: Windows saves it as its
    /// `Mode` setting to open in that mode next time. The ids never change; 16 is the
    /// Currency converter, which CalcCraft doesn't have.
    fn serialization_id(self) -> i32 {
        match self {
            Mode::Standard => 0,
            Mode::Scientific => 1,
            Mode::Programmer => 2,
            Mode::DateCalculation => 3,
            Mode::Converter(category) => match category {
                Category::Volume => 4,
                Category::Length => 5,
                Category::Weight => 6,
                Category::Temperature => 7,
                Category::Energy => 8,
                Category::Area => 9,
                Category::Speed => 10,
                Category::Time => 11,
                Category::Power => 12,
                Category::Data => 13,
                Category::Pressure => 14,
                Category::Angle => 15,
            },
            Mode::Graphing => 17,
        }
    }

    /// The mode with that `serialization_id`, if CalcCraft has it
    /// (`NavCategoryStates.Deserialize`).
    fn from_serialization_id(id: i32) -> Option<Mode> {
        Mode::CALCULATORS
            .into_iter()
            .chain(Category::ALL.map(Mode::Converter))
            .find(|mode| mode.serialization_id() == id)
    }
}

/// What fills the window: the calculator or, as in Windows, the Settings page instead.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Calculator,
    Settings,
}

/// The "App theme" choice on the Settings page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Theme {
    Light,
    Dark,
    /// Follow the Windows app mode, and switch when it changes.
    System,
}

impl Theme {
    /// The stored value, as Windows stores it (`Default` means "use system setting").
    fn key(self) -> &'static str {
        match self {
            Theme::Light => "Light",
            Theme::Dark => "Dark",
            Theme::System => "Default",
        }
    }

    fn from_key(key: &str) -> Option<Theme> {
        [Theme::Light, Theme::Dark, Theme::System]
            .into_iter()
            .find(|theme| theme.key() == key)
    }

    fn preference(self) -> egui::ThemePreference {
        match self {
            Theme::Light => egui::ThemePreference::Light,
            Theme::Dark => egui::ThemePreference::Dark,
            Theme::System => egui::ThemePreference::System,
        }
    }
}

/// The palette for the theme egui resolved (the system one for `Theme::System`).
fn palette_for(theme: egui::Theme) -> &'static Palette {
    match theme {
        egui::Theme::Dark => &palette::DARK,
        egui::Theme::Light => &palette::LIGHT,
    }
}

/// Button style.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Digit,
    Operator,
    Accent,
    /// Transparent button (memory row, header icons).
    Subtle,
    /// The Scientific mode's second functions while `2nd` is on: an operator key that
    /// turns to the accent color on hover (`EmphasizedCalcButtonStyle`).
    Emphasized,
}

/// The window sizes that keep-on-top mode switches between.
#[derive(Default)]
struct WindowSizes {
    /// The full window's size, which comes back when keep-on-top mode ends.
    full: Option<egui::Vec2>,
    /// The keep-on-top window's size when the mode was last left. As in Windows
    /// (`ApplicationViewModel.ToggleAlwaysOnTop`), the mode opens at it again, and the
    /// settings keep it (`calculatorAlwaysOnTopLastWidth` and `…Height`).
    compact: Option<egui::Vec2>,
}

impl WindowSizes {
    /// The sizes at start-up, with the keep-on-top size saved last time.
    fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        let number = |key| {
            storage?
                .get_string(key)?
                .parse::<f32>()
                .ok()
                .filter(|n| n.is_finite() && *n > 0.0)
        };
        let compact = number(COMPACT_WIDTH_KEY).zip(number(COMPACT_HEIGHT_KEY));
        WindowSizes {
            full: None,
            compact: compact.map(|(width, height)| vec2(width, height)),
        }
    }

    /// Saves the keep-on-top size, under Windows' keys.
    fn save(&self, storage: &mut dyn eframe::Storage) {
        if let Some(size) = self.compact {
            storage.set_string(COMPACT_WIDTH_KEY, size.x.to_string());
            storage.set_string(COMPACT_HEIGHT_KEY, size.y.to_string());
        }
    }

    /// Keep-on-top mode starts: keeps the full window's size and returns the size to switch
    /// to, the one keep-on-top mode was last left at, or Windows' 320 × 394 the first time.
    fn enter_compact(&mut self, input: &egui::InputState) -> egui::Vec2 {
        self.full = Some(window_size(input));
        self.compact
            .map_or(COMPACT_SIZE, |size| size.max(COMPACT_MIN_SIZE))
    }

    /// Keep-on-top mode ends: keeps the window's size for the next time and returns the
    /// full window's size to switch back to.
    fn leave_compact(&mut self, input: &egui::InputState) -> egui::Vec2 {
        // Only a normal window's size is kept: Windows' keep-on-top window can't be
        // maximized (X11 and Wayland keep the maximize button), and a minimized window has
        // no size to come back to.
        let viewport = input.viewport();
        if viewport.maximized != Some(true) && viewport.minimized != Some(true) {
            self.compact = Some(window_size(input));
        }
        self.full.take().unwrap_or(MIN_SIZE).max(MIN_SIZE)
    }
}

/// In the Android app, a light tap of the vibration motor as a finger lands on a key (see
/// `android::key_tap`), like the system keyboard's. Keys typed on a keyboard don't vibrate.
#[cfg_attr(not(target_os = "android"), allow(unused_variables))]
fn key_feedback(ui: &Ui, response: &Response) {
    #[cfg(target_os = "android")]
    if response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed()) {
        android::key_tap();
    }
}

/// Where the app is drawn: the whole window, except on phones, which draw apps under the
/// status bar, the navigation bar and a camera cutout (egui's safe area leaves them out).
fn app_rect(ui: &Ui) -> Rect {
    ui.ctx().content_rect().intersect(ui.max_rect())
}

/// The window's content size: the size of egui's area, which every platform reports.
/// `ViewportInfo::inner_rect` would also give it, but it is always `None` on Wayland,
/// which doesn't tell a window its position.
fn window_size(input: &egui::InputState) -> egui::Vec2 {
    input.viewport_rect().size()
}

/// The window commands that start (`on`) or end keep-on-top mode, switching to `size`.
/// Windows' keep-on-top window (a compact overlay) has no maximize button and is never
/// maximized: the mode turns the button off and leaves a maximized state before the new
/// size, and its end turns the button back on.
fn keep_on_top_commands(on: bool, size: egui::Vec2) -> Vec<ViewportCommand> {
    let buttons = ViewportCommand::EnableButtons {
        close: true,
        minimized: true,
        maximize: !on,
    };
    if on {
        vec![
            ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop),
            buttons,
            ViewportCommand::Maximized(false),
            ViewportCommand::MinInnerSize(COMPACT_MIN_SIZE),
            ViewportCommand::InnerSize(size),
        ]
    } else {
        vec![
            ViewportCommand::WindowLevel(WindowLevel::Normal),
            buttons,
            ViewportCommand::InnerSize(size),
            ViewportCommand::MinInnerSize(MIN_SIZE),
        ]
    }
}

pub struct CalculatorApp {
    mode: Mode,
    /// The Standard mode's engine; the Scientific and Programmer modes have their own in
    /// `scientific` and `programmer`.
    calc: Calculator,
    scientific: scientific_view::ScientificState,
    programmer: programmer_view::ProgrammerState,
    /// The calculator mode with a keypad (Standard, Scientific or Programmer) used last,
    /// which has the memory.
    last_calculator: Mode,
    date: date_view::DateState,
    converter: converter_view::ConverterState,
    graphing: graphing_view::GraphingState,
    flyout: Flyout,
    /// Whether the window is wide enough for the docked history and memory panel.
    docked: bool,
    /// The docked panel's tab. Like Windows, it follows the last flyout opened.
    dock_tab: Panel,
    /// The right-click menu of a history or memory item, when open.
    context_menu: Option<ContextMenu>,
    /// "Keep on top" mode: a small always-on-top window without the memory row.
    compact: bool,
    window_sizes: WindowSizes,
    /// Command triggered from the keyboard and when, to highlight its button.
    key_flash: Option<(Command, f64)>,
    page: Page,
    theme: Theme,
    /// Colors of the theme in effect, picked at the start of every frame.
    palette: &'static Palette,
    /// The theme the title bar was last colored for.
    title_bar_theme: Option<egui::Theme>,
    title_bar: TitleBar,
    /// Whether the "App theme" card on the Settings page shows its options.
    theme_expanded: bool,
    /// The window icon, shown in the About card.
    app_icon: TextureHandle,
    /// The control a popup or flyout was opened from with the keyboard. It gets the
    /// focus back when the popup closes, as in WinUI.
    focus_return: Option<Id>,
    /// A popup opened with the keyboard: its selected item takes the focus when drawn.
    focus_popup: bool,
    /// The control that had the keyboard focus last frame (egui drops it on `Esc`).
    last_focus: Option<Id>,
    /// The navigation menu has just opened.
    nav_opened: bool,
    /// The Language and Regional format choices: a tag, or `None` for the Windows setting.
    language_choice: Option<String>,
    region_choice: Option<String>,
    /// The script fonts loaded (see `strings::script_font`), and whether all of them are,
    /// for the lists that show every language and region in its own script.
    script_fonts: Vec<strings::ScriptFont>,
    all_scripts: bool,
    /// The open Settings drop-down, if any.
    settings_popup: Option<SettingsPopup>,
    /// What was typed into an open Settings drop-down to find an item, and when.
    type_ahead: (String, f64),
    #[cfg(feature = "support-prompt")]
    pub(crate) support: crate::support::SupportState,
}

impl CalculatorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(target_arch = "wasm32")]
        web::guard_shortcuts();
        let theme = cc
            .storage
            .and_then(|storage| storage.get_string(THEME_KEY))
            .and_then(|key| Theme::from_key(&key))
            .unwrap_or(Theme::System);
        // For screenshots of both themes: `CALCCRAFT_THEME=Light` or `Dark` (Windows' keys).
        #[cfg(not(target_arch = "wasm32"))]
        let theme = std::env::var("CALCCRAFT_THEME")
            .ok()
            .and_then(|key| Theme::from_key(&key))
            .unwrap_or(theme);
        cc.egui_ctx.set_theme(theme.preference());
        // A stored language or region, or `None` for the Windows setting.
        let choice = |key: &str| {
            cc.storage
                .and_then(|storage| storage.get_string(key))
                .filter(|value| value != SYSTEM_CHOICE)
        };
        let language_choice = choice(LANGUAGE_KEY);
        let region_choice = choice(REGION_KEY);
        // Before anything looks up a string or formats a number.
        let language = strings::set_language(language_choice.as_deref());
        let region = &region::set_region(region_choice.as_deref()).tag;
        let script_fonts = needed_fonts(language, region, false);
        setup_fonts(&cc.egui_ctx, &script_fonts);
        let icon = window_icon();
        let app_icon = cc.egui_ctx.load_texture(
            "app-icon",
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            ),
            // Mipmaps keep the 64 px icon smooth at the 16 pt it is drawn at.
            TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
        );
        let mut converter = converter_view::ConverterState::new();
        if let Some(saved) = cc.storage.and_then(|s| s.get_string(CONVERTER_KEY)) {
            converter.converter.restore_preferences(&saved);
        }
        let mut graphing = graphing_view::GraphingState::new();
        graphing.match_app_theme = cc
            .storage
            .and_then(|s| s.get_string(graphing_view::THEME_KEY))
            .is_some_and(|v| v == "true");
        let mode = cc
            .storage
            .and_then(|s| s.get_string(MODE_KEY))
            .and_then(|id| id.parse().ok())
            .and_then(Mode::from_serialization_id)
            .unwrap_or(Mode::Standard);
        // Ctrl with plus, minus and 0 zoom the graph, not the whole window, and Windows
        // Calculator has no such zoom elsewhere either.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let mut app = Self {
            mode: Mode::Standard,
            calc: Calculator::default(),
            scientific: scientific_view::ScientificState::new(),
            programmer: programmer_view::ProgrammerState::new(),
            last_calculator: Mode::Standard,
            date: date_view::DateState::new(),
            converter,
            graphing,
            flyout: Flyout::None,
            docked: false,
            dock_tab: Panel::History,
            context_menu: None,
            compact: false,
            window_sizes: WindowSizes::load(cc.storage),
            key_flash: None,
            page: Page::Calculator,
            theme,
            palette: palette_for(cc.egui_ctx.theme()),
            title_bar_theme: None,
            title_bar: TitleBar::new(cc),
            theme_expanded: false,
            app_icon,
            focus_return: None,
            focus_popup: false,
            last_focus: None,
            nav_opened: false,
            language_choice,
            region_choice,
            script_fonts,
            all_scripts: false,
            settings_popup: None,
            type_ahead: (String::new(), 0.0),
            #[cfg(feature = "support-prompt")]
            support: crate::support::SupportState::new(cc.storage),
        };
        // Like Windows (`MainPage.OnNavigatedTo`), open the mode the app was closed in, the
        // way the menu switches to it: a calculator's engine takes the memory and becomes
        // the one `engine()` returns.
        app.set_mode(mode);
        #[cfg(feature = "support-prompt")]
        if app.support.demo_opens_menu() {
            app.flyout = Flyout::Navigation;
        }
        app
    }

    /// Whether a popup covers the page: the navigation menu, a context menu, or a
    /// drop-down or calendar. The controls under it leave the `Tab` order.
    fn popup_covers(&self) -> bool {
        self.flyout == Flyout::Navigation
            || self.context_menu.is_some()
            || self.date.popup_open()
            || self.converter.popup_open()
            || self.programmer.popup_open()
            || self.scientific.popup_open()
            || self.graphing.popup_open()
            || self.settings_popup.is_some()
    }

    /// The engine of the calculator mode on screen (or used last).
    fn engine(&self) -> &Calculator {
        match self.last_calculator {
            Mode::Programmer => &self.programmer.calc,
            Mode::Scientific => &self.scientific.calc,
            _ => &self.calc,
        }
    }

    fn engine_mut(&mut self) -> &mut Calculator {
        match self.last_calculator {
            Mode::Programmer => &mut self.programmer.calc,
            Mode::Scientific => &mut self.scientific.calc,
            _ => &mut self.calc,
        }
    }

    /// A number from the engine for the screen: in the region's format, except in the
    /// Programmer mode's other radixes, which have their own digit groups.
    fn localize_number(&self, text: &str) -> String {
        match self.engine().radix() {
            Some(radix) if radix != Radix::Decimal => text.to_string(),
            _ => numbers().localize(text),
        }
    }

    /// Switches the interface language (`None`: the Windows setting).
    fn set_language(&mut self, ctx: &Context, choice: Option<String>) {
        strings::set_language(choice.as_deref());
        self.language_choice = choice;
        self.update_fonts(ctx);
    }

    /// Switches the regional format (`None`: the Windows setting).
    fn set_region(&mut self, ctx: &Context, choice: Option<String>) {
        region::set_region(choice.as_deref());
        self.region_choice = choice;
        self.update_fonts(ctx);
        // A decimal comma changes how equations are read.
        self.graphing.reparse(graphing_view::region_syntax());
    }

    /// Loads the script fonts the language and the region need (all of them once a list
    /// that shows every language or region has opened), if they aren't loaded yet.
    fn update_fonts(&mut self, ctx: &Context) {
        let fonts = needed_fonts(
            strings::language(),
            &region::formats().tag,
            self.all_scripts,
        );
        if fonts != self.script_fonts {
            setup_fonts(ctx, &fonts);
            self.script_fonts = fonts;
        }
    }

    /// Notes that a popup or flyout opened from the control `id`. If the keyboard
    /// opened it (the control has the focus), the popup's selected item takes the
    /// focus, and the control gets it back when the popup closes.
    fn opened_from(&mut self, ctx: &Context, id: Id) {
        if access::focused(ctx) == Some(id) {
            self.focus_return = Some(id);
            self.focus_popup = true;
        }
    }

    /// Takes the request to focus a popup's selected item, for the item that is drawn.
    fn take_popup_focus(&mut self, selected: bool) -> bool {
        let focus = selected && self.focus_popup;
        if focus {
            self.focus_popup = false;
        }
        focus
    }

    fn set_theme(&mut self, ctx: &Context, theme: Theme) {
        self.theme = theme;
        ctx.set_theme(theme.preference());
    }

    fn set_mode(&mut self, mode: Mode) {
        if mode.is_calculator() && mode != self.last_calculator {
            // Windows switches the engine: the memory goes along, and the engine starts
            // over (`SetStandardMode`, `SetScientificMode`, `SetProgrammerMode`). Leaving
            // the Programmer mode turns its bit toggling keypad off. Date calculation and
            // the converters in between change nothing.
            let memory = self.engine_mut().take_memory();
            let left = self.last_calculator;
            self.last_calculator = mode;
            let engine = self.engine_mut();
            engine.set_memory(memory);
            engine.reset();
            if left == Mode::Programmer {
                self.programmer.bit_flip = false;
            }
        }
        self.mode = mode;
        self.page = Page::Calculator;
        self.flyout = Flyout::None;
        self.context_menu = None;
        self.date.close_popup();
        self.converter.close_popup();
        self.programmer.close_popup();
        self.scientific.close_popup();
        self.graphing.close_popup();
        self.graphing.tracing = false;
        if let Mode::Converter(category) = mode {
            self.converter.converter.set_category(category);
        }
    }

    // -------------------------------------------------------------- keyboard

    /// The mode shortcuts: `Alt+1` … `Alt+5` (see `Mode::for_alt_key`) and `Ctrl+E`
    /// for Date calculation. As in Windows' `KeyboardShortcutManager`, they do nothing
    /// in keep-on-top mode or with `AltGr`, which arrives as `Ctrl+Alt`. The keys are
    /// consumed, so the mode that opens doesn't also type the digit.
    fn handle_mode_keys(&mut self, ctx: &Context) {
        // Taken in every state, so that no Alt chord types its digit.
        let chosen = ctx.input_mut(|input| take_mode_shortcut(&mut input.events));
        if self.compact || self.page != Page::Calculator {
            return;
        }
        if let Some(mode) = chosen
            && mode != self.mode
        {
            self.set_mode(mode);
        }
    }

    fn handle_keyboard(&mut self, ctx: &Context) {
        if self.page == Page::Settings {
            // The calculator is hidden, so its keys and shortcuts do nothing here. Esc
            // closes an open drop-down first, and typing in one jumps to an item that
            // starts with what was typed, as in a WinUI ComboBox.
            if ctx.input(|i| i.key_pressed(Key::Escape)) && self.settings_popup.take().is_none() {
                self.page = Page::Calculator;
            }
            if let Some(popup) = &mut self.settings_popup {
                let now = ctx.input(|i| i.time);
                let typed: String = ctx.input(|i| {
                    i.events
                        .iter()
                        .filter_map(|event| match event {
                            Event::Text(text) => Some(text.as_str()),
                            _ => None,
                        })
                        .collect()
                });
                let (prefix, at) = &mut self.type_ahead;
                if now - *at > 1.0 {
                    prefix.clear();
                }
                // A space continues a search ("deutsch (s…"); otherwise it presses the
                // focused item, so it doesn't start one.
                let typed = if prefix.is_empty() {
                    typed.trim_start()
                } else {
                    typed.as_str()
                };
                if !typed.is_empty() {
                    if typed.contains(' ') {
                        ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Space));
                    }
                    prefix.push_str(&typed.to_lowercase());
                    *at = now;
                    let entries = settings_entries(popup.list);
                    popup.jump = entries
                        .iter()
                        .position(|(_, name)| name.to_lowercase().starts_with(prefix.as_str()));
                }
            }
            return;
        }
        // With the menu or a context menu open, Windows ignores the calculator's keys
        // (`HonorShortcuts(false)`); Esc closes them.
        let covered = self.flyout == Flyout::Navigation
            || self.context_menu.is_some()
            || self.programmer.popup_open()
            || self.scientific.popup_open();
        let mut events = ctx.input(|i| i.events.clone());
        remove_alt_gr_keys(&mut events, ctx.input(|i| i.key_down(Key::AltRight)));
        // After AltGr's keys are gone, so a letter typed with AltGr doesn't become a shortcut.
        latin_letters(&mut events, layout_letter, self.mode == Mode::Programmer);
        let now = ctx.input(|i| i.time);
        for event in events {
            match event {
                Event::Text(text) if !covered && !self.keypad_disabled() => {
                    let commands: Vec<Command> = match self.mode {
                        Mode::Programmer => text
                            .chars()
                            .filter_map(|c| self.programmer.char_command(c))
                            .collect(),
                        Mode::Scientific => text
                            .chars()
                            .filter_map(scientific_view::ScientificState::char_command)
                            .collect(),
                        _ => text.chars().filter_map(char_command).collect(),
                    };
                    for command in commands {
                        self.press(command, now);
                    }
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if !covered || key == Key::Escape => {
                    self.handle_key(ctx, key, modifiers, now);
                }
                Event::Copy if !covered => {
                    if let Some(text) = self.engine().copy_text() {
                        ctx.copy_text(numbers().localize_plain(&text));
                    }
                }
                Event::Paste(text) if !covered => {
                    self.engine_mut().paste(&numbers().delocalize(&text));
                }
                _ => {}
            }
        }
        repaint_for_flash(ctx, &mut self.key_flash, now);
    }

    /// An open history or memory flyout disables the keypad and the memory buttons
    /// (`EnableControls(false)`), so their keys do nothing.
    fn keypad_disabled(&self) -> bool {
        matches!(self.flyout, Flyout::History | Flyout::Memory)
    }

    fn handle_key(&mut self, ctx: &Context, key: Key, modifiers: Modifiers, now: f64) {
        let programmer = self.mode == Mode::Programmer;
        // The Scientific mode's letter keys, some with Shift or Ctrl (sinh is Ctrl+S).
        let scientific = (self.mode == Mode::Scientific && key != Key::Escape)
            .then(|| scientific_view::ScientificState::key_command(key, modifiers))
            .flatten();
        let command = if let Some(command) = scientific {
            command
        } else if modifiers.command {
            // Keep on top hides the history and disables the memory buttons, and Windows
            // turns their shortcuts off (`AreHistoryShortcutsEnabled`).
            if self.compact {
                return;
            }
            // Windows' Ctrl+Shift shortcuts are only Ctrl+Shift+D and the Scientific
            // mode's letters.
            match (key, modifiers.shift) {
                (Key::M, false) => Command::MemoryStore,
                (Key::P, false) => Command::MemoryAdd,
                (Key::Q, false) => Command::MemorySubtract,
                (Key::R, false) => Command::MemoryRecall,
                (Key::L, false) => Command::MemoryClear,
                // The Programmer mode has no history.
                (Key::H | Key::D, _) if programmer => return,
                // The docked panel already shows the history, so Windows ignores it then.
                (Key::H, false) if self.docked => return,
                (Key::H, false) => return self.toggle_flyout(Flyout::History),
                (Key::D, true) => return self.engine_mut().clear_history(),
                _ => return,
            }
        } else if let Some(command) = (programmer && modifiers.is_none())
            .then(|| self.programmer.key_command(key))
            .flatten()
        {
            command
        } else {
            match key {
                // Enter presses a focused button that isn't one of the calculator's keys.
                Key::Enter if access::focused(ctx).is_some_and(|id| !is_calculator_key(id)) => {
                    return;
                }
                Key::Enter => {
                    // Otherwise it's `=`, and the focused key isn't pressed too.
                    ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                    Command::Equals
                }
                Key::Escape if self.context_menu.is_some() => return self.context_menu = None,
                Key::Escape if self.programmer.close_popup() => return,
                Key::Escape if self.scientific.close_popup() => return,
                Key::Escape if self.flyout != Flyout::None => return self.flyout = Flyout::None,
                Key::Escape => {
                    // egui drops the focus on Esc; in Windows it's only `C`.
                    if let Some(id) = self.last_focus {
                        ctx.memory_mut(|memory| memory.request_focus(id));
                    }
                    Command::Clear
                }
                Key::Delete => Command::ClearEntry,
                Key::Backspace => Command::Backspace,
                Key::F9 => Command::Function(Func::Negate),
                Key::F1 if self.compact => return self.set_compact(ctx, false),
                _ => return,
            }
        };
        if !self.keypad_disabled() {
            self.press(command, now);
        }
    }

    /// Tells the support prompt about a key the calculator executed.
    #[cfg(feature = "support-prompt")]
    pub(crate) fn check_calculation_trigger(&mut self, command: Command) {
        let has_error = self.engine().has_error();
        self.support.after_command(command, has_error);
    }

    fn press(&mut self, command: Command, now: f64) {
        self.engine_mut().execute(command);
        self.key_flash = Some((command, now));
        #[cfg(feature = "support-prompt")]
        self.check_calculation_trigger(command);
    }

    fn is_flashing(&self, command: Command, now: f64) -> bool {
        matches!(self.key_flash, Some((c, t)) if c == command && now - t < KEY_FLASH)
    }

    fn toggle_flyout(&mut self, flyout: Flyout) {
        self.flyout = if self.flyout == flyout {
            Flyout::None
        } else {
            flyout
        };
        // Windows' docked panel opens on the list whose flyout was opened last.
        match self.flyout {
            Flyout::History => self.dock_tab = Panel::History,
            Flyout::Memory => self.dock_tab = Panel::Memory,
            Flyout::Navigation => self.nav_opened = true,
            Flyout::None => {}
        }
    }

    fn set_compact(&mut self, ctx: &Context, compact: bool) {
        self.compact = compact;
        self.flyout = Flyout::None;
        let sizes = &mut self.window_sizes;
        let size = ctx.input(|i| {
            if compact {
                sizes.enter_compact(i)
            } else {
                sizes.leave_compact(i)
            }
        });
        for command in keep_on_top_commands(compact, size) {
            ctx.send_viewport_cmd(command);
        }
    }

    // --------------------------------------------------------------- buttons

    /// Draws a button and returns `true` if it was clicked. `name` is what a screen
    /// reader says.
    #[allow(clippy::too_many_arguments)]
    fn button(
        &self,
        ui: &Ui,
        rect: Rect,
        id: Id,
        kind: Kind,
        enabled: bool,
        label: LayoutJob,
        name: &str,
    ) -> bool {
        self.button_flash(ui, rect, id, kind, enabled, label, name, false)
    }

    #[allow(clippy::too_many_arguments)]
    fn button_flash(
        &self,
        ui: &Ui,
        rect: Rect,
        id: Id,
        kind: Kind,
        enabled: bool,
        label: LayoutJob,
        name: &str,
        flash: bool,
    ) -> bool {
        let p = self.palette;
        let response = access::control(
            ui,
            p,
            rect,
            id,
            enabled,
            RADIUS,
            access::button(enabled, name),
        );
        self.paint_key(ui, rect, kind, enabled, &response, flash, label);
        enabled && response.clicked()
    }

    /// Paints a button of `kind` in its state (disabled, hovered, pressed, or flashing
    /// for its keyboard key) with its label.
    #[allow(clippy::too_many_arguments)]
    fn paint_key(
        &self,
        ui: &Ui,
        rect: Rect,
        kind: Kind,
        enabled: bool,
        response: &Response,
        flash: bool,
        label: LayoutJob,
    ) {
        let p = self.palette;
        let pressed = enabled && (response.is_pointer_button_down_on() || flash);
        let hovered = enabled && response.hovered();
        key_feedback(ui, response);

        let pick = |normal, hover, down| {
            if pressed {
                down
            } else if hovered {
                hover
            } else {
                normal
            }
        };
        let fill = match kind {
            Kind::Digit if !enabled => p.digit_disabled,
            Kind::Digit => pick(p.digit, p.digit_hover, p.digit_pressed),
            Kind::Operator | Kind::Emphasized if !enabled => p.operator_disabled,
            Kind::Operator => pick(p.operator, p.operator_hover, p.operator_pressed),
            Kind::Emphasized => pick(p.operator, p.emphasized_hover, p.emphasized_pressed),
            Kind::Accent if !enabled => p.accent_disabled,
            Kind::Accent => pick(p.accent, p.accent_hover, p.accent_pressed),
            Kind::Subtle => pick(Color32::TRANSPARENT, p.subtle_hover, p.subtle_pressed),
        };
        let text = if !enabled {
            p.text_disabled
        } else if kind == Kind::Accent {
            p.text_on_accent
        } else if kind == Kind::Emphasized && (pressed || hovered) {
            p.text_on_emphasized
        } else if pressed {
            p.text_pressed
        } else {
            p.text
        };

        let painter = ui.painter();
        painter.rect_filled(rect, RADIUS, fill);
        if matches!(kind, Kind::Digit | Kind::Operator | Kind::Emphasized) {
            painter.rect_stroke(rect, RADIUS, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
        let galley = painter.layout_job(label);
        painter.galley(rect.center() - galley.size() / 2.0, galley, text);
    }

    // ---------------------------------------------------------------- header

    fn draw_header(&mut self, ui: &Ui, rect: Rect) {
        let p = self.palette;
        let ctx = ui.ctx().clone();
        let y = rect.center().y;
        let icon_button = |x: f32| Rect::from_center_size(pos2(x, y), vec2(40.0, 36.0));

        if self.compact {
            let label = icon_label(ICON_BACK_TO_FULL, 14.0);
            if self.button(
                ui,
                icon_button(rect.left() + 24.0),
                Id::new("full"),
                Kind::Subtle,
                true,
                label,
                tr("ExitAlwaysOnTopButton.AutomationName"),
            ) {
                self.set_compact(&ctx, false);
            }
            return;
        }

        let label = icon_label(ICON_MENU, 16.0);
        if self.button(
            ui,
            icon_button(rect.left() + 24.0),
            Id::new("menu"),
            Kind::Subtle,
            true,
            label,
            NAVIGATION_OPEN,
        ) {
            self.toggle_flyout(Flyout::Navigation);
            self.opened_from(&ctx, Id::new("menu"));
        }
        #[cfg(feature = "support-prompt")]
        self.support
            .draw_menu_badge(ui, icon_button(rect.left() + 24.0), Id::new("menu"));

        let title = ui.painter().text(
            pos2(rect.left() + 48.0, y),
            Align2::LEFT_CENTER,
            self.mode.title(),
            FontId::new(20.0, semibold()),
            p.text,
        );
        // Keep on top exists only in the Standard mode, the history in the Standard and
        // Scientific modes, and its button only while the history isn't docked. A browser
        // tab can't stay on top or change its size, so the web version has no keep on top, and
        // neither has the Android app, which fills the screen.
        if self.mode == Mode::Standard && !cfg!(any(target_arch = "wasm32", target_os = "android"))
        {
            let label = icon_label(ICON_KEEP_ON_TOP, 14.0);
            let keep_on_top = icon_button(title.right() + 26.0);
            if self.button(
                ui,
                keep_on_top,
                Id::new("keep-on-top"),
                Kind::Subtle,
                true,
                label,
                tr("EnterAlwaysOnTopButton.AutomationName"),
            ) {
                self.set_compact(&ctx, true);
            }
        }
        if !self.mode.has_history() || self.docked {
            return;
        }

        let label = icon_label(ICON_HISTORY, 16.0);
        let name = if self.flyout == Flyout::History {
            tr("HistoryButton_Close")
        } else {
            tr("HistoryButton_Open")
        };
        if self.button(
            ui,
            icon_button(rect.right() - 24.0),
            Id::new("history"),
            Kind::Subtle,
            true,
            label,
            name,
        ) {
            self.toggle_flyout(Flyout::History);
            self.opened_from(&ctx, Id::new("history"));
        }
    }

    // --------------------------------------------------------------- display

    /// The display's text: the number, or the error message in the interface's language.
    fn display_text(&self) -> String {
        match self.engine().error() {
            Some(error) => tr(&format!("CEngine.{}", error.resource_id())).to_string(),
            None => self.localize_number(&self.engine().display()),
        }
    }

    fn draw_display(&self, ui: &Ui, rect: Rect) {
        let base_size = (rect.height() * 0.46).clamp(28.0, 72.0);
        let center_y = rect.top() + rect.height() * 0.6;
        self.draw_display_sized(ui, rect, rect.top() + 6.0, base_size, center_y);
    }

    /// The expression line from `expression_top` and the number, at most `base_size` pt
    /// and vertically centered on `center_y`, right-aligned in `rect`.
    fn draw_display_sized(
        &self,
        ui: &Ui,
        rect: Rect,
        expression_top: f32,
        base_size: f32,
        center_y: f32,
    ) {
        #[cfg(feature = "support-prompt")]
        crate::support::note_display(ui.ctx(), rect);
        let p = self.palette;
        let painter = ui.painter();
        let right = rect.right() - 12.0;
        let max_width = rect.width() - 24.0;
        let calc = self.engine();

        let expression = numbers().localize_expression(&calc.expression());
        // Keep on top shows only the result (`DisplayModeAlwaysOnTop` sets the expression
        // row's height to 0).
        if !expression.is_empty() && !self.compact {
            let galley =
                layout_elided_left(painter, &expression, FontId::proportional(14.0), max_width);
            let pos = pos2(right - galley.size().x, expression_top);
            let name = tr_format("Format_CalculatorExpression", &[&expression]);
            let line = Rect::from_min_size(pos, galley.size());
            access::text(ui, line, Id::new("expression"), &name, false);
            painter.galley(pos, galley, p.text_secondary);
        }

        // Screen readers read the display out whenever it changes ("Display is 7"),
        // and say "point" for a decimal point that has no digits after it yet. Hex,
        // octal and binary numbers are read digit by digit.
        let display = self.display_text();
        let name = match display.strip_suffix(numbers().decimal.as_str()) {
            _ if calc.radix().is_some_and(|radix| radix != Radix::Decimal) && !calc.has_error() => {
                let digits = programmer_view::spell_digits(&display);
                tr_format("Format_CalculatorResults", &[&digits])
            }
            Some(number) if !calc.has_error() => {
                tr_format("Format_CalculatorResults_Decimal", &[number])
            }
            _ => tr_format("Format_CalculatorResults", &[&display]),
        };
        access::text(ui, rect, Id::new("display-value"), &name, true);

        // Long numbers shrink to fit the width.
        let mut galley =
            painter.layout_no_wrap(display.clone(), FontId::new(base_size, semibold()), p.text);
        if galley.size().x > max_width {
            let size = (base_size * max_width / galley.size().x).max(10.0);
            galley = painter.layout_no_wrap(display, FontId::new(size, semibold()), p.text);
        }
        let pos = pos2(right - galley.size().x, center_y - galley.size().y / 2.0);
        painter.galley(pos, galley, p.text);
    }

    // ------------------------------------------------------------ memory row

    /// The memory row, with its buttons disabled when `all_enabled` is false (while a
    /// flyout covers the keys, Windows' `EnableMemoryControls(false)`).
    fn draw_memory_row(&mut self, ui: &Ui, rect: Rect, now: f64, all_enabled: bool) {
        let buttons = [
            ("MC", Command::MemoryClear),
            ("MR", Command::MemoryRecall),
            ("M+", Command::MemoryAdd),
            ("M−", Command::MemorySubtract),
            ("MS", Command::MemoryStore),
        ];
        // Star columns as in `Calculator.xaml`: the buttons share the width up to 80 px
        // each, and a 0.01* spacer before `M˅` takes the rest. Docked, `M˅` is gone.
        let columns = if self.docked { 5.0 } else { 6.0 };
        let width = ((rect.width() - 2.0 * MARGIN) / (columns + 0.01)).min(MEMORY_BUTTON_MAX_WIDTH);
        let cell = |i: usize| {
            let left = if i == 5 {
                rect.right() - MARGIN - width
            } else {
                rect.left() + MARGIN + i as f32 * width
            };
            Rect::from_min_size(pos2(left, rect.top()), vec2(width, rect.height()))
                .shrink2(vec2(1.0, 0.0))
        };
        for (i, (label, command)) in buttons.into_iter().enumerate() {
            let enabled = all_enabled && self.engine().is_enabled(command);
            let flash = self.is_flashing(command, now);
            let label = text(label, 12.0, semibold());
            if self.button_flash(
                ui,
                cell(i),
                Id::new("memory").with(i),
                Kind::Subtle,
                enabled,
                label,
                command_name(command),
                flash,
            ) {
                self.engine_mut().execute(command);
            }
        }
        if self.docked {
            return;
        }

        let enabled = !self.engine().memory().is_empty() && !self.engine().has_error();
        let label = Self::memory_list_label();
        let name = if self.flyout == Flyout::Memory {
            tr("MemoryButton_Close")
        } else {
            tr("MemoryButton_Open")
        };
        let id = Id::new("memory-list");
        if self.button(ui, cell(5), id, Kind::Subtle, enabled, label, name) {
            self.toggle_flyout(Flyout::Memory);
            self.opened_from(ui.ctx(), id);
        }
    }

    // ---------------------------------------------------------------- keypad

    fn keypad_label(&self, command: Command, height: f32) -> LayoutJob {
        let digit_size = (height * 0.38).clamp(14.0, 40.0);
        let icon_size = (height * 0.31).clamp(12.0, 32.0);
        let text_size = (height * 0.29).clamp(12.0, 30.0);
        let regular = FontFamily::Proportional;
        // x², ¹⁄x, ²√x as in Windows, with an italic "x".
        let formula = |before: &str, after: &str| {
            let mut job = LayoutJob::default();
            let plain = format(text_size, regular.clone());
            job.append(before, 0.0, plain.clone());
            job.append(
                "x",
                0.0,
                TextFormat {
                    italics: true,
                    ..plain.clone()
                },
            );
            job.append(after, 0.0, plain);
            job
        };
        match command {
            Command::Digit(d) => text(&d.to_string(), digit_size, semibold()),
            Command::Decimal => text(".", digit_size, semibold()),
            Command::Function(Func::Negate) => negate_label(text_size),
            Command::Function(Func::Reciprocal) => formula("¹⁄", ""),
            Command::Function(Func::Square) => formula("", "²"),
            Command::Function(Func::SquareRoot) => formula("²√", ""),
            Command::Operator(Op::Divide) => icon_label(ICON_DIVIDE, icon_size),
            Command::Operator(Op::Multiply) => icon_label(ICON_MULTIPLY, icon_size),
            Command::Operator(Op::Subtract) => icon_label(ICON_MINUS, icon_size),
            Command::Operator(Op::Add) => icon_label(ICON_PLUS, icon_size),
            Command::Equals => icon_label(ICON_EQUALS, icon_size),
            Command::Backspace => icon_label(ICON_BACKSPACE, icon_size),
            Command::Percent => text("%", text_size, regular),
            Command::ClearEntry => text("CE", text_size, regular),
            Command::Clear => text("C", text_size, regular),
            _ => LayoutJob::default(),
        }
    }

    fn draw_keypad(&mut self, ui: &Ui, rect: Rect, now: f64) {
        use Command::*;
        #[rustfmt::skip]
        const KEYS: [Command; 24] = [
            Percent, ClearEntry, Clear, Backspace,
            Function(Func::Reciprocal), Function(Func::Square), Function(Func::SquareRoot), Operator(Op::Divide),
            Digit(7), Digit(8), Digit(9), Operator(Op::Multiply),
            Digit(4), Digit(5), Digit(6), Operator(Op::Subtract),
            Digit(1), Digit(2), Digit(3), Operator(Op::Add),
            Function(Func::Negate), Digit(0), Decimal, Equals,
        ];
        let width = (rect.width() - 3.0 * GAP) / 4.0;
        let height = (rect.height() - 5.0 * GAP) / 6.0;
        for (i, command) in KEYS.into_iter().enumerate() {
            let (row, col) = ((i / 4) as f32, (i % 4) as f32);
            let cell = Rect::from_min_size(
                rect.min + vec2(col * (width + GAP), row * (height + GAP)),
                vec2(width, height),
            );
            let kind = match command {
                Digit(_) | Decimal | Function(Func::Negate) => Kind::Digit,
                Equals => Kind::Accent,
                _ => Kind::Operator,
            };
            let enabled = self.calc.is_enabled(command);
            let flash = self.is_flashing(command, now);
            let label = self.keypad_label(command, height);
            if self.button_flash(
                ui,
                cell,
                Id::new("key").with(i),
                kind,
                enabled,
                label,
                command_name(command),
                flash,
            ) {
                self.calc.execute(command);
                #[cfg(feature = "support-prompt")]
                self.check_calculation_trigger(command);
            }
        }
    }

    // ---------------------------------------------------- history and memory

    /// The history or memory flyout, which covers the keypad.
    fn draw_flyout(&mut self, ui: &mut Ui, rect: Rect, panel: Panel) {
        let p = self.palette;
        let painter = ui.painter();
        let top = CornerRadius {
            nw: 8,
            ne: 8,
            sw: 0,
            se: 0,
        };
        painter.rect_filled(rect, top, p.flyout);
        painter.rect_stroke(
            rect.expand2(vec2(1.0, 0.0)),
            top,
            Stroke::new(1.0, p.flyout_border),
            StrokeKind::Inside,
        );
        // Clicks on the panel's empty space must not fall through it.
        ui.interact(rect, Id::new("flyout"), Sense::CLICK);
        self.draw_list(ui, rect, panel, vec2(16.0, 16.0));
    }

    /// The docked panel of wide windows: History and Memory tabs over the list, on the
    /// window background. The tabs are a WinUI `Pivot` (`DockPanelTemplate`): 48 px
    /// headers in 14 px text, 3 px higher than the panel, with a 16 × 3 px accent
    /// pipe under the selected one.
    fn draw_dock(&mut self, ui: &mut Ui, rect: Rect) {
        const HEADER_HEIGHT: f32 = 48.0;
        const HEADER_PADDING: f32 = 12.0;
        let p = self.palette;
        let painter = ui.painter().clone();
        let top = rect.top() - 3.0;
        let mut left = rect.left();
        // The Programmer mode has no history: the panel is the memory list alone.
        let programmer = self.mode == Mode::Programmer;
        let mut tabs = vec![(Panel::Memory, tr("MemoryLabel.Text"))];
        if !programmer {
            tabs.insert(0, (Panel::History, tr("HistoryLabel.Text")));
        }
        let shown = if programmer {
            Panel::Memory
        } else {
            self.dock_tab
        };
        for (i, (panel, label)) in tabs.into_iter().enumerate() {
            let galley = painter.layout_no_wrap(
                label.to_string(),
                FontId::proportional(14.0),
                Color32::PLACEHOLDER,
            );
            let header = Rect::from_min_size(
                pos2(left, top),
                vec2(galley.size().x + 2.0 * HEADER_PADDING, HEADER_HEIGHT),
            );
            left = header.right();
            let selected = shown == panel;
            let info = access::item(WidgetType::SelectableLabel, label, selected);
            let id = Id::new("dock-tab").with(i);
            let response = access::control(ui, p, header, id, true, RADIUS, info);
            let pressed = response.is_pointer_button_down_on();
            let hovered = response.hovered();
            let (color, pipe) = if pressed {
                (p.text_tertiary, p.accent_pressed)
            } else if hovered {
                (p.text_secondary, p.accent_hover)
            } else {
                (p.text, p.accent)
            };
            let text_pos = pos2(
                header.left() + HEADER_PADDING,
                header.center().y - galley.size().y / 2.0,
            );
            painter.galley(text_pos, galley, color);
            if selected {
                let pipe_rect = Rect::from_center_size(
                    pos2(header.center().x, header.bottom() - 2.0 - 1.5),
                    vec2(16.0, 3.0),
                );
                painter.rect_filled(pipe_rect, 1.5, pipe);
            }
            if response.clicked() && shown != panel {
                self.dock_tab = panel;
                self.context_menu = None;
            }
        }
        // `PivotItem` content has a 10 px top margin.
        let content = rect.with_min_y(top + HEADER_HEIGHT + 10.0);
        self.draw_list(ui, content, shown, vec2(12.0, 8.0));
    }

    /// The history or memory list with its clear-all button, in a flyout or docked.
    /// `empty_offset` places the message an empty list shows.
    fn draw_list(&mut self, ui: &mut Ui, rect: Rect, panel: Panel, empty_offset: egui::Vec2) {
        let p = self.palette;
        let painter = ui.painter();
        let history = panel == Panel::History;
        // A menu whose item is gone or moved (or that belongs to the other panel) closes.
        self.context_menu = self.context_menu.filter(|menu| match menu.target {
            MenuTarget::History(_, key) => history && history_key(self.engine().history()) == key,
            MenuTarget::Memory(i) => !history && i < self.engine().memory().len(),
            MenuTarget::Radix(_) => true,
        });
        let empty = if history {
            self.engine().history().is_empty()
        } else {
            self.engine().memory().is_empty()
        };
        if empty {
            let message = if history {
                tr("HistoryEmpty.Text")
            } else {
                tr("MemoryPaneEmpty.Text")
            };
            painter.text(
                rect.left_top() + empty_offset,
                Align2::LEFT_TOP,
                message,
                // `BodyTextBlockStyle`: regular, not semibold.
                FontId::proportional(14.0),
                p.text,
            );
            return;
        }

        let list = Rect::from_min_max(
            rect.min + vec2(4.0, 8.0),
            pos2(rect.right() - 4.0, rect.bottom() - 48.0),
        );
        let mut action = None;
        ui.scope_builder(UiBuilder::new().max_rect(list), |ui| {
            egui::ScrollArea::vertical()
                .id_salt(("list", history))
                .auto_shrink(false)
                .scroll_source(access::scroll_source(ui, Default::default()))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // A flyout opened with the keyboard focuses its first item.
                    if history {
                        for i in 0..self.engine().history().len() {
                            let focus = self.take_popup_focus(i == 0);
                            if let Some(a) = self.history_item(ui, i, focus) {
                                action = Some(a);
                            }
                        }
                    } else {
                        for i in 0..self.engine().memory().len() {
                            let focus = self.take_popup_focus(i == 0);
                            if let Some(a) = self.memory_item(ui, i, focus) {
                                action = Some(a);
                            }
                        }
                    }
                });
        });

        let trash = Rect::from_center_size(
            pos2(rect.right() - 28.0, rect.bottom() - 24.0),
            vec2(40.0, 36.0),
        );
        let label = icon_label(ICON_DELETE, 15.0);
        let name = if history {
            tr("ClearHistory.AutomationName")
        } else {
            tr("ClearMemory.AutomationName")
        };
        if self.button(ui, trash, Id::new("trash"), Kind::Subtle, true, label, name) {
            action = Some(FlyoutAction::ClearAll);
        }
        // Drawn last, so it covers the panel and catches the clicks outside it. The radix
        // list's menu is the Programmer mode's own.
        if let Some(menu) = self.context_menu
            && !matches!(menu.target, MenuTarget::Radix(_))
        {
            action = self.draw_context_menu(ui, menu);
        }

        // The context menu covers the list, so an action while it's open comes from it.
        let from_menu = self.context_menu.is_some();
        if action.is_some() {
            self.context_menu = None;
        }
        match action {
            Some(FlyoutAction::RecallHistory(i)) => {
                self.engine_mut().recall_history(i);
                self.flyout = Flyout::None;
            }
            Some(FlyoutAction::RecallMemory(i)) => {
                self.engine_mut().memory_recall_item(i);
                self.flyout = Flyout::None;
            }
            Some(FlyoutAction::MemoryAdd(i, subtract)) => self.engine_mut().memory_add(i, subtract),
            Some(FlyoutAction::MemoryClear(i)) => {
                self.engine_mut().memory_clear_item(i);
                // Clearing the last item from its context menu closes the flyout
                // (`MemoryEmptied`); the item's own MC button leaves it open, as in Windows.
                if from_menu && self.engine().memory().is_empty() && self.flyout == Flyout::Memory {
                    self.flyout = Flyout::None;
                }
            }
            Some(FlyoutAction::ClearAll) if history => self.engine_mut().clear_history(),
            Some(FlyoutAction::ClearAll) => self.engine_mut().execute(Command::MemoryClear),
            Some(FlyoutAction::OpenMenu(target, pos)) => {
                self.context_menu = Some(ContextMenu { target, pos });
            }
            // Windows copies the result as the list shows it, with separators.
            Some(FlyoutAction::CopyHistory(i)) => {
                ui.ctx()
                    .copy_text(numbers().localize(&self.engine().history()[i].text));
            }
            Some(FlyoutAction::DeleteHistory(i)) => self.engine_mut().delete_history_item(i),
            Some(FlyoutAction::CopyRadix(_) | FlyoutAction::CloseMenu) | None => {}
        }
    }

    /// History entry: small expression, large result. A click recalls it and a
    /// right click opens its menu. `focus` gives it the keyboard focus.
    fn history_item(&self, ui: &mut Ui, index: usize, focus: bool) -> Option<FlyoutAction> {
        let p = self.palette;
        let entry = &self.engine().history()[index];
        let width = ui.available_width();
        let painter = ui.painter().clone();
        let entry_expression = numbers().localize_expression(&entry.expression);
        let entry_result = numbers().localize(&entry.text);
        // Long lines wrap (`TextWrapping="Wrap"`), as the Scientific mode's 32 digits need.
        let expression = layout_wrapped_right(
            &painter,
            &entry_expression,
            FontId::proportional(14.0),
            width - 24.0,
            false,
        );
        let result = layout_wrapped_right(
            &painter,
            &entry_result,
            FontId::new(20.0, semibold()),
            width - 24.0,
            true,
        );
        let height = 10.0 + expression.size().y + result.size().y + 10.0;

        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        // Windows names the entry by its expression and result.
        let name = format!("{entry_expression} {entry_result}");
        let id = Id::new("history-item").with(index);
        let info = access::item(WidgetType::SelectableLabel, &name, false);
        let response = access::control(ui, p, rect, id, true, RADIUS, info);
        if focus {
            response.request_focus();
        }
        if response.hovered() {
            painter.rect_filled(rect, RADIUS, p.subtle_hover);
        }
        let right = rect.right() - 12.0;
        let y = rect.top() + 10.0;
        let expression_height = expression.size().y;
        paint_right(&painter, expression, pos2(right, y), p.text_secondary);
        paint_right(&painter, result, pos2(right, y + expression_height), p.text);
        if response.secondary_clicked() {
            let pos = response.interact_pointer_pos().unwrap_or(rect.center());
            let target = MenuTarget::History(index, history_key(self.engine().history()));
            return Some(FlyoutAction::OpenMenu(target, pos));
        }
        response
            .clicked()
            .then_some(FlyoutAction::RecallHistory(index))
    }

    /// Memory slot; on hover it shows MC, M+ and M− buttons, and so it does while it or
    /// one of them has the keyboard focus. `focus` gives it the focus.
    fn memory_item(&self, ui: &mut Ui, index: usize, focus: bool) -> Option<FlyoutAction> {
        let p = self.palette;
        let calc = self.engine();
        let value = &calc.memory()[index];
        let width = ui.available_width();
        // In the Programmer mode, in the current radix.
        let shown = self.localize_number(&calc.format_value(value));
        let result = layout_wrapped_right(
            ui.painter(),
            &shown,
            FontId::new(20.0, semibold()),
            width - 24.0,
            true,
        );
        let height = 10.0 + result.size().y + 6.0 + 26.0 + 10.0;

        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let id = Id::new("memory-slot").with(index);
        let info = access::item(WidgetType::SelectableLabel, &shown, false);
        let response = access::control(ui, p, rect, id, true, RADIUS, info);
        if focus {
            response.request_focus();
        }
        let buttons_focused = access::focused(ui.ctx()).is_some_and(|focused| {
            (0..3).any(|i| focused == Id::new("memory-item").with(index).with(i))
        });
        let hovered = ui.rect_contains_pointer(rect) || response.has_focus() || buttons_focused;
        let painter = ui.painter();
        if hovered {
            painter.rect_filled(rect, RADIUS, p.subtle_hover);
        }
        let right = rect.right() - 12.0;
        let result_height = result.size().y;
        paint_right(painter, result, pos2(right, rect.top() + 10.0), p.text);

        let mut action = response
            .clicked()
            .then_some(FlyoutAction::RecallMemory(index));
        if response.secondary_clicked() {
            let pos = response.interact_pointer_pos().unwrap_or(rect.center());
            action = Some(FlyoutAction::OpenMenu(MenuTarget::Memory(index), pos));
        }
        if hovered {
            let buttons = [
                (
                    "MC",
                    "ClearMemoryItemButton.AutomationName",
                    FlyoutAction::MemoryClear(index),
                ),
                (
                    "M+",
                    "MemPlusItem.AutomationName",
                    FlyoutAction::MemoryAdd(index, false),
                ),
                (
                    "M−",
                    "MemMinusItem.AutomationName",
                    FlyoutAction::MemoryAdd(index, true),
                ),
            ];
            let top = rect.top() + 10.0 + result_height + 6.0;
            for (i, (label, name, button_action)) in buttons.into_iter().enumerate() {
                let x = right - (3 - i) as f32 * 42.0 + 2.0;
                let cell = Rect::from_min_size(pos2(x, top), vec2(40.0, 26.0));
                let id = Id::new("memory-item").with(index).with(i);
                if self.button(
                    ui,
                    cell,
                    id,
                    Kind::Digit,
                    true,
                    text(label, 12.0, semibold()),
                    tr(name),
                ) {
                    action = Some(button_action);
                }
            }
        }
        action
    }

    /// The right-click menu of a history or memory item or of the Programmer mode's
    /// radix list, a WinUI `MenuFlyout` at the pointer. A click outside it closes it.
    fn draw_context_menu(&self, ui: &Ui, menu: ContextMenu) -> Option<FlyoutAction> {
        const ITEM_HEIGHT: f32 = 32.0;
        const STRIDE: f32 = ITEM_HEIGHT + 2.0;
        const ICON_CENTER: f32 = 20.0;
        const TEXT_LEFT: f32 = 40.0;
        let p = self.palette;
        let full = ui.max_rect();
        let backdrop = ui.interact(full, Id::new("context-menu-backdrop"), Sense::CLICK);
        access::set_covered(ui.ctx(), false);

        // The memory items show the Calculator font's MC, M+ and M− glyphs as text.
        let items = match menu.target {
            MenuTarget::History(i, _) => vec![
                (
                    icon_label(ICON_COPY, 16.0),
                    tr("CopyHistoryMenuItem.Text"),
                    FlyoutAction::CopyHistory(i),
                ),
                (
                    icon_label(ICON_DELETE, 16.0),
                    tr("DeleteHistoryMenuItem.Text"),
                    FlyoutAction::DeleteHistory(i),
                ),
            ],
            MenuTarget::Memory(i) => vec![
                (
                    text("MC", 11.0, semibold()),
                    tr("ClearMemoryMenuItem.Text"),
                    FlyoutAction::MemoryClear(i),
                ),
                (
                    text("M+", 11.0, semibold()),
                    tr("MemPlusMenuItem.Text"),
                    FlyoutAction::MemoryAdd(i, false),
                ),
                (
                    text("M−", 11.0, semibold()),
                    tr("MemMinusMenuItem.Text"),
                    FlyoutAction::MemoryAdd(i, true),
                ),
            ],
            MenuTarget::Radix(radix) => vec![(
                icon_label(ICON_COPY, 16.0),
                tr("copyMenuItem"),
                FlyoutAction::CopyRadix(radix),
            )],
        };
        let painter = ui.painter();
        let font = FontId::proportional(14.0);
        let text_width = items
            .iter()
            .map(|(_, label, _)| {
                painter
                    .layout_no_wrap(label.to_string(), font.clone(), p.text)
                    .size()
                    .x
            })
            .fold(0.0, f32::max);
        let size = vec2(
            (8.0 + TEXT_LEFT + text_width + 24.0).max(120.0),
            8.0 + items.len() as f32 * STRIDE - 2.0,
        );
        // The menu opens down and to the right of the pointer, and flips where
        // the window has no room for that.
        let mut min = menu.pos;
        if min.x + size.x > full.right() - 4.0 {
            min.x = (min.x - size.x).max(full.left() + 4.0);
        }
        if min.y + size.y > full.bottom() - 4.0 {
            min.y = (min.y - size.y).max(full.top() + 4.0);
        }
        let rect = Rect::from_min_size(min, size);
        date_view::popup_frame(p, painter, rect, p.popup);
        ui.interact(rect, Id::new("context-menu"), Sense::CLICK);

        let mut chosen = None;
        for (n, (icon, label, action)) in items.into_iter().enumerate() {
            let item = Rect::from_min_size(
                rect.min + vec2(4.0, 4.0 + n as f32 * STRIDE),
                vec2(rect.width() - 8.0, ITEM_HEIGHT),
            );
            // "Delete" is "Delete history item" to screen readers.
            let name = match action {
                FlyoutAction::DeleteHistory(_) => tr("DeleteHistoryMenuItem.AutomationName"),
                _ => label,
            };
            let id = Id::new("context-menu-item").with(n);
            let response =
                access::control(ui, p, item, id, true, RADIUS, access::button(true, name));
            let fill = if response.is_pointer_button_down_on() {
                p.popup_item_pressed
            } else if response.hovered() {
                p.popup_item
            } else {
                Color32::TRANSPARENT
            };
            painter.rect_filled(item, RADIUS, fill);
            let icon = painter.layout_job(icon);
            let icon_pos = pos2(item.left() + ICON_CENTER, item.center().y) - icon.size() / 2.0;
            painter.galley(icon_pos, icon, p.text);
            painter.text(
                pos2(item.left() + TEXT_LEFT, item.center().y),
                Align2::LEFT_CENTER,
                label,
                font.clone(),
                p.text,
            );
            if response.clicked() {
                chosen = Some(action);
            }
        }
        if chosen.is_none() && (backdrop.clicked() || backdrop.secondary_clicked()) {
            chosen = Some(FlyoutAction::CloseMenu);
        }
        chosen
    }

    // ------------------------------------------------------------------ menu

    fn draw_navigation(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette;
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, p.backdrop);
        let backdrop = ui.interact(rect, Id::new("nav-backdrop"), Sense::CLICK);
        access::set_covered(ui.ctx(), false);

        let panel = Rect::from_min_size(
            rect.min,
            vec2((rect.width() * 0.85).min(280.0), rect.height()),
        );
        painter.rect_filled(panel, 0.0, p.flyout);
        painter.vline(
            panel.right(),
            panel.y_range(),
            Stroke::new(1.0, p.flyout_border),
        );
        ui.interact(panel, Id::new("nav-panel"), Sense::CLICK);

        let menu = Rect::from_center_size(
            pos2(panel.left() + 24.0, panel.top() + HEADER_HEIGHT / 2.0),
            vec2(40.0, 36.0),
        );
        let label = icon_label(ICON_MENU, 16.0);
        let id = Id::new("nav-menu");
        let mut close = self.button(ui, menu, id, Kind::Subtle, true, label, NAVIGATION_CLOSE);

        #[cfg(feature = "support-prompt")]
        {
            self.support.draw_menu_badge(ui, menu, id);
            let button_rect = Rect::from_min_max(
                pos2(
                    menu.right() + 8.0,
                    panel.top() + (HEADER_HEIGHT - 32.0) / 2.0,
                ),
                pos2(
                    panel.right() - 10.0,
                    panel.top() + (HEADER_HEIGHT + 32.0) / 2.0,
                ),
            );
            if self.support.draw_menu_button(ui, button_rect, p) {
                close = true;
            }
        }

        // The modes in two groups. They scroll between the header and Settings
        // when the window is too short for all of them.
        let list = Rect::from_min_max(
            pos2(panel.left(), panel.top() + HEADER_HEIGHT),
            pos2(panel.right(), panel.bottom() - 58.0),
        );
        // Each group's heading and the name screen readers give it.
        let groups = [
            (
                tr("CalculatorModeTextCaps"),
                tr("CalculatorModePluralText"),
                Mode::CALCULATORS.to_vec(),
            ),
            (
                tr("ConverterModeTextCaps"),
                tr("ConverterModePluralText"),
                Category::ALL.map(Mode::Converter).to_vec(),
            ),
        ];
        // Like Windows, the menu opens scrolled to the top (egui would keep, and even save,
        // the last position).
        let mut area = egui::ScrollArea::vertical()
            .id_salt("nav-modes")
            .auto_shrink(false);
        if std::mem::take(&mut self.nav_opened) {
            area = area.vertical_scroll_offset(0.0);
        }
        let mut chosen = None;
        ui.scope_builder(UiBuilder::new().max_rect(list), |ui| {
            area.show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (title, plural, modes) in groups {
                    let size = vec2(ui.available_width(), 34.0);
                    let (header, _) = ui.allocate_exact_size(size, Sense::hover());
                    let name = tr_format("NavCategoryHeader_AutomationNameFormat", &[plural]);
                    access::text(ui, header, Id::new("nav-group").with(title), &name, false);
                    ui.painter().text(
                        pos2(header.left() + 16.0, header.top() + 8.0),
                        Align2::LEFT_TOP,
                        title,
                        FontId::new(14.0, semibold()),
                        p.text,
                    );
                    for mode in modes {
                        let size = vec2(ui.available_width(), 40.0);
                        let (row, _) = ui.allocate_exact_size(size, Sense::hover());
                        let item = row.shrink2(vec2(4.0, 0.0));
                        let icon = match mode {
                            Mode::Standard => icon_label(ICON_CALCULATOR, 16.0),
                            Mode::Scientific => icon_label(ICON_SCIENTIFIC, 16.0),
                            // Drawn below: Windows' comes from the Calculator's own font.
                            Mode::Graphing => LayoutJob::default(),
                            Mode::Programmer => icon_label(ICON_CODE, 16.0),
                            Mode::DateCalculation => icon_label(ICON_CALENDAR, 16.0),
                            Mode::Converter(category) => icon_label(category_icon(category), 16.0),
                        };
                        let id = Id::new("nav-mode").with(mode.title());
                        let selected = self.mode == mode;
                        // "Standard Calculator", "Length Converter".
                        let name = tr_format(
                            "NavCategoryItem_AutomationNameFormat",
                            &[mode.title(), title],
                        );
                        let focus = self.take_popup_focus(selected);
                        if self.nav_item(ui, item, id, icon, mode.title(), &name, selected, focus) {
                            chosen = Some(mode);
                        }
                        if mode == Mode::Graphing {
                            let center = pos2(item.left() + 24.0, item.center().y);
                            graphing_view::paint_graph_icon(ui.painter(), center, 16.0, p.text);
                        }
                    }
                }
            });
        });
        if let Some(mode) = chosen {
            self.set_mode(mode);
            close = true;
        }

        // Settings sits at the bottom of the pane, below a separator, as in Windows.
        let item = Rect::from_min_size(
            pos2(panel.left() + 4.0, panel.bottom() - 46.0),
            vec2(panel.width() - 8.0, 40.0),
        );
        painter.hline(
            panel.x_range(),
            item.top() - 12.0,
            Stroke::new(1.0, p.flyout_border),
        );
        let icon = icon_label(ICON_SETTINGS, 16.0);
        let label = tr("SettingsHeader.Text");
        let id = Id::new("nav-settings");
        let settings = self.nav_item(ui, item, id, icon, label, label, false, false);

        if settings {
            self.page = Page::Settings;
        }
        if close || settings || backdrop.clicked() {
            self.flyout = Flyout::None;
        }
    }

    /// Navigation pane entry. The selected one has a fill and the accent pill. `name` is
    /// what a screen reader says, and `focus` gives the entry the keyboard focus.
    /// Returns `true` if it was clicked.
    #[allow(clippy::too_many_arguments)]
    fn nav_item(
        &self,
        ui: &Ui,
        rect: Rect,
        id: Id,
        icon: LayoutJob,
        label: &str,
        name: &str,
        selected: bool,
        focus: bool,
    ) -> bool {
        let p = self.palette;
        let painter = ui.painter();
        let info = access::item(WidgetType::SelectableLabel, name, selected);
        let response = access::control(ui, p, rect, id, true, RADIUS, info);
        if focus {
            response.request_focus();
        }
        let fill = if response.is_pointer_button_down_on() || (selected && !response.hovered()) {
            p.subtle_hover
        } else if response.hovered() {
            p.operator_hover
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, RADIUS, fill);
        if selected {
            let pill =
                Rect::from_center_size(pos2(rect.left() + 2.0, rect.center().y), vec2(3.0, 16.0));
            painter.rect_filled(pill, 2.0, p.accent);
        }
        let icon = painter.layout_job(icon);
        painter.galley(
            pos2(rect.left() + 16.0, rect.center().y - icon.size().y / 2.0),
            icon,
            p.text,
        );
        painter.text(
            pos2(rect.left() + 48.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(14.0),
            p.text,
        );
        response.clicked()
    }

    // -------------------------------------------------------------- settings

    fn draw_settings(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette;
        // Windows puts the back button in its own title bar. This app keeps the system title
        // bar, so the button takes the place of the menu button.
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), HEADER_HEIGHT));
        let back = Rect::from_center_size(
            pos2(header.left() + 24.0, header.center().y),
            vec2(40.0, 36.0),
        );
        let label = icon_label(ICON_BACK, 16.0);
        if self.button(
            ui,
            back,
            Id::new("settings-back"),
            Kind::Subtle,
            true,
            label,
            tr("TitleBarBackButton.AutomationName"),
        ) {
            self.page = Page::Calculator;
        }
        ui.painter().text(
            pos2(header.left() + 48.0, header.center().y),
            Align2::LEFT_CENTER,
            tr("SettingsHeader.Text"),
            FontId::new(20.0, semibold()),
            p.text,
        );

        let mut action = None;
        // An open drop-down holds the page still: it stays over its box.
        let scrolling = if self.settings_popup.is_some() {
            egui::scroll_area::ScrollSource::NONE
        } else {
            egui::scroll_area::ScrollSource::default()
        };
        ui.scope_builder(
            UiBuilder::new().max_rect(rect.with_min_y(header.bottom())),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("settings")
                    .auto_shrink(false)
                    .scroll_source(scrolling)
                    .show(ui, |ui| action = self.settings_content(ui));
            },
        );
        let ctx = ui.ctx().clone();
        match action {
            Some(SettingsAction::ToggleTheme) => self.theme_expanded = !self.theme_expanded,
            Some(SettingsAction::SetTheme(theme)) => self.set_theme(&ctx, theme),
            Some(SettingsAction::OpenUrl(url)) => ctx.open_url(egui::OpenUrl::new_tab(url)),
            Some(SettingsAction::OpenList(list, anchor)) => {
                self.settings_popup = Some(SettingsPopup {
                    list,
                    anchor,
                    scroll_pending: true,
                    jump: None,
                });
                self.type_ahead.0.clear();
                self.opened_from(&ctx, list.box_id());
                // The entries are in their own scripts.
                self.all_scripts = true;
                self.update_fonts(&ctx);
            }
            None => {}
        }

        if let Some(popup) = self.settings_popup
            && let Some(choice) = self.draw_settings_list(ui, rect, popup)
        {
            let choice = choice.map(str::to_string);
            match popup.list {
                SettingsList::Language => self.set_language(&ctx, choice),
                SettingsList::Region => self.set_region(&ctx, choice),
            }
        }
    }

    /// The scrolling part of the Settings page: the Appearance and About sections and the links.
    fn settings_content(&self, ui: &mut Ui) -> Option<SettingsAction> {
        let p = self.palette;
        // Distances measured on the Windows 11 Settings page.
        const SIDE: f32 = 24.0;
        const CARD_HEIGHT: f32 = 68.0;
        const ICON_X: f32 = ComboCard::ICON_X;
        const TEXT_X: f32 = ComboCard::TEXT_X;
        const OPTION_HEIGHT: f32 = 40.0;
        // The themes and their labels' resource keys.
        const THEMES: [(Theme, &str); 3] = [
            (Theme::Light, "LightThemeRadioButton.Content"),
            (Theme::Dark, "DarkThemeRadioButton.Content"),
            (Theme::System, "SystemThemeRadioButton.Content"),
        ];

        let width = ui.available_width();
        let painter = ui.painter().clone();
        let body = FontId::proportional(14.0);

        // Lay out the text that wraps first: the page height depends on it.
        let text_width = width - 2.0 * SIDE - TEXT_X - 40.0;
        let title = painter.layout(
            tr("AppThemeExpander.Header").into(),
            body.clone(),
            p.text,
            text_width,
        );
        let subtitle = painter.layout(
            tr("AppThemeExpander.Description").into(),
            FontId::proportional(12.0),
            p.text_secondary,
            text_width,
        );
        let header_height = (title.size().y + subtitle.size().y + 32.0).max(CARD_HEIGHT);
        let options_height = if self.theme_expanded {
            2.0 * MARGIN + THEMES.len() as f32 * OPTION_HEIGHT
        } else {
            0.0
        };
        // The Language and Regional format cards: the region's describes it by example.
        let today = chrono::Local::now().date_naive();
        // A stored choice that isn't in the list shows by its own name.
        let chosen = |list: SettingsList| {
            let choice = list.choice(self);
            settings_entries(list)
                .into_iter()
                .find(|(tag, _)| *tag == choice)
                .map_or_else(
                    || choice.map(region::native_name),
                    |(_, name)| Some(name.to_string()),
                )
                .unwrap_or_default()
        };
        let combo_cards = [
            ComboCard::new(
                &painter,
                p,
                width - 2.0 * SIDE,
                SettingsList::Language,
                tr("CalcCraft.Language"),
                tr("CalcCraft.LanguageDescription"),
                &chosen(SettingsList::Language),
            ),
            ComboCard::new(
                &painter,
                p,
                width - 2.0 * SIDE,
                SettingsList::Region,
                tr("CalcCraft.RegionalFormat"),
                &region::formats().example(today),
                &chosen(SettingsList::Region),
            ),
        ];
        let link_text = "GitHub";
        // Windows' own sentence (`AboutControlContribute`) names the Windows Calculator, so
        // CalcCraft has its own, with the link at `%1`.
        let contribute_text = tr("CalcCraft.Contribute");
        let (prefix, suffix) = contribute_text
            .split_once("%1")
            .unwrap_or((contribute_text, ""));
        let paragraph = |link_color: Color32| {
            let mut job = LayoutJob::default();
            job.wrap.max_width = width - 2.0 * SIDE;
            for (part, color) in [(prefix, p.text), (link_text, link_color), (suffix, p.text)] {
                job.append(part, 0.0, TextFormat::simple(body.clone(), color));
            }
            painter.layout_job(job)
        };
        let mut contribute = paragraph(p.link);
        let row_height = contribute.rows.first().map_or(0.0, |row| row.size.y);

        // Vertical positions, from the top of the page body.
        let appearance_y = 23.0;
        let theme_top = appearance_y + 18.0;
        let theme_bottom = theme_top + header_height + options_height;
        // Cards in a group are 4 px apart.
        let language_top = theme_bottom + 4.0;
        let region_top = language_top + combo_cards[0].height + 4.0;
        let cards_bottom = region_top + combo_cards[1].height;
        let about_y = cards_bottom + 45.0;
        let about_top = about_y + 18.0;
        let feedback_y = about_top + CARD_HEIGHT + 25.0;
        let contribute_top = feedback_y + 40.0 - row_height / 2.0;
        let height = contribute_top + contribute.size().y + SIDE;

        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let (left, right, top) = (rect.left() + SIDE, rect.right() - SIDE, rect.top());
        let mut action = None;

        // Appearance: the App theme card expands to show the three choices.
        section_label(
            p,
            &painter,
            pos2(left, top + appearance_y),
            tr("SettingsAppearance.Text"),
        );
        let card = Rect::from_x_y_ranges(left..=right, top + theme_top..=top + theme_bottom);
        let header = card.with_max_y(card.top() + header_height);
        // WinUI's expander says whether it is expanded.
        let info = || {
            let mut info = WidgetInfo::labeled(
                WidgetType::CollapsingHeader,
                true,
                tr("AppThemeExpander.Header"),
            );
            info.selected = Some(self.theme_expanded);
            info
        };
        let response = access::control(ui, p, header, Id::new("theme-card"), true, RADIUS, info);
        let fill = if response.is_pointer_button_down_on() {
            p.card_pressed
        } else if response.hovered() {
            p.card_hover
        } else {
            p.card
        };
        painter.rect_filled(card, RADIUS, p.card);
        let corners = if self.theme_expanded {
            CornerRadius {
                nw: 4,
                ne: 4,
                sw: 0,
                se: 0,
            }
        } else {
            CornerRadius::same(4)
        };
        painter.rect_filled(header, corners, fill);
        painter.rect_stroke(
            card,
            RADIUS,
            Stroke::new(1.0, p.card_border),
            StrokeKind::Inside,
        );
        let icon = painter.layout_job(icon_label(ICON_THEME, 20.0));
        painter.galley(
            pos2(card.left() + ICON_X, header.center().y) - icon.size() / 2.0,
            icon,
            p.text,
        );
        let text_top = header.center().y - (title.size().y + subtitle.size().y) / 2.0;
        let title_height = title.size().y;
        painter.galley(pos2(card.left() + TEXT_X, text_top), title, p.text);
        painter.galley(
            pos2(card.left() + TEXT_X, text_top + title_height),
            subtitle,
            p.text_secondary,
        );
        let chevron = if self.theme_expanded {
            icon_label(ICON_CHEVRON_UP, 12.0)
        } else {
            icon_label(ICON_CHEVRON_DOWN, 12.0)
        };
        let chevron = painter.layout_job(chevron);
        painter.galley(
            pos2(card.right() - 24.0, header.center().y) - chevron.size() / 2.0,
            chevron,
            p.text,
        );
        if response.clicked() {
            action = Some(SettingsAction::ToggleTheme);
        }
        if self.theme_expanded {
            painter.hline(
                card.x_range(),
                header.bottom(),
                Stroke::new(1.0, p.card_border),
            );
            for (i, (theme, key)) in THEMES.into_iter().enumerate() {
                let y = header.bottom() + MARGIN + (i as f32 + 0.5) * OPTION_HEIGHT;
                let id = Id::new("theme").with(i);
                if radio_button(
                    p,
                    ui,
                    pos2(card.left() + TEXT_X, y),
                    tr(key),
                    self.theme == theme,
                    id,
                ) {
                    action = Some(SettingsAction::SetTheme(theme));
                }
            }
        }

        // Language and Regional format (CalcCraft's own; Windows follows its settings).
        for (card, card_top) in combo_cards.iter().zip([language_top, region_top]) {
            let rect =
                Rect::from_min_size(pos2(left, top + card_top), vec2(right - left, card.height));
            let icon = icon_label(
                match card.list {
                    SettingsList::Language => ICON_LANGUAGE,
                    SettingsList::Region => ICON_REGION,
                },
                20.0,
            );
            if let Some(anchor) = self.draw_combo_card(ui, &painter, rect, card, icon) {
                action = Some(SettingsAction::OpenList(card.list, anchor));
            }
        }

        // About: the app's name, icon and version
        section_label(
            p,
            &painter,
            pos2(left, top + about_y),
            tr("AboutGroupTitle.Text"),
        );
        let card = Rect::from_x_y_ranges(
            left..=right,
            top + about_top..=top + about_top + CARD_HEIGHT,
        );
        painter.rect_filled(card, RADIUS, p.card);
        painter.rect_stroke(
            card,
            RADIUS,
            Stroke::new(1.0, p.card_border),
            StrokeKind::Inside,
        );
        let icon = Rect::from_center_size(
            pos2(card.left() + ICON_X, card.center().y),
            vec2(16.0, 16.0),
        );
        let uv = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
        painter.image(self.app_icon.id(), icon, uv, Color32::WHITE);
        painter.text(
            pos2(card.left() + TEXT_X, card.center().y),
            Align2::LEFT_CENTER,
            "CalcCraft",
            body.clone(),
            p.text,
        );
        painter.text(
            pos2(card.right() - 20.0, card.center().y),
            Align2::RIGHT_CENTER,
            env!("CARGO_PKG_VERSION"),
            body.clone(),
            p.text_secondary,
        );

        let feedback = pos2(left, top + feedback_y);
        if link_button(
            p,
            ui,
            feedback,
            tr("FeedbackButton.Content"),
            Id::new("feedback"),
        ) {
            action = Some(SettingsAction::OpenUrl(FEEDBACK_URL));
        }

        // A sentence with an inline link: find the link's glyphs to make them clickable.
        let start = prefix.chars().count();
        let link = char_range_rect(&contribute, start..start + link_text.chars().count());
        let origin = pos2(left, top + contribute_top);
        let info = || WidgetInfo::labeled(WidgetType::Link, true, link_text);
        let link = link.translate(origin.to_vec2());
        let response = access::control(ui, p, link, Id::new("github"), true, RADIUS, info)
            .on_hover_cursor(CursorIcon::PointingHand);
        if response.is_pointer_button_down_on() {
            contribute = paragraph(p.link_pressed);
        } else if response.hovered() {
            contribute = paragraph(p.link_hover);
        }
        painter.galley(origin, contribute, p.text);
        if response.clicked() {
            action = Some(SettingsAction::OpenUrl(REPOSITORY_URL));
        }
        action
    }

    /// Draws a `ComboCard` in `rect`. Returns its box when the box was clicked.
    fn draw_combo_card(
        &self,
        ui: &Ui,
        painter: &Painter,
        rect: Rect,
        card: &ComboCard,
        icon: LayoutJob,
    ) -> Option<Rect> {
        let p = self.palette;
        painter.rect_filled(rect, RADIUS, p.card);
        painter.rect_stroke(
            rect,
            RADIUS,
            Stroke::new(1.0, p.card_border),
            StrokeKind::Inside,
        );
        let title_height = card.title.size().y;
        let text_height = title_height + card.description.size().y;
        let text_top = if card.wide {
            rect.center().y - text_height / 2.0
        } else {
            rect.top() + ComboCard::PADDING
        };
        let icon = painter.layout_job(icon);
        let icon_center = pos2(
            rect.left() + ComboCard::ICON_X,
            text_top + text_height / 2.0,
        );
        painter.galley(icon_center - icon.size() / 2.0, icon, p.text);
        let text_left = rect.left() + ComboCard::TEXT_X;
        painter.galley(pos2(text_left, text_top), card.title.clone(), p.text);
        painter.galley(
            pos2(text_left, text_top + title_height),
            card.description.clone(),
            p.text_secondary,
        );

        let size = vec2(card.box_width, ComboCard::BOX_HEIGHT);
        let min = if card.wide {
            pos2(
                rect.right() - ComboCard::PADDING - size.x,
                rect.center().y - size.y / 2.0,
            )
        } else {
            pos2(text_left, text_top + text_height + 12.0)
        };
        let field = Rect::from_min_size(min, size);
        let info = access::combo_box(card.title_text, &card.value_text);
        let response = access::control(ui, p, field, card.list.box_id(), true, RADIUS, info);
        let fill = if response.is_pointer_button_down_on() {
            p.control_pressed
        } else if response.hovered() {
            p.control_hover
        } else {
            p.control
        };
        painter.rect_filled(field, RADIUS, fill);
        painter.rect_stroke(
            field,
            RADIUS,
            Stroke::new(1.0, p.control_border),
            StrokeKind::Inside,
        );
        let value_pos = pos2(
            field.left() + 12.0,
            field.center().y - card.value.size().y / 2.0,
        );
        painter.galley(value_pos, card.value.clone(), p.text);
        let chevron = painter.layout_job(icon_label(ICON_CHEVRON_DOWN, 12.0));
        let chevron_center = pos2(field.right() - 18.0, field.center().y);
        painter.galley(
            chevron_center - chevron.size() / 2.0,
            chevron,
            p.text_secondary,
        );
        response.clicked().then_some(field)
    }

    /// The open Settings drop-down: the entries, with the selected one over the box where it
    /// fits, as in WinUI. Returns the entry chosen: `Some(None)` is the Windows setting.
    fn draw_settings_list(
        &mut self,
        ui: &mut Ui,
        full: Rect,
        popup: SettingsPopup,
    ) -> Option<Option<&'static str>> {
        const ITEM_HEIGHT: f32 = 36.0;
        /// WinUI's `MaxDropDownHeight` (504 px) in items.
        const VISIBLE_ITEMS: usize = 14;
        let p = self.palette;
        let entries = settings_entries(popup.list);
        let choice = popup.list.choice(self);
        let selected = entries
            .iter()
            .position(|(tag, _)| *tag == choice)
            .unwrap_or(0);
        let backdrop = ui.interact(full, Id::new("settings-backdrop"), Sense::CLICK);
        access::set_covered(ui.ctx(), false);

        let fits = ((full.height() - 16.0) / ITEM_HEIGHT).floor() as usize;
        let visible = entries.len().min(VISIBLE_ITEMS).min(fits).max(1);
        let height = visible as f32 * ITEM_HEIGHT + 8.0;
        let widest = entries
            .iter()
            .map(|(_, name)| {
                ui.painter()
                    .layout_no_wrap(name.to_string(), FontId::proportional(14.0), p.text)
                    .size()
                    .x
            })
            .fold(0.0, f32::max);
        let width = (widest + 48.0)
            .max(popup.anchor.width())
            .min(full.width() - 8.0);
        // As in the converters' unit lists: a list that scrolls is centered on the box and
        // scrolled to the selected entry, a short one is moved there.
        let lowest_top = (full.bottom() - 4.0 - height).max(full.top() + 4.0);
        let fit = |top: f32| top.clamp(full.top() + 4.0, lowest_top);
        let target = popup.anchor.center().y - ITEM_HEIGHT / 2.0 - 4.0;
        let max_offset = (entries.len() - visible) as f32 * ITEM_HEIGHT;
        let centered = fit(target - (visible / 2) as f32 * ITEM_HEIGHT);
        let row = selected as f32 * ITEM_HEIGHT;
        let offset = (row - (target - centered)).clamp(0.0, max_offset);
        let top = fit(target - (row - offset));
        let left = popup
            .anchor
            .left()
            .min(full.right() - 4.0 - width)
            .max(full.left() + 4.0);
        let rect = Rect::from_min_size(pos2(left, top), vec2(width, height));
        date_view::popup_frame(p, ui.painter(), rect, p.popup);
        ui.interact(rect, Id::new("settings-list"), Sense::CLICK);

        let mut chosen = None;
        ui.scope_builder(UiBuilder::new().max_rect(rect.shrink(4.0)), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut area = egui::ScrollArea::vertical()
                .id_salt(("settings-list", popup.list))
                .auto_shrink(false);
            if popup.scroll_pending {
                area = area.vertical_scroll_offset(offset);
            } else if let Some(jump) = popup.jump {
                // Typed ahead: the entry found goes to the top of the list.
                area = area.vertical_scroll_offset(jump as f32 * ITEM_HEIGHT);
            }
            area.show_rows(ui, ITEM_HEIGHT, entries.len(), |ui, range| {
                for index in range {
                    let size = vec2(ui.available_width(), ITEM_HEIGHT);
                    let (row, _) = ui.allocate_exact_size(size, Sense::hover());
                    let item = row.shrink2(vec2(0.0, 2.0));
                    let id = Id::new("settings-item").with(popup.list).with(index);
                    let (tag, name) = entries[index];
                    let focus =
                        self.take_popup_focus(index == selected) || popup.jump == Some(index);
                    if date_view::list_item(p, ui, id, item, name, index == selected, focus) {
                        chosen = Some(tag);
                    }
                }
            });
        });

        if let Some(open) = &mut self.settings_popup {
            open.scroll_pending = false;
            open.jump = None;
        }
        if chosen.is_some() || backdrop.clicked() {
            self.settings_popup = None;
        }
        chosen
    }
}

#[derive(Clone, Copy)]
enum SettingsAction {
    ToggleTheme,
    SetTheme(Theme),
    OpenUrl(&'static str),
    /// Open a drop-down list from its box.
    OpenList(SettingsList, Rect),
}

#[derive(Clone, Copy)]
enum FlyoutAction {
    RecallHistory(usize),
    RecallMemory(usize),
    MemoryAdd(usize, bool),
    MemoryClear(usize),
    ClearAll,
    OpenMenu(MenuTarget, Pos2),
    CopyHistory(usize),
    DeleteHistory(usize),
    /// Copy a value of the Programmer mode's radix list.
    CopyRadix(Radix),
    CloseMenu,
}

/// The item a context menu belongs to.
#[derive(Clone, Copy)]
enum MenuTarget {
    /// A history entry: its index, and the history's `history_key` when the menu opened.
    /// The menu closes if the history changes under it, as the index would then name
    /// another entry.
    History(usize, u64),
    Memory(usize),
    /// A row of the Programmer mode's radix list.
    Radix(Radix),
}

/// An open context menu and where the right click was.
#[derive(Clone, Copy)]
struct ContextMenu {
    target: MenuTarget,
    pos: Pos2,
}

/// A fingerprint of the history's entries, to tell when the history changed under an open
/// context menu.
fn history_key(history: &[HistoryEntry]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for entry in history {
        (&entry.expression, &entry.text).hash(&mut hasher);
    }
    hasher.finish()
}

impl eframe::App for CalculatorApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, self.theme.key().to_owned());
        // On the Settings page, the mode under it: Settings is a page, not a mode.
        storage.set_string(MODE_KEY, self.mode.serialization_id().to_string());
        storage.set_string(CONVERTER_KEY, self.converter.converter.preferences());
        for (key, choice) in [
            (LANGUAGE_KEY, &self.language_choice),
            (REGION_KEY, &self.region_choice),
        ] {
            let value = choice.as_deref().unwrap_or(SYSTEM_CHOICE);
            storage.set_string(key, value.to_owned());
        }
        storage.set_string(
            graphing_view::THEME_KEY,
            self.graphing.match_app_theme.to_string(),
        );
        self.window_sizes.save(storage);
        #[cfg(feature = "support-prompt")]
        self.support.save(storage);
    }

    /// On Android: the system bars' part of the screen as egui's safe area, and the back
    /// button, which closes what is open (as Esc does) or else leaves the app.
    #[cfg(target_os = "android")]
    fn raw_input_hook(&mut self, ctx: &Context, raw_input: &mut egui::RawInput) {
        let back_closes =
            self.popup_covers() || self.flyout != Flyout::None || self.page == Page::Settings;
        android::input_hook(ctx, raw_input, back_closes);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // egui resolves "Use system setting" and reports when Windows switches modes.
        let theme = ctx.theme();
        self.palette = palette_for(theme);
        if self.title_bar_theme != Some(theme) {
            self.title_bar.apply(theme);
            self.title_bar_theme = Some(theme);
        }
        self.docked =
            self.mode.is_calculator() && !self.compact && app_rect(ui).width() >= DOCK_WIDTH;

        // The controls under an open popup leave the Tab order, and the control a popup
        // was opened from with the keyboard gets the focus back when it closes.
        let popup_covers = self.popup_covers();
        access::set_covered(&ctx, popup_covers);
        if !popup_covers && !matches!(self.flyout, Flyout::History | Flyout::Memory) {
            if let Some(id) = self.focus_return.take() {
                ctx.memory_mut(|memory| memory.request_focus(id));
            }
            self.focus_popup = false;
        }

        self.draw_frame(ui, &ctx);
        self.last_focus = access::focused(&ctx);
        // The Settings lists load every script font (some 70 MB of CJK fonts); they go
        // again with the page.
        if self.page != Page::Settings && self.all_scripts {
            self.settings_popup = None;
            self.all_scripts = false;
            self.update_fonts(&ctx);
        }

        #[cfg(windows)]
        if let Ok(out_path) = std::env::var("CALCCRAFT_SCREENSHOT_PATH") {
            let screenshot_requested = ctx.data(|d| {
                d.get_temp::<bool>(Id::new("screenshot_requested"))
                    .unwrap_or(false)
            });
            if !screenshot_requested {
                ctx.data_mut(|d| d.insert_temp(Id::new("screenshot_requested"), true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                ctx.request_repaint();
            } else {
                let image = ctx.input(|i| {
                    i.events.iter().find_map(|e| match e {
                        egui::Event::Screenshot { image, .. } => Some(image.clone()),
                        _ => None,
                    })
                });
                if let Some(image) = image {
                    if let Ok(png_bytes) = share::png(&image) {
                        let _ = std::fs::write(&out_path, png_bytes);
                    }
                    std::process::exit(0);
                }
            }
        }
    }
}

impl CalculatorApp {
    /// Handles the keyboard and draws the page.
    fn draw_frame(&mut self, ui: &mut Ui, ctx: &Context) {
        let ctx = ctx.clone();
        #[cfg(feature = "support-prompt")]
        self.support.begin_frame(
            &ctx,
            self.page != Page::Settings && self.mode.is_calculator(),
            self.popup_covers() || self.flyout != Flyout::None,
            self.flyout == Flyout::Navigation,
        );
        self.handle_mode_keys(&ctx);
        match self.mode {
            _ if self.page == Page::Settings => self.handle_keyboard(&ctx),
            Mode::DateCalculation => self.handle_date_keyboard(&ctx),
            Mode::Converter(_) => self.handle_converter_keyboard(&ctx),
            Mode::Graphing => self.handle_graphing_keyboard(&ctx),
            Mode::Standard | Mode::Scientific | Mode::Programmer => self.handle_keyboard(&ctx),
        }
        let now = ctx.input(|i| i.time);

        // The background fills the screen, under a phone's system bars too.
        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, self.palette.background);
        let full = app_rect(ui);
        if self.page == Page::Settings {
            self.draw_settings(ui, full);
            return;
        }

        if self.mode == Mode::Graphing {
            let header = Rect::from_min_size(full.min, vec2(full.width(), HEADER_HEIGHT));
            self.draw_header(ui, header);
            self.draw_graphing_mode(ui, full, header.bottom());
            self.draw_graphing_popups(ui);
            #[cfg(feature = "support-prompt")]
            self.draw_support_popup(ui, full);
            if self.flyout == Flyout::Navigation {
                self.draw_navigation(ui, full);
            }
            return;
        }

        if !self.mode.is_calculator() {
            let header = Rect::from_min_size(full.min, vec2(full.width(), HEADER_HEIGHT));
            self.draw_header(ui, header);
            if self.mode == Mode::DateCalculation {
                self.draw_date_mode(ui, full, header.bottom());
            } else {
                self.draw_converter_mode(ui, full, header.bottom());
            }
            #[cfg(feature = "support-prompt")]
            self.draw_support_popup(ui, full);
            if self.flyout == Flyout::Navigation {
                self.draw_navigation(ui, full);
            }
            return;
        }

        // A wide window docks history and memory on the right; the calculator keeps
        // the rest. An open history or memory flyout turns into the docked tab.
        let (main, dock) = if self.docked {
            let width = (full.width() * 240.0 / DOCK_WIDTH).min(DOCK_MAX_WIDTH);
            let main = full.with_max_x(full.right() - width);
            (main, Some(full.with_min_x(main.right())))
        } else {
            (full, None)
        };
        if dock.is_some() && matches!(self.flyout, Flyout::History | Flyout::Memory) {
            self.flyout = Flyout::None;
        }

        if matches!(self.mode, Mode::Programmer | Mode::Scientific) {
            let header = Rect::from_min_size(main.min, vec2(main.width(), HEADER_HEIGHT));
            // The flyouts lie over the header too (see `draw_programmer` and
            // `draw_scientific`).
            if matches!(self.flyout, Flyout::History | Flyout::Memory) {
                access::draw_covered(ui, || self.draw_header(ui, header));
            } else {
                self.draw_header(ui, header);
            }
            let below = main.with_min_y(header.bottom());
            if self.mode == Mode::Programmer {
                self.draw_programmer(ui, below, now);
            } else {
                self.draw_scientific(ui, below, now);
            }
            if let Some(dock) = dock {
                self.draw_dock(ui, dock);
            }
            if self.mode == Mode::Programmer {
                self.draw_programmer_popups(ui);
            } else {
                self.draw_scientific_popups(ui);
            }
            #[cfg(feature = "support-prompt")]
            self.draw_support_popup(ui, full);
            if self.flyout == Flyout::Navigation {
                self.draw_navigation(ui, full);
            }
            return;
        }

        let header_height = if self.compact {
            COMPACT_HEADER_HEIGHT
        } else {
            HEADER_HEIGHT
        };
        let memory_height = if self.compact { 0.0 } else { MEMORY_ROW_HEIGHT };
        let header = Rect::from_min_size(main.min, vec2(main.width(), header_height));
        let below = main.with_min_y(header.bottom());
        let display_height = ((below.height() - memory_height) * 0.24).clamp(70.0, 220.0);
        let display = Rect::from_min_size(below.min, vec2(main.width(), display_height));
        let memory_row =
            Rect::from_min_size(display.left_bottom(), vec2(main.width(), memory_height));
        let keypad = Rect::from_min_max(
            memory_row.left_bottom() + vec2(MARGIN, GAP),
            main.max - vec2(MARGIN, MARGIN),
        );

        // An open history or memory flyout lies over the whole window, header included
        // (Windows' `Placement="Full"`), so the controls under it stay in view, out of reach.
        if matches!(self.flyout, Flyout::History | Flyout::Memory) {
            access::draw_covered(ui, || self.draw_header(ui, header));
        } else {
            self.draw_header(ui, header);
        }
        self.draw_display(ui, display);
        match self.flyout {
            Flyout::History | Flyout::Memory => {
                // Windows makes the list as tall as the keys (`RowHeight =
                // NumpadPanel.ActualHeight`): the memory row stays in view too, its buttons
                // disabled (`EnableControls(false)`), and a click anywhere above the list
                // closes it (`OnHistoryFlyOutTapped`).
                if !self.compact {
                    access::draw_covered(ui, || self.draw_memory_row(ui, memory_row, now, false));
                }
                let above = main.with_max_y(memory_row.bottom());
                if ui
                    .interact(above, Id::new("display"), Sense::CLICK)
                    .clicked()
                {
                    self.flyout = Flyout::None;
                }
                let panel = if self.flyout == Flyout::History {
                    Panel::History
                } else {
                    Panel::Memory
                };
                self.draw_flyout(ui, below.with_min_y(memory_row.bottom()), panel);
            }
            Flyout::None | Flyout::Navigation => {
                if dock.is_none() {
                    self.context_menu = None;
                }
                if !self.compact {
                    self.draw_memory_row(ui, memory_row, now, true);
                }
                self.draw_keypad(ui, keypad, now);
            }
        }
        if let Some(dock) = dock {
            self.draw_dock(ui, dock);
        }
        #[cfg(feature = "support-prompt")]
        self.draw_support_popup(ui, full);
        if self.flyout == Flyout::Navigation {
            self.draw_navigation(ui, full);
        }
    }

    #[cfg(feature = "support-prompt")]
    fn draw_support_popup(&mut self, ui: &mut Ui, full: Rect) {
        self.support.draw_overlay(ui, full, self.palette);
    }
}

// ------------------------------------------------------------------- text

/// The user's number format. The engine and the converter write numbers the en-US way;
/// this turns them into the region's format for the screen, and pasted text back.
fn numbers() -> &'static crate::locale::NumberFormat {
    &region::formats().numbers
}

pub(crate) fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// Segoe UI Light, for the converter value that isn't being typed into.
fn light() -> FontFamily {
    FontFamily::Name("light".into())
}

/// Segoe UI Symbol, for the pictures of the converters' everyday comparisons.
fn symbols() -> FontFamily {
    FontFamily::Name("symbols".into())
}

/// Text without a color: the color is picked at paint time from the button state.
fn text(text: &str, size: f32, family: FontFamily) -> LayoutJob {
    LayoutJob::simple_singleline(
        text.to_owned(),
        FontId::new(size, family),
        Color32::PLACEHOLDER,
    )
}

/// The `±` key's label, "⁺∕₋": a small plus at the top, a minus at the bottom.
fn negate_label(size: f32) -> LayoutJob {
    let regular = FontFamily::Proportional;
    let small = size * 0.85;
    let mut job = LayoutJob::default();
    job.append(
        "+",
        0.0,
        TextFormat {
            valign: Align::TOP,
            ..format(small, regular.clone())
        },
    );
    job.append("∕", 0.0, format(size, regular.clone()));
    job.append(
        "−",
        0.0,
        TextFormat {
            valign: Align::BOTTOM,
            ..format(small, regular)
        },
    );
    job
}

/// Format of one piece of a label; vertically centered by default.
fn format(size: f32, family: FontFamily) -> TextFormat {
    TextFormat {
        font_id: FontId::new(size, family),
        color: Color32::PLACEHOLDER,
        valign: Align::Center,
        ..Default::default()
    }
}

/// If the text doesn't fit, cuts it from the left: `…+ sqr(5)`.
fn layout_elided_left(painter: &Painter, text: &str, font: FontId, max_width: f32) -> Arc<Galley> {
    let mut galley = painter.layout_no_wrap(text.to_owned(), font.clone(), Color32::PLACEHOLDER);
    let mut start = 0;
    while galley.size().x > max_width {
        let Some((offset, _)) = text[start..].char_indices().nth(1) else {
            break;
        };
        start += offset;
        galley = painter.layout_no_wrap(
            format!("…{}", &text[start..]),
            font.clone(),
            Color32::PLACEHOLDER,
        );
    }
    galley
}

/// Text that wraps at `max_width` (a `number` anywhere in it), with its lines aligned to
/// the right, like the history and memory lists' `TextBlock`s.
fn layout_wrapped_right(
    painter: &Painter,
    text: &str,
    font: FontId,
    max_width: f32,
    number: bool,
) -> Arc<Galley> {
    let mut job = LayoutJob::simple(text.to_owned(), font, Color32::PLACEHOLDER, max_width);
    job.halign = Align::RIGHT;
    // A number fills its lines and breaks anywhere, not after its decimal point.
    job.wrap.break_anywhere = number;
    painter.layout_job(job)
}

/// Paints a galley so that its right edge is at `right_top.x` and its top at `right_top.y`.
fn paint_right(painter: &Painter, galley: Arc<Galley>, right_top: Pos2, color: Color32) {
    let rect = galley.rect;
    let pos = pos2(right_top.x - rect.max.x, right_top.y - rect.min.y);
    painter.galley(pos, galley, color);
}

/// The bounding rectangle of the characters in `range`, relative to the galley.
fn char_range_rect(galley: &Galley, range: std::ops::Range<usize>) -> Rect {
    // Every character, spaces included, is one glyph in the rows.
    let glyphs = galley.rows.iter().flat_map(|row| {
        row.glyphs
            .iter()
            .map(|glyph| glyph.logical_rect().translate(row.pos.to_vec2()))
    });
    glyphs
        .enumerate()
        .filter(|(i, _)| range.contains(i))
        .fold(Rect::NOTHING, |rect, (_, glyph)| rect.union(glyph))
}

// ------------------------------------------------------- settings controls

/// Section heading on the Settings page, vertically centered on `left_center`.
fn section_label(p: &Palette, painter: &Painter, left_center: Pos2, label: &str) {
    painter.text(
        left_center,
        Align2::LEFT_CENTER,
        label,
        FontId::new(14.0, semibold()),
        p.text,
    );
}

/// WinUI-style radio button: a 20 pt circle whose left edge is at `left_center`, then the label.
/// Returns `true` if it was clicked.
fn radio_button(
    p: &Palette,
    ui: &Ui,
    left_center: Pos2,
    label: &str,
    checked: bool,
    id: Id,
) -> bool {
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(label.to_owned(), FontId::proportional(14.0), p.text);
    let text_pos = pos2(left_center.x + 28.0, left_center.y - galley.size().y / 2.0);
    let rect = Rect::from_min_max(
        pos2(left_center.x, left_center.y - 16.0),
        pos2(text_pos.x + galley.size().x, left_center.y + 16.0),
    );
    let info = access::item(WidgetType::RadioButton, label, checked);
    let response = access::control(ui, p, rect, id, true, RADIUS, info);
    let pressed = response.is_pointer_button_down_on();
    let hovered = response.hovered();

    let center = pos2(left_center.x + 10.0, left_center.y);
    paint_radio(p, painter, center, checked, hovered, pressed);
    painter.galley(text_pos, galley, p.text);
    response.clicked()
}

/// A WinUI radio button's 20 pt circle around `center`.
fn paint_radio(
    p: &Palette,
    painter: &Painter,
    center: Pos2,
    checked: bool,
    hovered: bool,
    pressed: bool,
) {
    if checked || pressed {
        let fill = if pressed {
            p.accent_pressed
        } else if hovered {
            p.accent_hover
        } else {
            p.accent
        };
        // The dot grows on hover and shrinks while pressed, as in WinUI.
        let dot = if pressed {
            5.0
        } else if hovered {
            7.0
        } else {
            6.0
        };
        painter.circle_filled(center, 10.0, fill);
        painter.circle_filled(center, dot, p.radio_dot);
    } else {
        let fill = if hovered {
            p.subtle_hover
        } else {
            p.radio_fill
        };
        painter.circle(center, 9.5, fill, Stroke::new(1.0, p.radio_ring));
    }
}

/// A text link with a subtle background on hover, like a WinUI hyperlink button.
/// The text starts at `left_center`. Returns `true` if it was clicked.
fn link_button(p: &Palette, ui: &Ui, left_center: Pos2, label: &str, id: Id) -> bool {
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(
        label.to_owned(),
        FontId::proportional(14.0),
        Color32::PLACEHOLDER,
    );
    let text_pos = pos2(left_center.x, left_center.y - galley.size().y / 2.0);
    let rect = Rect::from_min_size(text_pos, galley.size()).expand2(vec2(4.0, 2.0));
    let info = || WidgetInfo::labeled(WidgetType::Link, true, label);
    let response = access::control(ui, p, rect, id, true, RADIUS, info)
        .on_hover_cursor(CursorIcon::PointingHand);
    let (fill, color) = if response.is_pointer_button_down_on() {
        (p.subtle_pressed, p.link_pressed)
    } else if response.hovered() {
        (p.subtle_hover, p.link_hover)
    } else {
        (Color32::TRANSPARENT, p.link)
    };
    painter.rect_filled(rect, RADIUS, fill);
    painter.galley(text_pos, galley, color);
    response.clicked()
}

/// Repaints until the key that a key press highlights is drawn plain again, then forgets
/// the press, so an idle window doesn't repaint.
fn repaint_for_flash<C>(ctx: &Context, flash: &mut Option<(C, f64)>, now: f64) {
    let Some(at) = flash.as_ref().map(|&(_, at)| at) else {
        return;
    };
    let left = at + KEY_FLASH - now;
    if left > 0.0 {
        ctx.request_repaint_after(Duration::from_secs_f64(left));
    } else {
        *flash = None;
    }
}

/// Removes the mode shortcuts (`Alt` + digit, `Ctrl+E`) from `events` and returns the
/// mode the first one opens. `Ctrl+Alt` is `AltGr`, which types characters, so it's left
/// alone. Text that comes with any `Alt` chord goes too, but on a Mac only the mode keys'.
fn take_mode_shortcut(events: &mut Vec<Event>) -> Option<Mode> {
    let mut chosen = None;
    let mut consumed = false;
    events.retain(|event| match event {
        Event::Key {
            key,
            pressed,
            modifiers,
            ..
        } if modifiers.alt && !modifiers.ctrl => {
            // In Windows an Alt chord is a system key and types nothing, but egui-winit
            // still sends the key's text along. A Mac's Option key types characters (`@` is
            // Option+L on a German keyboard), so there only the mode keys type nothing.
            let mode = Mode::for_alt_key(*key);
            consumed |= *pressed && (mode.is_some() || !cfg!(target_os = "macos"));
            match mode {
                Some(mode) => {
                    chosen = chosen.or(pressed.then_some(mode));
                    false
                }
                None => true,
            }
        }
        Event::Key {
            key: Key::E,
            pressed,
            modifiers,
            ..
        } if modifiers.command_only() => {
            chosen = chosen.or(pressed.then_some(Mode::DateCalculation));
            false
        }
        Event::Text(_) if consumed => false,
        _ => true,
    });
    chosen
}

/// Removes the key events of letter keys pressed with `AltGr`, so that only the character it
/// typed runs its shortcut: `AltGr+Q` types `@` on a German keyboard, which is √x, and the Q
/// key's x² mustn't run too. Windows sees `AltGr` as `Ctrl+Alt`, which has no key shortcuts
/// (`GetCurrentKeyDictionary`), but winit reports these keys without modifiers. (`Ctrl+Alt`
/// typed with the left keys arrives with both modifiers, and goes to `handle_key`.)
///
/// `AltGr` is the right `Alt` key, whose presses and releases come in order with the other
/// keys, so a key pressed before it in the same frame stays; `alt_gr` is whether it's down
/// after the frame's events (egui's `key_down`). A letter key that typed only other characters
/// than its letter counts as pressed with `AltGr` too, for where that isn't the right `Alt`
/// key: `@`, `µ`, `€`. Its letter with diacritics is still its letter, so a dead key's letter
/// keeps its key (`´` then E types `é`, which Windows sees as `VK_E`, and `´` then Q, which
/// can't combine, types `´q`), and so does a letter of another script (a Russian keyboard types
/// `й` on the Q key, `VK_Q` in Windows).
fn remove_alt_gr_keys(events: &mut Vec<Event>, alt_gr: bool) {
    // Whether AltGr is down before the frame's first event: the opposite of its first change
    // in the frame (a press that repeats is no change), or how it ends the frame.
    let mut held = events
        .iter()
        .find_map(|event| match event {
            Event::Key {
                key: Key::AltRight,
                pressed,
                repeat,
                ..
            } => Some(!pressed || *repeat),
            _ => None,
        })
        .unwrap_or(alt_gr);
    let mut i = 0;
    while i < events.len() {
        let with_alt_gr = match &events[i] {
            Event::Key {
                key: Key::AltRight,
                pressed,
                ..
            } => {
                held = *pressed;
                false
            }
            // egui lets go of every key then.
            Event::WindowFocused(false) => {
                held = false;
                false
            }
            Event::Key {
                key, pressed: true, ..
            } => key_letter(*key).is_some_and(|letter| {
                let other = |c: char| {
                    !base_letter(c).to_uppercase().eq([letter]) && !is_non_latin_letter(c)
                };
                // The text a key typed comes right after it.
                let typed_other = match events.get(i + 1) {
                    Some(Event::Text(text)) => !text.is_empty() && text.chars().all(other),
                    _ => false,
                };
                held || typed_other
            }),
            _ => false,
        };
        if with_alt_gr {
            events.remove(i);
        } else {
            i += 1;
        }
    }
}

/// The letter that a letter of Latin-1 or Latin Extended-A is written with, without its
/// diacritics (`é` is `e`, `Ś` `S`: their canonical decompositions), or `c` itself.
fn base_letter(c: char) -> char {
    // From U+00C0 and from U+0100 on, a space where a letter has no decomposition (`ß`, `ł`).
    const LATIN_1: &[u8; 64] = b"AAAAAA CEEEEIIII NOOOOO  UUUUY  aaaaaa ceeeeiiii nooooo  uuuuy y";
    const EXTENDED_A: &[u8; 128] = b"AaAaAaCcCcCcCcDd  EeEeEeEeEeGgGgGgGgHh  IiIiIiIiI   JjKk LlLlLl    NnNnNn   OoOoOo  RrRrRrSsSsSsSsTtTt  UuUuUuUuUuUuWwYyYZzZzZz ";
    let base = match u32::from(c) {
        code @ 0xC0..=0xFF => LATIN_1[code as usize - 0xC0],
        code @ 0x100..=0x17F => EXTENDED_A[code as usize - 0x100],
        _ => b' ',
    };
    if base == b' ' { c } else { char::from(base) }
}

/// The keypad's keys and the memory row are Windows' `CalculatorButton`s, which ignore
/// `Enter` so that it always means `=`.
fn is_calculator_key(id: Id) -> bool {
    (0..24).any(|i| id == Id::new("key").with(i))
        || (0..5).any(|i| id == Id::new("memory").with(i))
        || (0..30).any(|i| id == programmer_view::key_id(i))
        || id == programmer_view::memory_store_id()
        // The Scientific keys but `2nd`, a toggle button.
        || (1..scientific_view::KEY_COUNT).any(|i| id == scientific_view::key_id(i))
}

/// The name a screen reader gives a key (Windows' `AutomationProperties.Name`).
fn command_name(command: Command) -> &'static str {
    // The trigonometry keys are named `sinButton`, `invsinButton`, `sinhButton`,
    // `invsinhButton` and so on.
    if let Command::Function(Func::Trig(trig)) = command {
        let inverse = if trig.inverse { "inv" } else { "" };
        let hyperbolic = if trig.hyperbolic { "h" } else { "" };
        let label = trig.function.label();
        return tr(&format!(
            "{inverse}{label}{hyperbolic}Button.AutomationName"
        ));
    }
    tr(match command {
        Command::Digit(0) => "num0Button.AutomationName",
        Command::Digit(1) => "num1Button.AutomationName",
        Command::Digit(2) => "num2Button.AutomationName",
        Command::Digit(3) => "num3Button.AutomationName",
        Command::Digit(4) => "num4Button.AutomationName",
        Command::Digit(5) => "num5Button.AutomationName",
        Command::Digit(6) => "num6Button.AutomationName",
        Command::Digit(7) => "num7Button.AutomationName",
        Command::Digit(8) => "num8Button.AutomationName",
        Command::Digit(9) => "num9Button.AutomationName",
        Command::Digit(10) => "aButton.AutomationName",
        Command::Digit(11) => "bButton.AutomationName",
        Command::Digit(12) => "cButton.AutomationName",
        Command::Digit(13) => "dButton.AutomationName",
        Command::Digit(14) => "eButton.AutomationName",
        Command::Digit(_) => "fButton.AutomationName",
        Command::Decimal => "decimalSeparatorButton.AutomationName",
        Command::Operator(Op::Add) => "plusButton.AutomationName",
        Command::Operator(Op::Subtract) => "minusButton.AutomationName",
        Command::Operator(Op::Multiply) => "multiplyButton.AutomationName",
        Command::Operator(Op::Divide) => "divideButton.AutomationName",
        Command::Operator(Op::Mod) => "modButton.AutomationName",
        Command::Operator(Op::And) => "andButton.AutomationName",
        Command::Operator(Op::Or) => "orButton.AutomationName",
        Command::Operator(Op::Xor) => "xorButton.AutomationName",
        Command::Operator(Op::Nand) => "nandButton.AutomationName",
        Command::Operator(Op::Nor) => "norButton.AutomationName",
        Command::Operator(Op::LeftShift) => "lshButton.AutomationName",
        Command::Operator(Op::RightShift) => "rshButton.AutomationName",
        Command::Operator(Op::RightShiftLogical) => "rshLogicalButton.AutomationName",
        Command::Function(Func::Not) => "notButton.AutomationName",
        Command::Function(Func::RotateLeft) => "rolButton.AutomationName",
        Command::Function(Func::RotateRight) => "rorButton.AutomationName",
        Command::Function(Func::RotateLeftCarry) => "rolCarryButton.AutomationName",
        Command::Function(Func::RotateRightCarry) => "rorCarryButton.AutomationName",
        Command::OpenParen => "openParenthesisButton.AutomationName",
        Command::CloseParen => "closeParenthesisButton.AutomationName",
        Command::Equals => "equalButton.AutomationName",
        Command::Percent => "percentButton.AutomationName",
        Command::ClearEntry => "clearEntryButton.AutomationName",
        Command::Clear => "clearButton.AutomationName",
        Command::Backspace => "backSpaceButton.AutomationName",
        Command::Function(Func::Reciprocal) => "invertButton.AutomationName",
        Command::Function(Func::Square) => "xpower2Button.AutomationName",
        Command::Function(Func::SquareRoot) => "squareRootButton.AutomationName",
        Command::Function(Func::Negate) => "negateButton.AutomationName",
        Command::Function(Func::Cube) => "xpower3Button.AutomationName",
        Command::Function(Func::CubeRoot) => "cubeRootButton.AutomationName",
        Command::Function(Func::TenToX) => "powerOf10Button.AutomationName",
        Command::Function(Func::TwoToX) => "twoPowerXButton.AutomationName",
        Command::Function(Func::EToX) => "powerOfEButton.AutomationName",
        Command::Function(Func::Log) => "logBase10Button.AutomationName",
        Command::Function(Func::Ln) => "logBaseEButton.AutomationName",
        Command::Function(Func::Factorial) => "factorialButton.AutomationName",
        Command::Function(Func::Abs) => "absButton.AutomationName",
        Command::Function(Func::Floor) => "floorButton.AutomationName",
        Command::Function(Func::Ceil) => "ceilButton.AutomationName",
        Command::Function(Func::Dms) => "dmsButton.AutomationName",
        Command::Function(Func::Degrees) => "degreesButton.AutomationName",
        Command::Function(Func::Trig(_)) => unreachable!("named above"),
        Command::Operator(Op::Power) => "powerButton.AutomationName",
        Command::Operator(Op::Root) => "ySquareRootButton.AutomationName",
        Command::Operator(Op::LogBase) => "logBaseY.AutomationName",
        Command::Pi => "piButton.AutomationName",
        Command::Euler => "eulerButton.AutomationName",
        Command::Random => "randButton.AutomationName",
        Command::Exponent => "expButton.AutomationName",
        Command::ToggleNotation => "ftoeButton.AutomationName",
        Command::Angle(Angle::Degrees) => "degButton.AutomationName",
        Command::Angle(Angle::Radians) => "radButton.AutomationName",
        Command::Angle(Angle::Gradians) => "gradButton.AutomationName",
        Command::MemoryClear => "ClearMemoryButton.AutomationName",
        Command::MemoryRecall => "MemRecall.AutomationName",
        Command::MemoryAdd => "MemPlus.AutomationName",
        Command::MemorySubtract => "MemMinus.AutomationName",
        Command::MemoryStore => "memButton.AutomationName",
        // The bits, the radix list and the word size button have names of their own.
        Command::ToggleBit(_) | Command::Radix(_) | Command::WordSize(_) => return "",
    })
}

/// Keys that arrive as text: digits and operators, as in Windows.
fn char_command(c: char) -> Option<Command> {
    Some(match c {
        '0'..='9' => Command::Digit(c as u8 - b'0'),
        '.' | ',' => Command::Decimal,
        '+' => Command::Operator(Op::Add),
        '-' => Command::Operator(Op::Subtract),
        '*' => Command::Operator(Op::Multiply),
        '/' => Command::Operator(Op::Divide),
        '=' => Command::Equals,
        '%' => Command::Percent,
        '@' => Command::Function(Func::SquareRoot),
        'r' | 'R' => Command::Function(Func::Reciprocal),
        'q' | 'Q' => Command::Function(Func::Square),
        _ => return None,
    })
}

/// Gives the letters that a layout without Latin letters types (Russian, Greek, …) the
/// letter of their key, which is what Windows' letter shortcuts see: they're virtual keys
/// (`R`, `Q`, `A`–`F`), and the R key of a Russian keyboard is `VK_R` although it types `к`.
/// egui reports such a key by its place on a US keyboard (`Key::R`), right before its text.
/// Where such a layout types a symbol on a letter key (`;` on the Greek Q key), egui names the
/// key after the symbol, and `layout_letter` tells the letter of its virtual key (Windows
/// only) if the key types the symbol without AltGr: the key becomes that letter's, and so does
/// its text, if any (there's none while Ctrl is held, and the Greek `Ctrl+Q` is `M−`). Windows
/// has no letter shortcuts with Alt or AltGr (`Ctrl+Alt`), so a key pressed with Alt stays as
/// it is, and so does a Czech keyboard's `AltGr+Q` (`\`), which winit reports without
/// modifiers. With `Shift` only the Programmer mode's letters are shortcuts (`Shift+A` … `F`,
/// `shift_letters`): Windows has no `Shift` chords for the Standard mode's `R` and `Q`, so a
/// Russian `Shift+R` (`К`) stays as typed there.
fn latin_letters(
    events: &mut [Event],
    layout_letter: impl Fn(Key, char) -> Option<char>,
    shift_letters: bool,
) {
    for i in 0..events.len() {
        // The text a key typed comes right after it, and there's none while Ctrl is held.
        let (keys, rest) = events.split_at_mut(i + 1);
        let Event::Key {
            key,
            physical_key,
            pressed: true,
            modifiers,
            ..
        } = &mut keys[i]
        else {
            continue;
        };
        let text = match rest.first_mut() {
            Some(Event::Text(text)) => Some(text),
            _ => None,
        };
        // Latin letters stay as typed, as any text of a layout with Latin letters does.
        let latin = text.as_deref().is_some_and(|text| {
            text.is_empty()
                || text
                    .chars()
                    .any(|c| c.is_alphabetic() && !is_non_latin_letter(c))
        });
        if latin || modifiers.alt || (modifiers.shift && !shift_letters) {
            continue;
        }
        let other_script = text
            .as_deref()
            .is_some_and(|text| text.chars().all(is_non_latin_letter));
        let letter = match key_letter(*key) {
            Some(letter) if other_script => letter,
            // A symbol typed on a letter key (`@` with AltGr) is the symbol.
            Some(_) => continue,
            // The symbol typed, or the key's own while Ctrl is held.
            None => {
                let symbol = text.as_deref().map_or(key.symbol_or_name(), String::as_str);
                let mut chars = symbol.chars();
                let (Some(physical), Some(typed), None) =
                    (*physical_key, chars.next(), chars.next())
                else {
                    continue;
                };
                let Some(letter) = layout_letter(physical, typed) else {
                    continue;
                };
                letter
            }
        };
        if let Some(letter_key) = Key::from_name(&letter.to_string()) {
            *key = letter_key;
        }
        if let Some(text) = text {
            *text = letter.to_string();
        }
    }
}

/// The letter of the virtual key that the keyboard layout puts at a letter key's place, if
/// that key types `typed` without AltGr, which is what Windows' letter shortcuts see: a Greek
/// keyboard's Q key types `;` and is `VK_Q`, a Czech keyboard's Q key types `\` only with
/// AltGr, and a French keyboard's key at the US M key's place types `,` and is
/// `VK_OEM_COMMA`. Only Windows tells.
#[cfg(windows)]
fn layout_letter(physical: Key, typed: char) -> Option<char> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyboardLayout, MAPVK_VSC_TO_VK, MapVirtualKeyExW, VkKeyScanExW,
    };
    let scan_code = us_scan_code(physical)?;
    let typed = u16::try_from(u32::from(typed)).ok()?;
    // SAFETY: these only read the keyboard layout of the calling thread, which runs the UI.
    let (vk, typed) = unsafe {
        let layout = GetKeyboardLayout(0);
        (
            MapVirtualKeyExW(scan_code, MAPVK_VSC_TO_VK, layout),
            VkKeyScanExW(typed, layout),
        )
    };
    vk_letter(vk, typed)
}

#[cfg(not(windows))]
fn layout_letter(_physical: Key, _typed: char) -> Option<char> {
    None
}

/// The letter of a letter key's virtual key `vk` (the letters' virtual keys are their ASCII
/// codes), if `typed` is that key without Ctrl and Alt: `typed` is what `VkKeyScanExW` gives
/// for the typed character, the virtual key that types it and then the Shift (1), Ctrl (2) and
/// Alt (4) it takes. AltGr is Ctrl+Alt there.
#[cfg_attr(not(windows), allow(dead_code))]
fn vk_letter(vk: u32, typed: i16) -> Option<char> {
    let [typed_vk, shift_state] = typed.to_le_bytes();
    if u32::from(typed_vk) != vk || shift_state & 6 != 0 {
        return None;
    }
    char::from_u32(vk).filter(char::is_ascii_uppercase)
}

/// The scan code (set 1) of a letter key, by its place on a US keyboard.
#[cfg_attr(not(windows), allow(dead_code))]
fn us_scan_code(physical: Key) -> Option<u32> {
    let letter = key_letter(physical)?;
    // The letter rows of a US keyboard and the scan codes they start at.
    [(0x10, "QWERTYUIOP"), (0x1E, "ASDFGHJKL"), (0x2C, "ZXCVBNM")]
        .into_iter()
        .find_map(|(first, row)| Some(first + row.find(letter)? as u32))
}

/// The letter of a letter key (`Key::R` is `R`).
fn key_letter(key: Key) -> Option<char> {
    let mut name = key.name().chars();
    match (name.next(), name.next()) {
        (Some(letter @ 'A'..='Z'), None) => Some(letter),
        _ => None,
    }
}

/// Whether `c` is a letter of another script than Latin (Cyrillic, Greek, Hebrew, …).
fn is_non_latin_letter(c: char) -> bool {
    // The Latin letters are in the blocks before Greek (Basic Latin to the combining
    // accents) and in Latin Extended Additional.
    c.is_alphabetic() && !matches!(c, '\0'..='\u{36F}' | '\u{1E00}'..='\u{1EFF}')
}

/// The script fonts that the interface language and the regional format need, the
/// language's first (its glyphs win for characters that both have, like Chinese and
/// Japanese ideographs), or all of them.
fn needed_fonts(language: &str, region: &str, all: bool) -> Vec<strings::ScriptFont> {
    let mut fonts: Vec<strings::ScriptFont> =
        [strings::script_font(language), strings::script_font(region)]
            .into_iter()
            .flatten()
            .collect();
    if all {
        fonts.extend(strings::LIST_FONTS);
    }
    let mut seen = Vec::new();
    fonts.retain(|font| {
        let new = !seen.contains(font);
        seen.push(*font);
        new
    });
    fonts
}

/// Inter, the text font where Segoe UI is missing (on other systems, or a Windows without
/// it). It's a variable font, so one file has every weight. SIL Open Font License, see
/// `assets/fonts/README.md`.
const INTER: &[u8] = include_bytes!("../assets/fonts/Inter.ttf");

/// Noto Sans Math, the web version's and the Android app's font for the mathematical symbols
/// that Segoe UI Symbol has on Windows and Inter lacks (the analysis' `ℝ ℤ ∖ ∈ ∪ ∀`, the
/// Scientific mode's `⌊ ⌋ ⌈ ⌉ ∛`): a browser page can't read the system's fonts, and Android
/// has no Segoe UI. SIL Open Font License, see `assets/fonts/README.md`.
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
const NOTO_SANS_MATH: &[u8] = include_bytes!("../assets/fonts/NotoSansMath-Regular.ttf");

/// Loads the fonts: Segoe UI in its regular, semibold and light weights (or Inter at the same
/// weights where a Segoe UI file is missing), Segoe UI Symbol, the embedded icon font, and the
/// fonts for scripts that Segoe UI doesn't have (files in the Fonts folder and their index in
/// a collection).
pub(crate) fn setup_fonts(ctx: &Context, script_fonts: &[strings::ScriptFont]) {
    let dir =
        std::path::PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()))
            .join("Fonts");
    let load = |file: &str| std::fs::read(dir.join(file)).ok();
    let segoe_or_inter = |file: &str, weight: f32| match load(file) {
        Some(data) => egui::FontData::from_owned(data),
        None => egui::FontData::from_static(INTER).tweak(egui::FontTweak {
            coords: egui::epaint::text::VariationCoords::new([("wght", weight)]),
            ..Default::default()
        }),
    };

    let mut fonts = egui::FontDefinitions::default();
    let mut add = |name: &str, data: egui::FontData| {
        fonts.font_data.insert(name.to_owned(), Arc::new(data));
        name.to_owned()
    };
    let regular = add("regular", segoe_or_inter("segoeui.ttf", 400.0));
    let semibold_font = add("semibold", segoe_or_inter("seguisb.ttf", 600.0));
    let light_font = add("light", segoe_or_inter("segoeuil.ttf", 300.0));
    let symbol_font = load("seguisym.ttf").map(|d| add("symbols", egui::FontData::from_owned(d)));
    let icon_font = add("icons", egui::FontData::from_static(icons::FONT));
    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    let math_font = add("math", egui::FontData::from_static(NOTO_SANS_MATH));
    let scripts: Vec<String> = script_fonts
        .iter()
        .filter_map(|&(file, index)| {
            let mut font = egui::FontData::from_owned(load(file)?);
            font.index = index;
            Some(add(&format!("script-{file}"), font))
        })
        .collect();

    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, regular);
    // Right after Segoe UI, so the scripts' characters come from them and not from egui's
    // own fallback fonts.
    for (i, name) in scripts.into_iter().enumerate() {
        proportional.insert(1 + i, name);
    }
    // The web version's and the Android app's math font (`NOTO_SANS_MATH`), then egui's Hack:
    // without Segoe UI Symbol (Linux and macOS) Hack still has the analysis' `∈ ∪ ∀` and the
    // Scientific mode's `⌊ ⌋ ⌈ ⌉ ∛`, which Inter lacks.
    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    proportional.push(math_font);
    if fonts.font_data.contains_key("Hack") {
        proportional.push("Hack".to_owned());
    }
    let fallback = proportional.clone();
    let family =
        |first: Option<String>| first.into_iter().chain(fallback.iter().cloned()).collect();
    fonts
        .families
        .insert(semibold(), family(Some(semibold_font)));
    fonts.families.insert(light(), family(Some(light_font)));
    fonts.families.insert(symbols(), family(symbol_font));
    fonts
        .families
        .insert(icons::family(), family(Some(icon_font)));

    ctx.set_fonts(fonts);
}

/// The system title bar. Like the Windows 11 calculator, the app tints it with
/// the background color and switches its buttons between dark and light.
struct TitleBar {
    /// The native window handle (an `HWND`), if there is one.
    #[cfg(windows)]
    hwnd: Option<isize>,
}

impl TitleBar {
    #[cfg(windows)]
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let hwnd = cc
            .window_handle()
            .ok()
            .and_then(|handle| match handle.as_raw() {
                RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
                _ => None,
            });
        Self { hwnd }
    }

    #[cfg(not(windows))]
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {}
    }

    /// The window's `HWND`, for the share sheet.
    fn hwnd(&self) -> Option<isize> {
        #[cfg(windows)]
        return self.hwnd;
        #[cfg(not(windows))]
        None
    }

    #[cfg(windows)]
    fn apply(&self, theme: egui::Theme) {
        use windows_sys::Win32::Graphics::Dwm::{
            DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
        };

        let Some(hwnd) = self.hwnd else {
            return;
        };
        let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
        let dark_mode = u32::from(theme == egui::Theme::Dark);
        // COLORREF stores the color as 0x00BBGGRR.
        let [r, g, b, _] = palette_for(theme).background.to_array();
        let caption = u32::from_le_bytes([r, g, b, 0]);
        // SAFETY: hwnd is eframe's window, which lives as long as the app; the pointers point
        // to local u32 values, the size both attributes expect.
        unsafe {
            let set = |attribute: i32, value: &u32| {
                DwmSetWindowAttribute(hwnd, attribute as u32, (value as *const u32).cast(), 4);
            };
            set(DWMWA_USE_IMMERSIVE_DARK_MODE, &dark_mode);
            set(DWMWA_CAPTION_COLOR, &caption);
        }
    }

    #[cfg(not(windows))]
    fn apply(&self, _theme: egui::Theme) {}
}

/// The window icon, drawn by `icon.rs` (the executable embeds the same picture).
pub fn window_icon() -> egui::IconData {
    const SIZE: u32 = 64;
    egui::IconData {
        rgba: crate::icon::rgba(SIZE),
        width: SIZE,
        height: SIZE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::accesskit::{Action, NodeId, Role};

    const GRAPH_WINDOW: egui::Vec2 = vec2(1000.0, 700.0);

    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn mode_shortcuts() {
        let mut events = vec![key(Key::Num5, Modifiers::ALT), Event::Text("5".into())];
        assert_eq!(take_mode_shortcut(&mut events), Some(Mode::DateCalculation));
        assert!(events.is_empty(), "the digit isn't typed");

        let mut events = vec![key(Key::Num1, Modifiers::ALT)];
        assert_eq!(take_mode_shortcut(&mut events), Some(Mode::Standard));

        let mut events = vec![key(Key::E, Modifiers::COMMAND)];
        assert_eq!(take_mode_shortcut(&mut events), Some(Mode::DateCalculation));

        let mut events = vec![key(Key::Num4, Modifiers::ALT)];
        assert_eq!(take_mode_shortcut(&mut events), Some(Mode::Programmer));

        let mut events = vec![key(Key::Num2, Modifiers::ALT)];
        assert_eq!(take_mode_shortcut(&mut events), Some(Mode::Scientific));

        let mut events = vec![key(Key::Num3, Modifiers::ALT)];
        assert_eq!(take_mode_shortcut(&mut events), Some(Mode::Graphing));
        assert!(events.is_empty());

        // Alt+6 is no mode, and types nothing either (Option+6 types on a Mac).
        let mut events = vec![key(Key::Num6, Modifiers::ALT), Event::Text("6".into())];
        assert_eq!(take_mode_shortcut(&mut events), None);
        assert_eq!(events.len(), if cfg!(target_os = "macos") { 2 } else { 1 });
        assert!(matches!(events[0], Event::Key { .. }));
    }

    #[test]
    fn mode_shortcuts_leave_other_keys() {
        // AltGr arrives as Ctrl+Alt and types characters.
        let alt_gr = Modifiers::ALT | Modifiers::CTRL;
        let mut events = vec![key(Key::Num5, alt_gr), Event::Text("5".into())];
        assert_eq!(take_mode_shortcut(&mut events), None);
        assert_eq!(events.len(), 2);

        let mut events = vec![key(Key::E, Modifiers::COMMAND | Modifiers::SHIFT)];
        assert_eq!(take_mode_shortcut(&mut events), None);

        let mut events = vec![key(Key::Num5, Modifiers::NONE), Event::Text("5".into())];
        assert_eq!(take_mode_shortcut(&mut events), None);
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn option_chords_type_on_a_mac() {
        // Option types characters on a Mac: Option+L is `@` (√x) and Option+5 `[` (⌊x⌋) on a
        // German keyboard, where egui reports the latter as the `[` key. In Windows an Alt
        // chord types nothing.
        for (pressed, text) in [(Key::L, "@"), (Key::OpenBracket, "[")] {
            let mut events = vec![key(pressed, Modifiers::ALT), Event::Text(text.into())];
            assert_eq!(take_mode_shortcut(&mut events), None);
            let typed = events.contains(&Event::Text(text.into()));
            assert_eq!(typed, cfg!(target_os = "macos"), "{pressed:?} {text}");
        }
        // The mode keys never type: Option+1 is `¡` on a US Mac keyboard.
        let mut events = vec![key(Key::Num1, Modifiers::ALT), Event::Text("¡".into())];
        assert_eq!(take_mode_shortcut(&mut events), Some(Mode::Standard));
        assert!(events.is_empty());
    }

    /// What eframe saves in `app.ron`, in memory.
    #[derive(Default)]
    struct MemoryStorage(std::collections::BTreeMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }

        fn flush(&mut self) {}
    }

    /// The app as eframe starts it in `ctx`. In English, which other tests expect of `tr`,
    /// whatever the Windows display language.
    fn start_in(ctx: &Context) -> CalculatorApp {
        let mut storage = MemoryStorage::default();
        storage.0.insert(LANGUAGE_KEY.into(), "en-US".into());
        let mut cc = eframe::CreationContext::_new_kittest(ctx.clone());
        cc.storage = Some(&storage);
        CalculatorApp::new(&cc)
    }

    /// Runs a frame of `app` in a window of `size` with `events`, as eframe does.
    fn run(ctx: &Context, app: &mut CalculatorApp, size: egui::Vec2, events: Vec<Event>) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        let mut frame = eframe::Frame::_new_kittest();
        let mut output = ctx.run_ui(input, |ui| eframe::App::ui(app, ui, &mut frame));
        // There's no renderer to take the textures.
        output.textures_delta.clear();
    }

    /// Clicks at `pos`: the pointer moves there, presses and releases, a frame each.
    fn click(ctx: &Context, app: &mut CalculatorApp, size: egui::Vec2, pos: Pos2) {
        let button = |pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        for event in [Event::PointerMoved(pos), button(true), button(false)] {
            run(ctx, app, size, vec![event]);
        }
    }

    /// A screen reader's click on the control `id`, as Narrator sends it (AccessKit's
    /// `Click` action), which egui takes for a real click.
    fn screen_reader_click(id: Id) -> Event {
        Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Click,
            target_node: id.accesskit_id(),
            target_tree: egui::accesskit::TreeId::ROOT,
            data: None,
        })
    }

    #[test]
    fn flyouts_cover_only_the_keys() {
        for mode in [Mode::Standard, Mode::Programmer] {
            // MS, and the Programmer mode's radix list (its BIN row).
            let ms = match mode {
                Mode::Programmer => programmer_view::memory_store_id(),
                _ => Id::new("memory").with(4),
            };
            let mut ids = vec![ms];
            if mode == Mode::Programmer {
                ids.push(Id::new("radix").with(3));
            }
            let ctx = Context::default();
            let mut app = start_in(&ctx);
            app.set_mode(mode);
            app.engine_mut().execute(Command::MemoryStore);
            // The smallest window, Windows' 320 × 500.
            run(&ctx, &mut app, MIN_SIZE, vec![]);
            let rect = |id| ctx.read_response(id).map(|response| response.rect);
            let rows: Vec<_> = ids.into_iter().map(|id| (id, rect(id).unwrap())).collect();
            app.flyout = Flyout::Memory;
            run(&ctx, &mut app, MIN_SIZE, vec![]);
            let flyout = rect(Id::new("flyout")).unwrap();
            for (id, row) in rows {
                assert!(
                    row.bottom() <= flyout.top(),
                    "{mode:?}: the flyout from y = {} covers {row:?}",
                    flyout.top()
                );
                // Still drawn, out of the Tab order under the flyout.
                let covered = ctx.read_response(id.with("covered"));
                assert_eq!(covered.as_ref().map(|r| r.rect), Some(row), "{mode:?}");
                // MS is disabled, like the keys (`EnableControls(false)`).
                if id == ms {
                    assert!(!covered.unwrap().sense.senses_click(), "{mode:?}");
                }
            }
        }
    }

    #[test]
    fn flyouts_lie_over_the_header() {
        // The header's buttons: the menu, and in the Standard mode keep on top and history.
        let standard = ["menu", "keep-on-top", "history"];
        let cases = [
            (Mode::Standard, Flyout::History, &standard[..]),
            (Mode::Standard, Flyout::Memory, &standard[..]),
            (Mode::Programmer, Flyout::Memory, &standard[..1]),
        ];
        for (mode, flyout, buttons) in cases {
            // With a calculation in the history (not in the Programmer mode) and in memory.
            let start = |ctx: &Context| {
                let mut app = start_in(ctx);
                app.set_mode(mode);
                for command in [
                    Command::Digit(2),
                    Command::Operator(Op::Add),
                    Command::Digit(3),
                    Command::Equals,
                    Command::MemoryStore,
                ] {
                    app.engine_mut().execute(command);
                }
                run(ctx, &mut app, MIN_SIZE, vec![]);
                app
            };
            // A click on a header button only closes the flyout (`OnHistoryFlyOutTapped`).
            for name in buttons {
                let ctx = Context::default();
                let mut app = start(&ctx);
                let button = ctx.read_response(Id::new(name)).unwrap().rect;
                app.flyout = flyout;
                run(&ctx, &mut app, MIN_SIZE, vec![]);
                click(&ctx, &mut app, MIN_SIZE, button.center());
                assert!(
                    app.flyout == Flyout::None && !app.compact,
                    "{mode:?}: a click on {name} did more than close the flyout"
                );
            }
            // Tab only reaches the list.
            let ctx = Context::default();
            let mut app = start(&ctx);
            app.flyout = flyout;
            run(&ctx, &mut app, MIN_SIZE, vec![]);
            let list = ctx.read_response(Id::new("flyout")).unwrap().rect;
            let tab = key(Key::Tab, Modifiers::NONE);
            for _ in 0..6 {
                run(&ctx, &mut app, MIN_SIZE, vec![tab.clone()]);
                run(&ctx, &mut app, MIN_SIZE, vec![]);
                let focused = access::focused(&ctx).unwrap();
                let rect = ctx.read_response(focused).unwrap().rect;
                assert!(
                    rect.top() >= list.top(),
                    "{mode:?}: Tab reached {focused:?}"
                );
            }
        }

        // Docked there's no flyout, and none opens in keep-on-top mode: the header works.
        let ctx = Context::default();
        let mut app = start_in(&ctx);
        let docked = vec2(DOCK_WIDTH, 600.0);
        app.flyout = Flyout::Memory;
        run(&ctx, &mut app, docked, vec![]);
        let menu = ctx.read_response(Id::new("menu")).unwrap().rect;
        click(&ctx, &mut app, docked, menu.center());
        assert!(app.flyout == Flyout::Navigation);
        app.set_compact(&ctx, true);
        run(&ctx, &mut app, COMPACT_SIZE, vec![]);
        let full = ctx.read_response(Id::new("full")).unwrap().rect;
        click(&ctx, &mut app, COMPACT_SIZE, full.center());
        assert!(!app.compact);
    }

    #[test]
    fn covered_controls_ignore_screen_reader_clicks() {
        // What the flyouts lie over: the header and the memory row, and in the Programmer
        // mode the radix list and the row under it.
        let standard = [
            ("menu", Id::new("menu")),
            ("keep on top", Id::new("keep-on-top")),
            ("History", Id::new("history")),
            ("MS", Id::new("memory").with(4)),
            ("M˅", Id::new("memory-list")),
        ];
        let programmer = [
            ("menu", Id::new("menu")),
            ("HEX", Id::new("radix").with(0)),
            (
                "bit toggling keypad",
                Id::new("programmer-keypad-switch").with(1),
            ),
            ("word size", Id::new("programmer-word-size")),
            ("MS", programmer_view::memory_store_id()),
            ("M˅", Id::new("memory-list")),
        ];
        let cases = [
            (Mode::Standard, Flyout::History, &standard[..]),
            (Mode::Standard, Flyout::Memory, &standard[..]),
            (Mode::Programmer, Flyout::Memory, &programmer[..]),
        ];
        for (mode, flyout, controls) in cases {
            let ctx = Context::default();
            let mut app = start_in(&ctx);
            app.set_mode(mode);
            app.engine_mut().execute(Command::MemoryStore);
            app.flyout = flyout;
            run(&ctx, &mut app, MIN_SIZE, vec![]);
            // Nothing may change: egui takes AccessKit's `Click` for a real click, so the
            // controls must not sense clicks at all.
            let state = |app: &CalculatorApp| {
                let calc = app.engine();
                let programmer = (app.programmer.bit_flip, calc.radix(), calc.word_size());
                (app.flyout, app.compact, programmer, calc.memory().len())
            };
            let before = state(&app);
            for &(name, id) in controls {
                let covered = id.with("covered");
                run(&ctx, &mut app, MIN_SIZE, vec![screen_reader_click(covered)]);
                assert!(
                    state(&app) == before,
                    "{mode:?}: a screen reader's click on {name} went through"
                );
                let response = ctx.read_response(covered).unwrap();
                assert!(!response.sense.senses_click(), "{mode:?}: {name}");
            }
        }
    }

    /// The app as eframe starts it, with what `storage` holds.
    fn start(storage: &MemoryStorage) -> CalculatorApp {
        let mut cc = eframe::CreationContext::_new_kittest(Context::default());
        cc.storage = Some(storage);
        CalculatorApp::new(&cc)
    }

    #[test]
    fn opens_in_the_mode_it_was_closed_in() {
        // In English, which other tests expect of `tr`, whatever the Windows display language.
        let mut storage = MemoryStorage::default();
        storage.0.insert(LANGUAGE_KEY.into(), "en-US".into());
        let mut app = start(&storage);
        for mode in Mode::CALCULATORS
            .into_iter()
            .chain(Category::ALL.map(Mode::Converter))
        {
            app.set_mode(mode);
            // Settings is a page over the mode, not a mode.
            app.page = Page::Settings;
            eframe::App::save(&mut app, &mut storage);
            let reopened = start(&storage);
            assert_eq!(reopened.mode, mode);
            assert!(reopened.page == Page::Calculator);
            // The mode's own engine is the one with the memory, as after picking it in the menu.
            let engine = match mode {
                Mode::Scientific => &reopened.scientific.calc,
                Mode::Programmer => &reopened.programmer.calc,
                _ => &reopened.calc,
            };
            assert!(std::ptr::eq(reopened.engine(), engine), "{mode:?}");
        }
    }

    #[test]
    fn modes_are_saved_with_windows_ids() {
        // `NavCategory`'s `SerializationId`s.
        let ids = [
            (Mode::Standard, 0),
            (Mode::Scientific, 1),
            (Mode::Programmer, 2),
            (Mode::DateCalculation, 3),
            (Mode::Converter(Category::Volume), 4),
            (Mode::Converter(Category::Length), 5),
            (Mode::Converter(Category::Weight), 6),
            (Mode::Converter(Category::Temperature), 7),
            (Mode::Converter(Category::Energy), 8),
            (Mode::Converter(Category::Area), 9),
            (Mode::Converter(Category::Speed), 10),
            (Mode::Converter(Category::Time), 11),
            (Mode::Converter(Category::Power), 12),
            (Mode::Converter(Category::Data), 13),
            (Mode::Converter(Category::Pressure), 14),
            (Mode::Converter(Category::Angle), 15),
            (Mode::Graphing, 17),
        ];
        for (mode, id) in ids {
            assert_eq!(mode.serialization_id(), id, "{mode:?}");
            assert_eq!(Mode::from_serialization_id(id), Some(mode));
        }
        // Currency, and ids no version of Windows has: the app opens in Standard.
        for id in [16, -1, 18] {
            assert_eq!(Mode::from_serialization_id(id), None);
        }
    }

    /// Whether a key's event stays a key press, with the text it typed after it, and `AltGr`
    /// held since an earlier frame or not.
    fn stays_key(pressed: Key, text: &str, alt_gr: bool) -> bool {
        let mut events = vec![key(pressed, Modifiers::NONE), Event::Text(text.into())];
        remove_alt_gr_keys(&mut events, alt_gr);
        assert_eq!(
            events.last(),
            Some(&Event::Text(text.into())),
            "the text stays"
        );
        events.len() == 2
    }

    #[test]
    fn alt_gr_characters() {
        // AltGr+Q types `@` (√x), AltGr+M the micro sign `µ` and AltGr+E `€` on a German
        // keyboard, and winit reports these keys without modifiers. Windows has no key
        // shortcuts with AltGr, which it sees as Ctrl+Alt.
        assert!(!stays_key(Key::Q, "@", false));
        assert!(!stays_key(Key::M, "µ", false));
        assert!(!stays_key(Key::E, "€", false));
        // The key's letter in either case, and a letter of another script, which Windows'
        // shortcuts see as the key's letter (a Russian keyboard types `й` on the Q key, `VK_Q`).
        assert!(stays_key(Key::Q, "q", false));
        assert!(stays_key(Key::Q, "Q", false));
        assert!(stays_key(Key::Q, "й", false));
        // A Turkish keyboard's I key types `ı`, whose capital is `I`.
        assert!(stays_key(Key::I, "ı", false));
        // Only letter keys: Shift+2 types `@` on a US keyboard.
        assert!(stays_key(Key::Num2, "@", false));
        // Ctrl+Q types nothing, and stays a key press.
        let mut events = vec![key(Key::Q, Modifiers::COMMAND)];
        remove_alt_gr_keys(&mut events, false);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn dead_keys_and_alt_gr_held() {
        // A dead key's letter is still the key's: `´` then E types `é` on a French or German
        // keyboard, which is E (`exp`) in Windows (`VK_E`).
        assert!(stays_key(Key::E, "é", false));
        assert!(stays_key(Key::E, "Ê", false));
        // A dead key that can't combine with the letter types both: `´` then Q types `´q` on a
        // German keyboard, which is still Q (x²) in Windows (`VK_Q`).
        assert!(stays_key(Key::Q, "´q", false));
        // So is a `ś` typed without AltGr held (Polish AltGr+S's, where AltGr isn't the right
        // Alt key).
        assert!(stays_key(Key::S, "ś", false));
        // With AltGr held, no letter key is a shortcut, whatever it typed: AltGr+R types
        // nothing on a German keyboard, and egui-winit sends `r` then.
        assert!(!stays_key(Key::S, "ś", true));
        assert!(!stays_key(Key::R, "r", true));
        assert!(!stays_key(Key::Q, "@", true));
        // Other keys stay: AltGr+2 types `²`.
        assert!(stays_key(Key::Num2, "²", true));
    }

    #[test]
    fn alt_gr_in_order() {
        let alt_gr = |pressed, repeat| Event::Key {
            key: Key::AltRight,
            physical_key: Some(Key::AltRight),
            pressed,
            repeat,
            modifiers: Modifiers::NONE,
        };
        let q = key(Key::Q, Modifiers::NONE);
        // A plain Q, then AltGr+Q, in one frame: the right Alt key goes down between them, and
        // stays down, so only the second Q is AltGr's (4, Q, AltGr+Q is √(sqr(4)) = 4).
        let mut events = vec![q.clone(), Event::Text("q".into()), alt_gr(true, false)];
        events.extend([q.clone(), Event::Text("@".into())]);
        remove_alt_gr_keys(&mut events, true);
        let kept = [q.clone(), Event::Text("q".into()), alt_gr(true, false)];
        assert_eq!(events, [&kept[..], &[Event::Text("@".into())]].concat());
        // AltGr, held since an earlier frame (its press repeats), is let go before an S.
        let s = key(Key::S, Modifiers::NONE);
        let mut events = vec![alt_gr(true, true), alt_gr(false, false), s.clone()];
        events.push(Event::Text("s".into()));
        remove_alt_gr_keys(&mut events, false);
        assert!(events.contains(&s));
        // Held since an earlier frame, without an event in this one: Polish AltGr+S's `ś` is
        // AltGr's.
        let mut events = vec![s.clone(), Event::Text("ś".into())];
        remove_alt_gr_keys(&mut events, true);
        assert_eq!(events, [Event::Text("ś".into())]);
    }

    #[test]
    fn scientific_flyouts_lie_over_the_window() {
        // The header's menu and History buttons, and the unit, F-E, MS and M˅ buttons above
        // the list.
        let controls = [
            ("menu", Id::new("menu")),
            ("History", Id::new("history")),
            ("DEG", Id::new("scientific-angle")),
            ("F-E", Id::new("scientific-fe")),
            ("MS", Id::new("memory").with(4)),
            ("M˅", Id::new("memory-list")),
        ];
        for flyout in [Flyout::History, Flyout::Memory] {
            let label = if flyout == Flyout::History {
                "history"
            } else {
                "memory"
            };
            // With a calculation in the history and in memory.
            let ctx = Context::default();
            let mut app = start_in(&ctx);
            app.set_mode(Mode::Scientific);
            for command in [
                Command::Digit(2),
                Command::Operator(Op::Add),
                Command::Digit(3),
                Command::Equals,
                Command::MemoryStore,
            ] {
                app.engine_mut().execute(command);
            }
            run(&ctx, &mut app, MIN_SIZE, vec![]);
            let rect = |id| ctx.read_response(id).map(|response| response.rect);
            let rects: Vec<_> = controls.iter().map(|&(_, id)| rect(id).unwrap()).collect();
            // Nothing but the flyout changes.
            let unchanged = |app: &CalculatorApp| {
                let calc = &app.scientific.calc;
                calc.angle() == Angle::Degrees && !calc.notation() && calc.memory().len() == 1
            };
            // A click anywhere above the list only closes the flyout (`OnHistoryFlyOutTapped`).
            for (&(name, _), button) in controls.iter().zip(&rects) {
                app.flyout = flyout;
                run(&ctx, &mut app, MIN_SIZE, vec![]);
                click(&ctx, &mut app, MIN_SIZE, button.center());
                assert!(
                    app.flyout == Flyout::None && unchanged(&app),
                    "{label} flyout: a click on {name} did more than close it"
                );
            }
            app.flyout = flyout;
            run(&ctx, &mut app, MIN_SIZE, vec![]);
            // The rows stay in view, out of the Tab order, and nothing there can be clicked,
            // not even by a screen reader: egui takes AccessKit's `Click` for a real click.
            for (&(name, id), &before) in controls.iter().zip(&rects) {
                let covered = id.with("covered");
                run(&ctx, &mut app, MIN_SIZE, vec![screen_reader_click(covered)]);
                assert!(
                    app.flyout == flyout && unchanged(&app),
                    "{label} flyout: a screen reader's click on {name} went through"
                );
                let response = ctx.read_response(covered).unwrap();
                assert_eq!(response.rect, before, "{label} flyout: {name}");
                assert!(!response.sense.senses_click(), "{label} flyout: {name}");
            }
            // Tab only reaches the list.
            let list = rect(Id::new("flyout")).unwrap();
            let tab = key(Key::Tab, Modifiers::NONE);
            for _ in 0..6 {
                run(&ctx, &mut app, MIN_SIZE, vec![tab.clone()]);
                run(&ctx, &mut app, MIN_SIZE, vec![]);
                let focused = access::focused(&ctx).unwrap();
                let rect = ctx.read_response(focused).unwrap().rect;
                assert!(
                    rect.top() >= list.top(),
                    "{label} flyout: Tab reached {focused:?}"
                );
            }
        }

        // Docked there's no flyout, and the header works.
        let ctx = Context::default();
        let mut app = start_in(&ctx);
        app.set_mode(Mode::Scientific);
        let docked = vec2(DOCK_WIDTH, 600.0);
        app.flyout = Flyout::Memory;
        run(&ctx, &mut app, docked, vec![]);
        let menu = ctx.read_response(Id::new("menu")).unwrap().rect;
        click(&ctx, &mut app, docked, menu.center());
        assert!(app.flyout == Flyout::Navigation);
    }

    /// The text of a key press, as the calculator reads it.
    fn text_of(pressed: Key, text: &str) -> String {
        let mut events = vec![key(pressed, Modifiers::NONE), Event::Text(text.into())];
        latin_letters(&mut events, |_, _| None, false);
        match &events[1] {
            Event::Text(text) => text.clone(),
            event => panic!("{event:?}"),
        }
    }

    #[test]
    fn letters_of_other_scripts() {
        // A Russian keyboard types к, й and а on the R, Q and F keys, and a Greek one μ on
        // the M key. egui reports those keys as R, Q, F and M, and Windows' shortcuts
        // (virtual keys) see those letters.
        assert_eq!(text_of(Key::R, "к"), "R");
        assert_eq!(text_of(Key::Q, "Й"), "Q");
        assert_eq!(text_of(Key::F, "а"), "F");
        assert_eq!(text_of(Key::M, "μ"), "M");
        assert_eq!(char_command('R'), Some(Command::Function(Func::Reciprocal)));
        assert_eq!(char_command('Q'), Some(Command::Function(Func::Square)));
        // Latin letters, digits and what AltGr types stay as typed: AltGr+Q types `@` (√x)
        // and AltGr+M the micro sign `µ` on a German keyboard, AltGr+A `ą` on a Polish one.
        assert_eq!(text_of(Key::R, "r"), "r");
        assert_eq!(text_of(Key::Q, "@"), "@");
        assert_eq!(text_of(Key::A, "ą"), "ą");
        assert_eq!(text_of(Key::M, "µ"), "µ");
        assert_eq!(text_of(Key::Num1, "1"), "1");
        // A Russian letter on another key (ж is on the `;` key) or without its key stays too.
        assert_eq!(text_of(Key::Semicolon, "ж"), "ж");
        let mut events = vec![Event::Text("к".into())];
        latin_letters(&mut events, |_, _| None, false);
        assert_eq!(events, [Event::Text("к".into())]);
    }

    /// What Windows tells of a Greek keyboard (`layout_letter`): `VK_Q` at the Q key's place,
    /// which types `;`, and `:` with Shift (`VkKeyScanExW`).
    fn greek(physical: Key, typed: char) -> Option<char> {
        let typed = match typed {
            ';' => 0x0051,
            ':' => 0x0151,
            _ => -1,
        };
        vk_letter(if physical == Key::Q { 0x51 } else { 0 }, typed)
    }

    #[test]
    fn symbols_on_letter_keys() {
        let pressed = |key, physical| Event::Key {
            key,
            physical_key: Some(physical),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        // A Greek keyboard's Q key types `;`, and egui names the key after it, but Windows'
        // layout has `VK_Q` there (`layout_letter`): the key and its text become Q's.
        let mut events = vec![pressed(Key::Semicolon, Key::Q), Event::Text(";".into())];
        latin_letters(&mut events, greek, false);
        assert_eq!(events, [pressed(Key::Q, Key::Q), Event::Text("Q".into())]);
        // A French keyboard's `,` key and a Dvorak keyboard's `.` key are at the US M and E
        // keys' places, but their virtual keys aren't letters: they stay as typed.
        for (key, physical, text) in [(Key::Comma, Key::M, ","), (Key::Period, Key::E, ".")] {
            let mut events = vec![pressed(key, physical), Event::Text(text.into())];
            latin_letters(&mut events, |_, _| None, false);
            assert_eq!(events, [pressed(key, physical), Event::Text(text.into())]);
        }
        // A Czech keyboard's Q key types `\` with AltGr, which egui names, and winit reports
        // AltGr without modifiers. Windows sees `VK_Q` with Ctrl+Alt: no shortcut, so the key
        // and its text stay.
        let czech = |physical: Key, typed: char| {
            let typed = if typed == '\\' { 0x0651 } else { -1 };
            vk_letter(if physical == Key::Q { 0x51 } else { 0 }, typed)
        };
        let mut events = vec![pressed(Key::Backslash, Key::Q), Event::Text("\\".into())];
        latin_letters(&mut events, czech, false);
        let typed = [pressed(Key::Backslash, Key::Q), Event::Text("\\".into())];
        assert_eq!(events, typed);
        // The French A key is at the US Q key's place, and egui names it A already.
        let mut events = vec![pressed(Key::A, Key::Q), Event::Text("a".into())];
        latin_letters(&mut events, greek, false);
        assert_eq!(events, [pressed(Key::A, Key::Q), Event::Text("a".into())]);
        // The letter keys' scan codes, by their places on a US keyboard.
        assert_eq!(us_scan_code(Key::Q), Some(0x10));
        assert_eq!(us_scan_code(Key::P), Some(0x19));
        assert_eq!(us_scan_code(Key::A), Some(0x1E));
        assert_eq!(us_scan_code(Key::L), Some(0x26));
        assert_eq!(us_scan_code(Key::Z), Some(0x2C));
        assert_eq!(us_scan_code(Key::M), Some(0x32));
        assert_eq!(us_scan_code(Key::Semicolon), None);
    }

    #[test]
    fn virtual_key_letters() {
        // `VkKeyScanExW` gives a character's virtual key, then the Shift (1), Ctrl (2) and Alt
        // (4) it takes. The Greek `;` and `:` are `VK_Q`'s, without and with Shift: Q.
        assert_eq!(vk_letter(0x51, 0x0051), Some('Q'));
        assert_eq!(vk_letter(0x51, 0x0151), Some('Q'));
        // The Czech `\` is `VK_Q`'s with AltGr (Ctrl+Alt), which has no shortcuts.
        assert_eq!(vk_letter(0x51, 0x0651), None);
        // A character of another key (`VK_OEM_102`), or of none.
        assert_eq!(vk_letter(0x51, 0x00E2), None);
        assert_eq!(vk_letter(0x51, -1), None);
        // `VK_OEM_COMMA` (the French `,` at the US M key's place) is no letter.
        assert_eq!(vk_letter(0xBC, 0x00BC), None);
    }

    #[test]
    fn shifted_letters_of_other_scripts() {
        // Shift+R types `К` on a Russian keyboard. Windows has no Shift chords for the Standard
        // mode's R and Q, so it stays as typed there, while the Programmer mode's Shift+A to F
        // are digits.
        let typed = |pressed, text: &str, programmer| {
            let mut events = vec![key(pressed, Modifiers::SHIFT), Event::Text(text.into())];
            latin_letters(&mut events, |_, _| None, programmer);
            match &events[1] {
                Event::Text(text) => text.clone(),
                event => panic!("{event:?}"),
            }
        };
        assert_eq!(typed(Key::R, "К", false), "К");
        assert_eq!(typed(Key::Q, "Й", false), "Й");
        assert_eq!(typed(Key::F, "А", true), "F");
        // Caps Lock types `К` without Shift: R, 1/x.
        assert_eq!(text_of(Key::R, "К"), "R");
        // The Greek Shift+Q types `:`, `VK_Q` with Shift in Windows: no shortcut either.
        let colon = Event::Key {
            key: Key::Colon,
            physical_key: Some(Key::Q),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        };
        let mut events = vec![colon.clone(), Event::Text(":".into())];
        latin_letters(&mut events, greek, false);
        assert_eq!(events, [colon, Event::Text(":".into())]);
    }

    #[test]
    fn letter_keys_without_text() {
        // egui-winit sends no text while Ctrl is held: the Greek Ctrl+Q is `;`'s key alone, and
        // still `VK_Q` in Windows, which is M−.
        let ctrl = Modifiers::CTRL | Modifiers::COMMAND;
        let pressed = |key| Event::Key {
            key,
            physical_key: Some(Key::Q),
            pressed: true,
            repeat: false,
            modifiers: ctrl,
        };
        let mut events = vec![pressed(Key::Semicolon)];
        latin_letters(&mut events, greek, false);
        assert_eq!(events, [pressed(Key::Q)]);
        // Where the virtual key there isn't a letter, the key stays.
        let mut events = vec![pressed(Key::Semicolon)];
        latin_letters(&mut events, |_, _| None, false);
        assert_eq!(events, [pressed(Key::Semicolon)]);
        // Windows has no shortcuts with Alt, and left Ctrl+Alt is no Ctrl chord: the key stays.
        let ctrl_alt = Event::Key {
            key: Key::Semicolon,
            physical_key: Some(Key::Q),
            pressed: true,
            repeat: false,
            modifiers: ctrl | Modifiers::ALT,
        };
        let mut events = vec![ctrl_alt.clone()];
        latin_letters(&mut events, greek, false);
        assert_eq!(events, [ctrl_alt]);
    }

    /// A screen reader's `action` on the AccessKit node `node`.
    fn screen_reader(action: Action, node: NodeId) -> Event {
        Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
            action,
            target_node: node,
            target_tree: egui::accesskit::TreeId::ROOT,
            data: None,
        })
    }

    /// Runs a frame of `app` and returns the AccessKit nodes a screen reader gets from it.
    fn accesskit_nodes(
        ctx: &Context,
        app: &mut CalculatorApp,
        size: egui::Vec2,
    ) -> Vec<(NodeId, egui::accesskit::Node)> {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            ..Default::default()
        };
        let mut frame = eframe::Frame::_new_kittest();
        let mut output = ctx.run_ui(input, |ui| eframe::App::ui(app, ui, &mut frame));
        output.textures_delta.clear();
        output.platform_output.accesskit_update.unwrap().nodes
    }

    /// The mouse wheel turned 3 lines down at `at`, and the frames that egui spreads the
    /// scrolling over.
    fn wheel_down(ctx: &Context, app: &mut CalculatorApp, size: egui::Vec2, at: Pos2) {
        let wheel = Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: vec2(0.0, -3.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        };
        run(ctx, app, size, vec![Event::PointerMoved(at), wheel]);
        for _ in 0..20 {
            run(ctx, app, size, vec![]);
        }
    }

    /// Drags with the pointer from `from` by `by`: it presses there, moves in 8 frames and
    /// lets go.
    fn drag(ctx: &Context, app: &mut CalculatorApp, size: egui::Vec2, from: Pos2, by: egui::Vec2) {
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        run(ctx, app, size, vec![Event::PointerMoved(from)]);
        run(ctx, app, size, vec![button(from, true)]);
        for i in 1..=8 {
            let to = from + by * (i as f32 / 8.0);
            run(ctx, app, size, vec![Event::PointerMoved(to)]);
        }
        run(ctx, app, size, vec![button(from + by, false)]);
    }

    /// The Graphing mode in a `GRAPH_WINDOW` with a box for each of `equations`, and the
    /// graph options open over the graph's left side, away from the boxes.
    fn graph_options_over(ctx: &Context, equations: &[&str]) -> CalculatorApp {
        let mut app = start_in(ctx);
        app.set_mode(Mode::Graphing);
        run(ctx, &mut app, GRAPH_WINDOW, vec![]);
        for (i, text) in equations.iter().enumerate() {
            if i > 0 {
                app.graphing.add_row();
            }
            app.graphing.rows[i].text = (*text).into();
            app.graphing.submit(i, crate::graphing::Syntax::default());
        }
        let anchor = Rect::from_min_size(pos2(20.0, 50.0), vec2(40.0, 32.0));
        app.graphing.popup = Some(graphing_view::Popup::Settings {
            anchor,
            widths_open: false,
        });
        for _ in 0..3 {
            run(ctx, &mut app, GRAPH_WINDOW, vec![]);
        }
        app
    }

    #[test]
    fn screen_readers_cannot_click_under_popups() {
        // The menu over the Standard keypad: the covered 7 does nothing.
        let ctx = Context::default();
        let mut app = start_in(&ctx);
        app.flyout = Flyout::Navigation;
        run(&ctx, &mut app, MIN_SIZE, vec![]);
        let seven = Id::new("key").with(8).with("covered");
        run(&ctx, &mut app, MIN_SIZE, vec![screen_reader_click(seven)]);
        assert_eq!(app.display_text(), "0", "the 7 under the menu");
        assert!(app.flyout == Flyout::Navigation);

        // A converter's unit list over its keypad: no covered key types.
        let ctx = Context::default();
        let mut app = start_in(&ctx);
        app.set_mode(Mode::Converter(Category::Length));
        run(&ctx, &mut app, MIN_SIZE, vec![]);
        let unit = ctx.read_response(Id::new("converter-unit").with(true));
        click(&ctx, &mut app, MIN_SIZE, unit.unwrap().rect.center());
        assert!(app.converter.popup_open());
        for i in 0..15 {
            let key = Id::new("converter-key").with(i).with("covered");
            run(&ctx, &mut app, MIN_SIZE, vec![screen_reader_click(key)]);
            let converter = &app.converter.converter;
            assert_eq!(converter.value1(), "0", "the key {i} under the unit list");
            assert!(app.converter.popup_open());
        }

        // The date mode's page list over the From date picker, which the test finds by its
        // name, as a screen reader does: the From calendar doesn't open.
        let ctx = Context::default();
        ctx.enable_accesskit();
        let mut app = start_in(&ctx);
        app.set_mode(Mode::DateCalculation);
        run(&ctx, &mut app, MIN_SIZE, vec![]);
        let page = ctx.read_response(Id::new("date-page"));
        click(&ctx, &mut app, MIN_SIZE, page.unwrap().rect.center());
        assert!(app.date.popup_open());
        let nodes = accesskit_nodes(&ctx, &mut app, MIN_SIZE);
        let (from, _) = nodes
            .into_iter()
            .find(|(_, node)| node.role() == Role::ComboBox && node.label() == Some("From"))
            .expect("the From date picker");
        run(
            &ctx,
            &mut app,
            MIN_SIZE,
            vec![screen_reader(Action::Click, from)],
        );
        run(&ctx, &mut app, MIN_SIZE, vec![]);
        let calendar = ctx.read_response(Id::new("calendar-day").with(0));
        assert!(
            calendar.is_none(),
            "the From calendar opened under the page list"
        );
        assert!(app.date.popup_open());
    }

    #[test]
    fn history_menu_deletes_the_entry_it_was_opened_on() {
        // The docked history, with "2 + 3 = 5".
        let ctx = Context::default();
        let mut app = start_in(&ctx);
        for command in [
            Command::Digit(2),
            Command::Operator(Op::Add),
            Command::Digit(3),
            Command::Equals,
        ] {
            app.engine_mut().execute(command);
        }
        let size = vec2(DOCK_WIDTH + 100.0, 600.0);
        run(&ctx, &mut app, size, vec![]);
        let history = |app: &CalculatorApp| -> Vec<String> {
            let entries = app.engine().history().iter();
            entries
                .map(|e| format!("{} {}", e.expression, e.text))
                .collect()
        };
        let item = ctx.read_response(Id::new("history-item").with(0));
        let item = item.unwrap().rect.center();
        let right_click = |pressed| Event::PointerButton {
            pos: item,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let open_menu = |app: &mut CalculatorApp| {
            for event in [
                Event::PointerMoved(item),
                right_click(true),
                right_click(false),
            ] {
                run(&ctx, app, size, vec![event]);
            }
            assert!(app.context_menu.is_some());
        };
        let delete = screen_reader_click(Id::new("context-menu-item").with(1));

        // A screen reader's click on the covered = adds nothing, and Delete then deletes
        // the entry the menu was opened on.
        open_menu(&mut app);
        let equals = Id::new("key").with(23).with("covered");
        run(&ctx, &mut app, size, vec![screen_reader_click(equals)]);
        run(&ctx, &mut app, size, vec![delete.clone()]);
        assert_eq!(
            history(&app),
            [] as [&str; 0],
            "Delete removed another entry"
        );

        // If the history changes under the menu anyway, the menu closes: its entry moved.
        app.engine_mut().execute(Command::Equals);
        run(&ctx, &mut app, size, vec![]);
        open_menu(&mut app);
        app.engine_mut().execute(Command::Equals);
        run(&ctx, &mut app, size, vec![]);
        assert!(
            app.context_menu.is_none(),
            "the menu stays on a moved entry"
        );
        run(&ctx, &mut app, size, vec![delete]);
        assert_eq!(history(&app).len(), 2);
    }

    #[test]
    fn graph_controls_ignore_the_pointer_under_popups() {
        // y = ax: the graph options' backdrop senses only clicks, so the graph and a's slider
        // under it must not take the pointer's drags, wheel or presses.
        let ctx = Context::default();
        let mut app = graph_options_over(&ctx, &["y=ax"]);
        let view = app.graphing.view;
        let graph = ctx.read_response(Id::new("graph").with("covered"));
        let from = graph.unwrap().rect.right_bottom() - vec2(200.0, 100.0);
        drag(&ctx, &mut app, GRAPH_WINDOW, from, vec2(80.0, 0.0));
        wheel_down(&ctx, &mut app, GRAPH_WINDOW, from);
        let moved = app.graphing.view != view;
        assert!(!moved, "the graph moved under the graph options");
        assert!(app.graphing.popup.is_some());

        // A click on the slider closes the popup and leaves the slider alone.
        let slider = ctx.read_response(Id::new("graph-var-slider").with('a').with("covered"));
        let at = slider.unwrap().rect.left_center() + vec2(14.0, 0.0);
        click(&ctx, &mut app, GRAPH_WINDOW, at);
        assert!(app.graphing.popup.is_none());
        let value = app.graphing.variables[0].value;
        assert_eq!(value, 1.0, "the slider under the graph options");
    }

    #[test]
    fn pinching_the_graph_zooms_around_the_fingers() {
        // Two fingers on the graph's center spread from 100 to 200 px apart: the view halves
        // around that point. The first finger, which egui also takes for the pointer, would
        // drag the graph 50 px to the left on top of that.
        let ctx = Context::default();
        let mut app = start_in(&ctx);
        app.set_mode(Mode::Graphing);
        run(&ctx, &mut app, GRAPH_WINDOW, vec![]);
        let center = ctx.read_response(Id::new("graph")).unwrap().rect.center();
        let before = app.graphing.view;
        let touch = |id, pos, phase| Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(id),
            phase,
            pos,
            force: None,
        };
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let fingers = |apart: f32| {
            (
                center - vec2(apart / 2.0, 0.0),
                center + vec2(apart / 2.0, 0.0),
            )
        };
        let (left, right) = fingers(100.0);
        let start = vec![
            Event::PointerMoved(left),
            button(left, true),
            touch(0, left, egui::TouchPhase::Start),
            touch(1, right, egui::TouchPhase::Start),
        ];
        run(&ctx, &mut app, GRAPH_WINDOW, start);
        // egui's gesture starts in the frame after the fingers land, then they spread in steps.
        for apart in [100.0, 125.0, 150.0, 175.0, 200.0] {
            let (left, right) = fingers(apart);
            let spread = vec![
                Event::PointerMoved(left),
                touch(0, left, egui::TouchPhase::Move),
                touch(1, right, egui::TouchPhase::Move),
            ];
            run(&ctx, &mut app, GRAPH_WINDOW, spread);
        }
        let (left, right) = fingers(200.0);
        let end = vec![
            button(left, false),
            touch(0, left, egui::TouchPhase::End),
            touch(1, right, egui::TouchPhase::End),
        ];
        run(&ctx, &mut app, GRAPH_WINDOW, end);

        let after = app.graphing.view;
        let (width, height) = (before.x_max - before.x_min, before.y_max - before.y_min);
        // egui measures the pinch in f32.
        assert!(((after.x_max - after.x_min) / width - 0.5).abs() < 1e-6);
        assert!(((after.y_max - after.y_min) / height - 0.5).abs() < 1e-6);
        let middle = |min: f64, max: f64| (min + max) / 2.0;
        let moved = middle(after.x_min, after.x_max) - middle(before.x_min, before.x_max);
        assert!(moved.abs() < 1e-6 * width, "the graph moved by {moved}");
        assert!(app.graphing.manual, "the view no longer fits itself");
    }

    #[test]
    fn lists_under_popups_ignore_the_wheel() {
        // A history item's menu over the docked history, 20 entries long.
        let ctx = Context::default();
        let mut app = start_in(&ctx);
        for command in [
            Command::Digit(1),
            Command::Operator(Op::Add),
            Command::Digit(1),
        ] {
            app.engine_mut().execute(command);
        }
        for _ in 0..20 {
            app.engine_mut().execute(Command::Equals);
        }
        let size = vec2(DOCK_WIDTH + 100.0, 600.0);
        run(&ctx, &mut app, size, vec![]);
        let item = ctx.read_response(Id::new("history-item").with(0));
        let item = item.unwrap().rect.center();
        let right_click = |pressed| Event::PointerButton {
            pos: item,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        for event in [
            Event::PointerMoved(item),
            right_click(true),
            right_click(false),
        ] {
            run(&ctx, &mut app, size, vec![event]);
        }
        assert!(app.context_menu.is_some());
        run(&ctx, &mut app, size, vec![]);
        let row = Id::new("history-item").with(4).with("covered");
        let before = ctx.read_response(row).unwrap().rect;
        wheel_down(&ctx, &mut app, size, before.center());
        let after = ctx.read_response(row).unwrap().rect;
        assert_eq!(after, before, "the history under the menu scrolled");
        assert!(app.context_menu.is_some());

        // The graph options over a full equation list.
        let ctx = Context::default();
        let mut app = graph_options_over(&ctx, &[""; 14]);
        let row = Id::new("graph-eq-button").with(app.graphing.rows[1].id);
        let row = row.with("covered");
        let before = ctx.read_response(row).unwrap().rect;
        wheel_down(
            &ctx,
            &mut app,
            GRAPH_WINDOW,
            before.center() + vec2(100.0, 0.0),
        );
        let after = ctx.read_response(row).unwrap().rect;
        assert_eq!(
            after, before,
            "the equations under the graph options scrolled"
        );
        assert!(app.graphing.popup.is_some());
    }

    #[test]
    fn nested_lists_cover_their_flyout() {
        // The graph options with their line thickness list open: the flyout's other controls
        // take no input, so neither a screen reader nor a click picks Degrees, and a click on
        // the flyout closes only the list.
        let ctx = Context::default();
        let mut app = graph_options_over(&ctx, &["y=x"]);
        ctx.enable_accesskit();
        let anchor = Rect::from_min_size(pos2(20.0, 50.0), vec2(40.0, 32.0));
        let settings = |widths_open| {
            Some(graphing_view::Popup::Settings {
                anchor,
                widths_open,
            })
        };
        app.graphing.popup = settings(true);
        let nodes = accesskit_nodes(&ctx, &mut app, GRAPH_WINDOW);
        let degrees = tr("TrigModeDegrees.Content");
        let (node, _) = nodes
            .iter()
            .find(|(_, n)| n.label() == Some(degrees))
            .unwrap();
        run(
            &ctx,
            &mut app,
            GRAPH_WINDOW,
            vec![screen_reader(Action::Click, *node)],
        );
        assert!(
            app.graphing.angle == Angle::Radians,
            "Degrees under the list"
        );
        // A click on Degrees, outside the list, closes only the list.
        let unit = Id::new("graph-settings-unit").with(1usize);
        let unit = ctx
            .read_response(unit)
            .or_else(|| ctx.read_response(unit.with("covered")));
        let degrees = unit.unwrap().rect.center();
        click(&ctx, &mut app, GRAPH_WINDOW, degrees);
        let angle = app.graphing.angle;
        assert!(
            angle == Angle::Radians,
            "a click on Degrees outside the list"
        );
        assert!(
            app.graphing.popup == settings(false),
            "the list after a click on the flyout"
        );

        // The keyboard: Enter on the combo opens the list and keeps the focus there, Tab goes
        // past the theme, covered by the list, to the list, and Enter picks a thickness and
        // gives the focus back to the combo, as Esc and a click outside the list do.
        let combo = Id::new("graph-settings-width");
        let focused = |id| ctx.memory(|m| m.has_focus(id));
        let press = |k| vec![key(k, Modifiers::NONE)];
        ctx.memory_mut(|m| m.request_focus(combo));
        run(&ctx, &mut app, GRAPH_WINDOW, vec![]);
        run(&ctx, &mut app, GRAPH_WINDOW, press(Key::Enter));
        run(&ctx, &mut app, GRAPH_WINDOW, vec![]);
        assert!(app.graphing.popup == settings(true));
        assert!(focused(combo), "the combo lost the focus");
        run(&ctx, &mut app, GRAPH_WINDOW, press(Key::Tab));
        run(&ctx, &mut app, GRAPH_WINDOW, press(Key::Enter));
        run(&ctx, &mut app, GRAPH_WINDOW, vec![]);
        let width = app.graphing.line_width;
        assert_eq!(width, 1.0, "Tab went to the theme under the list");
        assert!(app.graphing.popup == settings(false));
        assert!(focused(combo), "the focus after a thickness");
        run(&ctx, &mut app, GRAPH_WINDOW, press(Key::Enter));
        assert!(
            app.graphing.popup == settings(true),
            "Enter after a thickness"
        );
        run(&ctx, &mut app, GRAPH_WINDOW, press(Key::Escape));
        run(&ctx, &mut app, GRAPH_WINDOW, vec![]);
        assert!(
            app.graphing.popup == settings(false),
            "Esc closed the flyout"
        );
        assert!(focused(combo), "the focus after Esc");
        run(&ctx, &mut app, GRAPH_WINDOW, press(Key::Enter));
        run(&ctx, &mut app, GRAPH_WINDOW, press(Key::Tab));
        click(&ctx, &mut app, GRAPH_WINDOW, degrees);
        run(&ctx, &mut app, GRAPH_WINDOW, vec![]);
        assert!(app.graphing.popup == settings(false));
        assert!(focused(combo), "the focus after a click outside the list");

        // The line options with their line style list open: no other color under it.
        let row = app.graphing.rows[0].id;
        let style = graphing_view::Popup::Style {
            row,
            anchor,
            styles_open: true,
        };
        app.graphing.popup = Some(style);
        let color = app.graphing.rows[0].color;
        let nodes = accesskit_nodes(&ctx, &mut app, GRAPH_WINDOW);
        // Another color, of the 13 whose names don't depend on the theme.
        let other = (color + 1) % 13;
        let other = tr(&format!("equationColor{}AutomationName", other + 1));
        let (node, _) = nodes
            .iter()
            .find(|(_, n)| n.label() == Some(other))
            .unwrap();
        run(
            &ctx,
            &mut app,
            GRAPH_WINDOW,
            vec![screen_reader(Action::Click, *node)],
        );
        assert_eq!(app.graphing.rows[0].color, color, "a color under the list");

        // The keyboard picks a line style in the list, and its combo gets the focus back.
        let combo = Id::new("graph-style-combo");
        let style = graphing_view::Popup::Style {
            row,
            anchor,
            styles_open: false,
        };
        app.graphing.popup = Some(style);
        ctx.memory_mut(|m| m.request_focus(combo));
        run(&ctx, &mut app, GRAPH_WINDOW, vec![]);
        run(&ctx, &mut app, GRAPH_WINDOW, press(Key::Enter));
        run(&ctx, &mut app, GRAPH_WINDOW, press(Key::Tab));
        run(&ctx, &mut app, GRAPH_WINDOW, press(Key::Tab));
        run(&ctx, &mut app, GRAPH_WINDOW, press(Key::Enter));
        run(&ctx, &mut app, GRAPH_WINDOW, vec![]);
        let dash = crate::graphing::plot::LineStyle::Dash;
        assert!(app.graphing.rows[0].style == dash, "the line style");
        assert!(app.graphing.popup == Some(style));
        assert!(focused(combo), "the focus after a line style");
    }

    #[test]
    fn a_held_esc_closes_only_the_list() {
        // The graph options' line thickness list, opened from the keyboard. Held down, the Esc
        // that closes it repeats before its key-up: the repeat closes nothing more, and the
        // combo keeps the focus. An Esc after the key-up closes the graph options.
        let ctx = Context::default();
        let mut app = graph_options_over(&ctx, &["y=x"]);
        let anchor = Rect::from_min_size(pos2(20.0, 50.0), vec2(40.0, 32.0));
        let settings = |widths_open| {
            Some(graphing_view::Popup::Settings {
                anchor,
                widths_open,
            })
        };
        let combo = Id::new("graph-settings-width");
        ctx.memory_mut(|m| m.request_focus(combo));
        run(&ctx, &mut app, GRAPH_WINDOW, vec![]);
        let enter = key(Key::Enter, Modifiers::NONE);
        run(&ctx, &mut app, GRAPH_WINDOW, vec![enter]);
        assert!(app.graphing.popup == settings(true));
        // Esc's key-down, a repeat or not, or its key-up.
        let esc = |down, repeated| {
            let mut event = key(Key::Escape, Modifiers::NONE);
            if let Event::Key {
                pressed, repeat, ..
            } = &mut event
            {
                (*pressed, *repeat) = (down, repeated);
            }
            event
        };
        run(&ctx, &mut app, GRAPH_WINDOW, vec![esc(true, false)]);
        assert!(app.graphing.popup == settings(false), "the first Esc");
        run(&ctx, &mut app, GRAPH_WINDOW, vec![esc(true, true)]);
        let popup = app.graphing.popup;
        assert!(popup == settings(false), "the held Esc's repeat");
        assert!(
            ctx.memory(|m| m.has_focus(combo)),
            "the focus after a repeat"
        );
        run(&ctx, &mut app, GRAPH_WINDOW, vec![esc(false, false)]);
        assert!(app.graphing.popup == settings(false));
        run(&ctx, &mut app, GRAPH_WINDOW, vec![esc(true, false)]);
        assert!(app.graphing.popup.is_none(), "an Esc after the key-up");
    }

    /// The settings file, in memory.
    #[derive(Default)]
    struct Settings(std::collections::HashMap<String, String>);

    impl eframe::Storage for Settings {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }

    /// A frame's input in a window of `size`, with what the platform says about it (`info`).
    fn frame(size: egui::Vec2, info: egui::ViewportInfo) -> egui::InputState {
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            ..Default::default()
        };
        raw.viewports.insert(egui::ViewportId::ROOT, info);
        let options = egui::InputOptions::default();
        egui::InputState::default().begin_pass(raw, false, 1.0, options)
    }

    /// What Windows, macOS and X11 tell about a window of `size` that is neither maximized
    /// nor minimized: where it is, too.
    fn placed(size: egui::Vec2) -> egui::ViewportInfo {
        egui::ViewportInfo {
            inner_rect: Some(Rect::from_min_size(pos2(40.0, 30.0), size)),
            maximized: Some(false),
            minimized: Some(false),
            ..Default::default()
        }
    }

    #[test]
    fn keep_on_top_opens_at_its_last_size() {
        let (full, larger) = (vec2(700.0, 800.0), vec2(420.0, 600.0));
        let mut sizes = WindowSizes::load(None);
        assert_eq!(
            sizes.enter_compact(&frame(full, placed(full))),
            COMPACT_SIZE
        );
        // Made larger, left and entered again, it gets that size back, as in Windows.
        assert_eq!(sizes.leave_compact(&frame(larger, placed(larger))), full);
        assert_eq!(sizes.enter_compact(&frame(full, placed(full))), larger);
        // A maximized or minimized window's size isn't kept.
        let huge = vec2(1920.0, 1040.0);
        let maximized = egui::ViewportInfo {
            maximized: Some(true),
            ..placed(huge)
        };
        let minimized = egui::ViewportInfo {
            minimized: Some(true),
            inner_rect: None,
            ..placed(huge)
        };
        for info in [maximized, minimized] {
            assert_eq!(sizes.leave_compact(&frame(huge, info)), full);
            assert_eq!(sizes.enter_compact(&frame(full, placed(full))), larger);
        }

        // Windows keeps it in its settings, so it lasts until the next start too.
        let mut settings = Settings::default();
        sizes.save(&mut settings);
        assert_eq!(settings.0[COMPACT_WIDTH_KEY], "420");
        assert_eq!(settings.0[COMPACT_HEIGHT_KEY], "600");
        let mut restarted = WindowSizes::load(Some(&settings));
        assert_eq!(restarted.enter_compact(&frame(full, placed(full))), larger);
        settings.0.insert(COMPACT_HEIGHT_KEY.into(), "tall".into());
        assert_eq!(WindowSizes::load(Some(&settings)).compact, None);
    }

    #[test]
    fn keep_on_top_window_cannot_be_maximized() {
        use ViewportCommand::*;
        // Windows' keep-on-top window has no maximize button and isn't maximized.
        let (compact, full) = (vec2(420.0, 600.0), vec2(700.0, 800.0));
        let buttons = |maximize| EnableButtons {
            close: true,
            minimized: true,
            maximize,
        };
        assert_eq!(
            keep_on_top_commands(true, compact),
            [
                WindowLevel(egui::WindowLevel::AlwaysOnTop),
                buttons(false),
                Maximized(false),
                MinInnerSize(COMPACT_MIN_SIZE),
                InnerSize(compact),
            ]
        );
        assert_eq!(
            keep_on_top_commands(false, full),
            [
                WindowLevel(egui::WindowLevel::Normal),
                buttons(true),
                InnerSize(full),
                MinInnerSize(MIN_SIZE),
            ]
        );
    }

    #[test]
    fn keep_on_top_size_without_a_window_position() {
        // Wayland doesn't tell a window where it is, so egui has no `inner_rect` there;
        // the size of egui's area is still the window's.
        let wayland = egui::ViewportInfo {
            maximized: Some(false),
            minimized: Some(false),
            ..Default::default()
        };
        let (full, larger) = (vec2(700.0, 800.0), vec2(420.0, 600.0));
        let mut sizes = WindowSizes::load(None);
        assert_eq!(
            sizes.enter_compact(&frame(full, wayland.clone())),
            COMPACT_SIZE
        );
        assert_eq!(sizes.leave_compact(&frame(larger, wayland.clone())), full);
        assert_eq!(sizes.enter_compact(&frame(full, wayland)), larger);
    }

    #[cfg(feature = "support-prompt")]
    #[test]
    fn support_state_persists_and_counts_launches() {
        let mut storage = MemoryStorage::default();
        storage.0.insert(LANGUAGE_KEY.into(), "en-US".into());
        let mut app = start(&storage);
        assert_eq!(app.support.stats.launches, 1);
        app.support.stats.donate_clicked_ever = true;
        eframe::App::save(&mut app, &mut storage);

        let reopened = start(&storage);
        assert_eq!(reopened.support.stats.launches, 2);
        assert!(reopened.support.stats.donate_clicked_ever);
    }

    #[cfg(feature = "support-prompt")]
    #[test]
    fn support_counts_successful_equals_in_the_calculator_modes() {
        let ctx = Context::default();
        let mut app = start_in(&ctx);
        let calculate = |app: &mut CalculatorApp, a, b| {
            app.press(Command::Digit(a), 0.0);
            app.press(Command::Operator(Op::Add), 0.0);
            app.press(Command::Digit(b), 0.0);
            app.press(Command::Equals, 0.0);
        };
        calculate(&mut app, 2, 2);
        assert_eq!(app.support.stats.calculations, 1);
        // An error isn't a calculation.
        app.press(Command::Digit(1), 0.0);
        app.press(Command::Operator(Op::Divide), 0.0);
        app.press(Command::Digit(0), 0.0);
        app.press(Command::Equals, 0.0);
        assert!(app.engine().has_error());
        assert_eq!(app.support.stats.calculations, 1);
        app.press(Command::Clear, 0.0);

        app.set_mode(Mode::Scientific);
        calculate(&mut app, 3, 3);
        assert_eq!(app.support.stats.calculations, 2);
        app.set_mode(Mode::Programmer);
        calculate(&mut app, 4, 1);
        assert_eq!(app.support.stats.calculations, 3);
        // Many calculations on one day are one active day.
        assert_eq!(app.support.stats.active_days, 1);
    }

    #[cfg(feature = "support-prompt")]
    #[test]
    fn support_popup_is_non_modal_and_escape_closes_it() {
        let ctx = Context::default();
        let mut app = start_in(&ctx);
        app.support.open_on_demand();
        run(&ctx, &mut app, MIN_SIZE, vec![]);
        assert!(app.support.is_open());

        // The calculator keeps working under it.
        run(&ctx, &mut app, MIN_SIZE, vec![Event::Text("5+5=".into())]);
        assert_eq!(app.engine().display(), "10");
        assert!(app.support.is_open());

        run(
            &ctx,
            &mut app,
            MIN_SIZE,
            vec![key(Key::Escape, Modifiers::NONE)],
        );
        assert!(!app.support.is_open(), "Escape closes the popup");
        assert_eq!(
            app.engine().display(),
            "10",
            "and doesn't clear the display"
        );
    }

    #[cfg(feature = "support-prompt")]
    #[test]
    fn support_menu_button_opens_popup_and_close_button_dismisses() {
        let ctx = Context::default();
        let mut app = start_in(&ctx);
        app.flyout = Flyout::Navigation;
        run(&ctx, &mut app, MIN_SIZE, vec![]);
        run(
            &ctx,
            &mut app,
            MIN_SIZE,
            vec![screen_reader_click(Id::new("support-menu-button"))],
        );
        assert!(app.support.is_open(), "the menu button opens the popup");
        assert!(app.flyout == Flyout::None, "and closes the menu");
        // Before any Donate click there is no "I already supported".
        run(&ctx, &mut app, MIN_SIZE, vec![]);
        assert!(ctx.read_response(Id::new("support-donate")).is_some());
        assert!(ctx.read_response(Id::new("support-already")).is_none());

        run(
            &ctx,
            &mut app,
            MIN_SIZE,
            vec![screen_reader_click(Id::new("support-close"))],
        );
        assert!(!app.support.is_open(), "the close button closes it");

        // After a Donate click it is there.
        app.support.stats.donate_clicked_ever = true;
        app.support.open_on_demand();
        run(&ctx, &mut app, MIN_SIZE, vec![]);
        run(&ctx, &mut app, MIN_SIZE, vec![]);
        assert!(ctx.read_response(Id::new("support-already")).is_some());
    }

    /// The popup lies under the display, inside the window, in every calculator mode.
    #[cfg(feature = "support-prompt")]
    #[test]
    fn support_popup_never_covers_the_display() {
        for size in [MIN_SIZE, vec2(480.0, 640.0), vec2(1200.0, 900.0)] {
            for mode in [Mode::Standard, Mode::Scientific, Mode::Programmer] {
                let ctx = Context::default();
                let mut app = start_in(&ctx);
                app.set_mode(mode);
                app.support.stats.donate_clicked_ever = true;
                app.support.open_on_demand();
                run(&ctx, &mut app, size, vec![]);
                run(&ctx, &mut app, size, vec![]);
                let display = ctx
                    .data(|d| d.get_temp::<(Rect, u64)>(Id::new("support-display")))
                    .unwrap()
                    .0;
                let window = Rect::from_min_size(Pos2::ZERO, size);
                for id in [
                    "support-close",
                    "support-donate",
                    "support-already",
                    "support-share",
                ] {
                    let rect = ctx.read_response(Id::new(id)).unwrap().rect;
                    assert!(
                        rect.top() >= display.bottom(),
                        "{id} at {rect:?} over the display {display:?} ({mode:?}, {size:?})"
                    );
                    assert!(
                        window.contains_rect(rect),
                        "{id} {rect:?} ({mode:?}, {size:?})"
                    );
                }
            }
        }
    }

    /// The red dot on the hamburger has a screen reader description.
    #[cfg(feature = "support-prompt")]
    #[test]
    fn support_dot_is_described() {
        let ctx = Context::default();
        let mut app = start_in(&ctx);
        app.support.stats.launches = 9;
        app.support.stats.active_days = 9;
        ctx.enable_accesskit();
        run(&ctx, &mut app, MIN_SIZE, vec![]);
        assert!(app.support.dot_visible());
        let nodes = accesskit_nodes(&ctx, &mut app, MIN_SIZE);
        let menu = nodes
            .iter()
            .find(|(id, _)| *id == Id::new("menu").accesskit_id())
            .map(|(_, node)| node)
            .unwrap();
        assert_eq!(menu.description(), Some("Support reminder"));
    }
}
