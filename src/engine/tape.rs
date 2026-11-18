//! The tape: what was done since the last `C`, to edit a number of the expression line and
//! work the calculation out again (CalcCraft's own; Windows has nothing like it, see
//! `docs/research/editable-expression.md`, options B and C).
//!
//! Re-reading the expression line would need a second evaluator that imitates every quirk of
//! `ProcessCommandWorker`. Replaying the keys through the same engine is exact by
//! construction instead: an edit replaces one typed number on the tape and replays the whole
//! tape into a fresh engine, which ends in the state that typing the edited keys would give.
//!
//! - A typed number is one `Item::Operand` with the keys that typed it (⌫ and ± included), so
//!   a replay types it the same way. An edited number is typed as its digits
//!   (`Input::keys`).
//! - MR, a history recall and `rand` replay the value they gave. MS, M+, M− and MC replay only
//!   their effect on the calculation: a replay doesn't touch the memory, and adds no history.
//! - The tape starts at `C` (and with the engine). The state after `C` is a fresh engine with
//!   the same angle unit, F-E and integer mode, which the property test in `engine.rs`
//!   checks. A tape longer than `MAX_ITEMS` stops recording until the next `C`.

use std::ops::Range;

use super::{
    Angle, CalcError, Calculator, Caret, Command, Func, HISTORY_LIMIT, HistoryEntry, Input,
    IntegerMode, Key, Number, Radix, math,
};

/// The most commands a tape records; a longer calculation can't be edited.
const MAX_ITEMS: usize = 400;
/// The most keys one typed number records (⌫ by ⌫).
const MAX_KEYS: usize = 200;

/// One step on the tape.
#[derive(Clone, Debug)]
pub(super) enum Item {
    /// A typed number.
    Operand(Operand),
    /// A command sent to `execute`.
    Command(Command),
    /// MS, M+, M− or MC: replayed as their effect on the calculation only.
    Memory(Command),
    /// MR or a memory item's value.
    Recall(Number),
    /// `rand` and the number it gave.
    Random(Number),
    /// A command paste sends to the engine directly (its CE or C).
    Raw(Command),
    /// Pasted text that wasn't valid.
    PasteError,
    /// A history entry without a tape recalled: its result and expression.
    HistoryRecall(Number, String),
    /// A history entry with a tape recalled: the entry's items come before this one, which
    /// leaves the engine as a recall does (the result after `C`, the line kept), with the angle
    /// unit and F-E of the moment of the recall.
    Recalled { angle: Angle, notation: bool },
}

/// A typed number on the tape.
#[derive(Clone, Debug)]
pub(super) struct Operand {
    keys: Vec<Command>,
    /// The number the keys typed (without a caret).
    input: Input,
    /// The integer mode it was typed in (its radix and word size limit the digits).
    integer: Option<IntegerMode>,
    /// How typing it ended: the kind of its last key (a `(` after a digit multiplies, after ±
    /// it doesn't) and whether the expression line was hidden (after `=` every key hides
    /// it, ⌫ too).
    last_key: Key,
    hidden: bool,
    /// `keys` were made from `input` (`Input::keys`) after the caret edited it: a replay then
    /// ends the number as it ended, since those keys can end otherwise.
    regenerated: bool,
}

impl Operand {
    /// Ends a number typed with regenerated keys as the original number ended.
    fn finish(&self, calc: &mut Calculator) {
        if !self.regenerated {
            return;
        }
        calc.this_key = self.last_key;
        if self.hidden {
            calc.hide_expression();
        } else if calc.shown.is_empty() {
            // While a number is typed the line shows what it had when the number started.
            calc.refresh();
        }
    }
}

/// What was done since the last `C`, and the settings it started with.
#[derive(Clone, Debug)]
pub(super) struct Tape {
    angle: Angle,
    notation: bool,
    integer: Option<IntegerMode>,
    items: Vec<Item>,
    /// Too long: nothing more is recorded, and nothing can be edited until the next `C`.
    full: bool,
}

impl Default for Tape {
    fn default() -> Self {
        Self {
            angle: Angle::Degrees,
            notation: false,
            integer: None,
            items: Vec::new(),
            full: false,
        }
    }
}

impl Tape {
    /// The typed number the tape ends with, while it is still being typed.
    pub(super) fn open_operand(&self) -> Option<usize> {
        match self.items.last() {
            Some(Item::Operand(_)) => Some(self.items.len() - 1),
            _ => None,
        }
    }

    fn operand(&self, item: usize) -> Option<&Operand> {
        match self.items.get(item) {
            Some(Item::Operand(operand)) => Some(operand),
            _ => None,
        }
    }
}

/// A typed number in the expression line: where it is in `Calculator::expression`, and the
/// tape item that typed it (to edit it with `Calculator::edit_operand`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpressionPart {
    pub range: Range<usize>,
    pub item: usize,
}

/// The number of the expression line being edited, for the UI to draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumberEdit {
    /// Its tape item, the `item` of its `ExpressionPart`.
    pub item: usize,
    /// The number as typed, en-US: `-12.5`, `1.5e+3`.
    pub text: String,
    /// Its digits with the decimal point, and the exponent's digits (as `entry_characters`).
    pub mantissa: String,
    pub exponent: Option<String>,
    pub caret: Caret,
}

/// An edit in progress.
#[derive(Clone, Debug)]
pub(super) enum Edit {
    /// A number of the expression line.
    Line(LineEdit),
    /// A copy of the computed value the display shows, which replaces the tape from `start`
    /// on (the last operand) once a key edits it.
    Display {
        start: usize,
        input: Input,
        integer: Option<IntegerMode>,
    },
}

#[derive(Clone, Debug)]
pub(super) struct LineEdit {
    item: usize,
    /// The number, with its caret.
    input: Input,
    integer: Option<IntegerMode>,
    last_key: Key,
    hidden: bool,
    /// The engine before the edit, for Esc.
    original: Box<Calculator>,
    /// The line ended with `=`: the edit makes a new calculation, which goes into the history
    /// when it is committed.
    evaluated: bool,
    changed: bool,
    /// The line when the edit started, with the parts' places.
    template: (String, Vec<ExpressionPart>),
    /// What `=` gives for the edited calculation: the display while editing.
    preview: Result<String, CalcError>,
}

impl Calculator {
    // ----------------------------------------------------------------- recording

    /// Starts a new tape with the current settings (at `C`, a mode switch, a history recall).
    pub(super) fn restart_tape(&mut self) {
        self.tape = Tape {
            angle: self.angle,
            notation: self.notation,
            integer: self.integer,
            items: Vec::new(),
            full: false,
        };
        self.line.item = 0;
    }

    /// Puts a step on the tape.
    pub(super) fn record_item(&mut self, item: Item) {
        if self.tape.full {
            return;
        }
        if self.tape.items.len() >= MAX_ITEMS {
            self.tape.full = true;
            return;
        }
        self.tape.items.push(item);
        self.line.item = self.tape.items.len() - 1;
    }

