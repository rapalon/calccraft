//! A caret in the number being typed (CalcCraft's own; Windows has none, see
//! `docs/research/editable-expression.md`, option A).
//!
//! The caret is a position in the typed characters: the digits and the decimal point of the
//! number (`Input::digits`), and the exponent's digits as a second segment. Group separators,
//! the Programmer mode's spaces and BIN's padding aren't characters of the number, so the UI
//! maps the caret onto the text it shows (`app/caret.rs`).
//!
//! Every edit is checked by typing the edited number again from scratch, key by key, with the
//! same limits as typing it (`Input::retype`): a digit that typing couldn't add (a 17th one, or
//! one that would make the number too large for the word) is refused, and zeros before the
//! first digit disappear as they do when typed first.

use super::{Calculator, Command, Func, Input, IntegerMode};

/// Where the caret is: before the `index`th character of the number's digits (with its
/// decimal point), or of the exponent's digits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Caret {
    pub exponent: bool,
    pub index: usize,
}

/// A caret movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaretMove {
    Left,
    Right,
    /// To the start of the number (Home, Ctrl+Left).
    Start,
    /// To its end, after the exponent if it has one (End, Ctrl+Right).
    End,
}

impl Input {
    /// The caret at the end, where typing adds today.
    pub(super) fn end_caret(&self) -> Caret {
        match &self.exponent {
            Some(exponent) => Caret {
                exponent: true,
                index: exponent.digits.len(),
            },
            None => Caret {
                exponent: false,
                index: self.digits.len(),
            },
        }
    }

    /// The caret if it is shown, kept inside the number.
    pub(super) fn caret(&self) -> Option<Caret> {
        let caret = self.caret?;
        Some(self.clamp(caret))
    }

    fn clamp(&self, caret: Caret) -> Caret {
        match (&self.exponent, caret.exponent) {
            (Some(exponent), true) => Caret {
                exponent: true,
                index: caret.index.min(exponent.digits.len()),
            },
            _ => Caret {
                exponent: false,
                index: caret.index.min(self.digits.len()),
            },
        }
    }

    /// Whether the caret is shown somewhere other than the end, where keys edit at the caret
    /// instead of adding at the end.
    fn caret_inside(&self) -> Option<Caret> {
        self.caret().filter(|&caret| caret != self.end_caret())
    }

    /// Shows the caret at `caret`.
    pub(super) fn show_caret(&mut self, caret: Caret) {
        self.caret = Some(self.clamp(caret));
    }

    pub(super) fn move_caret(&mut self, movement: CaretMove) {
        let caret = self.caret().unwrap_or_else(|| self.end_caret());
        let mantissa = self.digits.len();
        let next = match movement {
            CaretMove::Start => Caret::default(),
            CaretMove::End => self.end_caret(),
            CaretMove::Left if caret.exponent && caret.index == 0 => Caret {
                exponent: false,
                index: mantissa,
            },
            CaretMove::Left => Caret {
                index: caret.index.saturating_sub(1),
                ..caret
            },
            CaretMove::Right if !caret.exponent && caret.index == mantissa => {
                if self.exponent.is_some() {
                    Caret {
                        exponent: true,
                        index: 0,
                    }
                } else {
                    caret
                }
            }
            CaretMove::Right => Caret {
                index: caret.index + 1,
                ..caret
            },
        };
        self.show_caret(next);
    }

    /// Types a digit at the caret, or adds it at the end when the caret is there or hidden.
    pub(super) fn insert_digit(
        &mut self,
        digit: u8,
        integer: Option<&IntegerMode>,
        max: usize,
    ) -> bool {
        let Some(caret) = self.caret_inside() else {
            let shown = self.caret.is_some();
            let added = self.try_add_digit(digit, integer, max);
            if shown {
                self.caret = Some(self.end_caret());
            }
            return added;
        };
        let c = char::from_digit(digit.into(), 16)
            .unwrap_or('0')
            .to_ascii_uppercase();
        let (mut mantissa, mut exponent) = self.parts();
        match (&mut exponent, caret.exponent) {
            (Some(exponent), true) => exponent.insert(caret.index, c),
            _ => mantissa.insert(caret.index, c),
        }
        self.apply_edit(mantissa, exponent, caret, 1, integer, max)
    }

