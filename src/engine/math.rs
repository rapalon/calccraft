//! The Scientific mode's functions: powers, roots and logarithms, the trigonometric and
//! hyperbolic functions and their inverses, the factorial (with the gamma function for
//! fractions), the DMS conversions and the remainder, on exact rationals.
//!
//! Windows' `ratpak` computes them with Taylor series on rationals of 32 digits. Here
//! they run on binary floating point numbers of a few hundred bits (`Float`), and the
//! results are rounded to `INTERNAL_DIGITS` significant digits: they agree with Windows
//! in the 32 digits the display shows, and exact results stay exact (`sin(30°)` is 0.5,
//! `log(1000)` is 3, `8 yroot 3` is 2). The domain checks and special cases follow
//! `ratpak` (`exp.cpp`, `trans.cpp`, `itrans.cpp`, `transh.cpp`, `itransh.cpp`,
//! `fact.cpp`, `logic.cpp`) and `scifunc.cpp`, as the comments say.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use num_bigint::{BigInt, BigUint};
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

use super::{
    CalcError, INTERNAL_DIGITS, MAX_EXPONENT, Number, exponent, exponent_overflows, pow10,
    scaled_round,
};

/// Bits of the working precision, about 120 decimal digits.
const PREC: u64 = 400;
/// `rat_smallest` in the Scientific mode (`10^-precision`, 32 digits): sines and cosines
/// this close to 0 are 0, and powers of numbers this close to 1 are 1.
const SMALLEST_DIGITS: i64 = 32;
/// `rat_max_exp`: `e^x` (and `x^y` through it) only for |x| up to this.
const MAX_EXP: f64 = 100_000.0;
/// `rat_max_fact` and `rat_min_fact`: the factorial's range.
const MAX_FACT: i64 = 3249;
const MIN_FACT: i64 = -1000;
/// `m_maxTrigonometricNum`: Windows refuses trigonometry from 10^100 on.
const MAX_TRIG_EXPONENT: i64 = 100;

/// The angle unit of the trigonometric functions (`AngleType`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Angle {
    #[default]
    Degrees,
    Radians,
    Gradians,
}

impl Angle {
    /// A quarter turn in this unit, or `None` for radians.
    fn quarter(self) -> Option<i64> {
        match self {
            Angle::Degrees => Some(90),
            Angle::Gradians => Some(100),
            Angle::Radians => None,
        }
    }
}

// ------------------------------------------------------------------ Float

/// A binary floating point number `m · 2^e` with an unbounded mantissa; every operation
/// rounds the mantissa to the precision it is given.
#[derive(Clone, Debug)]
struct Float {
    m: BigInt,
    e: i64,
}

impl Float {
    fn zero() -> Self {
        Float {
            m: BigInt::zero(),
            e: 0,
        }
    }

    fn int(i: i64) -> Self {
        Float {
            m: BigInt::from(i),
            e: 0,
        }
    }

    fn is_zero(&self) -> bool {
        self.m.is_zero()
    }

    fn is_negative(&self) -> bool {
        self.m.is_negative()
    }

    /// The position of the highest bit: 2^(magnitude − 1) ≤ |x| < 2^magnitude.
    fn magnitude(&self) -> i64 {
        if self.is_zero() {
            i64::MIN / 4
        } else {
            self.m.bits() as i64 + self.e
        }
    }

    /// Rounds the mantissa to `prec` bits, halves away from zero.
    fn round(mut self, prec: u64) -> Self {
        let bits = self.m.bits();
        if bits > prec {
            let shift = bits - prec;
            let sign = self.m.sign();
            let magnitude = (self.m.magnitude() + (BigUint::one() << (shift - 1))) >> shift;
            self.m = BigInt::from_biguint(sign, magnitude);
            self.e += shift as i64;
        }
        self
    }

    fn from_rational(x: &BigRational, prec: u64) -> Self {
        if x.is_zero() {
            return Float::zero();
        }
        let (n, d) = (x.numer(), x.denom());
        let shift = prec as i64 + 2 - (n.bits() as i64 - d.bits() as i64);
        let m = if shift >= 0 {
            (n << shift as usize) / d
        } else {
            n / (d << (-shift) as usize)
        };
        Float { m, e: -shift }.round(prec)
    }

    fn to_rational(&self) -> BigRational {
        if self.e >= 0 {
            BigRational::from_integer(&self.m << self.e as usize)
        } else {
            BigRational::new(self.m.clone(), BigInt::one() << (-self.e) as usize)
        }
    }

    /// An approximation for decisions (the size of a number, a quadrant).
    fn to_f64(&self) -> f64 {
        if self.is_zero() {
            return 0.0;
        }
        let shift = self.m.bits().saturating_sub(60);
        let top = (&self.m >> shift as usize).to_f64().unwrap_or(0.0);
        let e = (self.e + shift as i64).clamp(-2000, 2000) as i32;
        top * 2f64.powi(e)
    }

    fn neg(mut self) -> Self {
        self.m = -self.m;
        self
    }

    fn abs(mut self) -> Self {
        self.m = self.m.abs();
        self
    }

    /// Multiplies by 2^k.
    fn shl(mut self, k: i64) -> Self {
        self.e += k;
        self
    }

    fn mul(&self, other: &Float, prec: u64) -> Float {
        Float {
            m: &self.m * &other.m,
            e: self.e + other.e,
        }
        .round(prec)
    }

    fn div(&self, other: &Float, prec: u64) -> Float {
        let shift = (prec as i64 + 2 - (self.m.bits() as i64 - other.m.bits() as i64)).max(0);
        Float {
            m: (&self.m << shift as usize) / &other.m,
            e: self.e - other.e - shift,
        }
        .round(prec)
    }

    fn add(&self, other: &Float, prec: u64) -> Float {
        if self.is_zero() {
            return other.clone().round(prec);
        }
        if other.is_zero() {
            return self.clone().round(prec);
        }
        // An addend far below the other's last bit can't change it.
        let (big, small) = if self.magnitude() >= other.magnitude() {
            (self, other)
        } else {
            (other, self)
        };
        if big.magnitude() - small.magnitude() > prec as i64 + 8 {
            return big.clone().round(prec);
        }
        let e = self.e.min(other.e);
        let m = (&self.m << (self.e - e) as usize) + (&other.m << (other.e - e) as usize);
        Float { m, e }.round(prec)
    }

    fn sub(&self, other: &Float, prec: u64) -> Float {
        self.add(&other.clone().neg(), prec)
    }

