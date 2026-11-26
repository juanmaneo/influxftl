// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the transforms of a graphics context: the shears of an angle, the inverse of the
//! transform on the plane, and the refusal of a product that overflows. The values are worked out
//! by hand from the matrix products.

use super::Transform;

#[test]
fn a_shear_of_an_infinite_tangent_is_refused_and_a_zero_one_is_the_identity() {
    assert!(Transform::shear_x(f32::INFINITY).is_err());
    assert!(Transform::shear_y(f32::NEG_INFINITY).is_err());
    assert_eq!(
        Transform::shear_x(0.0).expect("a zero angle is finite"),
        Transform::identity()
    );
    assert_eq!(
        Transform::shear_y(0.0).expect("a zero angle is finite"),
        Transform::identity()
    );
}

#[test]
fn the_inverse_of_a_translation_is_the_opposite_offset() {
    assert_eq!(
        Transform::translate(10.0, 20.0, 0.0)
            .inverse_plane()
            .expect("finite"),
        Some([1.0, 0.0, -10.0, 0.0, 1.0, -20.0])
    );
}

#[test]
fn the_inverse_of_a_scale_is_the_reciprocal_factors() {
    assert_eq!(
        Transform::scale(2.0, 4.0, 1.0)
            .inverse_plane()
            .expect("finite"),
        Some([0.5, 0.0, 0.0, 0.0, 0.25, 0.0])
    );
}

#[test]
fn a_transform_that_flattens_the_plane_has_no_inverse() {
    assert_eq!(
        Transform::scale(0.0, 1.0, 1.0)
            .inverse_plane()
            .expect("finite"),
        None
    );
}

#[test]
fn an_inverse_whose_determinant_or_reciprocal_overflows_is_refused() {
    assert!(
        Transform::scale(f32::MAX, f32::MAX, 1.0)
            .inverse_plane()
            .is_err()
    );
    assert!(Transform::scale(1e-39, 1.0, 1.0).inverse_plane().is_err());
}

#[test]
fn a_point_is_scaled_and_each_coordinate_must_stay_finite() {
    let scale = Transform::scale(2.0, 2.0, 2.0);
    assert_eq!(scale.apply(1.0, 2.0, 3.0).expect("finite"), [2.0, 4.0, 6.0]);
    assert!(scale.apply(f32::MAX, 0.0, 0.0).is_err());
    assert!(scale.apply(0.0, f32::MAX, 0.0).is_err());
    assert!(scale.apply(0.0, 0.0, f32::MAX).is_err());
}
