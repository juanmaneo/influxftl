// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Hand-written tests of the Processing functions, where Warp 10 is not the reference: the
//! values that are not finite numbers are refused rather than computed, the default limit on
//! pixels applies, a `null` context is refused, and the casts of [`super::processing`] refuse
//! what no script can reach. Each refusal is one of the deliberate divergences listed in
//! `DIVERGENCES.md`.

use super::processing::{float_of, int_of, parse_params};
use crate::stack::Stack;
use crate::tests::test_stack;
use crate::value::Value;

/// The message of the failure `script` gives on a fresh stack.
fn failure(script: &str) -> String {
    let mut stack = test_stack();
    stack
        .exec_checked(script)
        .expect_err("the script should fail")
        .to_string()
}

#[test]
fn three_d_contexts_are_created() {
    let mut stack = test_stack();
    stack
        .exec_checked("4 3 '3D' PGraphics")
        .expect("a 3D context should be created");
    let value = stack.pop().expect("the context should be on the stack");
    assert!(
        matches!(value, Value::Graphics(_)),
        "not a PGRAPHICS: {value:?}"
    );
}

#[test]
fn the_default_limit_refuses_a_large_context() {
    let mut stack = Stack::new();
    let err = stack
        .exec_checked("1001 1001 '2D' PGraphics")
        .expect_err("the context is over the default limit");
    assert!(
        err.to_string().contains(
            "PGraphics only allows PGRAPHICS with a total number of pixels less than 1000000 requested size was 1001x1001 (1002001)."
        ),
        "{err}"
    );
}

#[test]
fn a_result_that_is_not_finite_is_refused() {
    // Warp 10 pushes NaN (0/0) and Infinity (1e30 squared, a product that overflows, a division
    // by a zero range); the port refuses each one as a typed `NotFinite`.
    for script in [
        "4 4 '2D' PGraphics 1 1 1 Pnorm",
        "4 4 '2D' PGraphics 1e30 1e30 Pmag",
        "4 4 '2D' PGraphics 3e38 0 Ptranslate 3e38 0 Ptranslate",
        "4 4 '2D' PGraphics 5 0 0 0 20 Pmap",
        "4 4 '2D' PGraphics 1e30 1e30 0 0 Pdist",
    ] {
        let message = failure(script);
        assert!(
            message.contains("not a finite number"),
            "{script}: {message}"
        );
    }
}

#[test]
fn results_beyond_the_float_range_are_refused_where_warp_10_answers_infinity() {
    // Warp 10 3.5.0 answers `Infinity` for these (Java `float` arithmetic does not refuse it); the
    // port refuses the overflow, see `DIVERGENCES.md` section 5.4.
    for script in [
        "4 4 '2D' PGraphics 1e30 1e30 1e30 Pmag",
        "4 4 '2D' PGraphics -3e38 3e38 2 Plerp",
        "4 4 '2D' PGraphics '1E400' ->BD 4 Pmag",
        "4 4 '2D' PGraphics 3e38 0 Ptranslate 3e38 0 1 1 Pclip",
        "4 4 '2D' PGraphics 3e38 3e38 3e38 3e38 Pclip",
        "4 4 '2D' PGraphics 0 10 10 20 1e30 PbezierTangent",
        "4 4 '2D' PGraphics 0 10 10 20 1e30 PcurvePoint",
        "4 4 '2D' PGraphics 0 10 10 20 1e30 PcurveTangent",
    ] {
        let message = failure(script);
        assert!(
            message.contains("not a finite number"),
            "{script}: {message}"
        );
    }
}

