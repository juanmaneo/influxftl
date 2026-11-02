// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp
)]

//! Reference values below were produced with OpenJDK 27 (`java.lang.Math`).

use crate::jmath::*;

/// Compare doubles bit for bit (so that `0.0` and `-0.0` differ and `NaN` equals `NaN`).
#[track_caller]
fn assert_bits(actual: f64, expected: f64) {
    assert!(
        actual.to_bits() == expected.to_bits(),
        "expected {expected:?} ({:#x}), got {actual:?} ({:#x})",
        expected.to_bits(),
        actual.to_bits()
    );
}

#[test]
fn round_matches_java() {
    assert_eq!(round(0.5), 1);
    assert_eq!(round(-0.5), 0);
    assert_eq!(round(1.5), 2);
    assert_eq!(round(2.5), 3);
    assert_eq!(round(-2.5), -2);
    assert_eq!(round(-1.499_999_999_999_999_8), -1);
    // The classic `floor(x + 0.5)` bug: Java rounds this to 0, not 1.
    assert_eq!(round(0.499_999_999_999_999_94), 0);
    assert_eq!(round(4_503_599_627_370_495.5), 4_503_599_627_370_496);
    assert_eq!(round(-4_503_599_627_370_495.5), -4_503_599_627_370_495);
    assert_eq!(round(4_503_599_627_370_497.0), 4_503_599_627_370_497);
    assert_eq!(round(1e300), i64::MAX);
    assert_eq!(round(-1e300), i64::MIN);
    assert_eq!(round(f64::INFINITY), i64::MAX);
    assert_eq!(round(f64::NEG_INFINITY), i64::MIN);
    assert_eq!(round(f64::NAN), 0);
    assert_eq!(round(1e-320), 0);
    assert_eq!(round(-0.0), 0);
    // `-0.5 < a < 0` is the only range where `a - floor(a)` rounds: still 0.
    assert_eq!(round(-0.499_999_999_999_999_94), 0);
    assert_eq!(round(-0.1), 0);
    assert_eq!(round(-f64::MIN_POSITIVE), 0);
    assert_eq!(round(-1.0), -1);
    assert_eq!(round(-0.999_999_999_999_999_9), -1);
    assert_eq!(round(-9.007_199_254_740_993e15), -9_007_199_254_740_992);
    assert_eq!(round(9.223_372_036_854_776e18), i64::MAX);
    assert_eq!(round(-9.223_372_036_854_776e18), i64::MIN);
}

#[test]
fn rint_rounds_ties_to_even() {
    assert_bits(rint(2.5), 2.0);
    assert_bits(rint(3.5), 4.0);
    assert_bits(rint(-2.5), -2.0);
    assert_bits(rint(-0.4), -0.0);
}

#[test]
fn signum_keeps_zeros_and_nan() {
    assert_bits(signum(5.0), 1.0);
    assert_bits(signum(-3.0), -1.0);
    assert_bits(signum(0.0), 0.0);
    assert_bits(signum(-0.0), -0.0);
    assert_bits(signum(f64::NAN), f64::NAN);
    assert_bits(signum(f64::NEG_INFINITY), -1.0);
}

#[test]
fn abs_long_wraps_at_min() {
    assert_eq!(abs_long(-5), 5);
    assert_eq!(abs_long(5), 5);
    assert_eq!(abs_long(i64::MIN), i64::MIN);
}

#[test]
fn max_and_min_follow_java() {
    assert_bits(max(1.0, 2.0), 2.0);
    assert_bits(max(2.0, 1.0), 2.0);
    assert_bits(max(f64::NAN, 1.0), f64::NAN);
    assert_bits(max(1.0, f64::NAN), f64::NAN);
    assert_bits(max(-0.0, 0.0), 0.0);
    assert_bits(max(0.0, -0.0), 0.0);
    assert_bits(min(1.0, 2.0), 1.0);
    assert_bits(min(2.0, 1.0), 1.0);
    assert_bits(min(f64::NAN, 1.0), f64::NAN);
    assert_bits(min(1.0, f64::NAN), f64::NAN);
    assert_bits(min(0.0, -0.0), -0.0);
    assert_bits(min(-0.0, 0.0), -0.0);
}

#[test]
fn pow_special_cases() {
    assert_bits(pow(2.0, 10.0), 1024.0);
    assert_bits(pow(f64::NAN, 0.0), 1.0);
    assert_bits(pow(1.0, f64::NAN), f64::NAN);
    assert_bits(pow(1.0, f64::INFINITY), f64::NAN);
    assert_bits(pow(-1.0, f64::NEG_INFINITY), f64::NAN);
    assert_bits(pow(2.0, f64::INFINITY), f64::INFINITY);
}

