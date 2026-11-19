//! The Graphing mode's keypad (`GraphingNumPad.xaml`): the Trigonometry, Inequalities and
//! Function menus over 5 × 7 keys that type into the equation box that had the focus
//! last, as `GetButtonOutput` writes them (`sin()` with the cursor inside, `root(x,n)`
//! with the n selected, …).

use eframe::egui::{
    self, Color32, Context, FontFamily, FontId, Id, Painter, Pos2, Rect, Shape, Stroke, TextEdit,
    Ui, pos2,
    text::{CCursor, CCursorRange, LayoutJob},
    vec2,
};

use super::date_view::popup_frame;
use super::graphing_view::{KeypadMenu, Popup, edit_id, insert_text, region_syntax};
use super::icons::{
    ICON_BACKSPACE, ICON_CHEVRON_DOWN, ICON_DIVIDE, ICON_EQUALS, ICON_MINUS, ICON_MULTIPLY,
    ICON_PLUS, ICON_RETURN, icon_label,
};
use super::math_label::Label;
use super::scientific_view::{
    Sizes, function_icon, function_label, italic, log_base_label, paint_triangle_icon, power_label,
    root_label, second_label, trig_label,
};
use super::strings::tr;
use super::{CalculatorApp, GAP, Kind, RADIUS, access, text};
use crate::engine::{Func, Trig, TrigFunction};

/// A key of the keypad.
#[derive(Clone, Copy, Debug, PartialEq)]
enum GraphKey {
    Shift,
    Clear,
    Backspace,
    Submit,
    /// Keys that type: what, where the cursor goes in it, and how much it selects.
    Type(&'static str, usize, usize),
}

impl CalculatorApp {
    pub(super) fn draw_graphing_keypad(&mut self, ui: &Ui, rect: Rect) {
        let sizes = Sizes::for_keypad(rect);
        let menu_row = Rect::from_min_size(rect.min, vec2(rect.width(), sizes.menu[0]));
        self.draw_graphing_menus(ui, menu_row, sizes);
        let keys = Rect::from_min_max(pos2(rect.left(), menu_row.bottom()), rect.max);
        self.draw_graphing_keys(ui, keys, sizes);
    }

    /// The landscape layout's keypad (see `Landscape`): the menu row in `menu_row`, under the
    /// equations, and the keys alone in `keys`, the other column.
    pub(super) fn draw_graphing_keypad_split(&mut self, ui: &Ui, menu_row: Rect, keys: Rect) {
        let sizes = Sizes::for_keypad(keys);
        self.draw_graphing_menus(ui, menu_row, sizes);
        self.draw_graphing_keys(ui, keys, sizes);
    }

    /// The menu buttons' labels, names and widths, for `sizes`.
    fn graphing_menus(ui: &Ui, sizes: Sizes) -> [(KeypadMenu, &'static str, &'static str, f32); 3] {
        let [_, menu_font, glyph_size, chevron_size] = sizes.menu;
        [
            KeypadMenu::Trigonometry,
            KeypadMenu::Inequalities,
            KeypadMenu::Function,
        ]
        .map(|menu| {
            let (label, name) = match menu {
                KeypadMenu::Trigonometry => {
                    (tr("trigButton.Text"), tr("trigButton.AutomationName"))
                }
                KeypadMenu::Inequalities => (
                    tr("inequalityButton.Text"),
                    tr("inequalityButton.AutomationName"),
                ),
                KeypadMenu::Function => (tr("funcButton.Text"), tr("funcButton.AutomationName")),
            };
            let painter = ui.painter();
            let text = painter
                .layout_no_wrap(
                    label.to_string(),
                    FontId::proportional(menu_font),
                    Color32::PLACEHOLDER,
                )
                .size()
                .x;
            let chevron = painter
                .layout_job(icon_label(ICON_CHEVRON_DOWN, chevron_size))
                .size()
                .x;
            // Padding 8, the icon, the text and the chevron 8 px apart, padding 8.
            let width = 8.0 + glyph_size + 8.0 + text + 8.0 + chevron + 8.0;
            (menu, label, name, width)
        })
    }