#[test]
fn shapes_placed_beyond_the_float_range_are_refused_where_warp_10_draws_nothing() {
    // Warp 10 3.5.0 answers OK for these, with the shape drawn nowhere: its coordinates are
    // Infinity or NaN. The port refuses the overflow, see `DIVERGENCES.md` section 5.4.
    let square = r##"'<svg width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="#ffffff"/></svg>'"##;
    let stroke = r##"'<svg width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="none" stroke="#ff0000"/></svg>'"##;
    let narrow = r##"'<svg width="0" height="4"><rect x="0" y="0" width="4" height="4" fill="#ffffff"/></svg>'"##;
    for script in [
        format!("4 4 '2D' PGraphics {square} PloadShape 3e38 3e38 3e38 3e38 Pshape Ppixels"),
        format!("4 4 '2D' PGraphics {square} PloadShape 0 3e38 3e38 3e38 Pshape Ppixels"),
        format!("4 4 '2D' PGraphics {stroke} PloadShape 0 3e38 3e38 3e38 Pshape Ppixels"),
        format!("4 4 '2D' PGraphics {narrow} PloadShape 0 0 2 2 Pshape Ppixels"),
    ] {
        let message = failure(&script);
        assert!(
            message.contains("not a finite number"),
            "{script}: {message}"
        );
    }
}

#[test]
fn shapes_placed_beyond_the_float_range_by_their_mode_are_refused_where_warp_10_draws_nothing() {
    // Warp 10 3.5.0 answers OK for these, drawing nothing in any mode and on either axis: the
    // placement overflows a float. The port refuses the overflow, see `DIVERGENCES.md` section 5.4.
    for shape in [
        "'CORNERS' PrectMode -3e38 0 3e38 1 Prect",
        "'CENTER' PrectMode -3e38 0 3e38 1 Prect",
        "'RADIUS' PrectMode 0 0 3e38 1 Prect",
        "-3e38 0 3e38 1 Pellipse",
        "'CORNERS' PellipseMode -3e38 0 3e38 1 Pellipse",
        "'RADIUS' PellipseMode 0 0 3e38 1 Pellipse",
        "-3e38 0 3e38 1 0 1 Parc",
        "'CORNERS' PrectMode 0 -3e38 1 3e38 Prect",
        "'CENTER' PrectMode 0 -3e38 1 3e38 Prect",
        "'RADIUS' PrectMode 0 0 1 3e38 Prect",
        "0 -3e38 1 3e38 Pellipse",
        "'CORNERS' PellipseMode 0 -3e38 1 3e38 Pellipse",
        "'RADIUS' PellipseMode 0 0 1 3e38 Pellipse",
        "0 -3e38 1 3e38 0 1 Parc",
    ] {
        let script = format!("4 4 '2D' PGraphics {shape} Ppixels");
        let message = failure(&script);
        assert!(
            message.contains("not a finite number"),
            "{script}: {message}"
        );
    }
}

#[test]
fn curves_beyond_the_float_range_are_refused_where_warp_10_draws_nothing() {
    // Warp 10 3.5.0 answers OK for these, drawing nothing: the curve's coordinates overflow a
    // float. The port refuses the overflow, see `DIVERGENCES.md` section 5.4.
    for script in [
        "4 4 '2D' PGraphics 0 255 Pstroke -3e38 0 3e38 0 3e38 0 3e38 0 Pbezier Ppixels",
        "4 4 '2D' PGraphics 0 255 Pstroke -3e38 0 3e38 0 3e38 0 3e38 0 Pcurve Ppixels",
    ] {
        let message = failure(script);
        assert!(
            message.contains("not a finite number"),
            "{script}: {message}"
        );
    }
}

#[test]
fn a_three_d_curve_is_drawn_flat_on_its_context() {
    // Not oracle-checked: Warp 10 3.5.0 throws a NullPointerException on any 3D context. The
    // z-coordinates of the curve are read and the curve is drawn on the plane of the context.
    let mut stack = test_stack();
    stack
        .exec_checked("4 4 '3D' PGraphics 0 255 Pstroke 0 2.5 0 1 2.5 0 2 2.5 0 3 2.5 0 Pbezier")
        .expect("a 3D curve should be drawn");
    let value = stack.pop().expect("the context should be on the stack");
    assert!(
        matches!(value, Value::Graphics(_)),
        "not a PGRAPHICS: {value:?}"
    );
}

