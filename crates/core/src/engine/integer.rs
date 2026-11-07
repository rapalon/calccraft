//! The Programmer mode's integer arithmetic: what `CCalcEngine` does only in its integer
//! mode (`m_fIntegerMode`), with a radix (`m_radix`) and a word size (`m_numwidth`).
//!
//! Like Windows, the values stay rationals: the engine truncates the current value to
//! an unsigned integer of the word size whenever Windows' `DisplayNum` does
//! (`TruncateNumForIntMath`), and the operations work on rationals the way `ratpak`
//! does. Most of the time that is plain 64-bit arithmetic, but a division or a right
//! shift inside parentheses can leave a fraction for the next operation, and the
//! results then match Windows only if they are computed the same way.

use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, Signed, ToPrimitive, Zero};

use super::{CalcError, Number, Op, group_thousands};

/// A number base of the Programmer mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Radix {
    Hex,
    Decimal,
    Octal,
    Binary,
}

impl Radix {
    /// In the order of the radix list: HEX, DEC, OCT, BIN.
    pub const ALL: [Radix; 4] = [Radix::Hex, Radix::Decimal, Radix::Octal, Radix::Binary];

    pub fn base(self) -> u32 {
        match self {
            Radix::Hex => 16,
            Radix::Decimal => 10,
            Radix::Octal => 8,
            Radix::Binary => 2,
        }
    }

    /// The radix list's label (`hexButton.Tag` and so on, the same in every language).
    pub fn label(self) -> &'static str {
        match self {
            Radix::Hex => "HEX",
            Radix::Decimal => "DEC",
            Radix::Octal => "OCT",
            Radix::Binary => "BIN",
        }
    }
}

/// A word size of the Programmer mode (`NUM_WIDTH`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WordSize {
    Qword,
    Dword,
    Word,
    Byte,
}

impl WordSize {
    pub fn bits(self) -> u32 {
        match self {
            WordSize::Qword => 64,
            WordSize::Dword => 32,
            WordSize::Word => 16,
            WordSize::Byte => 8,
        }
    }

    /// The size the word size button switches to: QWORD → DWORD → WORD → BYTE → QWORD
    /// (`OnBitLengthButtonPressed`).
    pub fn next(self) -> WordSize {
        match self {
            WordSize::Qword => WordSize::Dword,
            WordSize::Dword => WordSize::Word,
            WordSize::Word => WordSize::Byte,
            WordSize::Byte => WordSize::Qword,
        }
    }

    /// The button's text, which Windows doesn't translate.
    pub fn label(self) -> &'static str {
        match self {
            WordSize::Qword => "QWORD",
            WordSize::Dword => "DWORD",
            WordSize::Word => "WORD",
            WordSize::Byte => "BYTE",
        }
    }

    /// All bits of the word set (`GetChopNumber`).
    fn chop(self) -> BigInt {
        (BigInt::one() << self.bits()) - 1
    }

    /// The largest positive value in decimal (`GetMaxDecimalValueString`): `127` for a byte.
    fn max_decimal(self) -> String {
        ((BigInt::one() << (self.bits() - 1)) - BigInt::one()).to_string()
    }
}

/// The Programmer mode's settings and its carry bit for the rotations through carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct IntegerMode {
    pub radix: Radix,
    pub word: WordSize,
    /// `m_carryBit`, cleared by `C`.
    pub carry: bool,
}

impl Default for IntegerMode {
    fn default() -> Self {
        Self {
            radix: Radix::Decimal,
            word: WordSize::Qword,
            carry: false,
        }
    }
}

impl IntegerMode {
    /// The value as an unsigned integer of the word size: truncated towards zero, and
    /// a negative one in two's complement (`TruncateNumForIntMath`).
    pub fn truncate(&self, v: &Number) -> Number {
        Number::from_integer(v.to_integer() & self.word.chop())
    }

    /// Whether the word's highest bit is set, which makes the value negative in DEC.
    fn msb(&self, v: &Number) -> bool {
        (to_u64(v) >> (self.word.bits() - 1)) & 1 == 1
    }

    /// The value for the display in `radix` (`GetStringForDisplay`): unsigned, except in
    /// decimal, where a value with the highest bit set is negative. No grouping.
    pub fn format(&self, v: &Number, radix: Radix) -> String {
        let truncated = self.truncate(v);
        let value = if radix == Radix::Decimal && self.msb(&truncated) {
            truncated.to_integer() - (BigInt::one() << self.word.bits())
        } else {
            truncated.to_integer()
        };
        to_radix(&value, radix)
    }

