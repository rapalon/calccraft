//! The Scientific mode: the display, the angle unit and F-E, the memory row, the
//! Trigonometry and Function menus and the 5 × 7 keys with `2nd`. The layout follows
//! `Calculator.xaml` in its `Scientific` state, `CalculatorScientificAngleButtons.xaml`
//! and `CalculatorScientificOperators.xaml`.

use eframe::egui::{
    Align, Color32, FontFamily, FontId, Id, Key, Modifiers, Painter, Pos2, Rect, Sense, Shape,
    Stroke, TextFormat, Ui, WidgetType, pos2, text::LayoutJob, vec2,
};

use super::converter_view::star_rows;
use super::date_view::popup_frame;
use super::icons::{
    ICON_BACKSPACE, ICON_CHEVRON_DOWN, ICON_DIVIDE, ICON_EQUALS, ICON_MINUS, ICON_MULTIPLY,
    ICON_PLUS, icon_label,
};
use super::strings::{tr, tr_format};
use super::{
    CalculatorApp, Flyout, GAP, Kind, MARGIN, MEMORY_BUTTON_MAX_WIDTH, Panel, RADIUS, access,
    command_name, format, negate_label, semibold, symbols, text,
};
use crate::engine::{Angle, Calculator, Command, Func, Op, Trig, TrigFunction};

/// The results' font size and their row's weight and minimum height for a window
/// height (`ResultsS`, `ResultsM` and `ResultsL` in `Calculator.xaml`, whose middle
/// trigger the Scientific mode moves to 544 px). Windows counts its 32 px title bar,
/// which CalcCraft leaves to the system.
fn results_size(window_height: f32) -> (f32, f32, f32) {
    let height = window_height + 32.0;
    if height >= 800.0 {
        (72.0, 72.0, 108.0)
    } else if height >= 544.0 {
        (46.0, 72.0, 72.0)
    } else {
        (26.0, 42.0, 42.0)
    }
}

/// From this size of the keys (with the menu row) on, they get larger (`Medium` and
/// `Large` in `CalculatorScientificOperators.xaml`).
const MEDIUM_KEYPAD: (f32, f32) = (527.0, 523.0);
const LARGE_KEYPAD: (f32, f32) = (878.0, 851.0);

/// The font sizes and the menu row of one of the keypad's size states.
#[derive(Clone, Copy)]
pub(super) struct Sizes {
    /// Digits (`NumericButtonStyle18`, 24 and 38).
    pub(super) digit: f32,
    /// Most keys (`CalcOperatorCaptionSize` and its larger variants).
    pub(super) operator: f32,
    pub(super) negate: f32,
    /// The trigonometric functions and `hyp` in their menu (`CalcOperatorCaptionSizeSmall`).
    pub(super) trig: f32,
    /// The Function menu's symbols (`CalcOperatorTextIconCaptionSize`).
    pub(super) text_icon: f32,
    /// The menu row: its height, text, icon and chevron (`OperatorPanelButton*Style`).
    pub(super) menu: [f32; 4],
    /// The menus' least sizes (`TrigGrid`, `FuncGrid`).
    pub(super) trig_menu: (f32, f32),
    pub(super) function_menu: (f32, f32),
}

impl Sizes {
    pub(super) fn for_keypad(keypad: Rect) -> Self {
        let at_least = |(w, h): (f32, f32)| keypad.width() >= w && keypad.height() >= h;
        if at_least(LARGE_KEYPAD) {
            Sizes {
                digit: 38.0,
                operator: 34.0,
                negate: 48.0,
                trig: 34.0,
                text_icon: 38.0,
                menu: [70.0, 24.0, 24.0, 16.0],
                trig_menu: (516.0, 192.0),
                function_menu: (387.0, 192.0),
            }
        } else if at_least(MEDIUM_KEYPAD) {
            Sizes {
                digit: 24.0,
                operator: 20.0,
                negate: 24.0,
                trig: 20.0,
                text_icon: 22.0,
                menu: [70.0, 16.0, 20.0, 10.0],
                trig_menu: (480.0, 144.0),
                function_menu: (360.0, 144.0),
            }
        } else {
            Sizes {
                digit: 18.0,
                operator: 14.0,
                negate: 14.0,
                trig: 12.0,
                text_icon: 16.0,
                menu: [44.0, 12.0, 16.0, 12.0],
                trig_menu: (258.0, 96.0),
                function_menu: (194.0, 96.0),
            }
        }
    }
}

/// The menus of the row above the keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Menu {
    Trigonometry,
    Function,
}

impl Menu {
    fn id(self) -> Id {
        Id::new("scientific-menu").with(self)
    }
}