    fn sqrt(&self, prec: u64) -> Float {
        if self.is_zero() {
            return Float::zero();
        }
        let mut shift = (2 * prec as i64 + 4 - self.m.bits() as i64).max(0);
        if (self.e - shift).rem_euclid(2) != 0 {
            shift += 1;
        }
        Float {
            m: (&self.m << shift as usize).sqrt(),
            e: (self.e - shift) / 2,
        }
        .round(prec)
    }

    /// The nearest integer.
    fn round_int(&self) -> BigInt {
        if self.e >= 0 {
            return &self.m << self.e as usize;
        }
        let shift = (-self.e) as usize;
        let half = BigInt::one() << (shift - 1);
        let magnitude = (self.m.abs() + half) >> shift;
        if self.is_negative() {
            -magnitude
        } else {
            magnitude
        }
    }

    /// `x` as a fixed point number with `q` fractional bits.
    fn fixed(&self, q: u64) -> BigInt {
        let shift = self.e + q as i64;
        if shift >= 0 {
            &self.m << shift as usize
        } else {
            &self.m >> (-shift) as usize
        }
    }

    fn from_fixed(v: BigInt, q: u64) -> Float {
        Float {
            m: v,
            e: -(q as i64),
        }
    }
}

/// The number of bits of |k|.
fn bit_length(k: i64) -> u64 {
    64 - k.unsigned_abs().leading_zeros() as u64
}

// --------------------------------------------------------------- constants

/// `atan(1/n)`, or `atanh(1/n)`, with `q` fractional bits:
/// Σ (±1)^k / ((2k + 1) n^(2k+1)).
fn arctan_inverse(n: u32, q: u64, hyperbolic: bool) -> BigInt {
    let n2 = BigInt::from(n) * n;
    let mut power = (BigInt::one() << q as usize) / n;
    let mut sum = power.clone();
    let mut k = 1u64;
    loop {
        power /= &n2;
        if power.is_zero() {
            return sum;
        }
        let term = &power / (2 * k + 1);
        if hyperbolic || k.is_multiple_of(2) {
            sum += term;
        } else {
            sum -= term;
        }
        k += 1;
    }
}

/// π (Machin's formula: 16 atan(1/5) − 4 atan(1/239)).
fn pi_float(prec: u64) -> Float {
    let q = prec + 16;
    let v = arctan_inverse(5, q, false) * 16 - arctan_inverse(239, q, false) * 4;
    Float::from_fixed(v, q).round(prec)
}

/// ln 2 = 2 atanh(1/3).
fn ln2_float(prec: u64) -> Float {
    let q = prec + 16;
    Float::from_fixed(arctan_inverse(3, q, true) * 2, q).round(prec)
}

/// ln 10 = 3 ln 2 + ln 1.25, and ln 1.25 = 2 atanh(1/9).
fn ln10_float(prec: u64) -> Float {
    let q = prec + 16;
    let v = arctan_inverse(3, q, true) * 6 + arctan_inverse(9, q, true) * 2;
    Float::from_fixed(v, q).round(prec)
}

// --------------------------------------------------------- elementary series

/// e^x. The callers keep |x| within Windows' limit of 100000 (twice that for `tanh`'s
/// e^−2|x|). Far beyond it, k would no longer fit and the series would never end, so
/// that's `Invalid input`, like `exprat`, rather than a hang.
fn exp_float(x: &Float, prec: u64) -> Result<Float, CalcError> {
    if x.to_f64().abs() > 4.0 * MAX_EXP {
        return Err(CalcError::InvalidInput);
    }
    if x.is_zero() {
        return Ok(Float::int(1));
    }
    // x = k ln 2 + r with |r| ≤ ln 2 / 2; e^r from its Taylor series after halving r
    // 16 times, then squared back. The guard bits cover k and the squarings.
    const HALVINGS: u64 = 16;
    let k = (x.to_f64() / std::f64::consts::LN_2).round() as i64;
    let q = prec + bit_length(k) + HALVINGS + 24;
    let r = x.sub(&ln2_float(q).mul(&Float::int(k), q), q);
    let r = r.fixed(q) >> HALVINGS as usize;
    let one = BigInt::one() << q as usize;
    let mut sum = one.clone();
    let mut term = one;
    let mut n = 1u64;
    loop {
        term = ((term * &r) >> q as usize) / n;
        if term.is_zero() {
            break;
        }
        sum += &term;
        n += 1;
    }
    for _ in 0..HALVINGS {
        sum = (&sum * &sum) >> q as usize;
    }
    Ok(Float {
        m: sum,
        e: k - q as i64,
    }
    .round(prec))
}

/// ln x for x > 0.
fn ln_float(x: &Float, prec: u64) -> Float {
    // x = y · 2^k with y in [1/√2, √2): ln x = k ln 2 + ln y, and ln y = 2 atanh(z)
    // = 2z (1 + z²/3 + z⁴/5 + …) with z = (y − 1)/(y + 1), |z| < 0.172. y − 1 is exact,
    // so ln y keeps its relative precision near 1.
    let q = prec + 24;
    let mut k = x.m.bits() as i64 - 1 + x.e;
    let mut y = Float {
        m: x.m.clone(),
        e: x.e - k,
    };
    if y.to_f64() > std::f64::consts::SQRT_2 {
        y = y.shl(-1);
        k += 1;
    }
    let exact = y.m.bits() + 2;
    let one = Float::int(1);
    let z = y.sub(&one, exact).div(&y.add(&one, exact), q);
    let z2 = z.mul(&z, q).fixed(q);
    let unit = BigInt::one() << q as usize;
    let mut sum = unit.clone();
    let mut power = unit;
    let mut j = 1u64;
    loop {
        power = (power * &z2) >> q as usize;
        if power.is_zero() {
            break;
        }
        sum += &power / (2 * j + 1);
        j += 1;
    }
    let ln_y = z.mul(&Float::from_fixed(sum, q), q).shl(1);
    if k == 0 {
        return ln_y.round(prec);
    }
    let q = q + bit_length(k);
    ln2_float(q).mul(&Float::int(k), q).add(&ln_y, prec)
}

/// The sums of sin x / x and cos x, or of sinh x / x and cosh x (`hyperbolic`), for
/// |x| ≤ 1: 1 ∓ x²/3! + x⁴/5! … and 1 ∓ x²/2! + x⁴/4! …, both near 1, so sin x =
/// x · (the first) keeps its relative precision for small x.
fn sin_cos_sums(x: &Float, prec: u64, hyperbolic: bool) -> (Float, Float) {
    let q = prec + 24;
    let x2 = x.mul(x, q).fixed(q);
    let one = BigInt::one() << q as usize;
    let (mut s, mut c, mut term) = (one.clone(), one.clone(), one);
    let mut j = 1u64;
    loop {
        // term = x^(2j) / (2j)!
        term = ((term * &x2) >> q as usize) / ((2 * j - 1) * (2 * j));
        if term.is_zero() {
            break;
        }
        let sine_term = &term / (2 * j + 1);
        if hyperbolic || j.is_multiple_of(2) {
            c += &term;
            s += sine_term;
        } else {
            c -= &term;
            s -= sine_term;
        }
        j += 1;
    }
    (Float::from_fixed(s, q), Float::from_fixed(c, q))
}

