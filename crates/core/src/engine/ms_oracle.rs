//! Replays key sequences that were recorded from Microsoft's own `CalcManager`
//! (microsoft/calculator, `src/CalcManager`, built unmodified) and compares what the
//! port shows after every key.
//!
//! Test code only. `replay_file` is `#[ignore]`d: it reads sequences in the oracle
//! harness' input format and writes the port's observations in the harness' output
//! format, so that a script can compare the two files (and normalise what the C#
//! view model owns rather than the engine). The golden vectors in `ms_golden.txt` are
//! sequences on which the port agrees with Microsoft after that normalisation; the
//! normal test run checks them.

use super::*;
use std::fmt::Write as _;
use std::sync::mpsc;
use std::time::Duration;

/// What is observable after one key.
#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct Observation {
    pub display: String,
    pub expression: String,
    pub error: bool,
    pub parens: usize,
    pub memory: Vec<String>,
    /// The Programmer mode's radix list (HEX, DEC, OCT, BIN), without digit groups.
    pub radixes: Option<Vec<String>>,
}

/// The port's command for the name of a `CalculationManager::Command` (upper case), or
/// the harness' extra names for the radix list and the memory buttons.
pub(super) fn command_for(key: &str) -> Option<Command> {
    use TrigFunction::{Cos, Cot, Csc, Sec, Sin, Tan};
    let function = |f| Some(Command::Function(f));
    let operator = |o| Some(Command::Operator(o));
    let trig = |function, inverse, hyperbolic| {
        Some(Command::Function(Func::Trig(Trig {
            function,
            inverse,
            hyperbolic,
        })))
    };
    if let [c] = key.as_bytes() {
        match c {
            b'0'..=b'9' => return Some(Command::Digit(c - b'0')),
            b'A'..=b'F' => return Some(Command::Digit(c - b'A' + 10)),
            _ => {}
        }
    }
    if let Some(bit) = key.strip_prefix("BINPOS") {
        return bit.parse().ok().map(Command::ToggleBit);
    }
    match key {
        "PNT" => Some(Command::Decimal),
        "ADD" => operator(Op::Add),
        "SUB" => operator(Op::Subtract),
        "MUL" => operator(Op::Multiply),
        "DIV" => operator(Op::Divide),
        "MOD" => operator(Op::Mod),
        "PWR" => operator(Op::Power),
        "ROOT" => operator(Op::Root),
        "LOGBASEY" => operator(Op::LogBase),
        "AND" => operator(Op::And),
        "OR" => operator(Op::Or),
        "XOR" => operator(Op::Xor),
        "NAND" => operator(Op::Nand),
        "NOR" => operator(Op::Nor),
        "LSHF" => operator(Op::LeftShift),
        "RSHF" => operator(Op::RightShift),
        "RSHFL" => operator(Op::RightShiftLogical),
        "EQU" => Some(Command::Equals),
        "PERCENT" => Some(Command::Percent),
        "SIGN" => function(Func::Negate),
        "CLEAR" => Some(Command::Clear),
        "CENTR" => Some(Command::ClearEntry),
        "BACK" => Some(Command::Backspace),
        "SQRT" => function(Func::SquareRoot),
        "SQR" => function(Func::Square),
        "REC" => function(Func::Reciprocal),
        "CUB" => function(Func::Cube),
        "CUBEROOT" => function(Func::CubeRoot),
        "POW10" => function(Func::TenToX),
        "POW2" => function(Func::TwoToX),
        "POWE" => function(Func::EToX),
        "LN" => function(Func::Ln),
        "LOG" => function(Func::Log),
        "FAC" => function(Func::Factorial),
        "ABS" => function(Func::Abs),
        "FLOOR" => function(Func::Floor),
        "CEIL" => function(Func::Ceil),
        "DMS" => function(Func::Dms),
        "DEGREES" => function(Func::Degrees),
        "NOT" | "COM" => function(Func::Not),
        "ROL" => function(Func::RotateLeft),
        "ROR" => function(Func::RotateRight),
        "ROLC" => function(Func::RotateLeftCarry),
        "RORC" => function(Func::RotateRightCarry),
        "PI" => Some(Command::Pi),
        "EULER" => Some(Command::Euler),
        "RAND" => Some(Command::Random),
        "FE" => Some(Command::ToggleNotation),
        "EXP" => Some(Command::Exponent),
        "DEG" => Some(Command::Angle(Angle::Degrees)),
        "RAD" => Some(Command::Angle(Angle::Radians)),
        "GRAD" => Some(Command::Angle(Angle::Gradians)),
        "OPENP" => Some(Command::OpenParen),
        "CLOSEP" => Some(Command::CloseParen),
        "HEX" => Some(Command::Radix(Radix::Hex)),
        "DEC" => Some(Command::Radix(Radix::Decimal)),
        "OCT" => Some(Command::Radix(Radix::Octal)),
        "BIN" => Some(Command::Radix(Radix::Binary)),
        "QWORD" => Some(Command::WordSize(WordSize::Qword)),
        "DWORD" => Some(Command::WordSize(WordSize::Dword)),
        "WORD" => Some(Command::WordSize(WordSize::Word)),
        "BYTE" => Some(Command::WordSize(WordSize::Byte)),
        "MS" => Some(Command::MemoryStore),
        "MR" => Some(Command::MemoryRecall),
        "MPLUS" => Some(Command::MemoryAdd),
        "MMINUS" => Some(Command::MemorySubtract),
        "MC" => Some(Command::MemoryClear),
        "SIN" => trig(Sin, false, false),
        "COS" => trig(Cos, false, false),
        "TAN" => trig(Tan, false, false),
        "SEC" => trig(Sec, false, false),
        "CSC" => trig(Csc, false, false),
        "COT" => trig(Cot, false, false),
        "ASIN" => trig(Sin, true, false),
        "ACOS" => trig(Cos, true, false),
        "ATAN" => trig(Tan, true, false),
        "ASEC" => trig(Sec, true, false),
        "ACSC" => trig(Csc, true, false),
        "ACOT" => trig(Cot, true, false),
        "SINH" => trig(Sin, false, true),
        "COSH" => trig(Cos, false, true),
        "TANH" => trig(Tan, false, true),
        "SECH" => trig(Sec, false, true),
        "CSCH" => trig(Csc, false, true),
        "COTH" => trig(Cot, false, true),
        "ASINH" => trig(Sin, true, true),
        "ACOSH" => trig(Cos, true, true),
        "ATANH" => trig(Tan, true, true),
        "ASECH" => trig(Sec, true, true),
        "ACSCH" => trig(Csc, true, true),
        "ACOTH" => trig(Cot, true, true),
        _ => None,
    }
}

