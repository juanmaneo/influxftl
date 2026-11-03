// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp
)]

//! Tests of the Commons Math interpolators (expected values computed with Commons Math 3.6.1).

use crate::interpolation::{
    akima, check_order, divided_difference, linear, loess, loess_smooth, message_number, spline,
};

const X: [f64; 6] = [10.0, 20.0, 40.0, 70.0, 75.0, 90.0];
const Y: [f64; 6] = [1.0, 2.0, 4.0, 1.0, 2.0, 3.0];

fn message<T: std::fmt::Debug>(result: crate::error::Result<T>) -> String {
    result.unwrap_err().to_string()
}

#[test]
fn messages() {
    assert_eq!(message_number(f64::NAN), "NaN");
    assert_eq!(message_number(f64::INFINITY), "∞");
    assert_eq!(message_number(f64::NEG_INFINITY), "-∞");
    assert_eq!(message_number(1_700_000.0), "1,700,000");
    assert_eq!(message_number(-123_456.0), "-123,456");
    assert_eq!(message_number(12.0), "12");
    assert_eq!(
        message(check_order(&[0.0, 1.0, 1700.0, 1700.0])),
        "points 2 and 3 are not strictly increasing (1,700 >= 1,700)"
    );
    assert!(check_order(&[0.0, 1.0]).is_ok());
}

#[test]
fn too_few_points() {
    assert_eq!(message(linear(&[1.0], &[1.0])), "number of points (1)");
    assert_eq!(
        message(spline(&[1.0, 2.0], &[1.0, 2.0])),
        "number of points (2)"
    );
    assert_eq!(message(akima(&X[..4], &Y[..4])), "number of points (4)");
    assert_eq!(
        message(divided_difference(&[1.0], &[1.0])),
        "2 points are required, got only 1"
    );
    assert_eq!(message(loess(&X, &Y, 0.2, 0, 1e-12)), "bandwidth (1)");
}

#[test]
fn functions() {
    let f = linear(&X, &Y).unwrap();
    assert!(f.is_valid_point(10.0) && f.is_valid_point(90.0) && !f.is_valid_point(9.0));
    assert!(!f.is_valid_point(91.0));
    assert_eq!(f.value(15.0), 1.5);
    assert_eq!(f.value(40.0), 4.0);
    assert_eq!(f.value(90.0), 3.0);
    // The values of Warp 10's `filler.spline`.
    let s = spline(&X, &Y).unwrap();
    assert_eq!(s.value(30.0), 3.419_282_511_210_762_5);
    assert_eq!(s.value(72.0), 1.373_470_852_017_937_3);
    // Akima with equal slopes on both sides of a point (zero weights).
    let a = akima(
        &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
    )
    .unwrap();
    assert_eq!(a.value(2.5), 2.5);
    let n = divided_difference(&[0.0, 1.0, 2.0], &[1.0, 2.0, 5.0]).unwrap();
    assert_eq!(n.value(3.0), 10.0);
}

#[test]
fn loess_smoothing() {
    // Non finite abscissas and ordinates, unordered abscissas.
    assert_eq!(
        message(loess_smooth(
            &[0.0, f64::INFINITY],
            &[0.0, 1.0],
            1.0,
            0,
            1e-12
        )),
        "∞ is not a finite number"
    );
    assert_eq!(
        message(loess_smooth(&[0.0, 1.0], &[f64::NAN, 1.0], 1.0, 0, 1e-12)),
        "NaN is not a finite number"
    );
    assert_eq!(
        message(loess_smooth(&[1.0, 0.0], &[0.0, 1.0], 1.0, 0, 1e-12)),
        "points 0 and 1 are not strictly increasing (1 >= 0)"
    );
    // Up to two points are returned as they are.
    assert_eq!(
        loess_smooth(&[0.0, 1.0], &[3.0, 5.0], 1.0, 0, 1e-12).unwrap(),
        vec![3.0, 5.0]
    );
    // Points on a line: no residual, the robustness iterations stop at once.
    let line = loess_smooth(&X, &[2.0, 4.0, 8.0, 14.0, 15.0, 18.0], 1.0, 3, 1e-12).unwrap();
    for (smoothed, y) in line.iter().zip([2.0, 4.0, 8.0, 14.0, 15.0, 18.0]) {
        assert!((smoothed - y).abs() < 1e-12, "{line:?}");
    }
    // An outlier gets a zero robustness weight.
    let robust = loess_smooth(
        &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        &[0.0, 1.0, 2.0, 100.0, 4.0, 5.0, 6.0],
        1.0,
        2,
        1e-12,
    )
    .unwrap();
    assert!((robust[3] - 3.0).abs() < 1.0, "{robust:?}");
    // Below the accuracy, the local regressions are flat: weighted means.
    let flat = loess_smooth(&X, &Y, 1.0, 0, 1e9).unwrap();
    assert!(flat.iter().all(|v| (1.0..4.0).contains(v)), "{flat:?}");
}
