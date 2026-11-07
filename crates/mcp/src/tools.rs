//! The tools' work, without MCP: each takes its parameters and gives its result, or the
//! message an agent gets as a tool error (with what to change). `server.rs` serves them.
//! Numbers are written with `.` and without digit groups, and dates as ISO 8601
//! (`2026-10-18`), whatever the region.

use calccraft_core::converter::{Category, Converter, Region, Unit};
use calccraft_core::dates;
use calccraft_core::engine::expression;
use calccraft_core::engine::{Angle, Calculator, Command, Func, MAX_DEPTH, Op, Radix, WordSize};
use calccraft_core::graphing::Equation;
use calccraft_core::graphing::analysis::{self, Period};
use calccraft_core::graphing::parse::Syntax;
use chrono::NaiveDate;
use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

use crate::english;

/// The unit of trigonometric angles.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AngleUnit {
    #[default]
    Degrees,
    Radians,
    Gradians,
}

impl From<AngleUnit> for Angle {
    fn from(unit: AngleUnit) -> Angle {
        match unit {
            AngleUnit::Degrees => Angle::Degrees,
            AngleUnit::Radians => Angle::Radians,
            AngleUnit::Gradians => Angle::Gradians,
        }
    }
}

// ------------------------------------------------------------------ calculate

/// Which of the Windows Calculator's modes calculates.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CalcMode {
    /// Operator precedence, parentheses and every function, with 32 significant digits.
    #[default]
    Scientific,
    /// Windows' Standard mode: it calculates as you type, without precedence
    /// (2 + 3 * 4 = 20), with 16 significant digits, and takes only numbers and + - * /.
    Standard,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalculateParams {
    /// The formula, such as `1234.56 / 7`, `sin(30) + 2^10` or `root(27, 3)`.
    #[schemars(length(max = 512))]
    pub expression: String,
    /// The unit of trigonometric angles: degrees (the default, as in Windows), radians or
    /// gradians.
    #[serde(default)]
    pub angle: AngleUnit,
    /// `scientific` (the default) or `standard`.
    #[serde(default)]
    pub mode: CalcMode,
}

#[derive(Debug, PartialEq, Serialize, JsonSchema)]
pub struct CalculateResult {
    /// The value as the calculator shows it: 32 significant digits (16 in the Standard
    /// mode), without digit groups, in scientific notation when very large or small
    /// (`1.e+40`).
    pub result: String,
    /// The exact value as a fraction, when it isn't a whole number and only exact
    /// operations led to it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fraction: Option<String>,
}

pub fn calculate(params: &CalculateParams) -> Result<CalculateResult, String> {
    match params.mode {
        CalcMode::Scientific => expression::evaluate(&params.expression, params.angle.into())
            .map(|evaluation| CalculateResult {
                result: evaluation.display,
                fraction: evaluation.fraction,
            })
            .map_err(|error| error.message()),
        CalcMode::Standard => {
            // A final `=` would repeat the last operation, as the key does.
            let text = params
                .expression
                .trim_end_matches(|c: char| c == '=' || c.is_whitespace())
                .trim_start();
            let mut calc = Calculator::default();
            if !calc.paste(text) {
                // Which of Windows' paste rules it breaks, where that's easy to tell.
                let long_number = text
                    .split(|c: char| !c.is_ascii_digit() && c != '.')
                    .map(|number| (number, number.chars().filter(char::is_ascii_digit).count()))
                    .find(|&(_, digits)| digits > 16);
                let reason = match long_number {
                    Some((number, digits)) => format!(" ({number} has {digits})"),
                    None => String::new(),
                };
                return Err(format!(
                    "Invalid input: the Standard mode takes what Windows' paste takes: numbers \
                     of at most 16 digits{reason}, + - * /, at most 100 numbers and 512 \
                     characters; use the scientific mode for more digits and functions"
                ));
            }
            calc.execute(Command::Equals);
            match (calc.error(), calc.copy_text()) {
                (None, Some(result)) => Ok(CalculateResult {
                    result,
                    fraction: None,
                }),
                (error, _) => Err(error.map_or("Invalid input", |e| e.message()).into()),
            }
        }
    }
}

// ------------------------------------------------------------------ programmer

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RadixName {
    Hex,
    #[default]
    Dec,
    Oct,
    Bin,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WordName {
    #[default]
    Qword,
    Dword,
    Word,
    Byte,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProgrammerParams {
    /// The expression, its numbers written in `radix`: digits, + - * / and % (the
    /// remainder), parentheses, AND OR XOR NAND NOR (or & | ^), NOT (or ~), LSH and RSH (or
    /// << >>, arithmetic shifts), and ROL(x) and ROR(x) (rotations by one bit). For example
    /// `FF AND 3C` in hex, or `-1` in dec.
    #[schemars(length(max = 512))]
    pub expression: String,
    /// The radix of the numbers: hex, dec (the default), oct or bin.
    #[serde(default)]
    pub radix: RadixName,
    /// The word size in two's complement: qword (64 bits, the default), dword, word or byte.
    #[serde(default)]
    pub word_size: WordName,
}

#[derive(Debug, PartialEq, Serialize, JsonSchema)]
pub struct ProgrammerResult {
    pub hex: String,
    /// Signed, as the Programmer mode shows decimal numbers.
    pub dec: String,
    pub oct: String,
    pub bin: String,
    /// The calculator's expression line, as Windows writes it.
    pub expression_line: String,
}

/// A token of a Programmer mode expression.
#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(String),
    Binary(Op),
    /// NOT, ROL, ROR: applied to the operand that follows.
    Prefix(Func),
    Minus,
    Plus,
    Open,
    Close,
}

pub fn programmer(params: &ProgrammerParams) -> Result<ProgrammerResult, String> {
    let (radix, base) = match params.radix {
        RadixName::Hex => (Radix::Hex, 16),
        RadixName::Dec => (Radix::Decimal, 10),
        RadixName::Oct => (Radix::Octal, 8),
        RadixName::Bin => (Radix::Binary, 2),
    };
    let word = match params.word_size {
        WordName::Qword => WordSize::Qword,
        WordName::Dword => WordSize::Dword,
        WordName::Word => WordSize::Word,
        WordName::Byte => WordSize::Byte,
    };
    if params.expression.chars().count() > expression::MAX_LENGTH {
        return Err(format!(
            "The expression is longer than {} characters",
            expression::MAX_LENGTH
        ));
    }
    let tokens = programmer_tokens(&params.expression)?;

    let mut calc = Calculator::programmer();
    calc.execute(Command::WordSize(word));
    calc.execute(Command::Radix(radix));
    // The prefixes waiting for their operand, one list per open parenthesis.
    let mut pending: Vec<Vec<Func>> = vec![Vec::new()];
    let mut operand_expected = true;
    let finish_operand = |calc: &mut Calculator, prefixes: &mut Vec<Func>| {
        for func in prefixes.drain(..).rev() {
            calc.execute(Command::Function(func));
        }
    };
    // The engine ignores a `(` or an operator beyond its 25 levels, as Windows' keys do;
    // here that would calculate with a part of the expression left out.
    let too_deep = || {
        format!(
            "The expression nests more than {MAX_DEPTH} levels of parentheses and pending \
             operations, the Programmer mode's limit: calculate the inner part first"
        )
    };
    for token in tokens {
        let operator = matches!(token, Token::Binary(_) | Token::Minus | Token::Plus);
        if !operand_expected && operator && calc.pending_operations() >= MAX_DEPTH {
            return Err(too_deep());
        }
        match token {
            Token::Number(text) => {
                if !operand_expected {
                    return Err(format!("An operator is missing before {text}"));
                }
                let prefixes = pending.last_mut().expect("a level");
                // In decimal, the sign right before the number is typed with it.
                let negative = base == 10 && prefixes.last() == Some(&Func::Negate);
                if negative {
                    prefixes.pop();
                }
                type_number(&mut calc, &text, base, word, params.radix, negative)?;
                finish_operand(&mut calc, pending.last_mut().expect("a level"));
                operand_expected = false;
            }
            Token::Prefix(func) if operand_expected => {
                pending.last_mut().expect("a level").push(func);
            }
            Token::Minus if operand_expected => {
                pending.last_mut().expect("a level").push(Func::Negate);
            }
            Token::Plus if operand_expected => {}
            Token::Open if operand_expected => {
                if calc.open_parentheses() >= MAX_DEPTH || calc.pending_operations() >= MAX_DEPTH {
                    return Err(too_deep());
                }
                calc.execute(Command::OpenParen);
                pending.push(Vec::new());
            }
            Token::Close if !operand_expected && pending.len() > 1 => {
                pending.pop();
                calc.execute(Command::CloseParen);
                finish_operand(&mut calc, pending.last_mut().expect("a level"));
            }
            Token::Binary(op) if !operand_expected => {
                calc.execute(Command::Operator(op));
                operand_expected = true;
            }
            Token::Minus if !operand_expected => {
                calc.execute(Command::Operator(Op::Subtract));
                operand_expected = true;
            }
            Token::Plus if !operand_expected => {
                calc.execute(Command::Operator(Op::Add));
                operand_expected = true;
            }
            Token::Close if pending.len() == 1 => return Err("A ')' has no '('".into()),
            _ if operand_expected => {
                return Err("A number is missing (before an operator or a ')')".into());
            }
            _ => return Err("An operator is missing (before a '(' or NOT)".into()),
        }
    }
    if operand_expected {
        return Err("The expression ends without its last number".into());
    }
    if pending.len() > 1 {
        return Err("A '(' has no ')'".into());
    }
    calc.execute(Command::Equals);
    if let Some(error) = calc.error() {
        return Err(error.message().into());
    }
    let [hex, dec, oct, bin] = calc
        .radix_values()
        .expect("the Programmer mode")
        .map(|value| value.replace([' ', ','], ""));
    Ok(ProgrammerResult {
        hex,
        dec,
        oct,
        bin,
        expression_line: calc.expression(),
    })
}

fn programmer_tokens(text: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
        let token = match c {
            _ if c.is_whitespace() => {
                i += 1;
                continue;
            }
            _ if two == "<<" || two == ">>" => {
                i += 2;
                tokens.push(Token::Binary(if two == "<<" {
                    Op::LeftShift
                } else {
                    Op::RightShift
                }));
                continue;
            }
            '+' => Token::Plus,
            '-' | '−' => Token::Minus,
            '*' | '×' => Token::Binary(Op::Multiply),
            '/' | '÷' => Token::Binary(Op::Divide),
            '%' => Token::Binary(Op::Mod),
            '&' => Token::Binary(Op::And),
            '|' => Token::Binary(Op::Or),
            '^' => Token::Binary(Op::Xor),
            '~' => Token::Prefix(Func::Not),
            '(' => Token::Open,
            ')' => Token::Close,
            _ if c.is_ascii_alphanumeric() => {
                let mut end = i;
                while end < chars.len() && chars[end].is_ascii_alphanumeric() {
                    end += 1;
                }
                let word: String = chars[i..end].iter().collect();
                i = end;
                tokens.push(match word.to_ascii_uppercase().as_str() {
                    "AND" => Token::Binary(Op::And),
                    "OR" => Token::Binary(Op::Or),
                    "XOR" => Token::Binary(Op::Xor),
                    "NAND" => Token::Binary(Op::Nand),
                    "NOR" => Token::Binary(Op::Nor),
                    "MOD" => Token::Binary(Op::Mod),
                    "LSH" => Token::Binary(Op::LeftShift),
                    "RSH" => Token::Binary(Op::RightShift),
                    "NOT" => Token::Prefix(Func::Not),
                    "ROL" => Token::Prefix(Func::RotateLeft),
                    "ROR" => Token::Prefix(Func::RotateRight),
                    _ => Token::Number(word),
                });
                continue;
            }
            _ => return Err(format!("Unknown character '{c}'")),
        };
        tokens.push(token);
        i += 1;
    }
    Ok(tokens)
}