/// An open menu, the button it opened from, and the size state of the keys.
#[derive(Clone, Copy)]
struct MenuPopup {
    menu: Menu,
    anchor: Rect,
    sizes: Sizes,
}

/// The Scientific mode's engine and what its UI keeps.
pub(super) struct ScientificState {
    pub(super) calc: Calculator,
    /// `2nd`: the first column shows its second functions (`ShiftButton`).
    shift: bool,
    /// The Trigonometry menu's `2nd` and `hyp` (`TrigShiftButton`, `HypButton`).
    trig_inverse: bool,
    trig_hyperbolic: bool,
    popup: Option<MenuPopup>,
}

impl ScientificState {
    pub(super) fn new() -> Self {
        Self {
            calc: Calculator::with_precedence(),
            shift: false,
            trig_inverse: false,
            trig_hyperbolic: false,
            popup: None,
        }
    }

    /// Closes the Trigonometry or Function menu. Returns whether one was open.
    pub(super) fn close_popup(&mut self) -> bool {
        self.popup.take().is_some()
    }

    pub(super) fn popup_open(&self) -> bool {
        self.popup.is_some()
    }

    /// The command a typed character stands for, from the keyboard shortcuts of
    /// `CalculatorScientificOperators.xaml`: `%` is mod, `^` xʸ, `!` n!, `@` √x, `#` x³,
    /// `|` |x|, `[` ⌊x⌋ and `]` ⌈x⌉. Letters are keys (`key_command`).
    pub(super) fn char_command(c: char) -> Option<Command> {
        Some(match c {
            '0'..='9' => Command::Digit(c as u8 - b'0'),
            '.' | ',' => Command::Decimal,
            '+' => Command::Operator(Op::Add),
            '-' => Command::Operator(Op::Subtract),
            '*' => Command::Operator(Op::Multiply),
            '/' => Command::Operator(Op::Divide),
            '%' => Command::Operator(Op::Mod),
            '^' => Command::Operator(Op::Power),
            '=' => Command::Equals,
            '(' => Command::OpenParen,
            ')' => Command::CloseParen,
            '!' => Command::Function(Func::Factorial),
            '@' => Command::Function(Func::SquareRoot),
            '#' => Command::Function(Func::Cube),
            '|' => Command::Function(Func::Abs),
            '[' => Command::Function(Func::Floor),
            ']' => Command::Function(Func::Ceil),
            _ => return None,
        })
    }

    /// The letter and function keys (`VirtualKey`, with `Shift` and `Ctrl` chords): S, O,
    /// T, U, I and J are sin, cos, tan, sec, csc and cot, inverse with `Shift` and
    /// hyperbolic with `Ctrl`. The angle unit keys belong to the three hidden unit
    /// buttons, each of which switches to the next unit, so F4 (the DEG button's key)
    /// picks radians, F5 gradians and F3 degrees.
    pub(super) fn key_command(key: Key, modifiers: Modifiers) -> Option<Command> {
        if modifiers.alt {
            return None;
        }
        let (ctrl, shift) = (modifiers.command, modifiers.shift);
        let trig = |function| {
            Some(Command::Function(Func::Trig(Trig {
                function,
                inverse: shift,
                hyperbolic: ctrl,
            })))
        };
        match key {
            Key::S => return trig(TrigFunction::Sin),
            Key::O => return trig(TrigFunction::Cos),
            Key::T => return trig(TrigFunction::Tan),
            Key::U => return trig(TrigFunction::Sec),
            Key::I => return trig(TrigFunction::Csc),
            Key::J => return trig(TrigFunction::Cot),
            _ => {}
        }
        Some(match (ctrl, shift, key) {
            // Windows' Shift and Ctrl shortcuts have no function keys.
            (false, false, Key::F3) => Command::Angle(Angle::Degrees),
            (false, false, Key::F4) => Command::Angle(Angle::Radians),
            (false, false, Key::F5) => Command::Angle(Angle::Gradians),
            // The `exp` button has both: the constructor of `CalculatorScientificOperators`
            // adds `E` to the resources' `X`, and the shortcut manager keeps both.
            (false, false, Key::X | Key::E) => Command::Exponent,
            (false, false, Key::M) => Command::Function(Func::Dms),
            (false, false, Key::V) => Command::ToggleNotation,
            (false, false, Key::R) => Command::Function(Func::Reciprocal),
            (false, true, Key::R) => Command::Random,
            (false, false, Key::L) => Command::Function(Func::Log),
            (false, true, Key::L) => Command::Operator(Op::LogBase),
            (false, false, Key::N) => Command::Function(Func::Ln),
            (true, false, Key::N) => Command::Function(Func::EToX),
            (false, false, Key::P) => Command::Pi,
            (false, true, Key::E) => Command::Euler,
            (false, false, Key::Y) => Command::Operator(Op::Power),
            (true, false, Key::Y) => Command::Operator(Op::Root),
            (false, false, Key::Q) => Command::Function(Func::Square),
            (false, false, Key::G) => Command::Function(Func::TwoToX),
            (true, false, Key::G) => Command::Function(Func::TenToX),
            (false, false, Key::B) => Command::Function(Func::CubeRoot),
            _ => return None,
        })
    }

