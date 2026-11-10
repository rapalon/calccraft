//! The Graphing mode (`GraphingCalculator.xaml`): the graph on the left and the equation
//! list, the variables' sliders and the keypad on the right (in a window under 800 px, one
//! or the other, with a switch in the header), the function analysis that takes the place
//! of the list, and the equation style and graph options flyouts.
//!
//! The equation boxes are egui `TextEdit`s, the only text fields of the app: Windows'
//! are math editors, which egui doesn't have. Their text is laid out like math
//! (`math_job`): variables in italics, raised exponents, spaced relations.

use std::sync::Arc;

use eframe::egui::{
    self, Align, Align2, Color32, Context, CursorIcon, FontFamily, FontId, Frame, Galley, Id, Key,
    Margin, Modifiers, Painter, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, TextEdit, TextFormat,
    Ui, UiBuilder, WidgetInfo, WidgetType, pos2,
    text::{CCursor, CCursorRange, LayoutJob},
    vec2,
};

use super::date_view::popup_frame;
use super::icons::{
    ICON_BACK, ICON_CHEVRON_DOWN, ICON_CHEVRON_UP, ICON_COPY, ICON_CUT, ICON_DELETE, ICON_HIDE,
    ICON_LIGHTNING, ICON_MULTIPLY, ICON_PASTE, ICON_REMOVE, ICON_THEME, ICON_UNDO, ICON_VIEW,
    ICON_WARNING, icon_label,
};
use super::palette::{DARK_GRAPH, GraphPalette, LIGHT_GRAPH};
use super::strings::{tr, tr_format};
use super::{CalculatorApp, HEADER_HEIGHT, Kind, RADIUS, access, numbers, radio_button, semibold};
use crate::engine::Angle;
use crate::graphing::analysis::{self, AnalysisError, Features, Period};
use crate::graphing::plot::{LineStyle, View};
use crate::graphing::{Equation, Syntax, SyntaxError, parse};

/// From this window width the graph and the equations sit side by side (`ColumnsState`).
const COLUMNS_WIDTH: f32 = 800.0;
/// The equation panel: a third of the window, but 300 to 420 px wide.
const PANEL_MIN: f32 = 300.0;
const PANEL_MAX: f32 = 420.0;
/// At most 14 equations (`maxEquationSize`).
const MAX_EQUATIONS: usize = 14;
/// The characters an equation box takes (`MathRichEditBox`'s `MaxLength`).
const MAX_EQUATION_LENGTH: usize = 2048;
/// The order in which new equations take the 14 colors (`colorAssignmentMapping`).
const COLOR_ORDER: [usize; 14] = [0, 3, 7, 10, 1, 4, 8, 11, 2, 5, 9, 12, 6, 13];
/// An equation box's height (`MinHeight="44"`), and its buttons' width.
const ROW_HEIGHT: f32 = 44.0;
const ROW_BUTTON: f32 = 38.0;

/// The stored "Graph theme" choice (`IsGraphThemeMatchApp`, Windows' own setting name).
pub(super) const THEME_KEY: &str = "IsGraphThemeMatchApp";

/// One box of the equation list (Windows' `EquationViewModel`).
pub(super) struct Row {
    /// Names the box's widgets across frames.
    pub(super) id: u64,
    /// What is typed.
    pub(super) text: String,
    /// What is graphed: Windows graphs an equation when it's submitted (Enter, or
    /// leaving the box after a change), not while it's typed.
    pub(super) submitted: String,
    /// The `n` of fₙ (`FunctionLabelIndex`).
    pub(super) label: usize,
    /// Which of the 14 equation colors it has (`LineColorIndex`).
    pub(super) color: usize,
    pub(super) style: LineStyle,
    /// Whether its line is shown (`IsLineEnabled`, the eye button).
    pub(super) visible: bool,
    /// The submitted text, parsed; `None` while it's empty.
    pub(super) equation: Option<Result<Equation, SyntaxError>>,
}

impl Row {
    pub(super) fn graphable(&self) -> Option<&Equation> {
        match &self.equation {
            Some(Ok(e)) if self.visible => Some(e),
            _ => None,
        }
    }

    fn error(&self) -> Option<SyntaxError> {
        match &self.equation {
            Some(Err(e)) => Some(*e),
            _ => None,
        }
    }
}

/// A free variable's slider (`VariableViewModel`, `GraphControl::Variable`).
pub(super) struct Variable {
    pub(super) name: char,
    pub(super) value: f64,
    pub(super) min: f64,
    pub(super) max: f64,
    pub(super) step: f64,
    /// Whether the Min, Step and Max boxes show (`SliderSettingsVisible`).
    pub(super) open: bool,
    /// The value, minimum, step and maximum boxes' text while one is edited.
    pub(super) texts: [String; 4],
}

impl Variable {
    /// A new variable: 1 on a slider from −5 to 5 in steps of 0.1.
    fn new(name: char) -> Self {
        let mut v = Variable {
            name,
            value: 1.0,
            min: -5.0,
            max: 5.0,
            step: 0.1,
            open: false,
            texts: Default::default(),
        };
        v.refresh_texts();
        v
    }

    fn refresh_texts(&mut self) {
        self.texts = [self.value, self.min, self.step, self.max].map(g6);
    }

    /// Sets the value; a value beyond the slider moves its end (`VariableViewModel.Value`).
    pub(super) fn set_value(&mut self, value: f64) {
        if value < self.min {
            self.min = value;
        } else if value > self.max {
            self.max = value;
        }
        self.value = value;
    }

    /// A minimum at or above the maximum moves the maximum 10 above it, and the other
    /// way around (`DefaultMinMaxRange`).
    pub(super) fn set_min(&mut self, min: f64) {
        if min >= self.max {
            self.max = min + 10.0;
        }
        self.min = min;
        self.keep_value_in_range();
    }

    pub(super) fn set_max(&mut self, max: f64) {
        if max <= self.min {
            self.min = max - 10.0;
        }
        self.max = max;
        self.keep_value_in_range();
    }

    /// Windows' slider pulls its value into a new range, and its two-way binding hands
    /// that value back to the variable (`Value="{x:Bind Value, Mode=TwoWay}"`).
    fn keep_value_in_range(&mut self) {
        self.value = self.value.max(self.min).min(self.max);
    }
}

/// `v` with 6 significant digits, like .NET's "G6": fixed point when the exponent is
/// from −5 (excluded) to 6 (excluded), scientific notation otherwise.
pub(super) fn g6(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        return if v.is_finite() {
            "0".into()
        } else {
            v.to_string()
        };
    }
    // The exponent is the rounded number's: rounding can carry into the next power of
    // ten (999999.7 is 1E+06).
    let scientific = format!("{:.5e}", v.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let digits = mantissa.replace('.', "");
    let sign = if v < 0.0 { "-" } else { "" };
    if !(-4..6).contains(&exponent) {
        let mut m = format!("{}.{}", &digits[..1], &digits[1..]);
        trim_zeros(&mut m);
        return format!(
            "{sign}{m}E{}{:02}",
            if exponent < 0 { '-' } else { '+' },
            exponent.abs()
        );
    }
    let mut s = if exponent >= 0 {
        let (int, fraction) = digits.split_at(exponent as usize + 1);
        format!("{int}.{fraction}")
    } else {
        format!("0.{}{digits}", "0".repeat((-exponent - 1) as usize))
    };
    trim_zeros(&mut s);
    format!("{sign}{s}")
}

/// The value at `t` (0 to 1) along a slider from `min` to `max`, snapped to the steps
/// from the minimum (`StepFrequency`). Blending the ends, unlike min + t × (max − min),
/// stays finite on a slider from −1e308 to 1e308.
fn slider_value(t: f64, min: f64, max: f64, step: f64) -> f64 {
    let raw = min * (1.0 - t) + max * t;
    let snapped = clean(min + ((raw - min) / step).round() * step, min, step);
    let value = if snapped.is_finite() { snapped } else { raw };
    value.max(min).min(max)
}

/// Where `value` is along a slider from `min` to `max`, from 0 to 1. The halves keep
/// the span finite on a slider from −1e308 to 1e308.
fn slider_fraction(value: f64, min: f64, max: f64) -> f64 {
    let t = (value / 2.0 - min / 2.0) / (max / 2.0 - min / 2.0);
    if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) }
}

/// `v`, which is `base` plus whole steps, without the floating point noise of the sum
/// (0.1 + 0.2 = 0.30000000000000004). The exact sum has no more decimals than `base` or
/// `step`, so `v` is rounded to theirs. Windows' slider keeps plain doubles, so a step
/// keeps all its digits too (1e-14, 1.23456789012345e-14).
fn clean(v: f64, base: f64, step: f64) -> f64 {
    let decimals = decimals(base).max(decimals(step));
    format!("{v:.decimals$}").parse().unwrap_or(v)
}

/// The decimals of `x`'s shortest representation: 1 for 0.1, 28 for 1.23456789012345e-14.
fn decimals(x: f64) -> usize {
    let text = format!("{x:e}");
    let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
    let fraction = mantissa
        .split_once('.')
        .map_or(0, |(_, digits)| digits.len()) as i64;
    (fraction - exponent.parse::<i64>().unwrap_or(0)).max(0) as usize
}

fn trim_zeros(s: &mut String) {
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
}

/// An open popup of the mode. Only one is open at a time.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Popup {
    /// The graph options, under their button.
    Settings { anchor: Rect, widths_open: bool },
    /// An equation's line options (color and style), under its box.
    Style {
        row: u64,
        anchor: Rect,
        styles_open: bool,
    },
    /// An equation box's context menu.
    RowMenu { row: u64, at: Pos2 },
    /// The graph's context menu (Copy).
    GraphMenu { at: Pos2 },
    /// A keypad menu, under its button.
    Keypad { menu: KeypadMenu, anchor: Rect },
    /// The dialog when the share sheet doesn't open (`ShowShareError`).
    ShareError,
}

/// The graph options' line thickness combo, which opens `Popup::Settings`'s list.
fn width_combo_id() -> Id {
    Id::new("graph-settings-width")
}

/// The line options' line style combo, which opens `Popup::Style`'s list.
fn style_combo_id() -> Id {
    Id::new("graph-style-combo")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum KeypadMenu {
    Trigonometry,
    Inequalities,
    Function,
}

/// The function analysis of one equation.
pub(super) struct Analysis {
    pub(super) row: u64,
    pub(super) result: Result<Features, AnalysisError>,
}

/// Where a picture of the graph is: requested (the next frames draw the graph alone), then
/// waiting for the screenshot of a rect since a time.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Capture {
    None,
    Requested(Purpose),
    Waiting(Purpose, Rect, f64),
}

/// What a picture of the graph is for: the clipboard (the context menu's Copy) or the
/// share sheet (the share button).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Purpose {
    Copy,
    Share,
}

/// The Graphing mode's state (`GraphingCalculatorViewModel` and the views' fields).
pub(super) struct GraphingState {
    pub(super) rows: Vec<Row>,
    last_label: usize,
    next_id: u64,
    pub(super) variables: Vec<Variable>,
    pub(super) view: View,
    /// The graph's size when its view was last fitted.
    pub(super) fitted: Option<(f32, f32)>,
    /// The graph's size last frame, for zooming from its center with the keyboard.
    pub(super) graph_size: (f32, f32),
    /// Whether the view was moved by hand (`IsManualAdjustment`), so that new equations
    /// keep it instead of fitting a new one.
    pub(super) manual: bool,
    /// The trigonometric unit (Graph options, Units).
    pub(super) angle: Angle,
    /// Line thickness (Graph options), 1 to 4.
    pub(super) line_width: f32,
    /// "Match app theme" rather than "Always light" (Graph theme).
    pub(super) match_app_theme: bool,
    /// In a narrow window, whether the equations show instead of the graph
    /// (`SwitchModeToggleButton.IsOn`).
    show_equations: bool,
    pub(super) analysis: Option<Analysis>,
    pub(super) popup: Option<Popup>,
    /// An Esc that closed a drop-down of the popup is still held (`swallow_held_escape`).
    esc_held: bool,
    /// The keypad's `2nd`, and the Trigonometry menu's `2nd` and `hyp`.
    pub(super) shift: bool,
    pub(super) trig_inverse: bool,
    pub(super) trig_hyperbolic: bool,
    /// The equation box the keypad types into: the one that had the focus last.
    pub(super) active: Option<u64>,
    /// The graph options' window boxes (X-Min, X-Max, Y-Min, Y-Max) while edited.
    pub(super) range_texts: [String; 4],
    /// The window boxes whose text was typed, kept until the four make a window.
    range_kept: [bool; 4],
    /// Active tracing (the tracing button), and where its pointer is, relative to the
    /// graph.
    pub(super) tracing: bool,
    pub(super) trace_at: Option<Pos2>,
    /// When the trace pointer last moved with the arrow keys.
    pub(super) trace_moved: f64,
    pub(super) capture: Capture,
    /// The plots of the last frame and what they were made for.
    pub(super) cache: Option<super::graph_canvas::PlotCache>,
    /// A zoom button held down: which, and when it repeats next (`RepeatButton`).
    pub(super) zoom_held: Option<(bool, f64)>,
    /// Where the keypad was last frame: a click there doesn't submit the equation.
    pub(super) keypad_rect: Rect,
}

impl GraphingState {
    pub(super) fn new() -> Self {
        let mut state = GraphingState {
            rows: Vec::new(),
            last_label: 0,
            next_id: 0,
            variables: Vec::new(),
            view: View::DEFAULT,
            fitted: None,
            graph_size: (800.0, 600.0),
            manual: false,
            angle: Angle::Radians,
            line_width: 2.0,
            match_app_theme: false,
            show_equations: false,
            analysis: None,
            popup: None,
            esc_held: false,
            shift: false,
            trig_inverse: false,
            trig_hyperbolic: false,
            active: None,
            range_texts: Default::default(),
            range_kept: [false; 4],
            tracing: false,
            trace_at: None,
            trace_moved: 0.0,
            capture: Capture::None,
            cache: None,
            zoom_held: None,
            keypad_rect: Rect::NOTHING,
        };
        state.add_row();
        state
    }

    pub(super) fn popup_open(&self) -> bool {
        self.popup.is_some()
    }

    pub(super) fn close_popup(&mut self) -> bool {
        self.popup.take().is_some()
    }

    /// Closes the line thickness or line style list, if one is open over its flyout, and
    /// returns its combo.
    pub(super) fn close_drop_down(&mut self) -> Option<Id> {
        match &mut self.popup {
            Some(Popup::Settings { widths_open, .. }) if *widths_open => {
                *widths_open = false;
                Some(width_combo_id())
            }
            Some(Popup::Style { styles_open, .. }) if *styles_open => {
                *styles_open = false;
                Some(style_combo_id())
            }
            _ => None,
        }
    }