fn new_calculator(mode: &str) -> Calculator {
    match mode {
        "std" => Calculator::default(),
        "sci" => Calculator::with_precedence(),
        "prog" => Calculator::programmer(),
        _ => panic!("unknown mode {mode}"),
    }
}

pub(super) fn observe(calc: &Calculator) -> Observation {
    Observation {
        display: calc.display(),
        expression: calc.expression(),
        error: calc.has_error(),
        parens: calc.open_parentheses(),
        memory: calc.memory().iter().map(|n| calc.format_value(n)).collect(),
        radixes: calc
            .radix_values()
            .map(|all| all.iter().map(|s| s.replace(' ', "")).collect()),
    }
}

/// Plays one sequence: the observation before the first key, then one per key. The
/// history (oldest first, as Microsoft's `GetHistoryItems`) comes last.
pub(super) fn play(mode: &str, keys: &[String]) -> (Vec<Observation>, Vec<(String, String)>) {
    let mut calc = new_calculator(mode);
    let mut steps = vec![observe(&calc)];
    for key in keys {
        let command = command_for(key).unwrap_or_else(|| panic!("no command for key {key}"));
        calc.execute(command);
        steps.push(observe(&calc));
    }
    let history = calc
        .history()
        .iter()
        .rev()
        .map(|entry| (entry.expression.clone(), entry.text.clone()))
        .collect();
    (steps, history)
}

fn escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn list(items: &[String], out: &mut String) {
    out.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        escape(item, out);
    }
    out.push(']');
}

