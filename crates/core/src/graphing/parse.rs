//! The parser of the equations typed into the graphing mode.
//!
//! Windows' equation boxes are math editors that hand MathML to a proprietary engine;
//! CalcCraft's are plain text, so this reads the linear syntax the keypad writes
//! (`sin()`, `sqrt()`, `root(x, n)`, `log(b, x)`, `^`, `π`, `≤` …) and what people type:
//! implicit multiplication (`2x`, `mx + b`, `(x + 1)(x − 1)`), function names without
//! parentheses (`sin x`, `sin 2x`), `|x|`, `x!`, `²` and the Unicode operators. The errors
//! are the engine's `SyntaxErrorCode`s that the calculator has messages for.

use super::expr::{BinOp, Expr, Func, MAX_DEPTH};

/// How numbers are written: the region's decimal separator, and the separator of a
/// function's arguments (Windows' `LocalizationType`: `,` and `;` with a decimal comma).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Syntax {
    pub decimal: char,
    pub separator: char,
}

impl Default for Syntax {
    fn default() -> Self {
        Syntax {
            decimal: '.',
            separator: ',',
        }
    }
}

impl Syntax {
    /// The syntax for a region whose decimal separator is `decimal`.
    pub fn for_decimal(decimal: char) -> Self {
        if decimal == ',' {
            Syntax {
                decimal: ',',
                separator: ';',
            }
        } else {
            Syntax::default()
        }
    }
}

/// The relation of an equation or inequality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Equal,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl Relation {
    /// The relation with its sides swapped (`2 < y` is `y > 2`).
    pub fn flip(self) -> Self {
        match self {
            Relation::Less => Relation::Greater,
            Relation::LessEqual => Relation::GreaterEqual,
            Relation::Greater => Relation::Less,
            Relation::GreaterEqual => Relation::LessEqual,
            Relation::Equal => Relation::Equal,
        }
    }

    /// Whether `lhs` and `rhs` satisfy the relation.
    pub fn holds(self, lhs: f64, rhs: f64) -> bool {
        match self {
            Relation::Equal => lhs == rhs,
            Relation::Less => lhs < rhs,
            Relation::LessEqual => lhs <= rhs,
            Relation::Greater => lhs > rhs,
            Relation::GreaterEqual => lhs >= rhs,
        }
    }
}

/// Windows' `SyntaxErrorCode`s, each with its message (`key`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxError {
    /// `)` without a `(`.
    ParenthesisMismatch,
    /// `(` without a `)`.
    UnmatchedParenthesis,
    /// `7.3.2`
    TooManyDecimalPoints,
    /// `3 + . + 4`
    DecimalPointWithoutDigits,
    /// `3 − 4 ×`
    UnexpectedEndOfExpression,
    /// `3 − × 4`
    UnexpectedToken,
    /// `[`, `#` …
    InvalidToken,
    /// `x = 3 = y`
    TooManyEquals,
    /// `4 + 83 = 9`
    EqualWithoutGraphVariable,
    /// `x + y`, which has no relation.
    InvalidEquationSyntax,
    EmptyExpression,
    /// `root(x)`
    IncorrectNumParameter,
    /// `i`, which the real number field doesn't allow.
    CannotUseIInReal,
}

impl SyntaxError {
    /// The key of the error's message in Windows' resources.
    pub fn key(self) -> &'static str {
        match self {
            SyntaxError::ParenthesisMismatch => "ParenthesisMismatch",
            SyntaxError::UnmatchedParenthesis => "UnmatchedParenthesis",
            SyntaxError::TooManyDecimalPoints => "TooManyDecimalPoints",
            SyntaxError::DecimalPointWithoutDigits => "DecimalPointWithoutDigits",
            SyntaxError::UnexpectedEndOfExpression => "UnexpectedEndOfExpression",
            SyntaxError::UnexpectedToken => "UnexpectedToken",
            SyntaxError::InvalidToken => "InvalidToken",
            SyntaxError::TooManyEquals => "TooManyEquals",
            SyntaxError::EqualWithoutGraphVariable => "EqualWithoutGraphVariable",
            SyntaxError::InvalidEquationSyntax => "InvalidEquationSyntax",
            SyntaxError::EmptyExpression => "EmptyExpression",
            SyntaxError::IncorrectNumParameter => "IncorrectNumParameter",
            SyntaxError::CannotUseIInReal => "CannotUseIInReal",
        }
    }
}