/// sin x and cos x for |x| ≤ π/4.
fn sin_cos_small(x: &Float, prec: u64) -> (Float, Float) {
    let (s, c) = sin_cos_sums(x, prec, false);
    (x.mul(&s, prec), c.round(prec))
}

/// atan x for |x| ≤ 1: the argument is halved four times
/// (atan x = 2 atan(x / (1 + √(1 + x²)))), then x (1 − x²/3 + x⁴/5 − …).
fn atan_small(x: &Float, prec: u64) -> Float {
    const HALVINGS: i64 = 4;
    let q = prec + 24;
    let one = Float::int(1);
    let mut x = x.clone();
    for _ in 0..HALVINGS {
        let root = one.add(&x.mul(&x, q), q).sqrt(q);
        x = x.div(&one.add(&root, q), q);
    }
    let x2 = x.mul(&x, q).fixed(q);
    let unit = BigInt::one() << q as usize;
    let mut sum = unit.clone();
    let mut power = unit;
    let mut j = 1u64;
    loop {
        power = (power * &x2) >> q as usize;
        if power.is_zero() {
            break;
        }
        let term = &power / (2 * j + 1);
        if j.is_multiple_of(2) {
            sum += term;
        } else {
            sum -= term;
        }
        j += 1;
    }
    x.mul(&Float::from_fixed(sum, q), prec).shl(HALVINGS)
}

/// atan x for any x: atan x = ±π/2 − atan(1/x) beyond 1.
fn atan_float(x: &Float, prec: u64) -> Float {
    if x.is_zero() {
        return Float::zero();
    }
    if x.magnitude() <= 0 || x.clone().abs().to_f64() <= 1.0 {
        return atan_small(x, prec);
    }
    let q = prec + 8;
    let inverse = Float::int(1).div(x, q);
    let half_pi = pi_float(q).shl(-1);
    let result = half_pi.sub(&atan_small(&inverse.clone().abs(), q), q);
    if x.is_negative() {
        result.neg().round(prec)
    } else {
        result.round(prec)
    }
}

// --------------------------------------------------------------- finishing

/// A result for the engine: rounded to `INTERNAL_DIGITS` significant digits, or
/// `Overflow` when its decimal exponent is beyond Windows' four digits however the display
/// rounds it (`exponent_overflows`).
fn finish(f: &Float) -> Result<Number, CalcError> {
    if f.is_zero() {
        return Ok(Number::zero());
    }
    // 2^33300 is about 10^10024: certainly too large (or too small) for the display.
    if f.magnitude().abs() > 33_300 {
        return Err(CalcError::Overflow);
    }
    let value = round_digits(&f.to_rational(), INTERNAL_DIGITS);
    let magnitude = value.numer().abs();
    if exponent_overflows(exponent(&magnitude, value.denom())) {
        return Err(CalcError::Overflow);
    }
    Ok(value)
}

/// `v` rounded to `digits` significant digits.
pub(super) fn round_digits(v: &Number, digits: i64) -> Number {
    if v.is_zero() {
        return Number::zero();
    }
    let magnitude = v.numer().abs();
    let k = digits - 1 - exponent(&magnitude, v.denom());
    let mut rounded = scaled_round(&magnitude, v.denom(), k);
    if v.is_negative() {
        rounded = -rounded;
    }
    if k >= 0 {
        Number::new(rounded, pow10(k))
    } else {
        Number::from_integer(rounded * pow10(-k))
    }
}

/// The decimal exponent of a non-zero rational (floor of log10 |v|).
fn decimal_exponent(v: &Number) -> i64 {
    exponent(&v.numer().abs(), v.denom())
}

/// 10^-32 (`rat_smallest`).
fn smallest() -> Number {
    Number::new(BigInt::one(), pow10(SMALLEST_DIGITS))
}

/// Whether |v| ≤ 10^-32 (`rat_smallest`), Windows' "practically zero".
fn is_negligible(v: &Float) -> bool {
    if v.is_zero() {
        return true;
    }
    if v.magnitude() > -100 {
        return false;
    }
    v.to_rational().abs() <= smallest()
}

/// The extra bits that keep the relative precision of a function f(x) ≈ x near 0.
fn extra_bits(x: &Number) -> u64 {
    if x.is_zero() {
        return 0;
    }
    let bits = x.numer().bits() as i64 - x.denom().bits() as i64;
    (-bits).max(0) as u64
}

// ---------------------------------------------------------------- constants

/// π, rounded to the engine's internal precision (`IDC_PI`).
pub fn pi() -> Number {
    static PI: OnceLock<Number> = OnceLock::new();
    PI.get_or_init(|| round_digits(&pi_float(PREC).to_rational(), INTERNAL_DIGITS))
        .clone()
}

/// Euler's number e (`IDC_EULER`).
pub fn euler() -> Number {
    static E: OnceLock<Number> = OnceLock::new();
    E.get_or_init(|| {
        let e = exp_float(&Float::int(1), PREC).expect("e^1 is within range");
        round_digits(&e.to_rational(), INTERNAL_DIGITS)
    })
    .clone()
}

