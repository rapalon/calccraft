//! Formulas typed as text, such as `sin(30) + 2^10` or `1234.56 / 7`, evaluated with the
//! Scientific mode's arithmetic: the same exact `Number`s, functions, limits and error
//! messages as its keys, for callers that have text instead of keys (the MCP server's
//! `calculate` tool). The keys' paste (`Calculator::paste`) takes only numbers and operators,
//! as Windows does; this takes the functions too.
//!
//! The grammar is ordinary math, from the loosest binding to the tightest:
//!
//! - `+` and `-`;
//! - `*`, `×`, `·`, `/`, `÷`, and `mod` or `%` (the remainder, as the Scientific mode's `%`
//!   key, with the divisor's sign), and multiplication without a sign (`2pi`, `3(4 + 1)`);
//! - a sign (`-x`), so that `-2^2` is −4;
//! - `^`, right to left (`2^3^2` is 2⁹), whose exponent may have a sign (`2^-1`);
//! - after a value, `!` (the factorial, the gamma function for fractions), `²` and `³`;
//! - numbers (`1.5`, `.5`, `1.5e3`, `2E-4`), `pi` or `π`, `e`, parentheses, `|x|`, `√x`, `∛x`,
//!   and functions with parentheses: `sin`, `cos`, `tan`, `sec`, `csc`, `cot`, each with an
//!   inverse (`asin` or `arcsin`, …) and hyperbolic forms (`sinh`, `asinh`, …), `sqrt`, `cbrt`,
//!   `root(x, n)` (the nth root), `log` (base 10), `log(b, x)` (base b), `ln`, `exp`, `abs`,
//!   `floor`, `ceil` and `fact`.
//!
//! Angles are in the unit given; names are case-insensitive, and spaces don't matter.

use num_bigint::BigInt;
use num_traits::Signed;

use super::{
    Angle, CalcError, Func, Number, Op, SCIENTIFIC_DIGITS, Trig, TrigFunction, apply, format_with,
    math, normalize,
};

/// The most characters a formula may have: the limit of Windows' paste.
pub const MAX_LENGTH: usize = super::MAX_PASTE_LENGTH;
/// How deep formulas may nest (parentheses, functions, signs), so that no input can
/// overflow the stack.
const MAX_DEPTH: usize = 100;
/// The largest numerator and denominator `Evaluation::fraction` shows: an exact result
/// with bigger ones is shown only as its digits.
const FRACTION_DIGITS: usize = 30;

/// A formula's value.
#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation {
    /// The value itself.
    pub value: Number,
    /// The value as the Scientific mode shows it: 32 significant digits, without digit
    /// groups, in scientific notation when it's very large or small (`1.e+40`).
    pub display: String,
    /// The exact value as a fraction (`30864/175`), when only exact operations led to it
    /// and it isn't a whole number. A function such as `sin` or `√` rounds, so its result
    /// has none, even where it happens to be exact.
    pub fraction: Option<String>,
}

/// Why a formula has no value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpressionError {
    /// The text isn't a formula: what's wrong, and where (a character index).
    Syntax { message: String, position: usize },
    /// The calculator's own error, with Windows' message (`Cannot divide by zero`, …).
    Calc(CalcError),
}

impl ExpressionError {
    pub fn message(&self) -> String {
        match self {
            ExpressionError::Syntax { message, position } => {
                format!("{message} (at character {})", position + 1)
            }
            ExpressionError::Calc(error) => error.message().to_string(),
        }
    }
}

impl From<CalcError> for ExpressionError {
    fn from(error: CalcError) -> Self {
        ExpressionError::Calc(error)
    }
}

/// Evaluates `text` with angles in `angle`.
pub fn evaluate(text: &str, angle: Angle) -> Result<Evaluation, ExpressionError> {
    if text.chars().count() > MAX_LENGTH {
        return Err(syntax(
            format!("The formula is longer than {MAX_LENGTH} characters"),
            MAX_LENGTH,
        ));
    }
    let tokens = tokenize(text)?;
    let mut parser = Parser {
        tokens: &tokens,
        next: 0,
        angle,
        depth: 0,
        end: text.chars().count(),
    };
    let value = parser.sum()?;
    if let Some(token) = parser.peek() {
        return Err(syntax(
            format!("Unexpected {}", token.kind.describe()),
            token.position,
        ));
    }
    let display = format_with(&value.number, false, SCIENTIFIC_DIGITS, false);
    let fraction = (value.exact && !value.number.is_integer())
        .then(|| {
            let (numer, denom) = (value.number.numer(), value.number.denom());
            let short = |n: &BigInt| n.abs().to_string().len() <= FRACTION_DIGITS;
            (short(numer) && short(denom)).then(|| format!("{numer}/{denom}"))
        })
        .flatten();
    Ok(Evaluation {
        value: value.number,
        display,
        fraction,
    })
}