    /// The trigonometric function the menu shows for `function`.
    fn trig(&self, function: TrigFunction) -> Command {
        Command::Function(Func::Trig(Trig {
            function,
            inverse: self.trig_inverse,
            hyperbolic: self.trig_hyperbolic,
        }))
    }
}

/// The id of a key of the Scientific keypad (they ignore Enter, like the Standard keys).
pub(super) fn key_id(index: usize) -> Id {
    Id::new("scientific-key").with(index)
}

/// How many keys `key_id` numbers.
pub(super) const KEY_COUNT: usize = 35;

/// The unit button's label and the name of its button (Windows has one per unit and
/// shows the current one).
fn angle_label(angle: Angle) -> (&'static str, &'static str) {
    match angle {
        Angle::Degrees => ("DEG", "degButton.AutomationName"),
        Angle::Radians => ("RAD", "radButton.AutomationName"),
        Angle::Gradians => ("GRAD", "gradButton.AutomationName"),
    }
}

impl CalculatorApp {
    // ---------------------------------------------------------------- layout

    /// Draws the Scientific mode in `area`, the window below the header (without a
    /// docked history and memory panel).
    pub(super) fn draw_scientific(&mut self, ui: &mut Ui, area: Rect, now: f64) {
        // The rows under the header (`Calculator.xaml` in its `Scientific` state): the
        // expression (20*), the result, the angle unit and F-E (32*, at least 32 px),
        // the memory row (32*) and the keys (276*).
        let (font, result_weight, result_min) = results_size(ui.max_rect().height());
        let heights = star_rows(
            area.height(),
            &[
                (20.0, 0.0),
                (result_weight, result_min),
                (32.0, 32.0),
                (32.0, 30.0),
                (276.0, 0.0),
            ],
        );
        let mut y = area.top();
        let mut row = |i: usize| {
            let rect = Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), heights[i]));
            y += heights[i];
            rect
        };
        let (expression, result, angle_row, memory_row, keypad) =
            (row(0), row(1), row(2), row(3), row(4));
        let display = expression.union(result);

        let center_y = result.center().y - result.height() * 0.14;
        self.draw_display_sized(ui, display, expression.bottom() - 15.0, font, center_y);
        if matches!(self.flyout, Flyout::History | Flyout::Memory) {
            // Windows makes the list as high as the keys (`RowHeight =
            // NumpadPanel.ActualHeight`) and lays the rest of its flyout over the whole window:
            // the unit, F-E and memory rows stay in view, out of the Tab order, the memory
            // buttons disabled, and a click anywhere above the list (from the top of the
            // window, header included) closes it (`OnHistoryFlyOutTapped`).
            access::draw_covered(ui, || {
                self.draw_angle_row(ui, angle_row);
                self.draw_memory_row(ui, memory_row, now, false);
            });
            let above = Rect::from_min_max(
                pos2(area.left(), ui.max_rect().top()),
                memory_row.right_bottom(),
            );
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
            self.draw_flyout(ui, area.with_min_y(keypad.top()), panel);
            return;
        }
        self.draw_angle_row(ui, angle_row);
        if !self.docked {
            self.context_menu = None;
        }
        self.draw_memory_row(ui, memory_row, now, true);
        // `Margin="3,0,3,3"` around the keys.
        let keypad = Rect::from_min_max(
            keypad.min + vec2(MARGIN - 1.0, 0.0),
            keypad.max - vec2(MARGIN - 1.0, MARGIN - 1.0),
        );
        self.draw_scientific_keys(ui, keypad, now);
    }

    /// The Trigonometry and Function menus, drawn over everything else.
    pub(super) fn draw_scientific_popups(&mut self, ui: &Ui) {
        if let Some(popup) = self.scientific.popup {
            self.draw_scientific_menu(ui, popup);
        }
    }

    // ------------------------------------------------------------- angle row

    /// The angle unit button, which cycles DEG → RAD → GRAD, and the F-E toggle, in the
    /// first two of the memory row's columns (`Margin="3,0,3,0"`).
    fn draw_angle_row(&mut self, ui: &Ui, rect: Rect) {
        let p = self.palette;
        let calc = &self.scientific.calc;
        // Enabled in the error state too, as in Windows (see `Calculator::is_enabled`).
        let enabled = calc.is_enabled(Command::ToggleNotation);
        let inner = rect.shrink2(vec2(MARGIN - 1.0, 0.0));
        let column = (inner.width() / 6.01).min(MEMORY_BUTTON_MAX_WIDTH);
        let cell = |i: usize| {
            Rect::from_min_size(
                pos2(inner.left() + i as f32 * column, inner.top()),
                vec2(column, inner.height()),
            )
            .shrink(1.0)
        };

        let angle = calc.angle();
        let (label, name) = angle_label(angle);
        let next = match angle {
            Angle::Degrees => Angle::Radians,
            Angle::Radians => Angle::Gradians,
            Angle::Gradians => Angle::Degrees,
        };
        let id = Id::new("scientific-angle");
        let label = text(label, 13.0, FontFamily::Proportional);
        if self.button(ui, cell(0), id, Kind::Subtle, enabled, label, tr(name)) {
            self.scientific.calc.execute(Command::Angle(next));
        }

        // F-E is a toggle with WinUI's 16 × 3 px accent pipe under it while it's on.
        let on = self.scientific.calc.notation();
        let button = cell(1);
        let name = tr("ftoeButton.AutomationName");
        let info = access::item(WidgetType::Checkbox, name, on);
        let response = access::control(
            ui,
            p,
            button,
            Id::new("scientific-fe"),
            enabled,
            RADIUS,
            info,
        );
        let pressed = enabled && response.is_pointer_button_down_on();
        let fill = if pressed {
            p.subtle_pressed
        } else if enabled && response.hovered() {
            p.subtle_hover
        } else {
            Color32::TRANSPARENT
        };
        let color = if !enabled {
            p.text_disabled
        } else if pressed {
            p.text_secondary
        } else {
            p.text
        };
        let painter = ui.painter();
        painter.rect_filled(button, RADIUS, fill);
        let galley = painter.layout_job(text("F-E", 13.0, FontFamily::Proportional));
        painter.galley(button.center() - galley.size() / 2.0, galley, color);
        if on {
            let pipe = Rect::from_center_size(
                pos2(button.center().x, button.bottom() - 1.5),
                vec2(16.0, 3.0),
            );
            let pipe_color = if enabled { p.accent } else { p.text_disabled };
            painter.rect_filled(pipe, 1.5, pipe_color);
        }
        if enabled && response.clicked() {
            self.scientific.calc.execute(Command::ToggleNotation);
        }
    }

    // ------------------------------------------------------------------ keys

    /// The menu row (Trigonometry and Function) and the 5 × 7 keys.
    fn draw_scientific_keys(&mut self, ui: &Ui, rect: Rect, now: f64) {
        let p = self.palette;
        let sizes = Sizes::for_keypad(rect);
        let [menu_height, menu_font, glyph_size, chevron_size] = sizes.menu;
        let menu_row = Rect::from_min_size(rect.min, vec2(rect.width(), menu_height));
        ui.painter().hline(
            rect.x_range().expand(MARGIN - 1.0),
            rect.top() + 0.5,
            Stroke::new(1.0, p.divider),
        );
        let enabled = !self.scientific.calc.has_error();
        let mut left = menu_row.left();
        for menu in [Menu::Trigonometry, Menu::Function] {
            let (label, name) = match menu {
                Menu::Trigonometry => (tr("trigButton.Text"), tr("trigButton.AutomationName")),
                Menu::Function => (tr("funcButton.Text"), tr("funcButton.AutomationName")),
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
                .scientific
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
                Menu::Trigonometry => paint_triangle_icon(painter, icon_center, glyph_size, color),
                Menu::Function => {
                    let f = painter.layout_job(italic("ƒ", glyph_size));
                    painter.galley(icon_center - f.size() / 2.0, f, color);
                }
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
                    self.scientific.popup = None;
                } else {
                    self.scientific.popup = Some(MenuPopup {
                        menu,
                        anchor: button,
                        sizes,
                    });
                    self.opened_from(ui.ctx(), menu.id());
                }
            }
        }

        let keys = Rect::from_min_max(pos2(rect.left(), menu_row.bottom()), rect.max);
        self.draw_scientific_keypad(ui, keys, sizes, now);
    }

    fn draw_scientific_keypad(&mut self, ui: &Ui, rect: Rect, sizes: Sizes, now: f64) {
        use Command::*;
        let calc = &self.scientific.calc;
        let shift = self.scientific.shift;
        // One key shows `C` while nothing is typed and `CE` otherwise.
        let clear = if calc.is_input_empty() {
            Clear
        } else {
            ClearEntry
        };
        // The first column's second functions while `2nd` is on (`InvRow1`).
        let first = |normal: Command, second: Command| if shift { second } else { normal };
        #[rustfmt::skip]
        let keys: [Option<Command>; KEY_COUNT] = [
            None, Some(Pi), Some(Euler), Some(clear), Some(Backspace),
            Some(first(Function(Func::Square), Function(Func::Cube))), Some(Function(Func::Reciprocal)), Some(Function(Func::Abs)), Some(Exponent), Some(Operator(Op::Mod)),
            Some(first(Function(Func::SquareRoot), Function(Func::CubeRoot))), Some(OpenParen), Some(CloseParen), Some(Function(Func::Factorial)), Some(Operator(Op::Divide)),
            Some(first(Operator(Op::Power), Operator(Op::Root))), Some(Digit(7)), Some(Digit(8)), Some(Digit(9)), Some(Operator(Op::Multiply)),
            Some(first(Function(Func::TenToX), Function(Func::TwoToX))), Some(Digit(4)), Some(Digit(5)), Some(Digit(6)), Some(Operator(Op::Subtract)),
            Some(first(Function(Func::Log), Operator(Op::LogBase))), Some(Digit(1)), Some(Digit(2)), Some(Digit(3)), Some(Operator(Op::Add)),
            Some(first(Function(Func::Ln), Function(Func::EToX))), Some(Function(Func::Negate)), Some(Digit(0)), Some(Decimal), Some(Equals),
        ];
        let open_parentheses = calc.open_parentheses();
        let width = (rect.width() - 4.0 * GAP) / 5.0;
        let height = (rect.height() - 6.0 * GAP) / 7.0;
        for (i, command) in keys.into_iter().enumerate() {
            let (row, col) = ((i / 5) as f32, (i % 5) as f32);
            let cell = Rect::from_min_size(
                rect.min + vec2(col * (width + GAP), row * (height + GAP)),
                vec2(width, height),
            );
            let Some(command) = command else {
                self.draw_shift_key(ui, cell, sizes);
                continue;
            };
            let second = shift && i % 5 == 0;
            let kind = match command {
                Digit(_) | Decimal | Function(Func::Negate) => Kind::Digit,
                Equals => Kind::Accent,
                _ if second => Kind::Emphasized,
                _ => Kind::Operator,
            };
            let label = self.scientific_label(command, sizes);
            let calc = &self.scientific.calc;
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
                self.scientific.calc.execute(command);
                #[cfg(feature = "support-prompt")]
                self.check_calculation_trigger(command);
                // The second functions turn `2nd` off again (`ShiftButton_Uncheck`).
                if second {
                    self.scientific.shift = false;
                }
            }
            // The count sits right of and below the `(` (`ParenthesisCount`).
            if command == OpenParen && open_parentheses > 0 {
                let p = self.palette;
                let color = if enabled { p.text } else { p.text_disabled };
                ui.painter().text(
                    cell.center() + vec2(9.0, 8.0),
                    eframe::egui::Align2::CENTER_CENTER,
                    open_parentheses.to_string(),
                    FontId::new(12.0, semibold()),
                    color,
                );
            }
        }
    }

    /// `2nd`, a toggle that fills with the accent color while it's on
    /// (`CaptionToggleEmphasizedButtonStyle`).
    fn draw_shift_key(&mut self, ui: &Ui, cell: Rect, sizes: Sizes) {
        let enabled = !self.scientific.calc.has_error();
        let on = self.scientific.shift;
        let label = second_label(sizes.operator);
        let name = tr("shiftButton.AutomationName");
        if self.toggle(ui, cell, key_id(0), enabled, on, label, name) {
            self.scientific.shift = !on;
        }
    }

    /// A toggle key of the Scientific mode (`2nd`, and `2nd` and `hyp` in the
    /// Trigonometry menu): an operator key, filled with the accent color while it's on.
    /// Returns whether it was clicked.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn toggle(
        &self,
        ui: &Ui,
        rect: Rect,
        id: Id,
        enabled: bool,
        on: bool,
        label: LayoutJob,
        name: &str,
    ) -> bool {
        // Off, it looks like any operator key, but tells Narrator it's a toggle.
        let kind = if on { Kind::Accent } else { Kind::Operator };
        let info = access::item(WidgetType::Checkbox, name, on);
        let response = access::control(ui, self.palette, rect, id, enabled, RADIUS, info);
        self.paint_key(ui, rect, kind, enabled, &response, false, label);
        enabled && response.clicked()
    }

    /// A key's label for the Scientific keypad. Windows draws most of them from the
    /// Calculator's own icon font, which may only be used inside Windows; these are the
    /// same formulas in Segoe UI, with an italic x.
    fn scientific_label(&self, command: Command, sizes: Sizes) -> LayoutJob {
        let size = sizes.operator;
        let plain = |s: &str| text(s, size, FontFamily::Proportional);
        match command {
            Command::Digit(d) => text(&d.to_string(), sizes.digit, FontFamily::Proportional),
            Command::Decimal => text(".", sizes.digit, FontFamily::Proportional),
            Command::Function(Func::Negate) => negate_label(sizes.negate),
            Command::Function(func) => function_label(func, size),
            Command::Operator(Op::Divide) => icon_label(ICON_DIVIDE, size),
            Command::Operator(Op::Multiply) => icon_label(ICON_MULTIPLY, size),
            Command::Operator(Op::Subtract) => icon_label(ICON_MINUS, size),
            Command::Operator(Op::Add) => icon_label(ICON_PLUS, size),
            Command::Operator(Op::Mod) => plain("mod"),
            Command::Operator(Op::Power) => superscript(italic("x", size), "y", size),
            Command::Operator(Op::Root) => {
                let mut job = superscript(LayoutJob::default(), "y", size);
                job.append("√", 0.0, format(size, FontFamily::Proportional));
                job.append(
                    "x",
                    0.0,
                    TextFormat {
                        italics: true,
                        ..format(size, FontFamily::Proportional)
                    },
                );
                job
            }
            Command::Operator(Op::LogBase) => log_base_label(size),
            Command::Equals => icon_label(ICON_EQUALS, size),
            Command::Backspace => icon_label(ICON_BACKSPACE, size),
            Command::Pi => plain("π"),
            Command::Euler => plain("e"),
            Command::Exponent => plain("exp"),
            Command::OpenParen => plain("("),
            Command::CloseParen => plain(")"),
            Command::ClearEntry => plain("CE"),
            Command::Clear => plain("C"),
            _ => LayoutJob::default(),
        }
    }

    // ------------------------------------------------------------- menus

    /// The open Trigonometry menu (`2nd` and `hyp` beside six functions) or Function
    /// menu (six functions), under its button, over a backdrop that closes it.
    fn draw_scientific_menu(&mut self, ui: &Ui, popup: MenuPopup) {
        let p = self.palette;
        let full = ui.max_rect();
        let backdrop = ui.interact(full, Id::new("scientific-menu-backdrop"), Sense::CLICK);
        access::set_covered(ui.ctx(), false);
        let mut close = backdrop.clicked() || backdrop.secondary_clicked();
        let sizes = popup.sizes;
        let (columns, (width, height)) = match popup.menu {
            Menu::Trigonometry => (4, sizes.trig_menu),
            Menu::Function => (3, sizes.function_menu),
        };
        // `OperatorPanelFlyoutStyle`: 2 px padding, right under the button (as measured
        // against Windows: 1 px).
        let outer = vec2(width, height) + vec2(6.0, 6.0);
        let left = popup
            .anchor
            .left()
            .min(full.right() - 4.0 - outer.x)
            .max(full.left() + 4.0);
        let rect = Rect::from_min_size(pos2(left, popup.anchor.bottom() + 1.0), outer);
        popup_frame(p, ui.painter(), rect, p.popup);
        ui.interact(rect, Id::new("scientific-menu-panel"), Sense::CLICK);
        let grid = rect.shrink(3.0);
        let (w, h) = (grid.width() / columns as f32, grid.height() / 2.0);
        let cell = |column: usize, row: usize| {
            Rect::from_min_size(
                grid.min + vec2(column as f32 * w, row as f32 * h),
                vec2(w, h),
            )
            .shrink(1.0)
        };
        let mut chosen = None;
        match popup.menu {
            Menu::Trigonometry => {
                let toggles = [
                    (
                        second_label(sizes.operator),
                        "trigShiftButton.AutomationName",
                        self.scientific.trig_inverse,
                    ),
                    (
                        text("hyp", sizes.trig, FontFamily::Proportional),
                        "hypButton.AutomationName",
                        self.scientific.trig_hyperbolic,
                    ),
                ];
                for (row, (label, name, on)) in toggles.into_iter().enumerate() {
                    let id = Id::new("scientific-trig-toggle").with(row);
                    if self.take_popup_focus(row == 0) {
                        ui.memory_mut(|memory| memory.request_focus(id));
                    }
                    if self.toggle(ui, cell(0, row), id, true, on, label, tr(name)) {
                        if row == 0 {
                            self.scientific.trig_inverse = !on;
                        } else {
                            self.scientific.trig_hyperbolic = !on;
                        }
                    }
                }
                for (i, function) in TrigFunction::ALL.into_iter().enumerate() {
                    let command = self.scientific.trig(function);
                    let Command::Function(Func::Trig(trig)) = command else {
                        continue;
                    };
                    let label = text(&trig_label(trig), sizes.trig, FontFamily::Proportional);
                    let id = Id::new("scientific-trig").with(i);
                    if self.button(
                        ui,
                        cell(1 + i % 3, i / 3),
                        id,
                        Kind::Operator,
                        true,
                        label,
                        command_name(command),
                    ) {
                        chosen = Some(command);
                    }
                }
            }
            Menu::Function => {
                let functions = [
                    Command::Function(Func::Abs),
                    Command::Function(Func::Floor),
                    Command::Function(Func::Ceil),
                    Command::Random,
                    Command::Function(Func::Dms),
                    Command::Function(Func::Degrees),
                ];
                for (i, command) in functions.into_iter().enumerate() {
                    let label = match command {
                        Command::Random => text("rand", sizes.trig, FontFamily::Proportional),
                        Command::Function(func) => function_label(func, sizes.text_icon),
                        _ => LayoutJob::default(),
                    };
                    let id = Id::new("scientific-function").with(i);
                    if self.take_popup_focus(i == 0) {
                        ui.memory_mut(|memory| memory.request_focus(id));
                    }
                    if self.button(
                        ui,
                        cell(i % 3, i / 3),
                        id,
                        Kind::Operator,
                        true,
                        label,
                        command_name(command),
                    ) {
                        chosen = Some(command);
                    }
                }
            }
        }
        // A function closes both menus and turns the Trigonometry menu's toggles off
        // (`FlyoutButton_Clicked`).
        if let Some(command) = chosen {
            self.scientific.calc.execute(command);
            self.scientific.trig_inverse = false;
            self.scientific.trig_hyperbolic = false;
            close = true;
        }
        if close {
            self.scientific.popup = None;
        }
    }
}

