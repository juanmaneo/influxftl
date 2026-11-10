// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! `INTERPOLATOR.ND.*` on what scripts cannot give it: a capability, an operating system that
//! has no entropy, parameters too deep to snapshot.

use super::interpolators::interpolate_nd;
use crate::error::Error;
use crate::error::fault::NumberFormatError;
use crate::error::platform::PlatformError;
use crate::tests::test_stack;
use crate::value::Value;

const SAMPLES: &str = "[ [ 0.0 ] [ 1.0 ] ] [ 0.0 1.0 ]";

/// The result of `script` on a stack with the capability `interpolation.microsphere.max.elements`.
fn with_limit(limit: &str, script: &str) -> Result<(), Error> {
    let mut stack = test_stack();
    stack.capabilities_mut().insert(
        "interpolation.microsphere.max.elements".to_owned(),
        limit.to_owned(),
    );
    stack
        .exec_checked(script)
        .map_err(|err| err.root_cause().clone())
}

#[test]
fn the_number_of_elements_is_limited_by_a_capability() {
    let script = format!("{SAMPLES} {{ 'elements' 60 }} INTERPOLATOR.ND.MICROSPHERE");
    assert!(with_limit("100", &script).is_ok());
    assert!(with_limit("60", &script).is_ok());
    assert_eq!(
        with_limit("59", &script)
            .expect_err("above the limit")
            .to_string(),
        "INTERPOLATOR.ND.MICROSPHERE: argument elements is above the limit interpolation.microsphere.max.elements"
    );
}

#[test]
fn the_limit_of_the_number_of_elements_is_a_number() {
    let script = format!("{SAMPLES} INTERPOLATOR.ND.MICROSPHERE");
    assert_eq!(
        with_limit("many", &script).expect_err("not a number"),
        NumberFormatError::InvalidInput("many".to_owned()).into()
    );
    assert_eq!(
        with_limit("4294967296", &script).expect_err("too large"),
        NumberFormatError::InvalidInput("4294967296".to_owned()).into()
    );
}

#[test]
fn an_unseeded_interpolation_needs_entropy() {
    let mut stack = test_stack();
    stack.exec(SAMPLES).expect("the samples should be pushed");
    assert_eq!(
        interpolate_nd(&mut stack, "INTERPOLATOR.ND.MICROSPHERE", false, || Err(
            getrandom::Error::UNSUPPORTED
        ))
        .expect_err("no entropy"),
        PlatformError::Entropy(getrandom::Error::UNSUPPORTED).into()
    );
}

#[test]
fn a_function_of_the_microsphere_has_a_name() {
    let mut stack = test_stack();
    stack
        .exec(&format!("{SAMPLES} INTERPOLATOR.ND.MICROSPHERE"))
        .expect("the function should be built");
    let Ok(Value::Function(function)) = stack.pop() else {
        panic!("a function should be left");
    };
    assert_eq!(function.name(), "MICROSPHERE");
}

#[test]
fn parameters_too_deep_to_snapshot_are_refused() {
    let deep = format!("{}{}", "[ ".repeat(20), "] ".repeat(20));
    let script = format!("{SAMPLES} {{ 'elements' 3 'deep' {deep} }} INTERPOLATOR.ND.MICROSPHERE");
    assert_eq!(
        test_stack()
            .exec_checked(&script)
            .expect_err("too deep")
            .root_cause()
            .to_string(),
        "Recursive data structures exceeded 16 levels."
    );
}