fn syntax(message: impl Into<String>, position: usize) -> ExpressionError {
    ExpressionError::Syntax {
        message: message.into(),
        position,
    }
}

// ------------------------------------------------------------------ tokens

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    /// A number's text, such as `1.5e3`.
    Number(String),
    /// A name: a function, `pi`, `e`, `mod`.
    Name(String),
    Plus,
    Minus,
    Times,
    Divide,
    Modulo,
    Power,
    Factorial,
    Squared,
    Cubed,
    Open,
    Close,
    Comma,
    Bar,
    SquareRoot,
    CubeRoot,
}

impl Kind {
    fn describe(&self) -> String {
        match self {
            Kind::Number(text) => format!("number {text}"),
            Kind::Name(name) => format!("'{name}'"),
            Kind::Plus => "'+'".into(),
            Kind::Minus => "'-'".into(),
            Kind::Times => "'*'".into(),
            Kind::Divide => "'/'".into(),
            Kind::Modulo => "'mod'".into(),
            Kind::Power => "'^'".into(),
            Kind::Factorial => "'!'".into(),
            Kind::Squared => "'²'".into(),
            Kind::Cubed => "'³'".into(),
            Kind::Open => "'('".into(),
            Kind::Close => "')'".into(),
            Kind::Comma => "','".into(),
            Kind::Bar => "'|'".into(),
            Kind::SquareRoot => "'√'".into(),
            Kind::CubeRoot => "'∛'".into(),
        }
    }
}

#[derive(Clone, Debug)]
struct Token {
    kind: Kind,
    /// The index of its first character.
    position: usize,
}

fn tokenize(text: &str) -> Result<Vec<Token>, ExpressionError> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let position = i;
        let simple = match c {
            '+' => Some(Kind::Plus),
            '-' | '−' | '–' => Some(Kind::Minus),
            '*' | '×' | '·' | '⋅' => Some(Kind::Times),
            '/' | '÷' | '∕' => Some(Kind::Divide),
            '%' => Some(Kind::Modulo),
            '^' => Some(Kind::Power),
            '!' => Some(Kind::Factorial),
            '²' => Some(Kind::Squared),
            '³' => Some(Kind::Cubed),
            '(' | '[' => Some(Kind::Open),
            ')' | ']' => Some(Kind::Close),
            ',' | ';' => Some(Kind::Comma),
            '|' => Some(Kind::Bar),
            '√' => Some(Kind::SquareRoot),
            '∛' => Some(Kind::CubeRoot),
            'π' => Some(Kind::Name("pi".into())),
            _ => None,
        };
        if let Some(kind) = simple {
            tokens.push(Token { kind, position });
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let mut end = i;
            while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '.') {
                end += 1;
            }
            // An exponent: `e` or `E`, a sign, digits. Without digits after it, the `e` is
            // Euler's number (`2e` is 2e).
            if end < chars.len() && matches!(chars[end], 'e' | 'E') {
                let mut digits = end + 1;
                if digits < chars.len() && matches!(chars[digits], '+' | '-' | '−') {
                    digits += 1;
                }
                if digits < chars.len() && chars[digits].is_ascii_digit() {
                    end = digits;
                    while end < chars.len() && chars[end].is_ascii_digit() {
                        end += 1;
                    }
                }
            }
            let number: String = chars[i..end].iter().collect();
            tokens.push(Token {
                kind: Kind::Number(number.replace('−', "-")),
                position,
            });
            i = end;
        } else if c.is_alphabetic() {
            let mut end = i;
            while end < chars.len() && chars[end].is_ascii_alphabetic() {
                end += 1;
            }
            if end == i {
                return Err(syntax(format!("Unknown character '{c}'"), position));
            }
            let name: String = chars[i..end].iter().collect();
            tokens.push(Token {
                kind: Kind::Name(name.to_ascii_lowercase()),
                position,
            });
            i = end;
        } else {
            return Err(syntax(format!("Unknown character '{c}'"), position));
        }
    }
    Ok(tokens)
}

// ------------------------------------------------------------------ parsing and evaluation

/// A value, and whether only exact operations led to it.
#[derive(Debug)]
struct Value {
    number: Number,
    exact: bool,
}