#[test]
fn three_d_shapes_refuse_the_coordinates_they_are_missing() {
    // Not oracle-checked: Warp 10 3.5.0 throws a NullPointerException on any 3D context, even
    // `4 4 '3D' PGraphics` alone. The messages and the counts are read from the bytecode of its
    // `Ppoint` and `Pline`, whose 3D branches need the z-coordinates after the graphics.
    let message = failure("4 4 '3D' PGraphics 1 1 Ppoint");
    assert!(
        message.contains("Ppoint missing z-coordinate."),
        "{message}"
    );
    let message = failure("4 4 '3D' PGraphics 1 1 2 2 Pline");
    assert!(
        message.contains("Pline missing z-coordinates."),
        "{message}"
    );
}

#[test]
fn rounded_corners_are_circular_arcs_where_warp_10_differs() {
    // Not oracle-checked as pixels: Processing 3 builds each corner of `Prect` as a quadratic
    // Bézier, and Warp 10 3.5.0 does not paint a rounded outline for `0 0 8 8 2 Prect` either.
    // It leaves (0, 0) and (1, 0) unpainted, where the port's circular corner of radius 2 paints
    // (1, 0). The port's corners are kept as circles; see `DIVERGENCES.md` section 43.
    let pixel = |x: i32, y: i32| {
        let mut stack = test_stack();
        stack
            .exec_checked(&format!(
                "8 8 '2D' PGraphics 0 Pbackground 255 Pfill PnoStroke 0 0 8 8 2 Prect {x} {y} Pget"
            ))
            .expect("the rectangle should be drawn");
        match stack.pop().expect("a pixel should be on the stack") {
            Value::Long(colour) => colour,
            other => panic!("not a pixel: {other:?}"),
        }
    };
    assert_eq!(pixel(0, 4), -1, "(0, 4) is on the left edge");
    assert_eq!(pixel(4, 0), -1, "(4, 0) is on the top edge");
    assert_eq!(pixel(1, 0), -1, "(1, 0) is inside the corner's circle");
    assert_eq!(
        pixel(0, 0),
        -16_777_216,
        "(0, 0) is outside the corner's circle"
    );
}

#[test]
fn data_that_is_not_an_image_is_refused_where_warp_10_pushes_a_broken_image() {
    // Warp 10 3.5.0 answers OK for these, with an image of size -1 by -1 (`Psize` gives `[-1,-1]`),
    // which JSON writes as `null`. The port has no image of negative size, so it refuses the data,
    // see `DIVERGENCES.md` section 43.
    for script in [
        "'data:image/png;base64,AAAA' Pdecode",
        "'data:image/png;base64,!!!' Pdecode",
        "'x' 'UTF-8' ->BYTES Pdecode",
    ] {
        let message = failure(script);
        assert!(
            message.contains("Pdecode cannot read the image"),
            "{script}: {message}"
        );
    }
}

#[test]
fn a_value_that_is_not_finite_is_refused_on_entry() {
    let message = failure("4 4 '2D' PGraphics NaN 1 Pmag");
    assert!(message.contains("not a finite number"), "{message}");
}

#[test]
fn a_colour_maximum_must_be_positive() {
    let message = failure("4 4 '2D' PGraphics 'RGB' 0 PcolorMode");
    assert!(
        message.contains("a colour maximum must be a finite positive number."),
        "{message}"
    );
}

#[test]
fn a_null_context_is_refused_by_a_maths_function() {
    // Warp 10 casts the null to a context without complaint and pushes it back as `null`; the
    // port has no null context, so the cast is refused as a null pointer.
    let message = failure("0 0 0 NULL Pdist");
    assert!(message.contains("NullPointerException"), "{message}");
}

