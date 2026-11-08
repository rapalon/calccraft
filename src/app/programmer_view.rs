//! The Programmer mode: the value in four radixes, the word size, the bitwise and shift
//! operators, and the bit toggling keypad. The layout follows `Calculator.xaml` in its
//! `Programmer` state, `CalculatorProgrammerOperators.xaml` (the radix list),
//! `CalculatorProgrammerDisplayPanel.xaml` (keypad switch and word size),
//! `CalculatorProgrammerRadixOperators.xaml` (the keys and the Bitwise and Bit shift
//! menus) and `CalculatorProgrammerBitFlipPanel.xaml` (the bits).

use eframe::egui::{
    Align2, Color32, FontFamily, FontId, Id, Key, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui,
    WidgetType, pos2, text::LayoutJob, vec2,
};

use super::converter_view::star_rows;
use super::date_view::popup_frame;
use super::icons::{
    self, ICON_BACKSPACE, ICON_CHEVRON_DOWN, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_DIVIDE,
    ICON_EQUALS, ICON_MINUS, ICON_MULTIPLY, ICON_PLUS, icon_label,
};
use super::strings::{tr, tr_format};
use super::{
    CalculatorApp, ContextMenu, Flyout, FlyoutAction, GAP, Kind, MARGIN, MEMORY_BUTTON_MAX_WIDTH,
    MenuTarget, Panel, RADIUS, access, command_name, negate_label, numbers, paint_radio, semibold,
    text,
};
use crate::engine::{Calculator, Command, Func, Op, Radix, WordSize};

/// The results' font size and their row's weight and minimum height for a window
/// height (`ResultsS`, `ResultsM` and `ResultsL` in `Calculator.xaml`, whose trigger
/// the Programmer mode moves to 640 px). Windows counts its 32 px title bar, which
/// CalcCraft leaves to the system.
fn results_size(window_height: f32) -> (f32, f32, f32) {
    let height = window_height + 32.0;
    if height >= 800.0 {
        (72.0, 72.0, 108.0)
    } else if height >= 640.0 {
        (46.0, 72.0, 72.0)
    } else {
        (26.0, 42.0, 42.0)
    }
}

/// The radix list is at most this high (`MaxHeight="244"`).
const RADIX_LIST_MAX_HEIGHT: f32 = 244.0;
/// From this keypad size on, the keys and the menu row get larger (`Medium`, `Large`).
const MEDIUM_KEYPAD: (f32, f32) = (630.0, 437.0);
const LARGE_KEYPAD: (f32, f32) = (1053.0, 729.0);

/// What the two shift keys and `<` and `>` do, as picked in the Bit shift menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShiftMode {
    Arithmetic,
    Logical,
    RotateCircular,
    RotateCarry,
}

impl ShiftMode {
    const ALL: [ShiftMode; 4] = [
        ShiftMode::Arithmetic,
        ShiftMode::Logical,
        ShiftMode::RotateCircular,
        ShiftMode::RotateCarry,
    ];

    fn label(self) -> &'static str {
        tr(match self {
            ShiftMode::Arithmetic => "arithmeticShiftButton.Content",
            ShiftMode::Logical => "logicalShiftButton.Content",
            ShiftMode::RotateCircular => "rotateCircularButton.Content",
            ShiftMode::RotateCarry => "rotateCarryShiftButton.Content",
        })
    }

    /// The commands of the left and right shift keys. A logical left shift is the
    /// ordinary one.
    fn commands(self) -> (Command, Command) {
        match self {
            ShiftMode::Arithmetic => (
                Command::Operator(Op::LeftShift),
                Command::Operator(Op::RightShift),
            ),
            ShiftMode::Logical => (
                Command::Operator(Op::LeftShift),
                Command::Operator(Op::RightShiftLogical),
            ),
            ShiftMode::RotateCircular => (
                Command::Function(Func::RotateLeft),
                Command::Function(Func::RotateRight),
            ),
            ShiftMode::RotateCarry => (
                Command::Function(Func::RotateLeftCarry),
                Command::Function(Func::RotateRightCarry),
            ),
        }
    }
}

/// The menus of the row above the keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Menu {
    Bitwise,
    BitShift,
}

impl Menu {
    fn id(self) -> Id {
        Id::new("programmer-menu").with(self)
    }
}

/// An open menu and the button it opened from.
#[derive(Clone, Copy)]
struct MenuPopup {
    menu: Menu,
    anchor: Rect,
    /// The keys under the menu row, whose size picks the Bitwise menu's size.
    keypad: Rect,
}

/// The Programmer mode's engine and what its UI keeps.
pub(super) struct ProgrammerState {
    pub(super) calc: Calculator,
    /// The bit toggling keypad replaces the keys (`IsBitFlipChecked`).
    pub(super) bit_flip: bool,
    shift: ShiftMode,
    popup: Option<MenuPopup>,
}

impl ProgrammerState {
    pub(super) fn new() -> Self {
        Self {
            calc: Calculator::programmer(),
            bit_flip: false,
            shift: ShiftMode::Arithmetic,
            popup: None,
        }
    }

    /// Closes the Bitwise or Bit shift menu. Returns whether one was open.
    pub(super) fn close_popup(&mut self) -> bool {
        self.popup.take().is_some()
    }

    pub(super) fn popup_open(&self) -> bool {
        self.popup.is_some()
    }

