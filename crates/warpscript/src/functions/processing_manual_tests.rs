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
