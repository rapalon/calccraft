//! The unit converters: two values with their unit drop-downs, and a keypad. The
//! layout and sizes follow `UnitConverter.xaml` from the Windows Calculator source.

use std::sync::Arc;

use eframe::egui::{
    self, Context, Event, FontFamily, FontId, Galley, Id, Key, Modifiers, Rect, Sense, Ui,
    UiBuilder, WidgetInfo, WidgetType, pos2, text::LayoutJob, vec2,
};

use super::date_view::{list_item, popup_frame};
use super::icons::{ICON_BACKSPACE, ICON_CHEVRON_DOWN, icon_label};
use super::strings::{tr, tr_format, unit_abbreviation, unit_name};
use super::{
    CalculatorApp, Flyout, GAP, KEY_FLASH, Kind, MARGIN, RADIUS, access, layout_elided_left, light,
    negate_label, numbers, repaint_for_flash, semibold, symbols, text,
};
use crate::converter::{Converter, ConverterCommand, Region};

/// From this window width on, the values and the keypad get larger (`Wide640`).
const WIDE_WIDTH: f32 = 640.0;
/// From this window size on, the digits grow again (`Wide`).
const HUGE_WIDTH: f32 = 1280.0;
const HUGE_HEIGHT: f32 = 768.0;
/// The row under the values with the "About equal to" results (`RowDltrUnits`).
const RESULTS_ROW_HEIGHT: f32 = 48.0;
const LIST_ITEM_HEIGHT: f32 = 36.0;
/// WinUI's `MaxDropDownHeight` (504 px) in items.
const LIST_VISIBLE_ITEMS: usize = 14;

/// The converter and what its UI keeps.
pub(super) struct ConverterState {
    pub(super) converter: Converter,
    /// The open unit drop-down, if any.
    popup: Option<UnitPopup>,
    /// A key pressed on the keyboard and when, to highlight its button.
    flash: Option<(ConverterCommand, f64)>,
    /// What is being typed in the bottom (`[0]`) and top (`[1]`) unit pickers.
    type_ahead: [TypeAhead; 2],
}

#[derive(Clone, Copy)]
struct UnitPopup {
    /// The top field's units (`true`) or the bottom field's.
    top: bool,
    /// The unit picker the list opened from.
    anchor: Rect,
    /// Scroll the selected unit into view on the first frame.
    scroll_pending: bool,
    /// A unit typed ahead to, to scroll to and focus.
    jump: Option<usize>,
}

impl ConverterState {
    pub(super) fn new() -> Self {
        Self {
            converter: Converter::new(Region::new(&user_region())),
            popup: None,
            flash: None,
            type_ahead: Default::default(),
        }
    }

    /// Closes the unit drop-down. Returns whether one was open.
    pub(super) fn close_popup(&mut self) -> bool {
        self.popup.take().is_some()
    }

    pub(super) fn popup_open(&self) -> bool {
        self.popup.is_some()
    }

    /// Lets the unit pickers use a typed text or an Enter press (`event`), with `focused`
    /// the control that has the keyboard focus. Typing in a focused picker or its open list
    /// goes to the unit whose name starts with what was typed (see `TypeAhead`): a closed
    /// picker takes that unit at once, as WinUI's `ComboBox` does; in an open list it takes
    /// the focus, so that Enter picks it, as after the arrow keys there. Returns the key that
    /// egui must no longer see: a space typed into a search, or an Enter used here.
    fn unit_picker_key(&mut self, event: &Event, focused: Option<Id>, now: f64) -> Option<Key> {
        match event {
            Event::Text(text) => self.type_ahead(text, focused, now),
            Event::Key {
                key: Key::Enter,
                pressed: true,
                ..
            } => self.enter(focused, now),
            _ => None,
        }
    }

    /// Enter after typing. In an open list it picks the unit typed to even before the list
    /// has focused it (which happens when the list is drawn, after the frame's keys), as
    /// WinUI's Enter picks the unit its search moved to (`ComboBox::PopupKeyDown`). A closed
    /// picker that is still searching ignores it: WinUI opens the list only when it isn't
    /// (`ComboBox::MainKeyDown`, `IsInSearchingMode`).
    fn enter(&mut self, focused: Option<Id>, now: f64) -> Option<Key> {
        match self.popup {
            Some(UnitPopup {
                top,
                jump: Some(index),
                ..
            }) => {
                self.set_unit(top, index);
                self.popup = None;
                Some(Key::Enter)
            }
            Some(_) => None,
            None => {
                let (top, _) = self.search_target(focused)?;
                let searching = self.type_ahead[usize::from(top)].searching(now);
                searching.then_some(Key::Enter)
            }
        }
    }

    /// The picker being typed into (`true` for the top one) and the unit its search goes on
    /// from: in an open list the focused unit, else the selected one.
    fn search_target(&self, focused: Option<Id>) -> Option<(bool, usize)> {
        let converter = &self.converter;
        match self.popup {
            Some(popup) => {
                let selected = if popup.top {
                    converter.unit1()
                } else {
                    converter.unit2()
                };
                let item = (0..converter.units().len())
                    .find(|&i| focused == Some(Id::new("converter-unit-item").with(i)));
                Some((popup.top, item.unwrap_or(selected)))
            }
            None if focused == Some(unit_picker_id(true)) => Some((true, converter.unit1())),
            None if focused == Some(unit_picker_id(false)) => Some((false, converter.unit2())),
            None => None,
        }
    }

