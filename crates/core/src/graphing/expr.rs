//! The expressions of the graphing mode: the tree the parser builds, its evaluation in
//! `f64` and its derivative.
//!
//! Windows evaluates graphs with doubles in its (proprietary) graphing engine, so this
//! module does too, unlike the exact rationals of the calculator engine. Results outside
//! the real numbers are NaN, and the plot leaves a gap there.

use crate::engine::Angle;
use std::f64::consts::{LN_10, PI};

/// The most levels an expression may have. Evaluating, copying and deriving recurse once
/// per level, on the UI thread's 1 MB stack: a box full of `xxx…` (2048 levels) overflowed
/// it. The parser refuses deeper expressions, and `derivative` deeper derivatives.
pub const MAX_DEPTH: usize = 100;

/// The most nodes a derivative may have. Derivatives grow fast (f″ of x · x · … · x has
/// about n³ nodes for n factors), and the analysis evaluates them thousands of times.
const MAX_DERIVATIVE_SIZE: usize = 1000;

/// The functions of one argument (the keypad's and a few common spellings).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Func {
    Sin,
    Cos,
    Tan,
    Sec,
    Csc,
    Cot,
    Asin,
    Acos,
    Atan,
    Asec,
    Acsc,
    Acot,
    Sinh,
    Cosh,
    Tanh,
    Sech,
    Csch,
    Coth,
    Asinh,
    Acosh,
    Atanh,
    Asech,
    Acsch,
    Acoth,
    Abs,
    Floor,
    Ceil,
    Sqrt,
    Cbrt,
    Ln,
    Log,
    Exp,
    Sign,
}

impl Func {
    /// Whether the argument is an angle in the graph's unit (sin, cos, tan, sec, csc, cot).
    pub fn takes_angle(self) -> bool {
        matches!(
            self,
            Func::Sin | Func::Cos | Func::Tan | Func::Sec | Func::Csc | Func::Cot
        )
    }

    /// Whether the result is an angle in the graph's unit (the inverse functions).
    pub fn gives_angle(self) -> bool {
        matches!(
            self,
            Func::Asin | Func::Acos | Func::Atan | Func::Asec | Func::Acsc | Func::Acot
        )
    }

    /// The function's value, with angles in radians.
    fn apply(self, v: f64) -> f64 {
        match self {
            Func::Sin => v.sin(),
            Func::Cos => v.cos(),
            Func::Tan => v.tan(),
            Func::Sec => 1.0 / v.cos(),
            Func::Csc => 1.0 / v.sin(),
            Func::Cot => 1.0 / v.tan(),
            Func::Asin => v.asin(),
            Func::Acos => v.acos(),
            Func::Atan => v.atan(),
            Func::Asec => (1.0 / v).acos(),
            Func::Acsc => (1.0 / v).asin(),
            Func::Acot => {
                if v == 0.0 {
                    PI / 2.0
                } else {
                    (1.0 / v).atan()
                }
            }
            Func::Sinh => v.sinh(),
            Func::Cosh => v.cosh(),
            Func::Tanh => v.tanh(),
            Func::Sech => 1.0 / v.cosh(),
            Func::Csch => 1.0 / v.sinh(),
            Func::Coth => 1.0 / v.tanh(),
            Func::Asinh => v.asinh(),
            Func::Acosh => v.acosh(),
            Func::Atanh => v.atanh(),
            Func::Asech => (1.0 / v).acosh(),
            Func::Acsch => (1.0 / v).asinh(),
            Func::Acoth => (1.0 / v).atanh(),
            Func::Abs => v.abs(),
            Func::Floor => v.floor(),
            Func::Ceil => v.ceil(),
            Func::Sqrt => v.sqrt(),
            Func::Cbrt => v.cbrt(),
            Func::Ln => v.ln(),
            Func::Log => v.log10(),
            Func::Exp => v.exp(),
            Func::Sign => {
                if v > 0.0 {
                    1.0
                } else if v < 0.0 {
                    -1.0
                } else if v == 0.0 {
                    0.0
                } else {
                    f64::NAN
                }
            }
        }
    }
}

/// The binary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