    /// How wide the menu row's three buttons are for `keys`, with their 1 px gaps.
    pub(super) fn graphing_menus_width(ui: &Ui, keys: Rect) -> f32 {
        Self::graphing_menus(ui, Sizes::for_keypad(keys))
            .iter()
            .map(|(_, _, _, width)| width + 1.0)
            .sum::<f32>()
            + 1.0
    }

    /// The Trigonometry, Inequalities and Function menu buttons in `menu_row`.
    fn draw_graphing_menus(&mut self, ui: &Ui, menu_row: Rect, sizes: Sizes) {
        let p = self.palette;
        let [_, menu_font, glyph_size, chevron_size] = sizes.menu;
        let mut left = menu_row.left();
        for (menu, label, name, width) in Self::graphing_menus(ui, sizes) {
            let painter = ui.painter();
            let galley = painter.layout_no_wrap(
                label.to_string(),
                FontId::proportional(menu_font),
                Color32::PLACEHOLDER,
            );
            let chevron = painter.layout_job(icon_label(ICON_CHEVRON_DOWN, chevron_size));
            let button = Rect::from_min_size(
                pos2(left + 1.0, menu_row.top() + 1.0),
                vec2(width, menu_row.height() - 2.0),
            );
            left = button.right() + 1.0;
            let open =
                matches!(self.graphing.popup, Some(Popup::Keypad { menu: m, .. }) if m == menu);
            let id = Id::new("graph-keypad-menu").with(menu);
            let response =
                access::control(ui, p, button, id, true, RADIUS, access::button(true, name));
            let pressed = response.is_pointer_button_down_on();
            let fill = if pressed {
                p.subtle_pressed
            } else if open || response.hovered() {
                p.subtle_hover
            } else {
                Color32::TRANSPARENT
            };
            let color = if pressed || open {
                p.text_secondary
            } else {
                p.text
            };
            painter.rect_filled(button, RADIUS, fill);
            let mut x = button.left() + 8.0;
            let icon_center = pos2(x + glyph_size / 2.0, button.center().y);
            match menu {
                KeypadMenu::Trigonometry => {
                    paint_triangle_icon(painter, icon_center, glyph_size, color)
                }
                KeypadMenu::Inequalities => {
                    paint_inequality_icon(painter, icon_center, glyph_size, color)
                }
                KeypadMenu::Function => {
                    let f = painter.layout_job(function_icon(glyph_size));
                    painter.galley(icon_center - f.size() / 2.0, f, color);
                }
            }
            x += glyph_size + 8.0;
            let size = galley.size();
            painter.galley(pos2(x, button.center().y - size.y / 2.0), galley, color);
            x += size.x + 8.0;
            painter.galley(
                pos2(x, button.center().y - chevron.size().y / 2.0),
                chevron.clone(),
                color,
            );
            if response.clicked() {
                self.graphing.popup = if open {
                    None
                } else {
                    self.opened_from(ui.ctx(), id);
                    Some(Popup::Keypad {
                        menu,
                        anchor: button,
                    })
                };
            }
        }
    }