    /// The command a typed character stands for in the Programmer mode, from the
    /// keyboard shortcuts of `CalculatorProgrammerRadixOperators.xaml`: the letters are
    /// hex digits, `%` is the remainder, `&` `|` `^` `~` are AND, OR, XOR and NOT, `.`
    /// and `\` are NAND and NOR, and `<` and `>` shift as the Bit shift menu says.
    pub(super) fn char_command(&self, c: char) -> Option<Command> {
        let (left, right) = self.shift.commands();
        Some(match c {
            '0'..='9' | 'a'..='f' | 'A'..='F' => Command::Digit(c.to_digit(16)? as u8),
            '+' => Command::Operator(Op::Add),
            '-' => Command::Operator(Op::Subtract),
            '*' => Command::Operator(Op::Multiply),
            '/' => Command::Operator(Op::Divide),
            '%' => Command::Operator(Op::Mod),
            '&' => Command::Operator(Op::And),
            '|' => Command::Operator(Op::Or),
            '^' => Command::Operator(Op::Xor),
            '.' => Command::Operator(Op::Nand),
            '\\' => Command::Operator(Op::Nor),
            '~' => Command::Function(Func::Not),
            '<' => left,
            '>' => right,
            '(' => Command::OpenParen,
            ')' => Command::CloseParen,
            '=' => Command::Equals,
            _ => return None,
        })
    }

    /// The function keys: F5 to F8 pick the radix, F2, F3, F4 and F12 the word size
    /// (the shortcuts of the four word size buttons, which Windows triggers even while
    /// they're hidden: each one switches to the next size).
    pub(super) fn key_command(&self, key: Key) -> Option<Command> {
        let command = match key {
            Key::F5 => Command::Radix(Radix::Hex),
            Key::F6 => Command::Radix(Radix::Decimal),
            Key::F7 => Command::Radix(Radix::Octal),
            Key::F8 => Command::Radix(Radix::Binary),
            Key::F2 => Command::WordSize(WordSize::Dword),
            Key::F3 => Command::WordSize(WordSize::Word),
            Key::F4 => Command::WordSize(WordSize::Byte),
            Key::F12 => Command::WordSize(WordSize::Qword),
            _ => return None,
        };
        // The radix and word size already chosen do nothing (and don't finish the number
        // being typed): the radix's radio button is already checked, and Windows'
        // `ValueBitLength` only acts on a change.
        let chosen = match command {
            Command::Radix(radix) => self.calc.radix() == Some(radix),
            Command::WordSize(word) => self.calc.word_size() == Some(word),
            _ => false,
        };
        (!chosen).then_some(command)
    }
}

/// The id of a key of the Programmer keypad (they ignore Enter, like the Standard keys).
pub(super) fn key_id(index: usize) -> Id {
    Id::new("programmer-key").with(index)
}

pub(super) fn memory_store_id() -> Id {
    Id::new("programmer-ms")
}

/// Text for Narrator in a radix other than decimal: every digit on its own
/// (`GetNarratorStringReadRawNumbers`), so "FF" isn't read as a word.
pub(super) fn spell_digits(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        out.push(c);
        if c.is_ascii_hexdigit() {
            out.push(' ');
        }
    }
    out.trim_end().to_string()
}

/// A radix value as the list shows it: decimal in the region's format.
fn localized(radix: Radix, value: &str) -> String {
    if radix == Radix::Decimal {
        numbers().localize(value)
    } else {
        value.to_string()
    }
}

impl CalculatorApp {
    // ---------------------------------------------------------------- layout

