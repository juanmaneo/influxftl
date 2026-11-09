// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of `CompareTo`. The expected values were checked against Warp 10 3.5.0
//! (`warp10io/warp10:3.5.0-alpine`).

use super::compare::{CompareTo, Comparison, Test};
use super::tests::{Window, longs};
use super::{Aggregated, Aggregator, Roles};
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::tests::{assert_fails, assert_top};
use crate::value::Value;

/// The `[ ticks values ]` of the series `datapoints` mapped by `mapper`, as JSON.
fn mapped(datapoints: &str, mapper: &str) -> String {
    format!(
        "[ NEWGTS {datapoints} 'g' STORE $g {mapper} 0 0 0 ] MAP 0 GET 'r' STORE \
         $r TICKLIST $r VALUES 2 ->LIST ->JSON"
    )
}

const NUMBERS: &str =
    "1 NaN NaN NaN 2.7 ADDVALUE 2 NaN NaN NaN 3.0 ADDVALUE 3 NaN NaN NaN NaN ADDVALUE";
const COUNT: &str = "1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 3 NaN NaN NaN 3 ADDVALUE";
const BOOLEANS: &str = "1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE";
const LOCATED: &str = "1 48 2 NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 3 10 20 100 3 ADDVALUE";

#[test]
fn comparisons_of_values() {
    for (datapoints, mapper, expected) in [
        (NUMBERS, "2 mapper.eq", "[[1],[2.7]]"),
        (NUMBERS, "2.7 mapper.ge", "[[1,2,3],[2.7,3.0,NaN]]"),
        (NUMBERS, "'3' mapper.gt", "[[2,3],[3.0,NaN]]"),
        (NUMBERS, "3.0 mapper.lt", "[[1],[2.7]]"),
        (COUNT, "1 mapper.ne", "[[2,3],[2,3]]"),
        (COUNT, "2 mapper.le", "[[1,2],[1,2]]"),
        (COUNT, "2.0 mapper.eq", "[[2],[2]]"),
        (BOOLEANS, "true mapper.eq", "[[1],[true]]"),
        (BOOLEANS, "true mapper.ne", "[[2],[false]]"),
        (BOOLEANS, "1 mapper.eq", "[[],[]]"),
        (BOOLEANS, "1.0 mapper.eq", "[[],[]]"),
        (BOOLEANS, "'true' mapper.eq", "[[1],[true]]"),
    ] {
        assert_top(&mapped(datapoints, mapper), expected);
    }
}

#[test]
fn comparisons_of_ticks_and_positions() {
    for (datapoints, mapper, expected) in [
        (COUNT, "2 mapper.le.tick", "[[1,2],[1,2]]"),
        (COUNT, "2 mapper.gt.tick", "[[3],[3]]"),
        (LOCATED, "47.9 mapper.gt.lat", "[[1],[1]]"),
        (LOCATED, "2 mapper.lt.lon", "[[1,2],[1,2]]"),
        (LOCATED, "100 mapper.eq.elev", "[[3],[3]]"),
        (LOCATED, "0 mapper.gt.hhcode", "[[2],[2]]"),
        (LOCATED, "0 mapper.lt.elev", "[[1,2],[1,2]]"),
        (LOCATED, "-90 mapper.ge.lat", "[[1,2,3],[1,2,3]]"),
        (LOCATED, "-180.0 mapper.eq.lon", "[[],[]]"),
    ] {
        assert_top(&mapped(datapoints, mapper), expected);
    }
}

