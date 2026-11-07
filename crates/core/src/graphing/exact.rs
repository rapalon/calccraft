//! Exact values for the numbers the analysis finds.
//!
//! Windows' engine solves symbolically and shows `π/2` or `−2√3 − 6`. CalcCraft finds
//! the same features numerically, then recognizes the doubles as the simple exact values
//! they are: integers, fractions, short decimals (`1.05`, to the roundoff), rational
//! multiples of π, and (a + b√c)/d. A multiple of π or a surd has to be much shorter to
//! write than the decimal (or exact to the roundoff), or it would only be a near miss.
//! Anything else is shown as a decimal with 6 significant digits.

use std::f64::consts::PI;

/// A recognized number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Exact {
    /// p/q, in lowest terms with q > 0.
    Rational(i64, i64),
    /// pπ/q.
    Pi(i64, i64),
    /// (p/q)√c + r/s, with c square-free and above 1.
    Surd {
        p: i64,
        q: i64,
        c: i64,
        r: i64,
        s: i64,
    },
    Decimal(f64),
}

/// The greatest common divisor (the analysis reduces periods with it too).
pub(super) fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

fn reduce(p: i64, q: i64) -> (i64, i64) {
    let g = gcd(p, q).max(1);
    let (p, q) = (p / g, q / g);
    if q < 0 { (-p, -q) } else { (p, q) }
}

/// Whether `v` is `target` within `tolerance` (relative to the size of the numbers).
fn near(v: f64, target: f64, tolerance: f64) -> bool {
    (v - target).abs() <= tolerance * v.abs().max(1.0)
}

/// p/q with q ≤ `max_q`, if `v` is one.
fn rational(v: f64, max_q: i64, tolerance: f64) -> Option<(i64, i64)> {
    if v.abs() > 1e12 {
        return None;
    }
    (1..=max_q).find_map(|q| {
        let p = (v * q as f64).round();
        near(v, p / q as f64, tolerance).then(|| reduce(p as i64, q))
    })
}

const SQUARE_FREE: [i64; 18] = [
    2, 3, 5, 6, 7, 10, 11, 13, 14, 15, 17, 19, 21, 22, 23, 26, 29, 30,
];

/// How many digits an exact form has to save, against a number written to the tolerance
/// (9 significant digits within `TIGHT`, 6 within `LOOSE`), to be believed: a short decimal
/// has its last digit more than 10^`SAVED_DIGITS` times the tolerance, and a multiple of π
/// or a surd takes that many fewer digits to write (the √ or π counting as one). Less is a
/// near miss: there are so many (p/q)√c + r/s that a long one is within the tolerance of
/// almost any number (within `LOOSE`, 1.05 is −6√21 + 314/11, 8 digits for the 3 of 1.05).
/// With 3, chance matches are rarer than one number in a thousand.
const SAVED_DIGITS: i32 = 3;

/// The digits written for an integer: none for 0.
fn digits(n: i64) -> i32 {
    n.unsigned_abs()
        .checked_ilog10()
        .map_or(0, |d| d as i32 + 1)
}

/// The digits written for a factor or a denominator: none for 1.
fn factor_digits(n: i64) -> i32 {
    if n.abs() == 1 { 0 } else { digits(n) }
}

/// The decimal `v` is to the roundoff (`roundoff`), if its last digit is more than
/// 10^`SAVED_DIGITS` times `error` (1.05, typed into the equation). A number only within the
/// tolerance of one is a number of its own: 1.0100002 is a slope, and 0.0100001 and
/// 0.0100003 two values (written 1.01, 0.0100001 and 0.0100003 all the same).
fn short_decimal(v: f64, error: f64, roundoff: f64) -> Option<f64> {
    let scale = 10f64.powi(-(error.log10().floor() as i32 + 1 + SAVED_DIGITS));
    let d = (v * scale).round() / scale;
    ((v - d).abs() <= roundoff).then_some(d)
}

/// How close (relative) a multiple of π or a surd has to be to a number to be it however
/// long it is: the roundoff of a few operations on doubles. A near miss is about the
/// tolerance off, and next to none of the forms tried is this close to a number by chance.
pub const ROUNDOFF: f64 = 1e-12;

