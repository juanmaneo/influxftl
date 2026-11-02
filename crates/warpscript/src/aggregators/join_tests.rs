// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the `join` aggregator. The expected values were checked against Warp 10 3.5.0
//! (`warp10io/warp10:3.5.0-alpine`).

use super::Aggregated;
use super::join::Join;
use super::tests::{Window, bucketized, longs};
use crate::tests::{assert_fails, assert_stack, assert_top};
use crate::value::Value;

#[test]
fn join_concatenates_the_values() {
    assert_top(
        &format!("{} VALUES", bucketized("1 2 3", "'-' bucketizer.join")),
        Value::list(vec![Value::from("1-2-3")]),
    );
    assert_top(
        &format!("{} VALUES", bucketized("1.5 2.0 1e100", "', ' mapper.join")),
        Value::list(vec![Value::from("1.5, 2.0, 1.0E100")]),
    );
    // A string in a BOOLEAN series is stored as a boolean.
    assert_top(
        &format!("{} VALUES", bucketized("true 'b'", "'-' reducer.join")),
        Value::list(vec![Value::from("true-true")]),
    );
    assert_top(
        &format!(
            "{} VALUES",
            bucketized("'a' 'b'", "'' reducer.join.nonnull")
        ),
        Value::list(vec![Value::from("ab")]),
    );
    // Location and elevation of the most recent datapoint.
    assert_stack(
        "[ NEWGTS 10 48.0 2.0 100 1 ADDVALUE 20 49.0 3.0 200 2 ADDVALUE '-' bucketizer.join 0 100 0 ] BUCKETIZE 0 GET DUP LOCATIONS 2 ->LIST SWAP ELEVATIONS",
        &[
            Value::list(vec![
                Value::list(vec![Value::Double(48.999_999_971_129)]),
                Value::list(vec![Value::Double(2.999_999_988_824_129)]),
            ]),
            Value::list(longs(&[200])),
        ],
    );
    assert_top("'-' bucketizer.join SNAPSHOT", "'-' bucketizer.join ");
    assert_top(
        "' ' reducer.join.urlencoded SNAPSHOT",
        "'%20' reducer.join.urlencoded ",
    );
    assert_top("'-' bucketizer.join TYPEOF", "AGGREGATOR");
    assert_fails(
        "1 bucketizer.join",
        "bucketizer.join expects a STRING seperator.",
    );
    assert_fails("mapper.join", "Empty stack");
}

#[test]
fn join_on_windows_with_nulls() {
    let ignore = Join {
        separator: "-".to_owned(),
        ignore_nulls: true,
        urlencode: false,
        null_string: None,
    };
    let forbid = Join {
        ignore_nulls: false,
        ..ignore.clone()
    };
    let with_null = Window::of(&[Value::Long(1), Value::Null, Value::Long(2)]);
    assert_eq!(with_null.value(&ignore), Value::from("1-2"));
    assert_eq!(with_null.value(&forbid), Value::Null);
    // A skipped null still counts as a first element.
    assert_eq!(
        Window::of(&[Value::Null, Value::Long(1)]).value(&ignore),
        Value::from("-1")
    );
    assert_eq!(Window::of(&[]).apply(&ignore).unwrap(), Aggregated::none());
    // `reducer.join.urlencoded`: percent-encoded values, nulls rendered as the empty string
    // (and still making the result null, as nulls are not ignored).
    let urlencoded = Join {
        separator: ",".to_owned(),
        ignore_nulls: false,
        urlencode: true,
        null_string: Some(""),
    };
    assert_eq!(
        Window::of(&[Value::from("a b"), Value::from("c/d")]).value(&urlencoded),
        Value::from("a%20b,c%2Fd")
    );
    assert_eq!(
        Window::of(&[Value::from("a"), Value::Null]).value(&urlencoded),
        Value::Null
    );
    let placeholder = Join {
        ignore_nulls: true,
        ..urlencoded
    };
    assert_eq!(
        Window::of(&[Value::Null, Value::Long(1), Value::Null, Value::Long(2)]).value(&placeholder),
        Value::from(",1,,2")
    );
    let latest = Window {
        ticks: vec![2, 1],
        locations: vec![7, 8],
        elevations: vec![70, 80],
        values: vec![Value::Boolean(true), Value::Double(0.5)],
    };
    assert_eq!(
        latest.apply(&ignore).unwrap(),
        Aggregated {
            tick: 0,
            location: 7,
            elevation: 70,
            value: Value::from("true-0.5"),
        }
    );
}