#[test]
fn get_exponent_matches_java() {
    assert_eq!(get_exponent(1.0), 0);
    assert_eq!(get_exponent(8.0), 3);
    assert_eq!(get_exponent(0.1), -4);
    assert_eq!(get_exponent(0.0), -1023);
    assert_eq!(get_exponent(1e-310), -1023);
    assert_eq!(get_exponent(f64::NAN), 1024);
    assert_eq!(get_exponent(f64::INFINITY), 1024);
}

#[test]
fn ulp_matches_java() {
    assert_bits(ulp(1.0), 2.220_446_049_250_313e-16);
    assert_bits(ulp(-3.0), 4.440_892_098_500_626e-16);
    assert_bits(ulp(1e-300), 1.657_809_2e-316);
    assert_bits(ulp(f64::MAX), 1.995_840_309_534_72e292);
    assert_bits(ulp(f64::MIN_POSITIVE), MIN_VALUE);
    assert_bits(ulp(1e-310), MIN_VALUE);
    assert_bits(ulp(0.0), MIN_VALUE);
    assert_bits(ulp(f64::NAN), f64::NAN);
    assert_bits(ulp(f64::NEG_INFINITY), f64::INFINITY);
}

#[test]
fn next_after_matches_java() {
    assert_bits(next_after(1.0, 2.0), 1.000_000_000_000_000_2);
    assert_bits(next_after(1.0, 0.0), 0.999_999_999_999_999_9);
    assert_bits(next_after(-0.0, 0.0), 0.0);
    assert_bits(next_after(0.0, -0.0), -0.0);
    assert_bits(next_after(0.0, -1.0), -MIN_VALUE);
    assert_bits(next_after(f64::NAN, 1.0), f64::NAN);
    assert_bits(next_after(1.0, f64::NAN), f64::NAN);
}

#[test]
fn scalb_matches_java() {
    assert_bits(scalb(1.0, 3), 8.0);
    assert_bits(scalb(0.1, 3), 0.8);
    assert_bits(scalb(1.0, -600), 2.409_919_865_102_884e-181);
    assert_bits(scalb(1.0, 1023), 8.988_465_674_311_58e307);
    assert_bits(scalb(1.5, 1024), f64::INFINITY);
    assert_bits(scalb(1.0, 5000), f64::INFINITY);
    assert_bits(scalb(1.0, -5000), 0.0);
    assert_bits(scalb(1.0, -1074), MIN_VALUE);
    assert_bits(scalb(1.0, -1075), 0.0);
    // Single rounding: 3 * 2^-1075 = 1.5 * MIN_VALUE rounds to even (2 * MIN_VALUE)…
    assert_bits(scalb(3.0, -1075), 9.9e-324);
    // …and 0.75 * MIN_VALUE rounds up to MIN_VALUE.
    assert_bits(scalb(3.0, -1076), MIN_VALUE);
    assert_bits(scalb(f64::MAX, -2098), MIN_VALUE);
    assert_bits(scalb(MIN_VALUE, 2000), 5.672_519_334_708_34e278);
    assert_bits(scalb(MIN_VALUE, 2500), f64::INFINITY);
    assert_bits(scalb(-0.0, 2500), -0.0);
    assert_bits(scalb(-1.0, -2500), -0.0);
    assert_bits(scalb(f64::MAX, -2099), 0.0);
    assert_bits(scalb(f64::NAN, 10), f64::NAN);
    assert_bits(scalb(f64::NEG_INFINITY, -10), f64::NEG_INFINITY);
    assert_bits(scalb(1.0, i32::MAX), f64::INFINITY);
    assert_bits(scalb(1.0, i32::MIN), 0.0);
    assert_bits(scalb(0.75, -1023), f64::from_bits(0x0006_0000_0000_0000));
    assert_bits(scalb(1.0, -1023), f64::from_bits(0x0008_0000_0000_0000));
    assert_bits(scalb(1.5, 1023), f64::from_bits(0x7FE8_0000_0000_0000));
}

