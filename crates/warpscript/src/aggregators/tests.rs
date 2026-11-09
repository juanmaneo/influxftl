// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Shared helpers of the aggregator tests: windows `BUCKETIZE` cannot produce (`null`
//! values, mixed value types, empty windows) applied to an aggregator directly.

use indexmap::IndexMap;

use super::{Aggregated, Aggregator, AggregatorArgs};
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::value::Value;

/// A window of datapoints to apply an aggregator to.
pub(super) struct Window {
    pub ticks: Vec<i64>,
    pub locations: Vec<i64>,
    pub elevations: Vec<i64>,
    pub values: Vec<Value>,
}

impl Window {
    /// Datapoints at ticks `1, 2, …` without location nor elevation.
    pub(super) fn of(values: &[Value]) -> Window {
        Window {
            ticks: (1..=values.len() as i64).collect(),
            locations: vec![NO_LOCATION; values.len()],
            elevations: vec![NO_ELEVATION; values.len()],
            values: values.to_vec(),
        }
    }

    /// Apply `aggregator` to the window, for tick `100`.
    pub(super) fn apply(&self, aggregator: &dyn Aggregator) -> crate::error::Result<Aggregated> {
        let names = ["g".to_owned()];
        let labels = [IndexMap::new()];
        aggregator.apply(&AggregatorArgs {
            tick: 100,
            names: &names,
            labels: &labels,
            ticks: &self.ticks,
            locations: &self.locations,
            elevations: &self.elevations,
            values: &self.values,
            window: None,
        })
    }

    /// The value `aggregator` produces on the window.
    pub(super) fn value(&self, aggregator: &dyn Aggregator) -> Value {
        self.apply(aggregator).expect("apply should succeed").value
    }

    /// The error `aggregator` produces on the window.
    pub(super) fn error(&self, aggregator: &dyn Aggregator) -> String {
        self.apply(aggregator)
            .expect_err("apply should fail")
            .to_string()
    }
}

/// `values` as `LONG`s.
pub(super) fn longs(values: &[i64]) -> Vec<Value> {
    values.iter().copied().map(Value::Long).collect()
}

/// `values` as `DOUBLE`s.
pub(super) fn doubles(values: &[f64]) -> Vec<Value> {
    values.iter().copied().map(Value::Double).collect()
}

/// A series literal with one datapoint per value, at ticks `1, 2, …`.
pub(super) fn series(values: &str) -> String {
    let mut script = String::from("NEWGTS");
    for (i, value) in values.split_whitespace().enumerate() {
        script.push_str(&format!(" {} NaN NaN NaN {value} ADDVALUE", i + 1));
    }
    script
}

/// `[ series bucketizer 0 10 0 ] BUCKETIZE 0 GET`: the series bucketized as a whole (all the
/// ticks are below 10) by `bucketizer`.
pub(super) fn bucketized(values: &str, bucketizer: &str) -> String {
    format!("[ {} {bucketizer} 0 10 0 ] BUCKETIZE 0 GET", series(values))
}

/// Every registered name of the parameterized aggregator tables produces an aggregator of
/// that name, through its builder when it has one.
#[test]
fn every_registered_name_builds_an_aggregator() {
    use crate::functions::Entry;
    use crate::tests::{assert_top, test_stack};

    for table in [
        super::stats::FUNCTIONS,
        super::order::FUNCTIONS,
        super::join::FUNCTIONS,
    ] {
        for (name, entry) in table {
            let parameter = if name.contains(".var") || name.contains(".sd") {
                "true"
            } else if name.contains(".percentile") {
                "50"
            } else if name.contains(".join") {
                "'-'"
            } else if name.contains(".circular") {
                "360"
            } else {
                ""
            };
            let object = match entry {
                Entry::Object(build) => build(name),
                Entry::Builder(build) => {
                    let mut stack = test_stack();
                    stack
                        .exec_checked(parameter)
                        .expect("exec_checked should succeed");
                    build(&mut stack, name).expect("build should succeed");
                    let Value::Function(object) = stack.pop().expect("pop should succeed") else {
                        panic!("{name} did not build a function object");
                    };
                    object
                }
                Entry::Stack(_) => panic!("{name} is a plain stack function"),
            };
            assert_eq!(object.name(), *name);
            assert_eq!(object.type_name(), "AGGREGATOR");
            assert_eq!(
                object
                    .as_aggregator()
                    .expect("as_aggregator should succeed")
                    .roles(),
                super::Roles::ALL
            );
            assert_top(&format!("{parameter} {name} TYPEOF"), "AGGREGATOR");
            if parameter.is_empty() {
                assert_top(&format!("{name} SNAPSHOT"), format!("'{name}' FUNCREF "));
            } else {
                let rendered = if parameter == "50" {
                    "50.0 'type1'"
                } else if parameter == "360" {
                    "360.0"
                } else {
                    parameter
                };
                assert_top(
                    &format!("{parameter} {name} SNAPSHOT"),
                    format!("{rendered} {name} "),
                );
            }
        }
    }
}