/// A parsed line: an expression, and the relation and right side if it has one.
#[derive(Clone, Debug, PartialEq)]
pub struct Parsed {
    pub left: Expr,
    pub right: Option<(Relation, Expr)>,
    /// Whether the left and the right side are written `f(x)`, a letter and x in
    /// parentheses: a function's name (`f(x) = x²`), though its expression is the product
    /// f · x, like `fx` or `ax`.
    pub names_function: [bool; 2],
}

/// The most characters a line can have: Windows' equation boxes have a `MaxLength` of
/// 2048. Only pasting gets more into them here, which this keeps from building
/// expressions too large to evaluate.
const MAX_LENGTH: usize = 2048;

/// The most levels of parentheses, bars, functions, signs and exponents inside each
/// other: the parser recurses through several functions for each, on the UI thread's 1 MB
/// stack (2047 `(` overflowed it). The expression is also kept within `MAX_DEPTH`.
const MAX_NESTING: usize = 64;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Func(Func),
    /// `root(x, n)`
    Root,
    /// `log(x)` or `log(b, x)`
    Log,
    Pi,
    E,
    X,
    Y,
    Var(char),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Bang,
    Open,
    Close,
    Bar,
    Separator,
    Rel(Relation),
}

/// The function names, longest first so that `sinh` isn't read as `sin h`.
const NAMES: [(&str, Tok); 50] = [
    ("arcsinh", Tok::Func(Func::Asinh)),
    ("arccosh", Tok::Func(Func::Acosh)),
    ("arctanh", Tok::Func(Func::Atanh)),
    ("arcsech", Tok::Func(Func::Asech)),
    ("arccsch", Tok::Func(Func::Acsch)),
    ("arccoth", Tok::Func(Func::Acoth)),
    ("ceiling", Tok::Func(Func::Ceil)),
    ("arcsin", Tok::Func(Func::Asin)),
    ("arccos", Tok::Func(Func::Acos)),
    ("arctan", Tok::Func(Func::Atan)),
    ("arcsec", Tok::Func(Func::Asec)),
    ("arccsc", Tok::Func(Func::Acsc)),
    ("arccot", Tok::Func(Func::Acot)),
    ("asinh", Tok::Func(Func::Asinh)),
    ("acosh", Tok::Func(Func::Acosh)),
    ("atanh", Tok::Func(Func::Atanh)),
    ("asech", Tok::Func(Func::Asech)),
    ("acsch", Tok::Func(Func::Acsch)),
    ("acoth", Tok::Func(Func::Acoth)),
    ("floor", Tok::Func(Func::Floor)),
    ("sinh", Tok::Func(Func::Sinh)),
    ("cosh", Tok::Func(Func::Cosh)),
    ("tanh", Tok::Func(Func::Tanh)),
    ("sech", Tok::Func(Func::Sech)),
    ("csch", Tok::Func(Func::Csch)),
    ("coth", Tok::Func(Func::Coth)),
    ("asin", Tok::Func(Func::Asin)),
    ("acos", Tok::Func(Func::Acos)),
    ("atan", Tok::Func(Func::Atan)),
    ("asec", Tok::Func(Func::Asec)),
    ("acsc", Tok::Func(Func::Acsc)),
    ("acot", Tok::Func(Func::Acot)),
    ("sqrt", Tok::Func(Func::Sqrt)),
    ("cbrt", Tok::Func(Func::Cbrt)),
    ("ceil", Tok::Func(Func::Ceil)),
    ("sign", Tok::Func(Func::Sign)),
    ("root", Tok::Root),
    ("sin", Tok::Func(Func::Sin)),
    ("cos", Tok::Func(Func::Cos)),
    ("tan", Tok::Func(Func::Tan)),
    ("sec", Tok::Func(Func::Sec)),
    ("csc", Tok::Func(Func::Csc)),
    ("cot", Tok::Func(Func::Cot)),
    ("abs", Tok::Func(Func::Abs)),
    ("exp", Tok::Func(Func::Exp)),
    ("sgn", Tok::Func(Func::Sign)),
    ("log", Tok::Log),
    ("ln", Tok::Func(Func::Ln)),
    ("pi", Tok::Pi),
    ("e", Tok::E),
];