    /// Puts a command sent to `execute` on the tape: a key that types a number goes into its
    /// operand.
    pub(super) fn record_command(&mut self, command: Command) {
        if self.tape.full {
            return;
        }
        let open = self.tape.open_operand().filter(|_| self.record);
        let decimal = self.radix().is_none_or(|radix| radix == Radix::Decimal);
        let typing = match command {
            Command::Digit(_) | Command::Decimal => true,
            Command::Backspace | Command::Exponent => open.is_some(),
            Command::Function(Func::Negate) => open.is_some() && decimal,
            _ => false,
        };
        match command {
            // `memory_add` and `memory_recall_item` record themselves; the tape starts
            // again after `C`.
            Command::MemoryAdd
            | Command::MemorySubtract
            | Command::MemoryRecall
            | Command::Clear => {}
            Command::MemoryStore | Command::MemoryClear => {
                self.record_item(Item::Memory(command));
            }
            Command::Random => {
                let value = self.next_random.take().unwrap_or_else(math::random);
                self.next_random = Some(value.clone());
                self.record_item(Item::Random(value));
            }
            _ if typing => match open {
                Some(item) => {
                    self.line.item = item;
                    self.typing_item = Some(item);
                    if let Some(Item::Operand(operand)) = self.tape.items.get_mut(item) {
                        operand.keys.push(command);
                        if operand.keys.len() > MAX_KEYS {
                            self.tape.full = true;
                        }
                    }
                }
                None => {
                    self.record_item(Item::Operand(Operand {
                        keys: vec![command],
                        input: Input::default(),
                        integer: self.integer,
                        last_key: Key::None,
                        hidden: false,
                        regenerated: false,
                    }));
                    self.typing_item = self.tape.open_operand();
                }
            },
            _ => self.record_item(Item::Command(command)),
        }
    }

    /// After a command: the tape restarts after `C`, and the typed number on the tape
    /// follows the input.
    pub(super) fn after_command(&mut self, command: Command) {
        if command == Command::Clear && !self.replaying {
            self.restart_tape();
        }
        self.sync_operand();
    }

    /// The typed number on the tape takes the input's digits. Once the caret has edited them,
    /// its keys are the ones that type them fresh, since keys at the end can't.
    pub(super) fn sync_operand(&mut self) {
        let Some(item) = self.tape.open_operand().filter(|_| self.record) else {
            return;
        };
        let last_key = self.this_key;
        let hidden = self.shown.is_empty();
        let caret = self.input.caret.is_some();
        let mut input = self.input.clone();
        input.caret = None;
        if let Some(Item::Operand(operand)) = self.tape.items.get_mut(item) {
            if caret {
                operand.keys = input.keys();
                operand.regenerated = true;
            }
            operand.input = input;
            operand.last_key = last_key;
            operand.hidden = hidden;
        }
    }

    /// The tape of the calculation a history entry logs now: the steps before this one, and
    /// `=` (a Standard mode operator logs `2 + 3 =` for `2 + 3 +`).
    pub(super) fn entry_tape(&self) -> Option<Box<Tape>> {
        if self.tape.full {
            return None;
        }
        let end = self.line.item.min(self.tape.items.len());
        let mut tape = Tape {
            items: self.tape.items[..end].to_vec(),
            ..self.tape.clone()
        };
        tape.items.push(Item::Command(Command::Equals));
        Some(Box::new(tape))
    }

    // ------------------------------------------------------------------- replay

    /// A fresh engine of this mode with `tape`'s settings, after `items` replayed into it.
    /// It has no memory or history: `adopt` gives it this engine's.
    fn replayed(&self, tape: &Tape, items: &[Item]) -> Calculator {
        let mut calc = Calculator {
            precedence: self.precedence,
            integer: tape.integer,
            digits: self.digits,
            angle: tape.angle,
            notation: tape.notation,
            memory: self.memory.clone(),
            ..Calculator::default()
        };
        calc.restart_tape();
        calc.replaying = true;
        for item in items {
            calc.replay_item(item);
        }
        calc.replaying = false;
        calc
    }

    fn replay_item(&mut self, item: &Item) {
        match item {
            Item::Operand(operand) => {
                for &key in &operand.keys {
                    self.execute(key);
                }
                operand.finish(self);
                // The tape gets the operand as it was, with how it ended.
                if let Some(at) = self.tape.open_operand()
                    && let Some(slot) = self.tape.items.get_mut(at)
                {
                    *slot = Item::Operand(operand.clone());
                }
            }
            Item::Command(command) => self.execute(*command),
            Item::Memory(command) => {
                if self.error.is_none() {
                    self.record_item(item.clone());
                    self.process(*command);
                }
            }
            Item::Recall(value) => {
                if self.error.is_none() {
                    self.record_item(item.clone());
                    self.recall_value(value.clone());
                }
            }
            Item::Random(value) => {
                self.next_random = Some(value.clone());
                self.execute(Command::Random);
            }
            Item::Raw(command) => {
                self.record_item(item.clone());
                self.process(*command);
            }
            Item::PasteError => {
                self.record_item(item.clone());
                self.display_error(CalcError::InvalidInput);
            }
            Item::HistoryRecall(result, expression) => {
                self.recall_entry(result.clone(), expression.clone());
            }
            Item::Recalled { angle, notation } => self.settle_recall(*angle, *notation),
        }
    }

    /// A plain recall of a history entry with a tape: the tape is replayed, so the expression
    /// line's numbers can be edited as after typing the calculation, and the engine is then
    /// left as `recall_entry` leaves it (a `=` doesn't repeat the entry's last operation, the
    /// angle unit and F-E stay as they are).
    pub(super) fn recall_tape(&mut self, tape: &Tape) {
        // What the recall's `C` logs (a pending `√(5)`), as `recall_entry` does.
        self.process(Command::Clear);
        let (angle, notation) = (self.angle, self.notation);
        let calc = self.replayed(tape, &tape.items);
        self.adopt(calc);
        let replaying = std::mem::replace(&mut self.replaying, true);
        self.settle_recall(angle, notation);
        self.replaying = replaying;
    }

    /// `Item::Recalled`: the steps of `recall_entry` after the entry's tape, with the result
    /// it gave and the line it shows (and its editable numbers).
    fn settle_recall(&mut self, angle: Angle, notation: bool) {
        self.record_item(Item::Recalled { angle, notation });
        self.angle = angle;
        self.notation = notation;
        if self.error.is_some() {
            return;
        }
        let result = self.current.clone();
        let shown = std::mem::take(&mut self.shown);
        let parts = std::mem::take(&mut self.shown_parts);
        self.process(Command::Clear);
        self.current = result.clone();
        self.current_source = None;
        self.last = result;
        self.record = false;
        self.shown = shown;
        self.shown_parts = parts;
        // The next key clears the expression line, as after `=`.
        self.no_prev_equ = false;
        self.this_key = Key::Equals;
    }

    /// Becomes `calc`, keeping this engine's memory and history.
    fn adopt(&mut self, mut calc: Calculator) {
        calc.memory = std::mem::take(&mut self.memory);
        calc.history = std::mem::take(&mut self.history);
        *self = calc;
    }