    /// Draws the Programmer mode in `area`, the window below the header (without a
    /// docked memory list).
    pub(super) fn draw_programmer(&mut self, ui: &mut Ui, area: Rect, now: f64) {
        // The rows under the header (`Calculator.xaml` in its `Programmer` state): the
        // expression (20*), the result, the radix list (96*, at least 96 px), the
        // keypad switches and word size (32*) and the keys (268*).
        let (font, result_weight, result_min) = results_size(ui.max_rect().height());
        let heights = star_rows(
            area.height(),
            &[
                (20.0, 0.0),
                (result_weight, result_min),
                (96.0, 96.0),
                (32.0, 30.0),
                (268.0, 0.0),
            ],
        );
        let mut y = area.top();
        let mut row = |i: usize| {
            let rect = Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), heights[i]));
            y += heights[i];
            rect
        };
        let (expression, result, radixes, controls, keypad) =
            (row(0), row(1), row(2), row(3), row(4));
        let display = expression.union(result);

        // As measured in Windows: the expression sits on the bottom of its row, and the
        // number a little above the middle of its row.
        let center_y = result.center().y - result.height() * 0.14;
        self.draw_display_sized(ui, display, expression.bottom() - 15.0, font, center_y);
        if matches!(self.flyout, Flyout::History | Flyout::Memory) {
            // As in the other modes, the list is as tall as the keys (`RowHeight =
            // NumpadPanel.ActualHeight`): the radix list and the row under it stay in view,
            // with `MS` disabled, and a click anywhere above the list (from the top of the
            // window, header included) closes the flyout.
            access::draw_covered(ui, || {
                self.draw_radix_list(ui, radixes);
                self.draw_programmer_controls(ui, controls, now);
            });
            let above = Rect::from_min_max(
                pos2(area.left(), ui.max_rect().top()),
                controls.right_bottom(),
            );
            if ui
                .interact(above, Id::new("display"), Sense::CLICK)
                .clicked()
            {
                self.flyout = Flyout::None;
            }
            self.draw_flyout(ui, area.with_min_y(keypad.top()), Panel::Memory);
            return;
        }
        if !self.docked {
            self.context_menu = self
                .context_menu
                .filter(|menu| matches!(menu.target, MenuTarget::Radix(_)));
        }
        self.draw_radix_list(ui, radixes);
        self.draw_programmer_controls(ui, controls, now);
        // `Margin="3,0,3,3"` around the keys.
        let keypad = Rect::from_min_max(
            keypad.min + vec2(MARGIN - 1.0, 0.0),
            keypad.max - vec2(MARGIN - 1.0, MARGIN - 1.0),
        );
        if self.programmer.bit_flip {
            self.draw_bit_panel(ui, keypad);
        } else {
            self.draw_programmer_keys(ui, keypad, now);
        }
    }

    /// The menus and the radix list's context menu, drawn over everything else.
    pub(super) fn draw_programmer_popups(&mut self, ui: &Ui) {
        if let Some(popup) = self.programmer.popup {
            self.draw_menu_popup(ui, popup);
        }
        if let Some(menu) = self.context_menu
            && matches!(menu.target, MenuTarget::Radix(_))
        {
            let action = self.draw_context_menu(ui, menu);
            if action.is_some() {
                self.context_menu = None;
            }
            if let Some(FlyoutAction::CopyRadix(radix)) = action
                && let Some(values) = self.programmer.calc.radix_values()
            {
                let index = Radix::ALL.iter().position(|&r| r == radix).unwrap_or(0);
                // Without the digit groups, like Windows' `GetRawDisplayValue`.
                let raw: String = values[index]
                    .chars()
                    .filter(|&c| c != ' ' && c != ',')
                    .collect();
                ui.ctx().copy_text(raw);
            }
        }
    }

    // ------------------------------------------------------------ radix list

    /// HEX, DEC, OCT and BIN with the value in each; the selected radix has the accent
    /// pill. A click picks the radix, a right click offers Copy.
    fn draw_radix_list(&mut self, ui: &Ui, rect: Rect) {
        let p = self.palette;
        let painter = ui.painter();
        let calc = &self.programmer.calc;
        let values = calc.radix_values().unwrap_or_default();
        let current = calc.radix();
        let font = FontId::proportional(12.0);
        let rect = rect.shrink2(vec2(MARGIN, 0.0));
        let height = rect.height().min(RADIX_LIST_MAX_HEIGHT);
        let list = Rect::from_center_size(rect.center(), vec2(rect.width(), height));
        // The value column starts after the 3 px pill, the 48 px label column (with its
        // 8 px padding) and a 4 px margin; long binary numbers wrap there.
        let value_left = 3.0 + 48.0 + 4.0;
        let value_width = (list.width() - value_left - 4.0).max(10.0);
        let galleys: Vec<_> = Radix::ALL
            .iter()
            .zip(&values)
            .map(|(&radix, value)| {
                painter.layout(localized(radix, value), font.clone(), p.text, value_width)
            })
            .collect();
        // Four equal rows over a 4 px gap; the binary row grows when its wrapped value
        // doesn't fit.
        let base = (list.height() - 4.0) / 4.0;
        let binary = galleys[3].size().y.max(base);
        let others = ((list.height() - 4.0 - binary) / 3.0).max(16.0);

        let mut chosen = None;
        let mut menu = None;
        let mut top = list.top();
        for (i, (&radix, galley)) in Radix::ALL.iter().zip(galleys).enumerate() {
            let row_height = if i == 3 { binary } else { others };
            let row = Rect::from_min_size(pos2(list.left(), top), vec2(list.width(), row_height));
            top += row_height;
            let selected = current == Some(radix);
            let name_key = match radix {
                Radix::Hex => "Format_HexButtonValue",
                Radix::Decimal => "Format_DecButtonValue",
                Radix::Octal => "Format_OctButtonValue",
                Radix::Binary => "Format_BinButtonValue",
            };
            let spoken = if radix == Radix::Decimal {
                localized(radix, &values[i])
            } else {
                spell_digits(&values[i])
            };
            let name = tr_format(name_key, &[&spoken]);
            let info = access::item(WidgetType::RadioButton, &name, selected);
            let id = Id::new("radix").with(i);
            let response = access::control(ui, p, row, id, true, RADIUS, info);
            let pressed = response.is_pointer_button_down_on();
            let fill = if pressed {
                p.subtle_pressed
            } else if response.hovered() {
                p.subtle_hover
            } else {
                Color32::TRANSPARENT
            };
            painter.rect_filled(row, RADIUS, fill);
            if selected {
                let pill_height = (row.height() - 10.0).clamp(4.0, 16.0);
                let pill = Rect::from_min_size(
                    pos2(row.left(), row.center().y - pill_height / 2.0),
                    vec2(3.0, pill_height),
                );
                painter.rect_filled(pill, 1.5, p.accent);
            }
            let color = if pressed { p.text_secondary } else { p.text };
            painter.text(
                pos2(row.left() + 3.0 + 8.0, row.center().y),
                Align2::LEFT_CENTER,
                radix.label(),
                font.clone(),
                color,
            );
            let value_pos = pos2(
                row.left() + value_left,
                row.center().y - galley.size().y / 2.0,
            );
            painter.galley(value_pos, galley, color);
            if response.clicked() && !selected {
                chosen = Some(radix);
            }
            if response.secondary_clicked() {
                let pos = response.interact_pointer_pos().unwrap_or(row.center());
                menu = Some(ContextMenu {
                    target: MenuTarget::Radix(radix),
                    pos,
                });
            }
        }
        if let Some(radix) = chosen {
            self.programmer.calc.execute(Command::Radix(radix));
        }
        if menu.is_some() {
            self.context_menu = menu;
        }
    }

    // ------------------------------------------------------- keypad controls

    /// The row under the radix list: the full keypad and bit toggling keypad switches,
    /// the word size button in the middle, and MS and M˅ on the right (`M˅` is gone when
    /// the memory list is docked).
    fn draw_programmer_controls(&mut self, ui: &Ui, rect: Rect, now: f64) {
        let p = self.palette;
        let ctx = ui.ctx().clone();
        let has_error = self.programmer.calc.has_error();
        // Star columns as in `Calculator.xaml` and `CalculatorProgrammerDisplayPanel.xaml`:
        // six 80 px columns and a 0.01* spacer that takes the rest.
        let inner = rect.shrink2(vec2(MARGIN - 1.0, 0.0));
        let column = (inner.width() / 6.01).min(MEMORY_BUTTON_MAX_WIDTH);
        let cell = |left: f32| {
            Rect::from_min_size(pos2(left, inner.top()), vec2(column, inner.height())).shrink(1.0)
        };

        // The keypad switches are radio buttons with WinUI's 16 × 3 px accent "carrot".
        for (i, bit_flip) in [false, true].into_iter().enumerate() {
            let button = cell(inner.left() + i as f32 * column);
            let selected = self.programmer.bit_flip == bit_flip;
            let name = if bit_flip {
                tr("bitFlip.AutomationName")
            } else {
                tr("fullKeypad.AutomationName")
            };
            let info = access::item(WidgetType::RadioButton, name, selected);
            let id = Id::new("programmer-keypad-switch").with(i);
            let response = access::control(ui, p, button, id, true, RADIUS, info);
            let pressed = response.is_pointer_button_down_on();
            let fill = if pressed {
                p.subtle_pressed
            } else if response.hovered() {
                p.subtle_hover
            } else {
                Color32::TRANSPARENT
            };
            let painter = ui.painter();
            painter.rect_filled(button, RADIUS, fill);
            let color = if pressed { p.text_secondary } else { p.text };
            let icon_center = pos2(button.center().x, button.center().y - 2.0);
            if bit_flip {
                paint_bits_icon(painter, icon_center, color);
            } else {
                paint_dialpad_icon(painter, icon_center, color);
            }
            if selected {
                let carrot = Rect::from_center_size(
                    pos2(button.center().x, button.bottom() - 1.5),
                    vec2(16.0, 3.0),
                );
                painter.rect_filled(carrot, 1.5, p.accent);
            }
            if response.clicked() && !selected {
                self.programmer.bit_flip = bit_flip;
                self.programmer.close_popup();
            }
        }

        // The word size button cycles QWORD → DWORD → WORD → BYTE.
        let word = self.programmer.calc.word_size().unwrap_or(WordSize::Qword);
        let label = word.label();
        let label_width = ui
            .painter()
            .layout_no_wrap(label.to_string(), FontId::proportional(12.0), p.text)
            .size()
            .x;
        let button = Rect::from_center_size(
            rect.center(),
            vec2((label_width + 24.0).max(80.0), inner.height() - 2.0),
        );
        let name = tr(match word {
            WordSize::Qword => "qwordButton.AutomationName",
            WordSize::Dword => "dwordButton.AutomationName",
            WordSize::Word => "wordButton.AutomationName",
            WordSize::Byte => "byteButton.AutomationName",
        });
        let id = Id::new("programmer-word-size");
        let label = text(label, 12.0, FontFamily::Proportional);
        if self.button(ui, button, id, Kind::Subtle, !has_error, label, name) {
            self.programmer.calc.execute(Command::WordSize(word.next()));
        }

        // MS, and M˅ for the memory flyout.
        let store = cell(if self.docked {
            inner.right() - column
        } else {
            inner.right() - 2.0 * column
        });
        let command = Command::MemoryStore;
        // An open flyout disables it, like the memory row's buttons (`EnableMemoryControls`).
        let enabled = self.programmer.calc.is_enabled(command) && !self.keypad_disabled();
        let flash = self.is_flashing(command, now);
        if self.button_flash(
            ui,
            store,
            memory_store_id(),
            Kind::Subtle,
            enabled,
            text("MS", 12.0, FontFamily::Proportional),
            command_name(command),
            flash,
        ) {
            self.programmer.calc.execute(command);
        }
        if self.docked {
            return;
        }
        let enabled = !self.programmer.calc.memory().is_empty() && !has_error;
        let name = if self.flyout == Flyout::Memory {
            tr("MemoryButton_Close")
        } else {
            tr("MemoryButton_Open")
        };
        let id = Id::new("memory-list");
        let label = Self::memory_list_label();
        if self.button(
            ui,
            cell(inner.right() - column),
            id,
            Kind::Subtle,
            enabled,
            label,
            name,
        ) {
            self.toggle_flyout(Flyout::Memory);
            self.opened_from(&ctx, id);
        }
    }

    // ------------------------------------------------------------------ keys

    /// Sizes of the keys' labels and of the menu row (`Small`, `Medium` and `Large` in
    /// `CalculatorProgrammerRadixOperators.xaml`): digits, hex letters, operators, and
    /// the menu row's height, text, icon and chevron.
    fn keypad_sizes(keypad: Rect) -> ([f32; 3], [f32; 4]) {
        let at_least = |(w, h): (f32, f32)| keypad.width() >= w && keypad.height() >= h;
        if at_least(LARGE_KEYPAD) {
            ([38.0, 38.0, 34.0], [70.0, 24.0, 24.0, 16.0])
        } else if at_least(MEDIUM_KEYPAD) {
            ([24.0, 24.0, 20.0], [70.0, 16.0, 20.0, 10.0])
        } else {
            ([18.0, 14.0, 14.0], [44.0, 12.0, 16.0, 12.0])
        }
    }

    /// The menu row (Bitwise and Bit shift) and the 5 × 6 keys.
    fn draw_programmer_keys(&mut self, ui: &Ui, rect: Rect, now: f64) {
        let p = self.palette;
        let (key_sizes, menu_sizes) = Self::keypad_sizes(rect);
        let [menu_height, menu_font, glyph_size, chevron_size] = menu_sizes;
        let menu_row = Rect::from_min_size(rect.min, vec2(rect.width(), menu_height));
        ui.painter().hline(
            rect.x_range().expand(MARGIN - 1.0),
            rect.top() + 0.5,
            Stroke::new(1.0, p.divider),
        );
        let enabled = !self.programmer.calc.has_error();
        let mut left = menu_row.left();
        for menu in [Menu::Bitwise, Menu::BitShift] {
            let (label, name) = match menu {
                Menu::Bitwise => (tr("bitwiseButton.Text"), tr("bitwiseButton.AutomationName")),
                Menu::BitShift => (
                    tr("bitShiftButton.Text"),
                    tr("bitShiftButton.AutomationName"),
                ),
            };
            let galley = ui.painter().layout_no_wrap(
                label.to_string(),
                FontId::proportional(menu_font),
                Color32::PLACEHOLDER,
            );
            let chevron = ui
                .painter()
                .layout_job(icon_label(ICON_CHEVRON_DOWN, chevron_size));
            // Padding 8, the icon, the text and the chevron 8 px apart, padding 8.
            let width = 8.0 + glyph_size + 8.0 + galley.size().x + 8.0 + chevron.size().x + 8.0;
            let button = Rect::from_min_size(
                pos2(left + 1.0, menu_row.top() + 1.0),
                vec2(width, menu_row.height() - 2.0),
            );
            left = button.right() + 1.0;
            let open = self
                .programmer
                .popup
                .is_some_and(|popup| popup.menu == menu);
            let info = access::button(enabled, name);
            let response = access::control(ui, p, button, menu.id(), enabled, RADIUS, info);
            let pressed = enabled && response.is_pointer_button_down_on();
            let fill = if pressed {
                p.subtle_pressed
            } else if open || (enabled && response.hovered()) {
                p.subtle_hover
            } else {
                Color32::TRANSPARENT
            };
            let color = if !enabled {
                p.text_disabled
            } else if pressed || open {
                p.text_secondary
            } else {
                p.text
            };
            let painter = ui.painter();
            painter.rect_filled(button, RADIUS, fill);
            let mut x = button.left() + 8.0;
            let icon_center = pos2(x + glyph_size / 2.0, button.center().y);
            match menu {
                Menu::Bitwise => paint_gate_icon(painter, icon_center, glyph_size, color),
                Menu::BitShift => paint_shift_icon(painter, icon_center, glyph_size, color),
            }
            x += glyph_size + 8.0;
            let text_size = galley.size();
            painter.galley(
                pos2(x, button.center().y - text_size.y / 2.0),
                galley,
                color,
            );
            x += text_size.x + 8.0;
            let chevron_size = chevron.size();
            painter.galley(
                pos2(x, button.center().y - chevron_size.y / 2.0),
                chevron,
                color,
            );
            if enabled && response.clicked() {
                if open {
                    self.programmer.popup = None;
                } else {
                    self.programmer.popup = Some(MenuPopup {
                        menu,
                        anchor: button,
                        keypad: rect,
                    });
                    self.opened_from(ui.ctx(), menu.id());
                }
            }
        }

        let keys = Rect::from_min_max(pos2(rect.left(), menu_row.bottom()), rect.max);
        self.draw_programmer_keypad(ui, keys, key_sizes, now);
    }

    fn draw_programmer_keypad(&mut self, ui: &Ui, rect: Rect, sizes: [f32; 3], now: f64) {
        use Command::*;
        let [digit_size, letter_size, operator_size] = sizes;
        let calc = &self.programmer.calc;
        let (left, right) = self.programmer.shift.commands();
        // One key shows `C` while nothing is typed and `CE` otherwise.
        let clear = if calc.is_input_empty() {
            Clear
        } else {
            ClearEntry
        };
        #[rustfmt::skip]
        let keys: [Command; 30] = [
            Digit(10), left, right, clear, Backspace,
            Digit(11), OpenParen, CloseParen, Operator(Op::Mod), Operator(Op::Divide),
            Digit(12), Digit(7), Digit(8), Digit(9), Operator(Op::Multiply),
            Digit(13), Digit(4), Digit(5), Digit(6), Operator(Op::Subtract),
            Digit(14), Digit(1), Digit(2), Digit(3), Operator(Op::Add),
            Digit(15), Function(Func::Negate), Digit(0), Decimal, Equals,
        ];
        let open_parentheses = calc.open_parentheses();
        let width = (rect.width() - 4.0 * GAP) / 5.0;
        let height = (rect.height() - 5.0 * GAP) / 6.0;
        for (i, command) in keys.into_iter().enumerate() {
            let (row, col) = ((i / 5) as f32, (i % 5) as f32);
            let cell = Rect::from_min_size(
                rect.min + vec2(col * (width + GAP), row * (height + GAP)),
                vec2(width, height),
            );
            let kind = match command {
                Digit(_) | Decimal | Function(Func::Negate) => Kind::Digit,
                Equals => Kind::Accent,
                _ => Kind::Operator,
            };
            // `NumericButtonStyle` is regular, not semibold.
            let label = match command {
                Digit(d) if d >= 10 => text(
                    &char::from_digit(d.into(), 16)
                        .unwrap_or('0')
                        .to_ascii_uppercase()
                        .to_string(),
                    letter_size,
                    FontFamily::Proportional,
                ),
                Digit(d) => text(&d.to_string(), digit_size, FontFamily::Proportional),
                Decimal => text(".", digit_size, FontFamily::Proportional),
                Function(Func::Negate) => negate_label(operator_size),
                Operator(Op::LeftShift) | Function(Func::RotateLeft | Func::RotateLeftCarry) => {
                    Self::double_chevron(false, operator_size)
                }
                Operator(Op::RightShift | Op::RightShiftLogical)
                | Function(Func::RotateRight | Func::RotateRightCarry) => {
                    Self::double_chevron(true, operator_size)
                }
                Operator(Op::Divide) => icon_label(ICON_DIVIDE, operator_size),
                Operator(Op::Multiply) => icon_label(ICON_MULTIPLY, operator_size),
                Operator(Op::Subtract) => icon_label(ICON_MINUS, operator_size),
                Operator(Op::Add) => icon_label(ICON_PLUS, operator_size),
                Equals => icon_label(ICON_EQUALS, operator_size),
                Backspace => icon_label(ICON_BACKSPACE, operator_size),
                Operator(Op::Mod) => text("%", operator_size, FontFamily::Proportional),
                OpenParen => text("(", operator_size, FontFamily::Proportional),
                CloseParen => text(")", operator_size, FontFamily::Proportional),
                ClearEntry => text("CE", operator_size, FontFamily::Proportional),
                Clear => text("C", operator_size, FontFamily::Proportional),
                _ => LayoutJob::default(),
            };
            let calc = &self.programmer.calc;
            let enabled = calc.is_enabled(command);
            let flash = self.is_flashing(command, now);
            // `(` says how many parentheses are open.
            let counted;
            let name = if command == OpenParen && open_parentheses > 0 {
                counted = tr_format(
                    "Format_OpenParenthesisAutomationNamePrefix",
                    &[&open_parentheses.to_string()],
                );
                counted.as_str()
            } else {
                command_name(command)
            };
            if self.button_flash(ui, cell, key_id(i), kind, enabled, label, name, flash) {
                self.programmer.calc.execute(command);
                #[cfg(feature = "support-prompt")]
                self.check_calculation_trigger(command);
            }
            // The count sits right of and below the `(` (`ParenthesisCount`).
            if command == OpenParen && open_parentheses > 0 {
                let p = self.palette;
                let color = if enabled { p.text } else { p.text_disabled };
                ui.painter().text(
                    cell.center() + vec2(9.0, 8.0),
                    Align2::CENTER_CENTER,
                    open_parentheses.to_string(),
                    FontId::new(12.0, semibold()),
                    color,
                );
            }
        }
    }

    // ------------------------------------------------------------ menus

    /// The open Bitwise menu (six operator keys under the button) or Bit shift menu
    /// (four radio buttons centered under it), over a backdrop that closes it.
    fn draw_menu_popup(&mut self, ui: &Ui, popup: MenuPopup) {
        let p = self.palette;
        let full = ui.max_rect();
        let backdrop = ui.interact(full, Id::new("programmer-menu-backdrop"), Sense::CLICK);
        access::set_covered(ui.ctx(), false);
        let mut close = backdrop.clicked() || backdrop.secondary_clicked();
        match popup.menu {
            Menu::Bitwise => {
                // `BitwiseGrid`: at least 194 × 96 px with 2 px padding, 4 px under the button
                // (as measured; `Margin="0,-3,0,0"` against the flyout's own offset).
                let at_least =
                    |(w, h): (f32, f32)| popup.keypad.width() >= w && popup.keypad.height() >= h;
                let (size, font) = if at_least(LARGE_KEYPAD) {
                    (vec2(387.0, 192.0), 34.0)
                } else if at_least(MEDIUM_KEYPAD) {
                    (vec2(416.0, 144.0), 20.0)
                } else {
                    (vec2(194.0, 96.0), 14.0)
                };
                let outer = size + vec2(6.0, 6.0);
                let left = popup
                    .anchor
                    .left()
                    .min(full.right() - 4.0 - outer.x)
                    .max(full.left() + 4.0);
                let rect = Rect::from_min_size(pos2(left, popup.anchor.bottom() + 4.0), outer);
                popup_frame(p, ui.painter(), rect, p.popup);
                ui.interact(rect, Id::new("programmer-menu-panel"), Sense::CLICK);
                let grid = rect.shrink(3.0);
                let keys = [
                    (Command::Operator(Op::And), "AND"),
                    (Command::Operator(Op::Or), "OR"),
                    (Command::Function(Func::Not), "NOT"),
                    (Command::Operator(Op::Nand), "NAND"),
                    (Command::Operator(Op::Nor), "NOR"),
                    (Command::Operator(Op::Xor), "XOR"),
                ];
                let (w, h) = (grid.width() / 3.0, grid.height() / 2.0);
                for (i, (command, label)) in keys.into_iter().enumerate() {
                    let cell = Rect::from_min_size(
                        grid.min + vec2((i % 3) as f32 * w, (i / 3) as f32 * h),
                        vec2(w, h),
                    )
                    .shrink(1.0);
                    let id = Id::new("programmer-bitwise").with(i);
                    if self.take_popup_focus(i == 0) {
                        ui.memory_mut(|memory| memory.request_focus(id));
                    }
                    let label = text(label, font, FontFamily::Proportional);
                    if self.button(
                        ui,
                        cell,
                        id,
                        Kind::Operator,
                        true,
                        label,
                        command_name(command),
                    ) {
                        self.programmer.calc.execute(command);
                        close = true;
                    }
                }
            }
            Menu::BitShift => {
                // A 192 px wide panel with 12 px padding; long labels wrap.
                let font = FontId::proportional(14.0);
                let text_width = 192.0 - 24.0 - 28.0;
                let galleys: Vec<_> = ShiftMode::ALL
                    .iter()
                    .map(|mode| {
                        ui.painter().layout(
                            mode.label().to_string(),
                            font.clone(),
                            p.text,
                            text_width,
                        )
                    })
                    .collect();
                let widest = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
                let heights: Vec<f32> = galleys
                    .iter()
                    .map(|g| (g.size().y + 12.0).max(32.0))
                    .collect();
                let size = vec2(24.0 + 28.0 + widest, 24.0 + heights.iter().sum::<f32>());
                let left = (popup.anchor.center().x - size.x / 2.0)
                    .min(full.right() - 4.0 - size.x)
                    .max(full.left() + 4.0);
                let rect = Rect::from_min_size(pos2(left, popup.anchor.bottom() + 4.0), size);
                popup_frame(p, ui.painter(), rect, p.popup);
                ui.interact(rect, Id::new("programmer-menu-panel"), Sense::CLICK);
                let mut top = rect.top() + 12.0;
                for (i, (mode, galley)) in ShiftMode::ALL.into_iter().zip(galleys).enumerate() {
                    let item = Rect::from_min_size(
                        pos2(rect.left() + 12.0, top),
                        vec2(size.x - 24.0, heights[i]),
                    );
                    top += heights[i];
                    let selected = self.programmer.shift == mode;
                    let id = Id::new("programmer-shift").with(i);
                    let info = access::item(WidgetType::RadioButton, mode.label(), selected);
                    let response = access::control(ui, p, item, id, true, RADIUS, info);
                    if self.take_popup_focus(selected) {
                        response.request_focus();
                    }
                    let painter = ui.painter();
                    let first_line = galley.rows.first().map_or(16.0, |row| row.size.y);
                    let text_top = item.center().y - galley.size().y / 2.0;
                    let circle = pos2(item.left() + 10.0, text_top + first_line / 2.0);
                    paint_radio(
                        p,
                        painter,
                        circle,
                        selected,
                        response.hovered(),
                        response.is_pointer_button_down_on(),
                    );
                    painter.galley(pos2(item.left() + 28.0, text_top), galley, p.text);
                    if response.clicked() {
                        if !selected {
                            close = true;
                        }
                        self.programmer.shift = mode;
                    }
                }
            }
        }
        if close {
            self.programmer.popup = None;
        }
    }

    // ------------------------------------------------------------- bit panel

    /// The bit toggling keypad: 64 bits in four rows of four groups, the highest first,
    /// each group labeled with the number of its lowest bit. Bits beyond the word size
    /// are disabled.
    fn draw_bit_panel(&mut self, ui: &Ui, rect: Rect) {
        let p = self.palette;
        let calc = &self.programmer.calc;
        let bits = calc.bits();
        let word_bits = calc.word_size().map_or(64, WordSize::bits);
        // The divider over the panel (`Margin="-3,4,-3,1"`).
        ui.painter().hline(
            rect.x_range().expand(MARGIN - 1.0),
            rect.top() + 0.5,
            Stroke::new(1.0, p.divider),
        );
        let area = rect.with_min_y(rect.top() + 6.0);
        // 20 star columns: a 0.8* gutter before, between and after the four groups of
        // four 1* bits.
        let unit = area.width() / 20.0;
        let row_height = area.height() / 8.0;
        let label_size = if rect.width() < super::MIN_SIZE.x {
            10.0
        } else {
            12.0
        };
        let mut toggled = None;
        for row in 0..4 {
            let top = area.top() + 2.0 * row as f32 * row_height;
            for group in 0..4 {
                for j in 0..4 {
                    let bit = 63 - (row * 16 + group * 4 + j);
                    let left = area.left() + (0.8 + group as f32 * 4.8 + j as f32) * unit;
                    let cell = Rect::from_min_size(pos2(left, top), vec2(unit, row_height));
                    let enabled = (bit as u32) < word_bits;
                    let set = (bits >> bit) & 1 == 1;
                    let position = match bit {
                        0 => tr("LeastSignificantBit").to_string(),
                        _ if bit as u32 == word_bits - 1 => tr("MostSignificantBit").to_string(),
                        _ => tr_format("BitPosition", &[tr(&bit.to_string())]),
                    };
                    let name = tr_format(
                        "BitFlipItemAutomationName",
                        &[&position, if set { "1" } else { "0" }],
                    );
                    let info = access::item(WidgetType::Checkbox, &name, set);
                    let id = Id::new("programmer-bit").with(bit);
                    let response = access::control(ui, p, cell, id, enabled, RADIUS, info);
                    super::key_feedback(ui, &response);
                    let pressed = enabled && response.is_pointer_button_down_on();
                    let hovered = enabled && response.hovered();
                    let color = match (enabled, set) {
                        (false, _) => p.bit_disabled,
                        (true, true) if pressed => p.bit_pressed,
                        (true, true) if hovered => p.text,
                        (true, true) => p.accent,
                        (true, false) if pressed => p.bit_pressed,
                        (true, false) if hovered => p.bit_hover,
                        (true, false) => p.text,
                    };
                    ui.painter().text(
                        cell.center(),
                        Align2::CENTER_CENTER,
                        if set { "1" } else { "0" },
                        FontId::new(20.0, semibold()),
                        color,
                    );
                    if enabled && response.clicked() {
                        toggled = Some(bit as u8);
                    }
                    if j == 3 {
                        ui.painter().text(
                            pos2(cell.center().x, cell.bottom()),
                            Align2::CENTER_TOP,
                            bit.to_string(),
                            FontId::new(label_size, semibold()),
                            p.bit_label,
                        );
                    }
                }
            }
        }
        if let Some(bit) = toggled {
            self.programmer.calc.execute(Command::ToggleBit(bit));
        }
    }

    /// The shift keys' `«` and `»`: two chevron icons close together, like the Calculator
    /// font's glyphs.
    fn double_chevron(right: bool, size: f32) -> LayoutJob {
        let glyph = if right {
            ICON_CHEVRON_RIGHT
        } else {
            ICON_CHEVRON_LEFT
        };
        let mut job = LayoutJob::default();
        let mut format = super::format(size * 0.85, icons::family());
        // egui adds the spacing between the glyphs of one section.
        format.extra_letter_spacing = -size * 0.55;
        job.append(&format!("{glyph}{glyph}"), 0.0, format);
        job
    }

    /// The label of the `M˅` button that opens the memory flyout.
    pub(super) fn memory_list_label() -> LayoutJob {
        let mut label = text("M", 12.0, semibold());
        label.append(" ", 0.0, super::format(6.0, FontFamily::Proportional));
        label.append(
            &ICON_CHEVRON_DOWN.to_string(),
            0.0,
            super::format(8.0, icons::family()),
        );
        label
    }
}