/// An expression in x, y and the free variables.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    Pi,
    E,
    X,
    Y,
    /// A free variable, which gets a slider (`y = mx + b` has m and b).
    Var(char),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Call(Func, Box<Expr>),
    /// `root(x, n)`: the n-th root of x.
    Root(Box<Expr>, Box<Expr>),
    /// `log(b, x)`: the logarithm of x to the base b.
    LogBase(Box<Expr>, Box<Expr>),
    Factorial(Box<Expr>),
}

/// What an expression is evaluated with: the angle unit and the free variables' values.
#[derive(Clone, Copy, Debug)]
pub struct Env<'a> {
    pub angle: Angle,
    pub vars: &'a [(char, f64)],
}

impl Env<'_> {
    /// An environment without free variables.
    pub const RADIANS: Env<'static> = Env {
        angle: Angle::Radians,
        vars: &[],
    };

    fn var(&self, name: char) -> f64 {
        self.vars
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(f64::NAN, |&(_, v)| v)
    }
}

/// Radians per unit of `angle`.
pub fn radians_per_unit(angle: Angle) -> f64 {
    match angle {
        Angle::Radians => 1.0,
        Angle::Degrees => PI / 180.0,
        Angle::Gradians => PI / 200.0,
    }
}

/// `b` to the power `e` over the real numbers: a negative base has real powers with an
/// odd denominator (`(-8)^(1/3)` is -2), as on Windows' graphs; others are NaN.
pub fn real_pow(b: f64, e: f64) -> f64 {
    if b >= 0.0 || e.fract() == 0.0 || !e.is_finite() || b.is_nan() {
        return b.powf(e);
    }
    for q in (3..=99).step_by(2) {
        let p = e * q as f64;
        if (p - p.round()).abs() < 1e-9 {
            let m = (-b).powf(e);
            return if p.round() as i64 % 2 == 0 { m } else { -m };
        }
    }
    f64::NAN
}

/// x!: n! for the whole numbers up to 170 (the largest that fits in a double), and
/// Γ(x + 1) for the rest.
pub fn factorial(x: f64) -> f64 {
    if x.fract() == 0.0 && (0.0..=170.0).contains(&x) {
        whole_factorial(x as u32)
    } else {
        gamma(x + 1.0)
    }
}

/// n!, rounded once. The product is kept as hi + lo, two doubles, and each hi · k is
/// exact as the sum of two products: hi's first 45 bits times k (k ≤ 170 has 8 bits) and
/// its last 8 bits times k. Multiplying doubles one by one would end up to 6 ulps off.
fn whole_factorial(n: u32) -> f64 {
    let (mut hi, mut lo) = (1.0_f64, 0.0_f64);
    for k in 2..=n {
        let k = f64::from(k);
        let top = f64::from_bits(hi.to_bits() & !0xff);
        let (a, b) = (top * k, (hi - top) * k);
        // s + e is a + b exactly (|a| ≥ |b|), as hi + lo is s + t below.
        let s = a + b;
        let e = b - (s - a);
        let t = e + lo * k;
        hi = s + t;
        lo = t - (hi - s);
    }
    hi
}

/// The gamma function (Lanczos approximation), for the factorials of fractions.
pub fn gamma(x: f64) -> f64 {
    if x <= 0.0 && x.fract() == 0.0 {
        return f64::NAN;
    }
    if x < 0.5 {
        // Reflection: Γ(x) Γ(1 − x) = π / sin(πx), dividing by Γ(1 − x)'s factors one by
        // one: Γ(1 − x) is beyond doubles from 1 − x = 171.62 on, while Γ(x) is still a
        // double beside its poles down to about −183, and a zero of its sign below.
        let s = sin_pi(x);
        if x < -200.0 {
            // Below every double even beside a pole, where the factors would turn into
            // ∞ · 0: a zero with the sign of sin(πx), NaN for −∞.
            return 0.0 * s;
        }
        let [a, b, c] = lanczos(1.0 - x);
        return PI / s / a / b / c;
    }
    // Γ(x) is beyond doubles from 171.62 on, and much further out the power in `lanczos`
    // would be ∞ and e^−t 0.
    if x > 172.0 {
        return f64::INFINITY;
    }
    let [a, b, c] = lanczos(x);
    a * b * c
}