// ------------------------------------------------------------------ labels

/// A label of pieces, each upright or in italics (the variables).
pub(super) fn formula(pieces: &[(&str, bool)], size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    for &(piece, italics) in pieces {
        job.append(
            piece,
            0.0,
            TextFormat {
                italics,
                ..format(size, FontFamily::Proportional)
            },
        );
    }
    job
}

pub(super) fn italic(s: &str, size: f32) -> LayoutJob {
    formula(&[(s, true)], size)
}

/// `job` followed by a raised, smaller italic `letter`: the exponent of `xʸ` and `10ˣ`,
/// or the index of `ʸ√x`. Unicode's modifier letters (ʸ, ˣ) are too small in Segoe UI.
pub(super) fn superscript(mut job: LayoutJob, letter: &str, size: f32) -> LayoutJob {
    job.append(
        letter,
        0.0,
        TextFormat {
            italics: true,
            valign: Align::TOP,
            ..format(size * 0.7, FontFamily::Proportional)
        },
    );
    job
}

/// `2ⁿᵈ`.
pub(super) fn second_label(size: f32) -> LayoutJob {
    text("2ⁿᵈ", size, FontFamily::Proportional)
}

/// `logᵧx`, with a lowered small y (Unicode has no subscript y).
pub(super) fn log_base_label(size: f32) -> LayoutJob {
    let mut job = text("log", size, FontFamily::Proportional);
    job.append(
        "y",
        0.0,
        TextFormat {
            italics: true,
            valign: Align::BOTTOM,
            ..format(size * 0.7, FontFamily::Proportional)
        },
    );
    job.append(
        "x",
        0.0,
        TextFormat {
            italics: true,
            ..format(size, FontFamily::Proportional)
        },
    );
    job
}