impl Value {
    fn exact(number: Number) -> Value {
        Value {
            number,
            exact: true,
        }
    }
}

/// A recursive descent parser that evaluates as it goes.
struct Parser<'a> {
    tokens: &'a [Token],
    next: usize,
    angle: Angle,
    depth: usize,
    /// The formula's length, the position of errors at its end.
    end: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.next)
    }

    fn peek_kind(&self) -> Option<&Kind> {
        self.peek().map(|token| &token.kind)
    }

    fn position(&self) -> usize {
        self.peek().map_or(self.end, |token| token.position)
    }

    fn take(&mut self, kind: &Kind) -> bool {
        if self.peek_kind() == Some(kind) {
            self.next += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: Kind, what: &str) -> Result<(), ExpressionError> {
        if self.take(&kind) {
            Ok(())
        } else {
            let found = self
                .peek()
                .map_or("the end".to_string(), |token| token.kind.describe());
            Err(syntax(
                format!("Expected {what}, found {found}"),
                self.position(),
            ))
        }
    }

    /// Guards the recursion: every nested formula goes one level deeper.
    fn nested<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, ExpressionError>,
    ) -> Result<T, ExpressionError> {
        if self.depth >= MAX_DEPTH {
            return Err(syntax("The formula nests too deeply", self.position()));
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }

    /// `+` and `-`.
    fn sum(&mut self) -> Result<Value, ExpressionError> {
        let mut left = self.product()?;
        loop {
            let op = match self.peek_kind() {
                Some(Kind::Plus) => Op::Add,
                Some(Kind::Minus) => Op::Subtract,
                _ => return Ok(left),
            };
            self.next += 1;
            let right = self.product()?;
            left = binary(left, op, right)?;
        }
    }

    /// `*`, `/`, `mod`, and multiplication without a sign.
    fn product(&mut self) -> Result<Value, ExpressionError> {
        let mut left = self.signed()?;
        loop {
            let op = match self.peek_kind() {
                Some(Kind::Times) => Op::Multiply,
                Some(Kind::Divide) => Op::Divide,
                Some(Kind::Modulo) => Op::Mod,
                Some(Kind::Name(name)) if name == "mod" => Op::Mod,
                // `2pi`, `3(4 + 1)`, `2 sin(30)`, `(1 + 2)(3 + 4)`, `2√3`; not two numbers in a
                // row, which would make `1 000` a thousand times nothing.
                Some(Kind::Name(_) | Kind::Open | Kind::SquareRoot | Kind::CubeRoot) => {
                    let right = self.signed()?;
                    left = binary(left, Op::Multiply, right)?;
                    continue;
                }
                _ => return Ok(left),
            };
            self.next += 1;
            let right = self.signed()?;
            left = binary(left, op, right)?;
        }
    }

    /// A sign before a power: `-2^2` is −(2²).
    fn signed(&mut self) -> Result<Value, ExpressionError> {
        if self.take(&Kind::Minus) {
            let value = self.nested(Self::signed)?;
            return Ok(Value {
                number: -value.number,
                exact: value.exact,
            });
        }
        if self.take(&Kind::Plus) {
            return self.nested(Self::signed);
        }
        self.power()
    }

    /// `^`, right to left, with a signed exponent.
    fn power(&mut self) -> Result<Value, ExpressionError> {
        let base = self.postfix()?;
        if !self.take(&Kind::Power) {
            return Ok(base);
        }
        let exponent = self.nested(Self::signed)?;
        let exact = base.exact && exponent.exact && exponent.number.is_integer();
        let number = apply(&base.number, Op::Power, &exponent.number)?;
        Ok(Value { number, exact })
    }

    /// `!`, `²` and `³` after a value.
    fn postfix(&mut self) -> Result<Value, ExpressionError> {
        let mut value = self.primary()?;
        loop {
            value = match self.peek_kind() {
                Some(Kind::Factorial) => {
                    let exact = value.exact && value.number.is_integer();
                    let mut result = function(Func::Factorial, value, self.angle)?;
                    result.exact = exact;
                    result
                }
                Some(Kind::Squared) => function(Func::Square, value, self.angle)?,
                Some(Kind::Cubed) => function(Func::Cube, value, self.angle)?,
                _ => return Ok(value),
            };
            self.next += 1;
        }
    }

    fn primary(&mut self) -> Result<Value, ExpressionError> {
        let Some(token) = self.peek().cloned() else {
            return Err(syntax("The formula ends too early", self.end));
        };
        self.next += 1;
        match token.kind {
            Kind::Number(text) => number(&text, token.position).map(Value::exact),
            Kind::Open => {
                let value = self.nested(Self::sum)?;
                self.expect(Kind::Close, "')'")?;
                Ok(value)
            }
            Kind::Bar => {
                let value = self.nested(Self::sum)?;
                self.expect(Kind::Bar, "'|'")?;
                function(Func::Abs, value, self.angle)
            }
            Kind::SquareRoot => {
                let value = self.nested(Self::signed)?;
                function(Func::SquareRoot, value, self.angle)
            }
            Kind::CubeRoot => {
                let value = self.nested(Self::signed)?;
                function(Func::CubeRoot, value, self.angle)
            }
            Kind::Name(name) => self.name(&name, token.position),
            kind => Err(syntax(
                format!("Unexpected {}", kind.describe()),
                token.position,
            )),
        }
    }

    /// A constant, or a function and its arguments in parentheses.
    fn name(&mut self, name: &str, position: usize) -> Result<Value, ExpressionError> {
        let constant = match name {
            "pi" => Some(math::pi()),
            "e" => Some(math::euler()),
            _ => None,
        };
        if let Some(number) = constant {
            return Ok(Value {
                number,
                exact: false,
            });
        }
        let arity = match name {
            "root" => 2,
            "log" => 0, // one or two arguments
            _ if unary_function(name).is_some() => 1,
            _ => return Err(syntax(format!("Unknown name '{name}'"), position)),
        };
        if self.peek_kind() != Some(&Kind::Open) {
            return Err(syntax(
                format!("'{name}' needs its argument in parentheses, as in {name}(2)"),
                self.position(),
            ));
        }
        self.next += 1;
        let mut arguments = vec![self.nested(Self::sum)?];
        while self.take(&Kind::Comma) {
            arguments.push(self.nested(Self::sum)?);
        }
        self.expect(Kind::Close, "')'")?;
        let count = arguments.len();
        let wrong =
            |expected: &str| syntax(format!("'{name}' takes {expected}, not {count}"), position);
        match (name, arity, count) {
            ("root", _, 2) => {
                let [x, n] = <[Value; 2]>::try_from(arguments).expect("two");
                let number = apply(&x.number, Op::Root, &n.number)?;
                Ok(Value {
                    number,
                    exact: false,
                })
            }
            ("root", _, _) => Err(wrong("2 arguments, the value and n")),
            ("log", _, 1) => {
                let x = arguments.pop().expect("one");
                function(Func::Log, x, self.angle)
            }
            ("log", _, 2) => {
                // `log(b, x)`, base first, as in the Graphing mode; `logᵧx` is `x LogBase b`.
                let [base, x] = <[Value; 2]>::try_from(arguments).expect("two");
                let number = apply(&x.number, Op::LogBase, &base.number)?;
                Ok(Value {
                    number,
                    exact: false,
                })
            }
            ("log", _, _) => Err(wrong("1 or 2 arguments")),
            (_, 1, 1) => {
                let x = arguments.pop().expect("one");
                let func = unary_function(name).expect("checked above");
                function(func, x, self.angle)
            }
            _ => Err(wrong("1 argument")),
        }
    }
}