    fn type_ahead(&mut self, text: &str, focused: Option<Id>, now: f64) -> Option<Key> {
        let (top, mut current) = self.search_target(focused)?;
        let units = self.converter.units();
        let names: Vec<&str> = units.iter().map(|unit| unit_name(unit.key)).collect();
        let search = &mut self.type_ahead[usize::from(top)];
        let (mut found, mut space) = (None, false);
        for c in text.chars() {
            if search.takes(c, now) {
                // A space in a name ("Square f…") doesn't press the focused control.
                space |= c == ' ';
                if let Some(index) = search.find(c, now, current, &names) {
                    (current, found) = (index, Some(index));
                }
            }
        }
        if let Some(index) = found {
            match &mut self.popup {
                Some(popup) => popup.jump = Some(index),
                None => self.set_unit(top, index),
            }
        }
        space.then_some(Key::Space)
    }

    fn set_unit(&mut self, top: bool, index: usize) {
        if top {
            self.converter.set_unit1(index);
        } else {
            self.converter.set_unit2(index);
        }
    }
}

/// The user's region (Windows Settings → Time & language → Language & region), which
/// picks the default units as Windows' `GeographicRegion` does. `US` when unknown. The web
/// version takes the country of the browser's regional format (`DE` for `de-DE`).
fn user_region() -> String {
    #[cfg(target_arch = "wasm32")]
    if let Some(country) = super::web::country() {
        return country;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::GetUserDefaultGeoName;
        let mut name = [0u16; 16];
        // SAFETY: the buffer and its length match; the call writes at most that many
        // characters, including the terminating zero it counts in its result.
        let length = unsafe { GetUserDefaultGeoName(name.as_mut_ptr(), name.len() as i32) };
        if length > 1 {
            return String::from_utf16_lossy(&name[..length as usize - 1]);
        }
    }
    "US".to_string()
}

fn unit_picker_id(top: bool) -> Id {
    Id::new("converter-unit").with(top)
}

fn is_converter_key(id: Id) -> bool {
    (0..15).any(|i| id == Id::new("converter-key").with(i))
}

/// The resource keys of what a screen reader calls a field's value and its unit picker:
/// "Convert from %1 %2" and "Input unit" for the field being typed into, "Converts into
/// %1 %2" and "Output unit" for the other. Windows swaps them when the other field becomes
/// the active one (`UnitConverterViewModel::OnSwitchActive`).
fn automation_keys(converter: &Converter, top: bool) -> (&'static str, &'static str) {
    if converter.value1_active() == top {
        ("Format_ValueFrom", "InputUnit_Name")
    } else {
        ("Format_ValueTo", "OutputUnit_Name")
    }
}

/// The name a screen reader gives a key of the converter's keypad.
fn command_name(command: ConverterCommand) -> &'static str {
    tr(match command {
        ConverterCommand::Digit(0) => "num0Button.AutomationName",
        ConverterCommand::Digit(1) => "num1Button.AutomationName",
        ConverterCommand::Digit(2) => "num2Button.AutomationName",
        ConverterCommand::Digit(3) => "num3Button.AutomationName",
        ConverterCommand::Digit(4) => "num4Button.AutomationName",
        ConverterCommand::Digit(5) => "num5Button.AutomationName",
        ConverterCommand::Digit(6) => "num6Button.AutomationName",
        ConverterCommand::Digit(7) => "num7Button.AutomationName",
        ConverterCommand::Digit(8) => "num8Button.AutomationName",
        ConverterCommand::Digit(_) => "num9Button.AutomationName",
        ConverterCommand::Decimal => "decimalSeparatorButton.AutomationName",
        ConverterCommand::Clear => "clearEntryButton.AutomationName",
        ConverterCommand::Backspace => "backSpaceButton.AutomationName",
        ConverterCommand::Negate => "converterNegateButton.AutomationName",
    })
}

/// The command a typed character stands for.
fn char_command(c: char) -> Option<ConverterCommand> {
    match c {
        '0'..='9' => Some(ConverterCommand::Digit(c as u8 - b'0')),
        '.' | ',' => Some(ConverterCommand::Decimal),
        '-' => Some(ConverterCommand::Negate),
        _ => None,
    }
}

/// The command a named key stands for. Windows gives `CE` two keys, Delete and Esc
/// (`UnitConverter` adds `Escape` to `ClearEntryButtonPos0`'s shortcut).
fn key_command(key: Key) -> Option<ConverterCommand> {
    match key {
        Key::Backspace => Some(ConverterCommand::Backspace),
        Key::Delete | Key::Escape => Some(ConverterCommand::Clear),
        Key::F9 => Some(ConverterCommand::Negate),
        _ => None,
    }
}