    /// `C` (`IDC_CLEAR`) clears the carry bit.
    pub fn clear(&mut self) {
        self.carry = false;
    }

    /// The value in the signed form that a smaller or larger word extends: a value
    /// whose highest bit is set becomes negative (`SetRadixTypeAndNumWidth`).
    pub fn signed(&self, v: &Number) -> Number {
        if self.msb(v) {
            -(boolean(v, &Number::from_integer(self.word.chop()), Bits::Xor) + Number::one())
        } else {
            v.clone()
        }
    }

    /// How many digits can be typed before the special cases for the last one
    /// (`UpdateMaxIntDigits`): in decimal one less than the largest value has, otherwise
    /// as many whole digits as fit into the word.
    fn max_digits(&self) -> usize {
        match self.radix {
            Radix::Decimal => self.word.max_decimal().len() - 1,
            radix => (self.word.bits() / radix.base().trailing_zeros()) as usize,
        }
    }

    /// Whether `digit` (`'0'`–`'9'`, `'A'`–`'F'`) can be added to the typed `digits`
    /// (`CalcInput::TryAddDigit` in integer mode). The last digit that fits is allowed
    /// only if the value stays within the word: in octal only after a small first digit,
    /// in decimal only up to the largest value (or one more for a negative number).
    pub fn digit_fits(&self, digits: &str, digit: char, negative: bool) -> bool {
        let max = self.max_digits();
        if digits.len() < max {
            return true;
        }
        if digits.len() > max {
            return false;
        }
        let first = digits.chars().next().unwrap_or('0');
        match self.radix {
            Radix::Octal => match self.word.bits() % 3 {
                1 => first == '1',
                2 => first <= '3',
                _ => false,
            },
            Radix::Decimal => {
                let limit = self.word.max_decimal();
                let Some(&last) = limit.as_bytes().get(digits.len()) else {
                    return false;
                };
                match digits.as_bytes().cmp(&limit.as_bytes()[..digits.len()]) {
                    std::cmp::Ordering::Less => true,
                    std::cmp::Ordering::Equal => {
                        digit as u8 <= last || (negative && digit as u8 <= last + 1)
                    }
                    std::cmp::Ordering::Greater => false,
                }
            }
            Radix::Hex | Radix::Binary => false,
        }
    }

    /// Whether a typed negative decimal number may become positive: `-128` can't in a
    /// byte, since 128 doesn't fit (`CalcInput::TryToggleSign`).
    pub fn sign_toggles(&self, digits: &str, negative: bool) -> bool {
        let limit = self.word.max_decimal();
        !(negative && digits.len() >= limit.len() && digits.bytes().last() > limit.bytes().last())
    }