/// A label with brackets from Segoe UI Symbol, which Segoe UI lacks: `⌊x⌋`, `⌈x⌉`, `∛x`.
pub(super) fn symbol_formula(before: &str, after: &str, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(before, 0.0, format(size, symbols()));
    job.append(
        "x",
        0.0,
        TextFormat {
            italics: true,
            ..format(size, FontFamily::Proportional)
        },
    );
    job.append(after, 0.0, format(size, symbols()));
    job
}

/// A function key's label.
pub(super) fn function_label(func: Func, size: f32) -> LayoutJob {
    let x = ("x", true);
    match func {
        Func::Square => formula(&[x, ("²", false)], size),
        Func::Cube => formula(&[x, ("³", false)], size),
        Func::Reciprocal => formula(&[("¹⁄", false), x], size),
        Func::Abs => formula(&[("|", false), x, ("|", false)], size),
        Func::SquareRoot => formula(&[("²√", false), x], size),
        Func::CubeRoot => symbol_formula("∛", "", size),
        Func::TenToX => superscript(formula(&[("10", false)], size), "x", size),
        Func::TwoToX => superscript(formula(&[("2", false)], size), "x", size),
        Func::EToX => superscript(formula(&[("e", false)], size), "x", size),
        Func::Log => text("log", size, FontFamily::Proportional),
        Func::Ln => text("ln", size, FontFamily::Proportional),
        Func::Factorial => formula(&[("n", true), ("!", false)], size),
        Func::Floor => symbol_formula("⌊", "⌋", size),
        Func::Ceil => symbol_formula("⌈", "⌉", size),
        Func::Dms => text("→dms", size, FontFamily::Proportional),
        Func::Degrees => text("→deg", size, FontFamily::Proportional),
        Func::Trig(trig) => text(&trig_label(trig), size, FontFamily::Proportional),
        _ => LayoutJob::default(),
    }
}

