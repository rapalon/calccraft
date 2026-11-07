//! Random key sequences, equations, pastes and dates, checked against invariants that need
//! no oracle: nothing panics or hangs, what the display shows is a number the mode could
//! write, `C` clears an error, the memory and the history stay within their limits, a
//! finished number pastes back as itself, and dates behave like dates.
//!
//! Every test is deterministic: a seeded generator (xorshift, no dependency) whose seed is
//! in the test's name. Each runs `CALCCRAFT_FUZZ_CASES` cases (a default that keeps a normal
//! `cargo test` quick); the `#[ignore]`d `long_*` tests run many more, for example
//!
//! ```text
//! CALCCRAFT_FUZZ_CASES=200000 cargo test --release monkey -- --ignored
//! ```
//!
//! A failure prints the shortest key sequence that still fails, in the words of the
//! engine tests' `run_scientific` and `run_programmer`.

use std::sync::mpsc;
use std::time::Duration;

use chrono::{Datelike, NaiveDate};

use crate::converter::{Category, Converter, ConverterCommand, Region};
use crate::dates;
use crate::engine::{Angle, Calculator, Command, Func, Op, Radix, Trig, TrigFunction, WordSize};
use crate::graphing::analysis;
use crate::graphing::parse::Syntax;
use crate::graphing::plot::{self, View};
use crate::graphing::{Env, Equation, Shape};