/// The (p/q)√c + r/s within `error` of `v` that is closest to it, of those that take no
/// more than `budget` digits to write or are within `roundoff` of it.
fn surd(v: f64, error: f64, budget: i32, roundoff: f64) -> Option<Exact> {
    let mut best: Option<(f64, Exact)> = None;
    for c in SQUARE_FREE {
        let root = (c as f64).sqrt();
        for q in 1..=12i64 {
            for p in -12..=12i64 {
                if p == 0 || gcd(p, q) != 1 {
                    continue;
                }
                // The rest as the fraction with the smallest denominator.
                let rest = v - p as f64 / q as f64 * root;
                let Some((r, s)) = (1..=12).find_map(|s| {
                    let r = (rest * s as f64).round();
                    ((rest - r / s as f64).abs() <= error).then(|| reduce(r as i64, s))
                }) else {
                    continue;
                };
                // The √ counts as a digit.
                let cost = factor_digits(p)
                    + factor_digits(q)
                    + digits(c)
                    + digits(r)
                    + factor_digits(s)
                    + 1;
                let off = (rest - r as f64 / s as f64).abs();
                if (cost <= budget || off <= roundoff)
                    && best.is_none_or(|(closest, _)| off < closest)
                {
                    best = Some((off, Exact::Surd { p, q, c, r, s }));
                }
            }
        }
    }
    best.map(|(_, exact)| exact)
}

/// Recognizes `v` within `tolerance` (relative): an integer or a fraction, else the
/// closest of a short decimal (to the `ROUNDOFF`), a multiple of π and a surd that save
/// `SAVED_DIGITS` (or are exact to the `ROUNDOFF`). A root is exact to its last few digits
/// and a near miss is off by about the tolerance: 599π/11 is within `TIGHT` of 171.074 too.
pub fn recognize(v: f64, tolerance: f64) -> Exact {
    if !v.is_finite() {
        return Exact::Decimal(v);
    }
    if let Some((p, q)) = rational(v, 12, tolerance) {
        return Exact::Rational(p, q);
    }
    // Large numbers have too many near misses to be told apart.
    if v.abs() > 1000.0 {
        return Exact::Decimal(v);
    }
    let error = tolerance * v.abs().max(1.0);
    let roundoff = ROUNDOFF * v.abs().max(1.0);
    let decimal = short_decimal(v, error, roundoff);
    if decimal == Some(v) {
        // Nothing is closer (a coefficient typed into the equation).
        return Exact::Decimal(v);
    }
    // The significant digits of a number written to the tolerance, less the saving.
    let budget = (v.abs().min(1.0) / tolerance).log10().ceil() as i32 - SAVED_DIGITS;
    // The π counts as a digit, like the √ of a surd. `rational` holds the tolerance for v/π,
    // which is closer to p/q than v is to pπ/q when v is below π, so check v itself too.
    let pi = rational(v / PI, 12, tolerance)
        .map(|(p, q)| (factor_digits(p) + factor_digits(q) + 1, Exact::Pi(p, q)))
        .filter(|&(cost, pi)| {
            let off = (pi.value() - v).abs();
            off <= error && (cost <= budget || off <= roundoff)
        })
        .map(|(_, pi)| pi);
    // On a tie, the decimal, then the multiple of π.
    [
        decimal.map(Exact::Decimal),
        pi,
        surd(v, error, budget, roundoff),
    ]
    .into_iter()
    .flatten()
    .min_by(|a, b| (a.value() - v).abs().total_cmp(&(b.value() - v).abs()))
    .unwrap_or(Exact::Decimal(v))
}

impl Exact {
    /// The number, computed from its exact form.
    pub fn value(self) -> f64 {
        match self {
            Exact::Rational(p, q) => p as f64 / q as f64,
            Exact::Pi(p, q) => p as f64 * PI / q as f64,
            Exact::Surd { p, q, c, r, s } => {
                p as f64 / q as f64 * (c as f64).sqrt() + r as f64 / s as f64
            }
            Exact::Decimal(v) => v,
        }
    }
}

/// `v` moved to the exact value it is near, if any.
pub fn snap(v: f64, tolerance: f64) -> f64 {
    recognize(v, tolerance).value()
}

/// The tolerance for values found by root finding, which are accurate to a few ulps.
pub const TIGHT: f64 = 1e-9;
/// The tolerance for limits, which are estimated from far-away values.
pub const LOOSE: f64 = 1e-6;

