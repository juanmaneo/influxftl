// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
//
// `ieee_remainder` follows the algorithm of fdlibm's e_remainder.c:
//   Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
//   Developed at SunSoft, a Sun Microsystems, Inc. business.
//   Permission to use, copy, modify, and distribute this software is freely granted,
//   provided that this notice is preserved.

//! The `java.lang.Math` semantics that differ from, or are missing in, Rust's `f64` / `i64`.
//!
//! WarpScript programs observe the exact Java results of these functions (rounding of ties,
//! the sign of zero, `NaN` propagation, overflow behaviour), so each one implements the
//! contract documented for the Java method rather than calling the nearest Rust method.

use crate::error::fault::ArithmeticError;
use crate::error::{Error, Result};

const EXP_BIT_MASK: u64 = 0x7FF0_0000_0000_0000;
const SIGNIFICAND_WIDTH: i32 = 53;
const EXP_BIAS: i32 = 1023;
/// `Double.MAX_EXPONENT`.
pub const MAX_EXPONENT: i32 = 1023;
/// `Double.MIN_EXPONENT`.
pub const MIN_EXPONENT: i32 = -1022;
/// `Double.MIN_VALUE`, the smallest positive subnormal.
pub const MIN_VALUE: f64 = f64::from_bits(1);

/// `Math.round(double)`: the closest `long` to `a`, ties rounding towards positive infinity;
/// `NaN` gives 0 and out of range values saturate.
pub fn round(a: f64) -> i64 {
    // `floor(a + 0.5)` without the rounding error of the addition: `a - floor(a)` is exact
    // by the Sterbenz lemma whenever `|a| >= 0.5` (and trivially when the floor is zero),
    // and for `-0.5 < a < 0` it rounds monotonically to a value that is still at least one
    // half, so the comparison always decides like the exact computation. The `as` conversion
    // saturates and maps `NaN` (for which the comparison is false) to 0, like the JVM's `d2l`.
    let floor = a.floor();
    if a - floor >= 0.5 {
        (floor + 1.0) as i64
    } else {
        floor as i64
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

/// The `f64` nearest to `m × 2^e` (ties to even), `sticky` telling that `m` is the truncation
/// of a value with more non-zero bits below: the correct rounding Java applies when it
/// converts an exact binary value (`Double.parseDouble` of a hexadecimal literal,
/// `BigDecimal.doubleValue()`). Zero gives `0.0`, overflows give infinity.
pub fn binary_to_f64(m: u64, e: i64, sticky: bool) -> f64 {
    if m == 0 {
        return 0.0;
    }
    let leading = m.leading_zeros();
    let m = m << leading;
    // The value is `m * 2^e` with the top bit of `m` set: its unbiased exponent is `top`.
    let top = 63 + e - i64::from(leading);
    if top > 1023 {
        return f64::INFINITY;
    }
    // The bits of `m` below the 53 kept (fewer for a subnormal) are rounded off.
    let shift = 11 + (-1022 - top).max(0);
    if shift > 64 {
        // Below half the least subnormal.
        return 0.0;
    }
    let shift = shift as u32;
    let wide = u128::from(m);
    let mut kept = (wide >> shift) as u64;
    let rest = wide & ((1u128 << shift) - 1);
    let half = 1u128 << (shift - 1);
    if rest > half || rest == half && (sticky || kept & 1 == 1) {
        kept += 1;
    }
    // A normal value adds its biased exponent minus one to the mantissa and its implicit
    // bit, so that a rounding carry moves to the next exponent (up to infinity).
    let bits = if top >= -1022 {
        (((top + 1022) as u64) << 52) + kept
    } else {
        kept
    };
    f64::from_bits(bits)
}

/// `Math.getExponent(double)`: the unbiased exponent; `MAX_EXPONENT + 1` for `NaN` and the
/// infinities, `MIN_EXPONENT - 1` for zero and the subnormals.
pub fn get_exponent(d: f64) -> i32 {
    (((d.to_bits() & EXP_BIT_MASK) >> (SIGNIFICAND_WIDTH - 1)) as i32) - EXP_BIAS
}

/// The normal double `2^n`, for `MIN_EXPONENT <= n <= MAX_EXPONENT`.
fn power_of_two(n: i32) -> f64 {
    f64::from_bits(((n + EXP_BIAS) as u64) << (SIGNIFICAND_WIDTH - 1))
}

/// `Math.ulp(double)`: the positive distance between `d` and the double of next larger
/// magnitude; `NaN` for `NaN`, infinity for an infinity, `Double.MIN_VALUE` for zero.
pub fn ulp(d: f64) -> f64 {
    let magnitude = d.abs();
    if !magnitude.is_finite() {
        return magnitude;
    }
    // Adjacent doubles subtract exactly. Above `Double.MAX_VALUE` there is no next double:
    // the ulp is then the distance to the previous one, which is the same.
    let next = magnitude.next_up();
    if next.is_infinite() {
        magnitude - magnitude.next_down()
    } else {
        next - magnitude
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

/// `Math.scalb(double, int)`: `d * 2^scale_factor` rounded as a single correctly rounded
/// multiplication (overflow gives an infinity, a subnormal result is rounded once).
pub fn scalb(d: f64, scale_factor: i32) -> f64 {
    // Multiplying by a power of two is exact as long as the result is normal, so the scaling
    // is done in steps that keep every intermediate result normal (or already zero or
    // infinite): only the final multiplication can round or overflow. Two steps are enough,
    // because `MIN_VALUE * 2^2046` and `MAX_VALUE * 2^-1938` are normal: whatever exponent
    // remains after them can only drive the result to infinity or to zero.
    const UP: f64 = f64::from_bits(((2 * EXP_BIAS) as u64) << (SIGNIFICAND_WIDTH - 1));
    const UP_EXPONENT: i32 = MAX_EXPONENT;
    // 2^-969: the largest magnitude times it stays normal, and so does a normal number down
    // to 2^-53, below which the result is zero anyway.
    const DOWN_EXPONENT: i32 = MIN_EXPONENT + SIGNIFICAND_WIDTH;
    const DOWN: f64 =
        f64::from_bits(((DOWN_EXPONENT + EXP_BIAS) as u64) << (SIGNIFICAND_WIDTH - 1));
    let mut d = d;
    let mut n = scale_factor;
    if n > MAX_EXPONENT {
        d *= UP;
        n -= UP_EXPONENT;
        if n > MAX_EXPONENT {
            d *= UP;
            n = (n - UP_EXPONENT).min(MAX_EXPONENT);
        }
    } else if n < MIN_EXPONENT {
        d *= DOWN;
        n -= DOWN_EXPONENT;
        if n < MIN_EXPONENT {
            d *= DOWN;
            n = (n - DOWN_EXPONENT).max(MIN_EXPONENT);
        }
    }
    d * power_of_two(n)
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
        return Err(Error::division_by_zero());
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
        return Err(Error::division_by_zero());
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
    x.checked_add(y)
        .ok_or_else(|| ArithmeticError::LongOverflow.into())
}

/// `Math.subtractExact(long, long)`.
pub fn subtract_exact(x: i64, y: i64) -> Result<i64> {
    x.checked_sub(y)
        .ok_or_else(|| ArithmeticError::LongOverflow.into())
}

/// `Math.multiplyExact(long, long)`.
pub fn multiply_exact(x: i64, y: i64) -> Result<i64> {
    x.checked_mul(y)
        .ok_or_else(|| ArithmeticError::LongOverflow.into())
}

/// `Math.incrementExact(long)`.
pub fn increment_exact(x: i64) -> Result<i64> {
    x.checked_add(1)
        .ok_or_else(|| ArithmeticError::LongOverflow.into())
}

/// `Math.decrementExact(long)`.
pub fn decrement_exact(x: i64) -> Result<i64> {
    x.checked_sub(1)
        .ok_or_else(|| ArithmeticError::LongOverflow.into())
}

/// `Math.negateExact(long)`.
pub fn negate_exact(x: i64) -> Result<i64> {
    x.checked_neg()
        .ok_or_else(|| ArithmeticError::LongOverflow.into())
}

/// `Math.toIntExact(long)`: the value when it fits in 32 bits.
pub fn to_int_exact(x: i64) -> Result<i64> {
    i32::try_from(x)
        .map(i64::from)
        .map_err(|_| ArithmeticError::IntegerOverflow.into())
}
