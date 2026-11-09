// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The `GEOSHAPE` operations given a shape that scripts cannot build, a geocell without
//! resolution: it fails like the Java does, on the index of its resolution.

use std::rc::Rc;

use crate::error::Error;
use crate::error::fault::BoundsError;
use crate::geoshape::GeoShape;
use crate::tests::test_stack;
use crate::value::Value;

/// A shape of one geocell of resolution 2.
fn shape() -> Value {
    Value::GeoShape(Rc::new(GeoShape::from_cells(vec![0x1100_0000_0000_0000])))
}

/// A shape of a geocell without resolution.
fn broken() -> Value {
    Value::GeoShape(Rc::new(GeoShape::from_cells(vec![1])))
}

/// The failure of `function` applied to `arguments` (bottom first), under the context of
/// the script.
fn failure(function: &str, arguments: Vec<Value>) -> Error {
    let mut stack = test_stack();
    for argument in arguments {
        stack.push(argument);
    }
    stack
        .exec_checked(function)
        .expect_err("the shape has a cell without resolution")
        .root_cause()
        .clone()
}

fn array_index() -> Error {
    BoundsError::ArrayIndex {
        index: -1,
        length: 16,
    }
    .into()
}

#[test]
fn a_union_fails_on_a_cell_without_resolution() {
    assert_eq!(failure("GEO.UNION", vec![shape(), broken()]), array_index());
    assert_eq!(
        failure("GEO.UNION", vec![Value::list(vec![shape(), broken()])]),
        array_index()
    );
}

#[test]
fn a_difference_fails_on_a_cell_without_resolution() {
    assert_eq!(
        failure("GEO.DIFFERENCE", vec![broken(), shape()]),
        array_index()
    );
}

#[test]
fn a_normalization_fails_on_a_cell_without_resolution() {
    assert_eq!(
        failure("GEO.NORMALIZE", vec![broken(), shape()]),
        array_index()
    );
}

#[test]
fn a_regexp_fails_on_a_cell_without_resolution() {
    assert_eq!(
        failure("GEO.REGEXP", vec![broken()]),
        BoundsError::StringSlice {
            begin: 1,
            end: 0,
            length: 16
        }
        .into()
    );
}