/// Types a number key by key, after checking that it's a number of the radix that fits
/// the word: the engine leaves out digits that don't fit, as Windows' keys do, which would
/// silently calculate with another number. A `negative` number (decimal only) is typed
/// with its sign, as Windows' ± does while typing, so that −128 fits into a byte.
fn type_number(
    calc: &mut Calculator,
    text: &str,
    base: u32,
    word: WordSize,
    radix: RadixName,
    negative: bool,
) -> Result<(), String> {
    let radix_name = format!("{radix:?}").to_uppercase();
    let mut digits = Vec::new();
    for c in text.chars() {
        let digit = c
            .to_digit(base)
            .ok_or_else(|| format!("'{c}' isn't a {radix_name} digit (in {text})"))?;
        digits.push(digit as u8);
    }
    // Leading zeros change nothing (the keys ignore them too), but they'd count as digits.
    let first = digits
        .iter()
        .position(|&d| d != 0)
        .unwrap_or(digits.len() - 1);
    let digits = &digits[first..];

    // The largest magnitude: decimal numbers are signed in the Programmer mode, one more
    // for a negative one; the other radixes show the bits.
    let bits = word.bits();
    let max: u128 = match (base, negative) {
        (10, false) => (1 << (bits - 1)) - 1,
        (10, true) => 1 << (bits - 1),
        _ => (1 << bits) - 1,
    };
    let doesnt_fit = || {
        let word = word.label();
        if base == 10 {
            let sign = if negative { "-" } else { "" };
            let (min, max) = ((1u128 << (bits - 1)), (1u128 << (bits - 1)) - 1);
            format!("{sign}{text} doesn't fit into a {word}: in DEC it takes -{min} to {max}")
        } else {
            let max = (1u128 << bits) - 1;
            let in_radix = match base {
                16 => format!("{max:X}"),
                8 => format!("{max:o}"),
                _ => format!("{max:b}"),
            };
            format!(
                "{text} doesn't fit into a {word}: in {radix_name} it takes at most {in_radix} \
                 ({max:X} in hex, {max} in dec)"
            )
        }
    };
    // No more digits than the largest value has, so that the value below fits a u128
    // (64 bits take at most 64 digits, in binary) before it's compared with the word.
    let mut limit_digits = 0;
    let mut rest = max;
    while rest > 0 {
        limit_digits += 1;
        rest /= u128::from(base);
    }
    if digits.len() > limit_digits.max(1) {
        return Err(doesnt_fit());
    }
    let value = digits.iter().try_fold(0u128, |value, &digit| {
        value
            .checked_mul(u128::from(base))?
            .checked_add(u128::from(digit))
    });
    if value.is_none_or(|value| value > max) {
        return Err(doesnt_fit());
    }

    let (head, tail) = digits.split_at(1);
    calc.execute(Command::Digit(head[0]));
    if negative && value != Some(0) {
        calc.execute(Command::Function(Func::Negate));
    }
    for &digit in tail {
        calc.execute(Command::Digit(digit));
    }
    Ok(())
}

// ------------------------------------------------------------------ convert_units

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConvertParams {
    /// The kind of unit: volume, length, weight (or mass), temperature, energy, area,
    /// speed, time, power, data, pressure or angle.
    pub category: String,
    /// The value to convert, such as `5` or `-40` (negative only for temperature, power
    /// and angle).
    pub value: String,
    /// The unit to convert from: its English name (`kilometers`, `kilometer`) or
    /// abbreviation (`km`), case-sensitive where it matters (`GB` gigabytes, `Gb`
    /// gigabits).
    pub from: String,
    /// The unit to convert to, written the same way.
    pub to: String,
}

#[derive(Debug, PartialEq, Serialize, JsonSchema)]
pub struct ConvertResult {
    /// The converted value, rounded as Windows rounds (at least 7 significant digits).
    pub value: String,
    pub from: String,
    pub to: String,
    /// The value in other units and an everyday comparison, as the converter's "About
    /// equal to" shows them.
    pub about_equal_to: Vec<String>,
}

const CATEGORIES: [(Category, &[&str]); 12] = [
    (Category::Volume, &["volume"]),
    (Category::Length, &["length", "distance"]),
    (Category::Weight, &["weight", "mass", "weight and mass"]),
    (Category::Temperature, &["temperature"]),
    (Category::Energy, &["energy"]),
    (Category::Area, &["area"]),
    (Category::Speed, &["speed", "velocity"]),
    (Category::Time, &["time", "duration"]),
    (Category::Power, &["power"]),
    (Category::Data, &["data"]),
    (Category::Pressure, &["pressure"]),
    (Category::Angle, &["angle"]),
];