/// sin(πx) without rounding πx, which next to an integer would be most of what's left of
/// it (π · 0.9999999999999998 is off by a third of its distance to π): the sine of πd for
/// the distance d to the nearest integer n, which is exact, with the sign of (−1)ⁿ.
fn sin_pi(x: f64) -> f64 {
    let n = x.round();
    let s = (PI * (x - n)).sin();
    if n % 2.0 == 0.0 { s } else { -s }
}

/// Γ(x) for x ≥ 0.5 as the product of three factors (Lanczos approximation), each a double
/// well beyond Γ's own range: t^(x − 0.5) alone overflows from Γ(143) on, before e^−t
/// brings it back down, so it's taken in two halves.
fn lanczos(x: f64) -> [f64; 3] {
    const G: f64 = 7.0;
    #[allow(clippy::excessive_precision)]
    const C: [f64; 9] = [
        0.999_999_999_999_809_93,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_13,
        -176.615_029_162_140_59,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_571_6e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + G + 0.5;
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    let half = t.powf((x + 0.5) / 2.0);
    [(2.0 * PI).sqrt() * (half * (-t).exp()), half, a]
}

fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
    Expr::Bin(op, Box::new(a), Box::new(b))
}

fn call(f: Func, a: Expr) -> Expr {
    Expr::Call(f, Box::new(a))
}

/// The derivative of `f` at `u`, with `k` radians per angle unit. It is a function of
/// its own to keep `Expr::derive`'s stack frame small: a debug build gives that frame
/// room for every arm's temporaries, and `derive` recurses once per level of the tree.
fn outer_derivative(f: Func, u: Expr, k: f64) -> Expr {
    use BinOp::*;
    use Expr::*;
    let sq = |e: Expr| bin(Pow, e, Num(2.0));
    let kk = || Num(k);
    match f {
        Func::Sin => bin(Mul, kk(), call(Func::Cos, u)),
        Func::Cos => Neg(Box::new(bin(Mul, kk(), call(Func::Sin, u)))),
        Func::Tan => bin(Mul, kk(), sq(call(Func::Sec, u))),
        Func::Sec => bin(
            Mul,
            kk(),
            bin(Mul, call(Func::Sec, u.clone()), call(Func::Tan, u)),
        ),
        Func::Csc => Neg(Box::new(bin(
            Mul,
            kk(),
            bin(Mul, call(Func::Csc, u.clone()), call(Func::Cot, u)),
        ))),
        Func::Cot => Neg(Box::new(bin(Mul, kk(), sq(call(Func::Csc, u))))),
        Func::Asin => bin(
            Div,
            Num(1.0),
            bin(Mul, kk(), call(Func::Sqrt, bin(Sub, Num(1.0), sq(u)))),
        ),
        Func::Acos => Neg(Box::new(bin(
            Div,
            Num(1.0),
            bin(Mul, kk(), call(Func::Sqrt, bin(Sub, Num(1.0), sq(u)))),
        ))),
        Func::Atan => bin(Div, Num(1.0), bin(Mul, kk(), bin(Add, Num(1.0), sq(u)))),
        Func::Acot => Neg(Box::new(bin(
            Div,
            Num(1.0),
            bin(Mul, kk(), bin(Add, Num(1.0), sq(u))),
        ))),
        Func::Asec | Func::Acsc => {
            let d = bin(
                Div,
                Num(1.0),
                bin(
                    Mul,
                    kk(),
                    bin(
                        Mul,
                        call(Func::Abs, u.clone()),
                        call(Func::Sqrt, bin(Sub, sq(u), Num(1.0))),
                    ),
                ),
            );
            if f == Func::Asec { d } else { Neg(Box::new(d)) }
        }
        Func::Sinh => call(Func::Cosh, u),
        Func::Cosh => call(Func::Sinh, u),
        Func::Tanh => sq(call(Func::Sech, u)),
        Func::Sech => Neg(Box::new(bin(
            Mul,
            call(Func::Sech, u.clone()),
            call(Func::Tanh, u),
        ))),
        Func::Csch => Neg(Box::new(bin(
            Mul,
            call(Func::Csch, u.clone()),
            call(Func::Coth, u),
        ))),
        Func::Coth => Neg(Box::new(sq(call(Func::Csch, u)))),
        Func::Asinh => bin(Div, Num(1.0), call(Func::Sqrt, bin(Add, sq(u), Num(1.0)))),
        Func::Acosh => bin(Div, Num(1.0), call(Func::Sqrt, bin(Sub, sq(u), Num(1.0)))),
        Func::Atanh | Func::Acoth => bin(Div, Num(1.0), bin(Sub, Num(1.0), sq(u))),
        Func::Asech => Neg(Box::new(bin(
            Div,
            Num(1.0),
            bin(Mul, u.clone(), call(Func::Sqrt, bin(Sub, Num(1.0), sq(u)))),
        ))),
        Func::Acsch => Neg(Box::new(bin(
            Div,
            Num(1.0),
            bin(
                Mul,
                call(Func::Abs, u.clone()),
                call(Func::Sqrt, bin(Add, Num(1.0), sq(u))),
            ),
        ))),
        Func::Abs => call(Func::Sign, u),
        Func::Floor | Func::Ceil | Func::Sign => Num(0.0),
        Func::Sqrt => bin(Div, Num(1.0), bin(Mul, Num(2.0), call(Func::Sqrt, u))),
        Func::Cbrt => bin(Div, Num(1.0), bin(Mul, Num(3.0), sq(call(Func::Cbrt, u)))),
        Func::Ln => bin(Div, Num(1.0), u),
        Func::Log => bin(Div, Num(1.0), bin(Mul, u, Num(LN_10))),
        Func::Exp => call(Func::Exp, u),
    }
}