#[test]
fn ieee_remainder_matches_java() {
    assert_bits(ieee_remainder(5.0, 3.0), -1.0);
    assert_bits(ieee_remainder(7.0, 2.0), -1.0);
    assert_bits(ieee_remainder(5.0, 2.0), 1.0);
    assert_bits(ieee_remainder(-5.0, 3.0), 1.0);
    assert_bits(ieee_remainder(-2.5, 1.0), -0.5);
    assert_bits(ieee_remainder(0.3, 0.1), -2.775_557_561_562_891_4e-17);
    // A zero result keeps the sign of the dividend.
    assert_bits(ieee_remainder(6.0, 3.0), 0.0);
    assert_bits(ieee_remainder(-6.0, 3.0), -0.0);
    assert_bits(ieee_remainder(-3.0, 3.0), -0.0);
    // Huge divisors skip the fmod reduction, tiny ones avoid halving.
    assert_bits(ieee_remainder(1.0, 1e308), 1.0);
    assert_bits(ieee_remainder(-1e308, 3.0), 1.0);
    assert_bits(ieee_remainder(1e308, 1e-308), 3.498_445_546_245_627e-309);
    assert_bits(ieee_remainder(6.0 * MIN_VALUE, 4.0 * MIN_VALUE), -9.9e-324);
    assert_bits(ieee_remainder(5.0 * MIN_VALUE, 4.0 * MIN_VALUE), MIN_VALUE);
    assert_bits(ieee_remainder(1.0, f64::INFINITY), 1.0);
    assert_bits(ieee_remainder(1.0, 0.0), f64::NAN);
    assert_bits(ieee_remainder(f64::INFINITY, 1.0), f64::NAN);
    assert_bits(ieee_remainder(f64::NAN, 1.0), f64::NAN);
    assert_bits(ieee_remainder(1.0, f64::NAN), f64::NAN);
}

#[test]
fn floor_div_and_mod() {
    assert_eq!(floor_div(7, 2).unwrap(), 3);
    assert_eq!(floor_div(-7, 2).unwrap(), -4);
    assert_eq!(floor_div(7, -2).unwrap(), -4);
    assert_eq!(floor_div(-7, -2).unwrap(), 3);
    assert_eq!(floor_div(6, 3).unwrap(), 2);
    assert_eq!(floor_div(-6, 3).unwrap(), -2);
    assert_eq!(floor_div(i64::MIN, -1).unwrap(), i64::MIN);
    assert_eq!(floor_div(1, 0).unwrap_err().message(), "/ by zero");
    assert_eq!(floor_mod(7, 2).unwrap(), 1);
    assert_eq!(floor_mod(-7, 2).unwrap(), 1);
    assert_eq!(floor_mod(7, -2).unwrap(), -1);
    assert_eq!(floor_mod(-7, -2).unwrap(), -1);
    assert_eq!(floor_mod(6, 3).unwrap(), 0);
    assert_eq!(floor_mod(-6, 3).unwrap(), 0);
    assert_eq!(floor_mod(i64::MIN, -1).unwrap(), 0);
    assert_eq!(floor_mod(1, 0).unwrap_err().message(), "/ by zero");
}

#[test]
fn exact_arithmetic_detects_overflow() {
    assert_eq!(add_exact(1, 2).unwrap(), 3);
    assert_eq!(
        add_exact(i64::MAX, 1).unwrap_err().message(),
        "long overflow"
    );
    assert_eq!(subtract_exact(1, 2).unwrap(), -1);
    assert_eq!(
        subtract_exact(i64::MIN, 1).unwrap_err().message(),
        "long overflow"
    );
    assert_eq!(multiply_exact(3, 4).unwrap(), 12);
    assert_eq!(
        multiply_exact(i64::MAX, 2).unwrap_err().message(),
        "long overflow"
    );
    assert_eq!(increment_exact(1).unwrap(), 2);
    assert_eq!(
        increment_exact(i64::MAX).unwrap_err().message(),
        "long overflow"
    );
    assert_eq!(decrement_exact(1).unwrap(), 0);
    assert_eq!(
        decrement_exact(i64::MIN).unwrap_err().message(),
        "long overflow"
    );
    assert_eq!(negate_exact(1).unwrap(), -1);
    assert_eq!(
        negate_exact(i64::MIN).unwrap_err().message(),
        "long overflow"
    );
    assert_eq!(to_int_exact(-5).unwrap(), -5);
    assert_eq!(
        to_int_exact(i64::from(i32::MAX)).unwrap(),
        i64::from(i32::MAX)
    );
    assert_eq!(
        to_int_exact(i64::from(i32::MAX) + 1).unwrap_err().message(),
        "integer overflow"
    );
}