#[test]
fn a_float_cast_refuses_what_is_not_finite() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let err = float_of(&Value::Double(value)).expect_err("a value that is not finite");
        assert!(err.to_string().contains("not a finite number"), "{err}");
        let err = int_of(&Value::Double(value)).expect_err("a value that is not finite");
        assert!(err.to_string().contains("not a finite number"), "{err}");
    }
}

#[test]
fn a_float_cast_refuses_a_double_too_big_for_a_float() {
    let err = float_of(&Value::Double(f64::MAX)).expect_err("the double overflows a float");
    assert!(err.to_string().contains("not a finite number"), "{err}");
}

#[test]
fn the_casts_refuse_null_and_other_kinds() {
    let err = float_of(&Value::Null).expect_err("null is no number");
    assert!(err.to_string().contains("NullPointerException"), "{err}");
    let err = int_of(&Value::Null).expect_err("null is no number");
    assert!(err.to_string().contains("NullPointerException"), "{err}");
    let text = Value::String("x".into());
    let err = float_of(&text).expect_err("a string is no number");
    assert!(err.to_string().contains("cannot be cast"), "{err}");
    let err = int_of(&text).expect_err("a string is no number");
    assert!(err.to_string().contains("cannot be cast"), "{err}");
}

#[test]
fn the_casts_keep_longs_and_truncate_doubles_as_java_does() {
    assert_eq!(float_of(&Value::Long(3)).expect("a long is a number"), 3.0);
    assert_eq!(
        int_of(&Value::Long(0x1_0000_0005)).expect("a long is a number"),
        5
    );
    assert_eq!(
        int_of(&Value::Double(-2.9)).expect("a finite double is a number"),
        -2
    );
}

#[test]
fn parameters_are_checked_against_their_counts() {
    let mut stack = test_stack();
    stack
        .exec_checked("4 4 '2D' PGraphics 1 2 3")
        .expect("the stack should be set up");
    let err = parse_params(&mut stack, &[2])
        .err()
        .expect("three values are not two");
    assert!(
        err.to_string().contains("expected one of [2] but found 3"),
        "{err}"
    );
}

/// The message of the failure `result` gives.
fn refusal<T>(result: crate::error::Result<T>) -> String {
    result
        .err()
        .expect("the count should be refused")
        .to_string()
}

