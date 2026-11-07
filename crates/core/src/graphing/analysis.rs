//! The key graph features of a function y = f(x), Windows' "Function analysis": domain,
//! range, intercepts, extrema, inflection points, asymptotes, parity, period and
//! monotonicity.
//!
//! Windows computes them symbolically with a proprietary engine. CalcCraft finds them
//! numerically, from the function and its symbolic derivatives: zeros by sign changes
//! (refined by bisection) on a grid from −1000 to 1000, limits from far-away and nearby
//! values, and the exact values with `exact`. A function made of trigonometric
//! functions of linear arguments (sin x, tan 2x, cos² x …) is periodic: it's analyzed
//! over one period, and its features are written as families (`{πn₁, n₁ ∈ ℤ}`), as in
//! Windows. What can't be found that way is listed as too complex, as Windows lists
//! what its engine can't do.

use super::exact::{self, LOOSE, TIGHT};
use super::expr::{BinOp, Env, Expr, Func, radians_per_unit};
use super::{Equation, Shape};
use crate::engine::Angle;

/// Why there is no analysis (Windows' `AnalysisErrorType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisError {
    /// Inequalities, functions with free variables (`y = mx + b`) and functions without
    /// real values (`y = √(−1)`).
    NotSupported,
    /// Equations that aren't y = f(x).
    VariableIsNotX,
}

impl AnalysisError {
    /// The key of the message in Windows' resources.
    pub fn key(self) -> &'static str {
        match self {
            AnalysisError::NotSupported => "KGFAnalysisNotSupported",
            AnalysisError::VariableIsNotX => "KGFVariableIsNotX",
        }
    }
}

/// A feature, to name the ones that are too complex.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    Domain,
    Range,
    XIntercept,
    Minima,
    Maxima,
    InflectionPoints,
    VerticalAsymptotes,
    HorizontalAsymptotes,
    ObliqueAsymptotes,
    Monotonicity,
}

impl Feature {
    /// The key of the feature's title in Windows' resources.
    pub fn key(self) -> &'static str {
        match self {
            Feature::Domain => "Domain",
            Feature::Range => "Range",
            Feature::XIntercept => "XIntercept",
            Feature::Minima => "Minima",
            Feature::Maxima => "Maxima",
            Feature::InflectionPoints => "InflectionPoints",
            Feature::VerticalAsymptotes => "VerticalAsymptotes",
            Feature::HorizontalAsymptotes => "HorizontalAsymptotes",
            Feature::ObliqueAsymptotes => "ObliqueAsymptotes",
            Feature::Monotonicity => "Monotonicity",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parity {
    Odd,
    Even,
    Neither,
    Unknown,
}

impl Parity {
    pub fn key(self) -> &'static str {
        match self {
            Parity::Odd => "KGFParityOdd",
            Parity::Even => "KGFParityEven",
            Parity::Neither => "KGFParityNeither",
            Parity::Unknown => "KGFParityUnknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Increasing,
    Decreasing,
    Constant,
}

impl Direction {
    pub fn key(self) -> &'static str {
        match self {
            Direction::Increasing => "KGFMonotonicityIncreasing",
            Direction::Decreasing => "KGFMonotonicityDecreasing",
            Direction::Constant => "KGFMonotonicityConstant",
        }
    }
}

/// The period: shown only for periodic functions (Windows' `PeriodicityDirection` 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Period {
    None,
    Periodic(String),
}

/// The features, as the analysis panel shows them. An empty list or `None` gets the
/// feature's "does not have" message (or "Unable to calculate" for domain and range).
#[derive(Clone, Debug, PartialEq)]
pub struct Features {
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
    pub parity: Parity,
    pub period: Period,
    pub monotonicity: Vec<(String, Direction)>,
    pub too_complex: Vec<Feature>,
}

impl Features {
    fn empty() -> Features {
        Features {
            domain: None,
            range: None,
            x_intercepts: None,
            y_intercept: None,
            minima: Vec::new(),
            maxima: Vec::new(),
            inflection_points: Vec::new(),
            vertical_asymptotes: Vec::new(),
            horizontal_asymptotes: Vec::new(),
            oblique_asymptotes: Vec::new(),
            parity: Parity::Unknown,
            period: Period::None,
            monotonicity: Vec::new(),
            too_complex: Vec::new(),
        }
    }
}

/// Analyzes an equation in the angle unit `angle`.
pub fn analyze(equation: &Equation, angle: Angle) -> Result<Features, AnalysisError> {
    let Shape::Function(e) = &equation.shape else {
        return Err(AnalysisError::VariableIsNotX);
    };
    if equation.is_inequality() || !equation.variables.is_empty() {
        return Err(AnalysisError::NotSupported);
    }
    let env = Env { angle, vars: &[] };
    let fun = Fun::new(e, env);
    // A function without real values (√(−1), √(−1 − x²)) has no features to show. When
    // Windows' analysis fails, it shows `AnalysisCouldNotBePerformed`'s message, which isn't
    // among the strings here; "not supported" is the closest.
    if !e.has_x() {
        let c = fun.f(0.0);
        return if c.is_finite() {
            Ok(constant(c))
        } else {
            Err(AnalysisError::NotSupported)
        };
    }
    let mut features = match period(e, angle).and_then(|p| fun.verified_period(p)) {
        Some(p) => periodic(&fun, p),
        None => aperiodic(&fun).ok_or(AnalysisError::NotSupported)?,
    };
    features.parity = parity(&fun);
    if features.domain.is_none() {
        features.too_complex.insert(0, Feature::Domain);
    }
    Ok(features)
}

/// The features of a constant function with a real value `c`.
fn constant(c: f64) -> Features {
    let mut features = Features::empty();
    features.domain = Some("{x ∈ ℝ}".to_string());
    let value = exact::format(c);
    features.range = Some(format!("{{y = {value}}}"));
    features.y_intercept = Some(value);
    if c == 0.0 {
        features.too_complex.push(Feature::XIntercept);
    }
    features.parity = Parity::Even;
    features
        .monotonicity
        .push(("(−∞, ∞)".to_string(), Direction::Constant));
    features
}

/// The function with its first two derivatives.
struct Fun<'a> {
    e: &'a Expr,
    d1: Option<Expr>,
    d2: Option<Expr>,
    env: Env<'a>,
    /// Whether it jumps (floor, ceiling, sign) or has a factorial, whose features need
    /// more than derivatives.
    rough: bool,
}

impl<'a> Fun<'a> {
    fn new(e: &'a Expr, env: Env<'a>) -> Self {
        let d1 = e.derivative(env.angle);
        let d2 = d1.as_ref().and_then(|d| d.derivative(env.angle));
        let rough = e.any(&|n| {
            matches!(
                n,
                Expr::Call(Func::Floor | Func::Ceil | Func::Sign, _) | Expr::Factorial(_)
            )
        });
        Fun {
            e,
            d1,
            d2,
            env,
            rough,
        }
    }

    fn f(&self, x: f64) -> f64 {
        self.e.eval(x, 0.0, &self.env)
    }

    fn d1(&self, x: f64) -> f64 {
        self.d1
            .as_ref()
            .map_or(f64::NAN, |d| d.eval(x, 0.0, &self.env))
    }

    fn d2(&self, x: f64) -> f64 {
        self.d2
            .as_ref()
            .map_or(f64::NAN, |d| d.eval(x, 0.0, &self.env))
    }

    /// Whether f is defined at x. A pole that doubles land next to (tan(π/2) is 1.6e16)
    /// isn't.
    fn defined(&self, x: f64) -> bool {
        let v = self.f(x);
        v.is_finite() && v.abs() < 1e14
    }

    /// `p`, or the smallest part of it that is a period too (|sin x| has π).
    fn verified_period(&self, p: f64) -> Option<f64> {
        let tests: Vec<f64> = (0..23).map(|k| -7.3 + 0.731 * k as f64).collect();
        // f repeats if its values differ by no more than a billionth of how much f varies
        // (not of f itself: 10¹² + sin x would repeat at every shift), or by rounding.
        let values: Vec<f64> = tests
            .iter()
            .map(|&t| self.f(t))
            .filter(|v| v.is_finite())
            .collect();
        let spread = values.iter().fold(f64::NEG_INFINITY, |m: f64, &v| m.max(v))
            - values.iter().fold(f64::INFINITY, |m: f64, &v| m.min(v));
        let repeats = |q: f64| {
            let mut compared = 0;
            for &t in &tests {
                let (a, b) = (self.f(t), self.f(t + q));
                if a.is_finite() != b.is_finite() {
                    return false;
                }
                if a.is_finite() {
                    compared += 1;
                    let rounding = self.rounding(t).max(self.rounding(t + q));
                    // t + q is rounded too, and f's slope multiplies that: cot(10⁷x) − 200
                    // differs by 4·10⁻⁷ a period on, though it varies by less than 200.
                    let slope = |x: f64| {
                        let s = self.d1(x).abs();
                        if s.is_finite() { s } else { 0.0 }
                    };
                    let shift =
                        slope(t).max(slope(t + q)) * 4.0 * f64::EPSILON * (t + q).abs().max(1.0);
                    let tolerance = 1e-9 * spread.min(1.0 + a.abs()) + rounding + shift;
                    if (a - b).abs() > tolerance {
                        return false;
                    }
                }
            }
            compared > 5
        };
        if !repeats(p) {
            return None;
        }
        // A constant repeats at every shift, so p/12 would pass as well as p: keep p, with
        // which the holes of its domain repeat (tan x · cot x). So does a function whose
        // values change too little next to their rounding to tell: 6·10¹³ + sin x changes by
        // at most 0.52 over p/12, and 64 units in the last place of 6·10¹³ are 0.85.
        let rounding = tests.iter().fold(0.0, |m: f64, &t| m.max(self.rounding(t)));
        if self.constant_over(p).is_some() || 20.0 * rounding > spread {
            return Some(p);
        }
        (1..=12).rev().map(|k| p / k as f64).find(|&q| repeats(q))
    }

    /// The value of f if it's the same at the `golden_points` of the period `p` but for
    /// rounding: sin²x + cos²x, or tan x · cot x (whose holes the points miss), but not
    /// 10¹² + sin x or 1 + 10⁻¹⁰ sin x. The value is snapped within that rounding. A function
    /// that jumps (sign, floor …) is none: sign(sin x)² is 1 at all the points, but 0 at πn.
    /// Nor is one whose derivative isn't `level` at the points: 10¹⁵ + sin x − 10¹⁵ moves
    /// within the rounding of 10¹⁵, but its derivative is cos x.
    fn constant_over(&self, p: f64) -> Option<f64> {
        if self.rough {
            return None;
        }
        let (mut lo, mut hi, mut most) = (f64::INFINITY, f64::NEG_INFINITY, 0.0f64);
        for x in golden_points(p) {
            let v = self.f(x);
            if !v.is_finite() || self.level(x) != Some(true) {
                return None;
            }
            (lo, hi, most) = (lo.min(v), hi.max(v), most.max(self.rounding(x)));
        }
        // Halves first: lo + hi overflows next to 10³⁰⁸. Only the rounding snaps: within
        // `TIGHT`, 10⁻¹⁴(sin²x + cos²x) would be 0, which has no x-intercepts to list.
        let c = lo / 2.0 + hi / 2.0;
        (hi - lo <= most && c.is_finite()).then(|| exact::snap(c, most / c.abs().max(1.0)))
    }

    /// Whether rounding hides how f varies over the period `p`: its values at the
    /// `golden_points` are all within a unit in the last place of its largest term, though
    /// its derivative isn't `level` there (10¹⁶ + sin x − 10¹⁶ is 0 at all of them).
    fn hidden(&self, p: f64) -> bool {
        let (mut lo, mut hi, mut unit) = (f64::INFINITY, f64::NEG_INFINITY, 0.0f64);
        let mut moves = false;
        for x in golden_points(p) {
            let v = self.f(x);
            if !v.is_finite() {
                return false;
            }
            moves |= self.level(x) == Some(false);
            (lo, hi, unit) = (lo.min(v), hi.max(v), unit.max(self.rounding(x) / 64.0));
        }
        moves && hi - lo <= unit
    }

    /// Whether f′(x) is 0 but for its own rounding (64 units in the last place of its
    /// `magnitude`), or `None` if f′ isn't a number there.
    fn level(&self, x: f64) -> Option<bool> {
        let d1 = self.d1.as_ref()?;
        let slope = d1.eval(x, 0.0, &self.env);
        let rounding = 64.0 * f64::EPSILON * d1.magnitude(x, 0.0, &self.env);
        slope.is_finite().then_some(slope.abs() <= rounding)
    }

    /// How far rounding may set two values of f apart near x: 64 units in the last place of
    /// f's largest term there (10¹² for 10¹² + sin x, 1 for sin²x + cos²x).
    fn rounding(&self, x: f64) -> f64 {
        let size = summands(self.e)
            .iter()
            .fold(0.0, |m: f64, t| m.max(t.eval(x, 0.0, &self.env).abs()));
        64.0 * f64::EPSILON * size
    }
}

/// 400 points across the period `p`: p times the fractional parts of the multiples of the
/// golden ratio, not evenly spaced (at multiples of p/400 + p/800, sin x · sin 400x is 0
/// everywhere).
fn golden_points(p: f64) -> impl Iterator<Item = f64> {
    (1..=400).map(move |i| p * (i as f64 * 0.618_033_988_749_895).fract())
}

/// The terms of a sum, without their signs: sin²x and cos²x for sin²x + cos²x.
fn summands(e: &Expr) -> Vec<&Expr> {
    match e {
        Expr::Bin(BinOp::Add | BinOp::Sub, a, b) => {
            let mut terms = summands(a);
            terms.extend(summands(b));
            terms
        }
        Expr::Neg(a) => summands(a),
        _ => vec![e],
    }
}

// ------------------------------------------------------------------ period

/// The period of `e` if it is built from trigonometric functions of linear arguments
/// (and doesn't use x anywhere else): the least common multiple of their periods.
fn period(e: &Expr, angle: Angle) -> Option<f64> {
    fn walk(e: &Expr, angle: Angle) -> Result<Option<f64>, ()> {
        match e {
            Expr::X => Err(()),
            Expr::Call(f, arg) if f.takes_angle() => {
                if let Some(slope) = linear_slope(arg) {
                    let turn = 2.0 * std::f64::consts::PI / radians_per_unit(angle);
                    let base = if matches!(f, Func::Tan | Func::Cot) {
                        turn / 2.0
                    } else {
                        turn
                    };
                    return Ok(Some(base / slope.abs()));
                }
                walk(arg, angle)
            }
            Expr::Neg(a) | Expr::Call(_, a) | Expr::Factorial(a) => walk(a, angle),
            Expr::Bin(_, a, b) | Expr::Root(a, b) | Expr::LogBase(a, b) => {
                match (walk(a, angle)?, walk(b, angle)?) {
                    (Some(p), Some(q)) => lcm(p, q).map(Some).ok_or(()),
                    (p, q) => Ok(p.or(q)),
                }
            }
            _ => Ok(None),
        }
    }
    walk(e, angle).ok().flatten()
}

/// The slope a of a linear argument a·x + b with a ≠ 0.
fn linear_slope(arg: &Expr) -> Option<f64> {
    if !arg.has_x() {
        return None;
    }
    let at = |x: f64| arg.eval(x, 0.0, &Env::RADIANS);
    let (b, a) = (at(0.0), at(1.0) - at(0.0));
    if !a.is_finite() || a == 0.0 {
        return None;
    }
    [-3.7, -1.1, 2.3, 5.9, 17.0]
        .iter()
        .all(|&x| (at(x) - (a * x + b)).abs() <= 1e-9 * (1.0 + (a * x).abs()))
        .then_some(a)
}

/// The least common multiple of two periods whose ratio is a simple fraction.
fn lcm(p: f64, q: f64) -> Option<f64> {
    // The larger over the smaller, so that the order doesn't matter: 13 is 13/1, but
    // its inverse 1/13 has a denominator beyond the 12 tried.
    let (p, q) = (p.min(q), p.max(q));
    let ratio = q / p;
    (1..=12).find_map(|d| {
        let n = (ratio * d as f64).round();
        ((ratio * d as f64 - n).abs() < 1e-9 && n >= 1.0)
            .then(|| p * n / exact::gcd(n as i64, d) as f64)
    })
}

// ------------------------------------------------------------------ roots

/// The x values from −1000 to 1000 the aperiodic analysis samples: every 0.01 near
/// the origin, sparser further out.
fn grid() -> Vec<f64> {
    let mut right = vec![0.0];
    let mut x: f64 = 0.0;
    while x < 1000.0 {
        x += 0.01 * (x / 10.0).max(1.0);
        right.push(x);
    }
    let mut xs: Vec<f64> = right.iter().skip(1).rev().map(|x| -x).collect();
    xs.extend(right);
    xs
}

/// The points between consecutive samples where `g` changes sign, by bisection; and
/// the samples where it's 0. With `through_poles`, a change through infinity counts
/// too (a cusp of the derivative); otherwise `g` has to be small there, so poles and
/// jumps don't.
fn sign_changes(g: &dyn Fn(f64) -> f64, xs: &[f64], through_poles: bool) -> Vec<(f64, f64, f64)> {
    let values: Vec<f64> = xs.iter().map(|&x| g(x)).collect();
    let mut out = Vec::new();
    for i in 1..xs.len() {
        let (prev, cur) = ((xs[i - 1], values[i - 1]), (xs[i], values[i]));
        if through_poles && !cur.1.is_finite() {
            // A sample right on a cusp (x^(2/3)′ at 0).
            if let Some(&after) = values.get(i + 1)
                && prev.1.is_finite()
                && after.is_finite()
                && prev.1 != 0.0
                && after != 0.0
                && (prev.1 > 0.0) != (after > 0.0)
            {
                out.push((cur.0, prev.0, xs[i + 1]));
            }
        } else if cur.1 == 0.0 {
            // A sample right on a zero: a sign change if the neighbors differ.
            if let (Some(&before), Some(&after)) = (values.get(i - 1), values.get(i + 1))
                && before.is_finite()
                && after.is_finite()
                && (before > 0.0) != (after > 0.0)
                && before != 0.0
                && after != 0.0
            {
                out.push((cur.0, prev.0, xs[i + 1]));
            }
        } else if prev.1.is_finite()
            && cur.1.is_finite()
            && prev.1 != 0.0
            && (prev.1 > 0.0) != (cur.1 > 0.0)
        {
            let (mut a, mut b) = (prev, cur);
            for _ in 0..80 {
                let m = (a.0 + b.0) / 2.0;
                if m <= a.0 || m >= b.0 {
                    break;
                }
                let v = g(m);
                if v == 0.0 {
                    a = (m, v);
                    b = (m, v);
                    break;
                }
                if !v.is_finite() {
                    break;
                }
                if (v > 0.0) == (a.1 > 0.0) {
                    a = (m, v);
                } else {
                    b = (m, v);
                }
            }
            let c = (a.0 + b.0) / 2.0;
            // A zero, not a pole or a jump: g is small there, or keeps shrinking towards
            // it, as next to a vertical tangent (∛(x² − 2) is still 7.6e-6 at the double
            // closest to √2, from a radicand of 4.4e-16).
            let span = cur.0 - prev.0;
            let zero = || {
                g(c).abs() <= 1e-6 * (prev.1.abs() + cur.1.abs()) + 1e-12
                    || (shrinks(g, c, -span) && shrinks(g, c, span))
            };
            if through_poles || zero() {
                out.push((c, prev.0, cur.0));
            }
        }
    }
    out
}