/// `sin`, `sin⁻¹`, `sinh` or `sinh⁻¹`, as the Trigonometry menu shows them.
pub(super) fn trig_label(trig: Trig) -> String {
    let mut label = trig.function.label().to_string();
    if trig.hyperbolic {
        label.push('h');
    }
    if trig.inverse {
        label.push_str("⁻¹");
    }
    label
}

/// The Trigonometry menu's icon: a right triangle with its angle marked. Windows draws
/// it from the Calculator's own icon font.
pub(super) fn paint_triangle_icon(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let s = size / 16.0;
    let stroke = Stroke::new(1.0 * s.max(1.0), color);
    let at = |x: f32, y: f32| center + vec2(x * s, y * s);
    painter.add(Shape::closed_line(
        vec![at(-7.0, 6.0), at(7.0, 6.0), at(7.0, -6.0)],
        stroke,
    ));
    // The angle at the left corner.
    let mut arc = Vec::new();
    let angle = (12.0f32).atan2(14.0);
    for step in 0..=6 {
        let a = -angle * step as f32 / 6.0;
        arc.push(at(-7.0 + 5.0 * a.cos(), 6.0 + 5.0 * a.sin()));
    }
    painter.add(Shape::line(arc, stroke));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_characters() {
        let command = ScientificState::char_command;
        assert_eq!(command('%'), Some(Command::Operator(Op::Mod)));
        assert_eq!(command('^'), Some(Command::Operator(Op::Power)));
        assert_eq!(command('!'), Some(Command::Function(Func::Factorial)));
        assert_eq!(command('#'), Some(Command::Function(Func::Cube)));
        assert_eq!(command('['), Some(Command::Function(Func::Floor)));
        // Letters are keys, not characters.
        assert_eq!(command('s'), None);
    }

    #[test]
    fn letter_keys() {
        let key = ScientificState::key_command;
        let sin = |inverse, hyperbolic| {
            Some(Command::Function(Func::Trig(Trig {
                function: TrigFunction::Sin,
                inverse,
                hyperbolic,
            })))
        };
        assert_eq!(key(Key::S, Modifiers::NONE), sin(false, false));
        assert_eq!(key(Key::S, Modifiers::SHIFT), sin(true, false));
        assert_eq!(key(Key::S, Modifiers::COMMAND), sin(false, true));
        assert_eq!(
            key(Key::S, Modifiers::COMMAND | Modifiers::SHIFT),
            sin(true, true)
        );
        assert_eq!(key(Key::P, Modifiers::NONE), Some(Command::Pi));
        assert_eq!(key(Key::E, Modifiers::SHIFT), Some(Command::Euler));
        assert_eq!(key(Key::E, Modifiers::NONE), Some(Command::Exponent));
        assert_eq!(key(Key::X, Modifiers::NONE), Some(Command::Exponent));
        assert_eq!(key(Key::F4, Modifiers::SHIFT), None);
        assert_eq!(key(Key::R, Modifiers::SHIFT), Some(Command::Random));
        assert_eq!(
            key(Key::Y, Modifiers::COMMAND),
            Some(Command::Operator(Op::Root))
        );
        assert_eq!(
            key(Key::G, Modifiers::COMMAND),
            Some(Command::Function(Func::TenToX))
        );
        // Each unit key belongs to a button that switches to the next unit.
        assert_eq!(
            key(Key::F4, Modifiers::NONE),
            Some(Command::Angle(Angle::Radians))
        );
        assert_eq!(
            key(Key::F3, Modifiers::NONE),
            Some(Command::Angle(Angle::Degrees))
        );
        // Ctrl+M, Ctrl+P … stay the memory keys.
        assert_eq!(key(Key::M, Modifiers::COMMAND), None);
        assert_eq!(key(Key::P, Modifiers::COMMAND), None);
    }

    #[test]
    fn results_sizes() {
        assert_eq!(results_size(500.0).0, 26.0);
        assert_eq!(results_size(520.0).0, 46.0);
        assert_eq!(results_size(800.0).0, 72.0);
    }
}