    /// Types the decimal point at the caret.
    pub(super) fn insert_point(&mut self, integer: Option<&IntegerMode>, max: usize) -> bool {
        let Some(caret) = self.caret_inside() else {
            let shown = self.caret.is_some();
            let added = self.try_add_point();
            if shown {
                self.caret = Some(self.end_caret());
            }
            return added;
        };
        if caret.exponent || self.digits.contains('.') {
            return false;
        }
        let (mut mantissa, exponent) = self.parts();
        mantissa.insert(caret.index, '.');
        self.apply_edit(mantissa, exponent, caret, 1, integer, max)
    }

    /// ⌫: removes the character before the caret (the last one when it is at the end or
    /// hidden, as today).
    pub(super) fn backspace_at_caret(&mut self, integer: Option<&IntegerMode>, max: usize) {
        let Some(caret) = self.caret_inside() else {
            let shown = self.caret.is_some();
            self.backspace();
            if shown {
                self.caret = Some(self.end_caret());
            }
            return;
        };
        let (mut mantissa, mut exponent) = self.parts();
        match (&mut exponent, caret.exponent) {
            (Some(digits), true) => {
                if caret.index == 0 {
                    // Before the exponent's first digit: `e+` goes only once it's empty.
                    if digits.is_empty() {
                        exponent = None;
                    } else {
                        return;
                    }
                } else {
                    digits.remove(caret.index - 1);
                }
            }
            _ => {
                if caret.index == 0 {
                    return;
                }
                mantissa.remove(caret.index - 1);
            }
        }
        let at = match (&exponent, caret.exponent) {
            (Some(_), true) => Caret {
                exponent: true,
                index: caret.index - 1,
            },
            (None, true) => Caret {
                exponent: false,
                index: mantissa.len(),
            },
            _ => Caret {
                exponent: false,
                index: caret.index - 1,
            },
        };
        self.apply_edit(mantissa, exponent, at, 0, integer, max);
    }

    /// Delete: removes the character after the caret.
    pub(super) fn delete_at_caret(&mut self, integer: Option<&IntegerMode>, max: usize) {
        let caret = self.caret().unwrap_or_else(|| self.end_caret());
        let (mut mantissa, mut exponent) = self.parts();
        match (&mut exponent, caret.exponent) {
            (Some(digits), true) => {
                if caret.index >= digits.len() {
                    return;
                }
                digits.remove(caret.index);
            }
            _ => {
                if caret.index >= mantissa.len() {
                    return;
                }
                mantissa.remove(caret.index);
            }
        }
        self.apply_edit(mantissa, exponent, caret, 0, integer, max);
    }

    /// The number's characters and the exponent's digits.
    fn parts(&self) -> (Vec<char>, Option<Vec<char>>) {
        (
            self.digits.chars().collect(),
            self.exponent
                .as_ref()
                .map(|exponent| exponent.digits.chars().collect()),
        )
    }

    /// Takes the edited characters if typing them is allowed, with the caret `moved`
    /// characters after `caret` (where the edit happened). Returns whether it took them.
    fn apply_edit(
        &mut self,
        mantissa: Vec<char>,
        exponent: Option<Vec<char>>,
        caret: Caret,
        moved: usize,
        integer: Option<&IntegerMode>,
        max: usize,
    ) -> bool {
        let mantissa: String = mantissa.into_iter().collect();
        let exponent_text: Option<String> = exponent.map(|digits| digits.into_iter().collect());
        let exponent_negative = self
            .exponent
            .as_ref()
            .is_some_and(|exponent| exponent.negative);
        let Some(mut edited) = Input::retype(
            self.negative,
            &mantissa,
            exponent_text
                .as_deref()
                .map(|digits| (exponent_negative, digits)),
            integer,
            max,
        ) else {
            return false;
        };
        // Typing drops zeros before the first digit (`05` is 5) and puts one before a
        // leading point (`.5` is 0.5): the caret moves with the characters after it.
        let caret = if caret.exponent {
            let typed = exponent_text.as_deref().unwrap_or_default();
            let kept = edited
                .exponent
                .as_ref()
                .map_or(0, |exponent| exponent.digits.len());
            let index = (caret.index + moved).saturating_sub(typed.len().saturating_sub(kept));
            Caret {
                exponent: edited.exponent.is_some(),
                index,
            }
        } else {
            let shift = edited.digits.len() as isize - mantissa.len() as isize;
            let index = (caret.index + moved) as isize + shift;
            Caret {
                exponent: false,
                index: index.max(0) as usize,
            }
        };
        let changed = edited.text() != self.text();
        edited.caret = Some(edited.clamp(caret));
        *self = edited;
        changed
    }