    // -------------------------------------------------------- the expression line

    /// The expression line and its typed numbers that can be edited; while one is edited,
    /// with the edited number.
    pub fn expression_parts(&self) -> (String, Vec<ExpressionPart>) {
        if let Some(Edit::Line(edit)) = self.edit.as_deref() {
            let (text, parts) = &edit.template;
            let Some(part) = parts.iter().find(|part| part.item == edit.item) else {
                return edit.template.clone();
            };
            let typed = edit.input.text();
            let mut line = text.clone();
            line.replace_range(part.range.clone(), &typed);
            let shift = typed.len() as isize - part.range.len() as isize;
            let moved = |at: usize| {
                if at >= part.range.end {
                    (at as isize + shift) as usize
                } else {
                    at
                }
            };
            let parts = parts
                .iter()
                .map(|other| ExpressionPart {
                    range: if other.item == edit.item {
                        part.range.start..part.range.start + typed.len()
                    } else {
                        moved(other.range.start)..moved(other.range.end)
                    },
                    item: other.item,
                })
                .collect();
            return (line, parts);
        }
        self.line_parts()
    }

    /// The expression line shown and its typed numbers that can be edited.
    fn line_parts(&self) -> (String, Vec<ExpressionPart>) {
        let parts = if self.tape.full {
            Vec::new()
        } else {
            let radix = self.radix();
            self.shown_parts
                .iter()
                .filter(|part| {
                    self.tape
                        .operand(part.item)
                        .is_some_and(|operand| operand.integer.map(|mode| mode.radix) == radix)
                })
                .cloned()
                .collect()
        };
        (self.shown.clone(), parts)
    }

    /// The number of the expression line being edited.
    pub fn number_edit(&self) -> Option<NumberEdit> {
        let Some(Edit::Line(edit)) = self.edit.as_deref() else {
            return None;
        };
        Some(NumberEdit {
            item: edit.item,
            text: edit.input.text(),
            mantissa: edit.input.digits.clone(),
            exponent: edit
                .input
                .exponent
                .as_ref()
                .map(|exponent| exponent.digits.clone()),
            caret: edit.input.caret().unwrap_or_else(|| edit.input.end_caret()),
        })
    }

    /// Starts editing the typed number of tape item `item` in the expression line, with the
    /// caret at `caret` (at its end without one). Returns whether it can be edited.
    pub fn edit_operand(&mut self, item: usize, caret: Option<Caret>) -> bool {
        let editable = self
            .expression_parts()
            .1
            .iter()
            .any(|part| part.item == item);
        if !editable {
            return false;
        }
        let Some(operand) = self.tape.operand(item).cloned() else {
            return false;
        };
        let mut input = operand.input.clone();
        input.show_caret(caret.unwrap_or_else(|| input.end_caret()));
        // Another number of the same line: the edit goes on there.
        if let Some(Edit::Line(edit)) = self.edit.as_deref_mut() {
            if edit.item != item {
                edit.item = item;
                edit.integer = operand.integer;
                edit.last_key = operand.last_key;
                edit.hidden = operand.hidden;
                edit.input = input;
            } else {
                edit.input
                    .show_caret(caret.unwrap_or_else(|| edit.input.end_caret()));
            }
            return true;
        }
        self.edit = None;
        self.input.caret = None;
        let evaluated = !self.no_prev_equ && self.this_key == Key::Equals;
        let template = self.expression_parts();
        let original = Box::new(self.clone());
        let preview = self.preview(evaluated);
        self.edit = Some(Box::new(Edit::Line(LineEdit {
            item,
            input,
            integer: operand.integer,
            last_key: operand.last_key,
            hidden: operand.hidden,
            original,
            evaluated,
            changed: false,
            template,
            preview,
        })));
        true
    }

    /// What the display shows while a number of the line is edited.
    pub(super) fn edit_preview(&self) -> Option<&Result<String, CalcError>> {
        match self.edit.as_deref() {
            Some(Edit::Line(edit)) => Some(&edit.preview),
            _ => None,
        }
    }