fn tokenize(text: &str, syntax: Syntax) -> Result<Vec<Tok>, SyntaxError> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let is_decimal = |c: char| c == syntax.decimal || c == '.';
        if c.is_ascii_digit() || is_decimal(c) {
            let start = i;
            let mut seen_decimal = false;
            let mut digits = false;
            while i < chars.len() && (chars[i].is_ascii_digit() || is_decimal(chars[i])) {
                if is_decimal(chars[i]) {
                    if seen_decimal {
                        return Err(SyntaxError::TooManyDecimalPoints);
                    }
                    seen_decimal = true;
                } else {
                    digits = true;
                }
                i += 1;
            }
            if !digits {
                return Err(SyntaxError::DecimalPointWithoutDigits);
            }
            let number: String = chars[start..i]
                .iter()
                .map(|&c| if c.is_ascii_digit() { c } else { '.' })
                .collect();
            let value = number
                .parse::<f64>()
                .map_err(|_| SyntaxError::InvalidToken)?;
            tokens.push(Tok::Num(value));
            continue;
        }
        if c.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect::<String>();
            split_word(&word, &mut tokens)?;
            continue;
        }
        i += 1;
        let tok = match c {
            c if c.is_whitespace() => continue,
            '+' => Tok::Plus,
            '-' | '−' | '–' => Tok::Minus,
            '*' | '×' | '·' | '∙' | '⋅' => Tok::Star,
            '/' | '÷' | '∕' => Tok::Slash,
            '^' => Tok::Caret,
            '!' => Tok::Bang,
            '(' => Tok::Open,
            ')' => Tok::Close,
            '|' => Tok::Bar,
            'π' => Tok::Pi,
            '√' => Tok::Func(Func::Sqrt),
            '∛' => Tok::Func(Func::Cbrt),
            '²' | '³' => {
                tokens.push(Tok::Caret);
                Tok::Num(if c == '²' { 2.0 } else { 3.0 })
            }
            '≤' => Tok::Rel(Relation::LessEqual),
            '≥' => Tok::Rel(Relation::GreaterEqual),
            '=' => Tok::Rel(Relation::Equal),
            '<' | '>' => {
                let or_equal = chars.get(i) == Some(&'=');
                if or_equal {
                    i += 1;
                }
                Tok::Rel(match (c, or_equal) {
                    ('<', false) => Relation::Less,
                    ('<', true) => Relation::LessEqual,
                    (_, false) => Relation::Greater,
                    (_, true) => Relation::GreaterEqual,
                })
            }
            c if c == syntax.separator => Tok::Separator,
            _ => return Err(SyntaxError::InvalidToken),
        };
        tokens.push(tok);
    }
    Ok(tokens)
}

/// Splits a run of letters into function names, constants and one-letter variables:
/// `xsinx` is x · sin x, `mx` is m · x.
fn split_word(word: &str, tokens: &mut Vec<Tok>) -> Result<(), SyntaxError> {
    let mut rest = word;
    'outer: while !rest.is_empty() {
        for (name, tok) in &NAMES {
            if rest.starts_with(name) {
                tokens.push(tok.clone());
                rest = &rest[name.len()..];
                continue 'outer;
            }
        }
        let c = rest.chars().next().unwrap_or(' ');
        tokens.push(match c {
            'x' => Tok::X,
            'y' => Tok::Y,
            'i' | 'I' => return Err(SyntaxError::CannotUseIInReal),
            c => Tok::Var(c),
        });
        rest = &rest[c.len_utf8()..];
    }
    Ok(())
}

/// How a run of letters splits, as the parser reads it: the length in characters of
/// each part, and whether it is a function or constant name (shown upright) rather than
/// a one-letter variable (shown in italics).
pub fn word_parts(word: &str) -> Vec<(usize, bool)> {
    let mut parts = Vec::new();
    let mut rest = word;
    'outer: while !rest.is_empty() {
        for (name, tok) in &NAMES {
            if rest.starts_with(name) {
                parts.push((name.chars().count(), !matches!(tok, Tok::E)));
                rest = &rest[name.len()..];
                continue 'outer;
            }
        }
        let c = rest.chars().next().unwrap_or(' ');
        parts.push((1, false));
        rest = &rest[c.len_utf8()..];
    }
    parts
}