/// The observations of one sequence as a line in the harness' output format.
fn json_line(
    index: usize,
    mode: &str,
    keys: &[String],
    result: &Result<PlayResult, String>,
) -> String {
    let mut out = String::new();
    let _ = write!(out, "{{\"i\":{index},\"mode\":");
    escape(mode, &mut out);
    out.push_str(",\"keys\":");
    list(keys, &mut out);
    match result {
        Err(problem) => {
            out.push_str(",\"fail\":");
            escape(problem, &mut out);
        }
        Ok((steps, history)) => {
            out.push_str(",\"steps\":[");
            for (i, step) in steps.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str("{\"d\":");
                escape(&step.display, &mut out);
                out.push_str(",\"e\":");
                escape(&step.expression, &mut out);
                let _ = write!(
                    out,
                    ",\"err\":{},\"p\":{},\"m\":",
                    u8::from(step.error),
                    step.parens
                );
                list(&step.memory, &mut out);
                if let Some(radixes) = &step.radixes {
                    out.push_str(",\"r\":");
                    list(radixes, &mut out);
                }
                out.push('}');
            }
            out.push(']');
            if mode != "prog" {
                out.push_str(",\"h\":[");
                for (i, (expression, result)) in history.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    list(&[expression.clone(), result.clone()], &mut out);
                }
                out.push(']');
            }
        }
    }
    out.push('}');
    out
}

type PlayResult = (Vec<Observation>, Vec<(String, String)>);

/// Plays a sequence on its own thread, so that a panic or a hang is a result and not the
/// end of the run.
fn play_guarded(mode: &str, keys: &[String]) -> Result<PlayResult, String> {
    let (tx, rx) = mpsc::channel();
    let (mode, keys) = (mode.to_string(), keys.to_vec());
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let result = std::panic::catch_unwind(|| play(&mode, &keys));
            let _ = tx.send(result.map_err(|payload| {
                let text = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()));
                format!("panic: {}", text.unwrap_or_default())
            }));
        })
        .expect("thread");
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(result) => result,
        Err(_) => Err("hang: no result after 5 s".to_string()),
    }
}

/// The cases of Microsoft's `CalculatorManagerTest.cpp`, see the file's header.
const MANAGER_TESTS: &str = include_str!("ms_manager_tests.txt");

fn without_spaces(s: &str) -> String {
    s.split_whitespace().collect()
}

#[test]
fn microsoft_calculator_manager_tests() {
    // Source: microsoft/calculator, src/CalculatorUnitTests/CalculatorManagerTest.cpp. The view
    // model writes the expression's tokens with spaces of its own (`4 + 3=` there, `4 + 3 =`
    // here), so the expression is compared without spaces.
    let mut failures = Vec::new();
    let mut checked = 0;
    for line in MANAGER_TESTS
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let fields: Vec<&str> = line.split('\t').collect();
        let [mode, keys, display, expression, source] = fields[..] else {
            panic!("bad line {line}");
        };
        let keys: Vec<String> = keys
            .split(' ')
            .filter(|key| !key.is_empty())
            .map(str::to_string)
            .collect();
        let (steps, _) = play(mode, &keys);
        let last = steps.last().unwrap();
        checked += 1;
        if last.display != display {
            failures.push(format!(
                "{source}: display {:?}, Microsoft {display:?}",
                last.display
            ));
        }
        if expression != "N/A" && without_spaces(&last.expression) != without_spaces(expression) {
            failures.push(format!(
                "{source}: expression {:?}, Microsoft {expression:?}",
                last.expression
            ));
        }
    }
    assert!(checked >= 150, "only {checked} cases");
    assert!(
        failures.is_empty(),
        "{} of {checked}:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Sequences on which this engine and Microsoft's `CalcManager` agree, see the file's header.
const GOLDEN: &str = include_str!("ms_golden.txt");

/// What the view model and the digit grouping own is compared without it: commas and spaces
/// in numbers, the binary list's leading zeros (the view model pads them), and the spaces
/// of the expression line.
fn plain_number(s: &str) -> String {
    let s = s.replace([',', ' '], "");
    let (sign, digits) = match s.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", s.as_str()),
    };
    let trimmed = digits.trim_start_matches('0');
    let digits = if trimmed.is_empty() || trimmed.starts_with(['.', 'e']) {
        // `0`, `0.5`: keep one zero in front of what follows.
        format!("0{trimmed}")
    } else {
        trimmed.to_string()
    };
    format!("{sign}{digits}")
}