    /// The input that typing these characters gives, or `None` if a key would be refused:
    /// the digits and decimal point in order, the sign after the first digit, then `exp`, the
    /// exponent's digits and its sign.
    pub(super) fn retype(
        negative: bool,
        mantissa: &str,
        exponent: Option<(bool, &str)>,
        integer: Option<&IntegerMode>,
        max: usize,
    ) -> Option<Input> {
        let mut input = Input::default();
        for c in mantissa.chars() {
            let typed = if c == '.' {
                input.try_add_point()
            } else {
                let digit = c.to_digit(16)? as u8;
                input.try_add_digit(digit, integer, max)
            };
            if !typed {
                return None;
            }
            if negative
                && !input.negative
                && !input.digits.is_empty()
                && !input.toggle_sign(integer)
            {
                return None;
            }
        }
        if let Some((exponent_negative, digits)) = exponent {
            input.try_begin_exponent();
            for c in digits.chars() {
                let digit = c.to_digit(10)? as u8;
                if !input.try_add_digit(digit, integer, max) {
                    return None;
                }
            }
            if exponent_negative
                && input
                    .exponent
                    .as_ref()
                    .is_some_and(|exponent| !exponent.digits.is_empty())
            {
                input.toggle_sign(integer);
            }
        }
        Some(input)
    }

    /// The keys that type this input fresh, as `retype` does it. In the Programmer mode the
    /// sign comes after the first digit, where a number like −128 in a BYTE still fits;
    /// elsewhere after the digits, as people type it.
    #[allow(dead_code)] // The tape (option B) types edited numbers with these.
    pub(super) fn keys(&self, integer: Option<&IntegerMode>) -> Vec<Command> {
        let mut keys = Vec::new();
        let mut signed = false;
        let mut stored = false;
        for c in self.digits.chars() {
            keys.push(match c {
                '.' => Command::Decimal,
                c => Command::Digit(c.to_digit(16).unwrap_or_default() as u8),
            });
            // A leading 0 isn't stored, so the sign waits for the next key.
            stored |= c != '0' || keys.len() > 1;
            if integer.is_some() && self.negative && !signed && stored {
                keys.push(Command::Function(Func::Negate));
                signed = true;
            }
        }
        if keys.is_empty() {
            keys.push(Command::Digit(0));
        }
        if self.negative && !signed {
            keys.push(Command::Function(Func::Negate));
        }
        if let Some(exponent) = &self.exponent {
            keys.push(Command::Exponent);
            for c in exponent.digits.chars() {
                keys.push(Command::Digit(c.to_digit(10).unwrap_or_default() as u8));
            }
            if exponent.negative {
                keys.push(Command::Function(Func::Negate));
            }
        }
        keys
    }

    /// The typed copy of a number the display shows (`-1,234.5`, `1.e+16`, `FF 00`), to edit
    /// it as if it had been typed. `None` if typing couldn't give it.
    #[allow(dead_code)] // The tape (option B) edits computed values through these.
    pub(super) fn from_display(
        text: &str,
        integer: Option<&IntegerMode>,
        max: usize,
    ) -> Option<Input> {
        let plain: String = text.chars().filter(|&c| !matches!(c, ',' | ' ')).collect();
        let (negative, unsigned) = match plain.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, plain.as_str()),
        };
        let (mantissa, exponent) = match unsigned.split_once('e') {
            Some((mantissa, exponent)) => {
                let (exponent_negative, digits) = match exponent.strip_prefix('-') {
                    Some(digits) => (true, digits),
                    None => (false, exponent.trim_start_matches('+')),
                };
                (
                    mantissa,
                    Some((exponent_negative, digits.trim_start_matches('0'))),
                )
            }
            None => (unsigned, None),
        };
        // BIN's padding and a lone 0 aren't typed.
        let mantissa = if integer.is_some() || !mantissa.starts_with("0.") {
            mantissa.trim_start_matches('0')
        } else {
            mantissa
        };
        let typed = Input::retype(negative, mantissa, exponent, integer, max)?;
        // The typed copy has to be the number shown, digit for digit.
        let sign = if negative && !typed.digits.is_empty() {
            "-"
        } else {
            ""
        };
        let shown_mantissa = if mantissa.is_empty() { "0" } else { mantissa };
        let shown_exponent = unsigned
            .split_once('e')
            .map(|(_, exponent)| format!("e{exponent}"))
            .unwrap_or_default();
        (typed.text() == format!("{sign}{shown_mantissa}{shown_exponent}")).then_some(typed)
    }
}