struct Parser {
    tokens: Vec<Tok>,
    at: usize,
    /// How many `|…|` are open: a bar then closes one instead of starting another.
    bars: usize,
    /// How many levels deep the parser is (`MAX_NESTING`).
    nesting: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Tok> {
        let tok = self.tokens.get(self.at).cloned();
        self.at += 1;
        tok
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == Some(tok) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// Whether the next token starts an operand, which multiplies what came before.
    fn operand_follows(&self) -> bool {
        match self.peek() {
            Some(
                Tok::Num(_)
                | Tok::Func(_)
                | Tok::Root
                | Tok::Log
                | Tok::Pi
                | Tok::E
                | Tok::X
                | Tok::Y
                | Tok::Var(_)
                | Tok::Open,
            ) => true,
            Some(Tok::Bar) => self.bars == 0,
            _ => false,
        }
    }

    fn sum(&mut self) -> Result<Expr, SyntaxError> {
        let mut e = self.product()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Plus) => BinOp::Add,
                Some(Tok::Minus) => BinOp::Sub,
                _ => return Ok(e),
            };
            self.at += 1;
            let rhs = self.product()?;
            e = Expr::Bin(op, Box::new(e), Box::new(rhs));
        }
    }

    fn product(&mut self) -> Result<Expr, SyntaxError> {
        let mut e = self.unary()?;
        loop {
            let (op, rhs) = match self.peek() {
                Some(Tok::Star) => {
                    self.at += 1;
                    (BinOp::Mul, self.unary()?)
                }
                Some(Tok::Slash) => {
                    self.at += 1;
                    (BinOp::Div, self.unary()?)
                }
                _ if self.operand_follows() => (BinOp::Mul, self.power()?),
                _ => return Ok(e),
            };
            e = Expr::Bin(op, Box::new(e), Box::new(rhs));
        }
    }

    /// A signed power. Every level of nesting passes through here, which counts them.
    fn unary(&mut self) -> Result<Expr, SyntaxError> {
        if self.nesting == MAX_NESTING {
            return Err(SyntaxError::InvalidEquationSyntax);
        }
        self.nesting += 1;
        let e = if self.eat(&Tok::Minus) {
            Expr::Neg(Box::new(self.unary()?))
        } else if self.eat(&Tok::Plus) {
            self.unary()?
        } else {
            self.power()?
        };
        self.nesting -= 1;
        Ok(e)
    }

    /// A power binds tighter than a sign: `-x^2` is −(x²), and `2^-x` is allowed.
    fn power(&mut self) -> Result<Expr, SyntaxError> {
        let base = self.postfix()?;
        if self.eat(&Tok::Caret) {
            let exponent = self.unary()?;
            return Ok(Expr::Bin(BinOp::Pow, Box::new(base), Box::new(exponent)));
        }
        Ok(base)
    }

    fn postfix(&mut self) -> Result<Expr, SyntaxError> {
        let mut e = self.primary()?;
        while self.eat(&Tok::Bang) {
            e = Expr::Factorial(Box::new(e));
        }
        Ok(e)
    }

    fn primary(&mut self) -> Result<Expr, SyntaxError> {
        let Some(tok) = self.next() else {
            return Err(SyntaxError::UnexpectedEndOfExpression);
        };
        Ok(match tok {
            Tok::Num(v) => Expr::Num(v),
            Tok::Pi => Expr::Pi,
            Tok::E => Expr::E,
            Tok::X => Expr::X,
            Tok::Y => Expr::Y,
            Tok::Var(c) => Expr::Var(c),
            Tok::Open => {
                let bars = std::mem::take(&mut self.bars);
                let e = self.sum()?;
                self.bars = bars;
                if !self.eat(&Tok::Close) {
                    return Err(self.unclosed());
                }
                e
            }
            Tok::Bar => {
                self.bars += 1;
                let e = self.sum()?;
                self.bars -= 1;
                if !self.eat(&Tok::Bar) {
                    return Err(self.unclosed());
                }
                Expr::Call(Func::Abs, Box::new(e))
            }
            Tok::Func(f) => {
                let arg = if self.peek() == Some(&Tok::Open) {
                    let mut args = self.arguments()?;
                    if args.len() != 1 {
                        return Err(SyntaxError::IncorrectNumParameter);
                    }
                    args.remove(0)
                } else {
                    self.bare_argument()?
                };
                Expr::Call(f, Box::new(arg))
            }
            Tok::Log => {
                if self.peek() != Some(&Tok::Open) {
                    let arg = self.bare_argument()?;
                    return Ok(Expr::Call(Func::Log, Box::new(arg)));
                }
                let mut args = self.arguments()?;
                match args.len() {
                    1 => Expr::Call(Func::Log, Box::new(args.remove(0))),
                    2 => {
                        let x = args.remove(1);
                        Expr::LogBase(Box::new(args.remove(0)), Box::new(x))
                    }
                    _ => return Err(SyntaxError::IncorrectNumParameter),
                }
            }
            Tok::Root => {
                if self.peek() != Some(&Tok::Open) {
                    return Err(SyntaxError::IncorrectNumParameter);
                }
                let mut args = self.arguments()?;
                if args.len() != 2 {
                    return Err(SyntaxError::IncorrectNumParameter);
                }
                let n = args.remove(1);
                Expr::Root(Box::new(args.remove(0)), Box::new(n))
            }
            Tok::Close => return Err(SyntaxError::ParenthesisMismatch),
            _ => return Err(SyntaxError::UnexpectedToken),
        })
    }

    /// The error for a `(` or `|` that isn't closed where it should be.
    fn unclosed(&self) -> SyntaxError {
        match self.peek() {
            None => SyntaxError::UnmatchedParenthesis,
            Some(_) => SyntaxError::UnexpectedToken,
        }
    }

    /// `(a, b, …)` after a function name.
    fn arguments(&mut self) -> Result<Vec<Expr>, SyntaxError> {
        self.at += 1;
        if self.peek() == Some(&Tok::Close) {
            return Err(SyntaxError::IncorrectNumParameter);
        }
        let bars = std::mem::take(&mut self.bars);
        let mut args = Vec::new();
        loop {
            args.push(self.sum()?);
            match self.next() {
                Some(Tok::Separator) => continue,
                Some(Tok::Close) => break,
                None => return Err(SyntaxError::UnmatchedParenthesis),
                Some(_) => return Err(SyntaxError::UnexpectedToken),
            }
        }
        self.bars = bars;
        Ok(args)
    }

    /// The argument of a function written without parentheses: the factors that
    /// follow without an operator, up to the next function (`sin 2x` is sin(2x), and
    /// `sin x cos x` is sin(x) · cos(x)).
    fn bare_argument(&mut self) -> Result<Expr, SyntaxError> {
        let mut e = self.unary()?;
        while self.operand_follows()
            && !matches!(self.peek(), Some(Tok::Func(_) | Tok::Root | Tok::Log))
        {
            let rhs = self.power()?;
            e = Expr::Bin(BinOp::Mul, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }
}

/// Parses one line of the equation list.
pub fn parse(text: &str, syntax: Syntax) -> Result<Parsed, SyntaxError> {
    // Text too long or too deep to evaluate gets the generic error: Windows has no syntax
    // error for it, and its engine's "too complex" errors aren't syntax errors.
    if text.chars().count() > MAX_LENGTH {
        return Err(SyntaxError::InvalidEquationSyntax);
    }
    let tokens = tokenize(text, syntax)?;
    if tokens.is_empty() {
        return Err(SyntaxError::EmptyExpression);
    }
    let mut parser = Parser {
        tokens,
        at: 0,
        bars: 0,
        nesting: 0,
    };
    let left = parser.sum()?;
    let left_end = parser.at;
    let mut right = None;
    if let Some(Tok::Rel(relation)) = parser.peek().cloned() {
        parser.at += 1;
        let rhs = parser.sum()?;
        right = Some((relation, rhs));
        if let Some(Tok::Rel(second)) = parser.peek() {
            return Err(
                if relation == Relation::Equal || *second == Relation::Equal {
                    SyntaxError::TooManyEquals
                } else {
                    SyntaxError::InvalidEquationSyntax
                },
            );
        }
    }
    match parser.peek() {
        None => {}
        Some(Tok::Close) => return Err(SyntaxError::ParenthesisMismatch),
        Some(_) => return Err(SyntaxError::UnexpectedToken),
    }
    // Long chains (`x + x + …`, `xxx…`) are deep without nesting.
    if left.depth() > MAX_DEPTH || right.as_ref().is_some_and(|(_, e)| e.depth() > MAX_DEPTH) {
        return Err(SyntaxError::InvalidEquationSyntax);
    }
    let names_function =
        |tokens: &[Tok]| matches!(tokens, [Tok::Var(_), Tok::Open, Tok::X, Tok::Close]);
    let tokens = &parser.tokens;
    Ok(Parsed {
        left,
        right,
        names_function: [
            names_function(&tokens[..left_end]),
            names_function(tokens.get(left_end + 1..).unwrap_or_default()),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphing::expr::Env;

    fn value(text: &str, x: f64) -> f64 {
        let parsed = parse(text, Syntax::default()).unwrap();
        assert!(parsed.right.is_none(), "{text}");
        parsed.left.eval(x, 0.0, &Env::RADIANS)
    }

    fn error(text: &str) -> SyntaxError {
        parse(text, Syntax::default()).unwrap_err()
    }

    #[test]
    fn precedence() {
        assert_eq!(value("2+3*4", 0.0), 14.0);
        assert_eq!(value("-x^2", 3.0), -9.0);
        assert_eq!(value("2^3^2", 0.0), 512.0);
        assert_eq!(value("2^-1", 0.0), 0.5);
        assert_eq!(value("10-4-3", 0.0), 3.0);
        assert_eq!(value("12/3/2", 0.0), 2.0);
    }

    #[test]
    fn implicit_multiplication() {
        assert_eq!(value("2x", 3.0), 6.0);
        assert_eq!(value("2x^2", 3.0), 18.0);
        assert_eq!(value("(x+1)(x-1)", 3.0), 8.0);
        assert_eq!(value("3(x)", 2.0), 6.0);
        assert_eq!(value("xx", 4.0), 16.0);
        assert!((value("2π", 0.0) - 2.0 * std::f64::consts::PI).abs() < 1e-15);
        assert!((value("xsin(x)", 2.0) - 2.0 * 2f64.sin()).abs() < 1e-15);
    }

    #[test]
    fn functions() {
        assert!((value("sin(x)", 1.0) - 1f64.sin()).abs() < 1e-15);
        assert!((value("sin x", 1.0) - 1f64.sin()).abs() < 1e-15);
        assert!((value("sin 2x", 1.0) - 2f64.sin()).abs() < 1e-15);
        assert!((value("sinx cosx", 1.0) - 1f64.sin() * 1f64.cos()).abs() < 1e-15);
        assert!((value("sinh(x)", 1.0) - 1f64.sinh()).abs() < 1e-15);
        assert!((value("arcsin(x)", 0.5) - 0.5f64.asin()).abs() < 1e-15);
        assert_eq!(value("sqrt(x)", 9.0), 3.0);
        assert_eq!(value("√x", 16.0), 4.0);
        assert_eq!(value("cbrt(x)", -8.0), -2.0);
        assert_eq!(value("root(x, 3)", -27.0), -3.0);
        assert!((value("log(2, x)", 8.0) - 3.0).abs() < 1e-12);
        assert!((value("log(x)", 1000.0) - 3.0).abs() < 1e-12);
        assert_eq!(value("ln(e)", 0.0), 1.0);
        assert_eq!(value("|x|", -3.0), 3.0);
        assert_eq!(value("|x|·|x - 5|", -1.0), 6.0);
        assert_eq!(value("abs(x)", -2.0), 2.0);
        assert_eq!(value("floor(x)", 2.5), 2.0);
        assert_eq!(value("ceiling(x)", 2.5), 3.0);
        assert!((value("x!", 4.0) - 24.0).abs() < 1e-9);
        assert_eq!(value("x²", 5.0), 25.0);
        assert_eq!(value("10^x", 2.0), 100.0);
        assert_eq!(value("1/x", 4.0), 0.25);
    }

    #[test]
    fn unicode_operators() {
        assert_eq!(value("6 × x ÷ 2 − 1", 2.0), 5.0);
    }

    #[test]
    fn relations() {
        let parsed = parse("y≤x^2", Syntax::default()).unwrap();
        assert_eq!(parsed.left, Expr::Y);
        assert_eq!(parsed.right.unwrap().0, Relation::LessEqual);
        let parsed = parse("x^2+y^2>=25", Syntax::default()).unwrap();
        assert_eq!(parsed.right.unwrap().0, Relation::GreaterEqual);
    }

    #[test]
    fn variables() {
        let parsed = parse("y=mx+b", Syntax::default()).unwrap();
        assert_eq!(parsed.right.unwrap().1.variables(), vec!['b', 'm']);
    }

    #[test]
    fn decimal_comma() {
        let syntax = Syntax::for_decimal(',');
        let parsed = parse("root(x; 2) + 0,5", syntax).unwrap();
        assert_eq!(parsed.left.eval(9.0, 0.0, &Env::RADIANS), 3.5);
    }

    #[test]
    fn errors() {
        assert_eq!(error("x)"), SyntaxError::ParenthesisMismatch);
        assert_eq!(error("(x+1"), SyntaxError::UnmatchedParenthesis);
        assert_eq!(error("sin(x"), SyntaxError::UnmatchedParenthesis);
        assert_eq!(error("7.3.2x"), SyntaxError::TooManyDecimalPoints);
        assert_eq!(error("3+.+4"), SyntaxError::DecimalPointWithoutDigits);
        assert_eq!(error("3-4*"), SyntaxError::UnexpectedEndOfExpression);
        assert_eq!(error("3-*4"), SyntaxError::UnexpectedToken);
        assert_eq!(error("x#2"), SyntaxError::InvalidToken);
        assert_eq!(error("[x]"), SyntaxError::InvalidToken);
        assert_eq!(error("y=x=2"), SyntaxError::TooManyEquals);
        assert_eq!(error("root(x)"), SyntaxError::IncorrectNumParameter);
        assert_eq!(error("sin(x, 2)"), SyntaxError::IncorrectNumParameter);
        assert_eq!(error("y=ix"), SyntaxError::CannotUseIInReal);
        assert_eq!(error("   "), SyntaxError::EmptyExpression);
    }

    #[test]
    fn function_names() {
        let names = |text: &str| parse(text, Syntax::default()).unwrap().names_function;
        assert_eq!(names("f(x)=x^2"), [true, false]);
        assert_eq!(names("x^2 = g (x)"), [false, true]);
        assert_eq!(names("f(x)"), [true, false]);
        // Products, not names.
        assert_eq!(names("ax=3"), [false, false]);
        assert_eq!(names("f·x=3"), [false, false]);
        assert_eq!(names("f(x+1)=3"), [false, false]);
        assert_eq!(names("2f(x)=3"), [false, false]);
    }

    /// Runs `f` on a thread with the 1 MB stack of the UI thread (the main thread on
    /// Windows), where too deep a recursion crashes the app.
    fn on_ui_sized_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(1 << 20)
            .spawn(f)
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn limits() {
        on_ui_sized_stack(|| {
            // Deep nesting is an error instead of a stack overflow.
            assert_eq!(error(&"(".repeat(2047)), SyntaxError::InvalidEquationSyntax);
            let nested = |n: usize| format!("{}x{}", "(".repeat(n), ")".repeat(n));
            assert_eq!(error(&nested(64)), SyntaxError::InvalidEquationSyntax);
            assert_eq!(value(&nested(63), 2.0), 2.0);
            assert_eq!(value("((((x))))", 3.0), 3.0);
            assert_eq!(error(&"√".repeat(300)), SyntaxError::InvalidEquationSyntax);
            assert_eq!(error(&"-".repeat(100)), SyntaxError::InvalidEquationSyntax);
            assert_eq!(value(&format!("{}x", "√".repeat(30)), 1.0), 1.0);
            // So are long chains, which are deep without nesting.
            assert_eq!(error(&"x".repeat(2048)), SyntaxError::InvalidEquationSyntax);
            assert_eq!(
                error(&format!("x{}", "!".repeat(2047))),
                SyntaxError::InvalidEquationSyntax
            );
            assert_eq!(
                error(&format!("{}x", "x+".repeat(100))),
                SyntaxError::InvalidEquationSyntax
            );
            assert_eq!(value(&format!("{}x", "x+".repeat(90)), 1.0), 91.0);
            // Text longer than the box holds (pasted) is too.
            let long = format!("x{}", " ".repeat(2048));
            assert_eq!(error(&long), SyntaxError::InvalidEquationSyntax);
            assert_eq!(value(&long[..2048], 5.0), 5.0);
        });
    }
}