/// Whether |`g`| shrinks towards `c` from `c + span`, as next to a zero with a vertical
/// tangent: at `c` it's less than half what it is a millionth of `span` away, which is less
/// than half what it is a thousandth away. Next to a pole it grows, and next to a jump it
/// levels off, so where g changes sign at `c`, c is a zero.
fn shrinks(g: &dyn Fn(f64) -> f64, c: f64, span: f64) -> bool {
    let (near, far) = (g(c + span * 1e-6).abs(), g(c + span * 1e-3).abs());
    g(c).abs() < 0.5 * near && near < 0.5 * far
}

/// Whether `f` tends to 0 at the end `x` of its domain, from the side of `side`: |f(x)| is
/// at most 1e-12, or its limit from inside is 0 to within the limit's own error. Next to a
/// vertical tangent, f(x) itself is far from the limit: √(x² − 2) is 2.1e-8 at the double
/// closest to √2, and root(x² − 2, 4) 1.5e-4.
///
/// Inside, f ≈ L + C·hᵖ + D·hᵖ⁺¹ at h = 2⁻⁴, 2⁻⁶, … 2⁻²⁴ of max(1, |x|) (rounded up to a
/// power of 2, so that x + h is exact), from the first h where f is defined all the way in.
/// The ratios of successive differences tell p, the exponent of a root, or another fraction
/// with a denominator up to 32; Richardson's extrapolation then removes hᵖ and hᵖ⁺¹. Further
/// in, rounding noise grows, so the two successive estimates of L that agree best give it,
/// and their difference its error: root(x² − 2, 4) + 10⁻⁸ comes out as 1.0004e-8 ± 4e-12.
/// L has to be 0 within three times that error (and 10⁻¹² of f, for an error that comes out
/// too small), and with it below the 10⁻⁹ the analysis shows as 0: 10⁶·root(x² − 2, 4) +
/// 10⁻⁶ comes out as 4.7e-6 ± 4e-6. And f(x) has to be what C·hᵖ makes of a few ulps:
/// √(x + 10⁻¹⁴) tends to 0 at −10⁻¹⁴, but it's 10⁻⁷ at 0.
fn tends_to_zero(f: &dyn Fn(f64) -> f64, x: f64, side: f64) -> bool {
    let at = f(x);
    if at.abs() <= 1e-12 {
        return true;
    }
    let scale = side * x.abs().max(1.0).log2().ceil().exp2();
    let r: f64 = 0.25;
    let all: Vec<f64> = (2..13).map(|k| f(x + scale * r.powi(k))).collect();
    let start = all
        .iter()
        .rposition(|y| !y.is_finite())
        .map_or(0, |i| i + 1);
    let v = &all[start..];
    if v.len() < 7 {
        return false;
    }
    let steps: Vec<f64> = v.windows(2).map(|w| w[1] - w[0]).collect();
    let exponent = |j: usize| fraction((steps[j + 1] / steps[j]).ln() / r.ln());
    let Some(p) = (0..steps.len() - 2).find_map(|j| {
        let p = exponent(j)?;
        (exponent(j + 1) == Some(p) && p < 1.0).then_some(p)
    }) else {
        return false;
    };
    let richardson = |s: &[f64], q: f64| -> Vec<f64> {
        s.windows(2)
            .map(|w| (w[1] - q * w[0]) / (1.0 - q))
            .collect()
    };
    let q = r.powf(p);
    let estimates = richardson(&richardson(v, q), q * r);
    let Some((error, limit)) = estimates
        .windows(2)
        .map(|w| ((w[1] - w[0]).abs(), w[0]))
        .min_by(|a, b| a.0.total_cmp(&b.0))
    else {
        return false;
    };
    let bound = 3.0 * error;
    let c = (v[0] - limit).abs() / (scale.abs() * r.powi(2 + start as i32)).powf(p);
    let rounding = c * (8.0 * f64::EPSILON * x.abs().max(1.0)).powf(p);
    limit.abs() <= bound + 1e-12 * v[0].abs().max(1.0)
        && limit.abs() + bound <= 1e-9
        && (at - limit).abs() <= rounding
}

/// The zeros of `g`: sign changes, and touching zeros (zeros of `dg` where g is 0, like
/// x² at 0). Runs of zeros, where a value underflows (e^(−x²) far out), don't count.
fn zeros(g: &dyn Fn(f64) -> f64, dg: Option<&dyn Fn(f64) -> f64>, xs: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = sign_changes(g, xs, false)
        .into_iter()
        .map(|c| c.0)
        .collect();
    if let Some(dg) = dg {
        for (c, a, b) in sign_changes(dg, xs, true) {
            let scale = g(a).abs().max(g(b).abs());
            if g(c).abs() <= 1e-9 * scale.max(1e-12) || g(c) == 0.0 {
                out.push(c);
            }
        }
    }
    dedupe(out)
}

/// The ends of the flat pieces of f, where f′ is exactly 0 at two samples or more in a
/// row (|x − 1| + |x + 1| on [−1, 1]), which `sign_changes` doesn't see. Each end comes
/// with whether it's a minimum (`Some(true)`), a maximum (`Some(false)`) or neither
/// (`None`, a step: f rises, or falls, on both sides of the piece). A side without a
/// slope doesn't count: where f′ is undefined (but for a cusp, past which the next
/// sample counts) or below 1e-12 (rounding noise, or e^(−x²)′ underflowing beyond
/// |x| ≈ 27), beyond a pole, and past the end of `xs`, where an `open` piece stays
/// flat; without `open`, pieces that reach the end of `xs` are left out.
fn flat_ends(fun: &Fun, xs: &[f64], open: bool) -> Vec<(f64, Option<bool>)> {
    let values: Vec<f64> = xs.iter().map(|&x| fun.d1(x)).collect();
    // The end between sample `slope`, where f′ isn't 0, and sample `flat`, by
    // bisection, with whether f rises at `slope`; `None` if that side doesn't count.
    let side = |slope: usize, flat: usize| {
        let mut v = values[slope];
        // A cusp has no slope at its sample (x^(1/3) − |x^(1/3)| at 0): take the next.
        if !v.is_finite() && fun.defined(xs[slope]) {
            let next = if slope < flat {
                slope.checked_sub(1)
            } else {
                Some(slope + 1).filter(|&k| k < xs.len())
            };
            v = next.map_or(f64::NAN, |k| values[k]);
        }
        if !v.is_finite() || v.abs() < 1e-12 {
            return None;
        }
        let (mut a, mut b) = (xs[slope], xs[flat]);
        for _ in 0..80 {
            let m = (a + b) / 2.0;
            if m == a || m == b {
                break;
            }
            if fun.d1(m) == 0.0 {
                b = m;
            } else {
                a = m;
            }
        }
        (fun.defined(a) && fun.defined(b)).then_some((b, v > 0.0))
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < xs.len() {
        if values[i] != 0.0 {
            i += 1;
            continue;
        }
        let first = i;
        while i + 1 < xs.len() && values[i + 1] == 0.0 {
            i += 1;
        }
        let last = i;
        i += 1;
        // A single zero is a turning point or a touching zero of f′ (x³ at 0); zeros
        // with slopes between them aren't flat either.
        if last == first
            || (!open && (first == 0 || last + 1 == xs.len()))
            || (first..last).any(|k| fun.d1((xs[k] + xs[k + 1]) / 2.0) != 0.0)
        {
            continue;
        }
        let before = (first > 0).then(|| side(first - 1, first)).flatten();
        let after = (last + 1 < xs.len())
            .then(|| side(last + 1, last))
            .flatten();
        let kind = match (before.map(|b| b.1), after.map(|a| a.1)) {
            (Some(false) | None, Some(true) | None) => Some(true),
            (Some(true) | None, Some(false) | None) => Some(false),
            _ => None,
        };
        out.extend(before.iter().chain(&after).map(|&(x, _)| (x, kind)));
    }
    out
}

fn dedupe(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() <= 1e-9 * a.abs().max(1.0));
    v
}

/// Whether `x` is one of `points`, as `dedupe` tells them apart.
fn among(points: &[f64], x: f64) -> bool {
    points
        .iter()
        .any(|&p| (p - x).abs() <= 1e-9 * x.abs().max(1.0))
}

/// The key points for the best fit: zeros of f, f′ and f″ from `lo` to `hi`.
pub fn key_points(
    f: &dyn Fn(f64) -> f64,
    derivative: Option<&Expr>,
    env: &Env,
    lo: f64,
    hi: f64,
) -> Vec<f64> {
    let xs: Vec<f64> = grid()
        .into_iter()
        .filter(|x| (lo..=hi).contains(x))
        .collect();
    let mut points: Vec<f64> = sign_changes(f, &xs, false)
        .into_iter()
        .map(|c| c.0)
        .collect();
    if let Some(d1) = derivative {
        let g = |x: f64| d1.eval(x, 0.0, env);
        points.extend(sign_changes(&g, &xs, true).into_iter().map(|c| c.0));
        if let Some(d2) = d1.derivative(env.angle) {
            let g = |x: f64| d2.eval(x, 0.0, env);
            points.extend(sign_changes(&g, &xs, true).into_iter().map(|c| c.0));
        }
    }
    dedupe(points)
}

// ------------------------------------------------------------------ domain

/// Where the expression is undefined: the zeros of these functions bound or punch the
/// domain (denominators, radicands, logarithms' arguments, cos of tan's argument …).
/// Each comes with whether f divides by it: its zeros are never in the domain, even where
/// f evaluates to a number by rounding (sin(2x)/cos(x) at π/2 is 1.2e-16 / 6.1e-17 = 2).
fn boundary_functions(e: &Expr, out: &mut Vec<(Expr, bool)>) {
    use Expr::*;
    let sub = |a: &Expr, b: f64| Bin(BinOp::Sub, Box::new(a.clone()), Box::new(Num(b)));
    let square = |a: &Expr| Bin(BinOp::Pow, Box::new(a.clone()), Box::new(Num(2.0)));
    match e {
        Bin(op, a, b) => {
            match op {
                BinOp::Div => out.push(((**b).clone(), true)),
                BinOp::Pow => out.push(((**a).clone(), false)),
                _ => {}
            }
            boundary_functions(a, out);
            boundary_functions(b, out);
        }
        Call(f, a) => {
            match f {
                Func::Tan | Func::Sec => out.push((Call(Func::Cos, a.clone()), true)),
                Func::Cot | Func::Csc => out.push((Call(Func::Sin, a.clone()), true)),
                Func::Asin | Func::Acos | Func::Atanh | Func::Acoth | Func::Asec | Func::Acsc => {
                    out.push((sub(&square(a), 1.0), false))
                }
                Func::Acosh | Func::Asech => {
                    out.push((sub(a, 1.0), false));
                    out.push(((**a).clone(), false));
                }
                Func::Coth | Func::Csch | Func::Acsch => out.push(((**a).clone(), true)),
                Func::Sqrt | Func::Ln | Func::Log => out.push(((**a).clone(), false)),
                _ => {}
            }
            boundary_functions(a, out);
        }
        Root(a, n) | LogBase(n, a) => {
            out.push(((**a).clone(), false));
            if matches!(e, LogBase(..)) {
                out.push(((**n).clone(), false));
                out.push((sub(n, 1.0), false));
            }
            boundary_functions(a, out);
            boundary_functions(n, out);
        }
        Neg(a) | Factorial(a) => boundary_functions(a, out),
        _ => {}
    }
}

/// An interval of the real line; an infinite end is open.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Interval {
    lo: f64,
    hi: f64,
    lo_closed: bool,
    hi_closed: bool,
}

/// The points where `fun` may start, stop or be punched out, and those of them where
/// it divides by 0.
fn domain_candidates(fun: &Fun, xs: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut functions: Vec<(Expr, bool)> = Vec::new();
    let mut found = Vec::new();
    boundary_functions(fun.e, &mut found);
    for (g, divides) in found {
        // A polynomial lists x once for each power.
        match functions.iter_mut().find(|(h, _)| *h == g) {
            Some(known) => known.1 |= divides,
            None => functions.push((g, divides)),
        }
    }
    let (mut candidates, mut divisions) = (Vec::new(), Vec::new());
    for (g, divides) in functions.iter().filter(|(g, _)| g.has_x()) {
        let dg = g.derivative(fun.env.angle);
        let gf = |x: f64| g.eval(x, 0.0, &fun.env);
        let dgf = |x: f64| dg.as_ref().map_or(f64::NAN, |d| d.eval(x, 0.0, &fun.env));
        let dgf: &dyn Fn(f64) -> f64 = &dgf;
        let zeros = zeros(&gf, dg.as_ref().map(|_| dgf), xs);
        if *divides {
            // Where g crosses 0 or is 0: x² + 2·10⁻¹⁴ only comes close at 0. A crossing
            // with a vertical tangent counts too, though g isn't small there (∛(x² − 2) is
            // 7.6e-6 at the double closest to √2): |g| dips there all the way in, a
            // millionth of a sample away to below 0.8 of what it is a sample away, and to
            // less still at the crossing. At a pole or a jump of g it doesn't, even where g
            // at the jump lies between its sides ((⌊x⌋ + ⌈x⌉)/2 + 0.1 is 0.1 at 0, −0.4 and
            // 0.6 next to it).
            let crossings: Vec<f64> = sign_changes(&gf, xs, true)
                .into_iter()
                .filter(|&(c, a, b)| {
                    let (at, span) = (gf(c).abs(), b - a);
                    let dips = |side: f64| {
                        let near = gf(c + side * span * 1e-6).abs();
                        near < 0.8 * gf(c + side * span).abs() && at < near
                    };
                    at <= 1e-6 * (gf(a).abs() + gf(b).abs()) + 1e-12 || (dips(-1.0) && dips(1.0))
                })
                .map(|c| c.0)
                .collect();
            divisions.extend(&crossings);
            divisions.extend(zeros.iter().filter(|&&x| gf(x) == 0.0));
            candidates.extend(crossings);
        }
        candidates.extend(zeros);
    }
    (dedupe(candidates), dedupe(divisions))
}

/// The size of the terms `e` adds up at x, which its roundoff is relative to.
fn terms(e: &Expr, x: f64, env: &Env) -> f64 {
    match e {
        Expr::Bin(BinOp::Add | BinOp::Sub, a, b) => terms(a, x, env) + terms(b, x, env),
        Expr::Neg(a) => terms(a, x, env),
        _ => e.eval(x, 0.0, env).abs(),
    }
}

/// Whether `g` is zero at x but for roundoff: within about an ulp of its terms and of what
/// rounding x (at least to the ulp of 1) changes it by.
fn vanishes(g: &Expr, x: f64, env: &Env) -> bool {
    let v = g.eval(x, 0.0, env);
    let slope = g
        .derivative(env.angle)
        .map_or(0.0, |d| d.eval(x, 0.0, env))
        .abs();
    let slope = if slope.is_finite() { slope } else { 0.0 };
    let roundoff = f64::EPSILON * (terms(g, x, env) + slope * x.abs().max(1.0));
    v.is_finite() && v.abs() <= roundoff
}

/// A point where the domain may start, stop or be punched out, moved to the exact value it
/// is near if a boundary function is zero there but for roundoff: x² − 2 is 4·10⁻¹⁶ at √2.
/// x − 1.0500000005 is 5·10⁻¹⁰ at 1.05, so that pole stays where it is (written as 1.05).
/// A short decimal has to make it exactly zero, as one typed into the equation does: a double
/// off, it can be on the other side of the end. A pole is moved exactly when f is still a
/// pole there: 1/(x − 100.00000000000001) is −7·10¹³ at 100, but tan(x − π/2 − 10⁻¹⁴) is
/// 10¹⁴ at 0.
fn snap_x(fun: &Fun, x: f64) -> f64 {
    let exact = exact::recognize(x, TIGHT);
    let exact_x = exact.value();
    if exact_x == x {
        return x;
    }
    let pole_at = |x: f64| fun.f(x).is_infinite() || fun.f(x).abs() >= 1e14;
    let zero = |g: &Expr| match exact {
        exact::Exact::Decimal(_) => g.eval(exact_x, 0.0, &fun.env) == 0.0,
        _ => vanishes(g, exact_x, &fun.env),
    };
    let mut functions = Vec::new();
    boundary_functions(fun.e, &mut functions);
    // Not where f is NaN: at the end of ln(x − 2.5000000001), f is −∞ at the root but
    // undefined at 2.5.
    let keeps = if pole_at(x) {
        pole_at(exact_x)
    } else {
        functions.iter().any(|(g, _)| g.has_x() && zero(g))
    };
    if keeps { exact_x } else { x }
}

/// Points snapped to the exact values they are near, without repeats. `recognize` is
/// slow: count the points first (tan(x²) has thousands of poles).
fn snapped(fun: &Fun, points: Vec<f64>) -> Vec<f64> {
    dedupe(points.into_iter().map(|c| snap_x(fun, c)).collect())
}

/// A point of one period (a root, turning point or inflection point) moved to the exact value
/// it is near, unless f is defined at only one of them. In a period f is defined but at its
/// poles: a root found next to one isn't moved onto it (cot(10⁷x) − 200 has its roots
/// 5·10⁻¹⁰ off them), nor a sign change at one onto a defined point next to it (1.05, 5·10⁻¹⁰
/// off the pole of 1/(sin x − sin 1.0500000005)).
fn snap_point(fun: &Fun, x: f64) -> f64 {
    let exact_x = exact::snap(x, TIGHT);
    if fun.defined(x) == fun.defined(exact_x) {
        exact_x
    } else {
        x
    }
}

/// The zeros of divisors that are exact values to rounding, snapped to them: f is left out
/// there even if it evaluates to a number. 1/(x + 10⁻¹⁰) divides by 0 at −10⁻¹⁰, which
/// snaps to 0, where f is 10¹⁰.
fn exact_zeros(divisions: Vec<f64>) -> Vec<f64> {
    let exact = divisions.into_iter().filter_map(|c| {
        let x = exact::snap(c, TIGHT);
        ((x - c).abs() <= 8.0 * f64::EPSILON * c.abs().max(1.0)).then_some(x)
    });
    dedupe(exact.collect())
}