    /// Adds an empty box at the end (`AddNewEquation`): the next number, and the first
    /// color in `COLOR_ORDER` that no box has. Returns its id, or `None` at 14 boxes.
    pub(super) fn add_row(&mut self) -> Option<u64> {
        if self.rows.len() >= MAX_EQUATIONS {
            return None;
        }
        let used: Vec<usize> = self.rows.iter().map(|r| r.color).collect();
        let color = COLOR_ORDER
            .iter()
            .copied()
            .find(|c| !used.contains(c))
            .unwrap_or(COLOR_ORDER[13]);
        self.last_label += 1;
        let id = self.next_id;
        self.next_id += 1;
        self.rows.push(Row {
            id,
            text: String::new(),
            submitted: String::new(),
            label: self.last_label,
            color,
            style: LineStyle::Solid,
            visible: true,
            equation: None,
        });
        Some(id)
    }

    pub(super) fn index_of(&self, id: u64) -> Option<usize> {
        self.rows.iter().position(|r| r.id == id)
    }

    /// Removes a box, but never the last one (`EquationTextBox_RemoveButtonClicked`),
    /// and renumbers the last one to follow the one before it.
    pub(super) fn remove_row(&mut self, index: usize) {
        if index + 1 >= self.rows.len() {
            return;
        }
        let removed = self.rows.remove(index);
        let last = self.rows.len() - 1;
        self.last_label = if self.rows.len() <= 1 {
            1
        } else {
            self.rows[last - 1].label + 1
        };
        self.rows[last].label = self.last_label;
        if self.analysis.as_ref().is_some_and(|a| a.row == removed.id) {
            self.analysis = None;
        }
        self.equations_changed();
    }

    /// Graphs what is typed in a box. Returns whether that changed anything.
    pub(super) fn submit(&mut self, index: usize, syntax: Syntax) -> bool {
        let row = &mut self.rows[index];
        if row.text == row.submitted && row.equation.is_some() != row.text.trim().is_empty() {
            return false;
        }
        row.submitted = row.text.clone();
        row.equation =
            (!row.submitted.trim().is_empty()).then(|| Equation::parse(&row.submitted, syntax));
        self.equations_changed();
        true
    }

    /// Reparses every box (the decimal separator changed).
    pub(super) fn reparse(&mut self, syntax: Syntax) {
        for row in &mut self.rows {
            row.equation =
                (!row.submitted.trim().is_empty()).then(|| Equation::parse(&row.submitted, syntax));
        }
        self.equations_changed();
        self.refresh_analysis();
    }

    /// After a change to what is graphed: the variables follow, and a view that wasn't
    /// moved by hand is fitted again. With no equations left the view starts over
    /// (`IsManualAdjustment = false`).
    pub(super) fn equations_changed(&mut self) {
        if self.rows.iter().all(|r| r.graphable().is_none()) && self.rows.len() == 1 {
            self.manual = false;
        }
        if !self.manual {
            self.fitted = None;
        }
        // The plots are cached by the submitted text, which `reparse` keeps.
        self.cache = None;
        self.update_variables();
    }

    /// Analyzes the analyzed equation again (it was parsed again, or the unit changed).
    /// An equation that no longer parses closes the analysis.
    fn refresh_analysis(&mut self) {
        let Some(open) = &mut self.analysis else {
            return;
        };
        match self
            .rows
            .iter()
            .find(|r| r.id == open.row)
            .and_then(|r| r.equation.as_ref())
        {
            Some(Ok(e)) => open.result = analysis::analyze(e, self.angle),
            _ => self.analysis = None,
        }
    }

    /// The free variables of the graphed equations, keeping the sliders of the ones that
    /// stay (`Grapher::UpdateVariables`), in alphabetical order.
    fn update_variables(&mut self) {
        let mut names: Vec<char> = self
            .rows
            .iter()
            .filter_map(Row::graphable)
            .flat_map(|e| e.variables.iter().copied())
            .collect();
        names.sort_unstable();
        names.dedup();
        let mut old = std::mem::take(&mut self.variables);
        self.variables = names
            .into_iter()
            .map(|name| match old.iter().position(|v| v.name == name) {
                Some(i) => old.swap_remove(i),
                None => Variable::new(name),
            })
            .collect();
    }

    /// The variables' values, for evaluating.
    pub(super) fn values(&self) -> Vec<(char, f64)> {
        self.variables.iter().map(|v| (v.name, v.value)).collect()
    }

    /// The graph's colors for the effective app theme.
    pub(super) fn graph_palette(&self, app_dark: bool) -> &'static GraphPalette {
        if self.match_app_theme && app_dark {
            &DARK_GRAPH
        } else {
            &LIGHT_GRAPH
        }
    }

    /// Fits the view to the equations again (Windows' automatic best fit).
    pub(super) fn reset_view(&mut self) {
        self.manual = false;
        self.fitted = None;
    }

    /// Reads the graph options' window boxes (`focused` is the one with the focus) and
    /// moves the view when they make a window (`UpdateDisplayRange`). Returns the boxes
    /// that show an error.
    pub(super) fn update_range(&mut self, focused: Option<usize>) -> [bool; 4] {
        let v = self.view;
        let values = [v.x_min, v.x_max, v.y_min, v.y_max];
        let mut parsed = [None; 4];
        for k in 0..4 {
            // Windows keeps what is typed in the boxes (`GraphingSettingsViewModel`): a
            // minimum typed above its maximum stays, as an error, until the four make a
            // window. The other boxes show the view, and so does an emptied one (the
            // flyout empties them when it opens and on "Reset view").
            if Some(k) == focused {
                self.range_kept[k] = true;
            } else if !self.range_kept[k] || self.range_texts[k].is_empty() {
                self.range_kept[k] = false;
                self.range_texts[k] = numbers().localize_plain(&g6(values[k]));
            }
            parsed[k] = range_value(&self.range_texts[k], values[k], self.range_kept[k]);
        }
        let errors = range_errors(parsed);
        if let [Some(a), Some(b), Some(c), Some(d)] = parsed
            && !errors.contains(&true)
        {
            // `range_errors` checked that the graph can draw both pairs.
            let new = View {
                x_min: a,
                x_max: b,
                y_min: c,
                y_max: d,
            };
            if new != self.view {
                self.view = new;
                self.manual = true;
            }
            // The view has the window: the boxes show it again.
            self.range_kept = [false; 4];
        }
        errors
    }
}

/// The syntax of the user's region: a decimal comma takes `;` between arguments.
pub(super) fn region_syntax() -> Syntax {
    Syntax::for_decimal(region_decimal())
}

/// The region's decimal separator.
fn region_decimal() -> char {
    numbers().localize("0.5").chars().nth(1).unwrap_or('.')
}

/// Replaces `range` of `text` with `insert`, then selects `select` characters from
/// `offset` characters into it, like Windows' `MathRichEditBox.InsertText`. Returns the
/// new text and the selection.
pub(super) fn insert_text(
    text: &str,
    range: std::ops::Range<usize>,
    insert: &str,
    offset: usize,
    select: usize,
) -> (String, std::ops::Range<usize>) {
    let chars: Vec<char> = text.chars().collect();
    let start = range.start.min(chars.len());
    let end = range.end.clamp(start, chars.len());
    let mut out: String = chars[..start].iter().collect();
    out.push_str(insert);
    out.extend(&chars[end..]);
    let at = start + offset.min(insert.chars().count());
    (out, at..at + select)
}

/// The keyboard's `*` and `-` in an equation, as math writes them. One character for
/// one, so the cursor stays where it is.
fn normalize(text: &mut String) {
    if text.contains(['*', '-']) {
        *text = text
            .chars()
            .map(|c| match c {
                '*' => '·',
                '-' => '−',
                c => c,
            })
            .collect();
    }
}

