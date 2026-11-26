// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the image helpers with values no script can give them: each function checks the count
//! of its values before the helper is called, so only a direct call reaches the arity failure.

use super::{blend_mode, image_at, set_or_paste};
use crate::functions::processing::{Params, parse_params};
use crate::tests::test_stack;

const ARITY: &str = "Invalid number of parameters, expected one of [0] but found 0";

/// The parameters of a context with no value above it.
fn context_alone() -> Params {
    let mut stack = test_stack();
    stack
        .exec_checked("4 4 '2D' PGraphics")
        .expect("the context should be created");
    parse_params(&mut stack, &[0]).expect("the context alone should be read")
}

#[test]
fn an_image_is_read_from_a_value_above_the_context() {
    let message = image_at(&context_alone(), 0)
        .expect_err("no value to read an image from")
        .to_string();
    assert!(message.contains(ARITY), "{message}");
}

#[test]
fn a_blend_mode_is_read_from_the_last_value_above_the_context() {
    let message = blend_mode(&context_alone())
        .expect_err("no value to read a mode from")
        .to_string();
    assert!(message.contains(ARITY), "{message}");
}

#[test]
fn a_pixel_write_needs_its_two_coordinates_and_its_value() {
    let mut stack = test_stack();
    stack
        .exec_checked("4 4 '2D' PGraphics 1 1")
        .expect("the context and its coordinates should be on the stack");
    let params = parse_params(&mut stack, &[2]).expect("two values above the context");
    let message = set_or_paste(&params)
        .expect_err("a write takes a value after its coordinates")
        .to_string();
    assert!(
        message.contains("Invalid number of parameters, expected one of [2] but found 2"),
        "{message}"
    );
}