    /// The display while the line is edited: the result after `=`, otherwise what `=` would
    /// give, worked out on a copy that adds no history.
    fn preview(&self, evaluated: bool) -> Result<String, CalcError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if evaluated {
            return Ok(self.display_without_edit());
        }
        let mut calc = self.clone();
        calc.edit = None;
        calc.replaying = true;
        calc.execute(Command::Equals);
        match calc.error {
            Some(error) => Err(error),
            None => Ok(calc.display_without_edit()),
        }
    }

    fn display_without_edit(&self) -> String {
        let mut calc = self.clone();
        calc.edit = None;
        calc.display()
    }

    /// A key while something is edited. Returns whether the edit took it; otherwise the edit
    /// is over and the key does what it does.
    pub(super) fn edit_key(&mut self, command: Command) -> bool {
        let Some(edit) = self.edit.as_deref().cloned() else {
            return false;
        };
        match edit {
            Edit::Display {
                start,
                mut input,
                integer,
            } => {
                if !is_typing_key(command) {
                    self.edit = None;
                    return false;
                }
                if !self.type_into(&mut input, integer.as_ref(), command) {
                    self.edit = Some(Box::new(Edit::Display {
                        start,
                        input,
                        integer,
                    }));
                    return true;
                }
                self.type_copy(start, input);
                true
            }
            Edit::Line(mut edit) => {
                if is_typing_key(command) {
                    if self.type_into(&mut edit.input, edit.integer.as_ref(), command) {
                        self.replay_edit(edit);
                    } else {
                        self.edit = Some(Box::new(Edit::Line(edit)));
                    }
                    return true;
                }
                // `=` after `=` would repeat the operation: it only commits the edit.
                let evaluated = edit.evaluated;
                self.commit_edit();
                command == Command::Equals && evaluated
            }
        }
    }

    /// Types `command` into an edited number. Returns whether the number changed.
    fn type_into(
        &self,
        input: &mut Input,
        integer: Option<&IntegerMode>,
        command: Command,
    ) -> bool {
        let before = input.text();
        let max = self.digits as usize;
        match command {
            Command::Digit(digit) => {
                let base = integer.map_or(10, |mode| mode.radix.base());
                if u32::from(digit) < base {
                    input.insert_digit(digit, integer, max);
                }
            }
            Command::Decimal if integer.is_none() => {
                input.insert_point(integer, max);
            }
            Command::Backspace => input.backspace_at_caret(integer, max),
            Command::Function(Func::Negate) => {
                input.toggle_sign(integer);
            }
            Command::Exponent if integer.is_none() && input.try_begin_exponent() => {
                input.show_caret(input.end_caret());
            }
            _ => {}
        }
        input.text() != before
    }

    /// Replays the tape with the edited number and goes on editing it.
    fn replay_edit(&mut self, mut edit: LineEdit) {
        let mut items = self.tape.items.clone();
        let mut typed = edit.input.clone();
        typed.caret = None;
        if let Some(slot) = items.get_mut(edit.item) {
            *slot = Item::Operand(Operand {
                keys: typed.keys(),
                input: typed,
                integer: edit.integer,
                last_key: edit.last_key,
                hidden: edit.hidden,
                regenerated: true,
            });
        }
        let calc = self.replayed(&self.tape, &items);
        self.adopt(calc);
        // The line as the replay shows it (another number edited before is in it), unless an
        // error stopped it before the edited number.
        let line = self.line_parts();
        if line.1.iter().any(|part| part.item == edit.item) {
            edit.template = line;
        }
        edit.changed = true;
        edit.preview = self.preview(edit.evaluated);
        self.edit = Some(Box::new(Edit::Line(edit)));
    }

    /// The edited copy of the display's value replaces the tape from `start` on, as if typed
    /// there; the caret stays in it, now the number being typed.
    fn type_copy(&mut self, start: usize, input: Input) {
        let mut calc = self.replayed(&self.tape, &self.tape.items[..start]);
        let mut typed = input.clone();
        typed.caret = None;
        calc.replaying = true;
        for key in typed.keys() {
            calc.execute(key);
        }
        calc.replaying = false;
        if let Some(caret) = input.caret {
            calc.input.show_caret(caret);
        }
        calc.sync_operand();
        self.adopt(calc);
    }

    /// Ends an edit and keeps it. An edited calculation that had been worked out (its line
    /// ended with `=`) is a new one: it goes into the history, and the old entry stays.
    pub fn commit_edit(&mut self) {
        let Some(edit) = self.edit.take() else {
            return;
        };
        let Edit::Line(edit) = *edit else {
            return;
        };
        if edit.evaluated && edit.changed && self.integer.is_none() && self.error.is_none() {
            self.history.insert(
                0,
                HistoryEntry {
                    expression: self.shown.clone(),
                    result: self.current.clone(),
                    text: self.format_value(&self.current),
                    tape: (!self.tape.full).then(|| Box::new(self.tape.clone())),
                },
            );
            self.history.truncate(HISTORY_LIMIT);
        }
    }

    /// Esc: ends an edit of the line and puts everything back as it was. Returns whether there
    /// was one.
    pub fn cancel_edit(&mut self) -> bool {
        match self.edit.take().map(|edit| *edit) {
            Some(Edit::Line(edit)) => {
                let original = *edit.original;
                self.adopt(original);
                true
            }
            Some(Edit::Display { .. }) => true,
            None => false,
        }
    }

    // -------------------------------------------------------------- the display

    /// The caret in the display's copy of a computed value.
    pub(super) fn display_copy_caret(&self) -> Option<(Caret, &Input)> {
        match self.edit.as_deref() {
            Some(Edit::Display { input, .. }) => {
                Some((input.caret().unwrap_or_else(|| input.end_caret()), input))
            }
            _ => None,
        }
    }

    /// Moves the caret of an edited number: the line's, or the display's copy.
    pub(super) fn edit_caret(&mut self, change: impl FnOnce(&mut Input)) -> bool {
        match self.edit.as_deref_mut() {
            Some(Edit::Line(edit)) => change(&mut edit.input),
            Some(Edit::Display { input, .. }) => change(input),
            None => return false,
        }
        true
    }

    /// Delete in an edited number of the line or the display's copy.
    pub(super) fn edit_delete(&mut self) -> bool {
        let max = self.digits as usize;
        match self.edit.as_deref().cloned() {
            Some(Edit::Line(mut edit)) => {
                let before = edit.input.text();
                edit.input.delete_at_caret(edit.integer.as_ref(), max);
                if edit.input.text() != before {
                    self.replay_edit(edit);
                } else {
                    self.edit = Some(Box::new(Edit::Line(edit)));
                }
                true
            }
            Some(Edit::Display {
                start,
                mut input,
                integer,
            }) => {
                let before = input.text();
                input.delete_at_caret(integer.as_ref(), max);
                if input.text() != before {
                    self.type_copy(start, input);
                } else {
                    self.edit = Some(Box::new(Edit::Display {
                        start,
                        input,
                        integer,
                    }));
                }
                true
            }
            None => false,
        }
    }

    /// Shows a caret in a copy of the computed value the display shows, which typing would
    /// replace: the last operand after `)`, a function, MR, π or `rand`, or a new number
    /// after `=` or an operator. Returns whether the value can be edited.
    pub(super) fn edit_display_copy(&mut self, caret: Caret) -> bool {
        if let Some(Edit::Display { input, .. }) = self.edit.as_deref_mut() {
            input.show_caret(caret);
            return true;
        }
        let Some(start) = self.display_copy_start() else {
            return false;
        };
        let Some(mut input) =
            Input::from_display(&self.display(), self.integer.as_ref(), self.digits as usize)
        else {
            return false;
        };
        input.show_caret(caret);
        self.edit = Some(Box::new(Edit::Display {
            start,
            input,
            integer: self.integer,
        }));
        true
    }

    /// Where the tape of the value the display shows starts, if typing a number from there
    /// gives the same calculation with that number in its place.
    fn display_copy_start(&self) -> Option<usize> {
        if self.tape.full || self.error.is_some() || self.record {
            return None;
        }
        let items = self.tape.items.len();
        let tokens = &self.line.tokens;
        let start = match self.this_key {
            Key::CloseParen | Key::Unary | Key::Sign | Key::Degrees => {
                let first = self.line.last_operand_start?;
                let start = tokens
                    .get(first..)?
                    .iter()
                    .map(|token| token.source.unwrap_or(token.created).min(token.created))
                    .min()?;
                // Nothing before the operand may come from the steps it replaces.
                if tokens[..first].iter().any(|token| token.created >= start) {
                    return None;
                }
                start
            }
            Key::Constant | Key::Recall | Key::BitToggle => items.checked_sub(1)?,
            _ => items,
        };
        // A number typed right after the steps before it would go on with a number they
        // leave open, or multiply a `)`.
        let prefix = self.replayed(&self.tape, &self.tape.items[..start]);
        let open = prefix.record && !prefix.input.is_empty();
        (!open && prefix.this_key != Key::CloseParen && prefix.error.is_none()).then_some(start)
    }

    // ---------------------------------------------------------------- history

    /// Opens a history entry's calculation again (CalcCraft's own "Edit"): its tape is
    /// replayed, so the expression line shows it with its typed numbers editable, and the
    /// caret goes into the last one. Returns whether the entry could be opened.
    pub fn edit_history(&mut self, index: usize) -> bool {
        self.commit_edit();
        let Some(tape) = self.history.get(index).and_then(|entry| entry.tape.clone()) else {
            return false;
        };
        let calc = self.replayed(&tape, &tape.items);
        self.adopt(calc);
        if let Some(part) = self.expression_parts().1.last() {
            self.edit_operand(part.item, None);
        }
        true
    }

    /// Whether `edit_history` can open a history entry.
    pub fn can_edit_history(&self, index: usize) -> bool {
        self.history
            .get(index)
            .is_some_and(|entry| entry.tape.is_some())
    }

    /// Ctrl+Left and Ctrl+Right at the start or end of an edited number: the caret goes to
    /// the previous or next number of the line. Returns whether there was one.
    pub fn edit_neighbor(&mut self, previous: bool) -> bool {
        let Some(edit) = self.number_edit() else {
            return false;
        };
        let parts = self.expression_parts().1;
        let Some(at) = parts.iter().position(|part| part.item == edit.item) else {
            return false;
        };
        let next = if previous {
            at.checked_sub(1)
        } else {
            Some(at + 1)
        };
        let Some(part) = next.and_then(|next| parts.get(next)) else {
            return false;
        };
        let caret = (!previous).then_some(Caret::default());
        self.edit_operand(part.item, caret)
    }
}

