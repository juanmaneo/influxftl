// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of `STRICTMAPPER` and `STRICTREDUCER`. The expected values were checked against
//! Warp 10 3.5.0 (`warp10io/warp10:3.5.0-alpine`).

use super::Roles;
use super::tests::{Window, longs};
use crate::tests::{assert_fails, assert_top, fail, top};
use crate::value::Value;

const SERIES: &str = "NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 3 NaN NaN NaN 3 ADDVALUE \
    5 NaN NaN NaN 5 ADDVALUE 9 NaN NaN NaN 9 ADDVALUE";

/// The ticks and values of the series mapped with `parameters`, as JSON.
fn mapped(parameters: &str) -> String {
    format!("[ {SERIES} {parameters} ] MAP 0 GET 'r' STORE $r TICKLIST $r VALUES 2 ->LIST ->JSON")
}

#[test]
fn strict_mappers() {
    for (parameters, expected) in [
        (
            "mapper.sum 2 3 STRICTMAPPER 2 0 0",
            "[[2,3,5,9],[3,6,10,17]]",
        ),
        ("mapper.sum 3 3 STRICTMAPPER 2 0 0", "[[3,5,9],[6,10,17]]"),
        ("mapper.sum -3 -5 STRICTMAPPER 2 0 0", "[[3,5],[6,10]]"),
        ("mapper.sum 0 -3 STRICTMAPPER 2 0 0", "[[1,2,3],[1,3,6]]"),
        (
            "mapper.sum 0 0 STRICTMAPPER 1 0 0",
            "[[1,2,3,5,9],[1,3,5,8,14]]",
        ),
        (
            "<% DROP [ 0 NaN NaN NaN 1 ] %> 2 3 STRICTMAPPER 1 0 0",
            "[[2,3,5,9],[1,1,1,1]]",
        ),
        (
            "<% DROP [ 0 NaN NaN NaN 1 ] %> MACROMAPPER 2 3 STRICTMAPPER 1 0 0",
            "[[2,3,5,9],[1,1,1,1]]",
        ),
        (
            "<% DROP [ 0 NaN NaN NaN 1 ] %> -2 -3 STRICTMAPPER 1 0 0",
            "[[2,3,5],[1,1,1]]",
        ),
    ] {
        assert_top(&mapped(parameters), expected);
    }
    // The function the macro form compiles: an empty series is replaced by `[ NULL ]`.
    assert_top(
        "<% 1 %> 1 2 STRICTMAPPER 'm' STORE NEWGTS $m EVAL",
        Value::list(vec![Value::Null]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1 ADDVALUE <% SIZE %> 1 2 STRICTMAPPER EVAL",
        1,
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1 ADDVALUE 3 NaN NaN NaN 1 ADDVALUE <% SIZE %> 0 -2 STRICTMAPPER EVAL",
        Value::list(vec![Value::Null]),
    );
    let message = fail("1 <% 1 %> 1 2 STRICTMAPPER EVAL").to_string();
    assert!(
        message.contains("STRICTMAPPER's resulting function can only be applied on a GTS."),
        "{message}"
    );
    assert_fails("<% 1 %> 1 2 STRICTMAPPER EVAL", "Empty stack");
}

#[test]
fn strict_mapper_builder() {
    for (script, snapshot) in [
        (
            "mapper.abs 1 2",
            "<% 'mapper.abs' FUNCREF 1 2 STRICTMAPPER %> EVAL ",
        ),
        (
            "3 mapper.add 1 -2",
            "<% 3 mapper.add 1 -2 STRICTMAPPER %> EVAL ",
        ),
        (
            "mapper.sum 3 -2",
            "<% 'mapper.sum' FUNCREF 3 -2 STRICTMAPPER %> EVAL ",
        ),
        (
            "mapper.sum 1.5 2.7",
            "<% 'mapper.sum' FUNCREF 1 2 STRICTMAPPER %> EVAL ",
        ),
        (
            "<% 1 %> 1 2",
            "<% <% <% 1 %> 1 2 STRICTMAPPER %> EVAL EVAL %> ",
        ),
        (
            "mapper.sum -9223372036854775808 -9223372036854775808",
            "<% 'mapper.sum' FUNCREF -9223372036854775807 -9223372036854775807 STRICTMAPPER %> EVAL ",
        ),
    ] {
        assert_top(&format!("{script} STRICTMAPPER SNAPSHOT"), snapshot);
    }
    assert_top("<% 1 %> 1 2 STRICTMAPPER TYPEOF", "MACRO");
    assert_top("mapper.abs 1 2 STRICTMAPPER TYPEOF", "AGGREGATOR");
    // The snapshot of the macro form evaluates back to it.
    assert_top(
        "<% 1 %> 1 2 STRICTMAPPER SNAPSHOT EVAL SNAPSHOT",
        "<% <% <% 1 %> 1 2 STRICTMAPPER %> EVAL EVAL %> ",
    );
    for (script, message) in [
        (
            "mapper.sum 3 2",
            "STRICTMAPPER expects abs(min) <= abs(max) when min and max both express a count or both express a duration",
        ),
        (
            "mapper.sum -3 -2",
            "STRICTMAPPER expects abs(min) <= abs(max) when min and max both express a count or both express a duration",
        ),
        (
            "mapper.sum 'a' 2",
            "STRICTMAPPER expects a minimum (inclusive) number of values or a minimum timespan below the top of the stack.",
        ),
        (
            "mapper.sum 1 'a'",
            "STRICTMAPPER expects a maximum (inclusive) number of values or a minimum timespan on top of the stack.",
        ),
        (
            "1 1 2",
            "STRICTMAPPER expects a mapper or a macro below the extrema defining the value count range or timespan.",
        ),
        (
            "reducer.sum STRICTREDUCER 1 2",
            "STRICTMAPPER expects a mapper or a macro below the extrema defining the value count range or timespan.",
        ),
        ("1 2", "Empty stack"),
        ("2", "Empty stack"),
        ("", "Empty stack"),
    ] {
        assert_fails(&format!("{script} STRICTMAPPER"), message);
    }
    let object = top("mapper.abs 1 2 STRICTMAPPER");
    let Value::Function(function) = &object else {
        panic!("not a function");
    };
    assert_eq!(function.name(), "STRICTMAPPER");
    let strict = super::aggregator_of(&object).expect("aggregator_of should succeed");
    assert_eq!(strict.roles(), Roles::MAPPER);
    // Applied directly: out of the bounds no result, within them the mapper's.
    assert_eq!(Window::of(&longs(&[-3])).value(strict), Value::Long(3));
    assert_eq!(Window::of(&longs(&[-3, 4, 5])).value(strict), Value::Null);
    assert_eq!(Window::of(&[]).value(strict), Value::Null);
    assert_fails(
        "[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE mapper.sum 0 0 STRICTMAPPER 0 10 0 ] BUCKETIZE",
        "BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
    );
}

const A: &str =
    "NEWGTS 'a' RENAME { 'h' 'h1' } RELABEL 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 5 ADDVALUE";
const B: &str = "NEWGTS 'a' RENAME { 'h' 'h2' } RELABEL 1 NaN NaN NaN 4 ADDVALUE \
    2 NaN NaN NaN 5 ADDVALUE 3 NaN NaN NaN 1 ADDVALUE";
const C: &str =
    "NEWGTS 'a' RENAME { 'h' 'h 3' } RELABEL 1 NaN NaN NaN 4 ADDVALUE 3 NaN NaN NaN 0 ADDVALUE";

#[test]
fn strict_reducers() {
    assert_top(
        &format!(
            "[ [ {A} {B} {C} ] [] reducer.sum STRICTREDUCER ] REDUCE 0 GET 'r' STORE \
             $r TICKLIST $r VALUES 2 ->LIST ->JSON"
        ),
        "[[1],[9]]",
    );
    assert_top(
        &format!(
            "[ [ {A} {B} {C} ] [] <% DROP [ 7 ] %> MACROREDUCER STRICTREDUCER ] REDUCE 0 GET VALUES ->JSON"
        ),
        "[7]",
    );
    for (script, snapshot) in [
        (
            "reducer.sum",
            "<% 'reducer.sum' FUNCREF STRICTREDUCER %> EVAL ",
        ),
        ("5 mapper.gt", "<% 5 mapper.gt STRICTREDUCER %> EVAL "),
    ] {
        assert_top(&format!("{script} STRICTREDUCER SNAPSHOT"), snapshot);
        assert_top(&format!("{script} STRICTREDUCER TYPEOF"), "AGGREGATOR");
    }
    for script in ["1", "mapper.abs", "mapper.abs 1 2 STRICTMAPPER"] {
        assert_fails(
            &format!("{script} STRICTREDUCER"),
            "STRICTREDUCER expects a reducer on top of the stack.",
        );
    }
    assert_fails("STRICTREDUCER", "Empty stack");
    let object = top("reducer.sum STRICTREDUCER");
    let Value::Function(function) = &object else {
        panic!("not a function");
    };
    assert_eq!(function.name(), "STRICTREDUCER");
    let strict = super::aggregator_of(&object).expect("aggregator_of should succeed");
    assert!(strict.roles().reducer && !strict.roles().mapper && !strict.roles().bucketizer);
    assert_eq!(Window::of(&longs(&[1, 2])).value(strict), Value::Long(3));
    assert_eq!(
        Window::of(&[Value::Long(1), Value::Null]).value(strict),
        Value::Null
    );
    assert_fails(
        &format!("[ {A} reducer.sum STRICTREDUCER 0 0 0 ] MAP"),
        "MAP expects a mapper function or a macro after Geo Time Series.",
    );
}