pub fn convert_units(params: &ConvertParams) -> Result<ConvertResult, String> {
    let wanted = params.category.trim().to_lowercase();
    let category = CATEGORIES
        .iter()
        .find(|(_, names)| names.contains(&wanted.as_str()))
        .map(|(category, _)| *category)
        .ok_or_else(|| {
            format!(
                "Unknown category '{}': use one of volume, length, weight, temperature, \
                 energy, area, speed, time, power, data, pressure, angle",
                params.category
            )
        })?;
    let value = params.value.trim();
    if value.starts_with(['-', '−']) && !category.supports_negative() {
        return Err(format!(
            "{} values can't be negative (only temperature, power and angle can)",
            category_name(category)
        ));
    }

    let mut converter = Converter::new(Region::new("US"));
    converter.set_category(category);
    let units = converter.units();
    let (from, to) = (
        find_unit(category, units, &params.from)?,
        find_unit(category, units, &params.to)?,
    );
    converter.set_unit1(from);
    converter.set_unit2(to);
    converter.activate(true);
    if !converter.paste(value) || converter.has_paste_error() {
        return Err(format!(
            "Invalid input: '{value}' isn't a plain number of at most 15 digits, such as 5, \
             2.5 or -40"
        ));
    }
    let plain = |text: String| text.replace(',', "");
    Ok(ConvertResult {
        value: plain(converter.value2()),
        from: units[from].name.to_string(),
        to: units[to].name.to_string(),
        about_equal_to: converter
            .suggestions()
            .into_iter()
            .map(|suggestion| format!("{} {}", plain(suggestion.value), suggestion.abbreviation))
            .collect(),
    })
}

fn category_name(category: Category) -> &'static str {
    CATEGORIES
        .iter()
        .find(|(c, _)| *c == category)
        .map_or("these", |(_, names)| names[0])
}

/// Other names for the converters' units, by the unit's key: common forms that aren't the
/// converters' own names and abbreviations. Only units the converters have; a test checks
/// that every key exists. The case matters as for the abbreviations (`kB` is a kilobyte,
/// `Kib` a kibibit); without it, a name has to be unambiguous.
const ALIASES: &[(Category, &str, &str)] = &[
    // Volume: the US units, as the converter's US region defaults to; the UK ones go by
    // their own names (`gal (UK)`, `Gallons (UK)`) or these.
    (Category::Volume, "gallon", "GallonUS"),
    (Category::Volume, "gal", "GallonUS"),
    (Category::Volume, "us gallon", "GallonUS"),
    (Category::Volume, "imperial gallon", "GallonUK"),
    (Category::Volume, "uk gallon", "GallonUK"),
    (Category::Volume, "cup", "CupUS"),
    (Category::Volume, "pint", "PintUS"),
    (Category::Volume, "pt", "PintUS"),
    (Category::Volume, "imperial pint", "PintUK"),
    (Category::Volume, "quart", "QuartUS"),
    (Category::Volume, "qt", "QuartUS"),
    (Category::Volume, "fluid ounce", "FluidOunceUS"),
    (Category::Volume, "fl oz", "FluidOunceUS"),
    (Category::Volume, "teaspoon", "TeaspoonUS"),
    (Category::Volume, "tsp", "TeaspoonUS"),
    (Category::Volume, "tablespoon", "TablespoonUS"),
    (Category::Volume, "tbsp", "TablespoonUS"),
    (Category::Volume, "l", "Liter"),
    (Category::Volume, "ltr", "Liter"),
    (Category::Volume, "cc", "CubicCentimeter"),
    // Length.
    (Category::Length, "um", "Micron"),
    (Category::Length, "micrometer", "Micron"),
    (Category::Length, "Å", "Angstrom"),
    // Weight.
    (Category::Weight, "lbs", "Pound"),
    (Category::Weight, "ct", "Carat"),
    (Category::Weight, "kilo", "Kilogram"),
    (Category::Weight, "tonne", "Tonne"),
    (Category::Weight, "metric ton", "Tonne"),
    (Category::Weight, "short ton", "ShortTon"),
    (Category::Weight, "us ton", "ShortTon"),
    (Category::Weight, "long ton", "LongTon"),
    (Category::Weight, "uk ton", "LongTon"),
    (Category::Weight, "imperial ton", "LongTon"),
    // Temperature.
    (Category::Temperature, "degree celsius", "DegreesCelsius"),
    (Category::Temperature, "centigrade", "DegreesCelsius"),
    (
        Category::Temperature,
        "degree fahrenheit",
        "DegreesFahrenheit",
    ),
    // Energy.
    (Category::Energy, "electronvolt", "Electron-Volt"),
    (Category::Energy, "ft lb", "Foot-Pound"),
    (Category::Energy, "ft-lb", "Foot-Pound"),
    (Category::Energy, "kilowatt hour", "Kilowatthour"),
    // Speed.
    (Category::Speed, "kph", "KilometersPerHour"),
    (Category::Speed, "kmh", "KilometersPerHour"),
    (Category::Speed, "km/hr", "KilometersPerHour"),
    (Category::Speed, "kilometer per hour", "KilometersPerHour"),
    (Category::Speed, "mi/h", "MilesPerHour"),
    (Category::Speed, "mile per hour", "MilesPerHour"),
    (Category::Speed, "m/sec", "MetersPerSecond"),
    (Category::Speed, "meter per second", "MetersPerSecond"),
    (Category::Speed, "fps", "FeetPerSecond"),
    (Category::Speed, "ft/sec", "FeetPerSecond"),
    (Category::Speed, "foot per second", "FeetPerSecond"),
    (
        Category::Speed,
        "centimeter per second",
        "CentimetersPerSecond",
    ),
    (Category::Speed, "kt", "Knot"),
    // Time.
    (Category::Time, "sec", "Second"),
    (Category::Time, "h", "Hour"),
    (Category::Time, "hrs", "Hour"),
    (Category::Time, "mins", "Minute"),
    (Category::Time, "us", "Microsecond"),
    (Category::Time, "msec", "Millisecond"),
    (Category::Time, "wks", "Week"),
    (Category::Time, "yrs", "Year"),
    // Power.
    (Category::Power, "hp", "Horsepower"),
    (Category::Power, "horsepower", "Horsepower"),
    (Category::Power, "ft lb/min", "Foot-PoundPerMinute"),
    (Category::Power, "ft-lb/min", "Foot-PoundPerMinute"),
    (
        Category::Power,
        "foot-pound per minute",
        "Foot-PoundPerMinute",
    ),
    (Category::Power, "btu per minute", "BTUPerMinute"),
    (Category::Power, "btu/minute", "BTUPerMinute"),
    // Data: the SI forms and bit abbreviations that the converter writes otherwise.
    (Category::Data, "bit", "Bit"),
    (Category::Data, "kB", "Kilobyte"),
    (Category::Data, "kbit", "Kilobit"),
    (Category::Data, "Kibit", "Kibibits"),
    (Category::Data, "Kib", "Kibibits"),
    (Category::Data, "Mbit", "Megabit"),
    (Category::Data, "Mibit", "Mebibits"),
    (Category::Data, "Mib", "Mebibits"),
    (Category::Data, "Gbit", "Gigabit"),
    (Category::Data, "Gibit", "Gibibits"),
    (Category::Data, "Gib", "Gibibits"),
    (Category::Data, "Tbit", "Terabit"),
    (Category::Data, "Tibit", "Tebibits"),
    (Category::Data, "Tib", "Tebibits"),
    (Category::Data, "Pbit", "Petabit"),
    (Category::Data, "Pibit", "Pebibits"),
    (Category::Data, "Pib", "Pebibits"),
    (Category::Data, "Ebit", "Exabits"),
    (Category::Data, "Eb", "Exabits"),
    (Category::Data, "Eibit", "Exbibits"),
    (Category::Data, "Eib", "Exbibits"),
    (Category::Data, "Zbit", "Zetabits"),
    (Category::Data, "Zb", "Zetabits"),
    (Category::Data, "zettabit", "Zetabits"),
    (Category::Data, "zettabyte", "Zetabytes"),
    (Category::Data, "Zibit", "Zebibits"),
    (Category::Data, "Zib", "Zebibits"),
    (Category::Data, "Ybit", "Yottabit"),
    (Category::Data, "Yb", "Yottabit"),
    (Category::Data, "Yibit", "Yobibits"),
    (Category::Data, "Yib", "Yobibits"),
    (Category::Data, "nybble", "Nibble"),
    // Pressure.
    (Category::Pressure, "bar", "Bar"),
    (Category::Pressure, "pound per square inch", "PSI"),
    // Angle.
    (Category::Angle, "°", "Degree"),
    (Category::Angle, "gon", "Gradian"),
];