/// An equation laid out like math (Windows' math editor shows it so): variables in
/// italics, function names upright, exponents raised and smaller with their `^` hidden,
/// and space around relations and the binary + and −. `syntax` is the region's, as for
/// parsing: a comma is a decimal separator only with a decimal comma.
pub(super) fn math_job(text: &str, size: f32, color: Color32, syntax: Syntax) -> LayoutJob {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    // Per character: italics, raised, hidden, extra space after.
    let mut italic = vec![false; n];
    let mut raised = vec![false; n];
    let mut hidden = vec![false; n];
    let mut space = vec![0.0f32; n];
    let mut i = 0;
    while i < n {
        if chars[i].is_ascii_alphabetic() {
            let start = i;
            while i < n && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let mut at = start;
            for (len, name) in parse::word_parts(&word) {
                for flag in &mut italic[at..at + len] {
                    *flag = !name;
                }
                at += len;
            }
            continue;
        }
        i += 1;
    }
    // Exponents: what follows a ^ (a parenthesized group, a number, a name or a letter,
    // with a sign) is raised. A number has the parser's decimal points (`tokenize`), so
    // the comma of `root(x^2,3)` ends it.
    let number = |c: char| c.is_ascii_digit() || c == '.' || c == syntax.decimal;
    let mut i = 0;
    while i < n {
        if chars[i] != '^' {
            i += 1;
            continue;
        }
        hidden[i] = true;
        let mut j = i + 1;
        if j < n && matches!(chars[j], '−' | '-') {
            j += 1;
        }
        let start = j;
        if j < n && chars[j] == '(' {
            j = group_end(&chars, j);
        } else if j < n && number(chars[j]) {
            while j < n && number(chars[j]) {
                j += 1;
            }
        } else if j < n && chars[j].is_ascii_alphabetic() {
            // One letter, or a function name and its parentheses.
            let word_end = (j..n)
                .find(|&k| !chars[k].is_ascii_alphabetic())
                .unwrap_or(n);
            let word: String = chars[j..word_end].iter().collect();
            let (len, name) = parse::word_parts(&word)[0];
            j += len;
            if name && j < n && chars[j] == '(' {
                j = group_end(&chars, j);
            }
        } else if j < n {
            j += 1;
        }
        for flag in &mut raised[i + 1..j.max(start)] {
            *flag = true;
        }
        i = j.max(i + 1);
    }
    // Space around relations, and around + and − between operands.
    let operand_end =
        |c: char| c.is_alphanumeric() || matches!(c, ')' | '|' | '!' | 'π' | '²' | '³');
    for k in 0..n {
        let c = chars[k];
        let gap = match c {
            '=' | '<' | '>' | '≤' | '≥' => size * 0.28,
            '+' | '−' | '-' => {
                let before = chars[..k].iter().rev().find(|c| !c.is_whitespace());
                if before.is_some_and(|&b| operand_end(b)) && !raised[k] {
                    size * 0.22
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };
        if gap > 0.0 {
            space[k] = gap;
            if let Some(prev) = (0..k).rev().find(|&p| !hidden[p]) {
                space[prev] += gap;
            }
        }
    }
    let mut job = LayoutJob::default();
    for k in 0..n {
        let font = if raised[k] { size * 0.7 } else { size };
        let format = TextFormat {
            font_id: FontId::new(if hidden[k] { 1.0 } else { font }, FontFamily::Proportional),
            color: if hidden[k] {
                Color32::TRANSPARENT
            } else {
                color
            },
            italics: italic[k],
            valign: if raised[k] { Align::TOP } else { Align::Center },
            extra_letter_spacing: space[k],
            line_height: Some(size * 1.35),
            ..Default::default()
        };
        let mut buf = [0u8; 4];
        job.append(chars[k].encode_utf8(&mut buf), 0.0, format);
    }
    job.wrap.max_width = f32::INFINITY;
    job.keep_trailing_whitespace = true;
    job
}

/// Where the parenthesized group that opens at `start` ends: after its closing
/// parenthesis, or at the end of the text while it isn't closed.
fn group_end(chars: &[char], start: usize) -> usize {
    let mut depth = 0;
    let mut j = start;
    while j < chars.len() {
        match chars[j] {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        j += 1;
        if depth == 0 {
            break;
        }
    }
    j
}

/// A color's text: black or white, whichever stands out (`Utilities.GetContrastColor`).
fn contrast(color: Color32) -> Color32 {
    let [r, g, b, _] = color.to_array();
    let luma = 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32;
    if luma > 160.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// `f` with its number as a subscript: the equation button's label.
fn function_label(index: Option<usize>, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(
        "f",
        0.0,
        TextFormat {
            font_id: FontId::new(size, FontFamily::Proportional),
            color: Color32::PLACEHOLDER,
            italics: true,
            valign: Align::Center,
            ..Default::default()
        },
    );
    if let Some(index) = index {
        job.append(
            &index.to_string(),
            1.0,
            TextFormat {
                font_id: FontId::proportional(11.0),
                color: Color32::PLACEHOLDER,
                valign: Align::BOTTOM,
                ..Default::default()
            },
        );
    }
    job
}

impl CalculatorApp {
    // -------------------------------------------------------------- keyboard

    /// The Graphing mode's keys besides what the text boxes take: Ctrl+plus and
    /// Ctrl+minus zoom (`ZoomInButton`'s and `ZoomOutButton`'s accelerators), Ctrl+0 the
    /// graph view button, and Esc closes the menu or a popup, or stops tracing.
    pub(super) fn handle_graphing_keyboard(&mut self, ctx: &Context) {
        let pressed = |key: Key| {
            ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, key))
                || ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, key))
        };
        let size = self.graphing.graph_size;
        if pressed(Key::Plus) || pressed(Key::Equals) {
            self.zoom_graph(true, size);
        }
        if pressed(Key::Minus) {
            self.zoom_graph(false, size);
        }
        if pressed(Key::Num0) {
            self.toggle_graph_view();
        }
        self.swallow_held_escape(ctx);
        // Under the menu, an open drop-down of the graph options or the line options is the
        // topmost thing: Esc closes it alone, and its combo takes the focus, as WinUI's
        // ComboBox handles Esc itself.
        if self.flyout == super::Flyout::None
            && ctx.input(|i| i.key_pressed(Key::Escape))
            && let Some(combo) = self.graphing.close_drop_down()
        {
            ctx.memory_mut(|m| m.request_focus(combo));
            ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
            self.graphing.esc_held = ctx.input(|i| i.key_down(Key::Escape));
        }
        // One Esc closes one thing, the topmost: the menu, a popup, active tracing, then
        // the function analysis (in `draw_analysis`). A used Esc is consumed, so it
        // doesn't close the next one too.
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            let handled = if self.flyout != super::Flyout::None {
                self.flyout = super::Flyout::None;
                true
            } else if self.graphing.close_popup() {
                true
            } else if self.graphing.tracing {
                self.graphing.tracing = false;
                ctx.memory_mut(|m| m.request_focus(super::graph_canvas::TRACE_ID.into()));
                true
            } else {
                false
            };
            if handled {
                ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
            }
        }
    }

    /// Zooms from the graph's center (`ZoomFromCenter`), which makes the view manual.
    pub(super) fn zoom_graph(&mut self, zoom_in: bool, size: (f32, f32)) {
        let scale = if zoom_in { 1.0 / 1.0625 } else { 1.0625 };
        let g = &mut self.graphing;
        g.view = g.view.zoomed(
            size.0 as f64 / 2.0,
            size.1 as f64 / 2.0,
            size.0,
            size.1,
            scale,
        );
        g.manual = true;
    }

    /// The graph view button: back to the automatic best fit when the view was moved by
    /// hand, and otherwise keeping the current view from now on (`GraphViewButton_Click`).
    pub(super) fn toggle_graph_view(&mut self) {
        if self.graphing.manual {
            self.graphing.reset_view();
        } else {
            self.graphing.manual = true;
        }
    }

    // ---------------------------------------------------------------- layout

    /// Draws the mode below the header (which ends at `top`).
    pub(super) fn draw_graphing_mode(&mut self, ui: &mut Ui, full: Rect, top: f32) {
        let columns = full.width() >= COLUMNS_WIDTH;
        let body = full.with_min_y(top);
        if self.graphing.capture != Capture::None {
            // A copy of the graph: draw it alone, as large as it is.
            let graph = if columns {
                let panel = (full.width() / 3.0).clamp(PANEL_MIN, PANEL_MAX);
                body.with_max_x(full.right() - panel)
            } else {
                body
            };
            self.draw_graph(ui, graph.with_min_y(top + 4.0), false);
            return;
        }
        if columns {
            let panel = (full.width() / 3.0).clamp(PANEL_MIN, PANEL_MAX);
            let graph = body.with_max_x(full.right() - panel).with_min_y(top + 4.0);
            let right = body.with_min_x(full.right() - panel);
            self.draw_graph(ui, graph, true);
            self.draw_equation_panel(ui, right);
        } else {
            let header = Rect::from_min_max(
                pos2(full.left(), top - HEADER_HEIGHT),
                pos2(full.right(), top),
            );
            self.draw_mode_switch(ui, header);
            if self.graphing.show_equations {
                self.draw_equation_panel(ui, body);
            } else {
                self.draw_graph(ui, body.with_min_y(top + 4.0), true);
            }
        }
    }

    /// The popups, drawn over everything.
    pub(super) fn draw_graphing_popups(&mut self, ui: &mut Ui) {
        let Some(popup) = self.graphing.popup else {
            return;
        };
        let full = ui.max_rect();
        let backdrop = ui.interact(full, Id::new("graph-popup-backdrop"), Sense::CLICK);
        access::set_covered(ui.ctx(), false);
        // A dialog stays open until it's dismissed.
        let dialog = popup == Popup::ShareError;
        let mut close = !dialog && (backdrop.clicked() || backdrop.secondary_clicked());
        match popup {
            Popup::Settings {
                anchor,
                widths_open,
            } => {
                close |= self.draw_graph_settings(ui, anchor, widths_open);
            }
            Popup::Style {
                row,
                anchor,
                styles_open,
            } => close |= self.draw_style_flyout(ui, row, anchor, styles_open),
            Popup::RowMenu { row, at } => close |= self.draw_row_menu(ui, row, at),
            Popup::GraphMenu { at } => close |= self.draw_graph_menu(ui, at),
            Popup::Keypad { menu, anchor } => close |= self.draw_keypad_menu(ui, menu, anchor),
            Popup::ShareError => close |= self.draw_share_error(ui),
        }
        if close && self.graphing.popup == Some(popup) {
            self.graphing.popup = None;
        }
    }

    /// The switch between the graph and the equations in a narrow window
    /// (`GraphModeToggleSwitchStyle`): an accent knob over the graph or the equations icon.
    fn draw_mode_switch(&mut self, ui: &Ui, header: Rect) {
        let p = self.palette;
        let on = self.graphing.show_equations;
        let rect = Rect::from_min_size(
            pos2(header.right() - 12.0 - 64.0, header.center().y - 16.0),
            vec2(64.0, 30.0),
        );
        let name = if on {
            tr("GraphSwitchToGraphMode")
        } else {
            tr("GraphSwitchToEquationMode")
        };
        let info = access::item(WidgetType::Checkbox, name, on);
        let response = access::control(ui, p, rect, Id::new("graph-switch"), true, RADIUS, info)
            .on_hover_text(name);
        let painter = ui.painter();
        let base = if response.is_pointer_button_down_on() {
            p.subtle_pressed
        } else if response.hovered() {
            p.subtle_hover
        } else {
            p.switch_track
        };
        painter.rect_filled(rect, RADIUS, base);
        let knob = Rect::from_min_size(
            pos2(rect.left() + if on { 32.0 } else { 0.0 }, rect.top()),
            vec2(32.0, rect.height()),
        );
        painter.rect_filled(knob, RADIUS, p.accent);
        painter.rect_stroke(rect, RADIUS, Stroke::new(1.0, p.accent), StrokeKind::Inside);
        let graph_color = if on { p.text } else { p.text_on_accent };
        let equations_color = if on { p.text_on_accent } else { p.text };
        paint_graph_icon(
            painter,
            pos2(rect.left() + 16.0, rect.center().y),
            14.0,
            graph_color,
        );
        let f = painter.layout_job(function_label(None, 13.0));
        let at = pos2(rect.left() + 46.0, rect.center().y);
        painter.galley(at - f.size() / 2.0 - vec2(2.0, 0.0), f, equations_color);
        painter.text(
            at + vec2(4.0, 4.0),
            Align2::CENTER_CENTER,
            "x",
            FontId::proportional(10.0),
            equations_color,
        );
        if response.clicked() {
            self.graphing.show_equations = !on;
        }
    }

    /// The equation list (with the variables) over the keypad, or the function analysis.
    fn draw_equation_panel(&mut self, ui: &mut Ui, rect: Rect) {
        if self.graphing.analysis.is_some() {
            self.draw_analysis(ui, rect.with_min_y(rect.top() + 4.0));
            return;
        }
        // `RowDefinition Height="*"` over `Height="2.2*" MaxHeight="400"`.
        let keypad_height = (rect.height() * 2.2 / 3.2).min(400.0);
        let list = rect.with_max_y(rect.bottom() - keypad_height);
        let keypad = Rect::from_min_max(
            pos2(rect.left() + 2.0, list.bottom()),
            rect.max - vec2(2.0, 2.0),
        );
        self.draw_equation_list(ui, list.with_min_y(list.top() + 4.0));
        self.graphing.keypad_rect = keypad;
        self.draw_graphing_keypad(ui, keypad);
    }

    // -------------------------------------------------------------- equations

    fn draw_equation_list(&mut self, ui: &mut Ui, rect: Rect) {
        let mut area = egui::ScrollArea::vertical()
            .id_salt("graph-equations")
            .auto_shrink(false)
            .scroll_source(access::scroll_source(ui, Default::default()));
        area = area.max_height(rect.height());
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            area.show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let ids: Vec<u64> = self.graphing.rows.iter().map(|r| r.id).collect();
                for id in ids {
                    let width = ui.available_width();
                    let (rect, _) =
                        ui.allocate_exact_size(vec2(width, ROW_HEIGHT + 2.0), Sense::hover());
                    // `Margin="4,0"` around the item and `1,0,1,2` around the box.
                    let rect =
                        Rect::from_min_max(rect.min + vec2(5.0, 0.0), rect.max - vec2(5.0, 2.0));
                    self.draw_equation_row(ui, id, rect);
                }
                if !self.graphing.variables.is_empty() {
                    let width = ui.available_width();
                    let (rect, _) = ui.allocate_exact_size(vec2(width, 25.0), Sense::hover());
                    ui.painter().hline(
                        (rect.left() + 12.0)..=(rect.right() - 12.0),
                        rect.center().y,
                        Stroke::new(1.0, self.palette.divider),
                    );
                    for k in 0..self.graphing.variables.len() {
                        self.draw_variable(ui, k);
                    }
                }
            });
        });
    }

    /// One equation box (`EquationTextBox`): the colored fₙ button that shows and hides
    /// the line, the text, and the buttons that appear on hover (function analysis, line
    /// options, remove) or with the focus (clear).
    fn draw_equation_row(&mut self, ui: &mut Ui, id: u64, rect: Rect) {
        let p = self.palette;
        let ctx = ui.ctx().clone();
        let graph = self
            .graphing
            .graph_palette(ctx.theme() == egui::Theme::Dark);
        let Some(index) = self.graphing.index_of(id) else {
            return;
        };
        let last = index + 1 == self.graphing.rows.len();
        let edit_id = edit_id(id);
        let focused = ctx.memory(|m| m.has_focus(edit_id));
        let covered = self.graphing.popup.is_some();
        let hovered = !covered && ui.rect_contains_pointer(rect);
        let row = &self.graphing.rows[index];
        let color = graph.equations[row.color];
        let empty = row.text.is_empty();
        let add_mode = last && empty;
        let error = row.error();

        // The box.
        let painter = ui.painter().clone();
        let fill = if error.is_some() && !focused {
            p.error_fill
        } else {
            p.control
        };
        painter.rect_filled(rect, RADIUS, fill);
        let border = if error.is_some() {
            Some(p.error_border)
        } else if add_mode && focused {
            Some(p.hidden_equation)
        } else if focused || hovered && !add_mode {
            Some(color)
        } else {
            None
        };
        if let Some(border) = border {
            painter.rect_stroke(rect, RADIUS, Stroke::new(1.0, border), StrokeKind::Inside);
        }

        // The fₙ button: the equation's color, the hide color while its line is hidden
        // (or the box is the empty last one), an eye on hover.
        let button = Rect::from_min_size(rect.min, vec2(ROW_HEIGHT, rect.height()));
        let visible = row.visible;
        let label = row.label;
        let name = if visible {
            tr_format("hideEquationButtonAutomationName", &[&label.to_string()])
        } else {
            tr_format("showEquationButtonAutomationName", &[&label.to_string()])
        };
        let info = access::button(!add_mode, &name);
        let response = access::control(
            ui,
            p,
            button,
            Id::new("graph-eq-button").with(id),
            !add_mode,
            RADIUS,
            info,
        );
        let (fill, text_color) = if add_mode || !visible {
            (p.hidden_equation, p.hidden_equation_text)
        } else {
            (color, contrast(color))
        };
        painter.rect_filled(button, RADIUS, fill);
        if !add_mode && response.hovered() {
            let overlay = if ctx.theme() == egui::Theme::Dark {
                Color32::WHITE
            } else {
                Color32::BLACK
            };
            let opacity = match (
                ctx.theme() == egui::Theme::Dark,
                response.is_pointer_button_down_on(),
            ) {
                (true, false) => 0.3,
                (true, true) => 0.5,
                (false, false) => 0.2,
                (false, true) => 0.4,
            };
            painter.rect_filled(button, RADIUS, overlay.gamma_multiply(opacity));
            let glyph = if visible { ICON_HIDE } else { ICON_VIEW };
            let icon = painter.layout_job(icon_label(glyph, 16.0));
            painter.galley(button.center() - icon.size() / 2.0, icon, text_color);
            let tip = if visible {
                tr("hideEquationButtonToolTip")
            } else {
                tr("showEquationButtonToolTip")
            };
            response.clone().on_hover_text(tip);
        } else {
            let f = painter.layout_job(function_label((!add_mode).then_some(label), 18.0));
            painter.galley(button.center() - f.size() / 2.0, f, text_color);
        }
        if !add_mode && response.clicked() {
            let row = &mut self.graphing.rows[index];
            row.visible = !row.visible;
            self.graphing.equations_changed();
        }

        // The buttons on the right.
        let mut right = rect.right() - 1.0;
        let mut slot = |width: f32| {
            let r = Rect::from_min_max(
                pos2(right - width, rect.top() + 2.0),
                pos2(right, rect.bottom() - 2.0),
            );
            right -= width + 2.0;
            r
        };
        let mut actions = RowActions::default();
        if focused && !empty {
            let r = slot(ROW_BUTTON);
            let label = icon_label(ICON_MULTIPLY, 12.0);
            if self.button(
                ui,
                r,
                Id::new("graph-eq-clear").with(id),
                Kind::Subtle,
                true,
                label,
                "",
            ) {
                actions.clear = true;
            }
        } else if hovered && !focused {
            if !last {
                let r = slot(ROW_BUTTON);
                let label = icon_label(ICON_DELETE, 14.0);
                let name = tr("removeButton.AutomationName");
                let response_rect = r;
                if self.button(
                    ui,
                    r,
                    Id::new("graph-eq-remove").with(id),
                    Kind::Subtle,
                    true,
                    label,
                    name,
                ) {
                    actions.remove = true;
                }
                tooltip(
                    ui,
                    response_rect,
                    Id::new("graph-eq-remove").with(id),
                    tr("removeButton.ToolTip"),
                );
            }
            if error.is_none() && !add_mode {
                let r = slot(ROW_BUTTON);
                let label = icon_label(ICON_THEME, 14.0);
                let name = tr("colorChooserButton.AutomationName");
                if self.button(
                    ui,
                    r,
                    Id::new("graph-eq-style").with(id),
                    Kind::Subtle,
                    true,
                    label,
                    name,
                ) {
                    actions.style = Some(rect);
                }
                tooltip(
                    ui,
                    r,
                    Id::new("graph-eq-style").with(id),
                    tr("colorChooserButton.ToolTip"),
                );
                let r = slot(ROW_BUTTON);
                let label = icon_label(ICON_LIGHTNING, 14.0);
                let name = tr("functionAnalysisButton.AutomationName");
                let enabled = !self.graphing.rows[index].submitted.is_empty();
                if self.button(
                    ui,
                    r,
                    Id::new("graph-eq-analyze").with(id),
                    Kind::Subtle,
                    enabled,
                    label,
                    name,
                ) {
                    actions.analyze = true;
                }
                tooltip(
                    ui,
                    r,
                    Id::new("graph-eq-analyze").with(id),
                    tr("functionAnalysisButton.ToolTip"),
                );
            }
        }
        if let Some(error) = error
            && !focused
        {
            let r = slot(32.0);
            let icon = painter.layout_job(icon_label(ICON_WARNING, 16.0));
            painter.galley(r.center() - icon.size() / 2.0, icon, p.critical);
            let message = tr(error.key());
            let response = ui.interact(r, Id::new("graph-eq-error").with(id), Sense::hover());
            response.on_hover_text(message);
        }

        // The text.
        let text_rect = Rect::from_min_max(
            pos2(button.right() + 10.0, rect.top()),
            pos2(right - 4.0, rect.bottom()),
        );
        let text_color = p.text;
        let syntax = region_syntax();
        let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, _wrap: f32| -> Arc<Galley> {
            ui.fonts_mut(|f| f.layout_job(math_job(buf.as_str(), 14.0, text_color, syntax)))
        };
        let row = &mut self.graphing.rows[index];
        let hint = egui::RichText::new(tr("mathRichEditBox.PlaceholderText"))
            .color(p.text_secondary)
            .size(14.0);
        // A child that doesn't move the list's layout cursor (`scope_builder` would).
        let mut child = text_ui(ui, text_rect, p);
        let output = TextEdit::singleline(&mut row.text)
            .id(edit_id)
            .frame(Frame::NONE)
            .margin(Margin::ZERO)
            .hint_text(hint)
            .font(FontId::proportional(14.0))
            .text_color(p.text)
            .desired_width(text_rect.width())
            .vertical_align(Align::Center)
            .min_size(vec2(text_rect.width(), text_rect.height()))
            .char_limit(MAX_EQUATION_LENGTH)
            .layouter(&mut layouter)
            .show(&mut child);
        normalize(&mut row.text);
        let response = output.response.response;
        if response.gained_focus() || response.has_focus() {
            self.graphing.active = Some(id);
        }
        if response.secondary_clicked() {
            let at = ui.ctx().pointer_interact_pos().unwrap_or(rect.center());
            self.graphing.popup = Some(Popup::RowMenu { row: id, at });
            // The menu's Paste needs the text by the time it's picked.
            #[cfg(target_arch = "wasm32")]
            super::web::prefetch_clipboard();
        }
        let enter = ctx.input(|i| i.key_pressed(Key::Enter));
        if response.lost_focus() {
            let on_keypad = ctx
                .pointer_interact_pos()
                .is_some_and(|p| self.graphing.keypad_rect.contains(p))
                && ctx.input(|i| i.pointer.any_down() || i.pointer.any_released());
            if enter {
                actions.submit_enter = true;
            } else if !on_keypad {
                actions.submit_left = true;
            }
        }
        self.apply_row_actions(&ctx, id, actions);
    }

    fn apply_row_actions(&mut self, ctx: &Context, id: u64, actions: RowActions) {
        let syntax = region_syntax();
        let Some(index) = self.graphing.index_of(id) else {
            return;
        };
        if actions.clear {
            self.graphing.rows[index].text.clear();
            self.graphing.submit(index, syntax);
            ctx.memory_mut(|m| m.request_focus(edit_id(id)));
        }
        if actions.submit_enter {
            self.submit_with_enter(ctx, index);
        } else if actions.submit_left {
            let row = &self.graphing.rows[index];
            if row.text != row.submitted && !row.text.trim().is_empty() {
                self.graphing.submit(index, syntax);
                if index + 1 == self.graphing.rows.len() {
                    self.graphing.add_row();
                }
            } else if row.text.trim().is_empty() && !row.submitted.is_empty() {
                self.graphing.submit(index, syntax);
            }
        }
        if actions.remove {
            self.graphing.remove_row(index);
            if let Some(next) = self.graphing.rows.get(index) {
                ctx.memory_mut(|m| m.request_focus(edit_id(next.id)));
            }
        }
        if let Some(anchor) = actions.style {
            self.graphing.popup = Some(Popup::Style {
                row: id,
                anchor,
                styles_open: false,
            });
        }
        if actions.analyze {
            self.open_analysis(index);
        }
    }

    /// Enter in a box (or the keypad's ↵): graph it and show its line, then go to the
    /// next box, adding one after the last (`EquationTextBox_Submitted`).
    pub(super) fn submit_with_enter(&mut self, ctx: &Context, index: usize) {
        let syntax = region_syntax();
        let g = &mut self.graphing;
        g.rows[index].visible = true;
        g.submit(index, syntax);
        let next = if index + 1 == g.rows.len() {
            if g.rows[index].text.trim().is_empty() {
                None
            } else {
                g.add_row()
            }
        } else {
            Some(g.rows[index + 1].id)
        };
        if let Some(next) = next {
            ctx.memory_mut(|m| m.request_focus(edit_id(next)));
            g.active = Some(next);
        } else {
            ctx.memory_mut(|m| m.request_focus(edit_id(g.rows[index].id)));
        }
    }

    /// Opens the function analysis of a box (`OnEquationKeyGraphFeaturesRequested`).
    pub(super) fn open_analysis(&mut self, index: usize) {
        let syntax = region_syntax();
        self.graphing.submit(index, syntax);
        let row = &self.graphing.rows[index];
        let result = match &row.equation {
            Some(Ok(e)) => analysis::analyze(e, self.graphing.angle),
            _ => return,
        };
        self.graphing.analysis = Some(Analysis {
            row: row.id,
            result,
        });
        self.graphing.popup = None;
    }

    // -------------------------------------------------------------- variables

    /// A variable's card: `a = value`, its slider, and the Min, Step and Max boxes
    /// behind the chevron (`VariableDataTemplate`).
    fn draw_variable(&mut self, ui: &mut Ui, k: usize) {
        let p = self.palette;
        let open = self.graphing.variables[k].open;
        let height = if open { 128.0 } else { 98.0 };
        let width = ui.available_width();
        let (outer, _) = ui.allocate_exact_size(vec2(width, height + 6.0), Sense::hover());
        let card = Rect::from_min_max(outer.min + vec2(5.0, 3.0), outer.max - vec2(5.0, 3.0));
        let name = self.graphing.variables[k].name;
        // A click on the card (not on its controls) opens its options too
        // (`VariableAreaTapped`).
        let card_response = ui.interact(card, Id::new("graph-var-card").with(name), Sense::CLICK);
        ui.painter().rect_filled(card, RADIUS, p.card);

        let inner = Rect::from_min_max(card.min + vec2(12.0, 0.0), card.max - vec2(12.0, 6.0));
        // `a = value`, in italics.
        let line_y = inner.top() + 22.0;
        let italic16 = |s: &str| {
            let mut job = LayoutJob::default();
            job.append(
                s,
                0.0,
                TextFormat {
                    font_id: FontId::proportional(16.0),
                    color: p.text,
                    italics: true,
                    ..Default::default()
                },
            );
            job
        };
        let painter = ui.painter().clone();
        let name_galley = painter.layout_job(italic16(&name.to_string()));
        let x = inner.left() + 8.0;
        painter.galley(
            pos2(x, line_y - name_galley.size().y / 2.0),
            name_galley.clone(),
            p.text,
        );
        let eq_x = x + name_galley.size().x.max(16.0) + 6.0;
        let eq = painter.layout_job(italic16("="));
        painter.galley(pos2(eq_x, line_y - eq.size().y / 2.0), eq.clone(), p.text);
        let value_rect = Rect::from_min_size(
            pos2(eq_x + eq.size().x.max(16.0) + 6.0, line_y - 14.0),
            vec2(56.0, 28.0),
        );
        self.value_box(
            ui,
            k,
            0,
            value_rect,
            14.0,
            tr("VariableValueTextBox.AutomationName"),
        );

        // The slider.
        let slider = Rect::from_min_max(
            pos2(inner.left() + 8.0, line_y + 16.0),
            pos2(inner.right() - 8.0, line_y + 48.0),
        );
        self.draw_slider(ui, k, slider);

        let mut y = slider.bottom() - 6.0;
        if open {
            // Min, Step and Max in three columns.
            let row = Rect::from_min_max(
                pos2(inner.left() + 8.0, y + 4.0),
                pos2(inner.right() - 8.0, y + 32.0),
            );
            let column = row.width() / 3.0;
            let labels = [
                (
                    1,
                    tr("MinTextBlock.Text"),
                    tr("VariableMinTextBox.AutomationName"),
                ),
                (
                    2,
                    tr("StepTextBlock.Text"),
                    tr("VariableStepTextBox.AutomationName"),
                ),
                (
                    3,
                    tr("MaxTextBlock.Text"),
                    tr("VariableMaxTextBox.AutomationName"),
                ),
            ];
            for (c, (field, label, box_name)) in labels.into_iter().enumerate() {
                let cell = Rect::from_min_size(
                    pos2(row.left() + c as f32 * column, row.top()),
                    vec2(column, row.height()),
                );
                let galley = painter.layout_no_wrap(
                    label.to_string(),
                    FontId::proportional(12.0),
                    p.text_secondary,
                );
                let total = galley.size().x + 4.0 + 44.0;
                let start = match c {
                    0 => cell.left(),
                    1 => cell.center().x - total / 2.0,
                    _ => cell.right() - total,
                };
                painter.galley(
                    pos2(start, cell.center().y - galley.size().y / 2.0 + 2.0),
                    galley.clone(),
                    p.text_secondary,
                );
                let field_rect = Rect::from_min_size(
                    pos2(start + galley.size().x + 4.0, cell.top()),
                    vec2(44.0, cell.height()),
                );
                self.value_box(ui, k, field, field_rect, 12.0, box_name);
            }
            y = row.bottom();
        }

        // The chevron that shows and hides the options.
        let chevron = Rect::from_center_size(pos2(card.center().x, y + 16.0), vec2(36.0, 22.0));
        let glyph = if open {
            ICON_CHEVRON_UP
        } else {
            ICON_CHEVRON_DOWN
        };
        let label = icon_label(glyph, 9.0);
        let name_text = tr("VariableAreaSettings.AutomationName");
        let toggled = self.button(
            ui,
            chevron,
            Id::new("graph-var-options").with(name),
            Kind::Subtle,
            true,
            label,
            name_text,
        );
        tooltip(
            ui,
            chevron,
            Id::new("graph-var-options").with(name),
            tr("VariableAreaSettings.ToolTip"),
        );
        let card_clicked = card_response.clicked()
            && !ui
                .ctx()
                .pointer_interact_pos()
                .is_some_and(|p| value_rect.contains(p) || slider.contains(p));
        if toggled || card_clicked {
            // Opening one closes the others (`ToggleVariableArea`).
            for (j, v) in self.graphing.variables.iter_mut().enumerate() {
                v.open = j == k && !open;
            }
        }
    }

    /// One of a variable's boxes: 0 is the value, 1 the minimum, 2 the step, 3 the
    /// maximum. The value goes in on Enter or when the box loses the focus
    /// (`SubmitTextbox`); a step of 0 or less is refused.
    fn value_box(
        &mut self,
        ui: &mut Ui,
        k: usize,
        field: usize,
        rect: Rect,
        size: f32,
        name: &str,
    ) {
        let p = self.palette;
        let name_char = self.graphing.variables[k].name;
        let id = Id::new("graph-var-box").with((name_char, field));
        let focused = ui.ctx().memory(|m| m.has_focus(id));
        let painter = ui.painter().clone();
        let underline = if focused { p.accent } else { p.value_underline };
        painter.hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(if focused { 2.0 } else { 1.0 }, underline),
        );
        let v = &mut self.graphing.variables[k];
        // The value is in italics, like the variable's name (`FontStyle="Italic"`).
        let italics = field == 0;
        let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, _wrap: f32| -> Arc<Galley> {
            let mut job = LayoutJob::single_section(
                buf.as_str().to_string(),
                TextFormat {
                    font_id: FontId::proportional(size),
                    color: p.text,
                    italics,
                    ..Default::default()
                },
            );
            job.halign = Align::Center;
            job.wrap.max_width = f32::INFINITY;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let mut child = text_ui(ui, rect, p);
        let output = TextEdit::singleline(&mut v.texts[field])
            .id(id)
            .frame(Frame::NONE)
            .margin(Margin::symmetric(2, 2))
            .font(FontId::proportional(size))
            .text_color(p.text)
            .horizontal_align(Align::Center)
            .vertical_align(Align::Center)
            .desired_width(rect.width())
            .min_size(rect.size())
            .layouter(&mut layouter)
            .show(&mut child);
        let response = output.response.response;
        response.widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, true, name));
        if response.gained_focus() {
            // `TextBoxGotFocus` selects everything.
            let mut state = output.state;
            let len = v.texts[field].chars().count();
            state
                .cursor
                .set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(len))));
            state.store(ui.ctx(), id);
        }
        if response.lost_focus() {
            let parsed = numbers()
                .delocalize(v.texts[field].trim())
                .parse::<f64>()
                .ok()
                .filter(|x| x.is_finite());
            if let Some(x) = parsed {
                match field {
                    0 => v.set_value(x),
                    1 => v.set_min(x),
                    2 if x > 0.0 => v.step = x,
                    3 => v.set_max(x),
                    _ => {}
                }
            }
            v.refresh_texts();
            for t in &mut v.texts {
                *t = numbers().localize_plain(t);
            }
        } else if !focused {
            let values = [v.value, v.min, v.step, v.max];
            v.texts[field] = numbers().localize_plain(&g6(values[field]));
        }
    }

    /// A variable's slider: the accent fill up to the thumb, which snaps to the step.
    fn draw_slider(&mut self, ui: &Ui, k: usize, rect: Rect) {
        let p = self.palette;
        let v = &self.graphing.variables[k];
        let id = Id::new("graph-var-slider").with(v.name);
        let name = tr("VariableValueSlider.AutomationName");
        let value = v.value;
        let info = move || {
            let mut info = WidgetInfo::labeled(WidgetType::Slider, true, name);
            info.value = Some(value);
            info
        };
        let response =
            super::access::interact(ui, p, rect, id, Sense::click_and_drag(), RADIUS, info);
        let track = Rect::from_min_max(
            pos2(rect.left() + 10.0, rect.center().y - 2.0),
            pos2(rect.right() - 10.0, rect.center().y + 2.0),
        );
        let v = &mut self.graphing.variables[k];
        if let Some(pos) = response.interact_pointer_pos()
            && (response.dragged() || response.clicked() || response.is_pointer_button_down_on())
        {
            let t = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0) as f64;
            v.value = slider_value(t, v.min, v.max, v.step);
        }
        if response.has_focus() {
            // The arrow keys move the thumb, not the focus.
            ui.ctx().memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
            let (left, right) = ui.ctx().input(|i| {
                (
                    i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::ArrowDown),
                    i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::ArrowUp),
                )
            });
            if left {
                v.value = clean(v.value - v.step, v.value, v.step).max(v.min);
            }
            if right {
                v.value = clean(v.value + v.step, v.value, v.step).min(v.max);
            }
        }
        let t = slider_fraction(v.value, v.min, v.max) as f32;
        let thumb_x = track.left() + t * track.width();
        let painter = ui.painter();
        painter.rect_filled(track, 2.0, p.slider_rail);
        painter.rect_filled(track.with_max_x(thumb_x), 2.0, p.accent);
        let center = pos2(thumb_x, track.center().y);
        painter.circle(
            center,
            10.0,
            p.slider_thumb,
            Stroke::new(1.0, p.slider_thumb_border),
        );
        let inner = if response.is_pointer_button_down_on() {
            5.0
        } else if response.hovered() {
            7.0
        } else {
            6.0
        };
        painter.circle_filled(center, inner, p.accent);
    }

    // --------------------------------------------------------------- analysis

    /// The function analysis (`KeyGraphFeaturesPanel`): the back button with the
    /// function's color and number, the equation, the "Function analysis" title and the
    /// features, or why there are none.
    fn draw_analysis(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette;
        let Some(analysis) = &self.graphing.analysis else {
            return;
        };
        let Some(index) = self.graphing.index_of(analysis.row) else {
            self.graphing.analysis = None;
            return;
        };
        let graph = self
            .graphing
            .graph_palette(ui.ctx().theme() == egui::Theme::Dark);
        let row = &self.graphing.rows[index];
        let color = graph.equations[row.color];
        let painter = ui.painter().clone();
        let header = Rect::from_min_size(
            rect.min + vec2(4.0, 0.0),
            vec2(rect.width() - 8.0, ROW_HEIGHT),
        );
        painter.rect_filled(header, RADIUS, p.control);
        let back = Rect::from_min_size(header.min, vec2(76.0, header.height()));
        let name = tr("equationAnalysisBack.AutomationName");
        let response = access::control(
            ui,
            p,
            back,
            Id::new("graph-analysis-back"),
            true,
            RADIUS,
            access::button(true, name),
        )
        .on_hover_text(tr("equationAnalysisBack.ToolTip"));
        painter.rect_filled(back, RADIUS, color);
        if response.hovered() {
            let dark = ui.ctx().theme() == egui::Theme::Dark;
            let overlay = if dark { Color32::WHITE } else { Color32::BLACK };
            let opacity = match (dark, response.is_pointer_button_down_on()) {
                (true, false) => 0.3,
                (true, true) => 0.5,
                (false, false) => 0.2,
                (false, true) => 0.4,
            };
            painter.rect_filled(back, RADIUS, overlay.gamma_multiply(opacity));
        }
        let fg = Color32::WHITE;
        let arrow = painter.layout_job(icon_label(ICON_BACK, 16.0));
        painter.galley(
            pos2(back.left() + 10.0, back.center().y - arrow.size().y / 2.0),
            arrow,
            fg,
        );
        let f = painter.layout_job(function_label(Some(row.label), 18.0));
        painter.galley(
            pos2(back.left() + 36.0, back.center().y - f.size().y / 2.0),
            f,
            fg,
        );
        let equation = painter.layout_job(math_job(&row.submitted, 14.0, p.text, region_syntax()));
        let text_clip = Rect::from_min_max(
            pos2(back.right() + 10.0, header.top()),
            header.max - vec2(6.0, 0.0),
        );
        painter.with_clip_rect(text_clip).galley(
            pos2(
                text_clip.left(),
                header.center().y - equation.size().y / 2.0,
            ),
            equation,
            p.text,
        );
        if response.clicked()
            || self.graphing.popup.is_none()
                && ui
                    .ctx()
                    .input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            let id = self.graphing.rows[index].id;
            self.graphing.analysis = None;
            ui.ctx().memory_mut(|m| m.request_focus(edit_id(id)));
            return;
        }

        let title_top = header.bottom() + 12.0;
        painter.text(
            pos2(rect.left() + 12.0, title_top),
            Align2::LEFT_TOP,
            tr("KeyGraphFeaturesLabel.Text"),
            FontId::new(20.0, semibold()),
            p.text,
        );
        let list = Rect::from_min_max(
            pos2(rect.left() + 12.0, title_top + 38.0),
            rect.max - vec2(10.0, 12.0),
        );
        let items = analysis_items(&analysis.result);
        let area = egui::ScrollArea::vertical()
            .id_salt("graph-analysis")
            .auto_shrink(false)
            .scroll_source(access::scroll_source(ui, Default::default()));
        ui.scope_builder(UiBuilder::new().max_rect(list), |ui| {
            area.show(ui, |ui| {
                let width = ui.available_width();
                for item in items {
                    let mut job = LayoutJob::default();
                    job.wrap.max_width = width;
                    if let Some(title) = &item.title {
                        job.append(
                            title,
                            0.0,
                            TextFormat {
                                font_id: FontId::proportional(12.0),
                                color: p.text_secondary,
                                ..Default::default()
                            },
                        );
                        job.append("\n", 0.0, TextFormat::default());
                    }
                    let values = TextFormat {
                        font_id: FontId::proportional(14.0),
                        color: p.text,
                        line_height: Some(20.0),
                        ..Default::default()
                    };
                    for (n, line) in item.lines.iter().enumerate() {
                        if n > 0 {
                            job.append("\n", 0.0, values.clone());
                        }
                        job.append(&localize_math(line), 0.0, values.clone());
                    }
                    let galley = ui.fonts_mut(|f| f.layout_job(job));
                    let (r, _) =
                        ui.allocate_exact_size(vec2(width, galley.size().y + 20.0), Sense::hover());
                    let label: String = galley.text().to_string();
                    access::text(
                        ui,
                        r,
                        Id::new("graph-analysis-item").with(&label),
                        &label,
                        false,
                    );
                    ui.painter().galley(r.min + vec2(0.0, 10.0), galley, p.text);
                }
            });
        });
    }

    // ----------------------------------------------------------------- popups

    /// The graph options flyout (`GraphingSettings`): the window ranges, the units, the
    /// line thickness and the graph theme. Returns whether it should close.
    fn draw_graph_settings(&mut self, ui: &mut Ui, anchor: Rect, widths_open: bool) -> bool {
        let p = self.palette;
        let full = ui.max_rect();
        // 318 wide, narrower in the narrowest windows.
        let size = vec2(318.0f32.min(full.width() - 8.0), 440.0);
        let left = (anchor.right() - size.x)
            .min(full.right() - size.x - 4.0)
            .max(full.left() + 4.0);
        let top = (anchor.bottom() + 4.0)
            .min(full.bottom() - size.y - 4.0)
            .max(full.top() + 4.0);
        let rect = Rect::from_min_size(pos2(left, top), size);
        popup_frame(p, ui.painter(), rect, p.popup);
        ui.interact(rect, Id::new("graph-settings-panel"), Sense::CLICK);
        // Under the line thickness list, the flyout's other controls take no input.
        access::set_covered(ui.ctx(), widths_open);
        let painter = ui.painter().clone();
        let inner = rect.shrink2(vec2(20.0, 15.0));
        painter.text(
            inner.min,
            Align2::LEFT_TOP,
            tr("GraphOptionsHeading.Text"),
            FontId::new(20.0, semibold()),
            p.text,
        );
        // Window, and Reset view.
        let mut y = inner.top() + 44.0;
        painter.text(
            pos2(inner.left(), y),
            Align2::LEFT_CENTER,
            tr("GridHeading.Text"),
            FontId::proportional(12.0),
            p.text_secondary,
        );
        let reset = tr("ResetViewButton.Content");
        let galley = painter.layout_no_wrap(reset.to_string(), FontId::proportional(12.0), p.link);
        let reset_rect = Rect::from_min_size(
            pos2(inner.right() - galley.size().x, y - galley.size().y / 2.0),
            galley.size(),
        )
        .expand(2.0);
        let response = access::control(
            ui,
            p,
            reset_rect,
            Id::new("graph-settings-reset"),
            true,
            RADIUS,
            || WidgetInfo::labeled(WidgetType::Link, true, reset),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
        let link = if response.is_pointer_button_down_on() {
            p.link_pressed
        } else if response.hovered() {
            p.link_hover
        } else {
            p.link
        };
        painter.galley(reset_rect.min + vec2(2.0, 2.0), galley, link);
        if response.clicked() {
            self.graphing.reset_view();
            self.graphing.range_texts = Default::default();
        }
        // X-Min, X-Max, Y-Min, Y-Max.
        y += 14.0;
        let half = (inner.width() - 10.0) / 2.0;
        let headers = [
            tr("GraphSettingsXMin.Header"),
            tr("GraphSettingsXMax.Header"),
            tr("GraphSettingsYMin.Header"),
            tr("GraphSettingsYMax.Header"),
        ];
        let focused = (0..4).find(|&k| ui.ctx().memory(|m| m.has_focus(range_id(k))));
        let errors = self.graphing.update_range(focused);
        for (k, header) in headers.iter().enumerate() {
            let x = inner.left() + (k % 2) as f32 * (half + 10.0);
            let top = y + (k / 2) as f32 * 62.0;
            painter.text(
                pos2(x, top),
                Align2::LEFT_TOP,
                header,
                FontId::proportional(12.0),
                p.text_secondary,
            );
            let field = Rect::from_min_size(pos2(x, top + 20.0), vec2(half.min(160.0), 32.0));
            self.range_box(ui, k, field, errors[k], header);
        }
        // Units.
        y += 136.0;
        painter.text(
            pos2(inner.left(), y),
            Align2::LEFT_TOP,
            tr("UnitsHeading.Text"),
            FontId::proportional(12.0),
            p.text_secondary,
        );
        let units = [
            (Angle::Radians, tr("TrigModeRadians.Content")),
            (Angle::Degrees, tr("TrigModeDegrees.Content")),
            (Angle::Gradians, tr("TrigModeGradians.Content")),
        ];
        let width = (inner.width() - 8.0) / 3.0;
        for (k, (angle, label)) in units.into_iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(inner.left() + k as f32 * (width + 4.0), y + 22.0),
                vec2(width, 38.0),
            );
            let checked = self.graphing.angle == angle;
            let info = access::item(WidgetType::RadioButton, label, checked);
            let response = access::control(
                ui,
                p,
                r,
                Id::new("graph-settings-unit").with(k),
                true,
                RADIUS,
                info,
            );
            let (fill, color) = if checked {
                let fill = if response.is_pointer_button_down_on() {
                    p.accent_pressed
                } else if response.hovered() {
                    p.accent_hover
                } else {
                    p.accent
                };
                (fill, p.text_on_accent)
            } else {
                let fill = if response.is_pointer_button_down_on() {
                    p.control_pressed
                } else if response.hovered() {
                    p.control_hover
                } else {
                    p.control
                };
                (fill, p.text)
            };
            painter.rect_filled(r, RADIUS, fill);
            if !checked {
                painter.rect_stroke(
                    r,
                    RADIUS,
                    Stroke::new(1.0, p.control_border),
                    StrokeKind::Inside,
                );
            }
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(14.0),
                color,
            );
            if response.clicked() && !checked {
                self.graphing.angle = angle;
                self.graphing.equations_changed();
                self.graphing.refresh_analysis();
            }
        }
        // Line thickness.
        y += 76.0;
        let thickness = tr("LineThicknessBoxHeading.Text");
        painter.text(
            pos2(inner.left(), y),
            Align2::LEFT_TOP,
            thickness,
            FontId::proportional(12.0),
            p.text_secondary,
        );
        let combo = Rect::from_min_size(pos2(inner.left(), y + 22.0), vec2(inner.width(), 32.0));
        let width_name = line_width_name(self.graphing.line_width);
        // The combo keeps the keyboard focus that opened its list.
        access::set_covered(ui.ctx(), false);
        let response = access::control(
            ui,
            p,
            combo,
            width_combo_id(),
            true,
            RADIUS,
            access::combo_box(thickness, width_name),
        );
        access::set_covered(ui.ctx(), widths_open);
        let fill = if response.is_pointer_button_down_on() {
            p.control_pressed
        } else if response.hovered() {
            p.control_hover
        } else {
            p.control
        };
        painter.rect_filled(combo, RADIUS, fill);
        painter.rect_stroke(
            combo,
            RADIUS,
            Stroke::new(1.0, p.control_border),
            StrokeKind::Inside,
        );
        painter.hline(
            (combo.left() + 12.0)..=(combo.right() - 44.0),
            combo.center().y,
            Stroke::new(self.graphing.line_width, p.text),
        );
        let chevron = painter.layout_job(icon_label(ICON_CHEVRON_DOWN, 12.0));
        painter.galley(
            pos2(
                combo.right() - 26.0,
                combo.center().y - chevron.size().y / 2.0,
            ),
            chevron,
            p.text_secondary,
        );
        let mut open_widths = widths_open;
        if response.clicked() {
            open_widths = !widths_open;
        }
        // Graph theme.
        y += 72.0;
        painter.text(
            pos2(inner.left(), y),
            Align2::LEFT_TOP,
            tr("GraphThemeHeading.Text"),
            FontId::proportional(12.0),
            p.text_secondary,
        );
        let light = tr("AlwaysLightTheme.Content");
        if radio_button(
            p,
            ui,
            pos2(inner.left(), y + 36.0),
            light,
            !self.graphing.match_app_theme,
            Id::new("graph-theme-light"),
        ) {
            self.graphing.match_app_theme = false;
        }
        let matched = tr("MatchAppTheme.Content");
        if radio_button(
            p,
            ui,
            pos2(inner.left(), y + 68.0),
            matched,
            self.graphing.match_app_theme,
            Id::new("graph-theme-match"),
        ) {
            self.graphing.match_app_theme = true;
        }
        if open_widths {
            // The line thickness list, over the flyout. A click elsewhere on the flyout closes
            // only the list, as a drop-down's light-dismiss layer does.
            access::set_covered(ui.ctx(), false);
            if ui
                .interact(rect, Id::new("graph-width-backdrop"), Sense::CLICK)
                .clicked()
            {
                open_widths = false;
            }
            let list = drop_down_rect(combo, 4, full);
            popup_frame(p, &painter, list, p.popup);
            // Its edges don't click the controls under it.
            ui.interact(list, Id::new("graph-width-list"), Sense::CLICK);
            for (k, width) in [1.0f32, 2.0, 3.0, 4.0].into_iter().enumerate() {
                let item = Rect::from_min_size(
                    list.min + vec2(4.0, 4.0 + k as f32 * 36.0),
                    vec2(list.width() - 8.0, 36.0),
                );
                let selected = self.graphing.line_width == width;
                let info = access::item(
                    WidgetType::SelectableLabel,
                    line_width_name(width),
                    selected,
                );
                let response = access::control(
                    ui,
                    p,
                    item,
                    Id::new("graph-width-item").with(k),
                    true,
                    RADIUS,
                    info,
                );
                let fill = if response.is_pointer_button_down_on() {
                    p.popup_item_pressed
                } else if response.hovered() || selected {
                    p.popup_item
                } else {
                    Color32::TRANSPARENT
                };
                painter.rect_filled(item, RADIUS, fill);
                if selected {
                    let pill = Rect::from_center_size(
                        pos2(item.left() + 5.0, item.center().y),
                        vec2(3.0, 16.0),
                    );
                    painter.rect_filled(pill, 2.0, p.accent);
                }
                painter.hline(
                    (item.left() + 14.0)..=(item.right() - 14.0),
                    item.center().y,
                    Stroke::new(width, p.text),
                );
                if response.clicked() {
                    self.graphing.line_width = width;
                    open_widths = false;
                }
            }
        }
        if open_widths != widths_open {
            if !open_widths {
                let item = Id::new("graph-width-item");
                self.drop_down_closed(ui.ctx(), width_combo_id(), item, 4);
            }
            self.graphing.popup = Some(Popup::Settings {
                anchor,
                widths_open: open_widths,
            });
        }
        false
    }

    /// One of the window boxes, red while `error`.
    fn range_box(&mut self, ui: &mut Ui, k: usize, rect: Rect, error: bool, name: &str) {
        let p = self.palette;
        let id = range_id(k);
        let focused = ui.ctx().memory(|m| m.has_focus(id));
        let painter = ui.painter().clone();
        let fill = if error {
            p.error_fill
        } else if focused {
            p.popup
        } else {
            p.control
        };
        painter.rect_filled(rect, RADIUS, fill);
        let border = if error {
            p.error_border
        } else {
            p.control_border
        };
        painter.rect_stroke(rect, RADIUS, Stroke::new(1.0, border), StrokeKind::Inside);
        if focused {
            painter.hline(
                (rect.left() + 1.0)..=(rect.right() - 1.0),
                rect.bottom() - 1.0,
                Stroke::new(2.0, p.accent),
            );
        }
        let text = &mut self.graphing.range_texts[k];
        let mut child = text_ui(ui, rect.shrink2(vec2(10.0, 0.0)), p);
        let output = TextEdit::singleline(text)
            .id(id)
            .frame(Frame::NONE)
            .margin(Margin::ZERO)
            .font(FontId::proportional(14.0))
            .text_color(p.text)
            .vertical_align(Align::Center)
            .desired_width(rect.width() - 20.0)
            .min_size(vec2(rect.width() - 20.0, rect.height()))
            .show(&mut child);
        output
            .response
            .response
            .widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, true, name));
        enter_to_next_box(ui.ctx(), k, &output.response.response);
    }

    /// The line options flyout (`EquationStylePanelControl`): 14 colors and, except for
    /// inequalities, the line style. Returns whether it should close.
    fn draw_style_flyout(&mut self, ui: &mut Ui, id: u64, anchor: Rect, styles_open: bool) -> bool {
        let p = self.palette;
        let Some(index) = self.graphing.index_of(id) else {
            return true;
        };
        let graph = self
            .graphing
            .graph_palette(ui.ctx().theme() == egui::Theme::Dark);
        let full = ui.max_rect();
        // Seven 44 px swatches a row, smaller ones in the narrowest windows.
        let swatch = ((full.width() - 8.0 - 32.0) / 7.0).min(44.0);
        let size = vec2(7.0 * swatch + 32.0, 148.0 + 2.0 * swatch);
        let left = (anchor.center().x - size.x / 2.0)
            .min(full.right() - size.x - 4.0)
            .max(full.left() + 4.0);
        let mut top = anchor.bottom() + 4.0;
        if top + size.y > full.bottom() - 4.0 {
            top = anchor.top() - 4.0 - size.y;
        }
        let rect = Rect::from_min_size(pos2(left, top.max(full.top() + 4.0)), size);
        popup_frame(p, ui.painter(), rect, p.popup);
        ui.interact(rect, Id::new("graph-style-panel"), Sense::CLICK);
        // Under the line style list, the flyout's other controls take no input.
        access::set_covered(ui.ctx(), styles_open);
        let painter = ui.painter().clone();
        let inner = rect.shrink(16.0);
        painter.text(
            inner.min,
            Align2::LEFT_TOP,
            tr("LineOptionsHeading.Text"),
            FontId::new(20.0, semibold()),
            p.text,
        );
        painter.text(
            inner.min + vec2(0.0, 36.0),
            Align2::LEFT_TOP,
            tr("LineColorText.Text"),
            FontId::proportional(14.0),
            p.text_secondary,
        );
        let row = &self.graphing.rows[index];
        let current = row.color;
        let inequality = row
            .equation
            .as_ref()
            .is_some_and(|e| e.as_ref().is_ok_and(Equation::is_inequality));
        for c in 0..14 {
            let cell = Rect::from_min_size(
                inner.min + vec2((c % 7) as f32 * swatch, 60.0 + (c / 7) as f32 * swatch),
                vec2(swatch, swatch),
            );
            let name = color_name(c, graph);
            let info = access::item(WidgetType::RadioButton, name, c == current);
            let response = access::control(
                ui,
                p,
                cell,
                Id::new("graph-style-color").with(c),
                true,
                22.0,
                info,
            )
            .on_hover_text(name);
            let grow = if response.hovered() { 2.0 } else { 0.0 };
            painter.circle_filled(cell.center(), 15.0 + grow, graph.equations[c]);
            if c == current {
                painter.circle_stroke(cell.center(), 19.0, Stroke::new(2.0, p.text));
            }
            if response.clicked() {
                self.graphing.rows[index].color = c;
            }
        }
        let style_top = inner.top() + 68.0 + 2.0 * swatch;
        let style_label = tr("StyleChooserBoxHeading.Text");
        painter.text(
            pos2(inner.left(), style_top),
            Align2::LEFT_TOP,
            style_label,
            FontId::proportional(14.0),
            p.text_secondary,
        );
        let combo = Rect::from_min_size(pos2(inner.left(), style_top + 22.0), vec2(200.0, 32.0));
        let style = self.graphing.rows[index].style;
        let enabled = !inequality;
        // The combo keeps the keyboard focus that opened its list.
        access::set_covered(ui.ctx(), false);
        let response = access::control(
            ui,
            p,
            combo,
            style_combo_id(),
            enabled,
            RADIUS,
            access::combo_box(style_label, line_style_name(style)),
        );
        access::set_covered(ui.ctx(), styles_open);
        let fill = if !enabled {
            p.control.gamma_multiply(0.5)
        } else if response.is_pointer_button_down_on() {
            p.control_pressed
        } else if response.hovered() {
            p.control_hover
        } else {
            p.control
        };
        painter.rect_filled(combo, RADIUS, fill);
        painter.rect_stroke(
            combo,
            RADIUS,
            Stroke::new(1.0, p.control_border),
            StrokeKind::Inside,
        );
        let line_color = if enabled { p.text } else { p.text_disabled };
        paint_style_line(
            &painter,
            (combo.left() + 12.0)..=(combo.right() - 44.0),
            combo.center().y,
            style,
            line_color,
        );
        let chevron = painter.layout_job(icon_label(ICON_CHEVRON_DOWN, 12.0));
        painter.galley(
            pos2(
                combo.right() - 26.0,
                combo.center().y - chevron.size().y / 2.0,
            ),
            chevron,
            line_color,
        );
        let mut open = styles_open;
        if enabled && response.clicked() {
            open = !styles_open;
        }
        if open {
            // A click elsewhere on the flyout closes only the list, as a drop-down's
            // light-dismiss layer does.
            access::set_covered(ui.ctx(), false);
            if ui
                .interact(rect, Id::new("graph-style-backdrop"), Sense::CLICK)
                .clicked()
            {
                open = false;
            }
            let list = drop_down_rect(combo, 3, full);
            popup_frame(p, &painter, list, p.popup);
            // Its edges don't click the controls under it.
            ui.interact(list, Id::new("graph-style-list"), Sense::CLICK);
            for (k, s) in [LineStyle::Solid, LineStyle::Dash, LineStyle::Dot]
                .into_iter()
                .enumerate()
            {
                let item = Rect::from_min_size(
                    list.min + vec2(4.0, 4.0 + k as f32 * 36.0),
                    vec2(list.width() - 8.0, 36.0),
                );
                let selected = style == s;
                let info = access::item(WidgetType::SelectableLabel, line_style_name(s), selected);
                let response = access::control(
                    ui,
                    p,
                    item,
                    Id::new("graph-style-item").with(k),
                    true,
                    RADIUS,
                    info,
                );
                let fill = if response.is_pointer_button_down_on() {
                    p.popup_item_pressed
                } else if response.hovered() || selected {
                    p.popup_item
                } else {
                    Color32::TRANSPARENT
                };
                painter.rect_filled(item, RADIUS, fill);
                paint_style_line(
                    &painter,
                    (item.left() + 14.0)..=(item.right() - 14.0),
                    item.center().y,
                    s,
                    p.text,
                );
                if response.clicked() {
                    self.graphing.rows[index].style = s;
                    open = false;
                }
            }
        }
        if open != styles_open {
            if !open {
                let item = Id::new("graph-style-item");
                self.drop_down_closed(ui.ctx(), style_combo_id(), item, 3);
            }
            self.graphing.popup = Some(Popup::Style {
                row: id,
                anchor,
                styles_open: open,
            });
        }
        false
    }

    /// Gives the keyboard focus back to a closing drop-down's `combo` if it was on the combo
    /// or one of its `items` (`item.with(k)`), as WinUI's ComboBox does when it holds the
    /// focus (`FinishClosingDropDown`), so that Enter and Tab go on from there. It looks at
    /// last frame's focus, which a click elsewhere has already taken from a list item. After
    /// the mouse alone nothing has the focus and nothing changes, as WinUI's pointer focus
    /// shows no focus visual.
    fn drop_down_closed(&self, ctx: &Context, combo: Id, item: Id, items: usize) {
        let in_it = |id: Id| id == combo || (0..items).any(|k| id == item.with(k));
        if self.last_focus.is_some_and(in_it) {
            ctx.memory_mut(|m| m.request_focus(combo));
        }
    }

    /// While the Esc that closed a drop-down is held, takes its repeats (egui's
    /// `key_pressed` counts them too) until its key-up, so that they don't close the flyout
    /// as well, as WinUI's ComboBox does (`m_ignoreCancelKeyDowns`). egui's Esc rule has
    /// already taken the focus from the combo for each, so last frame's focus comes back.
    fn swallow_held_escape(&mut self, ctx: &Context) {
        if !self.graphing.esc_held {
            return;
        }
        let mut held = true;
        let mut swallowed = false;
        ctx.input_mut(|i| {
            i.events.retain(|event| match event {
                egui::Event::Key {
                    key: Key::Escape,
                    pressed,
                    ..
                } if held => {
                    held = *pressed;
                    swallowed |= *pressed;
                    !*pressed
                }
                _ => true,
            });
            // A window that loses the focus lets go of every key without a key-up.
            held &= i.key_down(Key::Escape);
        });
        if swallowed && let Some(id) = self.last_focus {
            ctx.memory_mut(|m| m.request_focus(id));
        }
        self.graphing.esc_held = held;
    }

    /// An equation box's context menu (`MathRichEditContextMenu`).
    fn draw_row_menu(&mut self, ui: &mut Ui, id: u64, at: Pos2) -> bool {
        let Some(index) = self.graphing.index_of(id) else {
            return true;
        };
        let ctx = ui.ctx().clone();
        let edit = edit_id(id);
        let state = TextEdit::load_state(&ctx, edit).unwrap_or_default();
        let len = self.graphing.rows[index].text.chars().count();
        let range = state
            .cursor
            .char_range()
            .map(|r| r.as_sorted_char_range())
            .map(|r| r.start.0..r.end.0)
            .unwrap_or(len..len);
        let selected = !range.is_empty();
        let last = index + 1 == self.graphing.rows.len();
        let row = &self.graphing.rows[index];
        let valid = matches!(row.equation, Some(Ok(_))) && !row.text.is_empty();
        let error = row.error().is_some();
        let undoable = state.undoer().has_undo(&(
            state.cursor.char_range().unwrap_or_default(),
            row.text.clone(),
        ));
        let items = [
            (Some(ICON_CUT), tr("cutEquationMenuItem.Text"), selected),
            (Some(ICON_COPY), tr("copyEquationMenuItem.Text"), selected),
            (Some(ICON_PASTE), tr("pasteEquationMenuItem.Text"), true),
            (Some(ICON_UNDO), tr("undoEquationMenuItem.Text"), undoable),
            (None, tr("selectAllEquationMenuItem.Text"), true),
            (Some(ICON_LIGHTNING), tr("functionAnalysisMenuItem"), valid),
            (
                Some(ICON_THEME),
                tr("colorChooserMenuItem"),
                !error && !last,
            ),
            (Some(ICON_REMOVE), tr("removeMenuItem"), !last),
        ];
        let chosen = self.context_menu_at(ui, at, &items, Some(5));
        let Some(choice) = chosen else {
            return false;
        };
        let text = self.graphing.rows[index].text.clone();
        let chars: Vec<char> = text.chars().collect();
        let selection: String = chars[range.start.min(len)..range.end.min(len)]
            .iter()
            .collect();
        let set = |app: &mut CalculatorApp, new_text: String, cursor: std::ops::Range<usize>| {
            app.graphing.rows[index].text = new_text;
            let mut state = TextEdit::load_state(&ctx, edit).unwrap_or_default();
            state.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(cursor.start),
                CCursor::new(cursor.end),
            )));
            state.store(&ctx, edit);
            ctx.memory_mut(|m| m.request_focus(edit));
        };
        match choice {
            0 => {
                ctx.copy_text(selection);
                let (t, r) = insert_text(&text, range.clone(), "", 0, 0);
                set(self, t, r);
            }
            1 => ctx.copy_text(selection),
            2 => {
                if let Some(pasted) = clipboard_text() {
                    // Only as much as the box takes, as a keyboard paste (`char_limit`).
                    let kept = len - range.end.min(len).saturating_sub(range.start.min(len));
                    let mut pasted: String = pasted
                        .lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(MAX_EQUATION_LENGTH.saturating_sub(kept))
                        .collect();
                    normalize(&mut pasted);
                    let n = pasted.chars().count();
                    let (t, r) = insert_text(&text, range.clone(), &pasted, n, 0);
                    set(self, t, r);
                }
            }
            3 => {
                let mut undoer = state.undoer();
                let current = (state.cursor.char_range().unwrap_or_default(), text.clone());
                if let Some((cursor, previous)) = undoer.undo(&current).cloned() {
                    self.graphing.rows[index].text = previous;
                    let mut state = TextEdit::load_state(&ctx, edit).unwrap_or_default();
                    state.cursor.set_char_range(Some(cursor));
                    state.set_undoer(undoer);
                    state.store(&ctx, edit);
                    ctx.memory_mut(|m| m.request_focus(edit));
                }
            }
            4 => set(self, text.clone(), 0..len),
            5 => self.open_analysis(index),
            6 => {
                let anchor = Rect::from_center_size(at, vec2(1.0, 1.0));
                self.graphing.popup = Some(Popup::Style {
                    row: id,
                    anchor,
                    styles_open: false,
                });
                return false;
            }
            _ => {
                self.graphing.remove_row(index);
            }
        }
        true
    }

    /// The graph's context menu: Copy (`GraphMenuFlyoutItem_Click`).
    fn draw_graph_menu(&mut self, ui: &mut Ui, at: Pos2) -> bool {
        let items = [(Some(ICON_COPY), tr("GraphCopyMenuItem.Text"), true)];
        if self.context_menu_at(ui, at, &items, None).is_some() {
            self.graphing.capture = Capture::Requested(Purpose::Copy);
            return true;
        }
        false
    }

    /// The dialog when the share sheet doesn't open (`ShowShareError`, a `ContentDialog`):
    /// the message over the dimmed window, and OK under it. Returns whether it should close.
    fn draw_share_error(&mut self, ui: &mut Ui) -> bool {
        let p = self.palette;
        let full = ui.max_rect();
        let painter = ui.painter().clone();
        painter.rect_filled(full, 0.0, p.smoke);
        // At most 548 wide, 24 px padding, the buttons in an 80 px band at the bottom.
        let width = (full.width() - 16.0).min(548.0);
        let message = painter.layout(
            tr("ShareActionErrorMessage").to_string(),
            FontId::proportional(14.0),
            p.text,
            width - 48.0,
        );
        let content = message.size().y + 48.0;
        let rect = Rect::from_center_size(full.center(), vec2(width, content + 80.0));
        popup_frame(p, &painter, rect, p.popup);
        ui.interact(rect, Id::new("share-error"), Sense::CLICK);
        painter.galley(rect.min + vec2(24.0, 24.0), message, p.text);
        let commands = rect.with_min_y(rect.top() + content);
        painter.rect_filled(
            commands.shrink(1.0),
            egui::CornerRadius {
                sw: 7,
                se: 7,
                ..Default::default()
            },
            p.background,
        );
        painter.hline(
            commands.x_range(),
            commands.top(),
            Stroke::new(1.0, p.popup_border),
        );
        // The close button, in the right half.
        let ok = tr("ShareActionErrorOk");
        let button = Rect::from_min_max(
            pos2(commands.center().x + 4.0, commands.top() + 24.0),
            pos2(commands.right() - 24.0, commands.top() + 56.0),
        );
        let response = access::control(
            ui,
            p,
            button,
            Id::new("share-error-ok"),
            true,
            RADIUS,
            access::button(true, ok),
        );
        let fill = if response.is_pointer_button_down_on() {
            p.control_pressed
        } else if response.hovered() {
            p.control_hover
        } else {
            p.control
        };
        painter.rect_filled(button, RADIUS, fill);
        painter.rect_stroke(
            button,
            RADIUS,
            Stroke::new(1.0, p.control_border),
            StrokeKind::Inside,
        );
        painter.text(
            button.center(),
            Align2::CENTER_CENTER,
            ok,
            FontId::proportional(14.0),
            p.text,
        );
        response.clicked()
    }

    /// A WinUI menu flyout at `at` with icons, a separator before `separator_before`.
    /// Returns the index of the item clicked.
    fn context_menu_at(
        &self,
        ui: &mut Ui,
        at: Pos2,
        items: &[(Option<char>, &str, bool)],
        separator_before: Option<usize>,
    ) -> Option<usize> {
        let p = self.palette;
        let full = ui.max_rect();
        let painter = ui.painter().clone();
        let widest = items
            .iter()
            .map(|(_, label, _)| {
                painter
                    .layout_no_wrap(label.to_string(), FontId::proportional(14.0), p.text)
                    .size()
                    .x
            })
            .fold(0.0, f32::max);
        let width = (widest + 44.0 + 24.0).max(140.0);
        let height =
            items.len() as f32 * 32.0 + 8.0 + if separator_before.is_some() { 9.0 } else { 0.0 };
        let left = at.x.min(full.right() - width - 4.0).max(full.left() + 4.0);
        let top = if at.y + height > full.bottom() - 4.0 {
            at.y - height
        } else {
            at.y
        };
        let rect = Rect::from_min_size(pos2(left, top.max(full.top() + 4.0)), vec2(width, height));
        popup_frame(p, &painter, rect, p.popup);
        ui.interact(rect, Id::new("graph-menu-panel"), Sense::CLICK);
        let mut y = rect.top() + 4.0;
        let mut chosen = None;
        for (k, (icon, label, enabled)) in items.iter().enumerate() {
            if separator_before == Some(k) {
                painter.hline(
                    (rect.left() + 1.0)..=(rect.right() - 1.0),
                    y + 4.0,
                    Stroke::new(1.0, p.popup_border),
                );
                y += 9.0;
            }
            let item =
                Rect::from_min_size(pos2(rect.left() + 4.0, y), vec2(rect.width() - 8.0, 32.0));
            y += 32.0;
            let info = access::button(*enabled, label);
            let response = access::control(
                ui,
                p,
                item,
                Id::new("graph-menu-item").with(k),
                *enabled,
                RADIUS,
                info,
            );
            if *enabled {
                let fill = if response.is_pointer_button_down_on() {
                    p.popup_item_pressed
                } else if response.hovered() {
                    p.popup_item
                } else {
                    Color32::TRANSPARENT
                };
                painter.rect_filled(item, RADIUS, fill);
            }
            let color = if *enabled { p.text } else { p.text_disabled };
            if let Some(icon) = icon {
                let galley = painter.layout_job(icon_label(*icon, 16.0));
                painter.galley(
                    pos2(item.left() + 12.0, item.center().y - galley.size().y / 2.0),
                    galley,
                    color,
                );
            }
            painter.text(
                pos2(item.left() + 44.0, item.center().y),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(14.0),
                color,
            );
            if *enabled && response.clicked() {
                chosen = Some(k);
            }
        }
        chosen
    }
}