impl Expr {
    /// The value at (x, y).
    pub fn eval(&self, x: f64, y: f64, env: &Env) -> f64 {
        match self {
            Expr::Num(v) => *v,
            Expr::Pi => PI,
            Expr::E => std::f64::consts::E,
            Expr::X => x,
            Expr::Y => y,
            Expr::Var(name) => env.var(*name),
            Expr::Neg(a) => -a.eval(x, y, env),
            Expr::Bin(op, a, b) => {
                let (a, b) = (a.eval(x, y, env), b.eval(x, y, env));
                match op {
                    BinOp::Add => a + b,
                    BinOp::Sub => a - b,
                    BinOp::Mul => a * b,
                    BinOp::Div => a / b,
                    BinOp::Pow => real_pow(a, b),
                }
            }
            Expr::Call(f, a) => {
                let v = a.eval(x, y, env);
                let k = radians_per_unit(env.angle);
                if f.takes_angle() {
                    f.apply(v * k)
                } else if f.gives_angle() {
                    f.apply(v) / k
                } else {
                    f.apply(v)
                }
            }
            Expr::Root(a, n) => real_pow(a.eval(x, y, env), 1.0 / n.eval(x, y, env)),
            Expr::LogBase(b, a) => a.eval(x, y, env).ln() / b.eval(x, y, env).ln(),
            Expr::Factorial(a) => factorial(a.eval(x, y, env)),
        }
    }

    /// Whether `test` holds for this node or one inside it.
    pub fn any(&self, test: &impl Fn(&Expr) -> bool) -> bool {
        if test(self) {
            return true;
        }
        match self {
            Expr::Neg(a) | Expr::Call(_, a) | Expr::Factorial(a) => a.any(test),
            Expr::Bin(_, a, b) | Expr::Root(a, b) | Expr::LogBase(a, b) => {
                a.any(test) || b.any(test)
            }
            _ => false,
        }
    }

    pub fn has_x(&self) -> bool {
        self.any(&|e| matches!(e, Expr::X))
    }

    pub fn has_y(&self) -> bool {
        self.any(&|e| matches!(e, Expr::Y))
    }