impl Calculator {
    /// The caret in the number being typed, if it is shown.
    pub fn entry_caret(&self) -> Option<Caret> {
        if self.record && self.error.is_none() {
            self.input.caret()
        } else {
            None
        }
    }

    /// The characters the entry's caret moves through: the number's digits with its decimal
    /// point, and the exponent's digits once `exp` was pressed. Empty while 0 is shown.
    pub fn entry_characters(&self) -> (String, Option<String>) {
        (
            self.input.digits.clone(),
            self.input
                .exponent
                .as_ref()
                .map(|exponent| exponent.digits.clone()),
        )
    }

    /// Whether a number is being typed that the caret can be shown in.
    pub fn can_show_entry_caret(&self) -> bool {
        self.record && self.error.is_none() && !self.input.is_empty()
    }

    /// Shows the caret in the number being typed. Returns whether there is one to edit.
    pub fn show_entry_caret(&mut self, caret: Caret) -> bool {
        if !self.record || self.error.is_some() {
            return false;
        }
        self.input.show_caret(caret);
        true
    }

    /// Moves the entry's caret.
    pub fn move_entry_caret(&mut self, movement: CaretMove) {
        if self.entry_caret().is_some() {
            self.input.move_caret(movement);
        }
    }

    /// Delete: removes the character after the entry's caret.
    pub fn delete_after_caret(&mut self) {
        if self.entry_caret().is_some() {
            let max = self.digits as usize;
            self.input.delete_at_caret(self.integer.as_ref(), max);
        }
    }

    /// Hides the entry's caret; typing adds at the end again. Returns whether it was shown.
    pub fn hide_entry_caret(&mut self) -> bool {
        self.input.caret.take().is_some() && self.record
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Op, Radix, WordSize};
    use super::*;

    fn typed(calc: &mut Calculator, digits: &str) {
        for c in digits.chars() {
            calc.execute(match c {
                '.' => Command::Decimal,
                'e' => Command::Exponent,
                '~' => Command::Function(Func::Negate),
                '<' => Command::Backspace,
                c => Command::Digit(c.to_digit(16).unwrap() as u8),
            });
        }
    }

    fn at(index: usize) -> Caret {
        Caret {
            exponent: false,
            index,
        }
    }

    fn in_exponent(index: usize) -> Caret {
        Caret {
            exponent: true,
            index,
        }
    }

    #[test]
    fn a_hidden_caret_changes_nothing() {
        let mut calc = Calculator::default();
        typed(&mut calc, "1234");
        assert_eq!(calc.entry_caret(), None);
        typed(&mut calc, "5<");
        assert_eq!(calc.display(), "1,234");
    }

    #[test]
    fn digits_go_in_at_the_caret() {
        let mut calc = Calculator::default();
        typed(&mut calc, "324");
        assert!(calc.show_entry_caret(at(1)));
        // Konst's example: delete the 3, type 5.
        typed(&mut calc, "<5");
        assert_eq!(calc.display(), "524");
        assert_eq!(calc.entry_caret(), Some(at(1)));
        calc.move_entry_caret(CaretMove::End);
        typed(&mut calc, "6");
        assert_eq!(calc.display(), "5,246");
        calc.move_entry_caret(CaretMove::Start);
        typed(&mut calc, "9");
        assert_eq!(calc.display(), "95,246");
        calc.execute(Command::Operator(Op::Add));
        assert_eq!(calc.entry_caret(), None);
        assert_eq!(calc.expression(), "95246 +");
    }

    #[test]
    fn caret_moves_and_deletes() {
        let mut calc = Calculator::default();
        typed(&mut calc, "12.5");
        calc.show_entry_caret(at(4));
        calc.move_entry_caret(CaretMove::Left);
        calc.move_entry_caret(CaretMove::Left);
        assert_eq!(calc.entry_caret(), Some(at(2)));
        calc.delete_after_caret();
        assert_eq!(calc.display(), "125");
        calc.move_entry_caret(CaretMove::Right);
        calc.move_entry_caret(CaretMove::Right);
        calc.move_entry_caret(CaretMove::Right);
        assert_eq!(calc.entry_caret(), Some(at(3)));
        calc.delete_after_caret();
        assert_eq!(calc.display(), "125");
        calc.move_entry_caret(CaretMove::Start);
        typed(&mut calc, "<");
        assert_eq!(calc.display(), "125");
        calc.delete_after_caret();
        calc.delete_after_caret();
        calc.delete_after_caret();
        assert_eq!(calc.display(), "0");
        assert_eq!(calc.entry_caret(), Some(at(0)));
        typed(&mut calc, "7");
        assert_eq!(calc.display(), "7");
    }