/// What a box's buttons and keys asked for this frame.
#[derive(Default)]
struct RowActions {
    clear: bool,
    remove: bool,
    style: Option<Rect>,
    analyze: bool,
    submit_enter: bool,
    submit_left: bool,
}

/// The id of an equation box's text.
pub(super) fn edit_id(row: u64) -> Id {
    Id::new("graph-equation").with(row)
}

/// The id of a window box of the graph options: 0 is X-Min, then X-Max, Y-Min, Y-Max.
fn range_id(k: usize) -> Id {
    Id::new("graph-range-box").with(k)
}

/// A child `Ui` for a text box at `rect`, with the selection and caret in the app's
/// colors. Unlike `scope_builder`, it doesn't move the parent's layout cursor, which
/// matters in the scrolling lists.
fn text_ui(ui: &mut Ui, rect: Rect, p: &super::palette::Palette) -> Ui {
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect));
    child.visuals_mut().selection.bg_fill = p.accent.gamma_multiply(0.6);
    child.visuals_mut().selection.stroke = Stroke::new(1.0, p.text);
    child.visuals_mut().text_cursor.stroke = Stroke::new(1.5, p.text);
    child
}

/// Enter in the window box `k` moves the focus to the next box
/// (`GridSettingsTextBox_PreviewKeyDown`). Like Windows (`e.Handled = true`), it consumes
/// the key: the next box is drawn later in the same frame, and the same Enter would end
/// its input and take the focus away again.
fn enter_to_next_box(ctx: &Context, k: usize, response: &egui::Response) {
    if response.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
        ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        ctx.memory_mut(|m| m.request_focus(range_id((k + 1) % 4)));
    }
}

