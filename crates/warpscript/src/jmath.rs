//! The `java.lang.Math` semantics that differ from, or are missing in, Rust's `f64` / `i64`.
//!
//! WarpScript programs observe the exact Java results of these functions (rounding of ties,
//! the sign of zero, `NaN` propagation, overflow behaviour), so each one is a port of the JDK
//! algorithm rather than a call to the nearest Rust method.

use crate::error::{Error, Result};

const EXP_BIT_MASK: u64 = 0x7FF0_0000_0000_0000;
const SIGNIF_BIT_MASK: u64 = 0x000F_FFFF_FFFF_FFFF;
const SIGNIFICAND_WIDTH: i32 = 53;
const EXP_BIAS: i32 = 1023;
/// `Double.MAX_EXPONENT`.
pub const MAX_EXPONENT: i32 = 1023;
/// `Double.MIN_EXPONENT`.
pub const MIN_EXPONENT: i32 = -1022;
/// `Double.MIN_VALUE`, the smallest positive subnormal.
pub const MIN_VALUE: f64 = f64::from_bits(1);

/// Message of the `ArithmeticException` thrown by `Math.addExact` and friends.
pub const LONG_OVERFLOW: &str = "long overflow";
/// Message of the `ArithmeticException` thrown by `Math.toIntExact`.
pub const INTEGER_OVERFLOW: &str = "integer overflow";
/// Message of the `ArithmeticException` thrown by an integer division by zero.
pub const DIVISION_BY_ZERO: &str = "/ by zero";

/// `Math.round(double)`: the closest `long` to `a`, ties rounding towards positive infinity;
/// `NaN` gives 0 and out of range values saturate.
pub fn round(a: f64) -> i64 {
    let long_bits = a.to_bits() as i64;
    let biased_exp = (long_bits & EXP_BIT_MASK as i64) >> (SIGNIFICAND_WIDTH - 1);
    let shift = i64::from(SIGNIFICAND_WIDTH - 2 + EXP_BIAS) - biased_exp;
    if (shift & -64) == 0 {
        // A finite number such that 2^-64 <= ulp(a) < 1: round the significand by hand.
        let mut r = (long_bits & SIGNIF_BIT_MASK as i64) | (SIGNIF_BIT_MASK as i64 + 1);
        if long_bits < 0 {
            r = -r;
        }
        ((r >> shift) + 1) >> 1
    } else {
        // Either |a| < 2^-11 (rounds to zero), or ulp(a) >= 1 (a is an integer), or a is an
        // infinity or NaN: the JVM `d2l` conversion does the right thing.
        a as i64
    }
}

/// `Math.rint`: the closest integer-valued `double`, ties rounding to the even neighbour.
pub fn rint(a: f64) -> f64 {
    a.round_ties_even()
}

/// `Math.signum(double)`: `NaN` and the two zeros are returned as is.
pub fn signum(d: f64) -> f64 {
    if d == 0.0 || d.is_nan() {
        d
    } else {
        1.0_f64.copysign(d)
    }
}

/// `Math.abs(long)`: `Long.MIN_VALUE` is its own absolute value.
pub fn abs_long(l: i64) -> i64 {
    l.wrapping_abs()
}

/// `Math.max(double, double)`: `NaN` if either argument is `NaN`, `0.0` beats `-0.0`.
pub fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        return b;
    }
    if a >= b { a } else { b }
}

/// `Math.min(double, double)`: `NaN` if either argument is `NaN`, `-0.0` beats `0.0`.
pub fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        return b;
    }
    if a <= b { a } else { b }
}

/// `Math.pow`: like Rust's `powf` except that a `NaN` exponent always gives `NaN` and
/// `|x| == 1` raised to an infinite power is `NaN` (C99 returns `1.0` in both cases).
#[allow(clippy::float_cmp)]
pub fn pow(a: f64, b: f64) -> f64 {
    if b.is_nan() || (a.abs() == 1.0 && b.is_infinite()) {
        f64::NAN
    } else {
        a.powf(b)
    }
}

/// `Math.getExponent(double)`: the unbiased exponent; `MAX_EXPONENT + 1` for `NaN` and the
/// infinities, `MIN_EXPONENT - 1` for zero and the subnormals.
pub fn get_exponent(d: f64) -> i32 {
    (((d.to_bits() & EXP_BIT_MASK) >> (SIGNIFICAND_WIDTH - 1)) as i32) - EXP_BIAS
}

/// `2^n` as a double, for `MIN_EXPONENT <= n <= MAX_EXPONENT` (`Math.powerOfTwoD`).
fn power_of_two(n: i32) -> f64 {
    f64::from_bits(
        ((i64::from(n) + i64::from(EXP_BIAS)) << (SIGNIFICAND_WIDTH - 1)) as u64 & EXP_BIT_MASK,
    )
}

/// `Math.ulp(double)`: the distance to the next double of larger magnitude.
pub fn ulp(d: f64) -> f64 {
    let exp = get_exponent(d);
    if exp == MAX_EXPONENT + 1 {
        return d.abs();
    }
    if exp == MIN_EXPONENT - 1 {
        return MIN_VALUE;
    }
    let exp = exp - (SIGNIFICAND_WIDTH - 1);
    if exp >= MIN_EXPONENT {
        power_of_two(exp)
    } else {
        // The ulp is subnormal: build it directly from its bit pattern.
        f64::from_bits(1_u64 << (exp - (MIN_EXPONENT - (SIGNIFICAND_WIDTH - 1))))
    }
}