// ------------------------------------------------------------------ icons
//
// Windows draws these from the Calculator's own icon font, which may only be used inside
// Windows (`CalculatorIcons.ttf`), so they are drawn here, after its glyphs.

/// The full keypad switch: a phone dial pad, three by three dots and one below.
fn paint_dialpad_icon(painter: &Painter, center: Pos2, color: Color32) {
    for row in 0..4 {
        for col in 0..3 {
            if row == 3 && col != 1 {
                continue;
            }
            let at = center + vec2((col as f32 - 1.0) * 4.8, (row as f32 - 1.5) * 4.0);
            painter.circle_filled(at, 1.0, color);
        }
    }
}

/// The bit toggling keypad switch: bits as dots, set ones filled.
fn paint_bits_icon(painter: &Painter, center: Pos2, color: Color32) {
    let dots = [
        (-3.0, -5.0, true),
        (3.0, -5.0, false),
        (-3.0, 0.0, false),
        (3.0, 0.0, true),
        (0.0, 5.0, false),
    ];
    for (x, y, filled) in dots {
        let at = center + vec2(x, y);
        if filled {
            painter.circle_filled(at, 1.9, color);
        } else {
            painter.circle_stroke(at, 1.6, Stroke::new(0.8, color));
        }
    }
}

/// The Bitwise menu's icon: a NAND gate with two inputs.
fn paint_gate_icon(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let s = size / 16.0;
    let stroke = Stroke::new(1.0 * s.max(1.0), color);
    let at = |x: f32, y: f32| center + vec2(x * s, y * s);
    // Inputs and output.
    painter.line_segment([at(-8.0, -3.0), at(-5.0, -3.0)], stroke);
    painter.line_segment([at(-8.0, 3.0), at(-5.0, 3.0)], stroke);
    painter.line_segment([at(5.5, 0.0), at(8.0, 0.0)], stroke);
    // The body: a flat back and a half circle.
    let radius = 5.5;
    let mut points = vec![
        at(-1.0, -radius),
        at(-5.0, -radius),
        at(-5.0, radius),
        at(-1.0, radius),
    ];
    for step in 0..=12 {
        let angle = std::f32::consts::FRAC_PI_2 - step as f32 * std::f32::consts::PI / 12.0;
        points.push(at(-1.0 + 3.5 * angle.cos(), radius * angle.sin()));
    }
    painter.add(Shape::line(points, stroke));
    // The inverting bubble.
    painter.circle_stroke(at(4.2, 0.0), 1.3 * s, stroke);
}

