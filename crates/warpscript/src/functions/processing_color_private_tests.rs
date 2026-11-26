// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the colour helpers with no values. The functions refuse that count in
//! `parse_params` before a helper is called, so only a direct call reaches the arity failure.

use super::{colour_of, paint_colour, tint_colour};
use crate::processing::color::ColorMode;

const ARITY: &str = "Invalid number of parameters, expected one of [1, 2, 3, 4] but found 0";

#[test]
fn no_values_is_an_arity_failure_for_every_colour_helper() {
    let mode = ColorMode::default();
    let message = paint_colour(&mode, &[])
        .expect_err("a colour takes one to four values")
        .to_string();
    assert!(message.contains(ARITY), "{message}");
    let message = tint_colour(&mode, &[])
        .expect_err("a tint takes one to four values")
        .to_string();
    assert!(message.contains(ARITY), "{message}");
    let message = colour_of(&mode, &[])
        .expect_err("a colour takes one to four values")
        .to_string();
    assert!(message.contains(ARITY), "{message}");
}
