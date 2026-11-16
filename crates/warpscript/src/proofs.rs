// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Kani proof harnesses (compiled only under `cargo kani`).
//!
//! The harnesses are bounded and target the pure helpers of the runtime, which is where the
//! Java-compatibility corner cases live. Allocation-heavy code (strings, vectors) is covered by
//! the unit tests instead: symbolic execution of the allocator does not scale.

use crate::encoding::{OPB64_ALPHABET, opb64_value};
use crate::format::{is_double, is_long};
use crate::jmath::{abs_long, floor_div, floor_mod, max, min, signum};
use crate::lcg48::Lcg48;
use crate::value::double_to_long;

/// Any 3 ASCII characters: a string accepted by `is_long` is never accepted by `is_double`
/// (number literals are unambiguous), a long ends with a digit and a double ends with a digit,
/// a dot, an exponent marker or sign (`1.` and `1e` are accepted, like in Warp 10, and
/// `Double.valueOf` has the last word) or is `NaN`.
#[kani::proof]
#[kani::unwind(5)]
fn long_and_double_detection_are_disjoint() {
    let bytes: [u8; 3] = kani::any();
    kani::assume(bytes[0].is_ascii() && bytes[1].is_ascii() && bytes[2].is_ascii());
    let Ok(text) = core::str::from_utf8(&bytes) else {
        return;
    };
    let long = is_long(text);
    let double = is_double(text);
    assert!(!(long && double));
    if long {
        assert!(bytes[2].is_ascii_digit());
    }
    if double {
        assert!(matches!(
            bytes[2],
            b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-' | b'N'
        ));
    }
}

/// `opb64_value` is the inverse of the OPB64 alphabet: decoding a digit of the alphabet yields
/// its index, and the indices are the only accepted values.
#[kani::proof]
fn opb64_value_inverts_the_alphabet() {
    let i: u8 = kani::any();
    kani::assume(i < 64);
    assert!(opb64_value(OPB64_ALPHABET[i as usize]) == Some(i));
}

/// The OPB64 alphabet is strictly increasing, so the encoding preserves the byte order of the
/// payloads it is applied to.
#[kani::proof]
fn opb64_alphabet_is_sorted() {
    let i: u8 = kani::any();
    kani::assume(i < 63);
    assert!(OPB64_ALPHABET[i as usize] < OPB64_ALPHABET[i as usize + 1]);
}

/// `(long) d` saturates and maps NaN to zero, like the JVM.
#[kani::proof]
fn double_to_long_matches_the_jvm() {
    let d: f64 = kani::any();
    let l = double_to_long(d);
    if d.is_nan() {
        assert!(l == 0);
    } else if d >= 9_223_372_036_854_775_807.0 {
        assert!(l == i64::MAX);
    } else if d <= -9_223_372_036_854_775_808.0 {
        assert!(l == i64::MIN);
    }
}

/// `Math.floorDiv` / `Math.floorMod` for any dividend and the divisor `$y`: no panic, the
/// remainder has the sign of the divisor (or is zero) and is smaller than it in magnitude, and
/// the pair satisfies `x == q * y + m`. A symbolic 64-bit divisor is out of reach of the SAT
/// solver, hence one harness per (constant) divisor, covering both signs.
macro_rules! floor_division_identity {
    ($($name:ident => $y:literal;)*) => {
        $(
            #[kani::proof]
            fn $name() {
                let x: i64 = kani::any();
                let (q, m) = (succeeds(floor_div(x, $y)), succeeds(floor_mod(x, $y)));
                assert!(q.is_some() && m.is_some(), "a non-zero divisor never fails");
                let (Some(q), Some(m)) = (q, m) else {
                    return;
                };
                assert!(m == 0 || (m < 0) == ($y < 0));
                assert!(m.unsigned_abs() < i64::unsigned_abs($y));
                assert!(q.wrapping_mul($y).wrapping_add(m) == x);
            }
        )*
    };
}

floor_division_identity! {
    floor_division_identity_by_7 => 7;
    floor_division_identity_by_minus_7 => -7;
}

/// `Math.floorDiv(Long.MIN_VALUE, -1)` overflows to `Long.MIN_VALUE` without panicking, like
/// the JVM, and the remainder is zero: `-1` divides everything.
#[kani::proof]
fn floor_division_by_minus_one_wraps() {
    let x: i64 = kani::any();
    let (q, m) = (succeeds(floor_div(x, -1)), succeeds(floor_mod(x, -1)));
    assert!(q == Some(x.wrapping_neg()));
    assert!(m == Some(0));
}

/// The value of a result, or `None` for an error. The error is not dropped: CBMC cannot bound
/// the drop glue of `Error` (its cause chain and the I/O failures it can hold), and the
/// destructor is not what these harnesses are about. The harnesses assert that the value is
/// present, so an error is a failed proof rather than a panic.
fn succeeds<T>(result: crate::error::Result<T>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            core::mem::forget(error);
            None
        }
    }
}

/// A division by zero is reported as an error, never as a panic.
#[kani::proof]
fn floor_division_by_zero_is_an_error() {
    let x: i64 = kani::any();
    let (div, rem) = (floor_div(x, 0), floor_mod(x, 0));
    assert!(div.is_err());
    assert!(rem.is_err());
    // The errors are not dropped, see `succeeds`.
    core::mem::forget((div, rem));
}

/// `Math.abs(long)` is non-negative except for `Long.MIN_VALUE`, which is its own absolute value.
#[kani::proof]
fn abs_long_matches_java() {
    let l: i64 = kani::any();
    let a = abs_long(l);
    if l == i64::MIN {
        assert!(a == i64::MIN);
    } else {
        assert!(a >= 0 && (a == l || a == -l));
    }
}

/// `Math.max` / `Math.min`: a NaN operand is absorbing, otherwise the result is one of the two
/// operands and compares as expected.
#[kani::proof]
fn max_and_min_match_java() {
    let a: f64 = kani::any();
    let b: f64 = kani::any();
    let hi = max(a, b);
    let lo = min(a, b);
    if a.is_nan() || b.is_nan() {
        assert!(hi.is_nan() && lo.is_nan());
    } else {
        assert!(hi >= a && hi >= b && (hi == a || hi == b));
        assert!(lo <= a && lo <= b && (lo == a || lo == b));
    }
}

/// `Math.signum` is `-1.0`, `1.0`, or the argument itself for zeros and NaN.
#[kani::proof]
fn signum_matches_java() {
    let d: f64 = kani::any();
    let s = signum(d);
    if d.is_nan() {
        assert!(s.is_nan());
    } else if d == 0.0 {
        assert!(s.to_bits() == d.to_bits());
    } else {
        assert!(s == if d > 0.0 { 1.0 } else { -1.0 });
    }
}

/// `Random.next(bits)` keeps the state within 48 bits and returns at most `bits` bits; the
/// doubles it produces are in `[0, 1)`.
#[kani::proof]
fn java_random_stays_in_range() {
    let seed: i64 = kani::any();
    let bits: u32 = kani::any();
    kani::assume((1..=32).contains(&bits));
    let mut r = Lcg48::new(seed);
    let v = r.next(bits);
    assert!(r.state() >= 0 && r.state() < (1 << 48));
    assert!(bits == 32 || (v >= 0 && (v as u32) < (1u32 << bits)));
    let d = r.next_double();
    assert!((0.0..1.0).contains(&d));
}