/// xorshift64*: small, fast and the same on every platform.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() >> 11) as usize % n.max(1)
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// How many random cases a test runs: `CALCCRAFT_FUZZ_CASES`, or `default`.
fn cases(default: usize) -> usize {
    std::env::var("CALCCRAFT_FUZZ_CASES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

// ------------------------------------------------------------------ the engine

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Standard,
    Scientific,
    Programmer,
}

fn calculator(mode: Mode) -> Calculator {
    match mode {
        Mode::Standard => Calculator::default(),
        Mode::Scientific => Calculator::with_precedence(),
        Mode::Programmer => Calculator::programmer(),
    }
}

fn function(f: Func) -> Command {
    Command::Function(f)
}

/// Every command of a mode's keypad; digits are repeated so that numbers get typed.
fn pool(mode: Mode) -> Vec<Command> {
    use Command::{
        Backspace, Clear, ClearEntry, CloseParen, Decimal, Digit, Equals, Euler, Exponent,
        MemoryAdd, MemoryClear, MemoryRecall, MemoryStore, MemorySubtract, OpenParen, Operator,
        Percent, Pi, Random, ToggleBit, ToggleNotation,
    };
    let mut keys = Vec::new();
    let digits = if mode == Mode::Programmer { 16 } else { 10 };
    for _ in 0..3 {
        keys.extend((0..digits).map(Digit));
    }
    keys.extend([
        Operator(Op::Add),
        Operator(Op::Subtract),
        Operator(Op::Multiply),
        Operator(Op::Divide),
        Equals,
        Equals,
        Clear,
        ClearEntry,
        Backspace,
        Backspace,
        function(Func::Negate),
        MemoryStore,
        MemoryRecall,
        MemoryAdd,
        MemorySubtract,
        MemoryClear,
    ]);
    match mode {
        Mode::Standard => keys.extend([
            Decimal,
            Decimal,
            Percent,
            function(Func::Square),
            function(Func::SquareRoot),
            function(Func::Reciprocal),
            Exponent,
        ]),
        Mode::Scientific => {
            keys.extend([
                Decimal,
                Decimal,
                Percent,
                function(Func::Square),
                function(Func::SquareRoot),
                function(Func::Reciprocal),
                Exponent,
                Exponent,
                OpenParen,
                OpenParen,
                CloseParen,
                CloseParen,
                Operator(Op::Mod),
                Operator(Op::Power),
                Operator(Op::Root),
                Operator(Op::LogBase),
                Pi,
                Euler,
                Random,
                ToggleNotation,
                Command::Angle(Angle::Degrees),
                Command::Angle(Angle::Radians),
                Command::Angle(Angle::Gradians),
            ]);
            for f in [
                Func::Cube,
                Func::CubeRoot,
                Func::TenToX,
                Func::TwoToX,
                Func::EToX,
                Func::Log,
                Func::Ln,
                Func::Factorial,
                Func::Abs,
                Func::Floor,
                Func::Ceil,
                Func::Dms,
                Func::Degrees,
            ] {
                keys.push(function(f));
            }
            for function_name in TrigFunction::ALL {
                for inverse in [false, true] {
                    for hyperbolic in [false, true] {
                        keys.push(function(Func::Trig(Trig {
                            function: function_name,
                            inverse,
                            hyperbolic,
                        })));
                    }
                }
            }
        }
        Mode::Programmer => {
            keys.extend([
                OpenParen,
                CloseParen,
                Operator(Op::Mod),
                Operator(Op::And),
                Operator(Op::Or),
                Operator(Op::Xor),
                Operator(Op::Nand),
                Operator(Op::Nor),
                Operator(Op::LeftShift),
                Operator(Op::RightShift),
                Operator(Op::RightShiftLogical),
                function(Func::Not),
                function(Func::RotateLeft),
                function(Func::RotateRight),
                function(Func::RotateLeftCarry),
                function(Func::RotateRightCarry),
            ]);
            for radix in Radix::ALL {
                keys.push(Command::Radix(radix));
            }
            for word in [
                WordSize::Qword,
                WordSize::Dword,
                WordSize::Word,
                WordSize::Byte,
            ] {
                keys.push(Command::WordSize(word));
            }
            keys.extend((0..64).step_by(5).map(ToggleBit));
        }
    }
    keys
}

/// The key as the engine tests' `press`, `press_scientific` and `press_programmer` write it.
fn word(mode: Mode, command: Command) -> Option<String> {
    use Command::{
        Backspace, Clear, ClearEntry, CloseParen, Decimal, Digit, Equals, Euler, Exponent,
        OpenParen, Operator, Percent, Pi, Random, ToggleBit, ToggleNotation,
    };
    let operator = |op| {
        Some(
            match op {
                Op::Add => "+",
                Op::Subtract => "-",
                Op::Multiply => "*",
                Op::Divide => "/",
                Op::Mod if mode == Mode::Programmer => "%",
                Op::Mod => "mod",
                Op::And => "and",
                Op::Or => "or",
                Op::Xor => "xor",
                Op::Nand => "nand",
                Op::Nor => "nor",
                Op::LeftShift => "lsh",
                Op::RightShift => "rsh",
                Op::RightShiftLogical => "rshl",
                Op::Power => "^",
                Op::Root => "yroot",
                Op::LogBase => "logy",
            }
            .to_string(),
        )
    };
    match (mode, command) {
        (Mode::Standard, Digit(d)) => Some(d.to_string()),
        (Mode::Standard, Decimal) => Some(".".into()),
        (Mode::Standard, Operator(op)) => operator(op),
        (Mode::Standard, Equals) => Some("=".into()),
        (Mode::Standard, Percent) => Some("%".into()),
        (Mode::Standard, Command::Function(Func::Negate)) => Some("n".into()),
        (Mode::Standard, Command::Function(Func::Square)) => Some("s".into()),
        (Mode::Standard, Command::Function(Func::SquareRoot)) => Some("r".into()),
        (Mode::Standard, Command::Function(Func::Reciprocal)) => Some("i".into()),
        (Mode::Standard, Clear) => Some("c".into()),
        (Mode::Standard, ClearEntry) => Some("e".into()),
        (Mode::Standard, Backspace) => Some("<".into()),
        (Mode::Standard, _) => None,
        (_, Digit(d)) => Some(format!("{d:X}")),
        (_, Decimal) => Some(".".into()),
        (_, Operator(op)) => operator(op),
        (_, Equals) => Some("=".into()),
        (_, OpenParen) => Some("(".into()),
        (_, CloseParen) => Some(")".into()),
        (_, Clear) => Some("c".into()),
        (_, ClearEntry) => Some("ce".into()),
        (_, Backspace) => Some("bs".into()),
        (_, Command::Function(f)) => Some(
            match f {
                Func::Negate => "neg",
                Func::Square => "sqr",
                Func::SquareRoot => "sqrt",
                Func::Reciprocal => "inv",
                Func::Not => "not",
                Func::RotateLeft => "rol",
                Func::RotateRight => "ror",
                Func::RotateLeftCarry => "rolc",
                Func::RotateRightCarry => "rorc",
                Func::Cube => "cube",
                Func::CubeRoot => "cbrt",
                Func::TenToX => "10x",
                Func::TwoToX => "2x",
                Func::EToX => "ex",
                Func::Log => "log",
                Func::Ln => "ln",
                Func::Factorial => "fact",
                Func::Abs => "abs",
                Func::Floor => "floor",
                Func::Ceil => "ceil",
                Func::Dms => "dms",
                Func::Degrees => "todeg",
                Func::Trig(t) => {
                    return Some(format!(
                        "{}{}{}",
                        if t.inverse { "a" } else { "" },
                        t.function.label(),
                        if t.hyperbolic { "h" } else { "" }
                    ));
                }
            }
            .into(),
        ),
        (Mode::Scientific, Pi) => Some("pi".into()),
        (Mode::Scientific, Euler) => Some("euler".into()),
        (Mode::Scientific, Random) => Some("rand".into()),
        (Mode::Scientific, Exponent) => Some("exp".into()),
        (Mode::Scientific, ToggleNotation) => Some("fe".into()),
        (Mode::Scientific, Command::Angle(a)) => Some(
            match a {
                Angle::Degrees => "DEG",
                Angle::Radians => "RAD",
                Angle::Gradians => "GRAD",
            }
            .into(),
        ),
        (Mode::Programmer, Command::Radix(r)) => Some(
            match r {
                Radix::Hex => "hex",
                Radix::Decimal => "dec",
                Radix::Octal => "oct",
                Radix::Binary => "bin",
            }
            .into(),
        ),
        (Mode::Programmer, Command::WordSize(w)) => Some(
            match w {
                WordSize::Qword => "qword",
                WordSize::Dword => "dword",
                WordSize::Word => "word",
                WordSize::Byte => "byte",
            }
            .into(),
        ),
        (Mode::Programmer, ToggleBit(bit)) => Some(format!("bit{bit}")),
        _ => None,
    }
}

/// A call that reproduces a failing sequence.
fn reproduction(mode: Mode, commands: &[Command]) -> String {
    let words: Option<Vec<String>> = commands.iter().map(|&c| word(mode, c)).collect();
    match (mode, words) {
        (Mode::Standard, Some(words)) => format!("run(\"{}\")", words.concat()),
        (Mode::Scientific, Some(words)) => format!("run_scientific(\"{}\")", merge_numbers(&words)),
        (Mode::Programmer, Some(words)) => format!("run_programmer(\"{}\")", merge_numbers(&words)),
        _ => format!("{commands:?} on {mode:?}"),
    }
}

/// Joins runs of digit keys into numbers, as the helpers' key language writes them.
fn merge_numbers(words: &[String]) -> String {
    let is_digit = |w: &str| w.len() == 1 && w.chars().all(|c| c.is_ascii_hexdigit() || c == '.');
    let mut out: Vec<String> = Vec::new();
    for w in words {
        match out.last_mut() {
            Some(last)
                if is_digit(w) && last.chars().all(|c| c.is_ascii_hexdigit() || c == '.') =>
            {
                last.push_str(w)
            }
            _ => out.push(w.clone()),
        }
    }
    out.join(" ")
}

fn is_zero(display: &str) -> bool {
    display.chars().any(|c| c == '0') && display.chars().all(|c| matches!(c, '0' | ' '))
}

/// Whether a Standard or Scientific display is a number: `-1234.5`, `1.e+16`, `0.`.
fn plain_number_digits(display: &str) -> Option<usize> {
    let text = display.replace(',', "");
    let (mantissa, exponent) = match text.split_once('e') {
        Some((m, e)) => (m, Some(e)),
        None => (text.as_str(), None),
    };
    if let Some(exponent) = exponent {
        let digits = exponent.strip_prefix(['+', '-'])?;
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
    }
    let mantissa = mantissa.strip_prefix('-').unwrap_or(mantissa);
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    if !int.chars().chain(frac.chars()).all(|c| c.is_ascii_digit()) {
        return None;
    }
    let all: String = int.chars().chain(frac.chars()).collect();
    Some(all.trim_start_matches('0').len())
}

fn check_state(mode: Mode, calc: &Calculator, command: Command) -> Option<String> {
    if calc.memory().len() > 100 {
        return Some(format!("memory: {} items", calc.memory().len()));
    }
    if calc.history().len() > 20 {
        return Some(format!("history: {} items", calc.history().len()));
    }
    let display = calc.display();
    if display.is_empty() {
        return Some("display: empty".into());
    }
    if command == Command::Clear {
        if calc.has_error() {
            return Some(format!("clear: C left the error {display:?}"));
        }
        if !is_zero(&display) {
            return Some(format!("clear: C left {display:?}"));
        }
    }
    if let Some(error) = calc.error() {
        if display != error.message() {
            return Some(format!(
                "error: the display shows {display:?}, not {:?}",
                error.message()
            ));
        }
        return None;
    }
    match mode {
        Mode::Standard | Mode::Scientific => {
            let limit = if mode == Mode::Standard { 16 } else { 32 };
            match plain_number_digits(&display) {
                None => Some(format!("display: {display:?} is not a number")),
                Some(n) if n > limit => Some(format!(
                    "display: {display:?} has {n} significant digits, the limit is {limit}"
                )),
                Some(_) => None,
            }
        }
        Mode::Programmer => {
            let radix = calc.radix().unwrap();
            let text = display.replace([' ', ','], "");
            let (negative, digits) = match text.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, text.as_str()),
            };
            if digits.is_empty()
                || !digits
                    .chars()
                    .all(|c| c.is_digit(radix.base()) && !c.is_ascii_lowercase())
                || (negative && radix != Radix::Decimal)
            {
                return Some(format!("display: {display:?} is not a {radix:?} number"));
            }
            let value = i128::from_str_radix(digits, radix.base()).ok()?;
            let bits = calc.word_size().unwrap().bits();
            let (low, high) = if radix == Radix::Decimal {
                (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
            } else {
                (0, (1i128 << bits) - 1)
            };
            let value = if negative { -value } else { value };
            (value < low || value > high)
                .then(|| format!("display: {display:?} does not fit a {bits}-bit word ({radix:?})"))
        }
    }
}

