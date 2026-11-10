// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the microsphere on what scripts cannot reach: vectors at the extremes of the range
//! of doubles, and the dimension of a point that is not the one of the samples.

use crate::error::math::MathError;
use crate::jrandom::JavaRandom;
use crate::microsphere::{Microsphere, MicrosphereParams, cos_angle, safe_norm};

/// The relative distance between two numbers.
fn relative(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

#[test]
fn the_norm_is_the_euclidean_one() {
    assert_eq!(safe_norm(&[3.0, 4.0]), 5.0);
    assert_eq!(safe_norm(&[0.0, 0.0]), 0.0);
    assert_eq!(safe_norm(&[]), 0.0);
    assert_eq!(safe_norm(&[-2.0]), 2.0);
}

#[test]
fn the_norm_does_not_overflow_nor_underflow() {
    // Components that are above the giant threshold, or below the dwarf one, which a naive sum
    // of squares would lose.
    for (vector, expected) in [
        (vec![3e20, 4e20], 5e20),
        (vec![4e20, 3e20], 5e20),
        (vec![3e20, 4e20, 1.0], 5e20),
        (vec![3e-21, 4e-21], 5e-21),
        (vec![4e-21, 3e-21], 5e-21),
        (vec![0.0, 3e-21, 4e-21], 5e-21),
        (vec![3e-21, 4e-21, 1.0], 1.0),
        (vec![3e20, 4e-21, 1.0], 3e20),
        (vec![4e-21, 3e20], 3e20),
        (vec![3e-21, 2e-21, 1e-21, 5.0], 5.0),
        (vec![3e-20, 1e-19], 1.044_030_650_891_055e-19),
    ] {
        assert!(
            relative(safe_norm(&vector), expected) < 1e-12,
            "{vector:?}: {} instead of {expected}",
            safe_norm(&vector)
        );
    }
}

#[test]
fn the_cosine_of_an_angle() {
    assert!(
        relative(
            cos_angle(&[1.0, 0.0], &[1.0, 1.0]),
            std::f64::consts::FRAC_1_SQRT_2
        ) < 1e-15
    );
    assert_eq!(cos_angle(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
    assert_eq!(cos_angle(&[2.0], &[3.0]), 1.0);
    assert_eq!(cos_angle(&[2.0], &[-3.0]), -1.0);
}

fn sphere(dimension: usize) -> Microsphere {
    Microsphere::new(
        MicrosphereParams {
            dimension,
            size: 10,
            max_dark_fraction: 0.5,
            dark_threshold: 0.01,
            background: f64::NAN,
        },
        &mut JavaRandom::new(7),
    )
    .expect("a microsphere")
}

#[test]
fn a_microsphere_has_the_dimension_it_is_given() {
    assert_eq!(sphere(3).dimension(), 3);
}

#[test]
fn a_point_has_the_dimension_of_the_samples() {
    let samples = vec![vec![0.0, 0.0], vec![1.0, 1.0]];
    assert_eq!(
        sphere(2)
            .value(&[0.5], &samples, &[0.0, 1.0], 2.0, 1e-16)
            .expect_err("one dimension for two"),
        MathError::DimensionMismatch {
            wrong: 2,
            expected: 1
        }
        .into()
    );
}