/// The keys that type into a number being edited.
fn is_typing_key(command: Command) -> bool {
    matches!(
        command,
        Command::Digit(_)
            | Command::Decimal
            | Command::Backspace
            | Command::Exponent
            | Command::Function(Func::Negate)
    )
}

#[cfg(test)]
mod tests {
    use super::super::{CaretMove, Op, Trig, TrigFunction, WordSize};
    use super::*;

    /// Presses keys written as in Windows' engine strings, separated by spaces: numbers
    /// (digits, `.`, `~` for ±, `<` for ⌫), `+ - * / ^ mod % ( ) =`, `sqr`, `sqrt`, `inv`,
    /// `neg`, `sin`, `C`, `CE`, `MS`, `M+`, `MR`, `hex`, `dec`.
    fn press(calc: &mut Calculator, keys: &str) {
        for word in keys.split_whitespace() {
            for command in commands(word) {
                calc.execute(command);
            }
        }
    }

    fn commands(word: &str) -> Vec<Command> {
        let single = match word {
            "+" => Command::Operator(Op::Add),
            "-" => Command::Operator(Op::Subtract),
            "*" => Command::Operator(Op::Multiply),
            "/" => Command::Operator(Op::Divide),
            "^" => Command::Operator(Op::Power),
            "mod" => Command::Operator(Op::Mod),
            "and" => Command::Operator(Op::And),
            "%" => Command::Percent,
            "(" => Command::OpenParen,
            ")" => Command::CloseParen,
            "=" => Command::Equals,
            "sqr" => Command::Function(Func::Square),
            "sqrt" => Command::Function(Func::SquareRoot),
            "inv" => Command::Function(Func::Reciprocal),
            "neg" => Command::Function(Func::Negate),
            "sin" => Command::Function(Func::Trig(Trig {
                function: TrigFunction::Sin,
                inverse: false,
                hyperbolic: false,
            })),
            "C" => Command::Clear,
            "CE" => Command::ClearEntry,
            "MS" => Command::MemoryStore,
            "M+" => Command::MemoryAdd,
            "MR" => Command::MemoryRecall,
            "pi" => Command::Pi,
            "hex" => Command::Radix(Radix::Hex),
            "dec" => Command::Radix(Radix::Decimal),
            "byte" => Command::WordSize(WordSize::Byte),
            _ => {
                return word
                    .chars()
                    .map(|c| match c {
                        '.' => Command::Decimal,
                        '~' => Command::Function(Func::Negate),
                        '<' => Command::Backspace,
                        'e' => Command::Exponent,
                        c => Command::Digit(c.to_digit(16).expect("a key") as u8),
                    })
                    .collect();
            }
        };
        vec![single]
    }

    /// The tape item of the `n`th editable number of the line.
    fn part(calc: &Calculator, n: usize) -> usize {
        calc.expression_parts().1[n].item
    }

    /// Replaces the line's `n`th number by `text` with the keys: the caret at its end, ⌫ for
    /// every character, then the new characters.
    fn retype_part(calc: &mut Calculator, n: usize, text: &str) {
        assert!(calc.edit_operand(part(calc, n), None));
        let old = calc.number_edit().unwrap().mantissa.len();
        for _ in 0..old {
            calc.execute(Command::Backspace);
        }
        press(calc, text);
    }

    #[test]
    fn konsts_example() {
        let mut calc = Calculator::with_precedence();
        press(&mut calc, "2 + ( 5 * 7 ) + ( 100 - 92 ) + ( 6 * 54 )");
        assert_eq!(calc.display(), "324");
        assert_eq!(calc.expression(), "2 + (5 × 7) + (100 - 92) + (6 × 54)");
        let numbers: Vec<_> = {
            let (text, parts) = calc.expression_parts();
            parts
                .iter()
                .map(|p| text[p.range.clone()].to_string())
                .collect()
        };
        assert_eq!(numbers, ["2", "5", "7", "100", "92", "6", "54"]);
        // 92 → 85: the display shows what `=` gives, live.
        retype_part(&mut calc, 4, "85");
        assert_eq!(calc.expression(), "2 + (5 × 7) + (100 - 85) + (6 × 54)");
        assert_eq!(calc.display(), "376");
        let edit = calc.number_edit().unwrap();
        assert_eq!(edit.text, "85");
        // `=` commits and works it out, as typing it fresh would.
        calc.execute(Command::Equals);
        assert_eq!(calc.number_edit(), None);
        assert_eq!(calc.display(), "376");
        assert_eq!(calc.expression(), "2 + (5 × 7) + (100 - 85) + (6 × 54) =");
        assert_eq!(calc.history().len(), 1);
    }

    #[test]
    fn a_finished_calculation_adds_a_new_history_entry() {
        let mut calc = Calculator::with_precedence();
        press(&mut calc, "2 + 3 * 4 =");
        assert_eq!(calc.display(), "14");
        assert_eq!(calc.history().len(), 1);
        retype_part(&mut calc, 1, "5");
        assert_eq!(calc.display(), "22");
        assert_eq!(calc.expression(), "2 + 5 × 4 =");
        // Replays add nothing.
        assert_eq!(calc.history().len(), 1);
        // `=` commits without repeating the last operation.
        calc.execute(Command::Equals);
        assert_eq!(calc.display(), "22");
        assert_eq!(calc.history().len(), 2);
        assert_eq!(calc.history()[0].expression, "2 + 5 × 4 =");
        assert_eq!(calc.history()[1].expression, "2 + 3 × 4 =");
        // Another key commits too, then does what it does.
        retype_part(&mut calc, 2, "1");
        assert_eq!(calc.display(), "7");
        calc.execute(Command::Operator(Op::Multiply));
        assert_eq!(calc.history().len(), 3);
        assert_eq!(calc.expression(), "7 ×");
        // An edit that changes nothing adds nothing.
        let mut calc = Calculator::default();
        press(&mut calc, "2 + 3 =");
        calc.edit_operand(part(&calc, 0), None);
        calc.commit_edit();
        assert_eq!(calc.history().len(), 1);
    }

    #[test]
    fn esc_cancels() {
        let mut calc = Calculator::default();
        press(&mut calc, "12 + 30 =");
        retype_part(&mut calc, 0, "7");
        assert_eq!(calc.display(), "37");
        assert!(calc.hide_entry_caret());
        assert_eq!(calc.display(), "42");
        assert_eq!(calc.expression(), "12 + 30 =");
        assert_eq!(calc.history().len(), 1);
        // `=` still repeats the original operation.
        calc.execute(Command::Equals);
        assert_eq!(calc.display(), "72");
    }