/// The Bit shift menu's icon: `»` above and `«` below, offset diagonally.
fn paint_shift_icon(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let s = size / 16.0;
    let stroke = Stroke::new(1.0 * s.max(1.0), color);
    let at = |x: f32, y: f32| center + vec2(x * s, y * s);
    for dx in [0.0, 3.5] {
        painter.add(Shape::line(
            vec![at(-7.0 + dx, -7.0), at(-3.5 + dx, -3.5), at(-7.0 + dx, 0.0)],
            stroke,
        ));
        painter.add(Shape::line(
            vec![at(7.0 - dx, 0.0), at(3.5 - dx, 3.5), at(7.0 - dx, 7.0)],
            stroke,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_characters() {
        let mut state = ProgrammerState::new();
        let command = |state: &ProgrammerState, c| state.char_command(c);
        assert_eq!(command(&state, 'a'), Some(Command::Digit(10)));
        assert_eq!(command(&state, 'F'), Some(Command::Digit(15)));
        assert_eq!(command(&state, '%'), Some(Command::Operator(Op::Mod)));
        // As in Windows, `.` and `\` are NAND and NOR here.
        assert_eq!(command(&state, '.'), Some(Command::Operator(Op::Nand)));
        assert_eq!(command(&state, '\\'), Some(Command::Operator(Op::Nor)));
        assert_eq!(command(&state, '~'), Some(Command::Function(Func::Not)));
        assert_eq!(command(&state, 'x'), None);
        // `<` and `>` follow the Bit shift menu.
        assert_eq!(
            command(&state, '>'),
            Some(Command::Operator(Op::RightShift))
        );
        state.shift = ShiftMode::Logical;
        assert_eq!(
            command(&state, '>'),
            Some(Command::Operator(Op::RightShiftLogical))
        );
        state.shift = ShiftMode::RotateCarry;
        assert_eq!(
            command(&state, '<'),
            Some(Command::Function(Func::RotateLeftCarry))
        );
    }

    #[test]
    fn function_keys() {
        let mut state = ProgrammerState::new();
        assert_eq!(state.key_command(Key::F5), Some(Command::Radix(Radix::Hex)));
        // Each word size key belongs to a button that switches to the next size.
        assert_eq!(
            state.key_command(Key::F2),
            Some(Command::WordSize(WordSize::Dword))
        );
        assert_eq!(state.key_command(Key::F9), None);

        // The radix and word size already chosen (DEC and QWORD) do nothing, so typing
        // goes on: 12, F6, 3 is 123, as in Windows.
        assert_eq!(state.key_command(Key::F6), None);
        assert_eq!(state.key_command(Key::F12), None);
        state.calc.execute(Command::WordSize(WordSize::Byte));
        assert_eq!(
            state.key_command(Key::F12),
            Some(Command::WordSize(WordSize::Qword))
        );
    }

    #[test]
    fn spelled_digits() {
        assert_eq!(spell_digits("FF 01"), "F F  0 1");
        assert_eq!(spell_digits("-12"), "-1 2");
    }

    #[test]
    fn results_sizes() {
        assert_eq!(results_size(500.0).0, 26.0);
        assert_eq!(results_size(700.0).0, 46.0);
        assert_eq!(results_size(800.0).0, 72.0);
    }
}