/// A finished number copied and pasted into a fresh calculator shows the same number.
fn check_paste(mode: Mode, calc: &Calculator) -> Option<String> {
    let text = calc.copy_text()?;
    let mut fresh = calculator(mode);
    if let (Some(radix), Some(word)) = (calc.radix(), calc.word_size()) {
        fresh.execute(Command::Radix(radix));
        fresh.execute(Command::WordSize(word));
    }
    if !fresh.paste(&text) {
        return Some(format!(
            "paste: {text:?} (copied from {:?}) is rejected",
            calc.display()
        ));
    }
    let normal = |s: String| {
        let s = s.replace([',', ' '], "");
        let digits = s
            .trim_start_matches('-')
            .trim_start_matches('0')
            .to_string();
        // A binary display starts with zeros up to a whole group, and a pasted zero never
        // takes a sign (`-0.` pastes as `0.`, as in Windows' `OnPaste`).
        let mantissa = digits.split('e').next().unwrap_or("");
        let zero = mantissa.chars().all(|c| c == '0' || c == '.');
        let sign = if s.starts_with('-') && !zero { "-" } else { "" };
        format!("{sign}{digits}")
    };
    (normal(fresh.display()) != normal(calc.display())).then(|| {
        format!(
            "paste: {:?} pastes as {:?}",
            calc.display(),
            fresh.display()
        )
    })
}