/// What a window box holds: the view's `value` until something else is typed in it, or
/// `None` while the text isn't a number. The box shows the value with 6 digits, so
/// reading that text back would move the view by a click into the box.
fn range_value(text: &str, value: f64, typed: bool) -> Option<f64> {
    if !typed || text.trim() == numbers().localize_plain(&g6(value)) {
        return Some(value);
    }
    numbers()
        .delocalize(text.trim())
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

/// The window boxes that show an error, as Windows' `SelectTextBoxStyle` picks them: one
/// that isn't a number (`XMinError` …), and both boxes of a pair whose minimum isn't
/// below its maximum (`XError`, `YError`). A pair the graph can't draw (`View::is_valid`:
/// a span over 1e100, or too small for doubles) is one too, as the view can't take it.
fn range_errors(values: [Option<f64>; 4]) -> [bool; 4] {
    let mut errors = values.map(|v| v.is_none());
    for min in [0, 2] {
        if let (Some(a), Some(b)) = (values[min], values[min + 1]) {
            // The pair with the default view's other one, which can be drawn.
            let pair = if min == 0 {
                View {
                    x_min: a,
                    x_max: b,
                    ..View::DEFAULT
                }
            } else {
                View {
                    y_min: a,
                    y_max: b,
                    ..View::DEFAULT
                }
            };
            if a >= b || !pair.is_valid() {
                errors[min] = true;
                errors[min + 1] = true;
            }
        }
    }
    errors
}

/// Where the list of a drop-down with `items` 36 px items opens: under its combo box,
/// or over it when the window ends first (the line thickness list in a 500 px high
/// window).
fn drop_down_rect(combo: Rect, items: usize, full: Rect) -> Rect {
    let size = vec2(combo.width(), items as f32 * 36.0 + 8.0);
    let mut top = combo.bottom() + 2.0;
    if top + size.y > full.bottom() - 4.0 {
        top = (combo.top() - 2.0 - size.y).max(full.top() + 4.0);
    }
    Rect::from_min_size(pos2(combo.left(), top), size)
}

/// A tooltip for a control drawn by hand.
fn tooltip(ui: &Ui, rect: Rect, id: Id, text: &str) {
    let response = ui.interact(rect, id.with("tip"), Sense::hover());
    response.on_hover_text(text);
}

/// The clipboard's text, for Paste from the menu (keyboard pastes come as egui events).
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn clipboard_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

/// The clipboard's text as the browser gave it when the menu opened: browsers only read
/// the clipboard asynchronously (`web::prefetch_clipboard`).
#[cfg(target_arch = "wasm32")]
fn clipboard_text() -> Option<String> {
    super::web::clipboard_text()
}

/// No clipboard in the Android app yet: Paste from the menu does nothing.
#[cfg(target_os = "android")]
fn clipboard_text() -> Option<String> {
    None
}

/// An analysis value in the region's format: a decimal comma takes `;` between the
/// coordinates, as Windows' `DecimalCommaAndListSemicolon`.
fn localize_math(text: &str) -> String {
    let decimal = region_decimal();
    if decimal == ',' {
        text.replace(", ", "; ").replace('.', ",")
    } else if decimal != '.' {
        text.replace('.', &decimal.to_string())
    } else {
        text.to_string()
    }
}

/// The names of the 14 colors (`equationColorNAutomationName`); the 14th is black in a
/// light graph and white in a dark one.
fn color_name(c: usize, graph: &GraphPalette) -> &'static str {
    match c {
        13 if graph.background == Color32::WHITE => tr("equationColor14BlackAutomationName"),
        13 => tr("equationColor14WhiteAutomationName"),
        c => tr(&format!("equationColor{}AutomationName", c + 1)),
    }
}