fn unescape(s: &str) -> String {
    s.replace("\\t", "\t")
        .replace("\\n", "\n")
        .replace("\\\\", "\\")
}

#[test]
fn microsoft_golden_vectors() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for line in GOLDEN
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let fields: Vec<&str> = line.split('\t').collect();
        let [
            mode,
            keys,
            display,
            expression,
            error,
            parens,
            memory,
            history,
            radixes,
        ] = fields[..]
        else {
            panic!("bad line {line}");
        };
        let keys: Vec<String> = keys.split(' ').map(str::to_string).collect();
        let (steps, played_history) = play(mode, &keys);
        let last = steps.last().unwrap();
        let mut differences = Vec::new();
        if plain_number(&last.display) != plain_number(&unescape(display)) {
            differences.push(format!("display {:?}, Microsoft {display:?}", last.display));
        }
        if without_spaces(&last.expression) != without_spaces(&unescape(expression)) {
            differences.push(format!(
                "expression {:?}, Microsoft {expression:?}",
                last.expression
            ));
        }
        if u8::from(last.error).to_string() != error {
            differences.push(format!("error {}, Microsoft {error}", last.error));
        }
        if last.parens.to_string() != parens {
            differences.push(format!("parentheses {}, Microsoft {parens}", last.parens));
        }
        let memory: Vec<String> = memory
            .split('|')
            .filter(|item| !item.is_empty())
            .map(|item| plain_number(&unescape(item)))
            .collect();
        let played_memory: Vec<String> =
            last.memory.iter().map(|item| plain_number(item)).collect();
        if played_memory != memory {
            differences.push(format!("memory {:?}, Microsoft {memory:?}", last.memory));
        }
        if mode != "prog" {
            let expected: Vec<(String, String)> = history
                .split(';')
                .filter(|item| !item.is_empty())
                .map(|item| {
                    let (e, r) = item.split_once("=>").unwrap();
                    (without_spaces(&unescape(e)), plain_number(&unescape(r)))
                })
                .collect();
            let played: Vec<(String, String)> = played_history
                .iter()
                .map(|(e, r)| (without_spaces(e), plain_number(r)))
                .collect();
            if played != expected {
                differences.push(format!("history {played_history:?}, Microsoft {history:?}"));
            }
        } else if radixes != "-" {
            let expected: Vec<String> = radixes.split('|').map(plain_number).collect();
            let played: Vec<String> = last
                .radixes
                .iter()
                .flatten()
                .map(|item| plain_number(item))
                .collect();
            if played != expected {
                differences.push(format!("radixes {:?}, Microsoft {radixes:?}", last.radixes));
            }
        }
        checked += 1;
        if !differences.is_empty() {
            failures.push(format!(
                "{mode} {}: {}",
                keys.join(" "),
                differences.join("; ")
            ));
        }
    }
    assert!(checked >= 1500, "only {checked} vectors");
    assert!(
        failures.is_empty(),
        "{} of {checked}:\n{}",
        failures.len(),
        failures[..failures.len().min(20)].join("\n")
    );
}

/// Replays `CALCCRAFT_REPLAY_IN` (lines `mode KEY KEY …`) into `CALCCRAFT_REPLAY_OUT`.
#[test]
#[ignore = "tool for the oracle comparison: needs CALCCRAFT_REPLAY_IN and CALCCRAFT_REPLAY_OUT"]
fn replay_file() {
    let input = std::env::var("CALCCRAFT_REPLAY_IN").expect("CALCCRAFT_REPLAY_IN");
    let output = std::env::var("CALCCRAFT_REPLAY_OUT").expect("CALCCRAFT_REPLAY_OUT");
    let text = std::fs::read_to_string(input).expect("read input");
    let mut out = String::new();
    for (index, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let mut words = line.split_whitespace();
        let mode = words.next().unwrap();
        let keys: Vec<String> = words.map(str::to_string).collect();
        let result = play_guarded(mode, &keys);
        out.push_str(&json_line(index, mode, &keys, &result));
        out.push('\n');
    }
    std::fs::write(output, out).expect("write output");
}