    #[test]
    fn the_caret_moves_inside_a_number_of_the_line() {
        let mut calc = Calculator::default();
        press(&mut calc, "1234 + 1 =");
        calc.edit_operand(
            part(&calc, 0),
            Some(Caret {
                exponent: false,
                index: 1,
            }),
        );
        press(&mut calc, "9");
        assert_eq!(calc.expression(), "19234 + 1 =");
        calc.move_entry_caret(CaretMove::End);
        press(&mut calc, "<");
        calc.delete_after_caret();
        assert_eq!(calc.expression(), "1923 + 1 =");
        calc.move_entry_caret(CaretMove::Start);
        calc.delete_after_caret();
        assert_eq!(calc.expression(), "923 + 1 =");
        assert_eq!(calc.display(), "924");
        // Ctrl+Right goes on to the start of the next number.
        assert!(calc.edit_neighbor(false));
        assert_eq!(calc.number_edit().unwrap().text, "1");
        press(&mut calc, "7");
        assert_eq!(calc.expression(), "923 + 71 =");
        assert_eq!(calc.display(), "994");
        assert!(!calc.edit_neighbor(false));
        assert!(calc.edit_neighbor(true));
        assert_eq!(calc.number_edit().unwrap().text, "923");
    }

    #[test]
    fn an_unfinished_calculation_goes_on() {
        let mut calc = Calculator::with_precedence();
        press(&mut calc, "2 + 3 * 4");
        // The 4 is typed; the line has 2 and 3.
        assert_eq!(calc.expression(), "2 + 3 ×");
        retype_part(&mut calc, 1, "5");
        // The display previews what `=` gives.
        assert_eq!(calc.display(), "22");
        calc.commit_edit();
        assert_eq!(calc.display(), "4");
        assert_eq!(calc.history().len(), 0);
        press(&mut calc, "0 =");
        assert_eq!(calc.display(), "202");
        assert_eq!(calc.expression(), "2 + 5 × 40 =");
    }

    #[test]
    fn replays_keep_the_memory() {
        let mut calc = Calculator::default();
        press(&mut calc, "5 MS + 10 M+ =");
        assert_eq!(calc.memory(), &[Number::from_integer(15.into())]);
        retype_part(&mut calc, 0, "6");
        calc.commit_edit();
        // MS and M+ aren't done again.
        assert_eq!(calc.memory(), &[Number::from_integer(15.into())]);
        assert_eq!(calc.display(), "16");
        // MR gives the value it gave.
        let mut calc = Calculator::default();
        press(&mut calc, "7 MS C 1 + MR =");
        assert_eq!(calc.display(), "8");
        press(&mut calc, "9 MS");
        let mut fresh = calc.clone();
        fresh.memory = vec![Number::from_integer(7.into())];
        calc.recall_history(0);
        assert_eq!(calc.display(), "8");
    }

    #[test]
    fn a_computed_value_on_the_display() {
        // Konst's 324: the copy replaces the group `(6 × 54)`.
        let mut calc = Calculator::with_precedence();
        press(&mut calc, "2 + ( 5 * 7 ) + ( 100 - 92 ) + ( 6 * 54 )");
        assert!(calc.show_entry_caret(Caret {
            exponent: false,
            index: 1,
        }));
        assert_eq!(calc.entry_characters().0, "324");
        press(&mut calc, "< 5");
        assert_eq!(calc.display(), "524");
        assert_eq!(calc.expression(), "2 + (5 × 7) + (100 - 92) +");
        assert_eq!(
            calc.entry_caret(),
            Some(Caret {
                exponent: false,
                index: 1
            })
        );
        press(&mut calc, "=");
        assert_eq!(calc.display(), "569");
        // A function's result: sqr(5) becomes the typed 30.
        let mut calc = Calculator::default();
        press(&mut calc, "2 + 5 sqr");
        assert_eq!(calc.display(), "25");
        calc.show_entry_caret(Caret {
            exponent: false,
            index: 2,
        });
        press(&mut calc, "<< 30 =");
        assert_eq!(calc.display(), "32");
        // 2(3): the group can't be replaced, the 2 would take the digits.
        let mut calc = Calculator::with_precedence();
        press(&mut calc, "2 ( 3 )");
        assert!(!calc.show_entry_caret(Caret::default()));
        // Esc before a key changes nothing.
        let mut calc = Calculator::with_precedence();
        press(&mut calc, "( 6 * 54 )");
        assert!(calc.show_entry_caret(Caret::default()));
        assert!(calc.hide_entry_caret());
        assert_eq!(calc.entry_caret(), None);
        assert_eq!(calc.expression(), "(6 × 54)");
    }

    #[test]
    fn history_entries_open_for_editing() {
        let mut calc = Calculator::with_precedence();
        press(&mut calc, "2 + 3 * 4 = 5 sqrt");
        press(&mut calc, "1 + 1 =");
        // The entry `2 + 3 × 4 =` is the third: √(5) was logged by the next number.
        let index = calc
            .history()
            .iter()
            .position(|entry| entry.expression == "2 + 3 × 4 =")
            .unwrap();
        assert!(calc.can_edit_history(index));
        assert!(calc.edit_history(index));
        assert_eq!(calc.expression(), "2 + 3 × 4 =");
        // The caret is in the last number.
        assert_eq!(calc.number_edit().unwrap().text, "4");
        press(&mut calc, "< 6");
        assert_eq!(calc.display(), "20");
        let before = calc.history().len();
        calc.execute(Command::Equals);
        assert_eq!(calc.history().len(), before + 1);
        assert_eq!(calc.history()[0].expression, "2 + 3 × 6 =");
        // A Standard mode step logged by an operator opens as if `=` had been pressed.
        let mut calc = Calculator::default();
        press(&mut calc, "2 + 3 + 4 =");
        assert_eq!(calc.history()[1].expression, "2 + 3 =");
        assert!(calc.edit_history(1));
        assert_eq!(calc.expression(), "2 + 3 =");
        assert_eq!(calc.display(), "5");
    }

    /// The numbers of the line, as the UI offers them for the caret.
    fn line_numbers(calc: &Calculator) -> Vec<String> {
        let (text, parts) = calc.expression_parts();
        parts
            .iter()
            .map(|p| text[p.range.clone()].to_string())
            .collect()
    }