/// `v` written the way Windows shows numbers in the analysis.
pub fn format(v: f64) -> String {
    format_exact(recognize(v, TIGHT))
}

/// `v` written exactly if it is a limit-like estimate.
pub fn format_loose(v: f64) -> String {
    format_exact(recognize(v, LOOSE))
}

const MINUS: char = '−';

fn fraction(p: i64, q: i64) -> String {
    let sign = if p < 0 {
        MINUS.to_string()
    } else {
        String::new()
    };
    if q == 1 {
        format!("{sign}{}", p.abs())
    } else {
        format!("{sign}{}/{q}", p.abs())
    }
}

/// `coefficient · symbol` for a fraction p/q: `π`, `−π`, `3π/2`, `√3/3`, `2√3`.
fn times(p: i64, q: i64, symbol: &str) -> String {
    let sign = if p < 0 {
        MINUS.to_string()
    } else {
        String::new()
    };
    let a = p.abs();
    let mut s = sign;
    if a != 1 {
        s.push_str(&a.to_string());
    }
    s.push_str(symbol);
    if q != 1 {
        s.push('/');
        s.push_str(&q.to_string());
    }
    s
}

pub fn format_exact(exact: Exact) -> String {
    match exact {
        Exact::Rational(p, q) => fraction(p, q),
        Exact::Pi(0, _) => "0".to_string(),
        Exact::Pi(p, q) => times(p, q, "π"),
        Exact::Surd { p, q, c, r, s } => {
            let mut out = times(p, q, &format!("√{c}"));
            if r != 0 {
                out.push_str(if r > 0 { " + " } else { " − " });
                out.push_str(&fraction(r.abs(), s));
            }
            out
        }
        Exact::Decimal(v) => decimal(v),
    }
}

/// A decimal with 6 significant digits, without trailing zeros.
pub fn decimal(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    let digits = (5 - v.abs().log10().floor() as i32).clamp(0, 15) as usize;
    let mut s = format!("{:.*}", digits, v);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s.replace('-', &MINUS.to_string())
}

/// `k·x`, the slope part of a line: `x`, `−x`, `2x`, `x/2`, `−3x`, with x before the
/// denominator as there (`3πx/2`, `√3x/3`), and a slope of two terms as one factor:
/// `(√2 + 1)x`, not `√2 + 1x`.
pub fn coefficient_of_x(m: f64) -> String {
    match recognize(m, LOOSE) {
        Exact::Rational(p, q) => times(p, q, "x"),
        Exact::Pi(p, q) => times(p, q, "πx"),
        Exact::Surd { p, q, c, r: 0, .. } => times(p, q, &format!("√{c}x")),
        other @ Exact::Surd { .. } => format!("({})x", format_exact(other)),
        other => format!("{}x", format_exact(other)),
    }
}