/// The domain of an aperiodic function, as intervals, or `None` if it has too many
/// pieces to tell.
fn domain(fun: &Fun, xs: &[f64]) -> Option<Vec<Interval>> {
    let (points, divisions) = domain_candidates(fun, xs);
    if points.len() > 40 {
        return None;
    }
    let (points, divisions) = (snapped(fun, points), exact_zeros(divisions));
    let defined_between = |a: f64, b: f64| {
        let probes: Vec<f64> = match (a.is_finite(), b.is_finite()) {
            (true, true) => vec![a + (b - a) * 0.25, (a + b) / 2.0, a + (b - a) * 0.75],
            (false, true) => vec![b - 1.0, b - 10.0, b - 100.0],
            (true, false) => vec![a + 1.0, a + 10.0, a + 100.0],
            (false, false) => vec![-1.0, 0.5, 2.0],
        };
        probes.iter().filter(|&&x| fun.defined(x)).count() >= 2
    };
    let mut ends = vec![f64::NEG_INFINITY];
    ends.extend(&points);
    ends.push(f64::INFINITY);
    let mut intervals: Vec<Interval> = Vec::new();
    for pair in ends.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if !defined_between(a, b) {
            continue;
        }
        let closed = |x: f64| x.is_finite() && fun.defined(x) && !among(&divisions, x);
        let piece = Interval {
            lo: a,
            hi: b,
            lo_closed: closed(a),
            hi_closed: closed(b),
        };
        match intervals.last_mut() {
            Some(last) if last.hi == a && last.hi_closed => {
                last.hi = b;
                last.hi_closed = piece.hi_closed;
            }
            _ => intervals.push(piece),
        }
    }
    Some(intervals)
}

fn bound(v: f64) -> String {
    if v == f64::INFINITY {
        "∞".to_string()
    } else if v == f64::NEG_INFINITY {
        "−∞".to_string()
    } else {
        exact::format(v)
    }
}

fn interval_text(i: &Interval) -> String {
    if i.lo == i.hi {
        // A single value, which only a range has (x/|x| is −1, then 1).
        return format!("{{{}}}", bound(i.lo));
    }
    format!(
        "{}{}, {}{}",
        if i.lo_closed { '[' } else { '(' },
        bound(i.lo),
        bound(i.hi),
        if i.hi_closed { ']' } else { ')' }
    )
}

/// Whether intervals are the whole line but for single points, which they return.
fn line_without_points(intervals: &[Interval]) -> Option<Vec<f64>> {
    let first = intervals.first()?;
    let last = intervals.last()?;
    if first.lo != f64::NEG_INFINITY || last.hi != f64::INFINITY {
        return None;
    }
    intervals
        .windows(2)
        .map(|pair| {
            (pair[0].hi == pair[1].lo && !pair[0].hi_closed && !pair[1].lo_closed)
                .then_some(pair[0].hi)
        })
        .collect()
}

/// A set written for the variable `v`: `x ∈ ℝ`, `x ≠ 0`, `x > 0`, `x ∈ [−5, 5]`.
fn set_text(v: char, intervals: &[Interval]) -> String {
    let body = if let Some(points) = line_without_points(intervals) {
        if points.is_empty() {
            format!("{v} ∈ ℝ")
        } else if v == 'x' {
            let parts: Vec<String> = points
                .iter()
                .map(|p| format!("x ≠ {}", exact::format(*p)))
                .collect();
            parts.join(", ")
        } else {
            let parts: Vec<String> = points.iter().map(|p| exact::format(*p)).collect();
            format!("{v} ∈ ℝ∖{{{}}}", parts.join(", "))
        }
    } else if !intervals.is_empty() && intervals.iter().all(|i| i.lo == i.hi) {
        // Single values: `y = 1` like a constant's, or `y ∈ {−1, 1}`.
        let values: Vec<String> = intervals.iter().map(|i| exact::format(i.lo)).collect();
        match values.as_slice() {
            [value] => format!("{v} = {value}"),
            _ => format!("{v} ∈ {{{}}}", values.join(", ")),
        }
    } else if let [only] = intervals
        && v == 'x'
    {
        // Windows writes a domain's half-line as an inequality (x > 0), and a range's
        // as an interval (y ∈ (0, ∞)).
        match (only.lo.is_finite(), only.hi.is_finite()) {
            (true, false) => format!(
                "{v} {} {}",
                if only.lo_closed { '≥' } else { '>' },
                bound(only.lo)
            ),
            (false, true) => format!(
                "{v} {} {}",
                if only.hi_closed { '≤' } else { '<' },
                bound(only.hi)
            ),
            _ => format!("{v} ∈ {}", interval_text(only)),
        }
    } else {
        let parts: Vec<String> = intervals.iter().map(interval_text).collect();
        format!("{v} ∈ {}", parts.join(" ∪ "))
    };
    format!("{{{body}}}")
}

/// What `e` divides by: denominators, cos of tan's and sec's argument, sin of cot's and
/// csc's, the argument of coth, csch and acsch, ln of a logarithm's base (log(b, x) is
/// ln x/ln b), and the base of a negative power.
fn divisors(e: &Expr, env: &Env, out: &mut Vec<Expr>) {
    use Expr::*;
    match e {
        Bin(op, a, b) => {
            match op {
                BinOp::Div => out.push((**b).clone()),
                BinOp::Pow if !b.has_x() && b.eval(0.0, 0.0, env) < 0.0 => out.push((**a).clone()),
                _ => {}
            }
            divisors(a, env, out);
            divisors(b, env, out);
        }
        Call(f, a) => {
            match f {
                Func::Tan | Func::Sec => out.push(Call(Func::Cos, a.clone())),
                Func::Cot | Func::Csc => out.push(Call(Func::Sin, a.clone())),
                Func::Coth | Func::Csch | Func::Acsch => out.push((**a).clone()),
                _ => {}
            }
            divisors(a, env, out);
        }
        Root(a, n) | LogBase(n, a) => {
            if matches!(e, LogBase(..)) {
                out.push(Call(Func::Ln, n.clone()));
            }
            divisors(a, env, out);
            divisors(n, env, out);
        }
        Neg(a) | Factorial(a) => divisors(a, env, out),
        _ => {}
    }
}

/// Whether `fun` divides by 0 at `x`, though it may evaluate to a number there by rounding:
/// (x² − 2)/∛(x² − 2)² is ∛(x² − 2) but for holes at ±√2, and its divisor is 5.8e-11 at
/// the double closest to √2. A divisor is 0 at x, or tends to 0 there: |d| ≈ L + C·hᵖ at
/// h = 10⁻⁴, 10⁻⁶, 10⁻⁸ and 10⁻¹⁰ of max(1, |x|), with the same p from both ratios of
/// successive differences, and L, extrapolated from the last two values, is 0 to within
/// three times its change from the two before, plus what an ulp or so of x + h makes of
/// C·hᵖ (rounding). x² + 10⁻¹⁴ tends to 10⁻¹⁴ at 0, and 1 + 10000·root(|x − 1|, 32) to 1
/// at 1, though it is 5234 a billionth away.
fn divides_by_zero(fun: &Fun, x: f64) -> bool {
    let mut found = Vec::new();
    divisors(fun.e, &fun.env, &mut found);
    let h = |k: i32| x.abs().max(1.0) * 10f64.powi(-2 * k);
    found.iter().filter(|d| d.has_x()).any(|d| {
        let size = |x: f64| d.eval(x, 0.0, &fun.env).abs();
        size(x) == 0.0
            || [1.0, -1.0].iter().any(|&side| {
                let v: Vec<f64> = (2..6).map(|k| size(x + side * h(k))).collect();
                let steps: Vec<f64> = v.windows(2).map(|w| w[0] - w[1]).collect();
                let power = |i: usize| fraction((steps[i + 1] / steps[i]).ln() / 0.01f64.ln());
                let Some(p) = power(0).filter(|&p| power(1) == Some(p)) else {
                    return false;
                };
                let q = 0.01f64.powf(p);
                let limit = |i: usize| v[i + 1] - steps[i] * q / (1.0 - q);
                let ulps = 8.0 * ((x + side * h(5)).abs() * f64::EPSILON).max(f64::MIN_POSITIVE);
                let rounding = p * v[3] * ulps / h(5) / (1.0 - q);
                limit(2).abs() <= 3.0 * (limit(2) - limit(1)).abs() + rounding
            })
    })
}

// ------------------------------------------------------------------ limits

#[derive(Clone, Copy, Debug, PartialEq)]
enum Limit {
    Value(f64),
    PlusInfinity,
    MinusInfinity,
    Unknown,
}

/// The limit of the values `v`, taken ever closer to the point (or ever further out).
fn limit_of(v: &[f64]) -> Limit {
    if v.iter().any(|x| x.is_nan()) {
        return Limit::Unknown;
    }
    let last = v[v.len() - 1];
    if last.is_infinite() {
        return if last > 0.0 {
            Limit::PlusInfinity
        } else {
            Limit::MinusInfinity
        };
    }
    let steps: Vec<f64> = v.windows(2).map(|p| p[1] - p[0]).collect();
    let n = steps.len();
    let (first, final_step) = (steps[0], steps[n - 1]);
    if final_step.abs() <= 1e-7 * (1.0 + last.abs()) {
        return Limit::Value(exact::snap(last, LOOSE));
    }
    // Steps shrinking geometrically (like c/x) converge: add the rest of the series.
    let ratio = |k: usize| (steps[k] / steps[k - 1]).abs();
    if n >= 3 && ratio(n - 1) < 0.5 && ratio(n - 2) < 0.5 {
        let r = ratio(n - 1);
        let tail = final_step * r / (1.0 - r);
        return Limit::Value(exact::snap(last + tail, LOOSE));
    }
    let monotonic = steps.iter().all(|s| *s > 0.0) || steps.iter().all(|s| *s < 0.0);
    if monotonic && final_step.abs() >= 0.5 * first.abs() {
        return if final_step > 0.0 {
            Limit::PlusInfinity
        } else {
            Limit::MinusInfinity
        };
    }
    Limit::Unknown
}

/// The fraction k/n with n ≤ 32 that `p` is within 0.2% of, preferring small n.
fn fraction(p: f64) -> Option<f64> {
    if p.is_nan() || p <= 0.0 {
        return None;
    }
    (1..=32).find_map(|n| {
        let k = (p * n as f64).round();
        let near = k / n as f64;
        (k >= 1.0 && (near - p).abs() <= 2e-3 * p).then_some(near)
    })
}

/// The limit at x → ±∞ of `g`.
fn limit_at_infinity(g: &dyn Fn(f64) -> f64, sign: f64) -> Limit {
    let v: Vec<f64> = (1..=6).map(|k| g(sign * 10f64.powi(2 * k))).collect();
    limit_of(&v)
}

/// The last of values `v` that tend to a limit, with the rest of their steps added if those
/// shrink geometrically (like c/x): the limit, as `limit_of` takes it when they converge slowly.
fn extrapolated(v: &[f64]) -> f64 {
    let n = v.len();
    let (before, last) = (v[n - 2] - v[n - 3], v[n - 1] - v[n - 2]);
    let r = (last / before).abs();
    if r < 0.5 {
        v[n - 1] + last * r / (1.0 - r)
    } else {
        v[n - 1]
    }
}

/// The limit at x → ±∞ of `g`, for an end of the range. The limits at −∞ and +∞ of 1/x + e
/// are e ∓ 10⁻¹², the values 10¹² out: with the rest of the c/x added, they agree but for
/// rounding, and are one value. A gap stays (3·10⁻¹⁰ for 1/x + 0.00001234567 + atan(x)/10¹⁰).
fn end_limit_at_infinity(g: &dyn Fn(f64) -> f64, sign: f64) -> Limit {
    let limit = limit_at_infinity(g, sign);
    let settled = |s: f64| extrapolated(&[1e8, 1e10, 1e12].map(|x| g(s * x)));
    if let (Limit::Value(a), Limit::Value(b)) = (limit, limit_at_infinity(g, -sign))
        && a != b
    {
        let (a, b) = (settled(sign), settled(-sign));
        if (a - b).abs() <= 8.0 * f64::EPSILON * a.abs().max(b.abs()) {
            return Limit::Value(exact::snap((a + b) / 2.0, LOOSE));
        }
    }
    limit
}

/// The values of `fun` at 10⁻², 10⁻⁴, … 10⁻¹² of max(1, |p|) from `p` towards `side`.
fn approach(fun: &Fun, p: f64, side: f64) -> Vec<f64> {
    let scale = p.abs().max(1.0);
    (1..=6)
        .map(|k| fun.f(p + side * scale * 10f64.powi(-2 * k)))
        .collect()
}

/// The one-sided limit at `p` from the side of `side` (−1 left, 1 right). Next to a
/// hole, the closest values can be rounding noise (0/0 cancels there: (x² − 9)/(x + 3)
/// at −3 + 3e-12 is off by 3e-4); then the four furthest tell its value. Not at a pole,
/// where f is ±∞ but a small term can hide until the closest values (x + 10⁻¹⁴/x is
/// 1.01e-6 at 10⁻⁶ and 10⁻⁸, then 10⁻⁴ and 0.01), nor if those run off further than noise
/// or aren't numbers (√(x − 10⁻¹⁰) at 0, the end of its domain to rounding).
fn limit_at(fun: &Fun, p: f64, side: f64) -> Limit {
    let v = approach(fun, p, side);
    match limit_of(&v) {
        Limit::Unknown if !huge_at(fun, p) && !v.iter().any(|y| y.is_nan()) && !diverges(&v) => {
            match limit_of(&v[..4]) {
                Limit::Value(value) => Limit::Value(value),
                _ => Limit::Unknown,
            }
        }
        limit => limit,
    }
}

/// Whether the closest three of `approach`'s values run off to ±∞, by more than rounding
/// noise next to a hole does (a ten-thousandth of (2x² + 3x + 1)/(x + 1) at −1): next to a
/// pole whose term is small, or dominated further out by another one (1/sin x + 10⁶ sin x
/// is −31442, −3497 and −318313 at 10⁻², 10⁻⁴ and 10⁻⁶ of π past π). Their steps don't
/// shrink then, or shrink ever more slowly, like ln(−ln|sin x|)'s at 0 (by 0.78, then
/// 0.81). Next to a high root they shrink by the same ratio: root(x, 11) is 0.28, 0.19, 0.12
/// and 0.08 at 10⁻⁶ … 10⁻¹², on its way to 0.
fn diverges(v: &[f64]) -> bool {
    let step = |i: usize| (v[i + 1] - v[i]).abs();
    matches!(
        limit_of(&v[3..]),
        Limit::PlusInfinity | Limit::MinusInfinity
    ) && (v[5] - v[3]).abs() > 1e-3 * (1.0 + v[3].abs())
        && (v[5].is_infinite()
            || step(4) >= 0.95 * step(3)
            || step(4) / step(3) > step(3) / step(2) + 0.01)
}

/// Whether f is ±∞ at `x`, or too large for the domain (`Fun::defined`), as at a pole or
/// next to one (1/(x² + 10⁻¹⁴) is 10¹⁴ at 0).
fn huge_at(fun: &Fun, x: f64) -> bool {
    fun.f(x).abs() >= 1e14
}

/// Whether `x`, left out of the domain, is a hole: f tends to the same value there from
/// both sides, and is 0/0 or a rounded value at x itself. Not at a jump, where the limits
/// differ ((x − 500)/|x − 500| + (x − 500.04)² tends to −0.9984 and 1.0016 at 500). The
/// same to 2·10⁻⁵ of f: `limit_of` stops at a step of 10⁻⁷ of f, and the steps it leaves
/// out add up to a hundred times that where they shrink slowly (root(|x|, 99) at 0).
fn hole(fun: &Fun, x: f64) -> bool {
    !huge_at(fun, x)
        && matches!(
            [1.0, -1.0].map(|side| limit_at(fun, x, side)),
            [Limit::Value(a), Limit::Value(b)] if (a - b).abs() <= 2e-5 * (1.0 + a.abs().max(b.abs()))
        )
}

/// Whether `fun` tends to ±∞ at `x` from a side, by all six values of `approach` or by
/// the closest three.
fn pole(fun: &Fun, x: f64) -> bool {
    [1.0, -1.0].iter().any(|&side| {
        let v = approach(fun, x, side);
        matches!(limit_of(&v), Limit::PlusInfinity | Limit::MinusInfinity) || diverges(&v)
    })
}

/// The limit at `p` from the side of `side`, for an end of the range. The limits from both
/// sides of a hole are one value, but the values right next to it lose their digits (3·10⁻⁵
/// apart for (x − 0.001234)/(x² − 0.001234²)): taken 10⁻⁴ to 10⁻⁸ off it with the rest of
/// the c·(x − p) added, they agree to 10⁻⁹ of their size, and are one value. A jump stays.
fn end_limit(fun: &Fun, p: f64, side: f64) -> Limit {
    let limit = limit_at(fun, p, side);
    let scale = p.abs().max(1.0);
    let settled = |s: f64| extrapolated(&[1e-4, 1e-6, 1e-8].map(|d| fun.f(p + s * scale * d)));
    if let (Limit::Value(a), Limit::Value(b)) = (limit, limit_at(fun, p, -side))
        && a != b
    {
        let (a, b) = (settled(side), settled(-side));
        if (a - b).abs() <= 1e-9 * a.abs().max(b.abs()) {
            return Limit::Value(exact::snap((a + b) / 2.0, LOOSE));
        }
    }
    limit
}

// ------------------------------------------------------------------ text

fn point_text(x: f64, y: f64) -> String {
    format!("({}, {})", exact::format(x), exact::format(y))
}

/// The value of `fun` at a point: 0 at one of its `zeros` where f tends to 0 from each side
/// it's defined on. Next to a vertical tangent, f is far from 0 at the double closest to a
/// zero (∛(x² − 2) is 7.6e-6 at √2); but a point next to a zero needn't be on it:
/// x^(1/3) + 10⁻⁸ is 0 at −10⁻²⁴ but 10⁻⁸ at 0.
fn value_at(fun: &Fun, zeros: &[f64], x: f64) -> f64 {
    let f = |x: f64| fun.f(x);
    let step = 1e-9 * x.abs().max(1.0);
    let sides: Vec<f64> = [1.0, -1.0]
        .into_iter()
        .filter(|&side| f(x + side * step).is_finite())
        .collect();
    let on_zero = zeros.iter().any(|&z| (z - x).abs() <= step)
        && !sides.is_empty()
        && sides.iter().all(|&side| tends_to_zero(&f, x, side));
    if on_zero { 0.0 } else { f(x) }
}

/// The value of a limit, written for a range end.
fn limit_end(l: Limit) -> Option<f64> {
    match l {
        Limit::Value(v) => Some(v),
        Limit::PlusInfinity => Some(f64::INFINITY),
        Limit::MinusInfinity => Some(f64::NEG_INFINITY),
        Limit::Unknown => None,
    }
}

/// Merges ranges that overlap or touch.
fn union(mut pieces: Vec<Interval>) -> Vec<Interval> {
    pieces.sort_by(|a, b| a.lo.total_cmp(&b.lo).then(b.lo_closed.cmp(&a.lo_closed)));
    let mut out: Vec<Interval> = Vec::new();
    for p in pieces {
        if let Some(last) = out.last_mut() {
            let touches = p.lo < last.hi || (p.lo == last.hi && (last.hi_closed || p.lo_closed));
            if touches {
                if p.hi > last.hi {
                    last.hi = p.hi;
                    last.hi_closed = p.hi_closed;
                } else if p.hi == last.hi {
                    last.hi_closed |= p.hi_closed;
                }
                continue;
            }
        }
        out.push(p);
    }
    out
}

/// One value range end with whether it is attained.
#[derive(Clone, Copy)]
struct End {
    value: f64,
    attained: bool,
}