    #[test]
    fn a_recalled_entry_edits_like_a_fresh_calculation() {
        // Konst's bug: after a plain tap on an earlier entry only the result took the caret.
        for mut calc in [Calculator::default(), Calculator::with_precedence()] {
            press(&mut calc, "12 + 7 = C 3 * 4 = C 100 - 1 =");
            assert_eq!(calc.history().len(), 3);
            let before: Vec<_> = calc
                .history()
                .iter()
                .map(|e| (e.expression.clone(), e.text.clone()))
                .collect();
            // Entry 2 of 3.
            calc.recall_history(1);
            assert_eq!(calc.expression(), "3 × 4 =");
            assert_eq!(calc.display(), "12");
            assert_eq!(line_numbers(&calc), ["3", "4"]);
            retype_part(&mut calc, 0, "25");
            let mut fresh = if calc.precedence {
                Calculator::with_precedence()
            } else {
                Calculator::default()
            };
            press(&mut fresh, "25 * 4 =");
            assert_eq!(calc.display(), fresh.display());
            assert_eq!(calc.display(), "100");
            assert_eq!(calc.expression(), fresh.expression());
            calc.execute(Command::Equals);
            // A new entry; the old ones stay as they were.
            assert_eq!(calc.history().len(), 4);
            assert_eq!(calc.history()[0].expression, "25 × 4 =");
            assert_eq!(calc.history()[0].text, "100");
            let after: Vec<_> = calc.history()[1..]
                .iter()
                .map(|e| (e.expression.clone(), e.text.clone()))
                .collect();
            assert_eq!(after, before);
            // The new entry can be recalled and edited again.
            calc.recall_history(0);
            assert_eq!(line_numbers(&calc), ["25", "4"]);
        }
    }

    #[test]
    fn a_recall_keeps_its_windows_behavior() {
        // `=` after a recall doesn't repeat the entry's operation, and a new number starts
        // over, as before the tape came back with it.
        let mut calc = Calculator::default();
        press(&mut calc, "2 + 3 = C 9 =");
        calc.recall_history(1);
        assert_eq!(calc.display(), "5");
        press(&mut calc, "=");
        assert_eq!(calc.display(), "5");
        let mut calc = Calculator::default();
        press(&mut calc, "2 + 3 = C 9 =");
        calc.recall_history(1);
        press(&mut calc, "+ 1 =");
        assert_eq!(calc.display(), "6");
        assert_eq!(calc.expression(), "5 + 1 =");
        // An edit after more keys replays the recall too.
        let mut calc = Calculator::default();
        press(&mut calc, "2 + 3 = C 9 =");
        calc.recall_history(1);
        press(&mut calc, "* 2 =");
        assert_eq!(calc.display(), "10");
        retype_part(&mut calc, 0, "4");
        assert_eq!(calc.display(), "20");
        // The angle unit of the moment of the recall stays.
        let mut calc = Calculator::with_precedence();
        press(&mut calc, "30 sin =");
        calc.execute(Command::Angle(Angle::Radians));
        calc.recall_history(0);
        assert_eq!(calc.display(), "0.5");
        assert_eq!(calc.angle, Angle::Radians);
        retype_part(&mut calc, 0, "90");
        assert_eq!(calc.display(), "1");
        assert_eq!(calc.angle, Angle::Radians);
    }

    #[test]
    fn programmer_numbers_edit_in_their_radix() {
        let mut calc = Calculator::programmer();
        press(&mut calc, "hex FF and 3C =");
        assert_eq!(calc.display(), "3C");
        retype_part(&mut calc, 0, "F0");
        assert_eq!(calc.display(), "30");
        calc.commit_edit();
        // Typed in HEX, they can't be edited in DEC.
        press(&mut calc, "dec");
        let mut calc = Calculator::programmer();
        press(&mut calc, "hex FF and dec 7 +");
        let (text, parts) = calc.expression_parts();
        let numbers: Vec<_> = parts.iter().map(|p| &text[p.range.clone()]).collect();
        assert_eq!(numbers, ["7"]);
    }

    // ------------------------------------------------------------ property test

    /// A small random number generator (xorshift), so the test needs no crate.
    struct Random(u64);

    impl Random {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    #[derive(Clone, Copy, PartialEq)]
    enum Mode {
        Standard,
        Scientific,
        Programmer,
    }

    fn engine(mode: Mode) -> Calculator {
        match mode {
            Mode::Standard => Calculator::default(),
            Mode::Scientific => Calculator::with_precedence(),
            Mode::Programmer => Calculator::programmer(),
        }
    }

    /// A random step: a number typed (with ±, ⌫ and a point sometimes) or another key.
    fn random_step(random: &mut Random, mode: Mode) -> Vec<Command> {
        let base = if mode == Mode::Programmer { 16 } else { 10 };
        if random.below(5) < 2 {
            let mut keys = Vec::new();
            for _ in 0..=random.below(4) {
                keys.push(Command::Digit(random.below(base) as u8));
            }
            match random.below(8) {
                0 if mode != Mode::Programmer => keys.insert(1, Command::Decimal),
                1 => keys.push(Command::Function(Func::Negate)),
                2 => keys.insert(1, Command::Function(Func::Negate)),
                3 => keys.push(Command::Backspace),
                4 if mode == Mode::Scientific => {
                    keys.extend([Command::Exponent, Command::Digit(2)]);
                }
                _ => {}
            }
            return keys;
        }
        let mut pool = vec![
            Command::Operator(Op::Add),
            Command::Operator(Op::Subtract),
            Command::Operator(Op::Multiply),
            Command::Operator(Op::Divide),
            Command::Equals,
            Command::Equals,
            Command::Function(Func::Negate),
            Command::ClearEntry,
            Command::Clear,
            Command::Backspace,
            Command::MemoryStore,
            Command::MemoryAdd,
            Command::MemoryRecall,
        ];
        match mode {
            Mode::Standard => pool.extend([
                Command::Percent,
                Command::Function(Func::Square),
                Command::Function(Func::SquareRoot),
                Command::Function(Func::Reciprocal),
            ]),
            Mode::Scientific => pool.extend([
                Command::Percent,
                Command::OpenParen,
                Command::OpenParen,
                Command::CloseParen,
                Command::CloseParen,
                Command::Operator(Op::Power),
                Command::Operator(Op::Mod),
                Command::Function(Func::Square),
                Command::Function(Func::Factorial),
                Command::Function(Func::Abs),
                Command::Function(Func::Trig(Trig {
                    function: TrigFunction::Sin,
                    inverse: false,
                    hyperbolic: false,
                })),
                Command::Pi,
                Command::Angle(Angle::Radians),
                Command::Angle(Angle::Degrees),
                Command::ToggleNotation,
            ]),
            Mode::Programmer => pool.extend([
                Command::OpenParen,
                Command::CloseParen,
                Command::Operator(Op::And),
                Command::Operator(Op::Or),
                Command::Operator(Op::LeftShift),
                Command::Operator(Op::Mod),
                Command::Function(Func::Not),
                Command::Radix(Radix::Hex),
                Command::Radix(Radix::Decimal),
                Command::Radix(Radix::Binary),
                Command::WordSize(WordSize::Byte),
                Command::WordSize(WordSize::Qword),
                Command::ToggleBit(3),
            ]),
        }
        vec![pool[random.below(pool.len())]]
    }

