// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The spectral functions given series that scripts cannot build: an empty series of a
//! known type (`REMOVETICK` resets the type, so no script ends with one), and the numbers of
//! a series that holds none.

use super::spectral::doubles;
use crate::error::Error;
use crate::error::fault::IllegalArgumentError;
use crate::gts::{GeoTimeSerie, Type, Values};
use crate::stack::Stack;
use crate::tests::test_stack;
use crate::value::Value;

/// A series of the given values, `bucketized` with a span of 10 when asked.
fn series(values: Values, bucketized: bool) -> Value {
    let mut gts = if bucketized {
        GeoTimeSerie::with_buckets(100, 10, 10)
    } else {
        GeoTimeSerie::new()
    };
    gts.values = values;
    Value::Gts(gts.shared())
}

/// The stack after `script`, run over the given arguments (bottom first).
fn run(arguments: Vec<Value>, script: &str) -> Stack {
    let mut stack = test_stack();
    for argument in arguments {
        stack.push(argument);
    }
    stack.exec(script).expect("the script should succeed");
    stack
}

/// The failure of `script` run over the given arguments (bottom first).
fn failure(arguments: Vec<Value>, script: &str) -> Error {
    let mut stack = test_stack();
    for argument in arguments {
        stack.push(argument);
    }
    stack
        .exec_checked(script)
        .expect_err("the script should fail")
        .root_cause()
        .clone()
}

#[test]
fn numbers_of_each_type() {
    let mut gts = GeoTimeSerie::new();
    gts.values = Values::Long(vec![1, -2]);
    assert_eq!(doubles(&gts), [1.0, -2.0]);
    gts.values = Values::Double(vec![0.5]);
    assert_eq!(doubles(&gts), [0.5]);
    for values in [
        Values::Undefined,
        Values::Boolean(vec![true]),
        Values::String(vec!["a".into()]),
    ] {
        gts.values = values;
        assert_eq!(doubles(&gts), Vec::<f64>::new());
    }
}

#[test]
fn a_transform_needs_a_value() {
    let transform = IllegalArgumentError::TransformLength.into();
    assert_eq!(
        failure(vec![series(Values::Double(vec![]), true)], "FFT"),
        transform
    );
    assert_eq!(
        failure(vec![series(Values::Double(vec![]), true)], "FFTAP"),
        transform
    );
    assert_eq!(
        failure(
            vec![
                series(Values::Double(vec![]), false),
                series(Values::Double(vec![]), false)
            ],
            "IFFT"
        ),
        transform
    );
}

#[test]
fn an_empty_series_is_windowed_by_any_algorithm() {
    // Warp 10 only finds the algorithm unknown when it has a value to weigh.
    let mut stack = run(
        vec![series(Values::Double(vec![]), false), "foo".into()],
        "FFTWINDOW",
    );
    let Ok(Value::Gts(windowed)) = stack.pop() else {
        panic!("a series should be left");
    };
    assert_eq!(windowed.borrow().size(), 0);
    assert_eq!(windowed.borrow().type_of(), Type::Undefined);
}

#[test]
fn an_empty_series_has_no_levels() {
    let mut stack = run(
        vec![series(Values::Double(vec![]), false), "lvl".into()],
        "DWTSPLIT",
    );
    assert_eq!(stack.pop().expect("a list"), Value::list(vec![]));
}

#[test]
fn an_empty_series_is_transformed_to_an_empty_series() {
    for (values, function) in [
        (Values::Long(vec![]), "FDWT"),
        (Values::Double(vec![]), "FDWT"),
        (Values::Double(vec![]), "IDWT"),
    ] {
        let mut stack = run(vec![series(values, false), "haar".into()], function);
        let Ok(Value::Gts(transformed)) = stack.pop() else {
            panic!("a series should be left");
        };
        assert_eq!(transformed.borrow().size(), 0, "{function}");
        assert_eq!(
            transformed.borrow().type_of(),
            Type::Undefined,
            "{function}"
        );
    }
}