/// The values on one piece of the domain: from the lowest to the highest of its ends.
fn span(ends: &[End]) -> Option<Interval> {
    if ends.is_empty() {
        return None;
    }
    let lo = ends.iter().map(|e| e.value).fold(f64::INFINITY, f64::min);
    let hi = ends
        .iter()
        .map(|e| e.value)
        .fold(f64::NEG_INFINITY, f64::max);
    // Constant on the piece (x/x): the value is taken inside it.
    let attained = |v: f64| lo == hi || ends.iter().any(|e| e.value == v && e.attained);
    Some(Interval {
        lo,
        hi,
        lo_closed: attained(lo) && lo.is_finite(),
        hi_closed: attained(hi) && hi.is_finite(),
    })
}

// ------------------------------------------------------------------ aperiodic

/// Features that need the derivatives (or none of floor's jumps).
const SMOOTH_FEATURES: [Feature; 6] = [
    Feature::Range,
    Feature::XIntercept,
    Feature::Minima,
    Feature::Maxima,
    Feature::InflectionPoints,
    Feature::Monotonicity,
];

/// The features of a function that isn't periodic, or `None` if it is defined on no
/// interval (`√(−1 − x²)` nowhere, `√(−x²)` only at 0).
fn aperiodic(fun: &Fun) -> Option<Features> {
    let mut features = Features::empty();
    let xs = grid();
    let Some(intervals) = domain(fun, &xs) else {
        features.too_complex.extend(SMOOTH_FEATURES);
        return Some(features);
    };
    if intervals.is_empty() {
        return None;
    }
    features.domain = Some(set_text('x', &intervals));
    if intervals.iter().any(|i| contains(i, 0.0)) && fun.defined(0.0) {
        features.y_intercept = Some(exact::format(fun.f(0.0)));
    }
    if fun.rough || fun.d1.is_none() {
        features.too_complex.extend(SMOOTH_FEATURES);
        features.too_complex.extend([
            Feature::VerticalAsymptotes,
            Feature::HorizontalAsymptotes,
            Feature::ObliqueAsymptotes,
        ]);
        return Some(features);
    }
    let gaps = domain_gaps(&intervals);
    asymptotes(fun, &intervals, &xs, &gaps, &mut features);
    let holes: Vec<f64> = gaps.iter().copied().filter(|&x| hole(fun, x)).collect();

    let inside = |x: f64| intervals.iter().any(|i| x > i.lo && x < i.hi);
    // No turning or inflection points in the noise next to a hole, nor where the domain
    // leaves a point out (f″ of 1/∛(x² − 2) changes sign at its poles).
    let clear = |x: f64| inside(x) && fun.defined(x) && !next_to(&holes, x) && !among(&gaps, x);
    let f = |x: f64| fun.f(x);
    let d1 = |x: f64| fun.d1(x);
    let d2 = |x: f64| fun.d2(x);
    let slope: &dyn Fn(f64) -> f64 = &d1;

    // Zeros, also at the ends of the domain (√(25 − x²) at ±5, and √(x² − 2) at ±√2,
    // which is 2.1e-8 at the double closest to √2 but tends to 0). Exact values are looked
    // for only when there are few (sin(x)/x has hundreds).
    let mut roots: Vec<f64> = zeros(&f, Some(slope), &xs)
        .into_iter()
        .filter(|&x| inside(x) && fun.defined(x) && !among(&holes, x))
        .collect();
    // Not where f divides by 0: (x² − 2)/∛(x² − 2)² tends to 0 at ±√2, holes.
    roots.retain(|&x| !divides_by_zero(fun, x));
    for i in &intervals {
        for (end, closed, side) in [(i.lo, i.lo_closed, 1.0), (i.hi, i.hi_closed, -1.0)] {
            if closed && tends_to_zero(&f, end, side) && !divides_by_zero(fun, end) {
                roots.push(end);
            }
        }
    }
    let roots = if roots.len() > 20 {
        features.too_complex.push(Feature::XIntercept);
        Vec::new()
    } else {
        dedupe(roots.iter().map(|&x| exact::snap(x, TIGHT)).collect())
    };
    if !roots.is_empty() {
        let parts: Vec<String> = roots.iter().map(|x| exact::format(*x)).collect();
        features.x_intercepts = Some(parts.join(", "));
    }
    let y = |x: f64| value_at(fun, &roots, x);

    // Turning points, where f′ changes sign (through ∞ at a cusp), and the ends of flat
    // pieces; `steps` are the ends of the flat pieces that f rises (or falls) across. A
    // turning point is a minimum if f falls at the sample before it (right before it, f
    // may be flat).
    let mut turns: Vec<(f64, bool)> = sign_changes(&d1, &xs, true)
        .into_iter()
        .map(|(c, before, _)| (c, d1(before) < 0.0))
        .filter(|&(c, _)| clear(c))
        .collect();
    let mut steps = Vec::new();
    for (c, minimum) in flat_ends(fun, &xs, true)
        .into_iter()
        .filter(|&(c, _)| inside(c))
    {
        match minimum {
            Some(minimum) => turns.push((c, minimum)),
            None => steps.push(c),
        }
    }
    turns.sort_by(|a, b| a.0.total_cmp(&b.0));
    let too_many_turns = turns.len() + steps.len() > 20;
    if !too_many_turns {
        for turn in &mut turns {
            turn.0 = exact::snap(turn.0, TIGHT);
        }
        // The ends of flat pieces snap like the turns (`snapped` only snaps where f stays
        // defined, which is for domain boundaries).
        steps = dedupe(steps.into_iter().map(|c| exact::snap(c, TIGHT)).collect());
        // Snapped onto a point where f isn't defined, a turn is at an end of the domain (f′
        // of ln(2 − x²) changes sign at ±√2) or next to a hole, not an extremum.
        turns.retain(|t| fun.defined(t.0));
        turns.dedup_by(|a, b| (a.0 - b.0).abs() <= 1e-9 * a.0.abs().max(1.0));
    }
    if too_many_turns {
        features.too_complex.extend([
            Feature::Range,
            Feature::Minima,
            Feature::Maxima,
            Feature::Monotonicity,
        ]);
    } else {
        for &(c, minimum) in &turns {
            let text = point_text(c, y(c));
            if minimum {
                features.minima.push(text);
            } else {
                features.maxima.push(text);
            }
        }
        // The closed ends of the domain are extrema too.
        for i in &intervals {
            for (end, closed, side) in [(i.lo, i.lo_closed, 1.0), (i.hi, i.hi_closed, -1.0)] {
                if !closed {
                    continue;
                }
                let slope = d1(end + side * 1e-9 * end.abs().max(1.0));
                if slope == 0.0 || slope.is_nan() {
                    continue;
                }
                let text = point_text(end, y(end));
                if (slope > 0.0) == (side > 0.0) {
                    features.minima.push(text);
                } else {
                    features.maxima.push(text);
                }
            }
        }
    }

    let bends: Vec<f64> = sign_changes(&d2, &xs, true)
        .into_iter()
        .map(|c| c.0)
        .filter(|&c| clear(c))
        .collect();
    // Without f″ (too large to use), the bends can't be told.
    if bends.len() > 20 || fun.d2.is_none() {
        features.too_complex.push(Feature::InflectionPoints);
    } else {
        // Snapped and without repeats (#62), not where f isn't defined (#61), and with the
        // value at a zero with a vertical tangent written as 0 (`y`, #63).
        features.inflection_points = dedupe(bends.iter().map(|&c| exact::snap(c, TIGHT)).collect())
            .into_iter()
            .filter(|&c| fun.defined(c))
            .map(|c| point_text(c, y(c)))
            .collect();
    }

    if too_many_turns {
        return Some(features);
    }

    // The range: on each piece of the domain, from its lowest to its highest value
    // (at the turning points, the closed ends, or in the limit).
    let mut pieces = Vec::new();
    let mut known = true;
    for i in &intervals {
        let mut ends: Vec<End> = turns
            .iter()
            .filter(|(c, _)| *c > i.lo && *c < i.hi)
            .map(|&(c, _)| End {
                value: exact::snap(y(c), TIGHT),
                attained: true,
            })
            .collect();
        for (x, closed, side) in [(i.lo, i.lo_closed, 1.0), (i.hi, i.hi_closed, -1.0)] {
            if closed {
                ends.push(End {
                    value: exact::snap(y(x), TIGHT),
                    attained: true,
                });
                continue;
            }
            let limit = if x.is_finite() {
                end_limit(fun, x, side)
            } else {
                end_limit_at_infinity(&f, if x > 0.0 { 1.0 } else { -1.0 })
            };
            match limit_end(limit) {
                Some(value) => ends.push(End {
                    value,
                    attained: false,
                }),
                None => known = false,
            }
        }
        pieces.extend(span(&ends));
    }
    if known {
        let pieces = union(pieces);
        features.range = Some(set_text('y', &pieces));
    } else {
        features.too_complex.push(Feature::Range);
    }

    // Monotonicity between the turning points and steps.
    for i in &intervals {
        let inner = turns.iter().map(|t| t.0).chain(steps.iter().copied());
        let mut cuts = vec![i.lo];
        cuts.extend(dedupe(inner.filter(|&c| c > i.lo && c < i.hi).collect()));
        cuts.push(i.hi);
        for pair in cuts.windows(2) {
            let direction = direction_between(fun, pair[0], pair[1]);
            let text = format!("({}, {})", bound(pair[0]), bound(pair[1]));
            features.monotonicity.push((text, direction));
        }
    }
    Some(features)
}

/// The finite ends that the pieces of a domain leave out: its holes, poles and open edges.
fn domain_gaps(intervals: &[Interval]) -> Vec<f64> {
    intervals
        .iter()
        .flat_map(|i| [(i.lo, i.lo_closed), (i.hi, i.hi_closed)])
        .filter_map(|(x, closed)| (x.is_finite() && !closed).then_some(x))
        .collect()
}

/// Whether `x` is within 1e-4 (relative) of one of the `holes`. Next to a hole, f′ and f″
/// are rounding noise, and so is f closer still (0/0 cancels there).
fn next_to(holes: &[f64], x: f64) -> bool {
    holes
        .iter()
        .any(|&h| (x - h).abs() <= 1e-4 * h.abs().max(1.0))
}

fn contains(i: &Interval, x: f64) -> bool {
    (x > i.lo || (x == i.lo && i.lo_closed)) && (x < i.hi || (x == i.hi && i.hi_closed))
}

/// Whether f rises, falls or stays between two points (one may be infinite).
fn direction_between(fun: &Fun, a: f64, b: f64) -> Direction {
    let probe = match (a.is_finite(), b.is_finite()) {
        (true, true) => vec![a + (b - a) * 0.3, (a + b) / 2.0, a + (b - a) * 0.7],
        (false, true) => vec![b - 0.5, b - 5.0, b - 50.0],
        (true, false) => vec![a + 0.5, a + 5.0, a + 50.0],
        (false, false) => vec![-1.3, 0.4, 2.1],
    };
    let slopes: Vec<f64> = probe
        .iter()
        .map(|&x| fun.d1(x))
        .filter(|s| s.is_finite())
        .collect();
    // A slope of 0 votes for neither (f64::signum(0.0) is 1).
    let vote = |s: &f64| (*s > 0.0) as i32 - (*s < 0.0) as i32;
    if slopes.iter().all(|s| s.abs() < 1e-12) {
        Direction::Constant
    } else if slopes.iter().map(vote).sum::<i32>() > 0 {
        Direction::Increasing
    } else {
        Direction::Decreasing
    }
}

/// The limit of values taken further out by the same factor each time that tend to it like
/// two geometric series at once, as c + a/x + b/x² does: `extrapolated` twice over. That only
/// refines `extrapolated` from every other value, so it stands if it moves that by less than
/// that moved the last value; else the values aren't in such a tail yet (100/x + 10⁷/x² turns
/// between 10⁴ and 10⁸) and once over stands. With the limit, how much the last pass moved it,
/// a bound on what is left of the tail. `None` if a value isn't finite.
fn twice_extrapolated(v: [f64; 5]) -> Option<(f64, f64)> {
    if !v.iter().all(|v| v.is_finite()) {
        return None;
    }
    let once = extrapolated(&[v[0], v[2], v[4]]);
    let twice = extrapolated(&[0, 1, 2].map(|i| extrapolated(&v[i..i + 3])));
    let first = (once - v[4]).abs();
    Some(if (twice - once).abs() <= first {
        (twice, (twice - once).abs())
    } else {
        (once, first)
    })
}

/// The vertical, horizontal and oblique asymptotes, with the samples `xs` and the `gaps`
/// of the domain.
fn asymptotes(
    fun: &Fun,
    intervals: &[Interval],
    xs: &[f64],
    gaps: &[f64],
    features: &mut Features,
) {
    let f = |x: f64| fun.f(x);
    let infinite = |l: Limit| matches!(l, Limit::PlusInfinity | Limit::MinusInfinity);
    let mut vertical = Vec::new();
    for i in intervals {
        if i.lo.is_finite() && !i.lo_closed && infinite(limit_at(fun, i.lo, 1.0)) {
            vertical.push(i.lo);
        }
        if i.hi.is_finite() && !i.hi_closed && infinite(limit_at(fun, i.hi, -1.0)) {
            vertical.push(i.hi);
        }
    }
    features.vertical_asymptotes = dedupe(vertical)
        .iter()
        .map(|x| format!("x = {}", exact::format(*x)))
        .collect();

    let mut horizontal = Vec::new();
    let mut oblique = Vec::new();
    // A line isn't its own asymptote, nor is a line with holes (x − 2 with one at 3), but
    // nothing with a pole is a line (5 + 10⁻¹⁶/(x − 1000) has y = 5, though it's 5 to the
    // last bits further than 10 from 1000). On a line, f(x) − mx is c and f′(x) is m at
    // every sample of the domain, apart from the rounding noise within 1% of its gaps.
    // Fewer samples could miss a bump (1 + e^(−(x − 10)²) has y = 1 both ways), and the
    // values alone one that is small next to the line (10⁹ + e^(−x²) is within a billionth
    // of 10⁹, but its slope isn't 0). A constant (x/x) comes out to the last bits, while
    // 1/(x² + 10⁹), which has y = 0, is never more than 10⁻⁹ from 0.
    let no_poles = gaps.iter().all(|&g| !huge_at(fun, g) && !pole(fun, g));
    let is_line = |m: f64, c: f64| {
        no_poles
            && xs
                .iter()
                .filter(|&&x| {
                    fun.defined(x)
                        && gaps
                            .iter()
                            .all(|&g| (x - g).abs() > 1e-2 * g.abs().max(1.0))
                })
                .all(|&x| {
                    let (value, slope) = (fun.f(x), fun.d1(x));
                    let off = if m == 0.0 {
                        4.0 * f64::EPSILON * c.abs()
                    } else {
                        1e-9 * (1.0 + c.abs() + (m * x).abs())
                    };
                    (value - m * x - c).abs() <= off
                        && (!slope.is_finite() || (slope - m).abs() <= 1e-6 * (1.0 + m.abs()))
                })
    };
    let reaches = |sign: f64| {
        intervals.iter().any(|i| {
            if sign > 0.0 {
                i.hi == f64::INFINITY
            } else {
                i.lo == f64::NEG_INFINITY
            }
        })
    };
    for sign in [-1.0, 1.0] {
        if !reaches(sign) {
            continue;
        }
        match limit_at_infinity(&f, sign) {
            // With the rest of its c/x added, as for the range: 10¹² out, 1000/x + 1.05 is
            // 1.05 ± 10⁻⁹ at the two ends, two values to `dedupe`. A constant (x/x) isn't its
            // own asymptote.
            Limit::Value(v) if !is_line(0.0, v) => {
                let w = [1e8, 1e10, 1e12].map(|x| f(sign * x));
                horizontal.push(exact::snap(extrapolated(&w), LOOSE));
            }
            Limit::PlusInfinity | Limit::MinusInfinity => {
                let slope = |x: f64| fun.f(x) / x;
                if !matches!(limit_at_infinity(&slope, sign), Limit::Value(_)) {
                    continue;
                }
                // f(x)/x is m + c/x + …, and the c/x is taken away (the points are 100 times
                // apart): 10¹² out, f(x)/x of 1.05x + 10 + 1/x is 1.05 + 10⁻¹¹, which would
                // leave f − mx 10⁻³ off at 10⁸.
                let (near, far) = (slope(sign * 1e10), slope(sign * 1e12));
                let m = exact::snap(far + (far - near) / 99.0, LOOSE);
                if m == 0.0 {
                    continue;
                }
                let rest = |x: f64| fun.f(x) - m * x;
                // Up to 10⁸: further out, f(x) − mx loses its digits.
                let v: Vec<f64> = (1..=4).map(|k| rest(sign * 10f64.powi(2 * k))).collect();
                let Limit::Value(c) = limit_of(&v) else {
                    continue;
                };
                // A line isn't its own asymptote: f − mx is the same everywhere (c, estimated
                // 10⁸ out, is only that close to it). At a hole, where f − mx is the same on both
                // sides, it is taken next to it: (x² − 6.25)/(x + 2.5) is a line too, but
                // x + (x + 2.5)/|x + 2.5| isn't.
                let same = |a: f64, b: f64| (a - b).abs() <= 1e-9 * (1.0 + b.abs());
                let beside = |x: f64| {
                    let (left, right) = (rest(x - 1e-3), rest(x + 1e-3));
                    let hole = fun.f(x).is_nan() && same(left, right);
                    if hole { right } else { rest(x) }
                };
                let r = [-2.5, -0.5, 0.75, 3.0].map(beside);
                let line_here =
                    r.iter().all(|&v| same(v, r[0])) && (r[0] - c).abs() <= LOOSE * (1.0 + c.abs());
                // Nor is a line that f is on at every sample apart from its holes (`is_line`).
                if !line_here && !is_line(m, c) {
                    // With the rest of its tail added too, twice over: once, from 10⁴, 10⁶ and
                    // 10⁸, the 1005000/x² of 5000/(x − 201) would leave 10⁻⁶.
                    let w = [1e4, 1e5, 1e6, 1e7, 1e8].map(|x| rest(sign * x));
                    let c = twice_extrapolated(w).map_or_else(|| extrapolated(&v[1..]), |(c, _)| c);
                    oblique.push((m, c, f(sign * 1e8).abs()));
                }
            }
            Limit::Value(_) | Limit::Unknown => {}
        }
    }
    features.horizontal_asymptotes = dedupe(horizontal)
        .iter()
        .map(|v| format!("y = {}", exact::format_loose(*v)))
        .collect();
    // The intercepts at −∞ and +∞ are estimated 10⁸ out, where f(x) − mx keeps only the
    // digits that mx leaves (2x + 0.05 + 0.1/x gives 0.05 ± 10⁻⁸). Within that rounding, they
    // are one value if they read alike, or if f(x) − f(−x) − 2mx, with the rest of its tail
    // added, is 0 nearer in, but for its rounding there and what is left of the tail: from 10⁴
    // to 10⁶, where it is rounded 100 times less, and from 10⁵ to 10⁷, where the tail is
    // smaller. 2x + 0.0100002 + 10⁻⁷·x/|x| keeps its two, and so does 10⁷x + (x/|x| + 1)/16,
    // 1/8 apart, which 10⁷ out is in the rounding. (Written as the same exact value, they are
    // within the rounding too: ±10⁻⁶ that snap to 0.)
    let within = |a: f64, b: f64, size: f64| (a - b).abs() <= 8.0 * f64::EPSILON * size;
    let snapped = |c: f64| exact::snap(c, LOOSE);
    if let [(m0, c0, size0), (m1, c1, size1)] = oblique[..]
        && (m0 - m1).abs() < 1e-9
        && (within(c0, c1, size0.max(size1)) || within(snapped(c0), snapped(c1), size0.max(size1)))
    {
        let no_gap_from = |k: f64| {
            let points = [0.0, 0.5, 1.0, 1.5, 2.0].map(|i| 10f64.powf(k + i));
            let x = points[4];
            let rounding = 8.0 * f64::EPSILON * f(x).abs().max(f(-x).abs());
            twice_extrapolated(points.map(|x| f(x) - f(-x) - (m0 + m1) * x))
                .is_some_and(|(gap, tail)| gap.abs() <= rounding + tail)
        };
        let alike = exact::format_loose(c0) == exact::format_loose(c1);
        if alike || (no_gap_from(4.0) && no_gap_from(5.0)) {
            oblique = vec![(m0, (c0 + c1) / 2.0, size0)];
        }
    }
    oblique.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);
    features.oblique_asymptotes = oblique
        .iter()
        .map(|&(m, c, _)| {
            let mut s = format!("y = {}", exact::coefficient_of_x(m));
            let c = exact::snap(c, LOOSE);
            if c != 0.0 {
                // c with its own signs: |c| after a minus would need parentheses when it
                // has two terms (x − (√2 − 1)).
                s.push_str(&exact::plus_term(&exact::format_loose(c)));
            }
            s
        })
        .collect();
}