#[test]
fn readers_refuse_the_counts_they_cannot_read() {
    // A context with no values above it: each reader asks for a count, which is refused, both
    // for a count that is not zero and for values past the end of the list.
    let mut stack = test_stack();
    stack
        .exec_checked("4 4 '2D' PGraphics")
        .expect("the context should be created");
    let params = parse_params(&mut stack, &[0]).expect("a context with no values above it");
    let none = "Invalid number of parameters, expected one of [0] but found 0";
    assert_eq!(refusal(params.values::<1>(0)), none);
    assert_eq!(refusal(params.values::<1>(1)), none);
    assert_eq!(refusal(params.values::<2>(0)), none);
    assert_eq!(refusal(params.values::<2>(1)), none);
    assert_eq!(refusal(params.values::<3>(0)), none);
    assert_eq!(refusal(params.values::<3>(1)), none);
    assert_eq!(refusal(params.values::<4>(0)), none);
    assert_eq!(refusal(params.values::<4>(1)), none);
    assert_eq!(refusal(params.values::<5>(0)), none);
    assert_eq!(refusal(params.values::<5>(1)), none);
    assert_eq!(refusal(params.values::<6>(0)), none);
    assert_eq!(refusal(params.values::<6>(1)), none);
    assert_eq!(refusal(params.values::<8>(0)), none);
    assert_eq!(refusal(params.values::<8>(1)), none);
    assert_eq!(refusal(params.floats_after::<1>(0)), none);
    assert_eq!(refusal(params.floats_after::<2>(0)), none);
    assert_eq!(refusal(params.floats_after::<3>(0)), none);
    assert_eq!(refusal(params.floats_after::<4>(0)), none);
    assert_eq!(refusal(params.floats_after::<5>(0)), none);
    assert_eq!(refusal(params.floats_after::<6>(0)), none);
    assert_eq!(refusal(params.floats_after::<8>(0)), none);
    assert_eq!(refusal(params.floats::<1>()), none);
    assert_eq!(refusal(params.floats::<2>()), none);
    assert_eq!(refusal(params.floats::<3>()), none);
    assert_eq!(refusal(params.floats::<4>()), none);
    assert_eq!(refusal(params.floats::<5>()), none);
    assert_eq!(refusal(params.floats::<6>()), none);
    assert_eq!(refusal(params.floats::<8>()), none);
    assert_eq!(refusal(params.floats_window::<2>(0)), none);
    assert_eq!(refusal(params.floats_window::<4>(0)), none);
    assert_eq!(refusal(params.floats_window::<6>(0)), none);
    assert_eq!(refusal(params.ints::<1>()), none);
    assert_eq!(refusal(params.ints::<2>()), none);
    assert_eq!(refusal(params.ints::<3>()), none);
    assert_eq!(refusal(params.ints::<4>()), none);
    assert_eq!(refusal(params.ints_window::<2>(0)), none);
    assert_eq!(refusal(params.ints_window::<8>(0)), none);
    assert_eq!(refusal(params.first()), none);
    assert_eq!(refusal(params.keyword(0)), none);
    assert_eq!(params.arity().to_string(), none);
}

#[test]
fn a_context_no_memory_can_hold_is_refused() {
    // 2^31 by 2^31 pixels of `u32` are more than `isize::MAX` bytes, so the reservation fails
    // before any pixel is written.
    let mut stack = test_stack();
    stack.limits.max_pixels = i64::MAX - 1;
    let err = stack
        .exec_checked("2147483647 2147483647 '2D' PGraphics")
        .expect_err("the pixel buffer cannot be reserved");
    assert!(
        err.to_string()
            .contains("not enough memory for 4611686014132420609 pixels"),
        "{err}"
    );
}

#[test]
fn a_resize_no_memory_can_hold_is_refused_for_a_graphics_and_for_an_image() {
    // As `a_context_no_memory_can_hold_is_refused`: the buffer of the resized context, or of the
    // image copied from it, cannot be reserved.
    for script in [
        "4 4 '2D' PGraphics 2147483647 2147483647 Presize",
        "4 4 '2D' PGraphics PtoImage 2147483647 2147483647 Presize",
    ] {
        let mut stack = test_stack();
        stack.limits.max_pixels = i64::MAX - 1;
        let err = stack
            .exec_checked(script)
            .expect_err("the pixel buffer cannot be reserved");
        assert!(
            err.to_string()
                .contains("not enough memory for 4611686014132420609 pixels"),
            "{script}: {err}"
        );
    }
}

#[test]
fn an_image_placed_beyond_the_float_range_is_refused_where_warp_10_draws_nothing() {
    // Warp 10 3.5.0 answers OK for this, drawing nothing: the placed corners overflow a float.
    // The port refuses the overflow, see `DIVERGENCES.md` section 5.4.
    let script = "4 4 '2D' PGraphics 'CORNERS' PimageMode 2 2 '2D' PGraphics 255 Pbackground PtoImage -3e38 0 3e38 3e38 Pimage";
    let message = failure(script);
    assert!(
        message.contains("not a finite number"),
        "{script}: {message}"
    );
}

#[test]
fn no_context_on_an_empty_stack_is_an_index_failure() {
    let err = parse_params(&mut Stack::new(), &[0])
        .err()
        .expect("no context on an empty stack");
    assert!(
        err.to_string()
            .contains("Index 0 out of bounds for length 0"),
        "{err}"
    );
}