/// A unit name as it's compared: British spellings as American ones (`metre`, `litre`),
/// `m2`, `m^2` and `sq m` as `m²` (and the same with 3 and `cu`), Greek μ as the micro sign
/// the converters write, and `°C` as `C`.
fn spelled(name: &str) -> String {
    let mut name = name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("metre", "meter")
        .replace("Metre", "Meter")
        .replace("litre", "liter")
        .replace("Litre", "Liter")
        .replace('μ', "µ")
        .replace("^2", "²")
        .replace("^3", "³");
    for (prefix, power) in [
        ("sq ", '²'),
        ("square ", '²'),
        ("cu ", '³'),
        ("cubic ", '³'),
    ] {
        if let Some(rest) = name.strip_prefix(prefix)
            && rest.len() <= 3
        {
            name = format!("{rest}{power}");
        }
    }
    if let Some(stripped) = name.strip_suffix('2').or(name.strip_suffix('3'))
        && stripped.ends_with(|c: char| c.is_alphabetic())
    {
        let power = if name.ends_with('2') { '²' } else { '³' };
        name = format!("{stripped}{power}");
    }
    match name.strip_prefix('°') {
        Some(rest) if !rest.is_empty() => rest.to_string(),
        _ => name,
    }
}

/// The form compared without case: lower case, without spaces, dots, hyphens and
/// parentheses (`fl oz (US)` is `flozus`, `Foot-pounds` is `footpounds`).
fn loose(name: &str) -> String {
    spelled(name)
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '.' | '-' | '_' | '(' | ')'))
        .collect()
}

/// Every text that names `units[i]`: its name, its name without the plural s, its key, its
/// abbreviation and its aliases.
fn unit_names(category: Category, units: &[Unit]) -> Vec<(String, usize)> {
    let mut names = Vec::new();
    for (i, unit) in units.iter().enumerate() {
        names.push((spelled(unit.name), i));
        if let Some(singular) = unit.name.strip_suffix('s') {
            names.push((spelled(singular), i));
        }
        names.push((spelled(unit.key), i));
        names.push((spelled(unit.abbreviation), i));
        for (_, alias, _) in ALIASES
            .iter()
            .filter(|(c, _, key)| *c == category && *key == unit.key)
        {
            names.push((spelled(alias), i));
        }
    }
    names
}

fn describe(unit: &Unit) -> String {
    format!("{} ({})", unit.name, unit.abbreviation)
}

/// Finds a unit by name, abbreviation or alias. The case decides first, as in the
/// converter's own abbreviations (`GB` is gigabytes, `Gb` gigabits); a name that only
/// matches without case has to name one unit, or the agent gets the candidates.
fn find_unit(category: Category, units: &[Unit], name: &str) -> Result<usize, String> {
    let names = unit_names(category, units);
    let matching = |matches: &dyn Fn(&str) -> bool| {
        let mut found: Vec<usize> = names
            .iter()
            .filter(|(text, _)| matches(text))
            .map(|&(_, i)| i)
            .collect();
        found.sort_unstable();
        found.dedup();
        found
    };
    let wanted = spelled(name);
    let wanted_loose = loose(name);
    let singular = wanted_loose
        .strip_suffix('s')
        .filter(|s| s.len() >= 3)
        .map(str::to_string);
    let steps: [&dyn Fn(&str) -> bool; 2] = [&|text| text == wanted, &|text| {
        let text = loose(text);
        text == wanted_loose || Some(&text) == singular.as_ref()
    }];
    for step in steps {
        match matching(step)[..] {
            [] => continue,
            [i] => return Ok(i),
            ref several => {
                let candidates: Vec<String> =
                    several.iter().map(|&i| describe(&units[i])).collect();
                return Err(format!(
                    "The unit '{}' is ambiguous for {}: it could be {}. Write the \
                     abbreviation with its case (GB is gigabytes, Gb gigabits) or the full name",
                    name.trim(),
                    category_name(category),
                    candidates.join(" or ")
                ));
            }
        }
    }

    // Close matches: names that contain what was written or that it contains, then those
    // within a few typos.
    let mut scored: Vec<(usize, usize)> = Vec::new();
    for (text, i) in &names {
        let text = loose(text);
        let distance = if wanted_loose.len() >= 3
            && text.len() >= 3
            && (text.contains(&wanted_loose) || wanted_loose.contains(&text))
        {
            0
        } else {
            edit_distance(&text, &wanted_loose)
        };
        if distance <= 2.max(wanted_loose.chars().count() / 3) {
            scored.push((distance, *i));
        }
    }
    scored.sort_unstable();
    let mut close: Vec<usize> = Vec::new();
    for (_, i) in scored {
        if !close.contains(&i) && close.len() < 5 {
            close.push(i);
        }
    }
    let all: Vec<String> = units.iter().map(describe).collect();
    let close = if close.is_empty() {
        String::new()
    } else {
        let close: Vec<String> = close.iter().map(|&i| describe(&units[i])).collect();
        format!(" Close matches: {}.", close.join(", "))
    };
    Err(format!(
        "Unknown unit '{}' for {}.{close} All {} units: {}",
        name.trim(),
        category_name(category),
        category_name(category),
        all.join(", ")
    ))
}

/// The Levenshtein distance, in characters.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let substituted = diagonal + usize::from(ca != cb);
            diagonal = row[j + 1];
            row[j + 1] = substituted.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[b.len()]
}

// ------------------------------------------------------------------ dates

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DateDifferenceParams {
    /// The first date, as YYYY-MM-DD (1601-01-01 to 9999-12-31).
    pub from: String,
    /// The second date, as YYYY-MM-DD; it may come before the first.
    pub to: String,
}