/// `Math.nextAfter(double, double)`: the neighbour of `start` in the direction of `direction`;
/// `direction` itself when they are equal (so `-0.0` towards `0.0` gives `0.0`).
pub fn next_after(start: f64, direction: f64) -> f64 {
    match start.partial_cmp(&direction) {
        Some(std::cmp::Ordering::Less) => start.next_up(),
        Some(std::cmp::Ordering::Greater) => start.next_down(),
        Some(std::cmp::Ordering::Equal) => direction,
        None => start + direction,
    }
}

/// `Math.scalb(double, int)`: `d * 2^scale_factor` computed exactly, with a single rounding
/// when the result is subnormal.
pub fn scalb(d: f64, scale_factor: i32) -> f64 {
    const MAX_SCALE: i32 = MAX_EXPONENT - MIN_EXPONENT + SIGNIFICAND_WIDTH + 1;
    let (scale_factor, scale_increment, exp_delta) = if scale_factor < 0 {
        (scale_factor.max(-MAX_SCALE), -512, power_of_two(-512))
    } else {
        (scale_factor.min(MAX_SCALE), 512, power_of_two(512))
    };
    // The JDK computes `scaleFactor % 512` (sign of the dividend) with a Hacker's Delight trick;
    // the remainder operator gives the same value.
    let exp_adjust = scale_factor % 512;
    let mut d = d * power_of_two(exp_adjust);
    let mut scale_factor = scale_factor - exp_adjust;
    while scale_factor != 0 {
        d *= exp_delta;
        scale_factor -= scale_increment;
    }
    d
}

/// `Math.IEEEremainder` (fdlibm `__ieee754_remainder`): `x - n * p` where `n` is the integer
/// closest to `x / p`, ties to even; the result keeps the sign of `x`, and a zero result keeps
/// it too.
#[allow(clippy::float_cmp)]
pub fn ieee_remainder(x: f64, p: f64) -> f64 {
    if p == 0.0 || x.is_infinite() || x.is_nan() || p.is_nan() {
        return f64::NAN;
    }
    if x.abs() == p.abs() {
        return 0.0 * x;
    }
    let negative = x.is_sign_negative();
    let mut x = x;
    // |p| < 2^1023 so that 2p is finite: reduce x into [0, 2p) first.
    if p.abs() < f64::from_bits(0x7FE0_0000_0000_0000) {
        x %= p + p;
    }
    let mut x = x.abs();
    let p = p.abs();
    // Below 2^-1021 the half of p would lose precision; compare doubled x instead.
    if p < f64::from_bits(0x0020_0000_0000_0000) {
        if x + x > p {
            x -= p;
            if x + x >= p {
                x -= p;
            }
        }
    } else {
        let p_half = 0.5 * p;
        if x > p_half {
            x -= p;
            if x >= p_half {
                x -= p;
            }
        }
    }
    let x = if x == 0.0 { 0.0 } else { x };
    if negative { -x } else { x }
}

/// `Math.floorDiv(long, long)`: division rounding towards negative infinity.
pub fn floor_div(x: i64, y: i64) -> Result<i64> {
    if y == 0 {
        return Err(Error::new(DIVISION_BY_ZERO));
    }
    let q = x.wrapping_div(y);
    // Different signs and a non-zero remainder: round down.
    if (x ^ y) < 0 && q.wrapping_mul(y) != x {
        Ok(q.wrapping_sub(1))
    } else {
        Ok(q)
    }
}

/// `Math.floorMod(long, long)`: the remainder of [`floor_div`], which has the sign of `y`.
pub fn floor_mod(x: i64, y: i64) -> Result<i64> {
    if y == 0 {
        return Err(Error::new(DIVISION_BY_ZERO));
    }
    let m = x.wrapping_rem(y);
    if (m ^ y) < 0 && m != 0 {
        Ok(m.wrapping_add(y))
    } else {
        Ok(m)
    }
}

/// `Math.addExact(long, long)`.
pub fn add_exact(x: i64, y: i64) -> Result<i64> {
    x.checked_add(y).ok_or_else(|| Error::new(LONG_OVERFLOW))
}

/// `Math.subtractExact(long, long)`.
pub fn subtract_exact(x: i64, y: i64) -> Result<i64> {
    x.checked_sub(y).ok_or_else(|| Error::new(LONG_OVERFLOW))
}

/// `Math.multiplyExact(long, long)`.
pub fn multiply_exact(x: i64, y: i64) -> Result<i64> {
    x.checked_mul(y).ok_or_else(|| Error::new(LONG_OVERFLOW))
}

/// `Math.incrementExact(long)`.
pub fn increment_exact(x: i64) -> Result<i64> {
    x.checked_add(1).ok_or_else(|| Error::new(LONG_OVERFLOW))
}

/// `Math.decrementExact(long)`.
pub fn decrement_exact(x: i64) -> Result<i64> {
    x.checked_sub(1).ok_or_else(|| Error::new(LONG_OVERFLOW))
}

/// `Math.negateExact(long)`.
pub fn negate_exact(x: i64) -> Result<i64> {
    x.checked_neg().ok_or_else(|| Error::new(LONG_OVERFLOW))
}

/// `Math.toIntExact(long)`: the value when it fits in 32 bits.
pub fn to_int_exact(x: i64) -> Result<i64> {
    i32::try_from(x)
        .map(i64::from)
        .map_err(|_| Error::new(INTEGER_OVERFLOW))
}