/// A random number in [0, 1), like Windows' `GenerateRandomNumber`: a double from a
/// uniform distribution, written with 32 decimals (`setprecision(32)`).
pub fn random() -> Number {
    static STATE: AtomicU64 = AtomicU64::new(0);
    static SEED: OnceLock<u64> = OnceLock::new();
    let seed = *SEED.get_or_init(|| {
        use std::hash::{BuildHasher, Hasher};
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        // web_time, not std::time: the browser has no `SystemTime::now()`, it panics there.
        hasher.write_u128(
            web_time::SystemTime::now()
                .duration_since(web_time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        );
        hasher.finish()
    });
    // SplitMix64.
    let mut z = seed.wrapping_add(
        STATE
            .fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
            .wrapping_add(0x9E37_79B9_7F4A_7C15),
    );
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    // 53 random bits, as in a double.
    let double = Number::new(BigInt::from(z >> 11), BigInt::one() << 53);
    let scaled = scaled_round(double.numer(), double.denom(), 32);
    Number::new(scaled, pow10(32))
}

// ------------------------------------------------------- powers and logs

/// ln x (`lograt`): x ≤ 0 is `Invalid input`.
pub fn ln(x: &Number) -> Result<Number, CalcError> {
    finish(&ln_checked(x, PREC)?)
}

fn ln_checked(x: &Number, prec: u64) -> Result<Float, CalcError> {
    if !x.is_positive() {
        return Err(CalcError::InvalidInput);
    }
    if x.is_one() {
        return Ok(Float::zero());
    }
    // Near 1, ln x ≈ x − 1: x needs as many more bits as x − 1 has leading zeros.
    let extra = extra_bits(&(x - Number::one()));
    let p = prec + extra;
    Ok(ln_float(&Float::from_rational(x, p), p))
}

/// log₁₀ x (`log10rat`).
pub fn log10(x: &Number) -> Result<Number, CalcError> {
    let ln = ln_checked(x, PREC + 16)?;
    finish(&ln.div(&ln10_float(PREC + 16), PREC))
}

/// The base-`y` logarithm of `x` (`IDC_LOGBASEY`: `Log(x) / Log(y)`).
pub fn log_base(x: &Number, y: &Number) -> Result<Number, CalcError> {
    let ln_x = ln_checked(x, PREC + 16)?;
    let ln_y = ln_checked(y, PREC + 16)?;
    if ln_y.is_zero() {
        return Err(if ln_x.is_zero() {
            CalcError::Undefined
        } else {
            CalcError::DivideByZero
        });
    }
    finish(&ln_x.div(&ln_y, PREC))
}

/// e^x (`exprat`): beyond ±100000 Windows gives up with `Invalid input`.
pub fn exp(x: &Number) -> Result<Number, CalcError> {
    if x.to_f64().is_none_or(|v| v.abs() > MAX_EXP) {
        return Err(CalcError::InvalidInput);
    }
    finish(&exp_float(&Float::from_rational(x, PREC + 32), PREC)?)
}

/// `x^y` (`powrat`). Integer powers are exact while the result stays a reasonable size.
/// A negative base only takes exponents whose denominator is odd ((-8)^(1/3) is -2);
/// 0 to a negative power is `Invalid input`; and like `exprat`, a result beyond
/// e^±100000 is too.
pub fn power(x: &Number, y: &Number) -> Result<Number, CalcError> {
    if x.is_zero() {
        return match y.cmp(&Number::zero()) {
            std::cmp::Ordering::Less => Err(CalcError::InvalidInput),
            std::cmp::Ordering::Equal => Ok(Number::one()),
            std::cmp::Ordering::Greater => Ok(Number::zero()),
        };
    }
    if y.is_zero() {
        return Ok(Number::one());
    }
    if y.is_one() {
        return Ok(x.clone());
    }
    let negative = x.is_negative();
    let base = x.abs();
    // A positive base within 10^-32 of 1 is 1.
    let smallest = smallest();
    if !negative && (&base - Number::one()).abs() < smallest {
        return Ok(Number::one());
    }
    // ln|x| for the size check; f64 is plenty for comparing with 100000.
    let ln_base = log2_approx(&base) * std::f64::consts::LN_2;

    // An exponent within 10^-32 of an integer counts as one.
    let fraction = y.fract();
    if fraction.abs() < smallest {
        let n = y.trunc().to_integer();
        let Some(n) = n.to_i32() else {
            // `rattoi32` refuses exponents beyond 32 bits.
            return Err(CalcError::InvalidInput);
        };
        if (f64::from(n) * ln_base).abs() > MAX_EXP {
            return Err(CalcError::InvalidInput);
        }
        let result_negative = negative && n % 2 != 0;
        // Exact while the numbers stay under some 100000 bits.
        let size = (base.numer().bits() + base.denom().bits()) * u64::from(n.unsigned_abs());
        let magnitude = if size <= 100_000 {
            let exact = base.pow(n);
            if exponent_overflows(decimal_exponent(&exact)) {
                return Err(CalcError::Overflow);
            }
            exact
        } else {
            let ln = ln_checked(&base, PREC + 40)?;
            finish(&exp_float(&ln.mul(&Float::int(n.into()), PREC + 40), PREC)?)?
        };
        return Ok(if result_negative {
            -magnitude
        } else {
            magnitude
        });
    }

    // A fractional power of a negative number: only odd denominators.
    let mut result_negative = false;
    if negative {
        if y.denom().bit(0) {
            result_negative = y.numer().bit(0);
        } else {
            return Err(CalcError::InvalidInput);
        }
    }
    if (y.to_f64().unwrap_or(f64::INFINITY) * ln_base).abs() > MAX_EXP {
        return Err(CalcError::InvalidInput);
    }
    // |y| more bits keep y ln x precise enough for e^(y ln x).
    let y_bits = (y.numer().bits() as i64 - y.denom().bits() as i64).max(0) as u64;
    let p = PREC + 40 + y_bits;
    let ln = ln_checked(&base, p)?;
    let product = ln.mul(&Float::from_rational(y, p), p);
    // The estimate above is 0 for a base within about 10^-16 of 1, so like `exprat`,
    // check the product itself: 1.00000000000000001^(10^40 + 0.5) is `Invalid input`.
    if product.to_f64().abs() > MAX_EXP {
        return Err(CalcError::InvalidInput);
    }
    let magnitude = finish(&exp_float(&product, PREC)?)?;
    Ok(if result_negative {
        -magnitude
    } else {
        magnitude
    })
}

/// The `y`th root of `x` (`IDC_ROOT`: `Pow(x, 1/y)`); the 0th root divides by zero.
pub fn root(x: &Number, y: &Number) -> Result<Number, CalcError> {
    if y.is_zero() {
        return Err(CalcError::DivideByZero);
    }
    power(x, &y.recip())
}

/// log₂ |x| as a float, for size checks.
fn log2_approx(x: &Number) -> f64 {
    let (n, d) = (x.numer(), x.denom());
    let top = |v: &BigInt| {
        let shift = v.bits().saturating_sub(60);
        ((v >> shift as usize).to_f64().unwrap_or(1.0)).log2() + shift as f64
    };
    top(n) - top(d)
}

// ------------------------------------------------------------ trigonometry

/// Windows refuses trigonometric and hyperbolic functions from 10^100 on
/// (`IsCurrentTooBigForTrig`); only positive numbers count.
pub fn too_big_for_trig(x: &Number) -> bool {
    x.is_positive() && decimal_exponent(x) >= MAX_TRIG_EXPONENT
}

/// sin x and cos x, with `x` in `angle` units. Degrees and gradians are reduced to a
/// quarter turn exactly (so multiples of 90° give exact 0 and ±1); radians with as many
/// digits of π as `x` needs.
fn sin_cos(x: &Number, angle: Angle) -> (Float, Float) {
    let p = PREC + extra_bits(x);
    let (quadrant, r) = match angle.quarter() {
        Some(quarter) => {
            let quarter = Number::from_integer(quarter.into());
            let n = (x / &quarter).round();
            let rest = x - &n * &quarter;
            let radians = Float::from_rational(&rest, p)
                .mul(&pi_float(p).shl(-1), p)
                .div(&Float::from_rational(&quarter, p), p);
            (n.to_integer(), radians)
        }
        None => {
            let bits = (x.numer().bits() as i64 - x.denom().bits() as i64).max(0) as u64;
            let q = p + bits + 16;
            let xf = Float::from_rational(x, q);
            let half_pi = pi_float(q).shl(-1);
            let n = xf.div(&half_pi, q).round_int();
            let rest = xf.sub(&half_pi.mul(&Float { m: n.clone(), e: 0 }, q), q);
            (n, rest)
        }
    };
    let (s, c) = sin_cos_small(&r, p);
    let quadrant: BigInt = ((quadrant % 4) + 4) % 4;
    match quadrant.to_u8() {
        Some(0) => (s, c),
        Some(1) => (c, s.neg()),
        Some(2) => (s.neg(), c.neg()),
        _ => (c.neg(), s),
    }
}

/// A sine or cosine for the engine: practically 0 is 0 (`_sinrat`, `_cosrat`).
fn finish_sin_cos(v: Float) -> Float {
    if is_negligible(&v) { Float::zero() } else { v }
}

pub fn sin(x: &Number, angle: Angle) -> Result<Number, CalcError> {
    finish(&finish_sin_cos(sin_cos(x, angle).0))
}

pub fn cos(x: &Number, angle: Angle) -> Result<Number, CalcError> {
    finish(&finish_sin_cos(sin_cos(x, angle).1))
}

/// tan x (`_tanrat`): `Invalid input` where the cosine is 0.
pub fn tan(x: &Number, angle: Angle) -> Result<Number, CalcError> {
    let (s, c) = sin_cos(x, angle);
    let (s, c) = (finish_sin_cos(s), finish_sin_cos(c));
    if c.is_zero() {
        return Err(CalcError::InvalidInput);
    }
    finish(&s.div(&c, PREC))
}

/// An angle in radians converted to `angle` units (`ascalerat`).
fn from_radians(r: Float, angle: Angle, prec: u64) -> Float {
    match angle {
        Angle::Radians => r,
        Angle::Degrees => r.mul(&Float::int(180), prec).div(&pi_float(prec), prec),
        Angle::Gradians => r.mul(&Float::int(200), prec).div(&pi_float(prec), prec),
    }
}

/// asin x (`asinrat`): |x| more than 10^-32 beyond 1 is `Invalid input`.
pub fn asin(x: &Number, angle: Angle) -> Result<Number, CalcError> {
    let p = PREC + 16 + extra_bits(x);
    let r = asin_float(x, p)?;
    finish(&from_radians(r, angle, p))
}

fn asin_float(x: &Number, p: u64) -> Result<Float, CalcError> {
    let one = Number::one();
    let smallest = smallest();
    let distance = x.abs() - &one;
    if distance > smallest {
        return Err(CalcError::InvalidInput);
    }
    if distance.abs() <= smallest {
        let half_pi = pi_float(p).shl(-1);
        return Ok(if x.is_negative() {
            half_pi.neg()
        } else {
            half_pi
        });
    }
    // asin x = atan(x / √((1 − x)(1 + x))).
    let below = Float::from_rational(&((&one - x) * (&one + x)), p);
    let xf = Float::from_rational(x, p);
    Ok(atan_float(&xf.div(&below.sqrt(p), p), p))
}

/// acos x (`acosrat`): exactly ±1 give 0 and π, otherwise π/2 − asin x.
pub fn acos(x: &Number, angle: Angle) -> Result<Number, CalcError> {
    let p = PREC + 16;
    let r = if x.is_one() {
        Float::zero()
    } else if (-x).is_one() {
        pi_float(p)
    } else {
        let asin = asin_float(x, p + extra_bits(x))?;
        pi_float(p).shl(-1).sub(&asin, p)
    };
    finish(&from_radians(r, angle, p))
}

pub fn atan(x: &Number, angle: Angle) -> Result<Number, CalcError> {
    let p = PREC + 16 + extra_bits(x);
    let r = atan_float(&Float::from_rational(x, p), p);
    finish(&from_radians(r, angle, p))
}

/// 1/x for `1/x` and the reciprocal functions (sec, csc, cot and their inverses).
pub fn invert(x: &Number) -> Result<Number, CalcError> {
    if x.is_zero() {
        Err(CalcError::DivideByZero)
    } else {
        Ok(x.recip())
    }
}

// -------------------------------------------------------------- hyperbolic

/// sinh x (`sinhrat`): from 1 on with e^x (so up to 100000), below with its series,
/// which Windows doesn't start below −10000 (`IsValidForHypFunc`).
pub fn sinh(x: &Number) -> Result<Number, CalcError> {
    finish(&sinh_float(x)?)
}

fn sinh_float(x: &Number) -> Result<Float, CalcError> {
    let v = x.to_f64().unwrap_or(f64::NEG_INFINITY);
    if !(-MAX_EXP / 10.0..=MAX_EXP).contains(&v) {
        return Err(CalcError::InvalidInput);
    }
    let p = PREC + 16 + extra_bits(x);
    let xf = Float::from_rational(x, p);
    if v.abs() < 1.0 {
        let (s, _) = sin_cos_sums(&xf, p, true);
        return Ok(xf.mul(&s, p));
    }
    let (plus, minus) = (exp_float(&xf, p)?, exp_float(&xf.clone().neg(), p)?);
    Ok(plus.sub(&minus, p).shl(-1))
}

/// cosh x (`coshrat`), of |x|: e^|x| up to 100000.
pub fn cosh(x: &Number) -> Result<Number, CalcError> {
    finish(&cosh_float(x)?)
}

fn cosh_float(x: &Number) -> Result<Float, CalcError> {
    let v = x.to_f64().map_or(f64::INFINITY, f64::abs);
    if v > MAX_EXP {
        return Err(CalcError::InvalidInput);
    }
    let p = PREC + 16;
    let xf = Float::from_rational(&x.abs(), p);
    if v < 1.0 {
        return Ok(sin_cos_sums(&xf, p, true).1);
    }
    let (plus, minus) = (exp_float(&xf, p)?, exp_float(&xf.clone().neg(), p)?);
    Ok(plus.add(&minus, p).shl(-1))
}

/// tanh x (`tanhrat`: sinh x / cosh x, with both their limits).
pub fn tanh(x: &Number) -> Result<Number, CalcError> {
    let v = x.to_f64().unwrap_or(f64::NEG_INFINITY);
    if !(-MAX_EXP / 10.0..=MAX_EXP).contains(&v) {
        return Err(CalcError::InvalidInput);
    }
    let p = PREC + 16 + extra_bits(x);
    let xf = Float::from_rational(x, p);
    if v.abs() < 1.0 {
        let (s, c) = sin_cos_sums(&xf, p, true);
        return finish(&xf.mul(&s, p).div(&c, PREC));
    }
    // (1 − e^−2|x|) / (1 + e^−2|x|), which needs no huge powers.
    let one = Float::int(1);
    let small = exp_float(&xf.clone().abs().shl(1).neg(), p)?;
    let t = one.sub(&small, p).div(&one.add(&small, p), PREC);
    finish(&if xf.is_negative() { t.neg() } else { t })
}

/// asinh x = ln(x + √(x² + 1)), of |x| with the sign of x.
pub fn asinh(x: &Number) -> Result<Number, CalcError> {
    if x.is_zero() {
        return Ok(Number::zero());
    }
    let p = PREC + 16 + 2 * extra_bits(x);
    let a = Float::from_rational(&x.abs(), p);
    let root = a.mul(&a, p).add(&Float::int(1), p).sqrt(p);
    let r = ln_float(&a.add(&root, p), p);
    finish(&if x.is_negative() { r.neg() } else { r })
}

/// acosh x = ln(x + √(x² − 1)), `Invalid input` below 1.
pub fn acosh(x: &Number) -> Result<Number, CalcError> {
    if *x < Number::one() {
        return Err(CalcError::InvalidInput);
    }
    if x.is_one() {
        return Ok(Number::zero());
    }
    let p = PREC + 16 + extra_bits(&(x - Number::one()));
    let below = Float::from_rational(&((x - Number::one()) * (x + Number::one())), p);
    let xf = Float::from_rational(x, p);
    finish(&ln_float(&xf.add(&below.sqrt(p), p), p))
}

/// atanh x = ½ ln((1 + x)/(1 − x)) (`atanhrat`): 1 divides by zero, and from −1 down
/// the logarithm's argument isn't positive.
pub fn atanh(x: &Number) -> Result<Number, CalcError> {
    let one = Number::one();
    if x.is_one() {
        return Err(CalcError::DivideByZero);
    }
    let quotient = (&one + x) / (&one - x);
    let p = PREC + 16 + extra_bits(x);
    let ln = ln_checked(&quotient, p)?;
    finish(&ln.shl(-1))
}

// --------------------------------------------------------------- factorial

/// x! (`factrat`): exact for integers, Γ(x + 1) otherwise. Beyond 3249 or below −1000
/// Windows says `Overflow` right away; negative integers are `Invalid input`. Numbers
/// within 10^-32 of an integer count as one.
pub fn factorial(x: &Number) -> Result<Number, CalcError> {
    if *x > Number::from_integer(MAX_FACT.into()) || *x < Number::from_integer(MIN_FACT.into()) {
        return Err(CalcError::Overflow);
    }
    let nearest = x.round();
    if (x - &nearest).abs() < smallest() {
        if nearest.is_negative() {
            return Err(CalcError::InvalidInput);
        }
        let n = nearest.to_integer().to_u64().unwrap_or(0);
        let mut product = BigInt::one();
        for i in 2..=n {
            product *= i;
        }
        let result = Number::from_integer(product);
        if decimal_exponent(&result) > MAX_EXPONENT {
            return Err(CalcError::Overflow);
        }
        return Ok(result);
    }
    // x! = x (x − 1) … (t + 1) · Γ(t + 1) with t in (−1, 0), or Γ(t + 1) / ((t + 1) … x)
    // for negative x. With x = a/b every factor is (a − ib)/b; the product is kept as one
    // numerator and one denominator, since reducing thousands of fractions takes long.
    let (a, b) = (x.numer().clone(), x.denom().clone());
    let mut t = a.clone();
    let (mut numerator, mut denominator) = (BigInt::one(), BigInt::one());
    while t.is_positive() {
        numerator *= &t;
        denominator *= &b;
        t -= &b;
    }
    while t < -&b {
        t += &b;
        numerator *= &b;
        denominator *= &t;
    }
    if denominator.is_negative() {
        numerator = -numerator;
        denominator = -denominator;
    }
    let p = PREC + 16;
    let z = Number::new(t + &b, b);
    let gamma = gamma_unit(&Float::from_rational(&z, p), p)?;
    let factor = Float::from_rational(&BigRational::new_raw(numerator, denominator), p);
    finish(&gamma.mul(&factor, PREC))
}

/// Γ(z) for z in (0, 1): Γ(z) = Γ(z + N) / (z (z + 1) … (z + N − 1)), and ln Γ(z + N)
/// from Stirling's series, which is far more precise than needed that far from 0.
fn gamma_unit(z: &Float, prec: u64) -> Result<Float, CalcError> {
    const SHIFT: i64 = 40;
    let q = prec + 32;
    let w = z.add(&Float::int(SHIFT), q);
    // ln Γ(w) = (w − ½) ln w − w + ½ ln 2π + Σ B₂ₖ / (2k (2k − 1) w^(2k−1)).
    let ln_w = ln_float(&w, q);
    let half = Float {
        m: BigInt::one(),
        e: -1,
    };
    let mut sum = w.sub(&half, q).mul(&ln_w, q).sub(&w, q);
    let two_pi = pi_float(q).shl(1);
    sum = sum.add(&ln_float(&two_pi, q).shl(-1), q);
    let w2 = w.mul(&w, q);
    let mut power = w.clone();
    for (k, b) in bernoulli().iter().enumerate() {
        let k = k as i64 + 1;
        let coefficient = Float::from_rational(b, q).div(&Float::int(2 * k * (2 * k - 1)), q);
        sum = sum.add(&coefficient.div(&power, q), q);
        power = power.mul(&w2, q);
    }
    let mut product = z.clone();
    for i in 1..SHIFT {
        product = product.mul(&z.add(&Float::int(i), q), q);
    }
    Ok(exp_float(&sum, q)?.div(&product, prec))
}

/// The Bernoulli numbers B₂, B₄, … B₆₀, exact.
fn bernoulli() -> &'static [Number] {
    static NUMBERS: OnceLock<Vec<Number>> = OnceLock::new();
    NUMBERS.get_or_init(|| {
        // B_m = −1/(m + 1) Σ_{k<m} C(m + 1, k) B_k.
        const COUNT: usize = 60;
        let mut b: Vec<Number> = vec![Number::one()];
        for m in 1..=COUNT {
            let mut sum = Number::zero();
            let mut binomial = BigInt::one();
            for (k, bk) in b.iter().enumerate() {
                sum += Number::from_integer(binomial.clone()) * bk;
                binomial = binomial * (m + 1 - k) / (k + 1);
            }
            b.push(-sum / Number::from_integer((m + 1).into()));
        }
        b.into_iter().skip(2).step_by(2).collect()
    })
}

