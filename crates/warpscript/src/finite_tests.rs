// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

use super::finite::{NotFinite, ensure_finite};

#[test]
fn finite_numbers_pass() {
    for value in [
        0.0,
        -0.0,
        1.5,
        -1.5e300,
        f64::MAX,
        f64::MIN,
        f64::MIN_POSITIVE,
    ] {
        assert_eq!(ensure_finite(value), Ok(value));
    }
}

#[test]
fn nan_and_infinities_are_typed_failures() {
    assert_eq!(ensure_finite(f64::NAN), Err(NotFinite::Nan));
    assert_eq!(
        ensure_finite(f64::INFINITY),
        Err(NotFinite::PositiveInfinity)
    );
    assert_eq!(
        ensure_finite(f64::NEG_INFINITY),
        Err(NotFinite::NegativeInfinity)
    );
}

#[test]
fn failures_display_the_value() {
    assert_eq!(NotFinite::Nan.to_string(), "not a finite number: NaN");
    assert_eq!(
        NotFinite::PositiveInfinity.to_string(),
        "not a finite number: +Infinity"
    );
    assert_eq!(
        NotFinite::NegativeInfinity.to_string(),
        "not a finite number: -Infinity"
    );
}