    /// `left op right` (`CCalcEngine::DoOperation` in integer mode). The operands and
    /// results are rationals as in Windows; `DisplayNum` truncates them later.
    pub fn apply(&self, left: &Number, op: Op, right: &Number) -> Result<Number, CalcError> {
        let bits = self.word.bits();
        let chop = Number::from_integer(self.word.chop());
        // A shift by the word size or more has no result (`CALC_E_NORESULT`).
        let shift = || -> Result<i64, CalcError> {
            if *right >= Number::from_integer(bits.into()) {
                return Err(CalcError::NoResult);
            }
            right.to_integer().to_i64().ok_or(CalcError::InvalidInput)
        };
        Ok(match op {
            Op::And => boolean(right, left, Bits::And),
            Op::Or => boolean(right, left, Bits::Or),
            Op::Xor => boolean(right, left, Bits::Xor),
            Op::Nand => boolean(&boolean(right, left, Bits::And), &chop, Bits::Xor),
            Op::Nor => boolean(&boolean(right, left, Bits::Or), &chop, Bits::Xor),
            Op::LeftShift => Number::from_integer(left.to_integer()) * pow2(shift()?),
            Op::RightShiftLogical => Number::from_integer(left.to_integer()) / pow2(shift()?),
            Op::RightShift => {
                // An arithmetic shift fills the new high bits with the sign bit.
                let n = shift()?;
                let result = Number::from_integer(left.to_integer()) / pow2(n);
                if self.msb(left) {
                    let high = Number::from_integer((&chop / pow2(n)).to_integer());
                    let fill = boolean(&high, &chop, Bits::Xor);
                    boolean(&Number::from_integer(result.to_integer()), &fill, Bits::Or)
                } else {
                    result
                }
            }
            Op::Add => left + right,
            Op::Subtract => left - right,
            Op::Multiply => left * right,
            Op::Divide | Op::Mod => {
                // Both operands are signed for division: values with the highest bit
                // set divide as their two's complement magnitude.
                let magnitude = |v: &Number| {
                    if self.msb(v) {
                        (-self.signed(v), true)
                    } else {
                        (v.clone(), false)
                    }
                };
                let (dividend, negative_dividend) = magnitude(left);
                let (divisor, negative_divisor) = magnitude(right);
                if op == Op::Divide {
                    if divisor.is_zero() {
                        return Err(if dividend.is_zero() {
                            CalcError::Undefined
                        } else {
                            CalcError::DivideByZero
                        });
                    }
                    let quotient = dividend / divisor;
                    if negative_dividend != negative_divisor {
                        -Number::from_integer(quotient.to_integer())
                    } else {
                        quotient
                    }
                } else {
                    // `remrat`: the remainder has the sign of the dividend; any value
                    // modulo 0 is undefined.
                    if divisor.is_zero() {
                        return Err(CalcError::Undefined);
                    }
                    let remainder = dividend % divisor;
                    if negative_dividend {
                        -Number::from_integer(remainder.to_integer())
                    } else {
                        remainder
                    }
                }
            }
            // The Scientific mode's operators don't exist here.
            Op::Power | Op::Root | Op::LogBase => return Err(CalcError::InvalidInput),
        })
    }

    /// `NOT`: all bits of the word flipped (`IDC_COM`).
    pub fn not(&self, v: &Number) -> Number {
        boolean(v, &Number::from_integer(self.word.chop()), Bits::Xor)
    }

    /// Rotates the value by one bit (`IDC_ROL`, `IDC_ROR`, `IDC_ROLC`, `IDC_RORC`). A
    /// rotation through carry shifts the carry bit in and the bit that falls out into it.
    pub fn rotate(&mut self, v: &Number, left: bool, through_carry: bool) -> Number {
        let bits = self.word.bits();
        let value = to_u64(&Number::from_integer(v.to_integer()));
        let result = if left {
            let msb = (value >> (bits - 1)) & 1;
            let shifted = value << 1;
            if through_carry {
                let carry = u64::from(self.carry);
                self.carry = msb == 1;
                shifted | carry
            } else {
                shifted | msb
            }
        } else {
            let lsb = value & 1;
            let shifted = value >> 1;
            if through_carry {
                let carry = u64::from(self.carry);
                self.carry = lsb == 1;
                shifted | (carry << (bits - 1))
            } else {
                shifted | (lsb << (bits - 1))
            }
        };
        Number::from_integer(result.into())
    }

    /// Flips one bit of the value (`TryToggleBit`); bits beyond the word do nothing.
    pub fn toggle_bit(&self, v: &Number, bit: u32) -> Option<Number> {
        (bit < self.word.bits())
            .then(|| boolean(v, &Number::from_integer(BigInt::one() << bit), Bits::Xor))
    }
}

/// The low 64 bits of the value's integer part (`Rational::ToUInt64_t`).
fn to_u64(v: &Number) -> u64 {
    (v.to_integer() & BigInt::from(u64::MAX))
        .to_u64()
        .unwrap_or_default()
}

/// 2ⁿ as a rational, for the shifts.
fn pow2(n: i64) -> Number {
    let power = BigInt::one() << n.unsigned_abs();
    if n < 0 {
        Number::new(BigInt::one(), power)
    } else {
        Number::from_integer(power)
    }
}

#[derive(Clone, Copy)]
enum Bits {
    And,
    Or,
    Xor,
}