// ------------------------------------------------------------- exact functions

/// Decimal degrees to degrees, minutes and seconds as D.MMSS (`IDC_DMS`), or back
/// (`IDC_DEGREES`), exactly as Windows does it.
pub fn dms(x: &Number, inverse: bool) -> Number {
    let (shift_in, shift_out) = if inverse { (100, 60) } else { (60, 100) };
    let shift_in = Number::from_integer(shift_in.into());
    let shift_out = Number::from_integer(shift_out.into());
    let degrees = x.trunc();
    let minutes = (x - &degrees) * &shift_in;
    let whole_minutes = minutes.trunc();
    let seconds = (&minutes - &whole_minutes) * &shift_in / &shift_out;
    degrees + (whole_minutes + seconds) / shift_out
}

/// The remainder with the sign of the divisor (`modrat`); `x mod 0` is x.
pub fn modulo(x: &Number, y: &Number) -> Number {
    if y.is_zero() {
        return x.clone();
    }
    let remainder = x - y * (x / y).trunc();
    if !remainder.is_zero() && remainder.is_negative() != y.is_negative() {
        remainder + y
    } else {
        remainder
    }
}

/// ⌊x⌋ and ⌈x⌉ (`IDC_FLOOR`, `IDC_CEIL`).
pub fn floor(x: &Number) -> Number {
    x.floor()
}