    #[test]
    fn the_decimal_point_at_the_caret() {
        let mut calc = Calculator::default();
        typed(&mut calc, "1234");
        calc.show_entry_caret(at(2));
        typed(&mut calc, ".");
        assert_eq!(calc.display(), "12.34");
        assert_eq!(calc.entry_caret(), Some(at(3)));
        // Only one point.
        calc.show_entry_caret(at(1));
        typed(&mut calc, ".");
        assert_eq!(calc.display(), "12.34");
        // A point at the start gets its zero, and the caret stays after the point.
        let mut calc = Calculator::default();
        typed(&mut calc, "5");
        calc.show_entry_caret(at(0));
        typed(&mut calc, ".");
        assert_eq!(calc.display(), "0.5");
        assert_eq!(calc.entry_caret(), Some(at(2)));
        // Removing the point of 0.5 leaves 5, as typing 05 would.
        typed(&mut calc, "<");
        assert_eq!(calc.display(), "5");
        assert_eq!(calc.entry_caret(), Some(at(0)));
    }

    #[test]
    fn leading_zeros_disappear() {
        let mut calc = Calculator::default();
        typed(&mut calc, "5");
        calc.show_entry_caret(at(0));
        typed(&mut calc, "0");
        assert_eq!(calc.display(), "5");
        assert_eq!(calc.entry_caret(), Some(at(0)));
        // 105 without its 1 is 5.
        let mut calc = Calculator::default();
        typed(&mut calc, "105");
        calc.show_entry_caret(at(1));
        typed(&mut calc, "<");
        assert_eq!(calc.display(), "5");
        assert_eq!(calc.entry_caret(), Some(at(0)));
    }

    #[test]
    fn the_digit_limit_holds_at_the_caret() {
        let mut calc = Calculator::default();
        typed(&mut calc, "1234567890123456");
        calc.show_entry_caret(at(3));
        typed(&mut calc, "9");
        assert_eq!(calc.display(), "1,234,567,890,123,456");
        assert_eq!(calc.entry_caret(), Some(at(3)));
        // A point doesn't count.
        typed(&mut calc, ".");
        assert_eq!(calc.display(), "123.4567890123456");
        let mut calc = Calculator::with_precedence();
        typed(&mut calc, "12345678901234567890123456789012");
        calc.show_entry_caret(at(0));
        typed(&mut calc, "9");
        assert_eq!(calc.entry_characters().0.len(), 32);
    }

    #[test]
    fn the_exponent_is_a_second_segment() {
        let mut calc = Calculator::with_precedence();
        typed(&mut calc, "15e3");
        assert_eq!(calc.display(), "15.e+3");
        calc.show_entry_caret(at(0));
        calc.move_entry_caret(CaretMove::End);
        assert_eq!(calc.entry_caret(), Some(in_exponent(1)));
        calc.move_entry_caret(CaretMove::Left);
        assert_eq!(calc.entry_caret(), Some(in_exponent(0)));
        typed(&mut calc, "2");
        assert_eq!(calc.display(), "15.e+23");
        calc.move_entry_caret(CaretMove::Left);
        calc.move_entry_caret(CaretMove::Left);
        assert_eq!(calc.entry_caret(), Some(at(3)));
        calc.move_entry_caret(CaretMove::Left);
        typed(&mut calc, "<");
        assert_eq!(calc.display(), "1.e+23");
        // ± flips the exponent's sign wherever the caret is.
        typed(&mut calc, "~");
        assert_eq!(calc.display(), "1.e-23");
        // Four exponent digits at most.
        calc.show_entry_caret(in_exponent(0));
        typed(&mut calc, "99");
        assert_eq!(calc.display(), "1.e-9923");
        typed(&mut calc, "9");
        assert_eq!(calc.display(), "1.e-9923");
        // Backspace before the exponent's digits does nothing while it has some.
        calc.show_entry_caret(in_exponent(0));
        typed(&mut calc, "<");
        assert_eq!(calc.display(), "1.e-9923");
        for _ in 0..4 {
            calc.delete_after_caret();
        }
        assert_eq!(calc.display(), "1.e+0");
        typed(&mut calc, "<");
        assert_eq!(calc.display(), "1.");
        assert_eq!(calc.entry_caret(), Some(at(2)));
    }