// ------------------------------------------------------------------ periodic

/// `step·n₁ + offset`: `πn₁`, `2πn₁ + π/2`, `360n₁ + 90`. The offset of a pole is written as
/// the exact value it is near (`πn₁` for one 10⁻¹⁰ off 0). That of a root, a turning point or
/// an inflection point (`point`) only if it is that value to the roundoff, as it isn't if it
/// was kept off a pole next to it: the roots of cot(10⁷x) − 200, 5·10⁻¹⁰ off the poles, or
/// of tan(x) − 10¹⁰, 10⁻¹⁰ before them, would be written at them.
fn family_term(step: f64, offset: f64, point: bool) -> String {
    let n = match exact::recognize(step, TIGHT) {
        exact::Exact::Rational(p, q) => times_n(p, q, ""),
        exact::Exact::Pi(p, q) => times_n(p, q, "π"),
        other => format!("{}n₁", exact::format_exact(other)),
    };
    let exact = exact::recognize(offset, TIGHT);
    let is_exact = (exact.value() - offset).abs() <= exact::ROUNDOFF * offset.abs().max(1.0);
    let offset_text = if is_exact || !point {
        exact::format_exact(exact)
    } else {
        match exact::decimal(offset) {
            text if text == "0" => format!("{offset:e}"),
            text => text,
        }
    };
    if offset == 0.0 || offset_text == "0" {
        n
    } else {
        n + &exact::plus_term(&offset_text)
    }
}

/// `pπn₁/q`, without the ones.
fn times_n(p: i64, q: i64, symbol: &str) -> String {
    let mut s = String::new();
    if p == -1 {
        s.push('−');
    } else if p != 1 {
        s.push_str(&p.to_string().replace('-', "−"));
    }
    s.push_str(symbol);
    s.push_str("n₁");
    if q != 1 {
        s.push('/');
        s.push_str(&q.to_string());
    }
    s
}

/// Points in one period [0, p), as families: the largest k with the points repeating
/// every p/k (and `same` holding between repeats), and the points of the first p/k.
fn families(points: &[f64], p: f64, same: &dyn Fn(usize, usize) -> bool) -> (f64, Vec<usize>) {
    let n = points.len();
    for k in (1..=n).rev() {
        if !n.is_multiple_of(k) {
            continue;
        }
        let m = n / k;
        let step = p / k as f64;
        let repeats = (0..n - m)
            .all(|i| (points[i + m] - points[i] - step).abs() <= 1e-7 * p && same(i, i + m));
        if repeats {
            return (step, (0..m).collect());
        }
    }
    (p, (0..n).collect())
}

/// Points of [0, p) found on a slightly larger window, moved by `snap` to the exact values
/// they are near, without repeats.
fn in_period(mut points: Vec<f64>, p: f64, snap: &dyn Fn(f64) -> f64) -> Vec<f64> {
    for x in &mut points {
        let mut v = x.rem_euclid(p);
        if (p - v).abs() <= 1e-9 * p {
            v = 0.0;
        }
        *x = snap(v);
        if *x >= p {
            *x = 0.0;
        }
    }
    dedupe(points)
}

fn periodic(fun: &Fun, p: f64) -> Features {
    let mut features = Features::empty();
    features.period = Period::Periodic(exact::format(p));
    // A uniform grid over a little more than one period.
    const N: usize = 4000;
    let xs: Vec<f64> = (0..=N + 2)
        .map(|i| -p / N as f64 + p * i as f64 / N as f64)
        .collect();
    let f = |x: f64| fun.f(x);
    let d1 = |x: f64| fun.d1(x);
    let d2 = |x: f64| fun.d2(x);

    // The domain: ℝ but for poles (tan's π/2 + πn) and the zeros of denominators (the
    // holes of sin(2x)/cos(x)). Gaps can't be written here. As on the grid of `aperiodic`,
    // more points in a period than it writes (40 families of points out of the domain, 20
    // zeros, turning points or inflection points) are too complex. Only candidates out of the
    // domain count: the 42 zeros of sin(21x)² are no gaps in √(sin(21x)²) + sin x. So do
    // poles that rounding keeps below `defined`'s bound, where f is far larger than right
    // beside them (tan 45x reaches 3·10¹³ at some), and the zeros of denominators, where f
    // may evaluate to a number by rounding. Regularly spaced ones are one family, as
    // `families` writes them: tan 45x + tan x has 45 poles a period but one family, and
    // tan 50x + tan x 51 that aren't one. They're counted before `in_period` looks for their
    // exact values: `recognize` is slow, and more than 400 are too many to look up.
    let out_of_domain = |x: f64| {
        let h = 1e-6 * x.abs().max(1.0);
        let beside = fun.f(x - h).abs() + fun.f(x + h).abs();
        !fun.defined(x) || fun.f(x).abs() > 1e6 * beside
    };
    let (candidates, divisions) = domain_candidates(fun, &xs);
    let candidates: Vec<f64> = candidates
        .into_iter()
        .filter(|&x| out_of_domain(x) || among(&divisions, x))
        .collect();
    let rough_period: Vec<f64> = dedupe(
        candidates
            .iter()
            .map(|&x| {
                let v = x.rem_euclid(p);
                if (p - v).abs() <= 1e-9 * p { 0.0 } else { v }
            })
            .collect(),
    );
    if families(&rough_period, p, &|_, _| true).1.len() > 40 || rough_period.len() > 400 {
        // Nor are the poles among them: no vertical asymptotes would be wrong.
        features.too_complex.extend(SMOOTH_FEATURES);
        features.too_complex.push(Feature::VerticalAsymptotes);
        if fun.defined(0.0) {
            features.y_intercept = Some(exact::format(fun.f(0.0)));
        }
        return features;
    }
    let divisions = in_period(exact_zeros(divisions), p, &|x| exact::snap(x, TIGHT));
    let excluded: Vec<f64> = in_period(snapped(fun, candidates), p, &|x| snap_x(fun, x))
        .into_iter()
        .filter(|&x| !fun.defined(x) || among(&divisions, x))
        .collect();
    if (0..400).any(|i| !fun.f(p * (i as f64 + 0.5) / 400.0).is_finite()) {
        features.too_complex.extend(SMOOTH_FEATURES);
        return features;
    }
    let holes: Vec<f64> = excluded.iter().copied().filter(|&x| hole(fun, x)).collect();
    let defined = |x: f64| fun.defined(x) && !among(&excluded, x);
    // Next to a hole, f′ and f″ are rounding noise, as in `aperiodic`.
    let clear = |x: f64| {
        defined(x)
            && holes.iter().all(|&e| {
                let d = (x - e).abs();
                d.min(p - d) > 1e-4 * e.abs().max(1.0)
            })
    };
    if defined(0.0) {
        features.y_intercept = Some(exact::format(fun.f(0.0)));
    }
    if excluded.is_empty() {
        features.domain = Some("{x ∈ ℝ}".to_string());
    } else {
        let (step, firsts) = families(&excluded, p, &|_, _| true);
        let parts: Vec<String> = firsts
            .iter()
            .map(|&i| format!("x ≠ {}", family_term(step, excluded[i], false)))
            .collect();
        features.domain = Some(format!("{{{}, ∀n₁ ∈ ℤ}}", parts.join(", ")));
        // The poles, told apart from the holes before they're grouped: sin(x)/sin(2x) has
        // holes at πn and poles at πn + π/2, all in the family πn/2.
        let poles: Vec<f64> = excluded
            .iter()
            .copied()
            .filter(|&x| !among(&holes, x) && pole(fun, x))
            .collect();
        if !poles.is_empty() {
            let (step, firsts) = families(&poles, p, &|_, _| true);
            features.vertical_asymptotes = firsts
                .iter()
                .map(|&i| format!("x = {}, n₁ ∈ ℤ", family_term(step, poles[i], false)))
                .collect();
        }
    }
    // A constant (sin²x + cos²x, tan x · cot x) has no smallest period, and its derivatives
    // are only rounding noise, whose sign changes would be taken for hundreds of extrema:
    // it's analyzed like y = c, between the holes of its domain.
    if let Some(c) = fun.constant_over(p) {
        features.period = Period::None;
        features.range = Some(format!("{{y = {}}}", exact::format(c)));
        if features.y_intercept.is_some() {
            features.y_intercept = Some(exact::format(c));
        }
        if c == 0.0 {
            features.too_complex.push(Feature::XIntercept);
        }
        features.monotonicity = monotonicity(excluded, p, &|_, _| Direction::Constant);
        return features;
    }
    // Nor do derivatives help where rounding hides f's values (10¹⁶ + sin x − 10¹⁶ is 0).
    if fun.rough || fun.d1.is_none() || fun.hidden(p) {
        features.too_complex.extend(SMOOTH_FEATURES);
        return features;
    }
    let slope: &dyn Fn(f64) -> f64 = &d1;
    let snap = |x: f64| snap_point(fun, x);

    let roots = zeros(&f, Some(slope), &xs);
    let mut roots: Vec<f64> = if count_in_period(&roots, p) > 20 {
        features.too_complex.push(Feature::XIntercept);
        Vec::new()
    } else {
        in_period(roots, p, &snap)
            .into_iter()
            .filter(|&x| fun.defined(x) && !among(&holes, x))
            .collect()
    };
    // Not where f divides by 0: sin(x)/∛(sin x)² tends to 0 at πn, holes.
    roots.retain(|&x| !divides_by_zero(fun, x));
    if !roots.is_empty() {
        let (step, firsts) = families(&roots, p, &|_, _| true);
        let parts: Vec<String> = firsts
            .iter()
            .map(|&i| family_term(step, roots[i], true))
            .collect();
        features.x_intercepts = Some(format!("{{{}, n₁ ∈ ℤ}}", parts.join(", ")));
    }
    let y = |x: f64| value_at(fun, &roots, x);

    // Turning points and their kind (as in `aperiodic`), in one period, and the ends of
    // flat pieces (|sin x| + sin x on [π, 2π]), found on three periods so that every
    // piece is seen whole. None in the noise next to a hole (`clear`). More than 20 in a
    // period are too complex, counted before `in_period` looks for their exact values, and
    // again with the ends of the flat pieces.
    let changes = sign_changes(&d1, &xs, true);
    let positions: Vec<f64> = changes.iter().map(|c| c.0).collect();
    let mut too_many_turns = count_in_period(&positions, p) > 20;
    let mut turns: Vec<(f64, bool)> = Vec::new();
    let mut steps = Vec::new();
    if !too_many_turns {
        turns = changes
            .into_iter()
            .map(|(c, before, _)| (in_period(vec![c], p, &snap)[0], d1(before) < 0.0))
            .filter(|&(x, _)| clear(x))
            .collect();
        let wide: Vec<f64> = (0..=3 * N)
            .map(|i| p * (i as f64 / N as f64 - 1.0))
            .collect();
        for (x, minimum) in flat_ends(fun, &wide, false) {
            let x = in_period(vec![x], p, &snap)[0];
            match minimum {
                Some(minimum) => turns.push((x, minimum)),
                None => steps.push(x),
            }
        }
        turns.sort_by(|a, b| a.0.total_cmp(&b.0));
        turns.dedup_by(|a, b| (a.0 - b.0).abs() <= 1e-9 * p);
        too_many_turns = turns.len() + steps.len() > 20;
    }
    if too_many_turns {
        features.too_complex.extend([
            Feature::Range,
            Feature::Minima,
            Feature::Maxima,
            Feature::Monotonicity,
        ]);
        turns.clear();
        steps.clear();
    }
    for minimum in [true, false] {
        let points: Vec<f64> = turns
            .iter()
            .filter(|t| t.1 == minimum)
            .map(|t| t.0)
            .collect();
        if points.is_empty() {
            continue;
        }
        let values: Vec<f64> = points.iter().map(|&x| exact::snap(y(x), TIGHT)).collect();
        let (step, firsts) = families(&points, p, &|a, b| (values[a] - values[b]).abs() < 1e-9);
        let texts: Vec<String> = firsts
            .iter()
            .map(|&i| {
                format!(
                    "{{({}, {}), n₁ ∈ ℤ}}",
                    family_term(step, points[i], true),
                    exact::format(values[i])
                )
            })
            .collect();
        if minimum {
            features.minima = texts;
        } else {
            features.maxima = texts;
        }
    }

    let bends: Vec<f64> = sign_changes(&d2, &xs, true)
        .into_iter()
        .map(|c| c.0)
        .collect();
    let too_many_bends = count_in_period(&bends, p) > 20;
    let bends: Vec<f64> = if too_many_bends {
        Vec::new()
    } else {
        in_period(bends, p, &snap)
            .into_iter()
            .filter(|&x| clear(x))
            .collect()
    };
    if fun.d2.is_none() || too_many_bends {
        features.too_complex.push(Feature::InflectionPoints);
    } else if !bends.is_empty() {
        let values: Vec<f64> = bends.iter().map(|&x| exact::snap(y(x), TIGHT)).collect();
        let (step, firsts) = families(&bends, p, &|a, b| (values[a] - values[b]).abs() < 1e-9);
        features.inflection_points = firsts
            .iter()
            .map(|&i| {
                format!(
                    "{{({}, {}), n₁ ∈ ℤ}}",
                    family_term(step, bends[i], true),
                    exact::format(values[i])
                )
            })
            .collect();
    }

    if too_many_turns {
        return features;
    }

    // The range: on each piece of the period between the points outside the domain (all of
    // it if there are none), from the lowest to the highest of its turning values and the
    // limits at its ends, as on the pieces of an aperiodic domain. One interval for the
    // whole period would fill the gap between the branches of sec x, which has no values in
    // (−1, 1). A turning value at a zero with a vertical tangent is 0 (`y`).
    let turning = |x: f64| End {
        value: exact::snap(y(x), TIGHT),
        attained: true,
    };
    let mut pieces = Vec::new();
    let mut known = true;
    if excluded.is_empty() {
        let ends: Vec<End> = turns.iter().map(|t| turning(t.0)).collect();
        pieces.extend(span(&ends));
    }
    for (i, &a) in excluded.iter().enumerate() {
        // To the next point: after the last one, the first one of the next period.
        let next = excluded.get(i + 1).copied().unwrap_or(excluded[0]);
        let b = if next > a { next } else { next + p };
        let mut ends: Vec<End> = turns
            .iter()
            .filter(|t| (t.0 > a && t.0 < b) || (t.0 + p > a && t.0 + p < b))
            .map(|t| turning(t.0))
            .collect();
        for (x, side) in [(a, 1.0), (next, -1.0)] {
            match limit_end(limit_at(fun, x, side)) {
                Some(value) => ends.push(End {
                    value,
                    attained: false,
                }),
                None => known = false,
            }
        }
        pieces.extend(span(&ends));
    }
    // A function with turning points takes more than one value: one means the others were
    // lost (1 + 10⁻¹⁴ sin x's extremes snap to 1; 10¹⁴ + sin x's maximum is too large for
    // `defined`). Without turning points, it's a constant between holes.
    let lo = pieces.iter().map(|i| i.lo).fold(f64::INFINITY, f64::min);
    let hi = pieces
        .iter()
        .map(|i| i.hi)
        .fold(f64::NEG_INFINITY, f64::max);
    let lost = lo == hi && !turns.is_empty();
    if known && !pieces.is_empty() && !lost {
        features.range = Some(set_text('y', &union(pieces)));
    } else {
        features.too_complex.push(Feature::Range);
    }

    // Monotonicity between the turning points, steps and poles.
    let cuts: Vec<f64> = turns
        .iter()
        .map(|t| t.0)
        .chain(excluded.iter().copied())
        .chain(steps)
        .collect();
    features.monotonicity = monotonicity(cuts, p, &|a, b| direction_between(fun, a, b));
    features
}

/// Monotonicity between `cuts` (turning points and poles of one period [0, p)), from the
/// first one at or after 0, around one period, as families; `direction` tells how f goes
/// between two cuts.
fn monotonicity(
    cuts: Vec<f64>,
    p: f64,
    direction: &dyn Fn(f64, f64) -> Direction,
) -> Vec<(String, Direction)> {
    let cuts = dedupe(cuts);
    if cuts.is_empty() {
        return vec![("(−∞, ∞)".to_string(), direction(0.0, p))];
    }
    let n = cuts.len();
    let pieces: Vec<(f64, f64, Direction)> = (0..n)
        .map(|i| {
            let a = cuts[i];
            let b = if i + 1 < n { cuts[i + 1] } else { cuts[0] + p };
            (a, b, direction(a, b))
        })
        .collect();
    let (step, firsts) = families(&cuts, p, &|a, b| pieces[a].2 == pieces[b].2);
    firsts
        .iter()
        .map(|&i| {
            let (a, b, direction) = pieces[i];
            let text = format!(
                "{{({}, {}), n₁ ∈ ℤ}}",
                family_term(step, a, false),
                family_term(step, b, false)
            );
            (text, direction)
        })
        .collect()
}

/// How many of `points`, found on a window a little larger than one period, lie in the
/// period [0, p).
fn count_in_period(points: &[f64], p: f64) -> usize {
    points.iter().filter(|&&x| (0.0..p).contains(&x)).count()
}

// ------------------------------------------------------------------ parity