    fn draw_graphing_keys(&mut self, ui: &Ui, rect: Rect, sizes: Sizes) {
        use GraphKey::*;
        let group = access::group(
            ui,
            Id::new("graph-keypad"),
            rect,
            tr("fullKeypad.AutomationName"),
        );
        let ui = &group;
        let shift = self.graphing.shift;
        let first = |normal: GraphKey, second: GraphKey| if shift { second } else { normal };
        let syntax = region_syntax();
        let decimal: &'static str = if syntax.decimal == ',' { "," } else { "." };
        let (root, log_base) = if syntax.separator == ';' {
            ("root(x;n)", "log(b; x)")
        } else {
            ("root(x,n)", "log(b, x)")
        };
        #[rustfmt::skip]
        let keys: [GraphKey; 35] = [
            Shift, Type("π", 1, 0), Type("e", 1, 0), Clear, Backspace,
            first(Type("^2", 2, 0), Type("^3", 2, 0)), Type("1/", 2, 0), Type("abs()", 4, 0), Type("x", 1, 0), Type("y", 1, 0),
            first(Type("sqrt()", 5, 0), Type("cbrt()", 5, 0)), Type("(", 1, 0), Type(")", 1, 0), Type("=", 1, 0), Type("/", 1, 0),
            first(Type("^", 1, 0), Type(root, 7, 1)), Type("7", 1, 0), Type("8", 1, 0), Type("9", 1, 0), Type("·", 1, 0),
            first(Type("10^", 3, 0), Type("2^", 2, 0)), Type("4", 1, 0), Type("5", 1, 0), Type("6", 1, 0), Type("−", 1, 0),
            first(Type("log()", 4, 0), Type(log_base, 4, 1)), Type("1", 1, 0), Type("2", 1, 0), Type("3", 1, 0), Type("+", 1, 0),
            first(Type("ln()", 3, 0), Type("e^", 2, 0)), Type("−", 1, 0), Type("0", 1, 0), Type(decimal, 1, 0), Submit,
        ];
        let width = (rect.width() - 4.0 * GAP) / 5.0;
        let height = (rect.height() - 6.0 * GAP) / 7.0;
        for (i, key) in keys.into_iter().enumerate() {
            let (row, col) = ((i / 5) as f32, (i % 5) as f32);
            let cell = Rect::from_min_size(
                rect.min + vec2(col * (width + GAP), row * (height + GAP)),
                vec2(width, height),
            );
            let id = Id::new("graph-key").with(i);
            if key == Shift {
                let label = second_label(sizes.operator);
                if self.toggle(
                    ui,
                    cell,
                    id,
                    true,
                    shift,
                    label,
                    tr("shiftButton.AutomationName"),
                ) {
                    self.graphing.shift = !shift;
                }
                continue;
            }
            let second = shift && i % 5 == 0 && i >= 5;
            let digit = (16..=18).contains(&i)
                || (21..=23).contains(&i)
                || (26..=28).contains(&i)
                || (31..=33).contains(&i);
            let kind = match key {
                Submit => Kind::Accent,
                _ if digit => Kind::Digit,
                _ if second => Kind::Emphasized,
                _ => Kind::Operator,
            };
            let (label, name) = self.graphing_key_label(i, key, sizes);
            if self.button(ui, cell, id, kind, true, label, name) {
                self.press_graphing_key(ui.ctx(), key);
                if second {
                    self.graphing.shift = false;
                }
            }
        }
    }