    /// A random number that typing can give, in `operand`'s integer mode.
    fn random_input(random: &mut Random, operand: &Operand, digits: i64) -> Option<Input> {
        let base = operand.integer.map_or(10, |mode| mode.radix.base()) as usize;
        let mut text = String::new();
        for _ in 0..=random.below(5) {
            text.push(char::from_digit(random.below(base) as u32, base as u32)?);
        }
        if operand.integer.is_none() && random.below(3) == 0 {
            text.insert(1, '.');
        }
        let negative = random.below(4) == 0
            && operand
                .integer
                .is_none_or(|mode| mode.radix == Radix::Decimal);
        let exponent = (operand.integer.is_none() && random.below(6) == 0).then_some((false, "3"));
        Input::retype(
            negative,
            &text.to_uppercase(),
            exponent,
            operand.integer.as_ref(),
            digits as usize,
        )
    }

    /// Types a tape's items again into `calc` with the public keys, with the number of item
    /// `edited` typed as `input`: its digits fresh, ending as the original number ended (its
    /// last key, the line hidden or not: `Operand::finish`).
    fn retype_tape(calc: &mut Calculator, tape: &Tape, edited: usize, input: &Input) {
        for (i, item) in tape.items.iter().enumerate() {
            match item {
                Item::Operand(operand) => {
                    let operand = if i == edited {
                        Operand {
                            keys: input.keys(),
                            input: input.clone(),
                            regenerated: true,
                            ..operand.clone()
                        }
                    } else {
                        operand.clone()
                    };
                    for &key in &operand.keys {
                        calc.execute(key);
                    }
                    operand.finish(calc);
                }
                Item::Command(command) | Item::Memory(command) => calc.execute(*command),
                Item::Recall(_) => calc.execute(Command::MemoryRecall),
                other => panic!("not generated: {other:?}"),
            }
        }
    }

    /// The state the comparison looks at.
    fn seen(calc: &Calculator) -> (String, String, Option<CalcError>, usize) {
        (
            calc.display(),
            calc.expression(),
            calc.error(),
            calc.open_parentheses(),
        )
    }

    /// Replaying a tape with a number edited gives exactly what typing the edited keys fresh
    /// gives: the display, the expression line, the error and the parentheses, and so do the
    /// next `=` and more keys after it. The memory and the history stay as they were (but for
    /// the one new entry of an edited finished calculation).
    #[test]
    fn replaying_an_edit_is_typing_it_fresh() {
        let mut random = Random(0x5eed_1234_abcd_0001);
        let mut checked = 0;
        for case in 0..2400 {
            let mode = [Mode::Standard, Mode::Scientific, Mode::Programmer][case % 3];
            let mut calc = engine(mode);
            let mut keys: Vec<Command> = Vec::new();
            for _ in 0..random.below(18) + 2 {
                for key in random_step(&mut random, mode) {
                    calc.execute(key);
                    keys.push(key);
                }
            }
            let parts = calc.expression_parts().1;
            if parts.is_empty() {
                continue;
            }
            let item = parts[random.below(parts.len())].item;
            let operand = calc.tape.operand(item).unwrap().clone();
            let Some(input) = random_input(&mut random, &operand, calc.digits) else {
                continue;
            };
            // MR after the edited number recalls what the original stored; typing fresh would
            // recall what the edited one stores. Those cases differ on purpose.
            let recalled_later = calc.tape.items[item..]
                .iter()
                .any(|step| matches!(step, Item::Recall(_)));
            if recalled_later {
                continue;
            }
            let tape = calc.tape.clone();
            let memory = calc.memory.clone();
            let history: Vec<String> = calc
                .history
                .iter()
                .map(|entry| entry.expression.clone())
                .collect();

            // The edit, through the replay.
            assert!(calc.edit_operand(item, None), "case {case}");
            let Some(Edit::Line(mut edit)) = calc.edit.as_deref().cloned() else {
                unreachable!()
            };
            let evaluated = edit.evaluated;
            let mut shown = input.clone();
            shown.show_caret(shown.end_caret());
            edit.input = shown;
            calc.replay_edit(edit);
            assert_eq!(
                calc.history.len(),
                history.len(),
                "case {case}: a replay logged"
            );
            calc.commit_edit();

            // The same keys typed fresh, after the steps before the last `C`.
            let mut fresh = engine(mode);
            let last_clear = keys.iter().rposition(|&key| key == Command::Clear);
            for &key in &keys[..last_clear.map_or(0, |at| at + 1)] {
                fresh.execute(key);
            }
            retype_tape(&mut fresh, &tape, item, &input);

            let context = format!(
                "case {case} ({:?}): {keys:?}, item {item} → {}",
                mode as u8,
                input.text()
            );
            assert_eq!(seen(&calc), seen(&fresh), "{context}");
            assert_eq!(calc.memory, memory, "{context}: the memory changed");
            let old: Vec<String> = calc
                .history
                .iter()
                .skip(calc.history.len() - history.len())
                .map(|entry| entry.expression.clone())
                .collect();
            assert_eq!(old, history, "{context}: the history changed");
            assert!(calc.history.len() <= history.len() + usize::from(evaluated));
            for key in [
                Command::Equals,
                Command::Operator(Op::Add),
                Command::Digit(3),
                Command::Equals,
                Command::Equals,
            ] {
                calc.execute(key);
                fresh.execute(key);
                assert_eq!(seen(&calc), seen(&fresh), "{context}, then {key:?}");
            }
            checked += 1;
        }
        // Most cases edit something.
        assert!(checked > 800, "only {checked} edits checked");
    }

    /// Whatever the caret did to the numbers being typed, the tape replays to the same state.
    #[test]
    fn caret_edits_replay_the_same() {
        let mut random = Random(0x0dd_ba11_cafe_f00d);
        for case in 0..1500 {
            let mode = [Mode::Standard, Mode::Scientific, Mode::Programmer][case % 3];
            let mut calc = engine(mode);
            let mut log = Vec::new();
            for _ in 0..random.below(12) + 2 {
                for key in random_step(&mut random, mode) {
                    calc.execute(key);
                    log.push(format!("{key:?}"));
                }
                if calc.can_show_entry_caret() && random.below(2) == 0 {
                    let (mantissa, _) = calc.entry_characters();
                    let index = random.below(mantissa.len() + 1);
                    calc.show_entry_caret(Caret {
                        exponent: false,
                        index,
                    });
                    log.push(format!("caret {index}"));
                    for _ in 0..random.below(4) {
                        match random.below(4) {
                            0 => {
                                calc.delete_after_caret();
                                log.push("Delete".into());
                            }
                            1 => {
                                calc.execute(Command::Backspace);
                                log.push("Backspace".into());
                            }
                            2 => {
                                calc.move_entry_caret(CaretMove::Left);
                                log.push("Left".into());
                            }
                            _ => {
                                let digit = Command::Digit(random.below(8) as u8 + 1);
                                calc.execute(digit);
                                log.push(format!("{digit:?}"));
                            }
                        }
                    }
                }
            }
            if calc.tape.full {
                continue;
            }
            let replayed = calc.replayed(&calc.tape, &calc.tape.items);
            let context = format!("case {case}: {}\n{:?}", log.join(" "), calc.tape.items);
            assert_eq!(seen(&replayed), seen(&calc), "{context}");
            assert_eq!(replayed.input.text(), calc.input.text(), "{context}");
        }
    }
}