/// The heights of rows sized like XAML star rows: in proportion to their weights,
/// but never below their minimum heights.
pub(super) fn star_rows(total: f32, rows: &[(f32, f32)]) -> Vec<f32> {
    let mut heights = vec![0.0; rows.len()];
    let mut fixed = vec![false; rows.len()];
    loop {
        let free: f32 = total
            - (0..rows.len())
                .filter(|&i| fixed[i])
                .map(|i| heights[i])
                .sum::<f32>();
        let weight: f32 = (0..rows.len())
            .filter(|&i| !fixed[i])
            .map(|i| rows[i].0)
            .sum();
        let mut changed = false;
        for (i, &(star, min)) in rows.iter().enumerate() {
            if fixed[i] {
                continue;
            }
            let share = if weight > 0.0 {
                free.max(0.0) * star / weight
            } else {
                0.0
            };
            if share < min {
                heights[i] = min;
                fixed[i] = true;
                changed = true;
            } else {
                heights[i] = share;
            }
        }
        if !changed {
            return heights;
        }
    }
}

/// Typing a unit's name in a unit picker or its open list, like the text search of the
/// WinUI `ComboBox`es Windows uses for them (`IsTextSearchEnabled` stays on, and a unit's
/// text is its name). The characters typed within a second make one search, which keeps the
/// current unit while its name still starts with them; a search after a pause starts at the
/// next unit, so the same letter again goes on to the next unit with it. Case is ignored.
#[derive(Default)]
struct TypeAhead {
    /// What was typed, and when the last character came.
    text: String,
    at: f64,
}

impl TypeAhead {
    /// A pause this long (in seconds) starts a new search (`HasSearchStringTimedOut`).
    const TIMEOUT: f64 = 1.0;
    /// WinUI stops adding characters at 256.
    const MAX_CHARS: usize = 256;

    fn searching(&self, now: f64) -> bool {
        !self.text.is_empty() && now - self.at <= Self::TIMEOUT
    }

    /// Whether `c`, typed at `now`, is part of a search. A space only continues one: on
    /// its own it opens the list or picks the focused unit.
    fn takes(&self, c: char, now: f64) -> bool {
        !c.is_control() && (c != ' ' || self.searching(now))
    }

    /// Adds `c`, typed at `now`, to the search and returns the unit of `names` it finds,
    /// going on from the unit at `current` (WinUI's `ComboBox::SearchItemSourceIndex`).
    fn find(&mut self, c: char, now: f64, current: usize, names: &[&str]) -> Option<usize> {
        let new = !self.searching(now);
        if new {
            self.text.clear();
        }
        if self.text.chars().count() < Self::MAX_CHARS {
            self.text.push(c);
        }
        self.at = now;
        let text = self.text.to_lowercase();
        let start = current + usize::from(new);
        (0..names.len())
            .map(|i| (start + i) % names.len())
            .find(|&i| {
                let name = names[i].trim_start_matches(' ').to_lowercase();
                name.starts_with(&text)
            })
    }
}

impl CalculatorApp {
    // -------------------------------------------------------------- keyboard