/// `left op right`, with Windows' errors.
fn binary(left: Value, op: Op, right: Value) -> Result<Value, ExpressionError> {
    let number = apply(&left.number, op, &right.number)?;
    Ok(Value {
        number,
        exact: left.exact && right.exact,
    })
}

/// A function of the Scientific mode applied to `value`, with its checks: trigonometry
/// refuses 10¹⁰⁰ and more, as Windows' `IsCurrentTooBigForTrig` does. Only `abs`, `floor`,
/// `ceil`, `x²` and `x³` keep a value exact.
fn function(func: Func, value: Value, angle: Angle) -> Result<Value, ExpressionError> {
    if matches!(func, Func::Trig(_)) && math::too_big_for_trig(&value.number) {
        return Err(CalcError::InvalidInput.into());
    }
    let exact = value.exact
        && matches!(
            func,
            Func::Abs | Func::Floor | Func::Ceil | Func::Square | Func::Cube
        );
    let number = func.apply(&value.number, angle)?;
    Ok(Value { number, exact })
}

/// The functions of one argument, by name.
fn unary_function(name: &str) -> Option<Func> {
    use TrigFunction::*;
    let trig = |function, inverse, hyperbolic| {
        Some(Func::Trig(Trig {
            function,
            inverse,
            hyperbolic,
        }))
    };
    let trig_name = |stem: &str| match stem {
        "sin" => Some(Sin),
        "cos" => Some(Cos),
        "tan" => Some(Tan),
        "sec" => Some(Sec),
        "csc" => Some(Csc),
        "cot" => Some(Cot),
        _ => None,
    };
    match name {
        "sqrt" => return Some(Func::SquareRoot),
        "cbrt" => return Some(Func::CubeRoot),
        "ln" => return Some(Func::Ln),
        "exp" => return Some(Func::EToX),
        "abs" => return Some(Func::Abs),
        "floor" => return Some(Func::Floor),
        "ceil" => return Some(Func::Ceil),
        "fact" => return Some(Func::Factorial),
        _ => {}
    }
    // sin, asin, arcsin, sinh, asinh, arcsinh, and the same for the others.
    let (inverse, rest) = if let Some(rest) = name.strip_prefix("arc") {
        (true, rest)
    } else if let Some(rest) = name.strip_prefix('a').filter(|rest| rest.len() >= 3) {
        (true, rest)
    } else {
        (false, name)
    };
    let (hyperbolic, stem) = match rest.strip_suffix('h') {
        Some(stem) if trig_name(stem).is_some() => (true, stem),
        _ => (false, rest),
    };
    trig(trig_name(stem)?, inverse, hyperbolic)
}