/// A bitwise operation as `ratpak`'s `boolrat` does it: on the magnitudes of the
/// operands' integer parts, keeping the sign of `a`.
fn boolean(a: &Number, b: &Number, op: Bits) -> Number {
    let a = a.to_integer();
    let b = b.to_integer();
    let (x, y): (&BigUint, &BigUint) = (a.magnitude(), b.magnitude());
    let magnitude = match op {
        Bits::And => x & y,
        Bits::Or => x | y,
        Bits::Xor => x ^ y,
    };
    let sign = if a.is_negative() {
        Sign::Minus
    } else {
        Sign::Plus
    };
    Number::from_integer(BigInt::from_biguint(sign, magnitude))
}

/// An integer in a radix, with upper-case letters (`RatToString`).
pub(super) fn to_radix(v: &BigInt, radix: Radix) -> String {
    v.to_str_radix(radix.base()).to_uppercase()
}

/// Digit groups for the display (`GroupDigitsPerRadix`): decimal numbers get thousands
/// separators, octal numbers groups of 3 and hexadecimal and binary numbers groups of 4,
/// separated by spaces.
pub(super) fn group(text: &str, radix: Radix) -> String {
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text),
    };
    let size = match radix {
        Radix::Decimal => return format!("{sign}{}", group_thousands(digits)),
        Radix::Octal => 3,
        Radix::Hex | Radix::Binary => 4,
    };
    let mut out = String::with_capacity(digits.len() * 2);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(size) {
            out.push(' ');
        }
        out.push(c);
    }
    format!("{sign}{out}")
}

/// Fills the first group of a binary number with zeros, `1 0100` → `0001 0100`
/// (the view model's `AddPadding`). Zero stays `0`.
pub(super) fn pad_binary(text: &str) -> String {
    if text == "0" || text.is_empty() {
        return text.to_string();
    }
    let digits = text.chars().filter(|&c| c != ' ').count();
    let padding = (4 - digits % 4) % 4;
    format!("{}{text}", "0".repeat(padding))
}

// ------------------------------------------------------------------ paste

/// The characters `CopyPasteManager::ExtractOperands` looks at in the Programmer mode.
pub(super) const PASTE_CHARACTERS: &str = "0123456789+-.e*/()%abcdfABCDEF";

/// Checks one pasted operand like `CopyPasteManager` does in the Programmer mode: it must
/// match the radix's pattern (with C-style prefixes and suffixes such as `0x1F`, `1Fh`,
/// `0b101`, `255u`, and `_`, `'` or `` ` `` between digits), have no more digits than
/// fit into the word, and not be larger than the largest value (in decimal the largest
/// positive one, or one more for a negative number).
pub(super) fn paste_operand_is_valid(operand: &str, radix: Radix, word: WordSize) -> bool {
    if !operand_matches(operand, radix) {
        return false;
    }
    let sign_bit = u32::from(radix == Radix::Decimal);
    let bits = word.bits() - sign_bit;
    let bits_per_digit = match radix {
        Radix::Decimal => 10f64.log2(),
        radix => f64::from(radix.base().trailing_zeros()),
    };
    let max_length = (f64::from(bits) / bits_per_digit).ceil() as usize;
    let max_value = u64::MAX >> (64 - bits);

    let negative = operand.starts_with('-');
    let sanitized: String = operand
        .chars()
        .filter(|c| !matches!(c, '\'' | '_' | '`' | '(' | ')' | '-' | '+'))
        .collect();
    if operand_length(&sanitized, radix) > max_length {
        return false;
    }
    match leading_value(&sanitized, radix) {
        Some(value) => value <= max_value || (negative && value.wrapping_sub(1) == max_value),
        None => false,
    }
}

/// Whether an operand matches one of the radix's patterns in `CopyPasteManager`:
/// optional opening parentheses, the number, optional closing parentheses.
fn operand_matches(operand: &str, radix: Radix) -> bool {
    let space = |c: char| c.is_whitespace();
    let body = operand
        .trim_start_matches(space)
        .trim_start_matches('(')
        .trim_start_matches(space)
        .trim_end_matches(space)
        .trim_end_matches(')')
        .trim_end_matches(space);
    let hex = |c: char| c.is_ascii_hexdigit();
    let decimal = |c: char| c.is_ascii_digit();
    let octal = |c: char| ('0'..='7').contains(&c);
    let binary = |c: char| c == '0' || c == '1';
    match radix {
        Radix::Hex => {
            number(body, &["0x", "0X"], hex, unsigned_suffix)
                || number(body, &[], hex, |s| matches!(s, "" | "h" | "H"))
        }
        Radix::Decimal => {
            let signed = body.strip_prefix(['-', '+']).unwrap_or(body);
            number(signed, &[], decimal, long_suffix)
                || number(body, &["0n", "0N"], decimal, unsigned_suffix)
        }
        Radix::Octal => number(body, &["0o", "0t", "0O", "0T"], octal, unsigned_suffix),
        Radix::Binary => {
            number(body, &["0b", "0y", "0B", "0Y"], binary, unsigned_suffix)
                || number(body, &[], binary, |s| matches!(s, "" | "b" | "B"))
        }
    }
}