/// Plays `commands`, checking after every command; `progress` hears about each step.
fn play(
    mode: Mode,
    commands: &[Command],
    progress: Option<&mpsc::Sender<usize>>,
) -> Option<String> {
    let mut calc = calculator(mode);
    for (i, &command) in commands.iter().enumerate() {
        calc.execute(command);
        if let Some(problem) = check_state(mode, &calc, command) {
            return Some(problem);
        }
        if let Some(progress) = progress {
            let _ = progress.send(i);
        }
    }
    check_paste(mode, &calc)
}

/// `play` on its own thread: a panic or a step of more than `limit` is a result.
fn play_guarded(mode: Mode, commands: &[Command], limit: Duration) -> Option<String> {
    let (done_tx, done_rx) = mpsc::channel();
    let (step_tx, step_rx) = mpsc::channel();
    let owned = commands.to_vec();
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                play(mode, &owned, Some(&step_tx))
            }));
            let _ = done_tx.send(result.map_err(|payload| {
                payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default()
            }));
        })
        .expect("thread");
    let mut step = 0;
    loop {
        if let Ok(result) = done_rx.try_recv() {
            return match result {
                Ok(problem) => problem,
                Err(message) => Some(format!("panic: {message}")),
            };
        }
        match step_rx.recv_timeout(limit) {
            Ok(i) => step = i + 1,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Some(format!("hang: step {} took more than {limit:?}", step + 1));
            }
            // The thread ended: its result is in `done_rx`.
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return match done_rx.recv_timeout(Duration::from_secs(5)) {
                    Ok(Ok(problem)) => problem,
                    Ok(Err(message)) => Some(format!("panic: {message}")),
                    Err(_) => Some("hang: no result".into()),
                };
            }
        }
    }
}

/// The shortest sequence that still fails the same way (removing one key at a time).
fn shrink(mode: Mode, mut commands: Vec<Command>, problem: &str, limit: Duration) -> Vec<Command> {
    let kind = problem.split(':').next().unwrap_or("").to_string();
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..commands.len() {
            let mut candidate = commands.clone();
            candidate.remove(i);
            if play_guarded(mode, &candidate, limit)
                .is_some_and(|p| p.split(':').next() == Some(kind.as_str()))
            {
                commands = candidate;
                changed = true;
                break;
            }
        }
    }
    commands
}

