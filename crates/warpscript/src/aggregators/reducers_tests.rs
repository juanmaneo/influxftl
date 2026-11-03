// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of `reducer.argmin` / `argmax` and `reducer.shannonentropy.*`. The expected values
//! were checked against Warp 10 3.5.0 (`warp10io/warp10:3.5.0-alpine`).

use super::reducers::{Argminmax, ShannonEntropy};
use super::tests::{Window, longs};
use super::{Aggregator, Roles};
use crate::tests::{assert_fails, assert_top};
use crate::value::Value;

const A: &str =
    "NEWGTS 'a' RENAME { 'h' 'h1' } RELABEL 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 5 ADDVALUE";
const B: &str = "NEWGTS 'a' RENAME { 'h' 'h2' } RELABEL 1 NaN NaN NaN 4 ADDVALUE \
    2 NaN NaN NaN 5 ADDVALUE 3 NaN NaN NaN 1 ADDVALUE";
const C: &str =
    "NEWGTS 'a' RENAME { 'h' 'h 3' } RELABEL 1 NaN NaN NaN 4 ADDVALUE 3 NaN NaN NaN 0 ADDVALUE";
const D: &str = "NEWGTS 'a' RENAME { 'h' 'h4' } RELABEL 1 NaN NaN NaN 2.5 ADDVALUE \
    2 NaN NaN NaN 5.0 ADDVALUE 3 NaN NaN NaN 2.5 ADDVALUE";

/// The ticks and values of `[ series ]` reduced by `reducer`, as JSON.
fn reduced(series: &str, reducer: &str) -> String {
    format!(
        "[ [ {series} ] [] {reducer} ] REDUCE 0 GET 'r' STORE $r TICKLIST $r VALUES 2 ->LIST ->JSON"
    )
}

#[test]
fn arg_extrema() {
    let abc = format!("{A} {B} {C}");
    let abd = format!("{A} {B} {D}");
    for (series, reducer, expected) in [
        (
            &abc,
            "'h' 0 reducer.argmax",
            r#"[[1,2,3],["h2,h%203","h1,h2","h2"]]"#,
        ),
        (
            &abc,
            "'h' 1 reducer.argmax",
            r#"[[1,2,3],["h2","h1","h2"]]"#,
        ),
        (
            &abc,
            "'h' 0 reducer.argmin",
            r#"[[1,2,3],["h1","h1,h2","h%203"]]"#,
        ),
        (
            &abc,
            "'h' 2 reducer.argmin",
            r#"[[1,2,3],["h1","h1,h2","h%203"]]"#,
        ),
        (
            &abd,
            "'h' 0 reducer.argmax",
            r#"[[1,2,3],["h2","h1,h2,h4","h4"]]"#,
        ),
        (
            &abd,
            "'h' 0 reducer.argmin",
            r#"[[1,2,3],["h1","h1,h2,h4","h2"]]"#,
        ),
    ] {
        assert_top(&reduced(series, reducer), expected);
    }
    assert_top(
        "[ NEWGTS { 'h' 'v' } RELABEL 1 NaN NaN NaN 1 ADDVALUE 3 NaN NaN NaN 3 ADDVALUE \
         'h' 0 reducer.argmax 1 0 0 ] MAP 0 GET VALUES ->JSON",
        r#"["v","v"]"#,
    );
    assert_fails(
        &reduced(
            &format!("{A} NEWGTS 1 NaN NaN NaN 1 ADDVALUE"),
            "'h' 0 reducer.argmax",
        ),
        "reducer.argmax expects all labels to contain label 'h'.",
    );
    assert_fails(
        &reduced(
            &format!("{A} NEWGTS {{ 'h' 'x' }} RELABEL 1 NaN NaN NaN 'z' ADDVALUE"),
            "'h' 0 reducer.argmax",
        ),
        "reducer.argmax can only operate on numerical values.",
    );
    // `BUCKETIZE` hands a single label map.
    assert_fails(
        "[ NEWGTS { 'h' 'a' } RELABEL 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE \
         'h' 0 reducer.argmax 0 10 0 ] BUCKETIZE",
        "Index 1 out of bounds for length 1",
    );
}

/// An `Argminmax` over windows with the same label value `v` for every datapoint.
fn argminmax(argmin: bool) -> Argminmax {
    Argminmax {
        name: "reducer.x",
        label: "h".to_owned(),
        count: 0,
        argmin,
    }
}