/// `prefix? digits ((_|'|`) digits)* suffix`, where the prefix is optional.
fn number(
    text: &str,
    prefixes: &[&str],
    digit: impl Fn(char) -> bool,
    suffix: impl Fn(&str) -> bool,
) -> bool {
    let candidates = prefixes
        .iter()
        .filter_map(|prefix| text.strip_prefix(prefix))
        .chain([text]);
    for rest in candidates {
        let mut chars = rest.char_indices().peekable();
        let mut end = 0;
        let mut ok = false;
        // Groups of digits joined by single separators.
        loop {
            let start = end;
            while let Some(&(i, c)) = chars.peek() {
                if !digit(c) {
                    break;
                }
                end = i + c.len_utf8();
                chars.next();
            }
            if end == start {
                break;
            }
            ok = true;
            match chars.peek() {
                Some(&(i, c @ ('_' | '\'' | '`')))
                    if rest[i + 1..].chars().next().is_some_and(&digit) =>
                {
                    end = i + c.len_utf8();
                    chars.next();
                }
                _ => break,
            }
        }
        if ok && suffix(&rest[end..]) {
            return true;
        }
    }
    false
}

/// `[uU]?[lL]{0,2}`.
fn unsigned_suffix(suffix: &str) -> bool {
    long_suffix(suffix.strip_prefix(['u', 'U']).unwrap_or(suffix))
}

/// `[lL]{0,2}`.
fn long_suffix(suffix: &str) -> bool {
    suffix.len() <= 2 && suffix.chars().all(|c| c == 'l' || c == 'L')
}

/// The operand's length without its radix prefix and suffix
/// (`CopyPasteManager::ProgrammerOperandLength`).
fn operand_length(operand: &str, radix: Radix) -> usize {
    let (prefixes, radix_suffix): (&[&str], Option<&str>) = match radix {
        Radix::Binary => (&["0B", "0Y"], Some("B")),
        Radix::Decimal => (&["-", "0N"], None),
        Radix::Octal => (&["0T", "0O"], None),
        Radix::Hex => (&["0X"], Some("H")),
    };
    let upper = operand.to_uppercase();
    let mut length = operand.chars().count();
    let suffixes = radix_suffix
        .into_iter()
        .chain(["ULL", "UL", "LL", "U", "L"]);
    for suffix in suffixes {
        if length >= suffix.len() && upper.ends_with(suffix) {
            length -= suffix.len();
            break;
        }
    }
    for prefix in prefixes {
        if length >= prefix.len() && upper.starts_with(prefix) {
            length -= prefix.len();
            break;
        }
    }
    length
}