    pub(super) fn handle_converter_keyboard(&mut self, ctx: &Context) {
        let now = ctx.input(|i| i.time);
        for event in ctx.input(|i| i.events.clone()) {
            // An open drop-down or menu takes the keyboard (Windows' `HonorShortcuts(false)`
            // while the menu is open); Esc closes it without clearing (Windows' view model
            // ignores `CE` while a drop-down is open).
            if self.converter.popup_open() || self.flyout == Flyout::Navigation {
                if matches!(
                    event,
                    Event::Key {
                        key: Key::Escape,
                        pressed: true,
                        ..
                    }
                ) && !self.converter.close_popup()
                {
                    self.flyout = Flyout::None;
                } else if self.flyout != Flyout::Navigation {
                    // Typing in an open unit list finds a unit, and Enter picks it.
                    self.unit_picker_key(ctx, &event, now);
                }
                continue;
            }
            // Typing in a focused unit picker finds a unit; Enter waits for the search to end.
            self.unit_picker_key(ctx, &event, now);
            match event {
                Event::Text(text) => {
                    for command in text.chars().filter_map(char_command) {
                        self.converter_press(command, now);
                    }
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if !modifiers.command => {
                    // The keypad's keys ignore Enter, as Windows' `CalculatorButton`s do.
                    if key == Key::Enter && access::focused(ctx).is_some_and(is_converter_key) {
                        ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                    } else if let Some(command) = key_command(key) {
                        // egui drops the focus on Esc; in Windows it's only `CE`.
                        if key == Key::Escape
                            && let Some(id) = self.last_focus
                        {
                            ctx.memory_mut(|memory| memory.request_focus(id));
                        }
                        self.converter_press(command, now);
                    }
                }
                Event::Copy => {
                    ctx.copy_text(numbers().localize_plain(&self.converter.converter.copy_text()))
                }
                Event::Paste(text) => {
                    self.converter.converter.paste(&numbers().delocalize(&text));
                }
                _ => {}
            }
        }
        repaint_for_flash(ctx, &mut self.converter.flash, now);
    }

    fn converter_press(&mut self, command: ConverterCommand, now: f64) {
        // `±` is hidden where values can't be negative, so its key does nothing there.
        let hidden = command == ConverterCommand::Negate
            && !self.converter.converter.category().supports_negative();
        if !hidden {
            self.converter.converter.execute(command);
            self.converter.flash = Some((command, now));
        }
    }

    /// Lets the unit pickers use `event` (see `ConverterState::unit_picker_key`), and hides
    /// from egui a key they used.
    fn unit_picker_key(&mut self, ctx: &Context, event: &Event, now: f64) {
        if let Some(key) = self
            .converter
            .unit_picker_key(event, access::focused(ctx), now)
        {
            ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key));
        }
    }

    // ---------------------------------------------------------------- layout

    /// Draws the converter below the header. `full` is the whole window.
    pub(super) fn draw_converter_mode(&mut self, ui: &mut Ui, full: Rect, top: f32) {
        let now = ui.input(|i| i.time);
        let content = Rect::from_min_max(pos2(full.left(), top), full.max);
        let wide = full.width() >= WIDE_WIDTH;
        let huge = wide && full.width() >= HUGE_WIDTH && full.height() >= HUGE_HEIGHT;
        // In a landscape window the keypad moves next to the values (`LandscapeLayout`).
        let landscape = full.width() >= full.height();

        let (fields, keypad) = if landscape {
            let half = content.width() / 2.0;
            let fields = Rect::from_min_size(content.min, vec2(half, content.height()));
            let keypad =
                Rect::from_min_max(pos2(content.left() + half, content.top()), content.max);
            (fields, Some(keypad))
        } else {
            (content, None)
        };
        let heights = if landscape {
            star_rows(
                fields.height(),
                &[
                    (4.0, 56.0),
                    (2.0, 32.0),
                    (4.0, 56.0),
                    (2.0, 32.0),
                    (2.0, RESULTS_ROW_HEIGHT),
                ],
            )
        } else {
            let mut rows = star_rows(
                fields.height() - RESULTS_ROW_HEIGHT,
                &[
                    (56.0, 56.0),
                    (32.0, 32.0),
                    (56.0, 56.0),
                    (32.0, 32.0),
                    (272.0, 0.0),
                ],
            );
            rows.insert(4, RESULTS_ROW_HEIGHT);
            rows
        };
        let mut y = fields.top();
        let mut row = |i: usize| {
            let rect =
                Rect::from_min_size(pos2(fields.left(), y), vec2(fields.width(), heights[i]));
            y += heights[i];
            rect
        };
        let (value1, unit1, value2, unit2, results) = (row(0), row(1), row(2), row(3), row(4));
        let keypad = keypad.unwrap_or_else(|| row(5));

        let (max_font, value_margin, combo_height) = if wide {
            (46.0, 12.0, 44.0)
        } else {
            (40.0, 4.0, 32.0)
        };
        self.draw_value(ui, value1, true, max_font, value_margin);
        let anchor1 = self.draw_unit_picker(ui, unit1, true, combo_height);
        self.draw_value(ui, value2, false, max_font, value_margin);
        let anchor2 = self.draw_unit_picker(ui, unit2, false, combo_height);
        self.draw_suggestions(ui, results);
        for (top, anchor) in [(true, anchor1), (false, anchor2)] {
            if let Some(anchor) = anchor {
                self.converter.popup = Some(UnitPopup {
                    top,
                    anchor,
                    scroll_pending: true,
                    jump: None,
                });
                self.opened_from(ui.ctx(), unit_picker_id(top));
            }
        }

        // `Margin="3,0,3,3"` around the keypad, and 1 px around each key.
        let keypad = Rect::from_min_max(
            keypad.min + vec2(MARGIN, 0.0),
            keypad.max - vec2(MARGIN, MARGIN),
        );
        let (digit_size, operator_size, negate_size) = if huge {
            (46.0, 24.0, 24.0)
        } else if wide {
            (28.0, 20.0, 20.0)
        } else {
            (18.0, 14.0, 16.0)
        };
        self.draw_converter_keypad(ui, keypad, now, digit_size, operator_size, negate_size);

        if let Some(popup) = self.converter.popup {
            self.draw_unit_list(ui, full, popup);
        }
    }

    /// One of the two values: left-aligned at the bottom of its row, in semibold
    /// while it's the one being typed into, shrinking to fit. A click activates it.
    fn draw_value(&mut self, ui: &Ui, rect: Rect, top: bool, max_font: f32, margin: f32) {
        let p = self.palette;
        let converter = &self.converter.converter;
        let value = if converter.has_paste_error() {
            tr("CEngine.100").to_string()
        } else if top {
            numbers().localize(&converter.value1())
        } else {
            numbers().localize(&converter.value2())
        };
        let active = converter.value1_active() == top;
        let family = if active { semibold() } else { light() };

        // The value sits in a 12 px padded box with a 4 px margin (`CalculationResult`).
        let left = rect.left() + 16.0;
        let max_width = rect.width() - 28.0;
        let painter = ui.painter();
        let measured = painter
            .layout_no_wrap(value.clone(), FontId::new(max_font, family.clone()), p.text)
            .size()
            .x;
        let size = if measured > max_width {
            (max_font * max_width / measured).max(14.0)
        } else {
            max_font
        };
        let galley = layout_elided_left(painter, &value, FontId::new(size, family), max_width);
        let pos = pos2(left, rect.bottom() - margin - galley.size().y);
        painter.galley(pos, galley, p.text);

        // "Convert from 5 Kilometers" for the value being typed into, "Converts into
        // 3.106856 Miles" for the other; like Windows, screen readers read the bottom value
        // out when it changes.
        let unit = if top {
            converter.unit1()
        } else {
            converter.unit2()
        };
        let unit = unit_name(converter.units()[unit].key);
        let (format, _) = automation_keys(converter, top);
        let name = tr_format(format, &[&value, unit]);
        let id = Id::new("converter-value").with(top);
        let info = || WidgetInfo::labeled(WidgetType::Button, true, &name);
        let response = access::control(ui, p, rect, id, true, RADIUS, info);
        if !top {
            ui.ctx().accesskit_node_builder(id, |node| {
                node.set_live(egui::accesskit::Live::Polite);
            });
        }
        if response.clicked() {
            self.converter.converter.activate(top);
        }
    }

    /// The unit drop-down of a value: the unit's name and a chevron, without a box.
    /// Returns the picker's rectangle when it was clicked.
    fn draw_unit_picker(&self, ui: &Ui, row: Rect, top: bool, height: f32) -> Option<Rect> {
        let p = self.palette;
        let converter = &self.converter.converter;
        let index = if top {
            converter.unit1()
        } else {
            converter.unit2()
        };
        let name = unit_name(converter.units()[index].key);
        let painter = ui.painter();
        let label = painter.layout_no_wrap(name.to_string(), FontId::proportional(14.0), p.text);
        let chevron = painter.layout_job(icon_label(ICON_CHEVRON_DOWN, 12.0));
        // `Margin="12,0,0,0"`, `Padding="4,0,0,0"`, and the chevron's `Margin="8,3,12,0"`.
        let width = 4.0 + label.size().x + 8.0 + chevron.size().x + 12.0;
        let rect = Rect::from_min_size(pos2(row.left() + 12.0, row.top()), vec2(width, height));

        let (_, picker) = automation_keys(converter, top);
        let info = access::combo_box(tr(picker), name);
        let id = unit_picker_id(top);
        let response = access::control(ui, p, rect, id, true, RADIUS, info);
        let color = if response.is_pointer_button_down_on() {
            p.unit_pressed
        } else if response.hovered() {
            p.unit_hover
        } else {
            p.text
        };
        let y = rect.center().y;
        painter.galley(
            pos2(rect.left() + 4.0, y - label.size().y / 2.0),
            label.clone(),
            color,
        );
        painter.galley(
            pos2(
                rect.left() + 4.0 + label.size().x + 8.0,
                y + 1.5 - chevron.size().y / 2.0,
            ),
            chevron,
            color,
        );
        response.clicked().then_some(rect)
    }

    /// The "About equal to" results (`SupplementaryResults.xaml`): a caption, then as
    /// many results as fit on one line, like `HorizontalNoOverflowStackPanel`.
    fn draw_suggestions(&self, ui: &Ui, row: Rect) {
        /// A laid-out result and its width with the margins.
        struct Item {
            glyph: Option<(Arc<Galley>, f32)>,
            value: Arc<Galley>,
            unit: Arc<Galley>,
            width: f32,
            /// What a screen reader says: the value and the unit's full name.
            name: String,
        }

        let p = self.palette;
        let suggestions = self.converter.converter.suggestions();
        if suggestions.is_empty() {
            return;
        }
        let painter = ui.painter();
        let caption = painter.layout_no_wrap(
            tr("SupplementaryResultsHeader.Text").to_string(),
            FontId::proportional(12.0),
            p.text_secondary,
        );
        // The results' line overlaps the caption by 6 px and is 27 px high; the
        // texts sit at its bottom.
        let bottom = row.top() + caption.size().y - 6.0 + 27.0;
        painter.galley(
            pos2(row.left() + 16.0, row.top()),
            caption,
            p.text_secondary,
        );

        let items: Vec<Item> = suggestions
            .iter()
            .map(|s| {
                let shown = numbers().localize(&s.value);
                let value =
                    painter.layout_no_wrap(shown.clone(), FontId::new(14.0, semibold()), p.text);
                let unit = painter.layout_no_wrap(
                    unit_abbreviation(s.key).to_string(),
                    FontId::proportional(14.0),
                    p.text_secondary,
                );
                let name = tr_format(
                    "SupplementaryUnit_AutomationName",
                    &[&shown, unit_name(s.key)],
                );
                let texts = value.size().x + 4.0 + unit.size().x;
                match s.glyph {
                    // `Margin="16,0,4,0"` before the value and 16 px after the unit.
                    None => Item {
                        glyph: None,
                        width: 16.0 + texts + 16.0,
                        value,
                        unit,
                        name,
                    },
                    // The picture, its own gap, and the value 2 px closer.
                    Some((glyph, gap)) => {
                        let glyph = painter.layout_no_wrap(
                            glyph.to_string(),
                            FontId::new(20.0, symbols()),
                            p.text,
                        );
                        Item {
                            width: glyph.size().x + gap - 2.0 + texts,
                            glyph: Some((glyph, gap)),
                            value,
                            unit,
                            name,
                        }
                    }
                }
            })
            .collect();

        // Left to right while they fit; with more than two results, room is kept for
        // the comparison at the end, so the ones before it give way.
        let available = row.width() - 6.0;
        let last = items.len() - 1;
        let reserved = if items.len() > 2 && items[last].glyph.is_some() {
            items[last].width
        } else {
            0.0
        };
        let mut x = row.left();
        for (i, item) in items.into_iter().enumerate() {
            let mut room = available - (x - row.left());
            if i != last {
                room -= reserved;
            }
            if room <= 0.0 || item.width > room {
                continue;
            }
            let rect = Rect::from_min_max(pos2(x, bottom - 24.0), pos2(x + item.width, bottom));
            access::text(ui, rect, Id::new("suggestion").with(i), &item.name, false);
            let mut left = x;
            if let Some((glyph, gap)) = item.glyph {
                // `Margin="0,3,…,-3"`: the picture sits 3 px lower.
                let size = glyph.size();
                painter.galley(pos2(left, bottom + 3.0 - size.y), glyph, p.text);
                left += size.x + gap - 2.0;
            } else {
                left += 16.0;
            }
            let value_width = item.value.size().x;
            painter.galley(pos2(left, bottom - item.value.size().y), item.value, p.text);
            left += value_width + 4.0;
            painter.galley(
                pos2(left, bottom - item.unit.size().y),
                item.unit,
                p.text_secondary,
            );
            x += item.width;
        }
    }

    /// The units list of an open drop-down, with the selected unit over the picker
    /// where it fits, as in WinUI.
    fn draw_unit_list(&mut self, ui: &mut Ui, full: Rect, popup: UnitPopup) {
        let p = self.palette;
        let converter = &self.converter.converter;
        let units = converter.units();
        let selected = if popup.top {
            converter.unit1()
        } else {
            converter.unit2()
        };
        let backdrop = ui.interact(full, Id::new("converter-backdrop"), Sense::CLICK);
        access::set_covered(ui.ctx(), false);

        let fits = ((full.height() - 16.0) / LIST_ITEM_HEIGHT).floor() as usize;
        let visible = units.len().min(LIST_VISIBLE_ITEMS).min(fits).max(1);
        let height = visible as f32 * LIST_ITEM_HEIGHT + 8.0;
        let widest = units
            .iter()
            .map(|unit| {
                ui.painter()
                    .layout_no_wrap(
                        unit_name(unit.key).to_string(),
                        FontId::proportional(14.0),
                        p.text,
                    )
                    .size()
                    .x
            })
            .fold(0.0, f32::max);
        let width = (widest + 48.0)
            .max(popup.anchor.width())
            .min(full.width() - 8.0);
        // Like WinUI, put the selected unit on the picker: a list that scrolls is
        // centered on the picker and scrolled there, a short one is moved there.
        // Both stay inside the window.
        let lowest_top = (full.bottom() - 4.0 - height).max(full.top() + 4.0);
        let fit = |top: f32| top.clamp(full.top() + 4.0, lowest_top);
        // Where the selected row's top edge would sit on the picker.
        let target = popup.anchor.center().y - LIST_ITEM_HEIGHT / 2.0 - 4.0;
        let max_offset = (units.len() - visible) as f32 * LIST_ITEM_HEIGHT;
        let centered = fit(target - (visible / 2) as f32 * LIST_ITEM_HEIGHT);
        let row = selected as f32 * LIST_ITEM_HEIGHT;
        let offset = (row - (target - centered)).clamp(0.0, max_offset);
        let top = fit(target - (row - offset));
        let left = popup
            .anchor
            .left()
            .min(full.right() - 4.0 - width)
            .max(full.left() + 4.0);
        let rect = Rect::from_min_size(pos2(left, top), vec2(width, height));
        popup_frame(p, ui.painter(), rect, p.popup);
        ui.interact(rect, Id::new("converter-unit-list"), Sense::CLICK);

        let mut chosen = None;
        ui.scope_builder(UiBuilder::new().max_rect(rect.shrink(4.0)), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut area = egui::ScrollArea::vertical()
                .id_salt(("converter-unit-list", popup.top))
                .auto_shrink(false);
            if popup.scroll_pending {
                area = area.vertical_scroll_offset(offset);
            } else if let Some(jump) = popup.jump {
                // Typed ahead: the unit found goes to the top of the list, as in Settings.
                area = area.vertical_scroll_offset(jump as f32 * LIST_ITEM_HEIGHT);
            }
            area.show_rows(ui, LIST_ITEM_HEIGHT, units.len(), |ui, range| {
                for index in range {
                    let size = vec2(ui.available_width(), LIST_ITEM_HEIGHT);
                    let (row, _) = ui.allocate_exact_size(size, Sense::hover());
                    let item = row.shrink2(vec2(0.0, 2.0));
                    let id = Id::new("converter-unit-item").with(index);
                    let name = unit_name(units[index].key);
                    let focus =
                        self.take_popup_focus(index == selected) || popup.jump == Some(index);
                    if list_item(p, ui, id, item, name, index == selected, focus) {
                        chosen = Some(index);
                    }
                }
            });
        });

        if let Some(open) = &mut self.converter.popup {
            open.scroll_pending = false;
            open.jump = None;
        }
        if let Some(index) = chosen {
            if popup.top {
                self.converter.converter.set_unit1(index);
            } else {
                self.converter.converter.set_unit2(index);
            }
            self.converter.popup = None;
        } else if backdrop.clicked() {
            self.converter.popup = None;
        }
    }

    /// The keypad: `CE` and `⌫` above the number pad, and `±` left of `0` where
    /// values can be negative.
    fn draw_converter_keypad(
        &mut self,
        ui: &Ui,
        rect: Rect,
        now: f64,
        digit_size: f32,
        operator_size: f32,
        negate_size: f32,
    ) {
        use ConverterCommand::*;
        // Windows' converter names its keys' panel so (`access::group`).
        let id = Id::new("converter-keypad");
        let group = access::group(ui, id, rect, tr("NumberPad.AutomationName"));
        let ui = &group;
        #[rustfmt::skip]
        const KEYS: [Option<ConverterCommand>; 15] = [
            None, Some(Clear), Some(Backspace),
            Some(Digit(7)), Some(Digit(8)), Some(Digit(9)),
            Some(Digit(4)), Some(Digit(5)), Some(Digit(6)),
            Some(Digit(1)), Some(Digit(2)), Some(Digit(3)),
            Some(Negate), Some(Digit(0)), Some(Decimal),
        ];
        let negate = self.converter.converter.category().supports_negative();
        let width = (rect.width() - 2.0 * GAP) / 3.0;
        let height = (rect.height() - 4.0 * GAP) / 5.0;
        for (i, command) in KEYS.into_iter().enumerate() {
            let Some(command) = command else {
                continue;
            };
            if command == Negate && !negate {
                continue;
            }
            let (row, col) = ((i / 3) as f32, (i % 3) as f32);
            let cell = Rect::from_min_size(
                rect.min + vec2(col * (width + GAP), row * (height + GAP)),
                vec2(width, height),
            );
            let (kind, label): (Kind, LayoutJob) = match command {
                Digit(d) => (Kind::Digit, text(&d.to_string(), digit_size, semibold())),
                Decimal => (Kind::Digit, text(".", digit_size, semibold())),
                Negate => (Kind::Digit, negate_label(negate_size)),
                Clear => (
                    Kind::Operator,
                    text("CE", operator_size, FontFamily::Proportional),
                ),
                Backspace => (Kind::Operator, icon_label(ICON_BACKSPACE, operator_size)),
            };
            let flash =
                matches!(self.converter.flash, Some((c, t)) if c == command && now - t < KEY_FLASH);
            let id = Id::new("converter-key").with(i);
            let name = command_name(command);
            if self.button_flash(ui, cell, id, kind, true, label, name, flash)
                // Like Windows, CE does nothing while a drop-down is open.
                && !(command == Clear && self.converter.popup.is_some())
            {
                self.converter.converter.execute(command);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_lists_find_what_is_typed() {
        let mut converter = Converter::new(Region::new("US"));
        converter.set_category(crate::converter::Category::Length);
        let names: Vec<&str> = converter.units().iter().map(|unit| unit.name).collect();
        let at = |name: &str| names.iter().position(|n| *n == name);
        let mut search = TypeAhead::default();
        let mut find = |c, now, current: &str| search.find(c, now, at(current).unwrap(), &names);
        // A new search starts after the current unit, ignoring case, as in WinUI.
        assert_eq!(find('k', 10.0, "Centimeters"), at("Kilometers"));
        assert_eq!(find('K', 12.0, "Centimeters"), at("Kilometers"));
        // Letters typed within a second add up and keep the unit while it matches.
        assert_eq!(find('m', 14.0, "Kilometers"), at("Miles"));
        assert_eq!(find('i', 14.5, "Miles"), at("Miles"));
        assert_eq!(find('c', 15.0, "Miles"), at("Microns"));
        // After a pause, the same letter goes on to the next unit, round to the top.
        assert_eq!(find('m', 17.0, "Microns"), at("Millimeters"));
        assert_eq!(find('m', 19.0, "Nautical miles"), at("Microns"));
        // Nothing matches: the unit stays.
        assert_eq!(find('q', 19.5, "Microns"), None);
    }

    #[test]
    fn a_space_only_continues_a_search() {
        let mut converter = Converter::new(Region::new("US"));
        converter.set_category(crate::converter::Category::Area);
        let names: Vec<&str> = converter.units().iter().map(|unit| unit.name).collect();
        let at = |name: &str| names.iter().position(|n| *n == name).unwrap();
        let mut search = TypeAhead::default();
        // A leading space opens the list or picks the unit instead, as in WinUI.
        assert!(!search.takes(' ', 1.0));
        let (mut current, mut now) = (at("Square meters"), 1.0);
        for c in "square f".chars() {
            now += 0.2;
            assert!(search.takes(c, now), "{c:?}");
            current = search.find(c, now, current, &names).unwrap_or(current);
        }
        assert_eq!(current, at("Square feet"));
        // A second after the last letter, a space no longer continues the search.
        assert!(!search.takes(' ', now + 1.5));
    }

    fn length_converter() -> ConverterState {
        let mut converter = Converter::new(Region::new("US"));
        converter.set_category(crate::converter::Category::Length);
        ConverterState {
            converter,
            popup: None,
            flash: None,
            type_ahead: Default::default(),
        }
    }

    fn pressed(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    fn top_unit(state: &ConverterState) -> &'static str {
        state.converter.units()[state.converter.unit1()].name
    }

    #[test]
    fn enter_picks_the_unit_typed_to_in_an_open_list() {
        let mut state = length_converter();
        state.popup = Some(UnitPopup {
            top: true,
            anchor: Rect::ZERO,
            scroll_pending: false,
            jump: None,
        });
        // `k` and Enter in the same frame, while the focus is still on Centimeters.
        let focused = Some(Id::new("converter-unit-item").with(state.converter.unit1()));
        let typed = Event::Text("k".into());
        assert_eq!(state.unit_picker_key(&typed, focused, 1.0), None);
        assert_eq!(top_unit(&state), "Centimeters");
        let enter = state.unit_picker_key(&pressed(Key::Enter), focused, 1.0);
        assert_eq!(enter, Some(Key::Enter));
        assert!(state.popup.is_none());
        assert_eq!(top_unit(&state), "Kilometers");
    }

    #[test]
    fn a_closed_picker_ignores_enter_while_searching() {
        let mut state = length_converter();
        let picker = Some(unit_picker_id(true));
        let typed = Event::Text("k".into());
        assert_eq!(state.unit_picker_key(&typed, picker, 1.0), None);
        assert_eq!(top_unit(&state), "Kilometers");
        // Within the search, Enter doesn't open the list, as in WinUI.
        let enter = pressed(Key::Enter);
        assert_eq!(state.unit_picker_key(&enter, picker, 1.5), Some(Key::Enter));
        // A second after the last letter, it opens the list again.
        assert_eq!(state.unit_picker_key(&enter, picker, 2.6), None);
        // A space within the search is part of it, not a press of the picker.
        let space = Event::Text(" ".into());
        assert_eq!(state.unit_picker_key(&typed, picker, 5.0), None);
        assert_eq!(state.unit_picker_key(&space, picker, 5.2), Some(Key::Space));
    }

    #[test]
    fn star_rows_respect_minimums() {
        // Enough room: in proportion.
        let rows = star_rows(448.0, &[(56.0, 56.0), (32.0, 32.0), (360.0, 0.0)]);
        assert_eq!(rows, [56.0, 32.0, 360.0]);
        // Too little room: the minimums win and the last row shrinks.
        let rows = star_rows(100.0, &[(56.0, 56.0), (32.0, 32.0), (360.0, 0.0)]);
        assert_eq!(rows, [56.0, 32.0, 12.0]);
    }

    #[test]
    fn automation_names_follow_the_active_field() {
        let from = ("Format_ValueFrom", "InputUnit_Name");
        let into = ("Format_ValueTo", "OutputUnit_Name");
        let mut converter = Converter::new(Region::new("US"));
        assert_eq!(automation_keys(&converter, true), from);
        assert_eq!(automation_keys(&converter, false), into);
        // Typing into the bottom value makes it the source, and Windows swaps the names.
        converter.activate(false);
        assert_eq!(automation_keys(&converter, true), into);
        assert_eq!(automation_keys(&converter, false), from);
        converter.activate(true);
        assert_eq!(automation_keys(&converter, true), from);
        assert_eq!(automation_keys(&converter, false), into);
    }

    #[test]
    fn typed_characters() {
        assert_eq!(char_command('7'), Some(ConverterCommand::Digit(7)));
        assert_eq!(char_command(','), Some(ConverterCommand::Decimal));
        assert_eq!(char_command('-'), Some(ConverterCommand::Negate));
        assert_eq!(char_command('+'), None);
    }

    #[test]
    fn escape_is_ce() {
        // Windows' converter gives `CE` two keys: Delete and Esc.
        assert_eq!(key_command(Key::Delete), Some(ConverterCommand::Clear));
        assert_eq!(key_command(Key::Escape), Some(ConverterCommand::Clear));
        assert_eq!(
            key_command(Key::Backspace),
            Some(ConverterCommand::Backspace)
        );
        assert_eq!(key_command(Key::F9), Some(ConverterCommand::Negate));
        assert_eq!(key_command(Key::Enter), None);
    }
}