    /// The free variables, sorted and without repeats.
    pub fn variables(&self) -> Vec<char> {
        fn walk(e: &Expr, out: &mut Vec<char>) {
            match e {
                Expr::Var(c) => out.push(*c),
                Expr::Neg(a) | Expr::Call(_, a) | Expr::Factorial(a) => walk(a, out),
                Expr::Bin(_, a, b) | Expr::Root(a, b) | Expr::LogBase(a, b) => {
                    walk(a, out);
                    walk(b, out);
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        walk(self, &mut out);
        out.sort_unstable();
        out.dedup();
        out
    }

    /// The expression with x and y swapped (to plot x = g(y) like y = f(x)).
    pub fn swap_xy(&self) -> Expr {
        match self {
            Expr::X => Expr::Y,
            Expr::Y => Expr::X,
            Expr::Neg(a) => Expr::Neg(Box::new(a.swap_xy())),
            Expr::Call(f, a) => Expr::Call(*f, Box::new(a.swap_xy())),
            Expr::Factorial(a) => Expr::Factorial(Box::new(a.swap_xy())),
            Expr::Bin(op, a, b) => Expr::Bin(*op, Box::new(a.swap_xy()), Box::new(b.swap_xy())),
            Expr::Root(a, b) => Expr::Root(Box::new(a.swap_xy()), Box::new(b.swap_xy())),
            Expr::LogBase(a, b) => Expr::LogBase(Box::new(a.swap_xy()), Box::new(b.swap_xy())),
            other => other.clone(),
        }
    }

    /// The number of nodes.
    pub fn size(&self) -> usize {
        match self {
            Expr::Neg(a) | Expr::Call(_, a) | Expr::Factorial(a) => 1 + a.size(),
            Expr::Bin(_, a, b) | Expr::Root(a, b) | Expr::LogBase(a, b) => 1 + a.size() + b.size(),
            _ => 1,
        }
    }

    /// The number of levels: 1 for a number or a variable.
    pub fn depth(&self) -> usize {
        match self {
            Expr::Neg(a) | Expr::Call(_, a) | Expr::Factorial(a) => 1 + a.depth(),
            Expr::Bin(_, a, b) | Expr::Root(a, b) | Expr::LogBase(a, b) => {
                1 + a.depth().max(b.depth())
            }
            _ => 1,
        }
    }

    /// The derivative by x (y counts as a constant), in the angle unit `angle`, or
    /// `None` for factorials, which have no simple one, and when it (or the expression)
    /// is too large to use (`MAX_DERIVATIVE_SIZE`, `MAX_DEPTH`).
    pub fn derivative(&self, angle: Angle) -> Option<Expr> {
        let fits = |e: &Expr| e.size() <= MAX_DERIVATIVE_SIZE && e.depth() <= MAX_DEPTH;
        if !fits(self) {
            return None;
        }
        Some(self.derive(angle)?.simplify()).filter(fits)
    }

    /// How large the numbers are that evaluating the expression at (x, y) goes through:
    /// its value, but a sum or difference counts the sizes of its terms rather than their
    /// total, and a product or quotient carries them along. Rounding is a few units in the
    /// last place of it: the derivative of 10¹³(sin²x + cos²x) is 10¹³ times terms near 1
    /// that cancel, and its value is that rounding.
    pub fn magnitude(&self, x: f64, y: f64, env: &Env) -> f64 {
        use BinOp::*;
        match self {
            Expr::Neg(a) => a.magnitude(x, y, env),
            Expr::Bin(Add | Sub, a, b) => a.magnitude(x, y, env) + b.magnitude(x, y, env),
            Expr::Bin(Mul, a, b) => a.magnitude(x, y, env) * b.magnitude(x, y, env),
            Expr::Bin(Div, a, b) => {
                let value = self.eval(x, y, env).abs();
                (a.magnitude(x, y, env) + value * b.magnitude(x, y, env)) / b.eval(x, y, env).abs()
            }
            _ => self.eval(x, y, env).abs(),
        }
    }

    fn derive(&self, angle: Angle) -> Option<Expr> {
        use BinOp::*;
        use Expr::*;
        let k = radians_per_unit(angle);
        Some(match self {
            Num(_) | Pi | E | Y | Var(_) => Num(0.0),
            X => Num(1.0),
            Neg(a) => Neg(Box::new(a.derive(angle)?)),
            Bin(Add, a, b) => bin(Add, a.derive(angle)?, b.derive(angle)?),
            Bin(Sub, a, b) => bin(Sub, a.derive(angle)?, b.derive(angle)?),
            Bin(Mul, a, b) => bin(
                Add,
                bin(Mul, a.derive(angle)?, (**b).clone()),
                bin(Mul, (**a).clone(), b.derive(angle)?),
            ),
            Bin(Div, a, b) => bin(
                Div,
                bin(
                    Sub,
                    bin(Mul, a.derive(angle)?, (**b).clone()),
                    bin(Mul, (**a).clone(), b.derive(angle)?),
                ),
                bin(Pow, (**b).clone(), Num(2.0)),
            ),
            Bin(Pow, a, b) => {
                let (da, db) = (a.derive(angle)?.simplify(), b.derive(angle)?.simplify());
                if db == Num(0.0) {
                    // b · a^(b − 1) · a'
                    bin(
                        Mul,
                        bin(
                            Mul,
                            (**b).clone(),
                            bin(Pow, (**a).clone(), bin(Sub, (**b).clone(), Num(1.0))),
                        ),
                        da,
                    )
                } else if da == Num(0.0) {
                    // a^b · ln a · b'
                    bin(
                        Mul,
                        bin(Mul, self.clone(), call(Func::Ln, (**a).clone())),
                        db,
                    )
                } else {
                    // a^b · (b' ln a + b a' / a)
                    bin(
                        Mul,
                        self.clone(),
                        bin(
                            Add,
                            bin(Mul, db, call(Func::Ln, (**a).clone())),
                            bin(Div, bin(Mul, (**b).clone(), da), (**a).clone()),
                        ),
                    )
                }
            }
            // The derivative of f at u, times u'.
            Call(f, a) => bin(
                Mul,
                outer_derivative(*f, (**a).clone(), k),
                a.derive(angle)?,
            ),
            Root(a, n) => {
                if n.has_x() {
                    // root(a, n) = a^(1/n)
                    return bin(Pow, (**a).clone(), bin(Div, Num(1.0), (**n).clone()))
                        .derive(angle);
                }
                // root(a, n) / (n a) · a'
                bin(
                    Mul,
                    bin(Div, self.clone(), bin(Mul, (**n).clone(), (**a).clone())),
                    a.derive(angle)?,
                )
            }
            LogBase(b, a) => bin(
                Div,
                call(Func::Ln, (**a).clone()),
                call(Func::Ln, (**b).clone()),
            )
            .derive(angle)?,
            Factorial(_) => return None,
        })
    }

    /// Folds constants and drops the zeros and ones a derivative is full of.
    pub fn simplify(self) -> Expr {
        use BinOp::*;
        use Expr::*;
        match self {
            Neg(a) => match a.simplify() {
                Num(v) => Num(-v),
                Neg(inner) => *inner,
                other => Neg(Box::new(other)),
            },
            Bin(op, a, b) => {
                let (a, b) = (a.simplify(), b.simplify());
                match (op, &a, &b) {
                    (_, Num(x), Num(y)) => {
                        Num(bin(op, Num(*x), Num(*y)).eval(0.0, 0.0, &Env::RADIANS))
                    }
                    (Add, Num(z), _) if *z == 0.0 => b,
                    (Add | Sub, _, Num(z)) if *z == 0.0 => a,
                    (Sub, Num(z), _) if *z == 0.0 => Neg(Box::new(b)),
                    (Mul, Num(z), _) | (Mul, _, Num(z)) if *z == 0.0 => Num(0.0),
                    (Mul, Num(one), _) if *one == 1.0 => b,
                    (Mul | Div, _, Num(one)) if *one == 1.0 => a,
                    (Div, Num(z), _) if *z == 0.0 => Num(0.0),
                    (Pow, _, Num(one)) if *one == 1.0 => a,
                    (Pow, _, Num(z)) if *z == 0.0 => Num(1.0),
                    _ => bin(op, a, b),
                }
            }
            Call(f, a) => match a.simplify() {
                Num(v) if !f.takes_angle() && !f.gives_angle() => Num(f.apply(v)),
                other => Call(f, Box::new(other)),
            },
            Root(a, n) => Root(Box::new(a.simplify()), Box::new(n.simplify())),
            LogBase(b, a) => LogBase(Box::new(b.simplify()), Box::new(a.simplify())),
            Factorial(a) => Factorial(Box::new(a.simplify())),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn x() -> Expr {
        Expr::X
    }

    #[test]
    fn real_powers() {
        assert_eq!(real_pow(-8.0, 1.0 / 3.0), -2.0);
        assert!((real_pow(-8.0, 2.0 / 3.0) - 4.0).abs() < 1e-12);
        assert!(real_pow(-4.0, 0.5).is_nan());
        assert_eq!(real_pow(-2.0, 3.0), -8.0);
    }

    #[test]
    fn factorial_by_gamma() {
        assert!((gamma(5.0) - 24.0).abs() < 1e-9);
        assert!((gamma(0.5) - PI.sqrt()).abs() < 1e-12);
        assert!(gamma(-1.0).is_nan());
    }

    #[test]
    fn exact_factorials() {
        let factorial =
            |n: f64| Expr::Factorial(Box::new(Expr::Num(n))).eval(0.0, 0.0, &Env::RADIANS);
        // Whole numbers are exact (the tracer showed 3! as 6.000000000000007), and rounded
        // once up to 170!, the largest factorial in doubles: multiplying doubles one by one
        // is 1 ulp off at 28!.
        assert_eq!(factorial(3.0), 6.0);
        assert_eq!(factorial(5.0), 120.0);
        assert_eq!(factorial(17.0), 355_687_428_096_000.0);
        assert_eq!(factorial(28.0), 3.0488834461171387e29);
        assert_eq!(factorial(142.0), 2.695364137888163e245);
        assert_eq!(factorial(170.0), 7.257415615307999e306);
        assert_eq!(factorial(171.0), f64::INFINITY);
        // Fractions stay finite as long as they fit in a double.
        assert!((factorial(142.5) / 3.2203704817308093e246 - 1.0).abs() < 1e-12);
        assert!(factorial(170.6).is_finite());
        assert_eq!(factorial(170.7), f64::INFINITY);
        assert_eq!(factorial(1000.5), f64::INFINITY);
        // A negative fraction's factorial is tiny where Γ(1 − x) is beyond doubles:
        // (−172.1)! = Γ(−171.1) is 4.897e−309.
        let tiny = factorial(-172.1) * 1e308;
        assert!((tiny / 0.489722739514055 - 1.0).abs() < 1e-9, "{tiny}");
        // 150!/150! is a line at 1, not a gap.
        let ratio = bin(
            BinOp::Div,
            Expr::Factorial(Box::new(Expr::Num(150.0))),
            Expr::Factorial(Box::new(Expr::Num(150.0))),
        );
        assert_eq!(ratio.eval(0.0, 0.0, &Env::RADIANS), 1.0);
    }

    #[test]
    fn negative_factorials() {
        let factorial =
            |n: f64| Expr::Factorial(Box::new(Expr::Num(n))).eval(0.0, 0.0, &Env::RADIANS);
        let check = |n: f64, want: f64, tolerance: f64| {
            let got = factorial(n);
            assert!(
                (got / want - 1.0).abs() < tolerance,
                "{n}! is {got}, not {want}"
            );
        };
        // Next to the poles, where πx has lost most of the distance to the integer.
        check(-1.9999999999999998, -4503599627370496.0, 1e-12);
        check(-2.0000000000000004, 2251799813685247.5, 1e-12);
        check(-2.9999999999999996, 1125899906842624.5, 1e-12);
        check(-3.0000000000000004, -1125899906842623.5, 1e-12);
        check(-100.00000000000001, 7.54008333488311e-143, 1e-12);
        // Below −180, down to the smallest doubles, then zeros with Γ's sign.
        check(-181.00000000000003, -1.7513719e-316, 1e-6);
        check(-183.00000000000003, -5.316e-321, 1e-3);
        assert_eq!(1.0 / factorial(-181.1), f64::NEG_INFINITY);
        assert_eq!(1.0 / factorial(-1000.5), f64::INFINITY);
    }

    #[test]
    fn angle_units() {
        let sin = call(Func::Sin, x());
        let degrees = Env {
            angle: Angle::Degrees,
            vars: &[],
        };
        assert!((sin.eval(30.0, 0.0, &degrees) - 0.5).abs() < 1e-12);
        let asin = call(Func::Asin, x());
        assert!((asin.eval(1.0, 0.0, &degrees) - 90.0).abs() < 1e-12);
    }

    #[test]
    fn derivatives() {
        let check = |e: Expr, at: f64, want: f64| {
            let d = e.derivative(Angle::Radians).unwrap();
            let got = d.eval(at, 0.0, &Env::RADIANS);
            assert!((got - want).abs() < 1e-9, "{e:?} at {at}: {got} != {want}");
        };
        check(bin(BinOp::Pow, x(), Expr::Num(3.0)), 2.0, 12.0);
        check(call(Func::Sin, x()), 0.0, 1.0);
        check(bin(BinOp::Div, Expr::Num(1.0), x()), 2.0, -0.25);
        check(call(Func::Ln, x()), 4.0, 0.25);
        check(bin(BinOp::Pow, Expr::E, x()), 1.0, std::f64::consts::E);
        check(bin(BinOp::Pow, x(), x()), 1.0, 1.0);
        check(call(Func::Atan, x()), 1.0, 0.5);
        // The cube root has a real derivative for negative x too.
        check(bin(BinOp::Pow, x(), Expr::Num(1.0 / 3.0)), -8.0, 1.0 / 12.0);
        check(
            Expr::Root(Box::new(x()), Box::new(Expr::Num(3.0))),
            -8.0,
            1.0 / 12.0,
        );
        // Degrees: d/dx sin(x°) = π/180 · cos(x°).
        let d = call(Func::Sin, x()).derivative(Angle::Degrees).unwrap();
        let degrees = Env {
            angle: Angle::Degrees,
            vars: &[],
        };
        assert!((d.eval(0.0, 0.0, &degrees) - PI / 180.0).abs() < 1e-12);
        assert!(
            Expr::Factorial(Box::new(x()))
                .derivative(Angle::Radians)
                .is_none()
        );
    }

    #[test]
    fn sizes() {
        let e = bin(BinOp::Add, call(Func::Sin, x()), Expr::Num(1.0));
        assert_eq!((e.size(), e.depth()), (4, 3));
    }

    #[test]
    fn large_derivatives_are_skipped() {
        // x · x · … · x with 60 factors: f′ has thousands of nodes and f″ millions,
        // which froze the analysis for minutes.
        let mut product = x();
        for _ in 1..60 {
            product = bin(BinOp::Mul, product, x());
        }
        assert!(product.derivative(Angle::Radians).is_none());
        // √√…√x 300 times is too deep to derive.
        let mut roots = x();
        for _ in 0..300 {
            roots = call(Func::Sqrt, roots);
        }
        assert!(roots.derivative(Angle::Radians).is_none());
        // A few factors still have their derivatives.
        let cube = bin(BinOp::Mul, bin(BinOp::Mul, x(), x()), x());
        let d2 = cube
            .derivative(Angle::Radians)
            .and_then(|d| d.derivative(Angle::Radians))
            .unwrap();
        assert_eq!(d2.eval(2.0, 0.0, &Env::RADIANS), 12.0);
    }

    #[test]
    fn simplification() {
        let e = bin(
            BinOp::Add,
            bin(BinOp::Mul, Expr::Num(0.0), x()),
            bin(BinOp::Mul, Expr::Num(1.0), x()),
        );
        assert_eq!(e.simplify(), x());
        assert_eq!(
            bin(BinOp::Mul, Expr::Num(2.0), Expr::Num(3.0)).simplify(),
            Expr::Num(6.0)
        );
    }

    #[test]
    fn variables_are_sorted() {
        let e = bin(
            BinOp::Add,
            bin(BinOp::Mul, Expr::Var('m'), x()),
            bin(BinOp::Add, Expr::Var('b'), Expr::Var('m')),
        );
        assert_eq!(e.variables(), vec!['b', 'm']);
    }
}