/// The value of the operand's leading digits, after a radix prefix, or `None` if there
/// are none or they overflow 64 bits (`CopyPasteManager::TryOperandToULL`).
fn leading_value(operand: &str, radix: Radix) -> Option<u64> {
    let base = radix.base();
    let chars: Vec<char> = operand.trim_start().chars().collect();
    let mut index = 0;
    if chars.len() > 1 && chars[0] == '0' {
        let prefix = chars[1].to_ascii_uppercase();
        let has_prefix = match radix {
            Radix::Hex => prefix == 'X',
            Radix::Decimal => prefix == 'N',
            Radix::Octal => prefix == 'O' || prefix == 'T',
            Radix::Binary => prefix == 'B' || prefix == 'Y',
        };
        let first = chars.get(2).and_then(|c| c.to_digit(16));
        if has_prefix && first.is_some_and(|d| d < base) {
            index = 2;
        }
    }
    let mut value: u64 = 0;
    let mut digits = 0;
    for c in &chars[index..] {
        match c.to_digit(16).filter(|&d| d < base) {
            Some(d) => {
                value = value.checked_mul(base.into())?.checked_add(d.into())?;
                digits += 1;
            }
            None => break,
        }
    }
    (digits > 0).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(v: i64) -> Number {
        Number::from_integer(v.into())
    }

    fn mode(word: WordSize) -> IntegerMode {
        IntegerMode {
            word,
            ..IntegerMode::default()
        }
    }

    /// The radix list and the word size button show these texts in every language.
    #[test]
    fn radix_and_word_size_labels() {
        let radixes = [Radix::Hex, Radix::Decimal, Radix::Octal, Radix::Binary];
        assert_eq!(radixes.map(Radix::label), ["HEX", "DEC", "OCT", "BIN"]);
        let words = [
            WordSize::Qword,
            WordSize::Dword,
            WordSize::Word,
            WordSize::Byte,
        ];
        assert_eq!(
            words.map(WordSize::label),
            ["QWORD", "DWORD", "WORD", "BYTE"]
        );
    }

    #[test]
    fn truncation_wraps_into_the_word() {
        let byte = mode(WordSize::Byte);
        assert_eq!(byte.truncate(&n(-1)), n(255));
        assert_eq!(byte.truncate(&n(0x1FF)), n(0xFF));
        assert_eq!(byte.truncate(&Number::new(7.into(), 2.into())), n(3));
        assert_eq!(byte.truncate(&Number::new((-7).into(), 2.into())), n(253));
        let qword = mode(WordSize::Qword);
        assert_eq!(qword.format(&n(-5), Radix::Decimal), "-5");
        assert_eq!(qword.format(&n(-5), Radix::Hex), "FFFFFFFFFFFFFFFB");
        assert_eq!(byte.format(&n(200), Radix::Decimal), "-56");
        assert_eq!(byte.format(&n(200), Radix::Octal), "310");
    }

    #[test]
    fn clear_drops_the_carry_bit() {
        let mut mode = mode(WordSize::Byte);
        mode.carry = true;
        mode.clear();
        assert!(!mode.carry);
    }

    #[test]
    fn grouping_per_radix() {
        assert_eq!(group("FFFFFFFFFFFFFFFB", Radix::Hex), "FFFF FFFF FFFF FFFB");
        assert_eq!(group("12345", Radix::Hex), "1 2345");
        assert_eq!(
            group("1777777777777777777753", Radix::Octal),
            "1 777 777 777 777 777 777 753"
        );
        assert_eq!(group("-1234567", Radix::Decimal), "-1,234,567");
        assert_eq!(pad_binary(&group("10100", Radix::Binary)), "0001 0100");
        assert_eq!(pad_binary("1000"), "1000");
        assert_eq!(pad_binary("0"), "0");
    }

    #[test]
    fn digit_limits() {
        let mut m = mode(WordSize::Byte);
        m.radix = Radix::Decimal;
        assert!(m.digit_fits("12", '7', false));
        assert!(!m.digit_fits("12", '8', false));
        assert!(m.digit_fits("12", '8', true));
        assert!(!m.digit_fits("13", '0', false));
        assert!(!m.digit_fits("127", '0', false));
        assert!(m.sign_toggles("127", true));
        assert!(!m.sign_toggles("128", true));
        m.radix = Radix::Octal;
        assert!(m.digit_fits("37", '7', false));
        assert!(!m.digit_fits("47", '7', false));
        m.radix = Radix::Hex;
        assert!(m.digit_fits("F", 'F', false));
        assert!(!m.digit_fits("FF", 'F', false));
        let mut q = mode(WordSize::Qword);
        q.radix = Radix::Octal;
        assert!(q.digit_fits(&"7".repeat(20), '7', false));
        assert!(q.digit_fits(&format!("1{}", "7".repeat(20)), '7', false));
        assert!(!q.digit_fits(&format!("2{}", "7".repeat(20)), '7', false));
        q.radix = Radix::Binary;
        assert!(q.digit_fits(&"1".repeat(63), '1', false));
        assert!(!q.digit_fits(&"1".repeat(64), '1', false));
    }

    #[test]
    fn operations() {
        let byte = mode(WordSize::Byte);
        let apply = |l: i64, op: Op, r: i64| byte.truncate(&byte.apply(&n(l), op, &n(r)).unwrap());
        assert_eq!(apply(0b1010, Op::And, 0b1100), n(0b1000));
        assert_eq!(apply(0b1010, Op::Or, 0b1100), n(0b1110));
        assert_eq!(apply(0b1010, Op::Xor, 0b1100), n(0b0110));
        assert_eq!(apply(0b1010, Op::Nand, 0b1100), n(0b1111_0111));
        assert_eq!(apply(0b1010, Op::Nor, 0b1100), n(0b1111_0001));
        assert_eq!(apply(0b1000_0000, Op::RightShift, 1), n(0b1100_0000));
        assert_eq!(apply(0b1000_0000, Op::RightShiftLogical, 1), n(0b0100_0000));
        assert_eq!(apply(0b1100_0001, Op::LeftShift, 1), n(0b1000_0010));
        assert_eq!(
            byte.apply(&n(1), Op::LeftShift, &n(8)),
            Err(CalcError::NoResult)
        );
        // Division and remainder are signed: 0xF9 is -7.
        assert_eq!(apply(0xF9, Op::Divide, 2), n(0xFD));
        assert_eq!(apply(0xF9, Op::Mod, 2), n(0xFF));
        assert_eq!(apply(7, Op::Mod, 0xFE), n(1));
        assert_eq!(byte.apply(&n(7), Op::Mod, &n(0)), Err(CalcError::Undefined));
        assert_eq!(
            byte.apply(&n(7), Op::Divide, &n(0)),
            Err(CalcError::DivideByZero)
        );
        assert_eq!(
            byte.apply(&n(0), Op::Divide, &n(0)),
            Err(CalcError::Undefined)
        );
        // A right shift leaves a fraction, as in Windows; truncation comes later.
        assert_eq!(
            byte.apply(&n(5), Op::RightShift, &n(1)).unwrap(),
            Number::new(5.into(), 2.into())
        );
    }

    #[test]
    fn unary_operations() {
        let mut byte = mode(WordSize::Byte);
        assert_eq!(byte.not(&n(0)), n(0xFF));
        let rotated = byte.rotate(&n(0b1000_0001), true, false);
        assert_eq!(byte.truncate(&rotated), n(0b11));
        assert_eq!(byte.rotate(&n(0b1000_0001), false, false), n(0b1100_0000));
        // Through carry: the bit that falls out comes back one rotation later.
        let rotated = byte.rotate(&n(0b1000_0000), true, true);
        assert_eq!(byte.truncate(&rotated), n(0));
        assert!(byte.carry);
        assert_eq!(byte.rotate(&n(0), true, true), n(1));
        assert!(!byte.carry);
        assert_eq!(byte.toggle_bit(&n(0), 7), Some(n(0x80)));
        assert_eq!(byte.toggle_bit(&n(0), 8), None);
        assert_eq!(byte.signed(&n(0xFF)), n(-1));
    }

    #[test]
    fn paste_patterns() {
        use Radix::*;
        let ok = |s: &str, radix: Radix| paste_operand_is_valid(s, radix, WordSize::Qword);
        for s in [
            "0x1F",
            "1Fh",
            "1f",
            "FF_FF",
            "0xFFFF'FFFF'FFFF'FFFF",
            "ffULL",
            "(1F)",
        ] {
            assert!(ok(s, Hex), "{s}");
        }
        for s in ["0x", "1G", "1__2", "_1", "1Fhh", "0x1_0000_0000_0000_0000"] {
            assert!(!ok(s, Hex), "{s}");
        }
        for s in ["123", "-123", "+5", "0n12", "12L", "9223372036854775807"] {
            assert!(ok(s, Decimal), "{s}");
        }
        assert!(ok("-9223372036854775808", Decimal));
        assert!(!ok("9223372036854775808", Decimal));
        assert!(!ok("1A", Decimal));
        assert!(ok("0o17", Octal) && ok("1777777777777777777777", Octal));
        assert!(!ok("2777777777777777777777", Octal) && !ok("8", Octal));
        assert!(ok("0b101", Binary) && ok("101b", Binary) && ok("0y1", Binary));
        assert!(!ok("102", Binary));
        assert!(!paste_operand_is_valid("200", Decimal, WordSize::Byte));
        assert!(paste_operand_is_valid("-128", Decimal, WordSize::Byte));
        assert!(paste_operand_is_valid("FF", Hex, WordSize::Byte));
        assert!(!paste_operand_is_valid("1FF", Hex, WordSize::Byte));
    }
}