    #[test]
    fn negative_numbers_keep_their_sign() {
        let mut calc = Calculator::default();
        typed(&mut calc, "45~");
        calc.show_entry_caret(at(1));
        typed(&mut calc, "6");
        assert_eq!(calc.display(), "-465");
        calc.show_entry_caret(at(0));
        calc.delete_after_caret();
        calc.delete_after_caret();
        calc.delete_after_caret();
        assert_eq!(calc.display(), "0");
    }

    #[test]
    fn programmer_digits_at_the_caret() {
        let mut calc = Calculator::programmer();
        calc.execute(Command::Radix(Radix::Hex));
        typed(&mut calc, "FF00");
        calc.show_entry_caret(at(2));
        typed(&mut calc, "A");
        assert_eq!(calc.display(), "F FA00");
        // A QWORD holds 16 hex digits.
        let mut calc = Calculator::programmer();
        calc.execute(Command::Radix(Radix::Hex));
        typed(&mut calc, "123456789ABCDEF0");
        calc.show_entry_caret(at(4));
        typed(&mut calc, "1");
        assert_eq!(calc.display(), "1234 5678 9ABC DEF0");
        // Binary keeps its padding out of the caret's way.
        let mut calc = Calculator::programmer();
        calc.execute(Command::Radix(Radix::Binary));
        typed(&mut calc, "101");
        calc.show_entry_caret(at(1));
        typed(&mut calc, "1");
        assert_eq!(calc.display(), "1101");
        calc.show_entry_caret(at(0));
        calc.delete_after_caret();
        assert_eq!(calc.display(), "0101");
    }

    #[test]
    fn programmer_word_limits_at_the_caret() {
        let mut calc = Calculator::programmer();
        calc.execute(Command::WordSize(WordSize::Byte));
        typed(&mut calc, "17");
        calc.show_entry_caret(at(1));
        // 127 fits a BYTE, 1227 doesn't.
        typed(&mut calc, "2");
        assert_eq!(calc.display(), "127");
        typed(&mut calc, "2");
        assert_eq!(calc.display(), "127");
        // −128 fits only while negative.
        let mut calc = Calculator::programmer();
        calc.execute(Command::WordSize(WordSize::Byte));
        typed(&mut calc, "1~28");
        assert_eq!(calc.display(), "-128");
        calc.show_entry_caret(at(1));
        typed(&mut calc, "<1");
        assert_eq!(calc.display(), "-128");
        // OCT: a BYTE takes 3 digits only after a small first one.
        let mut calc = Calculator::programmer();
        calc.execute(Command::WordSize(WordSize::Byte));
        calc.execute(Command::Radix(Radix::Octal));
        typed(&mut calc, "77");
        calc.show_entry_caret(at(0));
        typed(&mut calc, "4");
        assert_eq!(calc.display(), "77");
        typed(&mut calc, "3");
        assert_eq!(calc.display(), "377");
    }

    #[test]
    fn no_caret_outside_typing() {
        let mut calc = Calculator::default();
        typed(&mut calc, "5");
        calc.execute(Command::Operator(Op::Add));
        assert!(!calc.show_entry_caret(at(0)));
        typed(&mut calc, "1");
        calc.execute(Command::Operator(Op::Divide));
        typed(&mut calc, "0");
        calc.execute(Command::Equals);
        assert!(!calc.show_entry_caret(at(0)));
    }

    #[test]
    fn typing_keys_round_trip() {
        for (text, word) in [
            ("-12.5", None),
            ("0.05", None),
            ("1.5e-30", None),
            ("7", None),
            ("-128", Some(WordSize::Byte)),
        ] {
            let mode = word.map(|word| IntegerMode {
                word,
                ..IntegerMode::default()
            });
            let input = Input::from_display(text, mode.as_ref(), 32).unwrap();
            let mut calc = match mode {
                Some(mode) => {
                    let mut calc = Calculator::programmer();
                    calc.integer = Some(mode);
                    calc
                }
                None => Calculator::with_precedence(),
            };
            for key in input.keys(mode.as_ref()) {
                calc.execute(key);
            }
            assert_eq!(calc.input.text(), input.text(), "{text}");
        }
        assert!(Input::from_display("0.3333333333333333", None, 16).is_some());
        assert!(Input::from_display("1.e+16", None, 16).is_some());
        // More digits than typing allows.
        assert!(Input::from_display("12345678901234567", None, 16).is_none());
    }
}