fn line_style_name(style: LineStyle) -> &'static str {
    match style {
        LineStyle::Solid => tr("solidLineStyleAutomationName"),
        LineStyle::Dash => tr("dashLineStyleAutomationName"),
        LineStyle::Dot => tr("dotLineStyleAutomationName"),
    }
}

fn line_width_name(width: f32) -> &'static str {
    match width as u32 {
        1 => tr("SmallLineWidthAutomationName"),
        2 => tr("MediumLineWidthAutomationName"),
        3 => tr("LargeLineWidthAutomationName"),
        _ => tr("ExtraLargeLineWidthAutomationName"),
    }
}

/// A sample of a line style, as the style list shows it.
fn paint_style_line(
    painter: &Painter,
    x: std::ops::RangeInclusive<f32>,
    y: f32,
    style: LineStyle,
    color: Color32,
) {
    let points = [pos2(*x.start(), y), pos2(*x.end(), y)];
    let stroke = Stroke::new(2.5, color);
    match style {
        LineStyle::Solid => {
            painter.line_segment(points, stroke);
        }
        LineStyle::Dash => {
            painter.extend(Shape::dashed_line(&points, stroke, 5.0, 2.5));
        }
        LineStyle::Dot => {
            painter.extend(Shape::dashed_line(&points, stroke, 2.5, 2.5));
        }
    }
}