pub fn ceil(x: &Number) -> Number {
    x.ceil()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{format_with, parse_decimal};

    fn n(text: &str) -> Number {
        match text.split_once('/') {
            Some((p, q)) => parse_decimal(p) / parse_decimal(q),
            None => parse_decimal(text),
        }
    }

    /// The result as the Scientific display writes it: 32 digits, no grouping.
    fn show(result: Result<Number, CalcError>) -> String {
        match result {
            Ok(v) => format_with(&v, false, 32, false),
            Err(error) => error.message().to_string(),
        }
    }

    #[test]
    fn constants() {
        assert_eq!(show(Ok(pi())), "3.1415926535897932384626433832795");
        assert_eq!(show(Ok(euler())), "2.7182818284590452353602874713527");
        let r = random();
        assert!(!r.is_negative() && r < Number::one());
        assert!(r.denom() <= &pow10(32));
    }

    #[test]
    fn logarithms() {
        assert_eq!(show(ln(&n("2"))), "0.69314718055994530941723212145818");
        assert_eq!(show(ln(&euler())), "1");
        assert_eq!(show(ln(&n("1"))), "0");
        assert_eq!(show(log10(&n("1000"))), "3");
        assert_eq!(show(log10(&n("0.001"))), "-3");
        assert_eq!(show(log10(&n("2"))), "0.30102999566398119521373889472449");
        assert_eq!(show(ln(&n("0"))), "Invalid input");
        assert_eq!(show(ln(&n("-5"))), "Invalid input");
        // Close to 1 the logarithm keeps its precision.
        assert_eq!(
            show(ln(&n("1.000000000000000000000000000001"))),
            "9.999999999999999999999999999995e-31"
        );
        assert_eq!(show(log_base(&n("8"), &n("2"))), "3");
        assert_eq!(show(log_base(&n("5"), &n("1"))), "Cannot divide by zero");
        assert_eq!(show(log_base(&n("1"), &n("1"))), "Result is undefined");
        assert_eq!(show(log_base(&n("-8"), &n("2"))), "Invalid input");
    }

    #[test]
    fn exponentials_and_powers() {
        assert_eq!(show(exp(&n("1"))), "2.7182818284590452353602874713527");
        assert_eq!(show(exp(&n("10"))), "22026.465794806716516957900645284");
        assert_eq!(show(exp(&n("-1"))), "0.36787944117144232159552377016146");
        assert_eq!(show(exp(&n("100001"))), "Invalid input");
        assert_eq!(show(exp(&n("50000"))), "Overflow");
        assert_eq!(show(power(&n("2"), &n("10"))), "1024");
        assert_eq!(
            show(power(&n("2"), &n("0.5"))),
            "1.4142135623730950488016887242097"
        );
        assert_eq!(show(power(&n("2"), &n("-2"))), "0.25");
        assert_eq!(show(power(&n("8"), &n("1/3"))), "2");
        assert_eq!(show(power(&n("27"), &n("2/3"))), "9");
        assert_eq!(show(power(&n("-8"), &n("1/3"))), "-2");
        assert_eq!(show(power(&n("-8"), &n("2/3"))), "4");
        assert_eq!(show(power(&n("-4"), &n("0.5"))), "Invalid input");
        assert_eq!(show(power(&n("-2"), &n("3"))), "-8");
        assert_eq!(show(power(&n("0"), &n("-1"))), "Invalid input");
        assert_eq!(show(power(&n("0"), &n("0"))), "1");
        assert_eq!(show(power(&n("10"), &n("9999"))), "1.e+9999");
        assert_eq!(show(power(&n("10"), &n("10000"))), "Overflow");
        assert_eq!(show(power(&n("10"), &n("50000"))), "Invalid input");
        // `exprat` checks y ln x itself: this close to 1 an f64 estimate of ln x is 0, but
        // the product is 150000.
        assert_eq!(
            show(power(
                &n("1.00000000000000001"),
                &n("15000000000000000000000.5")
            )),
            "Invalid input"
        );
        assert_eq!(
            show(power(
                &n("1.00000000000000001"),
                &n("10000000000000000000000000000000000000000.5")
            )),
            "Invalid input"
        );
        // Far beyond the limit, e^x itself refuses instead of never ending.
        assert!(matches!(
            exp_float(&Float::int(1 << 62), PREC),
            Err(CalcError::InvalidInput)
        ));
        assert_eq!(
            show(power(&n("1.5"), &n("2.5"))),
            "2.7556759606310753604719445840441"
        );
        assert_eq!(show(root(&n("27"), &n("3"))), "3");
        assert_eq!(show(root(&n("-32"), &n("5"))), "-2");
        assert_eq!(show(root(&n("16"), &n("4"))), "2");
        assert_eq!(show(root(&n("-16"), &n("4"))), "Invalid input");
        assert_eq!(show(root(&n("5"), &n("0"))), "Cannot divide by zero");
    }

    #[test]
    fn trigonometry_in_every_unit() {
        use Angle::*;
        assert_eq!(show(sin(&n("90"), Degrees)), "1");
        assert_eq!(show(sin(&n("30"), Degrees)), "0.5");
        assert_eq!(show(sin(&n("180"), Degrees)), "0");
        assert_eq!(show(sin(&n("-30"), Degrees)), "-0.5");
        assert_eq!(show(sin(&n("100"), Gradians)), "1");
        assert_eq!(show(sin(&(pi() / n("2")), Radians)), "1");
        assert_eq!(show(sin(&pi(), Radians)), "0");
        assert_eq!(
            show(sin(&n("1"), Radians)),
            "0.8414709848078965066525023216303"
        );
        assert_eq!(
            show(sin(&n("1"), Degrees)),
            "0.01745240643728351281941897851632"
        );
        assert_eq!(show(cos(&n("60"), Degrees)), "0.5");
        assert_eq!(show(cos(&n("90"), Degrees)), "0");
        assert_eq!(
            show(cos(&n("1"), Radians)),
            "0.54030230586813971740093660744298"
        );
        assert_eq!(show(tan(&n("45"), Degrees)), "1");
        assert_eq!(show(tan(&n("90"), Degrees)), "Invalid input");
        assert_eq!(show(tan(&n("270"), Degrees)), "Invalid input");
        assert_eq!(show(tan(&n("50"), Gradians)), "1");
        assert_eq!(
            show(tan(&n("1"), Radians)),
            "1.5574077246549022305069748074584"
        );
        // Large angles are reduced exactly.
        assert_eq!(
            show(sin(&n("1e30"), Degrees)),
            "-0.98480775301220805936674302458952"
        );
        assert_eq!(
            show(sin(&n("1e22"), Radians)),
            "-0.85220084976718880177270589375303"
        );
        assert!(too_big_for_trig(&n("1e100")));
        assert!(!too_big_for_trig(&n("-1e100")));
    }

    #[test]
    fn inverse_trigonometry() {
        use Angle::*;
        assert_eq!(show(asin(&n("0.5"), Degrees)), "30");
        assert_eq!(show(asin(&n("1"), Degrees)), "90");
        assert_eq!(show(asin(&n("-1"), Gradians)), "-100");
        assert_eq!(
            show(asin(&n("1"), Radians)),
            "1.5707963267948966192313216916398"
        );
        assert_eq!(show(asin(&n("1.5"), Degrees)), "Invalid input");
        assert_eq!(show(acos(&n("0.5"), Degrees)), "60");
        assert_eq!(show(acos(&n("-1"), Degrees)), "180");
        assert_eq!(
            show(acos(&n("0"), Radians)),
            "1.5707963267948966192313216916398"
        );
        assert_eq!(show(atan(&n("1"), Degrees)), "45");
        assert_eq!(show(atan(&n("-1"), Gradians)), "-50");
        assert_eq!(show(atan(&n("1e40"), Degrees)), "90");
        assert_eq!(
            show(atan(&n("0.5"), Radians)),
            "0.46364760900080611621425623146121"
        );
    }

    #[test]
    fn hyperbolic_functions() {
        assert_eq!(show(sinh(&n("1"))), "1.1752011936438014568823818505956");
        assert_eq!(show(sinh(&n("0.5"))), "0.52109530549374736162242562641149");
        assert_eq!(show(sinh(&n("0"))), "0");
        assert_eq!(show(cosh(&n("1"))), "1.5430806348152437784779056207571");
        assert_eq!(show(cosh(&n("-2"))), "3.7621956910836314595622134777737");
        assert_eq!(show(tanh(&n("1"))), "0.76159415595576488811945828260479");
        assert_eq!(show(tanh(&n("50000"))), "1");
        assert_eq!(show(sinh(&n("-20000"))), "Invalid input");
        assert_eq!(show(cosh(&n("200000"))), "Invalid input");
        assert_eq!(show(asinh(&n("1"))), "0.88137358701954302523260932497979");
        assert_eq!(show(asinh(&n("-1"))), "-0.88137358701954302523260932497979");
        assert_eq!(show(acosh(&n("2"))), "1.316957896924816708625046347308");
        assert_eq!(show(acosh(&n("0.5"))), "Invalid input");
        assert_eq!(show(atanh(&n("0.5"))), "0.54930614433405484569762261846126");
        assert_eq!(show(atanh(&n("1"))), "Cannot divide by zero");
        assert_eq!(show(atanh(&n("-1"))), "Invalid input");
        assert_eq!(show(atanh(&n("2"))), "Invalid input");
        // Tiny arguments keep their precision.
        assert_eq!(show(sinh(&n("1e-40"))), "1.e-40");
        assert_eq!(show(asinh(&n("1e-40"))), "1.e-40");
    }

    #[test]
    fn factorials() {
        assert_eq!(show(factorial(&n("5"))), "120");
        assert_eq!(show(factorial(&n("0"))), "1");
        assert_eq!(
            show(factorial(&n("0.5"))),
            "0.88622692545275801364908374167057"
        );
        assert_eq!(
            show(factorial(&n("-0.5"))),
            "1.7724538509055160272981674833411"
        );
        assert_eq!(
            show(factorial(&n("-1.5"))),
            "-3.5449077018110320545963349666823"
        );
        assert_eq!(
            show(factorial(&n("2.5"))),
            "3.3233509704478425511840640312646"
        );
        assert_eq!(show(factorial(&n("-3"))), "Invalid input");
        assert_eq!(show(factorial(&n("3250"))), "Overflow");
        assert_eq!(show(factorial(&n("3249"))), "Overflow");
        assert_eq!(show(factorial(&n("-1001"))), "Overflow");
        assert_eq!(show(factorial(&n("30"))), "2.6525285981219105863630848e+32");
    }

    #[test]
    fn exact_functions() {
        assert_eq!(show(Ok(dms(&n("30.5"), false))), "30.3");
        assert_eq!(show(Ok(dms(&n("30.3"), true))), "30.5");
        assert_eq!(show(Ok(dms(&n("1.2575"), false))), "1.1527");
        assert_eq!(show(Ok(modulo(&n("10"), &n("3")))), "1");
        assert_eq!(show(Ok(modulo(&n("-10"), &n("3")))), "2");
        assert_eq!(show(Ok(modulo(&n("10"), &n("-3")))), "-2");
        assert_eq!(show(Ok(modulo(&n("5.5"), &n("2")))), "1.5");
        assert_eq!(show(Ok(modulo(&n("7"), &n("0")))), "7");
        assert_eq!(show(Ok(floor(&n("-2.5")))), "-3");
        assert_eq!(show(Ok(ceil(&n("-2.5")))), "-2");
        assert_eq!(show(Ok(ceil(&n("2.1")))), "3");
    }
}