#[derive(Debug, PartialEq, Serialize, JsonSchema)]
pub struct DateDifferenceResult {
    pub years: u32,
    pub months: u32,
    pub weeks: u32,
    pub days: u32,
    pub total_days: u32,
    /// As Windows writes it, such as `1 year, 2 months, 3 days`.
    pub description: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DateAddParams {
    /// The date, as YYYY-MM-DD.
    pub date: String,
    /// Years to add (0–999).
    #[serde(default)]
    #[schemars(range(max = 999))]
    pub years: u32,
    /// Months to add (0–999).
    #[serde(default)]
    #[schemars(range(max = 999))]
    pub months: u32,
    /// Days to add (0–999).
    #[serde(default)]
    #[schemars(range(max = 999))]
    pub days: u32,
    /// Subtract instead of adding.
    #[serde(default)]
    pub subtract: bool,
}

#[derive(Debug, PartialEq, Serialize, JsonSchema)]
pub struct DateAddResult {
    /// The result, as YYYY-MM-DD.
    pub date: String,
    /// The result as Windows writes it, such as `Sunday, October 18, 2026`.
    pub long_date: String,
}

fn date(text: &str) -> Result<NaiveDate, String> {
    let date = NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d")
        .map_err(|_| format!("'{text}' isn't a date: write it as YYYY-MM-DD"))?;
    if !(dates::MIN_DATE..=dates::MAX_DATE).contains(&date) {
        return Err(format!(
            "{text} is outside 1601-01-01 to 9999-12-31, the dates Windows supports"
        ));
    }
    Ok(date)
}

pub fn date_difference(params: &DateDifferenceParams) -> Result<DateDifferenceResult, String> {
    let (from, to) = (date(&params.from)?, date(&params.to)?);
    let difference = dates::difference(from, to);
    let (description, _) = dates::describe_difference(from, to, &dates::ENGLISH);
    Ok(DateDifferenceResult {
        years: difference.years,
        months: difference.months,
        weeks: difference.weeks,
        days: difference.days,
        total_days: dates::days_between(from, to),
        description,
    })
}

pub fn date_add(params: &DateAddParams) -> Result<DateAddResult, String> {
    let start = date(&params.date)?;
    if params.years > 999 || params.months > 999 || params.days > 999 {
        return Err("Years, months and days go from 0 to 999 each, as in Windows".into());
    }
    let (years, months, days) = (params.years, params.months, params.days);
    let result = if params.subtract {
        dates::subtract(start, years, months, days)
    } else {
        dates::add(start, years, months, days)
    };
    let result = result.ok_or_else(|| english::text("Date_OutOfBoundMessage").to_string())?;
    Ok(DateAddResult {
        date: result.format("%Y-%m-%d").to_string(),
        long_date: dates::format_long_date(result),
    })
}

// ------------------------------------------------------------------ analyze_function

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalyzeParams {
    /// A function of x as the Graphing mode takes it, such as `y = 1/x`, `x^2 - 4x + 3` or
    /// `sin(x)`.
    #[schemars(length(max = 2048))]
    pub equation: String,
    /// The unit of trigonometric angles: degrees (the default), radians or gradians.
    #[serde(default)]
    pub angle: AngleUnit,
}

#[derive(Debug, PartialEq, Serialize, JsonSchema)]
pub struct Monotonicity {
    /// The interval, such as `(−∞, 0)`.
    pub interval: String,
    /// Increasing, Decreasing or Constant.
    pub direction: String,
}

/// The function's key features, as the Graphing mode's analysis shows them: exact values
/// (fractions, multiples of π, square roots) where it finds them, else 6 significant
/// digits. An empty list or a missing value means the function has none (or that it
/// couldn't be calculated, for the domain and range).
#[derive(Debug, PartialEq, Serialize, JsonSchema)]
pub struct AnalyzeResult {
    pub domain: Option<String>,
    pub range: Option<String>,
    pub x_intercepts: Option<String>,
    pub y_intercept: Option<String>,
    pub minima: Vec<String>,
    pub maxima: Vec<String>,
    pub inflection_points: Vec<String>,
    pub vertical_asymptotes: Vec<String>,
    pub horizontal_asymptotes: Vec<String>,
    pub oblique_asymptotes: Vec<String>,
    /// Whether the function is even, odd or neither.
    pub parity: String,
    /// The period of a periodic function.
    pub period: Option<String>,
    pub monotonicity: Vec<Monotonicity>,
    /// The features that are too complex to calculate.
    pub too_complex: Vec<String>,
}

pub fn analyze_function(params: &AnalyzeParams) -> Result<AnalyzeResult, String> {
    if params.equation.chars().count() > 2048 {
        return Err("The equation is longer than 2048 characters".into());
    }
    let equation = Equation::parse(&params.equation, Syntax::default())
        .map_err(|error| english::text(error.key()).to_string())?;
    let features = analysis::analyze(&equation, params.angle.into())
        .map_err(|error| english::text(error.key()).to_string())?;
    Ok(AnalyzeResult {
        domain: features.domain,
        range: features.range,
        x_intercepts: features.x_intercepts,
        y_intercept: features.y_intercept,
        minima: features.minima,
        maxima: features.maxima,
        inflection_points: features.inflection_points,
        vertical_asymptotes: features.vertical_asymptotes,
        horizontal_asymptotes: features.horizontal_asymptotes,
        oblique_asymptotes: features.oblique_asymptotes,
        parity: english::text(features.parity.key()).to_string(),
        period: match features.period {
            Period::None => None,
            Period::Periodic(period) => Some(period),
        },
        monotonicity: features
            .monotonicity
            .into_iter()
            .map(|(interval, direction)| Monotonicity {
                interval,
                direction: english::text(direction.key()).to_string(),
            })
            .collect(),
        too_complex: features
            .too_complex
            .into_iter()
            .map(|feature| english::text(feature.key()).to_string())
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calc(expression: &str) -> Result<CalculateResult, String> {
        calculate(&CalculateParams {
            expression: expression.into(),
            angle: AngleUnit::Degrees,
            mode: CalcMode::Scientific,
        })
    }

    #[test]
    fn calculates_exactly() {
        let result = calc("1234.56 / 7").unwrap();
        assert_eq!(result.result, "176.36571428571428571428571428571");
        assert_eq!(result.fraction.as_deref(), Some("30864/175"));
        assert_eq!(calc("sin(30)").unwrap().result, "0.5");
        assert_eq!(calc("1/0").unwrap_err(), "Cannot divide by zero");
        assert!(calc("2 +").unwrap_err().contains("at character 4"));
    }

    #[test]
    fn standard_mode_calculates_as_you_type() {
        let standard = |expression: &str| {
            calculate(&CalculateParams {
                expression: expression.into(),
                angle: AngleUnit::Degrees,
                mode: CalcMode::Standard,
            })
        };
        assert_eq!(standard("2 + 3 * 4").unwrap().result, "20");
        assert_eq!(standard("2 + 3 * 4 =").unwrap().result, "20");
        assert_eq!(standard("1/3").unwrap().result, "0.3333333333333333");
        assert!(
            standard("sin(30)")
                .unwrap_err()
                .starts_with("Invalid input")
        );
    }

    /// The review's M2.
    #[test]
    fn standard_mode_errors() {
        let standard = |expression: &str| {
            calculate(&CalculateParams {
                expression: expression.into(),
                angle: AngleUnit::Degrees,
                mode: CalcMode::Standard,
            })
        };
        // Every final `=` is the end of the formula, not a repeat of the last operation.
        assert_eq!(standard("2 + 3 * 4 = =").unwrap().result, "20");
        assert_eq!(standard("2 + 3 * 4==  ").unwrap().result, "20");
        let error = standard("12345678901234567 + 1").unwrap_err();
        assert!(
            error.contains("at most 16 digits (12345678901234567 has 17)"),
            "{error}"
        );
        assert_eq!(
            standard("1234567890123456 + 1").unwrap().result,
            "1234567890123457"
        );
        let error = standard("sin(30)").unwrap_err();
        assert!(error.contains("functions"), "{error}");
        assert!(!error.contains(" has "), "{error}");
    }

    fn program(expression: &str, radix: RadixName, word_size: WordName) -> ProgrammerResult {
        programmer(&ProgrammerParams {
            expression: expression.into(),
            radix,
            word_size,
        })
        .unwrap()
    }

    #[test]
    fn programmer_mode() {
        let result = program("FF AND 3C", RadixName::Hex, WordName::Byte);
        assert_eq!(result.hex, "3C");
        assert_eq!(result.dec, "60");
        assert_eq!(result.bin, "00111100");
        assert_eq!(program("ff & 3c", RadixName::Hex, WordName::Byte).hex, "3C");
        assert_eq!(program("-1", RadixName::Dec, WordName::Word).hex, "FFFF");
        assert_eq!(program("~0", RadixName::Dec, WordName::Byte).dec, "-1");
        assert_eq!(program("NOT(0)", RadixName::Hex, WordName::Byte).hex, "FF");
        assert_eq!(program("1 << 4", RadixName::Dec, WordName::Qword).dec, "16");
        assert_eq!(
            program("4 OR 2 + 2", RadixName::Dec, WordName::Qword).dec,
            "4"
        );
        assert_eq!(
            program("(255 AND 10 - 7 / 2)", RadixName::Dec, WordName::Qword).dec,
            "6"
        );
        assert_eq!(program("ROL(80)", RadixName::Hex, WordName::Byte).hex, "1");
        assert_eq!(program("7 % 3", RadixName::Dec, WordName::Qword).dec, "1");
    }

    #[test]
    fn programmer_errors() {
        let error = |expression: &str, radix, word_size| {
            programmer(&ProgrammerParams {
                expression: expression.into(),
                radix,
                word_size,
            })
            .unwrap_err()
        };
        assert!(error("1G", RadixName::Hex, WordName::Qword).contains("isn't a HEX digit"));
        assert!(error("300", RadixName::Dec, WordName::Byte).contains("doesn't fit"));
        assert!(error("1 +", RadixName::Dec, WordName::Qword).contains("ends without"));
        assert!(error("(1 + 2", RadixName::Dec, WordName::Qword).contains("has no ')'"));
        assert_eq!(
            error("5 % 0", RadixName::Dec, WordName::Qword),
            "Result is undefined"
        );
    }

    fn programmer_error(expression: &str, radix: RadixName, word_size: WordName) -> String {
        programmer(&ProgrammerParams {
            expression: expression.into(),
            radix,
            word_size,
        })
        .unwrap_err()
    }

    const RADIXES: [RadixName; 4] = [
        RadixName::Hex,
        RadixName::Dec,
        RadixName::Oct,
        RadixName::Bin,
    ];
    const WORDS: [WordName; 4] = [
        WordName::Qword,
        WordName::Dword,
        WordName::Word,
        WordName::Byte,
    ];

    /// The review's B1: a u128 overflowed from 39 digits on, wrapped silently in release
    /// builds and panicked in debug builds.
    #[test]
    fn programmer_rejects_numbers_beyond_u128() {
        let nines = "9".repeat(44);
        let error = programmer_error(&nines, RadixName::Dec, WordName::Qword);
        assert_eq!(
            error,
            format!(
                "{nines} doesn't fit into a QWORD: in DEC it takes -9223372036854775808 to \
                 9223372036854775807"
            )
        );
        let two_to_128 = "340282366920938463463374607431768211456";
        assert!(
            programmer_error(two_to_128, RadixName::Dec, WordName::Qword).contains("doesn't fit")
        );
        assert!(
            programmer_error(&format!("-{nines}"), RadixName::Dec, WordName::Qword)
                .starts_with(&format!("-{nines} doesn't fit"))
        );
        for radix in RADIXES {
            for word_size in WORDS {
                // 39 and 40 binary digits fit a QWORD.
                for length in [39, 40, 65, 100, 512] {
                    if matches!((radix, word_size), (RadixName::Bin, WordName::Qword))
                        && length < 64
                    {
                        continue;
                    }
                    let number = "1".repeat(length);
                    let error = programmer_error(&number, radix, word_size);
                    assert!(error.contains("doesn't fit"), "{length} {radix:?}: {error}");
                }
            }
        }
        // Leading zeros don't count.
        let padded = format!("{}1", "0".repeat(100));
        assert_eq!(program(&padded, RadixName::Bin, WordName::Byte).dec, "1");
        assert_eq!(program("000", RadixName::Hex, WordName::Byte).dec, "0");
    }

    /// The largest and smallest value of each word size, and one beyond.
    #[test]
    fn programmer_word_boundaries() {
        for (word_size, bits) in [
            (WordName::Qword, 64),
            (WordName::Dword, 32),
            (WordName::Word, 16),
            (WordName::Byte, 8),
        ] {
            let max = (1u128 << (bits - 1)) - 1;
            let min = 1u128 << (bits - 1);
            let unsigned = (1u128 << bits) - 1;
            let dec = |text: &str| {
                programmer(&ProgrammerParams {
                    expression: text.into(),
                    radix: RadixName::Dec,
                    word_size,
                })
            };
            assert_eq!(dec(&max.to_string()).unwrap().dec, max.to_string());
            assert_eq!(
                dec(&format!("{}", max - 1)).unwrap().dec,
                (max - 1).to_string()
            );
            assert_eq!(dec(&format!("-{min}")).unwrap().dec, format!("-{min}"));
            assert_eq!(dec(&format!("-{min}")).unwrap().hex, format!("{min:X}"));
            assert_eq!(
                dec(&format!("-{}", min - 1)).unwrap().dec,
                format!("-{}", min - 1)
            );
            assert!(
                dec(&(max + 1).to_string())
                    .unwrap_err()
                    .contains("doesn't fit")
            );
            assert!(
                dec(&format!("-{}", min + 1))
                    .unwrap_err()
                    .contains("doesn't fit")
            );
            // As in the GUI, the largest value plus one wraps around.
            assert_eq!(dec(&format!("{max} + 1")).unwrap().dec, format!("-{min}"));

            for (radix, text, above) in [
                (
                    RadixName::Hex,
                    format!("{unsigned:X}"),
                    format!("1{:0>1$}", "", bits / 4),
                ),
                (
                    RadixName::Oct,
                    format!("{unsigned:o}"),
                    format!("{:o}", unsigned + 1),
                ),
                (
                    RadixName::Bin,
                    format!("{unsigned:b}"),
                    format!("1{:0>1$}", "", bits),
                ),
            ] {
                let result = program(&text, radix, word_size);
                assert_eq!(result.dec, "-1", "{radix:?} {word_size:?}");
                assert_eq!(result.hex, format!("{unsigned:X}"));
                let error = programmer_error(&above, radix, word_size);
                assert!(error.contains("doesn't fit"), "{above}: {error}");
            }
        }
        // M1: as in Windows, −128 is a byte, typed with its sign.
        let result = program("-128", RadixName::Dec, WordName::Byte);
        assert_eq!((result.dec.as_str(), result.hex.as_str()), ("-128", "80"));
        assert_eq!(result.expression_line, "-128 =");
        assert_eq!(program("--128", RadixName::Dec, WordName::Word).dec, "128");
        assert_eq!(program("NOT -1", RadixName::Dec, WordName::Byte).dec, "0");
        assert_eq!(
            program("2 - -128", RadixName::Dec, WordName::Byte).dec,
            "-126"
        );
        assert_eq!(program("-0", RadixName::Dec, WordName::Byte).dec, "0");
        // Outside DEC, a sign negates the finished number.
        assert_eq!(program("-1", RadixName::Hex, WordName::Byte).hex, "FF");
    }

    /// The engine fed key by key, as the GUI's keys feed it.
    fn engine_keys(word: WordSize, radix: Radix, keys: &[Command]) -> ProgrammerResult {
        let mut calc = Calculator::programmer();
        calc.execute(Command::WordSize(word));
        calc.execute(Command::Radix(radix));
        for &key in keys {
            calc.execute(key);
        }
        calc.execute(Command::Equals);
        let [hex, dec, oct, bin] = calc
            .radix_values()
            .unwrap()
            .map(|value| value.replace([' ', ','], ""));
        ProgrammerResult {
            hex,
            dec,
            oct,
            bin,
            expression_line: calc.expression(),
        }
    }

    fn digits(text: &str) -> Vec<Command> {
        text.chars()
            .map(|c| Command::Digit(c.to_digit(16).unwrap() as u8))
            .collect()
    }

    /// Results beyond the word wrap around in two's complement, as the GUI's engine does:
    /// the tool sends the engine its keys and does no arithmetic of its own.
    #[test]
    fn programmer_results_wrap_like_the_engine() {
        let op = |op| [Command::Operator(op)];
        let cases = [
            (
                "FFFFFFFFFFFFFFFF + 1",
                RadixName::Hex,
                WordName::Qword,
                WordSize::Qword,
                Radix::Hex,
                [digits("FFFFFFFFFFFFFFFF"), op(Op::Add).into(), digits("1")].concat(),
            ),
            (
                "4294967296 * 4294967296",
                RadixName::Dec,
                WordName::Qword,
                WordSize::Qword,
                Radix::Decimal,
                [
                    digits("4294967296"),
                    op(Op::Multiply).into(),
                    digits("4294967296"),
                ]
                .concat(),
            ),
            (
                "9223372036854775807 * 3",
                RadixName::Dec,
                WordName::Qword,
                WordSize::Qword,
                Radix::Decimal,
                [
                    digits("9223372036854775807"),
                    op(Op::Multiply).into(),
                    digits("3"),
                ]
                .concat(),
            ),
            (
                "200 * 200",
                RadixName::Dec,
                WordName::Word,
                WordSize::Word,
                Radix::Decimal,
                [digits("200"), op(Op::Multiply).into(), digits("200")].concat(),
            ),
            (
                "1 LSH 63",
                RadixName::Dec,
                WordName::Qword,
                WordSize::Qword,
                Radix::Decimal,
                [digits("1"), op(Op::LeftShift).into(), digits("63")].concat(),
            ),
            (
                "7F << 1",
                RadixName::Hex,
                WordName::Byte,
                WordSize::Byte,
                Radix::Hex,
                [digits("7F"), op(Op::LeftShift).into(), digits("1")].concat(),
            ),
        ];
        for (expression, radix_name, word_name, word, radix, keys) in cases {
            assert_eq!(
                program(expression, radix_name, word_name),
                engine_keys(word, radix, &keys),
                "{expression}"
            );
        }
        assert_eq!(
            program("FFFFFFFFFFFFFFFF + 1", RadixName::Hex, WordName::Qword).hex,
            "0"
        );
        assert_eq!(
            program("4294967296 * 4294967296", RadixName::Dec, WordName::Qword).dec,
            "0"
        );
        assert_eq!(
            programmer_error("1 LSH 64", RadixName::Dec, WordName::Qword),
            "Result not defined"
        );
        assert_eq!(
            programmer_error("1 << 8", RadixName::Dec, WordName::Byte),
            "Result not defined"
        );
    }

    /// `levels` parentheses: (((1+1)*2+1)*2 … +1)*2, which is 3·2^levels − 2.
    fn nested(levels: usize) -> String {
        format!("{}1+1)*2{}", "(".repeat(levels), "+1)*2".repeat(levels - 1))
    }

    /// The review's I1: beyond the engine's 25 levels, keys were left out silently.
    #[test]
    fn programmer_nesting_limit() {
        let value = |levels: u32| ((3u128 << levels) - 2).to_string();
        assert_eq!(
            program(&nested(24), RadixName::Dec, WordName::Qword).dec,
            "50331646"
        );
        let mut deepest = 0;
        for levels in 1..=40 {
            match programmer(&ProgrammerParams {
                expression: nested(levels),
                radix: RadixName::Dec,
                word_size: WordName::Qword,
            }) {
                Ok(result) => {
                    assert_eq!(result.dec, value(levels as u32), "{levels} levels");
                    assert_eq!(deepest, levels - 1, "no error before {levels} levels");
                    deepest = levels;
                }
                Err(error) => assert!(error.contains("more than 25 levels"), "{error}"),
            }
        }
        assert!(deepest >= 24, "{deepest}");
        for levels in [25, 28, 40] {
            assert!(
                programmer_error(&nested(levels), RadixName::Dec, WordName::Qword)
                    .contains("more than 25 levels")
            );
        }
        // Parentheses around a number alone go as deep as the engine's.
        let wrapped = format!("{}1{}", "(".repeat(25), ")".repeat(25));
        assert_eq!(program(&wrapped, RadixName::Dec, WordName::Qword).dec, "1");
        let wrapped = format!("{}1{}", "(".repeat(26), ")".repeat(26));
        assert!(programmer_error(&wrapped, RadixName::Dec, WordName::Qword).contains("25 levels"));
    }

    fn convert(category: &str, value: &str, from: &str, to: &str) -> Result<ConvertResult, String> {
        convert_units(&ConvertParams {
            category: category.into(),
            value: value.into(),
            from: from.into(),
            to: to.into(),
        })
    }

    #[test]
    fn converts_units_like_windows() {
        let result = convert("length", "5", "km", "mi").unwrap();
        assert_eq!(result.value, "3.106856");
        assert_eq!(
            (result.from.as_str(), result.to.as_str()),
            ("Kilometers", "Miles")
        );
        assert!(!result.about_equal_to.is_empty());
        assert_eq!(
            convert("Length", "1", "meter", "Feet").unwrap().value,
            "3.28084"
        );
        assert_eq!(
            convert("temperature", "-40", "°C", "fahrenheit")
                .unwrap()
                .value,
            "-40"
        );
        assert_eq!(convert("data", "1", "GB", "MB").unwrap().value, "1000");
    }

    #[test]
    fn conversion_errors() {
        assert!(
            convert("lenght", "1", "m", "ft")
                .unwrap_err()
                .starts_with("Unknown category")
        );
        assert!(
            convert("length", "1", "parsec", "ft")
                .unwrap_err()
                .contains("Kilometers (km)")
        );
        assert!(
            convert("length", "-1", "m", "ft")
                .unwrap_err()
                .contains("can't be negative")
        );
        assert!(
            convert("length", "1+1", "m", "ft")
                .unwrap_err()
                .starts_with("Invalid input")
        );
    }

    /// The review's I3: a mistyped parameter was ignored, and its default used instead.
    #[test]
    fn unknown_fields_are_refused() {
        fn refused<T: serde::de::DeserializeOwned + JsonSchema + std::fmt::Debug>(
            arguments: serde_json::Value,
            message: &str,
        ) {
            let error = serde_json::from_value::<T>(arguments)
                .unwrap_err()
                .to_string();
            assert!(error.starts_with(message), "{error}");
            let schema = serde_json::to_value(schemars::schema_for!(T)).unwrap();
            assert_eq!(schema["additionalProperties"], false);
        }
        use serde_json::json;
        refused::<ProgrammerParams>(
            json!({ "expression": "-1", "word": "byte" }),
            "unknown field `word`, expected one of `expression`, `radix`, `word_size`",
        );
        refused::<CalculateParams>(
            json!({ "expression": "sin(90)", "angles": "radians" }),
            "unknown field `angles`, expected one of `expression`, `angle`, `mode`",
        );
        refused::<CalculateParams>(
            json!({ "expression": "1+1", "extra": true }),
            "unknown field `extra`",
        );
        refused::<ConvertParams>(
            json!({ "category": "data", "value": "1", "from": "GB", "to": "MB", "unit": "x" }),
            "unknown field `unit`, expected one of `category`, `value`, `from`, `to`",
        );
        refused::<DateDifferenceParams>(
            json!({ "from": "2026-01-01", "to": "2026-01-02", "inclusive": true }),
            "unknown field `inclusive`",
        );
        refused::<DateAddParams>(
            json!({ "date": "2026-01-01", "weeks": 1 }),
            "unknown field `weeks`, expected one of `date`, `years`, `months`, `days`, `subtract`",
        );
        refused::<AnalyzeParams>(
            json!({ "equation": "x", "variable": "t" }),
            "unknown field `variable`, expected `equation` or `angle`",
        );
        // The limits the code checks are in the schemas too.
        let schema = serde_json::to_value(schemars::schema_for!(DateAddParams)).unwrap();
        assert_eq!(schema["properties"]["days"]["maximum"], 999);
        let schema = serde_json::to_value(schemars::schema_for!(CalculateParams)).unwrap();
        assert_eq!(schema["properties"]["expression"]["maxLength"], 512);
    }

    /// The GUI's converter, driven directly with the units' indexes.
    fn gui_convert(category: Category, from: usize, to: usize, value: &str) -> String {
        let mut converter = Converter::new(Region::new("US"));
        converter.set_category(category);
        converter.set_unit1(from);
        converter.set_unit2(to);
        converter.activate(true);
        assert!(converter.paste(value));
        converter.value2().replace(',', "")
    }

    fn gui_units(category: Category) -> &'static [Unit] {
        let mut converter = Converter::new(Region::new("US"));
        converter.set_category(category);
        converter.units()
    }

    /// The review's B2: every Data unit, by its abbreviation and by its name, to every other
    /// one, as the GUI's converter converts it.
    #[test]
    fn data_units_match_the_gui_converter() {
        let units = gui_units(Category::Data);
        assert_eq!(units.len(), 35);
        for (i, from) in units.iter().enumerate() {
            for (j, to) in units.iter().enumerate() {
                for (a, b) in [(from.abbreviation, to.abbreviation), (from.name, to.name)] {
                    let result = convert("data", "1.5", a, b).unwrap();
                    assert_eq!(
                        (result.from.as_str(), result.to.as_str()),
                        (from.name, to.name),
                        "{a} -> {b}"
                    );
                    assert_eq!(
                        result.value,
                        gui_convert(Category::Data, i, j, "1.5"),
                        "{a} -> {b}"
                    );
                }
            }
        }
    }

    /// Every unit of every category, by its abbreviation, name and key, to the category's
    /// first and last unit, as the GUI's converter converts it.
    #[test]
    fn every_category_matches_the_gui_converter() {
        for (category, names) in CATEGORIES {
            let units = gui_units(category);
            let value = if category.supports_negative() {
                "-2.5"
            } else {
                "2.5"
            };
            for (i, unit) in units.iter().enumerate() {
                for j in [0, units.len() - 1] {
                    for name in [unit.abbreviation, unit.name, unit.key] {
                        let result = convert(names[0], value, name, units[j].abbreviation)
                            .unwrap_or_else(|error| panic!("{name}: {error}"));
                        assert_eq!(result.from, unit.name, "{name}");
                        assert_eq!(result.to, units[j].name);
                        assert_eq!(result.value, gui_convert(category, i, j, value), "{name}");
                    }
                }
            }
        }
    }

    #[test]
    fn byte_and_bit_abbreviations() {
        let data = |value: &str, from: &str, to: &str| {
            let result = convert("data", value, from, to).unwrap();
            (result.from, result.to, result.value)
        };
        let owned = |from: &str, to: &str, value: &str| (from.into(), to.into(), value.into());
        assert_eq!(
            data("1", "GB", "MiB"),
            owned("Gigabytes", "Mebibytes", "953.6743")
        );
        assert_eq!(
            data("1", "Gb", "MiB"),
            owned("Gigabits", "Mebibytes", "119.2093")
        );
        assert_eq!(
            data("1", "GB", "MB"),
            owned("Gigabytes", "Megabytes", "1000")
        );
        assert_eq!(data("1", "Gb", "MB"), owned("Gigabits", "Megabytes", "125"));
        assert_eq!(data("1", "Mb", "Kb"), owned("Megabits", "Kilobits", "1000"));
        assert_eq!(data("1", "MB", "Mb"), owned("Megabytes", "Megabits", "8"));
        assert_eq!(data("1", "KB", "B"), owned("Kilobytes", "Bytes", "1000"));
        assert_eq!(data("1", "Kb", "b"), owned("Kilobits", "Bits", "1000"));
        assert_eq!(data("1", "KiB", "B"), owned("Kibibytes", "Bytes", "1024"));
        assert_eq!(
            data("1", "TB", "GB"),
            owned("Terabytes", "Gigabytes", "1000")
        );
        assert_eq!(
            data("1", "Tb", "TB"),
            owned("Terabits", "Terabytes", "0.125")
        );
        assert_eq!(
            data("1", "TiB", "GiB"),
            owned("Tebibytes", "Gibibytes", "1024")
        );
        assert_eq!(data("1", "kB", "B").2, "1000");
        assert_eq!(data("1", "Kib", "b").2, "1024");
        assert_eq!(data("1", "gigabytes", "MiB").2, "953.6743");
        assert_eq!(data("1", "YB", "b").2, "8.000000e+24");
        // Without its case, `gb` could be either.
        let error = convert("data", "1", "gb", "MB").unwrap_err();
        assert!(error.contains("ambiguous"), "{error}");
        assert!(error.contains("Gigabits (Gb) or Gigabytes (GB)"), "{error}");
        assert!(
            convert("data", "1", "kib", "B")
                .unwrap_err()
                .contains("ambiguous")
        );
    }

    #[test]
    fn aliases_name_existing_units() {
        for &(category, alias, key) in ALIASES {
            let units = gui_units(category);
            let i = units
                .iter()
                .position(|unit| unit.key == key)
                .unwrap_or_else(|| panic!("{alias}: no unit {key}"));
            assert_eq!(find_unit(category, units, alias), Ok(i), "{alias}");
        }
    }

    /// The review's I4 (its I5): common names of units the converters have.
    #[test]
    fn unit_aliases() {
        let value = |category: &str, from: &str, to: &str| {
            let result = convert(category, "1", from, to).unwrap();
            format!("{} {} = {}", result.from, result.to, result.value)
        };
        assert_eq!(
            value("volume", "gallon", "liter"),
            "Gallons (US) Liters = 3.785412"
        );
        assert_eq!(
            value("volume", "gallons", "litres"),
            "Gallons (US) Liters = 3.785412"
        );
        assert_eq!(
            value("volume", "imperial gallon", "L"),
            "Gallons (UK) Liters = 4.54609"
        );
        assert_eq!(
            value("volume", "cup", "mL"),
            "Cups (US) Milliliters = 236.5882"
        );
        assert_eq!(
            value("volume", "cups", "ml"),
            "Cups (US) Milliliters = 236.5882"
        );
        assert_eq!(
            value("volume", "cu ft", "m3"),
            "Cubic feet Cubic meters = 0.028317"
        );
        assert_eq!(
            value("power", "hp", "W"),
            "Horsepower (US) Watts = 745.6999"
        );
        assert_eq!(
            value("power", "kw", "horsepower"),
            "Kilowatts Horsepower (US) = 1.341022"
        );
        assert_eq!(
            value("length", "kilometre", "mile"),
            "Kilometers Miles = 0.621371"
        );
        assert_eq!(value("length", "um", "nm"), "Microns Nanometers = 1000");
        assert_eq!(value("length", "μm", "nm"), "Microns Nanometers = 1000");
        assert_eq!(
            value("area", "m2", "sq ft"),
            "Square meters Square feet = 10.76391"
        );
        assert_eq!(
            value("area", "km^2", "acre"),
            "Square kilometers Acres = 247.1054"
        );
        assert_eq!(
            value("speed", "kph", "mph"),
            "Kilometers per hour Miles per hour = 0.621427"
        );
        assert_eq!(
            value("speed", "knot", "km/h"),
            "Knots Kilometers per hour = 1.85184"
        );
        assert_eq!(value("temperature", "C", "F"), "Celsius Fahrenheit = 33.8");
        assert_eq!(
            value("temperature", "degrees celsius", "kelvin"),
            "Celsius Kelvin = 274.15"
        );
        assert_eq!(
            value("weight", "lbs", "kilo"),
            "Pounds Kilograms = 0.453592"
        );
        assert_eq!(
            value("weight", "tonne", "short ton"),
            "Metric tonnes Short tons (US) = 1.102311"
        );
        assert_eq!(value("time", "h", "sec"), "Hours Seconds = 3600");
        assert_eq!(
            value("energy", "kilowatt hour", "J"),
            "Kilowatt-hours Joules = 3600000"
        );
        assert_eq!(
            value("pressure", "bar", "psi"),
            "Bars Pounds per square inch = 14.50377"
        );
        assert_eq!(value("angle", "°", "rad"), "Degrees Radians = 0.017453");
    }

    #[test]
    fn unknown_units_list_close_matches() {
        let error = convert("volume", "1", "galon", "L").unwrap_err();
        assert!(
            error.starts_with("Unknown unit 'galon' for volume. Close matches: "),
            "{error}"
        );
        assert!(error.contains("Gallons (US) (gal (US))"), "{error}");
        let error = convert("weight", "1", "ton", "kg").unwrap_err();
        let close = error.split(" All ").next().unwrap();
        for unit in ["Short tons (US)", "Long tons (UK)", "Metric tonnes"] {
            assert!(close.contains(unit), "{error}");
        }
        // Units the converters don't have stay errors, and every unit is listed.
        for (category, unit) in [
            ("length", "parsec"),
            ("pressure", "torr"),
            ("pressure", "hPa"),
        ] {
            let error = convert(category, "1", unit, "m").unwrap_err();
            assert!(
                error.starts_with(&format!("Unknown unit '{unit}'")),
                "{error}"
            );
        }
        let error = convert("length", "1", "parsec", "m").unwrap_err();
        assert!(
            error.contains("All length units: Angstroms (A), "),
            "{error}"
        );
        assert!(error.contains("Nautical miles (nmi)"), "{error}");
    }

    #[test]
    fn dates_like_windows() {
        let difference = date_difference(&DateDifferenceParams {
            from: "2026-01-01".into(),
            to: "2026-10-18".into(),
        })
        .unwrap();
        assert_eq!(
            (
                difference.years,
                difference.months,
                difference.weeks,
                difference.days
            ),
            (0, 9, 2, 3)
        );
        assert_eq!(difference.total_days, 290);
        assert_eq!(difference.description, "9 months, 2 weeks, 3 days");

        let add = |date: &str, months, subtract| {
            date_add(&DateAddParams {
                date: date.into(),
                years: 0,
                months,
                days: 0,
                subtract,
            })
        };
        assert_eq!(add("2026-01-31", 1, false).unwrap().date, "2026-02-28");
        assert_eq!(
            add("2026-10-18", 0, false).unwrap().long_date,
            "Sunday, October 18, 2026"
        );
        assert_eq!(
            add("9999-12-31", 1, false).unwrap_err(),
            "Date out of Bound"
        );
        assert!(
            add("2026-13-01", 0, false)
                .unwrap_err()
                .contains("YYYY-MM-DD")
        );
        assert!(
            add("1500-01-01", 0, false)
                .unwrap_err()
                .contains("1601-01-01")
        );
    }

    #[test]
    fn analyzes_functions() {
        let result = analyze_function(&AnalyzeParams {
            equation: "y = 1/x".into(),
            angle: AngleUnit::Radians,
        })
        .unwrap();
        assert_eq!(result.domain.as_deref(), Some("{x ≠ 0}"));
        assert_eq!(result.vertical_asymptotes, ["x = 0"]);
        assert_eq!(result.horizontal_asymptotes, ["y = 0"]);
        assert_eq!(result.parity, "The function is odd.");

        let error = |equation: &str| {
            analyze_function(&AnalyzeParams {
                equation: equation.into(),
                angle: AngleUnit::Radians,
            })
            .unwrap_err()
        };
        assert_eq!(error("x)"), english::text("ParenthesisMismatch"));
        assert!(error("x = y^2").starts_with("Analysis is only supported"));
    }
}