fn parity(fun: &Fun) -> Parity {
    let (mut even, mut odd, mut compared) = (true, true, 0);
    for k in 0..24 {
        let t = 0.173 * 1.6f64.powi(k);
        let (a, b) = (fun.f(t), fun.f(-t));
        if a.is_finite() != b.is_finite() {
            return Parity::Neither;
        }
        if !a.is_finite() {
            continue;
        }
        compared += 1;
        let tolerance = 1e-9 * (1.0 + a.abs() + b.abs());
        even &= (a - b).abs() <= tolerance;
        odd &= (a + b).abs() <= tolerance;
    }
    match (compared > 3, even, odd) {
        (false, _, _) => Parity::Unknown,
        (true, true, _) => Parity::Even,
        (true, false, true) => Parity::Odd,
        (true, false, false) => Parity::Neither,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphing::Syntax;

    fn analyze_text(text: &str) -> Features {
        let equation = Equation::parse(text, Syntax::default()).unwrap();
        analyze(&equation, Angle::Radians).unwrap()
    }

    fn directions(f: &Features) -> Vec<(&str, Direction)> {
        f.monotonicity
            .iter()
            .map(|(s, d)| (s.as_str(), *d))
            .collect()
    }

    // The functions of Windows' manual tests (docs/ManualTests.md, "Key Graph Features").

    #[test]
    fn line() {
        let f = analyze_text("y=x");
        assert_eq!(f.domain.as_deref(), Some("{x ∈ ℝ}"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ}"));
        assert_eq!(f.x_intercepts.as_deref(), Some("0"));
        assert_eq!(f.y_intercept.as_deref(), Some("0"));
        assert!(f.minima.is_empty() && f.maxima.is_empty() && f.inflection_points.is_empty());
        assert!(f.vertical_asymptotes.is_empty() && f.horizontal_asymptotes.is_empty());
        assert!(f.oblique_asymptotes.is_empty());
        assert_eq!(f.parity, Parity::Odd);
        assert_eq!(directions(&f), vec![("(−∞, ∞)", Direction::Increasing)]);
        assert_eq!(f.period, Period::None);
        assert!(f.too_complex.is_empty());
    }

    #[test]
    fn reciprocal() {
        let f = analyze_text("y=1/x");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ 0}"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ∖{0}}"));
        assert_eq!(f.x_intercepts, None);
        assert_eq!(f.y_intercept, None);
        assert!(f.minima.is_empty() && f.maxima.is_empty() && f.inflection_points.is_empty());
        assert_eq!(f.vertical_asymptotes, vec!["x = 0"]);
        assert_eq!(f.horizontal_asymptotes, vec!["y = 0"]);
        assert!(f.oblique_asymptotes.is_empty());
        assert_eq!(f.parity, Parity::Odd);
        assert_eq!(
            directions(&f),
            vec![
                ("(−∞, 0)", Direction::Decreasing),
                ("(0, ∞)", Direction::Decreasing)
            ]
        );
    }

    #[test]
    fn parabola() {
        let f = analyze_text("y=x^2");
        assert_eq!(f.domain.as_deref(), Some("{x ∈ ℝ}"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ [0, ∞)}"));
        assert_eq!(f.x_intercepts.as_deref(), Some("0"));
        assert_eq!(f.y_intercept.as_deref(), Some("0"));
        assert_eq!(f.minima, vec!["(0, 0)"]);
        assert!(f.maxima.is_empty() && f.inflection_points.is_empty());
        assert_eq!(f.parity, Parity::Even);
        assert_eq!(
            directions(&f),
            vec![
                ("(−∞, 0)", Direction::Decreasing),
                ("(0, ∞)", Direction::Increasing)
            ]
        );
    }

    #[test]
    fn cubic() {
        let f = analyze_text("y=x^3");
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ}"));
        assert_eq!(f.inflection_points, vec!["(0, 0)"]);
        assert!(f.minima.is_empty() && f.maxima.is_empty());
        assert_eq!(f.parity, Parity::Odd);
        assert_eq!(directions(&f), vec![("(−∞, ∞)", Direction::Increasing)]);
    }

    #[test]
    fn exponential() {
        let f = analyze_text("y=e^x");
        assert_eq!(f.range.as_deref(), Some("{y ∈ (0, ∞)}"));
        assert_eq!(f.x_intercepts, None);
        assert_eq!(f.y_intercept.as_deref(), Some("1"));
        assert_eq!(f.horizontal_asymptotes, vec!["y = 0"]);
        assert_eq!(f.parity, Parity::Neither);
        assert_eq!(directions(&f), vec![("(−∞, ∞)", Direction::Increasing)]);
    }

    #[test]
    fn logarithm() {
        let f = analyze_text("y=ln(x)");
        assert_eq!(f.domain.as_deref(), Some("{x > 0}"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ}"));
        assert_eq!(f.x_intercepts.as_deref(), Some("1"));
        assert_eq!(f.y_intercept, None);
        assert_eq!(f.vertical_asymptotes, vec!["x = 0"]);
        assert!(f.horizontal_asymptotes.is_empty());
        assert_eq!(f.parity, Parity::Neither);
        assert_eq!(directions(&f), vec![("(0, ∞)", Direction::Increasing)]);
    }

    #[test]
    fn sine() {
        let f = analyze_text("y=sin(x)");
        assert_eq!(f.domain.as_deref(), Some("{x ∈ ℝ}"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ [−1, 1]}"));
        assert_eq!(f.x_intercepts.as_deref(), Some("{πn₁, n₁ ∈ ℤ}"));
        assert_eq!(f.y_intercept.as_deref(), Some("0"));
        assert_eq!(f.maxima, vec!["{(2πn₁ + π/2, 1), n₁ ∈ ℤ}"]);
        assert_eq!(f.minima, vec!["{(2πn₁ + 3π/2, −1), n₁ ∈ ℤ}"]);
        assert_eq!(f.inflection_points, vec!["{(πn₁, 0), n₁ ∈ ℤ}"]);
        assert!(f.vertical_asymptotes.is_empty());
        assert_eq!(f.parity, Parity::Odd);
        assert_eq!(f.period, Period::Periodic("2π".to_string()));
        assert_eq!(
            directions(&f),
            vec![
                ("{(2πn₁ + π/2, 2πn₁ + 3π/2), n₁ ∈ ℤ}", Direction::Decreasing),
                (
                    "{(2πn₁ + 3π/2, 2πn₁ + 5π/2), n₁ ∈ ℤ}",
                    Direction::Increasing
                ),
            ]
        );
    }

    #[test]
    fn cosine() {
        let f = analyze_text("y=cos(x)");
        assert_eq!(f.x_intercepts.as_deref(), Some("{πn₁ + π/2, n₁ ∈ ℤ}"));
        assert_eq!(f.y_intercept.as_deref(), Some("1"));
        assert_eq!(f.maxima, vec!["{(2πn₁, 1), n₁ ∈ ℤ}"]);
        assert_eq!(f.minima, vec!["{(2πn₁ + π, −1), n₁ ∈ ℤ}"]);
        assert_eq!(f.inflection_points, vec!["{(πn₁ + π/2, 0), n₁ ∈ ℤ}"]);
        assert_eq!(f.parity, Parity::Even);
        assert_eq!(
            directions(&f),
            vec![
                ("{(2πn₁, 2πn₁ + π), n₁ ∈ ℤ}", Direction::Decreasing),
                ("{(2πn₁ + π, 2πn₁ + 2π), n₁ ∈ ℤ}", Direction::Increasing),
            ]
        );
    }

    #[test]
    fn tangent() {
        let f = analyze_text("y=tan(x)");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ πn₁ + π/2, ∀n₁ ∈ ℤ}"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ}"));
        assert_eq!(f.x_intercepts.as_deref(), Some("{πn₁, n₁ ∈ ℤ}"));
        assert_eq!(f.y_intercept.as_deref(), Some("0"));
        assert!(f.minima.is_empty() && f.maxima.is_empty());
        assert_eq!(f.inflection_points, vec!["{(πn₁, 0), n₁ ∈ ℤ}"]);
        assert_eq!(f.vertical_asymptotes, vec!["x = πn₁ + π/2, n₁ ∈ ℤ"]);
        assert_eq!(f.parity, Parity::Odd);
        assert_eq!(f.period, Period::Periodic("π".to_string()));
        assert_eq!(
            directions(&f),
            vec![("{(πn₁ + π/2, πn₁ + 3π/2), n₁ ∈ ℤ}", Direction::Increasing)]
        );
    }

    #[test]
    fn semicircle() {
        let f = analyze_text("y=sqrt(25-x^2)");
        assert_eq!(f.domain.as_deref(), Some("{x ∈ [−5, 5]}"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ [0, 5]}"));
        assert_eq!(f.x_intercepts.as_deref(), Some("−5, 5"));
        assert_eq!(f.y_intercept.as_deref(), Some("5"));
        assert_eq!(f.maxima, vec!["(0, 5)"]);
        assert_eq!(f.minima, vec!["(−5, 0)", "(5, 0)"]);
        assert!(f.inflection_points.is_empty());
        assert_eq!(f.parity, Parity::Even);
        assert_eq!(
            directions(&f),
            vec![
                ("(−5, 0)", Direction::Increasing),
                ("(0, 5)", Direction::Decreasing)
            ]
        );
    }

    #[test]
    fn vertical_tangent_zeros() {
        // Next to √2 the radicand is a rounding error, 4.4e-16, whose cube root is 7.6e-6:
        // far more than a millionth of the values around it, which a zero needed.
        for text in ["y=cbrt(x^2-2)", "y=root(x^2-2,3)", "y=root(x^2-2,5)"] {
            let f = analyze_text(text);
            assert_eq!(f.x_intercepts.as_deref(), Some("−√2, √2"), "{text}");
            assert_eq!(f.inflection_points, vec!["(−√2, 0)", "(√2, 0)"], "{text}");
        }
        let f = analyze_text("y=cbrt(x^3-2x)");
        assert_eq!(f.x_intercepts.as_deref(), Some("−√2, 0, √2"));
        assert_eq!(f.inflection_points, vec!["(−√2, 0)", "(0, 0)", "(√2, 0)"]);
        // At the ends of the domain.
        let f = analyze_text("y=sqrt(x^2-2)");
        assert_eq!(f.x_intercepts.as_deref(), Some("−√2, √2"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ [0, ∞)}"));
        assert_eq!(f.minima, vec!["(−√2, 0)", "(√2, 0)"]);
        // √x + 0.00001 shrinks towards 0.00001 there, not 0.
        let f = analyze_text("y=sqrt(x)+0.00001");
        assert_eq!(f.x_intercepts, None);
        assert_eq!(f.minima, vec!["(0, 0.00001)"]);
        assert_eq!(f.range.as_deref(), Some("{y ∈ [0.00001, ∞)}"));
        // √(x² − 2) + 10⁻⁸ is 3.1e-8 at the double closest to √2, and tends to 10⁻⁸ there.
        let f = analyze_text("y=sqrt(x^2-2)+10^(-8)");
        assert_eq!(f.x_intercepts, None);
        assert_eq!(f.minima.len(), 2);
        assert!(
            f.minima.iter().all(|m| !m.ends_with(", 0)")),
            "{:?}",
            f.minima
        );
        // Higher roots tend to their limit more slowly: root(x² − 2, 4) is 1.5e-4 at the
        // double closest to √2, and root(x² − 2, 4) + 10⁻⁸ tends to 10⁻⁸.
        for text in ["y=root(x^2-2,4)", "y=root(x^2-2,6)"] {
            let f = analyze_text(text);
            assert_eq!(f.x_intercepts.as_deref(), Some("−√2, √2"), "{text}");
            assert_eq!(f.minima, vec!["(−√2, 0)", "(√2, 0)"], "{text}");
        }
        for text in ["y=root(x^2-2,4)+10^(-8)", "y=root(x^2-2,6)+10^(-8)"] {
            let f = analyze_text(text);
            assert_eq!(f.x_intercepts, None, "{text}");
            assert!(f.minima.iter().all(|m| !m.ends_with(", 0)")), "{text}");
        }
        // 10⁶ times as large, the limit comes out as 4.7e-6 ± 4e-6: no telling 10⁻⁶ from 0.
        let f = analyze_text("y=10^6 root(x^2-2,4)+10^(-6)");
        assert_eq!(f.x_intercepts, None);
        // An end at 0 can be off by rounding too: (√2)² − 2 is 4.4e-16.
        let f = analyze_text("y=sqrt((x+sqrt(2))^2-2)");
        assert_eq!(f.x_intercepts.as_deref(), Some("−2√2, 0"));
        // A point next to a zero needn't be on it: √x − 10⁻⁸ is 0 at 10⁻¹⁶ but −10⁻⁸ at 0,
        // and x^(1/3) + 10⁻⁸ is 0 at −10⁻²⁴ but 10⁻⁸ at 0.
        let f = analyze_text("y=sqrt(x)-10^(-8)");
        assert_eq!(f.minima, vec!["(0, −0.00000001)"]);
        let f = analyze_text("y=x^(1/3)+10^(-8)");
        assert_eq!(f.inflection_points, vec!["(0, 0.00000001)"]);
        // √(x + 10⁻¹⁴) is 0 where its domain starts, at −10⁻¹⁴, which is written as 0 like
        // the domain ({x ≥ 0}, `snap_x`).
        let f = analyze_text("y=sqrt(x+10^(-14))");
        assert_eq!(f.x_intercepts.as_deref(), Some("0"));
        // cos(π/2) is 6.1e-17 in doubles.
        let f = analyze_text("y=cbrt(cos(x))");
        assert_eq!(f.x_intercepts.as_deref(), Some("{πn₁ + π/2, n₁ ∈ ℤ}"));
        assert_eq!(f.inflection_points, vec!["{(πn₁ + π/2, 0), n₁ ∈ ℤ}"]);
        // sin π is 1.2e-16 in doubles, but sin 0 is 0: only every other zero was found.
        let f = analyze_text("y=cbrt(sin(x))");
        assert_eq!(f.x_intercepts.as_deref(), Some("{πn₁, n₁ ∈ ℤ}"));
        assert_eq!(f.inflection_points, vec!["{(πn₁, 0), n₁ ∈ ℤ}"]);
        // A hole isn't a zero: (x² − 2)/∛(x² − 2)² is ∛(x² − 2) but for ±√2, where it
        // divides by 0, and root(x² − 2, 4)³/root(x² − 2, 4)² has holes at the ends of its
        // domain, as log(x² − 1, x² − 1) has at ±√2, where its base is 1 and it divides by
        // ln 1. x/(x² + 10⁻¹⁴) only divides by 10⁻¹⁴ at its zero.
        for text in [
            "y=(x^2-2)/cbrt(x^2-2)^2",
            "y=sin(x)/cbrt(sin(x))^2",
            "y=root(x^2-2,4)^3/root(x^2-2,4)^2",
            "y=cbrt(x^2-2)*log(x^2-1,x^2-1)",
        ] {
            assert_eq!(analyze_text(text).x_intercepts, None, "{text}");
        }
        for text in ["y=x/(x^2+10^(-14))", "y=x/(x^2+10^(-30))"] {
            assert_eq!(
                analyze_text(text).x_intercepts.as_deref(),
                Some("0"),
                "{text}"
            );
        }
        // Nor by 1 + 10000·root(|x − 1|, 32), though it is 5234 a billionth away from 1.
        let f = analyze_text("y=(x-1)/(1+10000*root(abs(x-1),32))");
        assert_eq!(f.x_intercepts.as_deref(), Some("1"));
    }

    #[test]
    fn rational_with_an_oblique_asymptote() {
        let f = analyze_text("y=(-3x^2+2)/(x-1)");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ 1}"));
        assert_eq!(
            f.range.as_deref(),
            Some("{y ∈ (−∞, −2√3 − 6] ∪ [2√3 − 6, ∞)}")
        );
        assert_eq!(f.x_intercepts.as_deref(), Some("−√6/3, √6/3"));
        assert_eq!(f.y_intercept.as_deref(), Some("−2"));
        assert_eq!(f.maxima, vec!["(√3/3 + 1, −2√3 − 6)"]);
        assert_eq!(f.minima, vec!["(−√3/3 + 1, 2√3 − 6)"]);
        assert!(f.inflection_points.is_empty());
        assert_eq!(f.vertical_asymptotes, vec!["x = 1"]);
        assert!(f.horizontal_asymptotes.is_empty());
        assert_eq!(f.oblique_asymptotes, vec!["y = −3x − 3"]);
        assert_eq!(f.parity, Parity::Neither);
        assert_eq!(
            directions(&f),
            vec![
                ("(−∞, −√3/3 + 1)", Direction::Decreasing),
                ("(−√3/3 + 1, 1)", Direction::Increasing),
                ("(1, √3/3 + 1)", Direction::Increasing),
                ("(√3/3 + 1, ∞)", Direction::Decreasing),
            ]
        );
    }

    #[test]
    fn compound_terms_keep_their_signs() {
        // An offset of two terms is added with its own signs: x + (1 − √2) is x − √2 + 1,
        // which x − (√2 − 1) without the parentheses, x − √2 − 1, is not.
        for (text, asymptote) in [
            ("y=x+1-sqrt(2)+1/x", "y = x − √2 + 1"),
            ("y=x-(2+sqrt(3))+1/x", "y = x − √3 − 2"),
            ("y=-x-(sqrt(2)-1)+1/x", "y = −x − √2 + 1"),
            ("y=x+2-sqrt(3)+1/x", "y = x − √3 + 2"),
            ("y=x+sqrt(3)-2+1/x", "y = x + √3 − 2"),
            // A slope of two terms is one factor.
            ("y=(1+sqrt(2))x+1/x", "y = (√2 + 1)x"),
        ] {
            let f = analyze_text(text);
            assert_eq!(f.oblique_asymptotes, vec![asymptote], "{text}");
        }
        // The offset of a family too.
        let f = analyze_text("y=sin(x-1+sqrt(3)/3)");
        assert_eq!(f.x_intercepts.as_deref(), Some("{πn₁ − √3/3 + 1, n₁ ∈ ℤ}"));
    }

    #[test]
    fn removable_holes() {
        // 2x + 1 but at −1, where 0/0 leaves f′ to rounding noise: no turning points.
        let f = analyze_text("y=(2x^2+3x+1)/(x+1)");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ −1}"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ∖{−1}}"));
        assert!(f.minima.is_empty() && f.maxima.is_empty());
        // x² + x + 1 but at 1: no inflection point there.
        let f = analyze_text("y=(x^3-1)/(x-1)");
        assert_eq!(f.range.as_deref(), Some("{y ∈ [3/4, ∞)}"));
        assert_eq!(f.minima, vec!["(−1/2, 3/4)"]);
        assert!(f.inflection_points.is_empty());
        // x − 2 but at 3: a line with a hole isn't its own asymptote either.
        let f = analyze_text("y=(x^2-5x+6)/(x-3)");
        assert!(f.oblique_asymptotes.is_empty());
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ∖{1}}"));
        // The values next to the hole at −3 cancel to noise: the range is still found.
        let f = analyze_text("y=(x^2-9)/(x+3)");
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ∖{−6}}"));
        // 2 sin x but where cos x is 0, though f evaluates to ±2 there by rounding: the
        // extrema of 2 sin x are holes.
        let f = analyze_text("y=sin(2x)/cos(x)");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ πn₁ + π/2, ∀n₁ ∈ ℤ}"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ (−2, 2)}"));
        assert!(f.minima.is_empty() && f.maxima.is_empty());
        assert!(f.vertical_asymptotes.is_empty());
        assert_eq!(f.inflection_points, vec!["{(πn₁, 0), n₁ ∈ ℤ}"]);
        // sin x but where sin x is 0: every zero is a hole.
        let f = analyze_text("y=sin(x)^2/sin(x)");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ πn₁, ∀n₁ ∈ ℤ}"));
        assert_eq!(f.x_intercepts, None);
        // Holes at πn and poles at πn + π/2, all in the family πn/2.
        let f = analyze_text("y=sin(x)/sin(2x)");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ πn₁/2, ∀n₁ ∈ ℤ}"));
        assert_eq!(f.vertical_asymptotes, vec!["x = πn₁ + π/2, n₁ ∈ ℤ"]);
        // 1 but at 0: a constant isn't its own asymptote.
        let f = analyze_text("y=x/x");
        assert!(f.horizontal_asymptotes.is_empty());
        // 1 but for a bump at 10, which a few probes around 0 don't see: y = 1 is its
        // asymptote both ways.
        let f = analyze_text("y=1+e^(-(x-10)^2)");
        assert_eq!(f.horizontal_asymptotes, vec!["y = 1"]);
        // A bump that is small next to its line: 10⁹ + e^(−x²) is within a billionth of 10⁹.
        let f = analyze_text("y=10^9+e^(-x^2)");
        assert_eq!(f.horizontal_asymptotes, vec!["y = 1000000000"]);
        // A line far from the origin is still a line.
        let f = analyze_text("y=x+10^9");
        assert!(f.oblique_asymptotes.is_empty());
        let f = analyze_text("y=10^9 x/x");
        assert!(f.horizontal_asymptotes.is_empty());
        // A divisor that only comes close to 0 (2·10⁻¹⁴ at 0) leaves the domain whole.
        let f = analyze_text("y=1/(x^2+0.00000000000002)");
        assert_eq!(f.domain.as_deref(), Some("{x ∈ ℝ}"));
        assert_eq!(f.maxima, vec!["(0, 50000000000000)"]);
        // Nothing with a pole is a line, though 5 + 10⁻¹⁶/(x − 1000) is 5 to the last bits
        // away from 1000; and 1/(x² + 10⁹), within 10⁻⁹ of 0, is no constant.
        let f = analyze_text("y=5+10^(-16)/(x-1000)");
        assert_eq!(f.horizontal_asymptotes, vec!["y = 5"]);
        let f = analyze_text("y=1/(x^2+10^9)");
        assert_eq!(f.horizontal_asymptotes, vec!["y = 0"]);
        // 1/(x + 10⁻¹⁰) divides by 0 at −10⁻¹⁰, not at 0, the exact value next to it.
        let f = analyze_text("y=1/(x+10^(-10))");
        assert_eq!(f.y_intercept.as_deref(), Some("10000000000"));
        // Next to a pole, f′ isn't noise: 3x + 1 + 10⁻⁸/(x − 2) turns at 2 ± 5.8e-5.
        let f = analyze_text("y=3x+1+10^(-8)/(x-2)");
        assert_eq!(f.minima, vec!["(2.00006, 7.00035)"]);
        assert_eq!(f.maxima, vec!["(1.99994, 6.99965)"]);
        // Poles at πn, though 10⁶ sin x dominates 1/sin x until 10⁻⁶ of π past π.
        let f = analyze_text("y=1/sin(x)+10^6 sin(x)");
        assert_eq!(f.vertical_asymptotes, vec!["x = πn₁, n₁ ∈ ℤ"]);
        // A jump isn't a hole: f tends to −0.9984 and 1.0016 at 500, and keeps the minimum
        // next to it.
        let f = analyze_text("y=(x-500)/abs(x-500)+(x-500.04)^2");
        assert_eq!(f.minima, vec!["(500.04, 1)"]);
        // A divisor crossing 0 with a vertical tangent divides by 0 there, though it is
        // 7.6e-6 at the double closest to √2.
        let f = analyze_text("y=1/cbrt(x^2-2)");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ −√2, x ≠ √2}"));
        assert_eq!(f.vertical_asymptotes, vec!["x = −√2", "x = √2"]);
        assert!(f.inflection_points.is_empty());
        let f = analyze_text("y=1/cbrt(sin(x))");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ πn₁, ∀n₁ ∈ ℤ}"));
        assert_eq!(f.vertical_asymptotes, vec!["x = πn₁, n₁ ∈ ℤ"]);
        // root(sin x, 11) tends to 0 at 0, slowly: no pole.
        let f = analyze_text("y=root(sin(x),11)^3/root(sin(x),11)^2");
        assert!(f.vertical_asymptotes.is_empty());
        // ln(−ln|sin x|) runs off to ∞ at πn ever more slowly: a pole all the same.
        let f = analyze_text("y=ln(-ln(abs(sin(x))))");
        assert_eq!(f.vertical_asymptotes, vec!["x = πn₁/2, n₁ ∈ ℤ"]);
        // A divisor that jumps past 0 doesn't divide by it, though at the jump it lies
        // between its sides: (⌊x − 1⌋ + ⌈x − 1⌉)/2 + 0.1 is 0.1 at 1.
        let f = analyze_text("y=1/((floor(x-1)+ceil(x-1))/2+0.1)");
        assert_eq!(f.domain.as_deref(), Some("{x ∈ ℝ}"));
    }

    #[test]
    fn unsupported() {
        let error = |text| {
            let equation = Equation::parse(text, Syntax::default()).unwrap();
            analyze(&equation, Angle::Radians).unwrap_err()
        };
        assert_eq!(error("y=mx+b"), AnalysisError::NotSupported);
        assert_eq!(error("y<x"), AnalysisError::NotSupported);
        assert_eq!(error("x=y^2"), AnalysisError::VariableIsNotX);
        assert_eq!(error("x^2+y^2=1"), AnalysisError::VariableIsNotX);
    }

    #[test]
    fn rough_functions_are_too_complex() {
        let f = analyze_text("floor(x)");
        assert_eq!(f.domain.as_deref(), Some("{x ∈ ℝ}"));
        assert!(f.too_complex.contains(&Feature::Monotonicity));
    }

    #[test]
    fn more_functions() {
        let f = analyze_text("y=sqrt(x)");
        assert_eq!(f.domain.as_deref(), Some("{x ≥ 0}"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ [0, ∞)}"));
        assert_eq!(f.minima, vec!["(0, 0)"]);
        let f = analyze_text("y=abs(x)");
        assert_eq!(f.minima, vec!["(0, 0)"]);
        assert_eq!(f.parity, Parity::Even);
        let f = analyze_text("y=5");
        assert_eq!(f.range.as_deref(), Some("{y = 5}"));
        assert_eq!(f.y_intercept.as_deref(), Some("5"));
        let f = analyze_text("y=atan(x)");
        assert_eq!(f.range.as_deref(), Some("{y ∈ (−π/2, π/2)}"));
        assert_eq!(f.horizontal_asymptotes, vec!["y = −π/2", "y = π/2"]);
        let f = analyze_text("y=1/(x^2-1)");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ −1, x ≠ 1}"));
        assert_eq!(f.vertical_asymptotes, vec!["x = −1", "x = 1"]);
        let f = analyze_text("y=sin(2x)");
        assert_eq!(f.period, Period::Periodic("π".to_string()));
        let f = analyze_text("y=sin(x)^2");
        assert_eq!(f.period, Period::Periodic("π".to_string()));
        let f = analyze_text("y=x^2-4");
        assert_eq!(f.x_intercepts.as_deref(), Some("−2, 2"));
        // A cusp.
        let f = analyze_text("y=x^(2/3)");
        assert_eq!(f.x_intercepts.as_deref(), Some("0"));
        assert_eq!(f.minima, vec!["(0, 0)"]);
        // A hole is no asymptote.
        let f = analyze_text("y=(x^2-1)/(x-1)");
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ∖{2}}"));
        assert!(f.vertical_asymptotes.is_empty());
        let f = analyze_text("y=sqrt(x^2-4)");
        assert_eq!(f.oblique_asymptotes, vec!["y = −x", "y = x"]);
        // e^(−x) underflows to 0 far out: no zeros there.
        let f = analyze_text("y=x e^(-x)");
        assert_eq!(f.x_intercepts.as_deref(), Some("0"));
        let f = analyze_text("y=cos(2x)+sin(x)");
        assert_eq!(f.x_intercepts.as_deref(), Some("{2πn₁/3 + π/2, n₁ ∈ ℤ}"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ [−2, 9/8]}"));
    }

    #[test]
    fn ranges() {
        // [0, max] on [−1, 0] and [0, ∞) on [1, ∞): the union is open at ∞.
        let f = analyze_text("y=sqrt(x^3-x)");
        assert_eq!(f.domain.as_deref(), Some("{x ∈ [−1, 0] ∪ [1, ∞)}"));
        assert_eq!(f.range.as_deref(), Some("{y ∈ [0, ∞)}"));
        // Constant on each piece of the domain.
        let f = analyze_text("y=x/x");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ 0}"));
        assert_eq!(f.range.as_deref(), Some("{y = 1}"));
        let f = analyze_text("y=x/|x|");
        assert_eq!(f.range.as_deref(), Some("{y ∈ {−1, 1}}"));
        assert_eq!(
            directions(&f),
            vec![
                ("(−∞, 0)", Direction::Constant),
                ("(0, ∞)", Direction::Constant)
            ]
        );
    }

    #[test]
    fn flat_pieces() {
        // f′ is exactly 0 on [−1, 1]: the ends of the flat piece are its minima.
        let f = analyze_text("y=|x-1|+|x+1|");
        assert_eq!(f.range.as_deref(), Some("{y ∈ [2, ∞)}"));
        assert_eq!(f.minima, vec!["(−1, 2)", "(1, 2)"]);
        assert!(f.maxima.is_empty());
        assert_eq!(
            directions(&f),
            vec![
                ("(−∞, −1)", Direction::Decreasing),
                ("(−1, 1)", Direction::Constant),
                ("(1, ∞)", Direction::Increasing)
            ]
        );
        let f = analyze_text("y=|x|+|x-2|");
        assert_eq!(f.range.as_deref(), Some("{y ∈ [2, ∞)}"));
        assert_eq!(f.minima, vec!["(0, 2)", "(2, 2)"]);
        let f = analyze_text("y=|x^2-1|+|x^2-4|");
        assert_eq!(f.range.as_deref(), Some("{y ∈ [3, ∞)}"));
        assert_eq!(f.minima, vec!["(−2, 3)", "(−1, 3)", "(1, 3)", "(2, 3)"]);
        assert_eq!(f.maxima, vec!["(0, 5)"]);
        assert_eq!(
            directions(&f),
            vec![
                ("(−∞, −2)", Direction::Decreasing),
                ("(−2, −1)", Direction::Constant),
                ("(−1, 0)", Direction::Increasing),
                ("(0, 1)", Direction::Decreasing),
                ("(1, 2)", Direction::Constant),
                ("(2, ∞)", Direction::Increasing)
            ]
        );
        // Flat out to ±∞: the values −4 and 4 are taken, not only approached.
        let f = analyze_text("y=|x+2|-|x-2|");
        assert_eq!(f.range.as_deref(), Some("{y ∈ [−4, 4]}"));
        assert_eq!(f.minima, vec!["(−2, −4)"]);
        assert_eq!(f.maxima, vec!["(2, 4)"]);
        assert_eq!(
            directions(&f),
            vec![
                ("(−∞, −2)", Direction::Constant),
                ("(−2, 2)", Direction::Increasing),
                ("(2, ∞)", Direction::Constant)
            ]
        );
        // A flat step on the way up is no extremum.
        let f = analyze_text("y=x+(|x-1|-|x+1|)/2");
        assert!(f.minima.is_empty() && f.maxima.is_empty());
        assert_eq!(
            directions(&f),
            vec![
                ("(−∞, −1)", Direction::Increasing),
                ("(−1, 1)", Direction::Constant),
                ("(1, ∞)", Direction::Increasing)
            ]
        );
        // Flat between two samples: f′ is 0 right before the minimum.
        let f = analyze_text("y=|x-0.001|+|x+0.001|");
        assert_eq!(f.minima, vec!["(0, 0.002)"]);
        assert!(f.maxima.is_empty());
        // Flat from a cusp on, where f′ is undefined.
        let f = analyze_text("y=x^(1/3)-|x^(1/3)|");
        assert_eq!(f.range.as_deref(), Some("{y ∈ (−∞, 0]}"));
        assert_eq!(f.maxima, vec!["(0, 0)"]);
        assert_eq!(
            directions(&f),
            vec![
                ("(−∞, 0)", Direction::Increasing),
                ("(0, ∞)", Direction::Constant)
            ]
        );
        // e^(−x²)′ underflows to 0 beyond |x| ≈ 27, which isn't a flat piece.
        let f = analyze_text("y=e^(-x^2)");
        assert!(f.minima.is_empty());
        assert_eq!(
            directions(&f),
            vec![
                ("(−∞, 0)", Direction::Increasing),
                ("(0, ∞)", Direction::Decreasing)
            ]
        );
        // Flat on (π, 2π) in every period.
        let f = analyze_text("y=|sin(x)|+sin(x)");
        assert_eq!(f.range.as_deref(), Some("{y ∈ [0, 2]}"));
        assert_eq!(f.minima, vec!["{(πn₁, 0), n₁ ∈ ℤ}"]);
        assert_eq!(f.maxima, vec!["{(2πn₁ + π/2, 2), n₁ ∈ ℤ}"]);
        assert_eq!(
            directions(&f),
            vec![
                ("{(2πn₁, 2πn₁ + π/2), n₁ ∈ ℤ}", Direction::Increasing),
                ("{(2πn₁ + π/2, 2πn₁ + π), n₁ ∈ ℤ}", Direction::Decreasing),
                ("{(2πn₁ + π, 2πn₁ + 2π), n₁ ∈ ℤ}", Direction::Constant)
            ]
        );
    }

    #[test]
    fn zero_slopes_dont_vote() {
        // Falling, then flat: f64::signum(0.0) is 1, which made it rising.
        let equation = Equation::parse("y=|x-2|-|x+2|", Syntax::default()).unwrap();
        let Shape::Function(e) = &equation.shape else {
            unreachable!()
        };
        let fun = Fun::new(e, Env::RADIANS);
        assert_eq!(
            direction_between(&fun, 1.0, f64::INFINITY),
            Direction::Decreasing
        );
    }

    #[test]
    fn no_real_values() {
        let error = |text| {
            let equation = Equation::parse(text, Syntax::default()).unwrap();
            analyze(&equation, Angle::Radians).unwrap_err()
        };
        assert_eq!(error("y=sqrt(-1-x^2)"), AnalysisError::NotSupported);
        assert_eq!(error("y=ln(-x^2-1)"), AnalysisError::NotSupported);
        assert_eq!(error("y=sqrt(-1)"), AnalysisError::NotSupported);
        assert_eq!(error("y=ln(0)"), AnalysisError::NotSupported);
    }

    #[test]
    fn many_domain_candidates_are_too_complex() {
        // Thousands of poles: counted before any exact value is looked for, which took
        // seconds.
        for text in ["y=tan(x^2)", "y=1/sin(x^2)"] {
            let f = analyze_text(text);
            assert_eq!(f.domain, None);
            assert!(f.too_complex.contains(&Feature::Domain));
        }
    }

    #[test]
    fn large_derivatives_are_too_complex() {
        // f″ of x·x·…·x is too large to use: no inflection points can be told.
        let product = format!("y={}x", "x·".repeat(25));
        let f = analyze_text(&product);
        assert!(f.too_complex.contains(&Feature::InflectionPoints), "{f:?}");
        assert!(f.inflection_points.is_empty());
    }

    #[test]
    fn trigonometric_identities_are_constants() {
        // Every shift repeats them, and their derivatives are only rounding noise: they
        // have no period, extrema or inflection points, like y = 1 and y = 0.
        assert_eq!(analyze_text("y=sin(x)^2+cos(x)^2"), analyze_text("y=1"));
        assert_eq!(
            analyze_text("y=cos(x)^2-sin(x)^2-cos(2x)"),
            analyze_text("y=0")
        );
        // 1 where both are defined.
        let f = analyze_text("y=tan(x)cot(x)");
        assert_eq!(f.range.as_deref(), Some("{y = 1}"));
        assert_eq!(f.period, Period::None);
        assert!(f.minima.is_empty() && f.maxima.is_empty() && f.inflection_points.is_empty());
        // 0 at every multiple of 2π/400 + 2π/800, but no constant.
        let f = analyze_text("y=sin(x)sin(400x)");
        assert_eq!(f.period, Period::Periodic("2π".to_string()));
        // Rounding is measured on the terms: next to 10¹², sin x is far above it, and
        // 10⁻¹⁰ sin x is far above the rounding of 1.
        for text in [
            "y=1000000000000+sin(x)",
            "y=60000000000000+sin(x)",
            "y=1+0.0000000001sin(x)",
        ] {
            let f = analyze_text(text);
            assert_eq!(f.period, Period::Periodic("2π".to_string()), "{text}");
            assert_eq!((f.minima.len(), f.maxima.len()), (1, 1), "{text}");
        }
        // 1 but at πn, where it's 0: no constant, but too complex as other jumps are.
        let f = analyze_text("y=sign(sin(x))^2");
        assert_eq!(f.y_intercept.as_deref(), Some("0"));
        assert!(f.too_complex.contains(&Feature::Range), "{f:?}");
        // A constant is only snapped within its rounding: 10⁻¹⁴ isn't 0.
        assert_eq!(
            analyze_text("y=10^(-14)*(sin(x)^2+cos(x)^2)"),
            analyze_text("y=10^(-14)")
        );
        // Next to 10³⁰⁸: the range of y = 10³⁰⁸, not ∞. (Written out: the derivative of
        // 10^308 overflows to NaN.)
        let f = analyze_text(&format!("y=1{}(sin(x)^2+cos(x)^2)", "0".repeat(308)));
        assert_eq!(f.range, analyze_text("y=10^308").range);
        assert_eq!(f.period, Period::None);
        // A constant's derivative is 0 too: 10¹⁵ + sin x − 10¹⁵ takes values an eighth
        // apart, within the rounding of 10¹⁵, but its derivative is cos x.
        let f = analyze_text("y=10^15+sin(x)-10^15");
        assert_eq!(f.range.as_deref(), Some("{y ∈ [−1, 1]}"));
        assert_eq!(f.period, Period::Periodic("2π".to_string()));
        // 10¹⁶ + sin x − 10¹⁶ is 0 in doubles, but its derivative is cos x: rounding hides
        // its values, and features read from them would be wrong.
        let f = analyze_text("y=10^16+sin(x)-10^16");
        assert_eq!(f.period, Period::Periodic("2π".to_string()));
        for feature in SMOOTH_FEATURES {
            assert!(f.too_complex.contains(&feature), "{feature:?}: {f:?}");
        }
        assert!(f.x_intercepts.is_none() && f.minima.is_empty() && f.maxima.is_empty());
        // One value is no range of a function that isn't constant: 1 ± 10⁻¹⁴ snap to 1, and
        // 10¹⁴ + 1 is too large for `defined`, so only the minimum is left.
        for text in ["y=1+10^(-14)*sin(x)", "y=100000000000000+sin(x)"] {
            let f = analyze_text(text);
            assert_eq!(f.period, Period::Periodic("2π".to_string()), "{text}");
            assert_eq!(f.range, None, "{text}");
            assert!(f.too_complex.contains(&Feature::Range), "{text}: {f:?}");
        }
    }

    #[test]
    fn many_features_in_a_period_are_too_complex() {
        // 50 zeros, turning points and inflection points in each period: too many to
        // write, as on the grid of a function that isn't periodic.
        let f = analyze_text("y=sin(25x)+sin(x)");
        assert_eq!(f.period, Period::Periodic("2π".to_string()));
        for feature in [
            Feature::XIntercept,
            Feature::Minima,
            Feature::Maxima,
            Feature::InflectionPoints,
            Feature::Range,
            Feature::Monotonicity,
        ] {
            assert!(
                f.too_complex.contains(&feature),
                "{feature:?} isn't too complex: {:?}",
                f.too_complex
            );
        }
        assert_eq!(f.x_intercepts, None);
        assert!(f.minima.is_empty() && f.maxima.is_empty() && f.inflection_points.is_empty());
        assert!(f.monotonicity.is_empty());
        // More than 40 families of poles in a period (tan 50x + tan x has 51 poles, not
        // evenly spaced): its domain and vertical asymptotes are too complex to write, not
        // missing.
        let f = analyze_text("y=tan(50x)+tan(x)");
        assert!(f.vertical_asymptotes.is_empty());
        for feature in [Feature::Domain, Feature::VerticalAsymptotes] {
            assert!(
                f.too_complex.contains(&feature),
                "{feature:?}: {:?}",
                f.too_complex
            );
        }
        assert_eq!(f.y_intercept.as_deref(), Some("0"));
        // 45 poles in a period, evenly spaced, are one family: they're written out, in every
        // unit.
        for angle in [Angle::Radians, Angle::Degrees, Angle::Gradians] {
            let equation = Equation::parse("y=tan(45x)+tan(x)", Syntax::default()).unwrap();
            let f = analyze(&equation, angle).unwrap();
            assert!(f.domain.is_some(), "{angle:?}: {f:?}");
            assert!(!f.vertical_asymptotes.is_empty(), "{angle:?}");
            assert!(
                !f.too_complex.contains(&Feature::VerticalAsymptotes),
                "{angle:?}"
            );
        }
        // tan x's poles (π/2 + πn) are among tan 45x's (π/90 + πn/45), and the poles are
        // grouped on their own (apart from the holes), so they're one family.
        let f = analyze_text("y=tan(45x)+tan(x)");
        assert_eq!(
            f.vertical_asymptotes,
            vec!["x = 0.0698132n₁ + 0.0349066, n₁ ∈ ℤ"]
        );
        // But points where f has a value are no gaps, however many: sin(21x)² has 42 zeros
        // in a period, and √(sin(21x)²) + sin x the domain ℝ.
        let f = analyze_text("y=sqrt(sin(21x)^2)+sin(x)");
        assert_eq!(f.domain.as_deref(), Some("{x ∈ ℝ}"));
        assert_eq!(f.period, Period::Periodic("2π".to_string()));
        assert!(!f.too_complex.contains(&Feature::VerticalAsymptotes));
    }

    #[test]
    fn degrees() {
        let equation = Equation::parse("y=sin(x)", Syntax::default()).unwrap();
        let f = analyze(&equation, Angle::Degrees).unwrap();
        assert_eq!(f.period, Period::Periodic("360".to_string()));
        assert_eq!(f.x_intercepts.as_deref(), Some("{180n₁, n₁ ∈ ℤ}"));
    }

    #[test]
    fn periodic_ranges_between_poles() {
        // sec x and csc x have no values between −1 and 1: each branch between two poles has
        // a range of its own.
        for text in ["y=sec(x)", "y=csc(x)", "y=1/cos(x)"] {
            let f = analyze_text(text);
            assert_eq!(
                f.range.as_deref(),
                Some("{y ∈ (−∞, −1] ∪ [1, ∞)}"),
                "{text}"
            );
        }
        let f = analyze_text("y=1/(1-2sin(x))");
        assert_eq!(f.range.as_deref(), Some("{y ∈ (−∞, −1] ∪ [1/3, ∞)}"));
        let equation = Equation::parse("y=2sec(x)+1", Syntax::default()).unwrap();
        let f = analyze(&equation, Angle::Degrees).unwrap();
        assert_eq!(f.range.as_deref(), Some("{y ∈ (−∞, −1] ∪ [3, ∞)}"));
        // Branches that overlap make one range.
        let f = analyze_text("y=sec(x)+csc(x)");
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ}"));
        let f = analyze_text("y=sec(x)^2");
        assert_eq!(f.range.as_deref(), Some("{y ∈ [1, ∞)}"));
    }

    #[test]
    fn decimals_stay_decimals() {
        // 1.05 was taken for −6√21 + 314/11, and the oblique asymptote got lost: f − mx
        // grows with a slope that is a little off.
        let f = analyze_text("y=1/x+1.05");
        assert_eq!(f.horizontal_asymptotes, vec!["y = 1.05"]);
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ∖{1.05}}"));
        let f = analyze_text("y=1.05x+1/x");
        assert_eq!(f.oblique_asymptotes, vec!["y = 1.05x"]);
        let f = analyze_text("y=atan(x)*0.3");
        assert_eq!(
            f.horizontal_asymptotes,
            vec!["y = −0.471239", "y = 0.471239"]
        );
        // Its quotient by π is within LOOSE of 1/12, but the number is 2·10⁻⁶ off π/12.
        let f = analyze_text("y=1/x+0.2618013877991494");
        assert_eq!(f.horizontal_asymptotes, vec!["y = 0.261801"]);
        let f = analyze_text("y=0.2618013877991494x+1/x");
        assert_eq!(f.oblique_asymptotes, vec!["y = 0.261801x"]);
        // A limit that is no exact value comes as e ∓ 10⁻¹² from either side, one point all
        // the same. Limits further apart than that stay two: 1.2·10⁻⁷ or 3·10⁻¹⁰ apart.
        let f = analyze_text("y=1/x+e");
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ∖{2.71828}}"));
        let f = analyze_text("y=1/x+e+atan(x)/10^7");
        assert_eq!(
            f.range.as_deref(),
            Some("{y ∈ (−∞, 2.71828) ∪ (2.71828, ∞)}")
        );
        let f = analyze_text("y=1/x+0.00001234567+atan(x)/10^10");
        assert_eq!(
            f.range.as_deref(),
            Some("{y ∈ (−∞, 0.0000123455) ∪ (0.0000123458, ∞)}")
        );
        // With a larger c/x, which is taken away before the limits are compared.
        let f = analyze_text("y=100/x+0.00001234567+atan(x)/10^10");
        assert_eq!(
            f.range.as_deref(),
            Some("{y ∈ (−∞, 0.0000123454) ∪ (0.0000123459, ∞)}")
        );
        // A jump at a point stays, however small.
        let f = analyze_text("y=0.1234565+0.0000004x/abs(x)");
        assert_eq!(f.range.as_deref(), Some("{y ∈ {0.123456, 0.123457}}"));
        let f = analyze_text("y=0.0001234565+0.0000000004x/abs(x)");
        assert_eq!(f.range.as_deref(), Some("{y ∈ {0.000123456, 0.000123457}}"));
        // Branches that overlap, however little, make one range.
        let f = analyze_text("y=1/x+e-atan(x)/10^10");
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ}"));
        // 3/x leaves 3·10⁻¹² on either side. The limits from both sides of a hole are one
        // value too, which rounding sets 3·10⁻⁵ apart.
        let f = analyze_text("y=3/x+ln(3)");
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ∖{1.09861}}"));
        let f = analyze_text("y=(x-0.001234)/(x^2-0.001234^2)");
        assert_eq!(f.range.as_deref(), Some("{y ∈ ℝ∖{0, 405.186}}"));
        // A pole within TIGHT of a short decimal stays where it is, written as the decimal:
        // x − 1.0500000005 isn't zero at 1.05.
        let f = analyze_text("y=1/(x-1.0500000005)");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ 1.05}"));
        assert_eq!(f.vertical_asymptotes, vec!["x = 1.05"]);
        // Nor does one 6 ulps off 12.34, where f is −9·10¹³, or the end of a root there.
        let f = analyze_text("y=1/(x-(12.34+1/10^14))");
        assert_eq!(f.vertical_asymptotes, vec!["x = 12.34"]);
        let f = analyze_text("y=sqrt(x-(12.34+1/10^14))");
        assert_eq!(f.domain.as_deref(), Some("{x ≥ 12.34}"));
        // In a period too, with no inflection point at the pole, and next to 0.
        let f = analyze_text("y=1/(sin(x)-sin(1.0500000005))");
        assert_eq!(
            f.domain.as_deref(),
            Some("{x ≠ 2πn₁ + 1.05, x ≠ 2πn₁ + 2.09159, ∀n₁ ∈ ℤ}")
        );
        assert!(f.inflection_points.is_empty(), "{:?}", f.inflection_points);
        // A pole 10⁻¹⁰ off 0 stays a pole, written at 0; 10⁻¹⁴ off, f is a pole at 0 too.
        let f = analyze_text("y=tan(x-(pi/2+1/10^10))");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ πn₁, ∀n₁ ∈ ℤ}"));
        let f = analyze_text("y=tan(x-(pi/2+1/10^14))");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ πn₁, ∀n₁ ∈ ℤ}"));
        // The roots 5·10⁻¹⁰ off the poles stay off them, written so, and so do those 10⁻¹⁰
        // before π/2.
        let f = analyze_text("y=cot(10000000x)-200");
        assert_eq!(
            f.x_intercepts.as_deref(),
            Some("{0.000000314159n₁ + 0.000000000499996, n₁ ∈ ℤ}")
        );
        let f = analyze_text("y=tan(x)-10^10");
        assert_eq!(f.x_intercepts.as_deref(), Some("{πn₁ + 1.5708, n₁ ∈ ℤ}"));
        // Within LOOSE of a short decimal but not it: two values 2·10⁻⁷ apart, and a slope
        // that f − mx would grow with if it were taken for 1.01.
        let f = analyze_text("y=0.0100002+0.0000001x/abs(x)");
        assert_eq!(f.range.as_deref(), Some("{y ∈ {0.0100001, 0.0100003}}"));
        assert_eq!(
            f.horizontal_asymptotes,
            vec!["y = 0.0100001", "y = 0.0100003"]
        );
        let f = analyze_text("y=1.0100002x+1/x");
        assert_eq!(f.oblique_asymptotes, vec!["y = 1.01x"]);
        // 10¹² out, the ends of 1000/x + 1.05 are 1.05 ± 10⁻⁹ (more with 10⁶/x), and with
        // atan(x)/10¹⁰ 3·10⁻¹⁰ apart too: one asymptote, with the rest of the c/x added.
        for text in [
            "y=1000/x+1.05",
            "y=10^6/x+1.05",
            "y=1000/x+1.05+atan(x)/10^10",
        ] {
            let f = analyze_text(text);
            assert_eq!(f.horizontal_asymptotes, vec!["y = 1.05"], "{text}");
        }
        // 10⁸ out, f − mx of 2x + 0.05 + 0.1/x is 0.05 but for rounding at both ends, while
        // 10⁻⁷·x/|x| sets them apart nearer in too.
        let f = analyze_text("y=2x+0.05+0.1/x");
        assert_eq!(f.oblique_asymptotes, vec!["y = 2x + 0.05"]);
        let f = analyze_text("y=2x+0.0100002+0.0000001x/abs(x)");
        assert_eq!(
            f.oblique_asymptotes,
            vec!["y = 2x + 0.0100001", "y = 2x + 0.0100003"]
        );
        // 1/8 apart is within the rounding of f − mx 10⁷ out at a slope of 10⁷, but not 10⁶ out:
        // two lines, not their midpoint y = 10000000x + 0.0625.
        let f = analyze_text("y=10000000x+(x/abs(x)+1)/16");
        assert_eq!(
            f.oblique_asymptotes,
            vec!["y = 10000000x", "y = 10000000x + 1/8"]
        );
        // f(x)/x is 1.05 + 10⁻¹¹ at 10¹² for 1.05x + 10 + 1/x, and 1.05 + 1.6·10⁻¹² for
        // 1.05x + atan(x): taken for the slope, it would leave f − mx growing.
        let f = analyze_text("y=1.05x+10+1/x");
        assert_eq!(f.oblique_asymptotes, vec!["y = 1.05x + 10"]);
        let f = analyze_text("y=1.05x+atan(x)");
        assert_eq!(
            f.oblique_asymptotes,
            vec!["y = 1.05x − π/2", "y = 1.05x + π/2"]
        );
        // f − mx of x + 5000/(x − 201) is 5000/x + 1005000/x² + …, two geometric tails: with
        // only the first taken away, it would end 10⁻⁶ off 0 at either end. That of
        // x + 100/x + 10⁷/x² turns between 10⁴ and 10⁸, where once over has to do.
        for (text, line) in [
            ("y=x+5000/(x-201)", "y = x"),
            ("y=x+5000/(x+201)", "y = x"),
            ("y=2x+1/(x-1000)+3", "y = 2x + 3"),
            ("y=x+100/x+10000000/x^2", "y = x"),
        ] {
            let f = analyze_text(text);
            assert_eq!(f.oblique_asymptotes, vec![line], "{text}");
        }
        // A line with a hole where f − mx is compared (−2.5, −0.5, 0.75 or 3) isn't its own
        // asymptote either.
        for text in [
            "y=(x^2-6.25)/(x+2.5)",
            "y=(x^2-0.25)/(x+0.5)",
            "y=(x^2-0.5625)/(x-0.75)",
            "y=(x^2-9)/(x-3)",
            "y=(x^2-6.25)*(x-3)/((x+2.5)*(x-3))",
        ] {
            let f = analyze_text(text);
            assert!(
                f.oblique_asymptotes.is_empty(),
                "{text}: {:?}",
                f.oblique_asymptotes
            );
        }
        // Undefined at −2.5 too, but with a jump there.
        let f = analyze_text("y=x+(x+2.5)/abs(x+2.5)");
        assert_eq!(f.oblique_asymptotes, vec!["y = x − 1", "y = x + 1"]);
        // Two intercepts 1.6·10⁻⁷ apart, which a hole at 10⁵ doesn't make one.
        let f = analyze_text("y=(2x+0.0100002+0.00000008x/abs(x)-0.08/x)*((x-100000)/(x-100000))");
        assert_eq!(
            f.oblique_asymptotes,
            vec!["y = 2x + 0.0100001", "y = 2x + 0.0100003"]
        );
        // An end found a double off √2 is moved to it: x² − 2 is zero there but for roundoff.
        let f = analyze_text("y=sqrt(x^2-2)");
        assert_eq!(f.domain.as_deref(), Some("{x ∈ (−∞, −√2] ∪ [√2, ∞)}"));
        let f = analyze_text("y=sqrt(x-1.05)");
        assert_eq!(f.domain.as_deref(), Some("{x ≥ 1.05}"));
        // At 0, the radicand is cos(π/2) = 6·10⁻¹⁷, as π/2 is rounded.
        let f = analyze_text("y=sqrt(cos(atan(x)+pi/2))");
        assert_eq!(f.domain.as_deref(), Some("{x ≤ 0}"));
        // A turning point snapped onto a hole, where f is 0/0, isn't one.
        let f = analyze_text("y=(x-1.67)/(sqrt(x)-sqrt(1.67))");
        assert!(
            f.maxima.iter().all(|t| !t.contains("NaN")),
            "{:?}",
            f.maxima
        );
        for text in ["y=ln(2-x^2)", "y=1/sqrt(2-x^2)"] {
            let f = analyze_text(text);
            assert_eq!(f.domain.as_deref(), Some("{x ∈ (−√2, √2)}"), "{text}");
            assert_eq!(f.vertical_asymptotes, vec!["x = −√2", "x = √2"], "{text}");
            // f′ changes sign at the ends, where f isn't defined: no extremum there.
            assert!(
                f.minima.iter().all(|t| !t.contains("NaN")),
                "{text}: {:?}",
                f.minima
            );
        }
        // The ends of √(2 − x²) come out open: at the double nearest √2, 2 − x² is −4·10⁻¹⁶.
        let f = analyze_text("y=sqrt(2-x^2)");
        assert_eq!(f.domain.as_deref(), Some("{x ∈ (−√2, √2)}"));
        let degrees = |text| {
            let equation = Equation::parse(text, Syntax::default()).unwrap();
            analyze(&equation, Angle::Degrees).unwrap()
        };
        // sin(x)/x tends to π/180 at 0.
        let f = degrees("y=sin(x)/x");
        assert_eq!(f.range.as_deref(), Some("{y ∈ [−0.00379144, 0.0174533)}"));
        let f = degrees("y=x sin(x)");
        let points = [&f.minima, &f.maxima, &f.inflection_points];
        assert!(
            points.iter().all(|p| p.iter().all(|t| !t.contains('√'))),
            "{points:?}"
        );
        // A surd or a multiple of π near a short decimal (3.248452, 940.907, 171.074,
        // 342.148) stays what it is: moved to the decimal, the pole would be a defined point.
        let f = analyze_text("y=1/(x-(3+sqrt(5)/9))");
        assert_eq!(f.domain.as_deref(), Some("{x ≠ √5/9 + 3}"));
        assert_eq!(f.vertical_asymptotes, vec!["x = √5/9 + 3"]);
        let f = analyze_text("y=1/(x-599pi/2)");
        assert_eq!(f.vertical_asymptotes, vec!["x = 599π/2"]);
        let f = analyze_text("y=1/(x-599pi/11)");
        assert_eq!(f.vertical_asymptotes, vec!["x = 599π/11"]);
        let f = analyze_text("y=1/(x-1198pi/11)");
        assert_eq!(f.vertical_asymptotes, vec!["x = 1198π/11"]);
    }

    #[test]
    fn periods() {
        let p = |text: &str| {
            let equation = Equation::parse(text, Syntax::default()).unwrap();
            let Shape::Function(e) = equation.shape else {
                unreachable!()
            };
            period(&e, Angle::Radians)
        };
        let pi = std::f64::consts::PI;
        assert_eq!(p("sin(x)"), Some(2.0 * pi));
        assert_eq!(p("tan(x)"), Some(pi));
        assert!((p("sin(2x)+cos(3x)").unwrap() - 2.0 * pi).abs() < 1e-12);
        assert_eq!(p("x sin(x)"), None);
        assert_eq!(p("sin(x^2)"), None);
        assert!((p("sin(sin(x))").unwrap() - 2.0 * pi).abs() < 1e-12);
        // In either order.
        assert!((p("sin(13x)+sin(x)").unwrap() - 2.0 * pi).abs() < 1e-12);
        assert!((p("sin(x)+sin(13x)").unwrap() - 2.0 * pi).abs() < 1e-12);
    }
}