fn random_sequence(rng: &mut Rng, pool: &[Command], longest: usize) -> Vec<Command> {
    let length = 1 + rng.below(longest);
    (0..length).map(|_| *rng.pick(pool)).collect()
}

fn run_engine(mode: Mode, seed: u64, count: usize, limit: Duration) {
    let keys = pool(mode);
    let mut rng = Rng::new(seed);
    let mut failures = Vec::new();
    for _ in 0..count {
        let commands = random_sequence(&mut rng, &keys, 40);
        if let Some(problem) = play_guarded(mode, &commands, limit) {
            let minimal = shrink(mode, commands, &problem, limit);
            failures.push(format!("{problem}\n    {}", reproduction(mode, &minimal)));
            if failures.len() >= 5 {
                break;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{mode:?}, seed {seed}:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn monkey_standard_seed_1() {
    run_engine(Mode::Standard, 1, cases(3000), Duration::from_secs(2));
}

#[test]
fn monkey_scientific_seed_2() {
    run_engine(Mode::Scientific, 2, cases(1500), Duration::from_secs(5));
}

#[test]
fn monkey_programmer_seed_3() {
    run_engine(Mode::Programmer, 3, cases(3000), Duration::from_secs(2));
}

#[test]
#[ignore = "long campaign: set CALCCRAFT_FUZZ_CASES"]
fn long_engine_campaign() {
    let count = cases(100_000);
    run_engine(Mode::Standard, 101, count, Duration::from_secs(2));
    run_engine(Mode::Scientific, 102, count / 2, Duration::from_secs(10));
    run_engine(Mode::Programmer, 103, count, Duration::from_secs(2));
}

#[test]
fn memory_and_history_stay_within_their_limits() {
    for mode in [Mode::Standard, Mode::Scientific, Mode::Programmer] {
        let mut calc = calculator(mode);
        for i in 0..130u8 {
            calc.execute(Command::Digit(i % 10));
            calc.execute(Command::MemoryStore);
            if mode != Mode::Programmer {
                calc.execute(Command::Operator(Op::Add));
                calc.execute(Command::Digit(1));
                calc.execute(Command::Equals);
            }
        }
        assert_eq!(calc.memory().len(), 100, "{mode:?} memory");
        assert!(calc.history().len() <= 20, "{mode:?} history");
        if mode != Mode::Programmer {
            assert_eq!(calc.history().len(), 20, "{mode:?} history");
        }
    }
}

// ------------------------------------------------------------------- equations

const EQUATION_PIECES: &[&str] = &[
    "0", "1", "2", "3", "7", "9", "10", "100", "0.5", ".5", "x", "y", "m", "b", "a", "e", "i", "+",
    "-", "*", "/", "^", "(", ")", "(", ")", "|", "!", "²", "√", "π", "=", "<", ">", "≤", "≥",
    "sin", "cos", "tan", "sec", "csc", "cot", "log", "ln", "abs", "sqrt", "root", "asin", "sinh",
    "exp", "floor", " ", ",", ";", "x^2", "2x", "mx+b", "1/x", "x!", "|x|", "sin(x)", "√x", "10^x",
    "x^x", "x^(x^x)", "(x)(x)", "−", "×", "÷", "·", "∞", "$", "#", "é", "\u{0}",
];

fn random_equation(rng: &mut Rng) -> String {
    let mut text = String::new();
    match rng.below(10) {
        // Deep nesting of one construct.
        0 => {
            let depth = 1 + rng.below(300);
            let (open, close, core) = *rng.pick(&[
                ("(", ")", "x"),
                ("sin(", ")", "x"),
                ("|", "|", "x"),
                ("√(", ")", "2x"),
                ("(", ")!", "5"),
            ]);
            text.push_str(&open.repeat(depth));
            text.push_str(core);
            text.push_str(&close.repeat(depth));
        }
        // A chain of one operator.
        1 => {
            let op = *rng.pick(&["^", "!", "√", "²", "*", "+", "sin", "-", "x"]);
            for _ in 0..1 + rng.below(80) {
                text.push_str(op);
                if rng.chance(50) {
                    text.push('x');
                }
            }
        }
        _ => {
            for _ in 0..1 + rng.below(40) {
                text.push_str(rng.pick(EQUATION_PIECES));
            }
        }
    }
    text
}

/// Runs `job` on its own thread; `Err` is a panic or no answer within `limit`.
fn guarded<T: Send + 'static>(
    limit: Duration,
    job: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            let _ = tx.send(result.map_err(|payload| {
                payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default()
            }));
        })
        .expect("thread");
    match rx.recv_timeout(limit) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(message)) => Err(format!("panic: {message}")),
        Err(_) => Err(format!("hang: no result after {limit:?}")),
    }
}