#[test]
fn earliest_passing_datapoint() {
    // `BUCKETIZE` puts the result at the end of the bucket.
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN 5 ADDVALUE 3 NaN NaN NaN 1 ADDVALUE 5 NaN NaN NaN 7 ADDVALUE \
         2 mapper.gt 0 10 0 ] BUCKETIZE 0 GET 'r' STORE $r TICKLIST $r VALUES 2 ->LIST ->JSON",
        "[[10],[5]]",
    );
    // Unsorted ticks: the earliest one wins, the first of equal ticks.
    let window = Window {
        ticks: vec![5, 3, 3, 9],
        locations: vec![11, 12, 13, 14],
        elevations: vec![21, 22, 23, 24],
        values: longs(&[1, 2, 3, 4]),
    };
    let greater_than_zero = CompareTo {
        reference: Value::Long(0),
        test: Test::Long(0),
        comparison: Comparison::Gt,
    };
    assert_eq!(
        window.apply(&greater_than_zero).unwrap(),
        Aggregated {
            tick: 3,
            location: 12,
            elevation: 22,
            value: Value::Long(2),
        }
    );
    let nothing = CompareTo {
        reference: Value::Long(9),
        test: Test::Long(9),
        comparison: Comparison::Gt,
    };
    assert_eq!(
        window.apply(&nothing).unwrap(),
        Aggregated {
            tick: 100,
            location: NO_LOCATION,
            elevation: NO_ELEVATION,
            value: Value::Null,
        }
    );
    assert_eq!(nothing.roles(), Roles::ALL);
}

#[test]
fn null_values_of_reducers() {
    let window = Window::of(&[Value::Null, Value::Long(1)]);
    let tests = [
        Test::Long(1),
        Test::Double(1.0),
        Test::Boolean(true),
        Test::String("1".into()),
    ];
    for (test, expected) in tests.into_iter().zip([
        Some(Value::Long(1)),
        Some(Value::Long(1)),
        Some(Value::Null),
        None,
    ]) {
        let compare = CompareTo {
            reference: Value::Null,
            test,
            comparison: Comparison::Eq,
        };
        match expected {
            Some(value) => assert_eq!(window.value(&compare), value),
            None => {
                let err = window.apply(&compare).unwrap_err();
                assert_eq!(
                    crate::error::compat::exception_class(&err),
                    "java.lang.NullPointerException"
                );
                assert_eq!(err.to_string(), "NullPointerException");
            }
        }
    }
}

#[test]
fn builders() {
    for (name, _) in super::compare::FUNCTIONS {
        assert_top(&format!("1 {name} SNAPSHOT"), format!("1 {name} "));
        assert_top(&format!("1 {name} TYPEOF"), "AGGREGATOR");
        let object = crate::tests::top(&format!("1 {name}"));
        let roles = super::aggregator_of(&object).unwrap().roles();
        assert_eq!(roles, Roles::ALL, "{name}");
        assert_fails(name, "Empty stack");
    }
    assert_top("'a b' mapper.eq SNAPSHOT", "'a%20b' mapper.eq ");
    assert_top("2.5 mapper.lt.lon SNAPSHOT", "2.5 mapper.lt.lon ");
    assert_top("true mapper.ne SNAPSHOT", "true mapper.ne ");
    for (script, message) in [
        ("true mapper.gt", "mapper.gt cannot compare with a BOOLEAN."),
        ("true mapper.le", "mapper.le cannot compare with a BOOLEAN."),
        (
            "true mapper.eq.tick",
            "mapper.eq.tick can only compare with a LONG.",
        ),
        (
            "1.5 mapper.eq.tick",
            "mapper.eq.tick can only compare with a LONG.",
        ),
        (
            "1.5 mapper.ne.hhcode",
            "mapper.ne.hhcode can only compare with a LONG.",
        ),
        (
            "'a' mapper.lt.elev",
            "mapper.lt.elev can only compare with a LONG.",
        ),
        (
            "'a' mapper.eq.lat",
            "mapper.eq.lat can only compare with a DOUBLE or a LONG.",
        ),
        (
            "'a' mapper.eq.lon",
            "mapper.eq.lon can only compare with a DOUBLE or a LONG.",
        ),
        (
            "[ ] mapper.eq",
            "mapper.eq can only compare with a STRING, a DOUBLE, a LONG or a BOOLEAN.",
        ),
        (
            "NULL mapper.eq",
            "mapper.eq can only compare with a STRING, a DOUBLE, a LONG or a BOOLEAN.",
        ),
    ] {
        assert_fails(script, message);
    }
}