/// A number's exact value: `1.5`, `.5`, `1.5e3`, `2E-4`.
/// Beyond the engine's limits (10^±9999), a number is its `Overflow`.
fn number(text: &str, position: usize) -> Result<Number, ExpressionError> {
    let invalid = || syntax(format!("{text} isn't a number"), position);
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(at) => (
            &text[..at],
            text[at + 1..].parse::<i64>().map_err(|_| invalid())?,
        ),
        None => (text, 0),
    };
    if mantissa.matches('.').count() > 1 || mantissa == "." || mantissa.is_empty() {
        return Err(invalid());
    }
    let (whole, decimals) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits: BigInt = format!("{whole}{decimals}")
        .parse()
        .map_err(|_| invalid())?;
    let scale = exponent - decimals.len() as i64;
    if scale.abs() > 2 * MAX_LENGTH as i64 + 10_000 {
        return Err(CalcError::Overflow.into());
    }
    let ten = BigInt::from(10);
    let number = if scale >= 0 {
        Number::from_integer(digits * ten.pow(scale as u32))
    } else {
        Number::new(digits, ten.pow((-scale) as u32))
    };
    Ok(normalize(number)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Calculator, Command};

    fn eval(text: &str) -> String {
        match evaluate(text, Angle::Degrees) {
            Ok(evaluation) => evaluation.display,
            Err(error) => error.message(),
        }
    }

    fn fraction(text: &str) -> Option<String> {
        evaluate(text, Angle::Degrees).unwrap().fraction
    }

    #[test]
    fn arithmetic_is_exact() {
        assert_eq!(eval("1234.56 / 7"), "176.36571428571428571428571428571");
        assert_eq!(fraction("1234.56 / 7").as_deref(), Some("30864/175"));
        assert_eq!(eval("0.1 + 0.2 - 0.3"), "0");
        assert_eq!(eval("1/3*3"), "1");
        assert_eq!(fraction("1/3*3"), None, "a whole number");
        assert_eq!(fraction("2^-1").as_deref(), Some("1/2"));
    }

    #[test]
    fn precedence_and_signs() {
        assert_eq!(eval("2 + 3 * 4"), "14");
        assert_eq!(eval("(2 + 3) * 4"), "20");
        assert_eq!(eval("-2^2"), "-4");
        assert_eq!(eval("(-2)^2"), "4");
        assert_eq!(eval("2^3^2"), "512");
        assert_eq!(eval("2^-2"), "0.25");
        assert_eq!(eval("--3"), "3");
        assert_eq!(eval("10 - 2 - 3"), "5");
        assert_eq!(eval("100 / 10 / 2"), "5");
        assert_eq!(eval("2 × 3 ÷ 4 − 1"), "0.5");
    }

    #[test]
    fn multiplication_without_a_sign() {
        assert_eq!(eval("2(3 + 4)"), "14");
        assert_eq!(eval("(1 + 1)(2 + 3)"), "10");
        assert_eq!(eval("2pi"), eval("2 * pi"));
        assert_eq!(eval("3sqrt(4)"), "6");
        assert_eq!(eval("2√9"), "6");
    }

    #[test]
    fn functions() {
        assert_eq!(eval("sin(30)"), "0.5");
        assert_eq!(
            evaluate("sin(pi/6)", Angle::Radians).unwrap().display,
            "0.5"
        );
        assert_eq!(evaluate("cos(200)", Angle::Gradians).unwrap().display, "-1");
        assert_eq!(eval("asin(1)"), "90");
        assert_eq!(eval("arctan(1)"), "45");
        assert_eq!(eval("sqrt(2)"), "1.4142135623730950488016887242097");
        assert_eq!(eval("√16"), "4");
        assert_eq!(eval("cbrt(27)"), "3");
        assert_eq!(eval("∛-8"), "-2");
        assert_eq!(eval("root(32, 5)"), "2");
        assert_eq!(eval("log(1000)"), "3");
        assert_eq!(eval("log(2, 8)"), "3");
        assert_eq!(eval("ln(e)"), "1");
        assert_eq!(eval("exp(0)"), "1");
        assert_eq!(eval("abs(-3) + |-4|"), "7");
        assert_eq!(eval("floor(2.7) + ceil(2.1)"), "5");
        assert_eq!(eval("5!"), "120");
        assert_eq!(eval("fact(5)"), "120");
        assert_eq!(eval("0.5!"), "0.88622692545275801364908374167057");
        assert_eq!(eval("3² + 2³"), "17");
        assert_eq!(eval("SIN(30)"), "0.5");
        assert_eq!(eval("cosh(0)"), "1");
        assert_eq!(eval("10 mod 3"), "1");
        assert_eq!(eval("-10 % 3"), "2", "the divisor's sign, as in Windows");
    }

    #[test]
    fn windows_errors() {
        assert_eq!(eval("1/0"), "Cannot divide by zero");
        assert_eq!(eval("0/0"), "Result is undefined");
        assert_eq!(eval("tan(90)"), "Invalid input");
        assert_eq!(eval("sqrt(-1)"), "Invalid input");
        assert_eq!(eval("ln(0)"), "Invalid input");
        assert_eq!(eval("3250!"), "Overflow");
        assert_eq!(eval("sin(10^100)"), "Invalid input");
    }

    #[test]
    fn syntax_errors_say_where() {
        fn error(text: &str) -> (String, usize) {
            match evaluate(text, Angle::Degrees) {
                Err(ExpressionError::Syntax { message, position }) => (message, position),
                other => panic!("{text}: {other:?}"),
            }
        }
        assert_eq!(error("2 +").1, 3);
        assert_eq!(error("2 + * 3").1, 4);
        assert_eq!(error("(1 + 2").0, "Expected ')', found the end");
        assert_eq!(error("x + 1").0, "Unknown name 'x'");
        assert_eq!(
            error("sin 30").0,
            "'sin' needs its argument in parentheses, as in sin(2)"
        );
        assert_eq!(
            error("root(8)").0,
            "'root' takes 2 arguments, the value and n, not 1"
        );
        assert_eq!(error("1 # 2").0, "Unknown character '#'");
        assert_eq!(error("1..2").0, "1..2 isn't a number");
        assert_eq!(error("").0, "The formula ends too early");
        assert!(error(&"(".repeat(200)).0.contains("nests too deeply"));
        assert!(error(&"1+".repeat(300)).0.contains("longer than 512"));
    }

    #[test]
    fn exponents_and_e() {
        assert_eq!(eval("1.5e3"), "1500");
        assert_eq!(eval("2E-4"), "0.0002");
        assert_eq!(eval("2e"), eval("2 * e"));
        assert_eq!(eval("1e10000"), "Overflow");
        assert_eq!(eval("10^10000"), "Overflow");
        assert_eq!(eval("10^40"), "1.e+40");
    }

    /// The formulas the keys can paste give the same digits as the Scientific mode itself.
    #[test]
    fn agrees_with_the_scientific_keys() {
        for text in [
            "1234.56/7",
            "2+3*4",
            "(2+3)*4",
            "2^10-1",
            "1/3",
            "0.1+0.2",
            "123456789*987654321",
            "2^0.5",
            "7%3",
            "-5+2",
            "1.5e3/4",
        ] {
            let mut calc = Calculator::with_precedence();
            assert!(calc.paste(text), "{text}");
            calc.execute(Command::Equals);
            let keys = calc.copy_text().unwrap_or_else(|| calc.display());
            assert_eq!(eval(text), keys, "{text}");
        }
    }
}