    /// A key's label and what a screen reader calls it.
    fn graphing_key_label(&self, i: usize, key: GraphKey, sizes: Sizes) -> (Label, &'static str) {
        let size = sizes.operator;
        let plain = |s: &str| Label::from(text(s, size, FontFamily::Proportional));
        let icon = |glyph: char| Label::from(icon_label(glyph, size));
        let f = |func: Func| function_label(func, size);
        match (i, key) {
            (_, GraphKey::Clear) => (plain("C"), tr("clearButton.AutomationName")),
            (_, GraphKey::Backspace) => {
                (icon(ICON_BACKSPACE), tr("backSpaceButton.AutomationName"))
            }
            (_, GraphKey::Submit) => (icon(ICON_RETURN), tr("submitButton.AutomationName")),
            (1, _) => (plain("π"), tr("piButton.AutomationName")),
            (2, _) => (plain("e"), tr("eulerButton.AutomationName")),
            (5, GraphKey::Type("^2", ..)) => (f(Func::Square), tr("xpower2Button.AutomationName")),
            (5, _) => (f(Func::Cube), tr("xpower3Button.AutomationName")),
            (6, _) => (f(Func::Reciprocal), tr("invertButton.AutomationName")),
            (7, _) => (f(Func::Abs), tr("absButton.AutomationName")),
            (8, _) => (italic('x', size + 2.0), tr("xButton.AutomationName")),
            (9, _) => (italic('y', size + 2.0), tr("yButton.AutomationName")),
            (10, GraphKey::Type("sqrt()", ..)) => {
                (f(Func::SquareRoot), tr("squareRootButton.AutomationName"))
            }
            (10, _) => (f(Func::CubeRoot), tr("cubeRootButton.AutomationName")),
            (11, _) => (plain("("), tr("openParenthesisButton.AutomationName")),
            (12, _) => (plain(")"), tr("closeParenthesisButton.AutomationName")),
            (13, _) => (icon(ICON_EQUALS), tr("graphingEqualButton.AutomationName")),
            (14, _) => (icon(ICON_DIVIDE), tr("divideButton.AutomationName")),
            (15, GraphKey::Type("^", ..)) => (power_label(size), tr("powerButton.AutomationName")),
            (15, _) => (root_label(size), tr("ySquareRootButton.AutomationName")),
            (19, _) => (icon(ICON_MULTIPLY), tr("multiplyButton.AutomationName")),
            (20, GraphKey::Type("10^", ..)) => {
                (f(Func::TenToX), tr("powerOf10Button.AutomationName"))
            }
            (20, _) => (f(Func::TwoToX), tr("twoPowerXButton.AutomationName")),
            (24, _) => (icon(ICON_MINUS), tr("minusButton.AutomationName")),
            (25, GraphKey::Type("log()", ..)) => {
                (plain("log"), tr("logBase10Button.AutomationName"))
            }
            (25, _) => (log_base_label(size), tr("logBaseY.AutomationName")),
            (29, _) => (icon(ICON_PLUS), tr("plusButton.AutomationName")),
            (30, GraphKey::Type("ln()", ..)) => (plain("ln"), tr("logBaseEButton.AutomationName")),
            (30, _) => (f(Func::EToX), tr("powerOfEButton.AutomationName")),
            (31, _) => (
                text("(−)", sizes.digit * 0.8, FontFamily::Proportional).into(),
                tr("negateButton.AutomationName"),
            ),
            // The key types the region's separator, so it shows what it types.
            (33, GraphKey::Type(decimal, ..)) => (
                text(decimal, sizes.digit, FontFamily::Proportional).into(),
                tr("decimalSeparatorButton.AutomationName"),
            ),
            (_, GraphKey::Type(digit, ..)) => {
                let name = format!("num{digit}Button.AutomationName");
                (
                    text(digit, sizes.digit, FontFamily::Proportional).into(),
                    tr(&name),
                )
            }
            _ => (LayoutJob::default().into(), ""),
        }
    }

    /// What a key does to the equation box that had the focus last (or the last box).
    fn press_graphing_key(&mut self, ctx: &Context, key: GraphKey) {
        let g = &mut self.graphing;
        let id = g
            .active
            .filter(|id| g.index_of(*id).is_some())
            .unwrap_or_else(|| g.rows[g.rows.len() - 1].id);
        let Some(index) = g.index_of(id) else {
            return;
        };
        g.active = Some(id);
        let edit = edit_id(id);
        let mut state = TextEdit::load_state(ctx, edit).unwrap_or_default();
        let text = g.rows[index].text.clone();
        let len = text.chars().count();
        let range = state
            .cursor
            .char_range()
            .map(|r| {
                let r = r.as_sorted_char_range();
                r.start.0.min(len)..r.end.0.min(len)
            })
            .unwrap_or(len..len);
        let (new_text, cursor) = match key {
            GraphKey::Type(insert, offset, select) => {
                insert_text(&text, range, insert, offset, select)
            }
            GraphKey::Backspace => {
                if range.is_empty() && range.start > 0 {
                    insert_text(&text, range.start - 1..range.start, "", 0, 0)
                } else {
                    insert_text(&text, range, "", 0, 0)
                }
            }
            GraphKey::Clear => {
                // `ClearButton_Clicked`: an empty equation, submitted.
                g.rows[index].text.clear();
                g.submit(index, region_syntax());
                ctx.memory_mut(|m| m.request_focus(edit));
                return;
            }
            GraphKey::Submit => {
                self.submit_with_enter(ctx, index);
                return;
            }
            GraphKey::Shift => return,
        };
        g.rows[index].text = new_text;
        state.cursor.set_char_range(Some(CCursorRange::two(
            CCursor::new(cursor.start),
            CCursor::new(cursor.end),
        )));
        state.store(ctx, edit);
        ctx.memory_mut(|m| m.request_focus(edit));
    }