/// `text` as a term after another: ` + text`, or ` − rest` if `text` is `−rest`, so that a
/// value of two terms adds with its own signs: x + (1 − √2), whose value is written
/// `−√2 + 1`, is `x − √2 + 1`.
pub fn plus_term(text: &str) -> String {
    match text.strip_prefix(MINUS) {
        Some(rest) => format!(" {MINUS} {rest}"),
        None => format!(" + {text}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rationals() {
        assert_eq!(format(0.0), "0");
        assert_eq!(format(5.0), "5");
        assert_eq!(format(-2.5), "−5/2");
        assert_eq!(format(1.0 / 3.0), "1/3");
        assert_eq!(format(1e-12), "0");
    }

    #[test]
    fn multiples_of_pi() {
        assert_eq!(format(PI), "π");
        assert_eq!(format(-PI), "−π");
        assert_eq!(format(2.0 * PI), "2π");
        assert_eq!(format(PI / 2.0), "π/2");
        assert_eq!(format(3.0 * PI / 2.0), "3π/2");
        assert_eq!(format(-PI / 4.0), "−π/4");
    }

    #[test]
    fn square_roots() {
        // The extrema of (−3x² + 2)/(x − 1) in Windows' manual tests.
        let s3 = 3f64.sqrt();
        assert_eq!(format(s3 / 3.0 + 1.0), "√3/3 + 1");
        assert_eq!(format(-s3 / 3.0 + 1.0), "−√3/3 + 1");
        assert_eq!(format(-2.0 * s3 - 6.0), "−2√3 − 6");
        assert_eq!(format(2.0 * s3 - 6.0), "2√3 − 6");
        assert_eq!(format(6f64.sqrt() / 3.0), "√6/3");
        assert_eq!(format(-(2f64.sqrt())), "−√2");
    }

    #[test]
    fn decimals() {
        assert_eq!(format(1f64.sin()), "0.841471");
        assert_eq!(format(-123456.789), "−123457");
        assert_eq!(decimal(0.000123456789), "0.000123457");
    }

    #[test]
    fn decimals_are_no_surds() {
        // Within LOOSE, some (p/q)√c + r/s is near almost any number: 1.05 was −6√21 + 314/11,
        // 0.3 · π/2 was −9√19/8 + 43/8 and π/180 was √2/11 − 1/9.
        assert_eq!(format_loose(1.05), "1.05");
        assert_eq!(format_loose(1.05 + 1e-12), "1.05");
        assert_eq!(coefficient_of_x(1.05), "1.05x");
        assert_eq!(format_loose(0.3 * PI / 2.0), "0.471239");
        assert_eq!(format_loose(PI / 180.0), "0.0174533");
        // Within TIGHT too, for larger numbers: an inflection point of x·sin(x) in degrees
        // was −9√11/10 − 1852/9.
        assert_eq!(format(-208.762_739_939_297), "−208.763");
        // A surd or a multiple of π within TIGHT of a short decimal (3.248452, 940.907,
        // 171.074, 342.148) is still what it is: it is closer. Exact to the roundoff, even
        // a long one is.
        assert_eq!(format(3.0 + 5f64.sqrt() / 9.0), "√5/9 + 3");
        assert_eq!(format(599.0 * PI / 2.0), "599π/2");
        assert_eq!(format(599.0 * PI / 11.0), "599π/11");
        assert_eq!(format(1198.0 * PI / 11.0), "1198π/11");
        // A short decimal is what it looks like. To compute with, it is exact only to the
        // roundoff: the limits from both sides of 1/x + 1.05 are one number, but 1.0100002
        // isn't 1.01, though it is written so.
        assert_eq!(snap(1.05 + 1e-12, LOOSE), 1.05);
        assert_eq!(snap(1.05 - 1e-12, LOOSE), 1.05);
        assert_eq!(format_loose(1.0100002), "1.01");
        assert_eq!(snap(1.0100002, LOOSE), 1.0100002);
        assert_eq!(snap(1.0500000005, TIGHT), 1.0500000005);
        // Half of these were surds within LOOSE.
        for k in -1000..=1000 {
            let v = k as f64 / 100.0;
            for tolerance in [TIGHT, LOOSE] {
                let exact = recognize(v, tolerance);
                assert!(
                    matches!(exact, Exact::Rational(..) | Exact::Decimal(_)),
                    "{v} is {}",
                    format_exact(exact)
                );
            }
        }
    }

    #[test]
    fn multiples_of_pi_are_within_the_tolerance() {
        // The tolerance holds for the number, not for its quotient by π, which is closer for a
        // number below π: π/12 + 2·10⁻⁶ is more than LOOSE off π/12, though its quotient is
        // within 10⁻⁶ of 1/12.
        assert_eq!(format_loose(PI / 12.0 + 2e-6), "0.261801");
        assert_eq!(format_loose(PI / 2.0 + 3e-6), "1.5708");
        assert_eq!(format_loose(PI / 12.0 + 5e-7), "π/12");
    }

    #[test]
    fn slopes() {
        assert_eq!(coefficient_of_x(1.0), "x");
        assert_eq!(coefficient_of_x(-1.0), "−x");
        assert_eq!(coefficient_of_x(-3.0000001), "−3x");
        assert_eq!(coefficient_of_x(0.5), "x/2");
        // x before the denominator, as in x/2, and a slope of two terms in parentheses.
        assert_eq!(coefficient_of_x(1.5 * PI), "3πx/2");
        assert_eq!(coefficient_of_x(-PI / 4.0), "−πx/4");
        assert_eq!(coefficient_of_x(3f64.sqrt() / 3.0), "√3x/3");
        assert_eq!(coefficient_of_x(2f64.sqrt() + 1.0), "(√2 + 1)x");
    }
}