/// The Graphing mode's icon: axes and a rising line, like Windows' (which comes from the
/// Calculator's own font).
pub(super) fn paint_graph_icon(painter: &Painter, center: Pos2, size: f32, color: Color32) {
    let s = size / 16.0;
    let at = |x: f32, y: f32| center + vec2(x * s, y * s);
    let stroke = Stroke::new(1.0 * s.max(1.0), color);
    painter.add(Shape::line(
        vec![at(-7.0, -7.0), at(-7.0, 7.0), at(7.0, 7.0)],
        stroke,
    ));
    painter.add(Shape::line(
        vec![at(-5.0, 4.0), at(-1.5, -0.5), at(1.5, 2.0), at(6.5, -4.5)],
        stroke,
    ));
}

/// What the analysis panel lists: a title and its lines (or a lone message).
struct Item {
    title: Option<String>,
    lines: Vec<String>,
}

/// The analysis panel's items, in Windows' order (`PopulateKeyGraphFeatures`).
fn analysis_items(result: &Result<Features, AnalysisError>) -> Vec<Item> {
    let features = match result {
        Ok(f) => f,
        Err(e) => {
            return vec![Item {
                title: None,
                lines: vec![tr(e.key()).to_string()],
            }];
        }
    };
    let mut items = Vec::new();
    let mut one = |title: &str, value: &Option<String>, none: &str| {
        items.push(Item {
            title: Some(tr(title).to_string()),
            lines: vec![value.clone().unwrap_or_else(|| tr(none).to_string())],
        });
    };
    one("Domain", &features.domain, "KGFDomainNone");
    one("Range", &features.range, "KGFRangeNone");
    one("XIntercept", &features.x_intercepts, "KGFXInterceptNone");
    one("YIntercept", &features.y_intercept, "KGFYInterceptNone");
    let mut many = |title: &str, values: &[String], none: &str| {
        items.push(Item {
            title: Some(tr(title).to_string()),
            lines: if values.is_empty() {
                vec![tr(none).to_string()]
            } else {
                values.to_vec()
            },
        });
    };
    many("Minima", &features.minima, "KGFMinimaNone");
    many("Maxima", &features.maxima, "KGFMaximaNone");
    many(
        "InflectionPoints",
        &features.inflection_points,
        "KGFInflectionPointsNone",
    );
    many(
        "VerticalAsymptotes",
        &features.vertical_asymptotes,
        "KGFVerticalAsymptotesNone",
    );
    many(
        "HorizontalAsymptotes",
        &features.horizontal_asymptotes,
        "KGFHorizontalAsymptotesNone",
    );
    many(
        "ObliqueAsymptotes",
        &features.oblique_asymptotes,
        "KGFObliqueAsymptotesNone",
    );
    items.push(Item {
        title: Some(tr("Parity").to_string()),
        lines: vec![tr(features.parity.key()).to_string()],
    });
    if let Period::Periodic(period) = &features.period {
        items.push(Item {
            title: Some(tr("Periodicity").to_string()),
            lines: vec![period.clone()],
        });
    }
    let monotonicity: Vec<String> = features
        .monotonicity
        .iter()
        .map(|(interval, direction)| format!("{interval}  {}", tr(direction.key())))
        .collect();
    items.push(Item {
        title: Some(tr("Monotonicity").to_string()),
        lines: if monotonicity.is_empty() {
            vec![tr("KGFMonotonicityError").to_string()]
        } else {
            monotonicity
        },
    });
    if !features.too_complex.is_empty() {
        let separator = super::region::formats().list_separator.clone();
        let names: Vec<&str> = features.too_complex.iter().map(|f| tr(f.key())).collect();
        items.push(Item {
            title: None,
            lines: vec![
                tr("KGFTooComplexFeaturesError").to_string(),
                names.join(&separator),
            ],
        });
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_and_numbers() {
        let mut g = GraphingState::new();
        assert_eq!(g.rows[0].label, 1);
        assert_eq!(g.rows[0].color, 0);
        g.add_row();
        g.add_row();
        // The second takes EquationBrush4, the third EquationBrush8.
        assert_eq!(g.rows[1].color, 3);
        assert_eq!(g.rows[2].color, 7);
        assert_eq!(g.rows[2].label, 3);
        // Removing one renumbers the last after the one before it.
        g.remove_row(0);
        assert_eq!(g.rows.len(), 2);
        assert_eq!(g.rows[1].label, g.rows[0].label + 1);
        // The last box can't be removed.
        g.remove_row(1);
        assert_eq!(g.rows.len(), 2);
        // A freed color is the first taken again.
        g.add_row();
        assert_eq!(g.rows[2].color, 0);
        for _ in 0..20 {
            g.add_row();
        }
        assert_eq!(g.rows.len(), MAX_EQUATIONS);
    }

    #[test]
    fn variables_follow_the_equations() {
        let mut g = GraphingState::new();
        g.rows[0].text = "y=mx+b".into();
        g.submit(0, Syntax::default());
        let names: Vec<char> = g.variables.iter().map(|v| v.name).collect();
        assert_eq!(names, vec!['b', 'm']);
        assert_eq!(g.variables[0].value, 1.0);
        g.variables[1].set_value(3.0);
        g.rows[0].text = "y=mx".into();
        g.submit(0, Syntax::default());
        assert_eq!(g.variables.len(), 1);
        // m keeps its value.
        assert_eq!(g.variables[0].value, 3.0);
        // A hidden equation's variables go.
        g.rows[0].visible = false;
        g.equations_changed();
        assert!(g.variables.is_empty());
    }

    #[test]
    fn variable_limits() {
        let mut v = Variable::new('a');
        v.set_value(8.0);
        assert_eq!(v.max, 8.0);
        // A new range pulls the value in, as Windows' slider does.
        v.set_min(9.0);
        assert_eq!((v.min, v.max, v.value), (9.0, 19.0, 9.0));
        v.set_max(2.0);
        assert_eq!((v.min, v.max, v.value), (-8.0, 2.0, 2.0));
        // A value inside the range stays.
        v.set_value(1.0);
        v.set_min(-1.0);
        v.set_max(3.0);
        assert_eq!(v.value, 1.0);
    }

    #[test]
    fn huge_sliders() {
        // From −1e308 to 1e308 the span overflows; the thumb and the value don't.
        let (min, max) = (-1e308, 1e308);
        assert_eq!(slider_fraction(1.0, min, max), 0.5);
        assert_eq!(slider_fraction(max, min, max), 1.0);
        for t in [0.0, 0.25, 0.5, 1.0] {
            let value = slider_value(t, min, max, 0.1);
            assert!(
                value.is_finite() && (min..=max).contains(&value),
                "{t}: {value}"
            );
        }
        // An ordinary slider snaps to its steps.
        assert_eq!(slider_value(0.5, -5.0, 5.0, 0.1), 0.0);
        assert_eq!(slider_value(0.62, -5.0, 5.0, 0.1), 1.2);
        assert_eq!(slider_value(1.0, -5.0, 5.0, 3.0), 4.0);
        assert_eq!(slider_fraction(1.0, -5.0, 5.0), 0.6);
    }

    #[test]
    fn small_slider_steps() {
        // A slider for a small parameter: 0 to 1e-12 in steps of 1e-14. Dragging
        // reaches every step.
        assert_eq!(slider_value(0.52, 0.0, 1e-12, 1e-14), 5.2e-13);
        let values: std::collections::BTreeSet<u64> = (0..=100)
            .map(|i| slider_value(i as f64 / 100.0, 0.0, 1e-12, 1e-14).to_bits())
            .collect();
        assert_eq!(values.len(), 101);
        // The arrow keys move it by a step (`SmallChange`): from 5e-13, two up and one
        // down, as `draw_slider` adds them.
        let up = |v: f64| clean(v + 1e-14, v, 1e-14);
        let down = |v: f64| clean(v - 1e-14, v, 1e-14);
        assert_eq!(up(up(5e-13)), 5.2e-13);
        assert_eq!(down(up(up(5e-13))), 5.1e-13);
        // Ordinary steps still lose the noise: 0.1 + 0.2 is 0.3, and −100000 plus 1000001
        // steps of 0.1 is 0.1 (to 12 decimals it was 0.100000000006).
        assert_eq!(clean(0.1 + 0.2, 0.1, 0.1), 0.3);
        assert_eq!(clean(-100000.0 + 1000001.0 * 0.1, -100000.0, 0.1), 0.1);
        // A step keeps all its digits: dragging to the end, or one step from the start,
        // reaches a maximum one step away.
        let step = 1.23456789012345e-14;
        assert_eq!(slider_value(1.0, 0.0, step, step), step);
        assert_eq!(clean(0.0 + step, 0.0, step), step);
        assert_eq!((decimals(0.1), decimals(-5.0), decimals(step)), (1, 0, 28));
    }

    #[test]
    fn range_boxes() {
        let shown = |v: f64| numbers().localize_plain(&g6(v));
        // A click into a box doesn't move the view: its text is the value rounded.
        let x = -27.220412;
        assert_eq!(range_value(&shown(x), x, true), Some(x));
        assert_eq!(range_value(&shown(x), x, false), Some(x));
        // Something else typed in is read.
        assert_eq!(range_value(&shown(-20.0), x, true), Some(-20.0));
        assert_eq!(range_value("", x, true), None);
        assert_eq!(range_value("", x, false), Some(x));
    }

    #[test]
    fn range_boxes_keep_a_minimum_above_the_maximum() {
        // An empty graph's options (x from −10 to 10): X-Min 20, Tab, X-Max 30, Tab.
        let mut g = GraphingState::new();
        g.update_range(None);
        g.range_texts[0] = "20".into();
        g.update_range(Some(0));
        // X-Min keeps what was typed, and both X boxes show the error, as in Windows.
        let errors = g.update_range(Some(1));
        assert_eq!(g.range_texts[0], "20");
        assert_eq!(errors, [true, true, false, false]);
        g.range_texts[1] = "30".into();
        assert_eq!(g.update_range(Some(1)), [false; 4]);
        g.update_range(Some(2));
        assert_eq!((g.view.x_min, g.view.x_max), (20.0, 30.0));
        assert_eq!(g.range_texts[..2], ["20", "30"]);
    }

    #[test]
    fn range_boxes_keep_a_window_the_graph_cant_draw() {
        // X-Max 1e101 is in order, but the graph draws spans up to 1e100 (`is_valid`):
        // it stays, as an error, rather than being dropped.
        let mut g = GraphingState::new();
        g.update_range(None);
        g.range_texts[1] = "1e101".into();
        g.update_range(Some(1));
        let errors = g.update_range(Some(2));
        assert_eq!(g.range_texts[1], "1e101");
        assert_eq!(errors, [true, true, false, false]);
        assert_eq!(g.view, View::DEFAULT);
        // So does a span too small for doubles.
        g.range_texts[1] = "10".into();
        g.range_texts[3] = "-9.99999999999".into();
        assert_eq!(g.update_range(Some(3)), [false, false, true, true]);
        assert_eq!(g.update_range(None), [false, false, true, true]);
        assert_eq!(g.range_texts[3], "-9.99999999999");
    }

    #[test]
    fn range_boxes_start_over_on_reset_view() {
        let mut g = GraphingState::new();
        g.update_range(None);
        g.range_texts[0] = "20".into();
        g.update_range(Some(0));
        // "Reset view" while X-Max has the focus empties the boxes, and the view is
        // fitted again before they're read next.
        g.update_range(Some(1));
        g.reset_view();
        g.range_texts = Default::default();
        g.update_range(Some(1));
        let fitted = View {
            x_min: -12.0,
            ..View::DEFAULT
        };
        g.view = fitted;
        assert_eq!(g.update_range(None), [false; 4]);
        assert_eq!(g.view, fitted);
        assert!(!g.manual);
        assert_eq!(g.range_texts[0], "-12");
    }

    #[test]
    fn drop_downs_fit_the_window() {
        let full = Rect::from_min_size(Pos2::ZERO, vec2(320.0, 500.0));
        // Under the combo box when there's room.
        let combo = Rect::from_min_size(pos2(20.0, 100.0), vec2(200.0, 32.0));
        let list = drop_down_rect(combo, 4, full);
        assert_eq!(list.top(), combo.bottom() + 2.0);
        // The line thickness list in a 320 × 500 window opens over it.
        let combo = Rect::from_min_size(pos2(20.0, 363.0), vec2(278.0, 32.0));
        let list = drop_down_rect(combo, 4, full);
        assert_eq!(list.bottom(), combo.top() - 2.0);
        assert!(full.contains_rect(list));
    }

    #[test]
    fn enter_moves_to_the_next_range_box() {
        // The four window boxes, each with its Enter handling, in a bare egui context.
        let ctx = Context::default();
        let mut texts: [String; 4] = Default::default();
        let mut frame = |events: Vec<egui::Event>| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0))),
                events,
                ..Default::default()
            };
            // The font atlas has no renderer to go to.
            ctx.run_ui(input, |ui| {
                for (k, text) in texts.iter_mut().enumerate() {
                    let output = TextEdit::singleline(text).id(range_id(k)).show(ui);
                    enter_to_next_box(ui.ctx(), k, &output.response.response);
                }
            })
            .textures_delta
            .clear();
        };
        let enter = egui::Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        let focused = |ctx: &Context| (0..4).find(|&k| ctx.memory(|m| m.has_focus(range_id(k))));
        ctx.memory_mut(|m| m.request_focus(range_id(0)));
        frame(vec![]);
        // X-Min, X-Max, Y-Min, Y-Max, and from there back to X-Min.
        for next in [1, 2, 3, 0] {
            frame(vec![enter.clone()]);
            frame(vec![]);
            assert_eq!(focused(&ctx), Some(next));
        }
    }

    #[test]
    fn reparsing_refreshes_the_analysis() {
        let mut g = GraphingState::new();
        g.rows[0].text = "y=x^2".into();
        g.submit(0, Syntax::default());
        g.analysis = Some(Analysis {
            row: g.rows[0].id,
            result: Err(AnalysisError::NotSupported),
        });
        g.reparse(Syntax::for_decimal(','));
        assert!(g.analysis.as_ref().is_some_and(|a| a.result.is_ok()));
        // An equation that no longer parses closes it.
        g.rows[0].submitted = "y=x)".into();
        g.reparse(Syntax::default());
        assert!(g.analysis.is_none());
    }

    #[test]
    fn inserting_text() {
        // The keypad's sin() puts the cursor between the parentheses.
        assert_eq!(
            insert_text("2x", 2..2, "sin()", 4, 0),
            ("2xsin()".into(), 6..6)
        );
        // root(x,n) selects the n.
        assert_eq!(
            insert_text("", 0..0, "root(x,n)", 7, 1),
            ("root(x,n)".into(), 7..8)
        );
        // A selection is replaced.
        assert_eq!(insert_text("y=x", 2..3, "π", 1, 0), ("y=π".into(), 3..3));
    }

    #[test]
    fn g6_numbers() {
        assert_eq!(g6(27.220412), "27.2204");
        assert_eq!(g6(-3.025), "-3.025");
        assert_eq!(g6(30.25), "30.25");
        assert_eq!(g6(-10.0), "-10");
        assert_eq!(g6(0.1), "0.1");
        assert_eq!(g6(1234567.0), "1.23457E+06");
        assert_eq!(g6(0.0), "0");
        // The exponent is the rounded number's.
        assert_eq!(g6(9999996.0), "1E+07");
        assert_eq!(g6(999999.7), "1E+06");
        assert_eq!(g6(999999.0), "999999");
        assert_eq!(g6(99999.97), "100000");
        // Fixed point down to an exponent of −4, scientific below.
        assert_eq!(g6(0.0001), "0.0001");
        assert_eq!(g6(-0.00012345678), "-0.000123457");
        assert_eq!(g6(0.000012345), "1.2345E-05");
        assert_eq!(g6(-1e-300), "-1E-300");
        assert_eq!(g6(1e308), "1E+308");
    }

    #[test]
    fn math_layout() {
        let job = math_job("y=mx^2+sin(x)", 14.0, Color32::WHITE, Syntax::default());
        // The format of character k (sections with the same format merge).
        let style = |k: usize| {
            let at = job.text.char_indices().nth(k).unwrap().0;
            &job.sections
                .iter()
                .find(|s| s.byte_range.start.0 <= at && at < s.byte_range.end.0)
                .unwrap()
                .format
        };
        // y, m and x in italics; sin upright; the 2 raised; the ^ hidden.
        assert!(style(0).italics);
        assert!(style(2).italics && style(3).italics);
        assert_eq!(style(4).color, Color32::TRANSPARENT);
        assert_eq!(style(5).valign, Align::TOP);
        assert!(!style(7).italics);
        // Space around = and the binary +.
        assert!(style(0).extra_letter_spacing > 0.0 && style(1).extra_letter_spacing > 0.0);
        assert!(style(6).extra_letter_spacing > 0.0);
        // A leading minus isn't spaced.
        let job = math_job("−x", 14.0, Color32::WHITE, Syntax::default());
        assert_eq!(job.sections[0].format.extra_letter_spacing, 0.0);
    }

    #[test]
    fn typed_operators() {
        let mut s = "2*x-1".to_string();
        normalize(&mut s);
        assert_eq!(s, "2·x−1");
    }

    #[test]
    fn exponents_end_at_the_argument_separator() {
        // The characters `math_job` raises.
        let raised = |text: &str, syntax: Syntax| -> String {
            let job = math_job(text, 14.0, Color32::WHITE, syntax);
            text.char_indices()
                .filter(|&(at, _)| {
                    job.sections
                        .iter()
                        .find(|s| s.byte_range.start.0 <= at && at < s.byte_range.end.0)
                        .is_some_and(|s| s.format.valign == Align::TOP)
                })
                .map(|(_, c)| c)
                .collect()
        };
        // With a decimal point, a comma separates the arguments.
        let point = Syntax::default();
        assert_eq!(raised("root(x^2,3)", point), "2");
        assert_eq!(raised("log(x^2,8)", point), "2");
        assert_eq!(raised("x^2.5", point), "2.5");
        // With a decimal comma it's in the number, and a semicolon separates them.
        let comma = Syntax::for_decimal(',');
        assert_eq!(raised("x^2,5", comma), "2,5");
        assert_eq!(raised("root(x^2;3)", comma), "2");
    }
}