#[test]
fn arg_extrema_edge_cases() {
    let labelled = |values: Vec<Value>| {
        let n = values.len();
        let mut window = Window::of(&values);
        window.ticks = (1..=n as i64).collect();
        (window, n)
    };
    let apply = |aggregator: &Argminmax, values: Vec<Value>| {
        let (window, n) = labelled(values);
        let names = vec!["g".to_owned(); n];
        let labels: Vec<_> = (0..n)
            .map(|i| indexmap::IndexMap::from([("h".to_owned(), i.to_string())]))
            .collect();
        aggregator
            .apply(&super::AggregatorArgs {
                tick: 100,
                names: &names,
                labels: &labels,
                ticks: &window.ticks,
                locations: &window.locations,
                elevations: &window.elevations,
                values: &window.values,
                window: None,
            })
            .unwrap()
            .value
    };
    // The sentinel extremum counts as an extremum.
    assert_eq!(
        apply(&argminmax(true), longs(&[i64::MAX, i64::MAX])),
        Value::string("0,1")
    );
    // A `DOUBLE` first: the `LONG` extremum is not carried over.
    assert_eq!(
        apply(&argminmax(false), vec![Value::Double(1.5), Value::Long(2)]),
        Value::string("1")
    );
    // `NaN`: never the maximum, always replacing the minimum.
    assert_eq!(
        apply(
            &argminmax(false),
            vec![Value::Double(1.0), Value::Double(f64::NAN)]
        ),
        Value::string("0")
    );
    assert_eq!(
        apply(
            &argminmax(true),
            vec![
                Value::Double(1.0),
                Value::Double(f64::NAN),
                Value::Double(0.5)
            ]
        ),
        Value::string("2")
    );
    // Nothing but `null`s: the empty string.
    assert_eq!(
        apply(&argminmax(true), vec![Value::Null]),
        Value::string("")
    );
    assert_eq!(argminmax(true).roles(), Roles::ALL);
    let negative = Argminmax {
        count: -1,
        ..argminmax(false)
    };
    assert_eq!(apply(&negative, longs(&[3, 3])), Value::string("0,1"));
}

#[test]
fn arg_extrema_builders() {
    assert_top("'h' 2 reducer.argmax SNAPSHOT", "'h' 2 reducer.argmax ");
    assert_top(
        "'a b' 4294967297 reducer.argmin SNAPSHOT",
        "'a%20b' 1 reducer.argmin ",
    );
    assert_top("'h' 2 reducer.argmax TYPEOF", "AGGREGATOR");
    for (script, message) in [
        (
            "'h' 'a' reducer.argmax",
            "reducer.argmax expects an integer number of maxima to report (use 0 to report them all).",
        ),
        (
            "1 2 reducer.argmin",
            "reducer.argmin expects the name of the label to report on the second level of the stack.",
        ),
        ("reducer.argmax", "Empty stack"),
        ("2 reducer.argmax", "Empty stack"),
    ] {
        assert_fails(script, message);
    }
}

#[test]
fn shannon_entropy() {
    let abc = format!("{A} {B} {C}");
    for reducer in ["reducer.shannonentropy.0", "reducer.shannonentropy.1"] {
        assert_top(
            &reduced(&abc, reducer),
            "[[1,2,3],[0.8783471047618534,1.0,0.0]]",
        );
    }
    assert_fails(
        &reduced(&format!("{A} {B} {D}"), "reducer.shannonentropy.0"),
        "reducer.shannonentropy.0 can only operate on long values.",
    );
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE 3 NaN NaN NaN 3 ADDVALUE reducer.shannonentropy.1 1 0 0 ] \
         MAP 0 GET VALUES ->JSON",
        "[1.0,0.8112781244591328]",
    );
    let entropy = |invert| ShannonEntropy {
        name: "reducer.x",
        invert,
    };
    assert_eq!(
        Window::of(&longs(&[5])).value(&entropy(false)),
        Value::Double(0.0)
    );
    assert_eq!(
        Window::of(&longs(&[5])).value(&entropy(true)),
        Value::Double(1.0)
    );
    // No value: `0 / log(0)`.
    let nothing = Window::of(&[Value::Null]).value(&entropy(false));
    assert!(nothing.as_f64().unwrap() == 0.0 && nothing.as_f64().unwrap().is_sign_negative());
    assert_eq!(entropy(true).roles(), Roles::ALL);
    assert_top(
        "reducer.shannonentropy.0 SNAPSHOT",
        "'reducer.shannonentropy.0' FUNCREF ",
    );
}