    /// An open keypad menu under its button: Trigonometry (`2nd` and `hyp` beside six
    /// functions), Inequalities (five relations) or Function (|x|, ⌊x⌋, ⌈x⌉). A choice
    /// closes it and turns the toggles off (`FlyoutButton_Clicked`).
    pub(super) fn draw_keypad_menu(&mut self, ui: &Ui, menu: KeypadMenu, anchor: Rect) -> bool {
        let p = self.palette;
        let full = ui.max_rect();
        let sizes = Sizes::for_keypad(self.graphing.keypad_rect);
        let (columns, rows, (width, height)) = match menu {
            KeypadMenu::Trigonometry => (4, 2, sizes.trig_menu),
            KeypadMenu::Inequalities => (
                5,
                1,
                (
                    sizes.function_menu.0 * 312.0 / 194.0,
                    sizes.function_menu.1 / 2.0,
                ),
            ),
            KeypadMenu::Function => (3, 1, (sizes.function_menu.0, sizes.function_menu.1 / 2.0)),
        };
        let outer = vec2(width, height) + vec2(6.0, 6.0);
        let left = anchor
            .left()
            .min(full.right() - 4.0 - outer.x)
            .max(full.left() + 4.0);
        let mut top = anchor.bottom() + 1.0;
        if top + outer.y > full.bottom() {
            top = anchor.top() - 1.0 - outer.y;
        }
        let rect = Rect::from_min_size(pos2(left, top), outer);
        popup_frame(p, ui.painter(), rect, p.popup);
        ui.interact(rect, Id::new("graph-keypad-menu-panel"), egui::Sense::CLICK);
        // The menu's items are a group named as its button (`access::group`).
        let name = match menu {
            KeypadMenu::Trigonometry => tr("trigButton.AutomationName"),
            KeypadMenu::Inequalities => tr("inequalityButton.AutomationName"),
            KeypadMenu::Function => tr("funcButton.AutomationName"),
        };
        let group = access::group(ui, Id::new("graph-keypad-menu-items"), rect, name);
        let ui = &group;
        let grid = rect.shrink(3.0);
        let (w, h) = (grid.width() / columns as f32, grid.height() / rows as f32);
        let cell = |column: usize, row: usize| {
            Rect::from_min_size(
                grid.min + vec2(column as f32 * w, row as f32 * h),
                vec2(w, h),
            )
            .shrink(1.0)
        };
        let mut chosen: Option<&'static str> = None;
        match menu {
            KeypadMenu::Trigonometry => {
                let toggles = [
                    (
                        second_label(sizes.operator),
                        "trigShiftButton.AutomationName",
                        self.graphing.trig_inverse,
                    ),
                    (
                        text("hyp", sizes.trig, FontFamily::Proportional),
                        "hypButton.AutomationName",
                        self.graphing.trig_hyperbolic,
                    ),
                ];
                for (row, (label, name, on)) in toggles.into_iter().enumerate() {
                    let id = Id::new("graph-trig-toggle").with(row);
                    if self.take_popup_focus(row == 0) {
                        ui.memory_mut(|memory| memory.request_focus(id));
                    }
                    if self.toggle(ui, cell(0, row), id, true, on, label, tr(name)) {
                        if row == 0 {
                            self.graphing.trig_inverse = !on;
                        } else {
                            self.graphing.trig_hyperbolic = !on;
                        }
                    }
                }
                for (i, function) in TrigFunction::ALL.into_iter().enumerate() {
                    let trig = Trig {
                        function,
                        inverse: self.graphing.trig_inverse,
                        hyperbolic: self.graphing.trig_hyperbolic,
                    };
                    let label = text(&trig_label(trig), sizes.trig, FontFamily::Proportional);
                    let id = Id::new("graph-trig").with(i);
                    let name =
                        super::command_name(crate::engine::Command::Function(Func::Trig(trig)));
                    if self.button(
                        ui,
                        cell(1 + i % 3, i / 3),
                        id,
                        Kind::Operator,
                        true,
                        label,
                        name,
                    ) {
                        chosen = Some(trig_output(trig));
                    }
                }
            }
            KeypadMenu::Inequalities => {
                let items = [
                    ("<", "lessThanFlyoutButton.AutomationName"),
                    ("≤", "lessThanOrEqualFlyoutButton.AutomationName"),
                    ("=", "equalsFlyoutButton.AutomationName"),
                    ("≥", "greaterThanOrEqualFlyoutButton.AutomationName"),
                    (">", "greaterThanFlyoutButton.AutomationName"),
                ];
                for (i, (symbol, name)) in items.into_iter().enumerate() {
                    let id = Id::new("graph-inequality").with(i);
                    if self.take_popup_focus(i == 0) {
                        ui.memory_mut(|memory| memory.request_focus(id));
                    }
                    let label = text(symbol, sizes.text_icon, FontFamily::Proportional);
                    if self.button(ui, cell(i, 0), id, Kind::Operator, true, label, tr(name)) {
                        chosen = Some(symbol);
                    }
                }
            }
            KeypadMenu::Function => {
                let items = [
                    (Func::Abs, "abs()", "absButton.AutomationName"),
                    (Func::Floor, "floor()", "floorButton.AutomationName"),
                    (Func::Ceil, "ceiling()", "ceilButton.AutomationName"),
                ];
                for (i, (func, output, name)) in items.into_iter().enumerate() {
                    let id = Id::new("graph-function").with(i);
                    if self.take_popup_focus(i == 0) {
                        ui.memory_mut(|memory| memory.request_focus(id));
                    }
                    let label = function_label(func, sizes.text_icon);
                    if self.button(ui, cell(i, 0), id, Kind::Operator, true, label, tr(name)) {
                        chosen = Some(output);
                    }
                }
            }
        }
        if let Some(output) = chosen {
            // The cursor goes before the closing parenthesis of a function.
            let offset = output.chars().count() - usize::from(output.ends_with(')'));
            self.press_graphing_key(ui.ctx(), GraphKey::Type(output, offset, 0));
            self.graphing.trig_inverse = false;
            self.graphing.trig_hyperbolic = false;
            return true;
        }
        false
    }
}

/// What a trigonometric function types: `sin()`, `arcsin()`, `sinh()`, `arcsinh()`.
fn trig_output(trig: Trig) -> &'static str {
    use TrigFunction::*;
    match (trig.function, trig.inverse, trig.hyperbolic) {
        (Sin, false, false) => "sin()",
        (Cos, false, false) => "cos()",
        (Tan, false, false) => "tan()",
        (Sec, false, false) => "sec()",
        (Csc, false, false) => "csc()",
        (Cot, false, false) => "cot()",
        (Sin, true, false) => "arcsin()",
        (Cos, true, false) => "arccos()",
        (Tan, true, false) => "arctan()",
        (Sec, true, false) => "arcsec()",
        (Csc, true, false) => "arccsc()",
        (Cot, true, false) => "arccot()",
        (Sin, false, true) => "sinh()",
        (Cos, false, true) => "cosh()",
        (Tan, false, true) => "tanh()",
        (Sec, false, true) => "sech()",
        (Csc, false, true) => "csch()",
        (Cot, false, true) => "coth()",
        (Sin, true, true) => "arcsinh()",
        (Cos, true, true) => "arccosh()",
        (Tan, true, true) => "arctanh()",
        (Sec, true, true) => "arcsech()",
        (Csc, true, true) => "arccsch()",
        (Cot, true, true) => "arccoth()",
    }
}

/// The Inequalities menu's icon: a small square beside a larger one. Windows draws it
/// from the Calculator's own font.
fn paint_inequality_icon(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let s = size / 16.0;
    let stroke = Stroke::new(1.0 * s.max(1.0), color);
    let at = |x: f32, y: f32| center + vec2(x * s, y * s);
    painter.add(Shape::closed_line(
        vec![at(-7.0, 1.0), at(-2.0, 1.0), at(-2.0, 6.0), at(-7.0, 6.0)],
        stroke,
    ));
    painter.add(Shape::closed_line(
        vec![at(-2.0, -5.0), at(7.0, -5.0), at(7.0, 6.0), at(-2.0, 6.0)],
        stroke,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trig_outputs() {
        let t = |function, inverse, hyperbolic| {
            trig_output(Trig {
                function,
                inverse,
                hyperbolic,
            })
        };
        assert_eq!(t(TrigFunction::Sin, false, false), "sin()");
        assert_eq!(t(TrigFunction::Cot, true, false), "arccot()");
        assert_eq!(t(TrigFunction::Tan, true, true), "arctanh()");
    }
}