fn run_equations(seed: u64, count: usize, analyse_every: usize) {
    let mut rng = Rng::new(seed);
    let mut failures = Vec::new();
    for case in 0..count {
        let text = random_equation(&mut rng);
        let syntax = if rng.chance(20) {
            Syntax::for_decimal(',')
        } else {
            Syntax::default()
        };
        let analyse = case % analyse_every == 0;
        let owned = text.clone();
        let outcome = guarded(Duration::from_secs(5), move || {
            let Ok(equation) = Equation::parse(&owned, syntax) else {
                return;
            };
            let vars: Vec<(char, f64)> = equation.variables.iter().map(|&v| (v, 1.0)).collect();
            let env = Env {
                angle: Angle::Radians,
                vars: &vars,
            };
            let view = View::DEFAULT;
            let _ = plot::plot(&equation, &env, &view, 200.0, 150.0);
            let _ = plot::best_fit(
                &[(
                    &equation,
                    Env {
                        angle: Angle::Degrees,
                        vars: &vars,
                    },
                )],
                200.0,
                150.0,
            );
            if analyse && matches!(equation.shape, Shape::Function(_)) {
                let _ = analysis::analyze(&equation, Angle::Radians);
            }
        });
        if let Err(problem) = outcome {
            let shown: String = text.chars().take(200).collect();
            failures.push(format!(
                "{problem}\n    Equation::parse({shown:?}) ({} chars)",
                text.chars().count()
            ));
            if failures.len() >= 5 {
                break;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "seed {seed}:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn monkey_equations_seed_4() {
    run_equations(4, cases(3000), 20);
}

#[test]
#[ignore = "long campaign: set CALCCRAFT_FUZZ_CASES"]
fn long_equation_campaign() {
    run_equations(104, cases(100_000), 10);
}

// ------------------------------------------------------------------ converters

fn run_converter_pastes(seed: u64, count: usize) {
    const ALPHABET: &[char] = &[
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '.', ',', '-', 'e', '+', ' ',
    ];
    let mut rng = Rng::new(seed);
    let mut failures = Vec::new();
    for _ in 0..count {
        let category = *rng.pick(&Category::ALL);
        let length = rng.below(21);
        let text: String = (0..length).map(|_| *rng.pick(ALPHABET)).collect();
        let (code, from, to) = (
            *rng.pick(&["US", "DE", "GB", "JP"]),
            rng.below(40),
            rng.below(40),
        );
        let keys: Vec<ConverterCommand> = (0..rng.below(6))
            .map(|_| match rng.below(14) {
                d @ 0..=9 => ConverterCommand::Digit(d as u8),
                10 => ConverterCommand::Decimal,
                11 => ConverterCommand::Negate,
                12 => ConverterCommand::Backspace,
                _ => ConverterCommand::Clear,
            })
            .collect();
        let owned = text.clone();
        let outcome = guarded(Duration::from_secs(5), move || {
            let mut converter = Converter::new(Region::new(code));
            converter.set_category(category);
            let count = converter.units().len();
            converter.set_unit1(from % count);
            converter.set_unit2(to % count);
            let accepted = converter.paste(&owned);
            for key in keys {
                converter.execute(key);
            }
            let _ = (
                converter.value1(),
                converter.value2(),
                converter.copy_text(),
                converter.suggestions(),
            );
            accepted
        });
        if let Err(problem) = outcome {
            failures.push(format!(
                "{problem}\n    {category:?} {code} paste({text:?})"
            ));
            if failures.len() >= 5 {
                break;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "seed {seed}:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn monkey_converter_pastes_seed_5() {
    run_converter_pastes(5, cases(5000));
}

#[test]
#[ignore = "long campaign: set CALCCRAFT_FUZZ_CASES"]
fn long_converter_campaign() {
    run_converter_pastes(105, cases(200_000));
}

// ----------------------------------------------------------------------- dates

fn random_date(rng: &mut Rng) -> NaiveDate {
    let first = dates::MIN_DATE.num_days_from_ce();
    let last = dates::MAX_DATE.num_days_from_ce();
    let span = (last - first + 1) as usize;
    // Now and then a date at a month end, a leap day or an edge of the range.
    let day = match rng.below(8) {
        0 => first + rng.below(60) as i32,
        1 => last - rng.below(60) as i32,
        _ => first + rng.below(span) as i32,
    };
    let date = NaiveDate::from_num_days_from_ce_opt(day).unwrap();
    if rng.chance(15) {
        // Move to the end of the month.
        let (y, m) = (date.year(), date.month());
        let next = if m == 12 {
            NaiveDate::from_ymd_opt(y + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(y, m + 1, 1)
        };
        if let Some(next) = next {
            return (next - chrono::Days::new(1)).clamp(dates::MIN_DATE, dates::MAX_DATE);
        }
    }
    date
}

#[test]
fn monkey_dates_seed_6() {
    let mut rng = Rng::new(6);
    for _ in 0..cases(20_000) {
        let (a, b) = (random_date(&mut rng), random_date(&mut rng));
        let diff = dates::difference(a, b);
        assert_eq!(
            diff,
            dates::difference(b, a),
            "the order of {a} and {b} matters"
        );
        assert_eq!(dates::days_between(a, b), dates::days_between(b, a));
        assert_eq!(
            i64::from(dates::days_between(a, b)),
            (b - a).num_days().abs(),
            "{a} {b}"
        );
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        // The breakdown, added to the earlier date as offsets, gives the later date.
        let back = dates::add(start, diff.years, diff.months, diff.weeks * 7 + diff.days);
        assert_eq!(back, Some(end), "{start} + {diff:?}");
        assert_eq!(a == b, diff == dates::difference(a, a), "{a} {b}");

        let n = rng.below(1000) as u32;
        // Adding days only is undone by subtracting them, when both stay in range.
        if let Some(moved) = dates::add(a, 0, 0, n) {
            assert!(
                (dates::MIN_DATE..=dates::MAX_DATE).contains(&moved),
                "{a} + {n} days"
            );
            assert_eq!(
                dates::subtract(moved, 0, 0, n),
                Some(a),
                "{a} + {n} days - {n} days"
            );
        }
        if let Some(moved) = dates::subtract(a, 0, 0, n) {
            assert!(
                (dates::MIN_DATE..=dates::MAX_DATE).contains(&moved),
                "{a} - {n} days"
            );
            assert_eq!(
                dates::add(moved, 0, 0, n),
                Some(a),
                "{a} - {n} days + {n} days"
            );
        }
        let (y, m) = (rng.below(1000) as u32, rng.below(1000) as u32);
        for result in [dates::add(a, y, m, n), dates::subtract(a, y, m, n)]
            .into_iter()
            .flatten()
        {
            assert!(
                (dates::MIN_DATE..=dates::MAX_DATE).contains(&result),
                "{a} +- {y}y {m}m {n}d = {result}"
            );
        }
    }
}

/// Replays date queries (`D y m d y m d`, `A y m d years months days`, `S ...`) of
/// `CALCCRAFT_DATES_IN` into `CALCCRAFT_DATES_OUT`, in the format of a Python port of
/// Microsoft's `DateCalculationEngine` that is used as the oracle.
#[test]
#[ignore = "tool for the oracle comparison: needs CALCCRAFT_DATES_IN and CALCCRAFT_DATES_OUT"]
fn replay_dates() {
    let input = std::env::var("CALCCRAFT_DATES_IN").expect("CALCCRAFT_DATES_IN");
    let output = std::env::var("CALCCRAFT_DATES_OUT").expect("CALCCRAFT_DATES_OUT");
    let text = std::fs::read_to_string(input).expect("read input");
    let date = |y: &str, m: &str, d: &str| {
        NaiveDate::from_ymd_opt(y.parse().unwrap(), m.parse().unwrap(), d.parse().unwrap()).unwrap()
    };
    let show = |d: Option<NaiveDate>| match d {
        Some(d) => format!("{} {} {}", d.year(), d.month(), d.day()),
        None => "none".to_string(),
    };
    let mut out = String::new();
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w[0] {
            "D" => {
                let d = dates::difference(date(w[1], w[2], w[3]), date(w[4], w[5], w[6]));
                out.push_str(&format!(
                    "D {} {} {} {}\n",
                    d.years, d.months, d.weeks, d.days
                ));
            }
            kind => {
                let start = date(w[1], w[2], w[3]);
                let n: Vec<u32> = w[4..7].iter().map(|x| x.parse().unwrap()).collect();
                let result = if kind == "A" {
                    dates::add(start, n[0], n[1], n[2])
                } else {
                    dates::subtract(start, n[0], n[1], n[2])
                };
                out.push_str(&format!("{kind} {}\n", show(result)));
            }
        }
    }
    std::fs::write(output, out).expect("write output");
}

/// The category named like its variant (`Length`, `Temperature`, …).
fn category_named(name: &str) -> Category {
    *Category::ALL
        .iter()
        .find(|c| format!("{c:?}") == name)
        .unwrap_or_else(|| panic!("no category {name}"))
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// What the replay writes for a converter: both fields (the typed one first, without group
/// separators) and the "About equal to" results.
fn converter_snapshot(conv: &Converter) -> String {
    let (typed, other) = if conv.value1_active() {
        (conv.value1(), conv.value2())
    } else {
        (conv.value2(), conv.value1())
    };
    let suggestions: Vec<String> = conv
        .suggestions()
        .iter()
        .map(|s| {
            let value = json_escape(&s.value.replace(',', ""));
            format!("[\"{value}\",\"{}\"]", s.key)
        })
        .collect();
    format!(
        "{{\"from\":\"{}\",\"to\":\"{}\",\"sug\":[{}]}}",
        json_escape(&typed.replace(',', "")),
        json_escape(&other.replace(',', "")),
        suggestions.join(",")
    )
}

/// Runs one converter session (`U:From:To` picks the units of the first and second field in
/// the order of the typed and the other one, as Microsoft's harness does).
fn converter_session(region: &str, name: &str, keys: &[String]) -> Vec<String> {
    let mut conv = Converter::new(Region::new(region));
    conv.set_category(category_named(name));
    let mut steps = vec![converter_snapshot(&conv)];
    for key in keys {
        match key.as_str() {
            d if d.len() == 1 && d.as_bytes()[0].is_ascii_digit() => {
                conv.execute(ConverterCommand::Digit(d.as_bytes()[0] - b'0'))
            }
            "PNT" => conv.execute(ConverterCommand::Decimal),
            "NEG" => conv.execute(ConverterCommand::Negate),
            "BACK" => conv.execute(ConverterCommand::Backspace),
            "CLEAR" => conv.execute(ConverterCommand::Clear),
            "SWITCH" => conv.activate(!conv.value1_active()),
            k if k.starts_with("U:") => {
                let (from, to) = k[2..].split_once(':').expect("U:From:To");
                let units = conv.units();
                let index = |key: &str| units.iter().position(|u| u.key == key).expect("unit");
                let (from, to) = (index(from), index(to));
                if conv.value1_active() {
                    conv.set_unit1(from);
                    conv.set_unit2(to);
                } else {
                    conv.set_unit1(to);
                    conv.set_unit2(from);
                }
            }
            k if k.starts_with("C:") => conv.set_category(category_named(&k[2..])),
            other => panic!("unknown key {other}"),
        }
        steps.push(converter_snapshot(&conv));
    }
    steps
}

/// Replays converter sessions (`region Category key key …`, keys `0`-`9`, `PNT`, `NEG`, `BACK`,
/// `CLEAR`, `SWITCH`, `U:From:To`, `C:Category`) of `CALCCRAFT_CONV_IN` into
/// `CALCCRAFT_CONV_OUT`: what both fields and the "About equal to" results show after each key,
/// in the format of a harness around Microsoft's `UnitConverter.cpp`.
#[test]
#[ignore = "tool for the oracle comparison: needs CALCCRAFT_CONV_IN and CALCCRAFT_CONV_OUT"]
fn replay_converter() {
    let input = std::env::var("CALCCRAFT_CONV_IN").expect("CALCCRAFT_CONV_IN");
    let output = std::env::var("CALCCRAFT_CONV_OUT").expect("CALCCRAFT_CONV_OUT");
    let text = std::fs::read_to_string(input).expect("read input");
    let mut out = String::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let mut words = line.split_whitespace();
        let (region, name) = (
            words.next().unwrap().to_string(),
            words.next().unwrap().to_string(),
        );
        let keys: Vec<String> = words.map(str::to_string).collect();
        let outcome = guarded(Duration::from_secs(5), {
            let keys = keys.clone();
            move || converter_session(&region, &name, &keys)
        });
        let keys_json: Vec<String> = keys.iter().map(|k| format!("\"{k}\"")).collect();
        match outcome {
            Ok(steps) => out.push_str(&format!(
                "{{\"keys\":[{}],\"steps\":[{}]}}
",
                keys_json.join(","),
                steps.join(",")
            )),
            Err(problem) => out.push_str(&format!(
                "{{\"keys\":[{}],\"fail\":\"{}\"}}
",
                keys_json.join(","),
                json_escape(&problem)
            )),
        }
    }
    std::fs::write(output, out).expect("write output");
}
