// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of `BUCKETIZE`. The expected values were checked against Warp 10 3.5.0
//! (`warp10io/warp10:3.5.0-alpine`).

use crate::tests::{assert_fails, assert_stack, assert_top, exec, test_stack};
use crate::value::Value;

/// A `LONG` series `g{l=v}` with three datapoints, the second one located and elevated.
const SERIES: &str = "NEWGTS 'g' RENAME { 'l' 'v' } RELABEL \
    10 NaN NaN NaN 1 ADDVALUE 20 48.0 2.0 35 2 ADDVALUE 30 NaN NaN 5 3 ADDVALUE";

/// A `LONG` series whose values are their ticks.
const M: &str = "NEWGTS 'g' RENAME 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE \
    5 NaN NaN NaN 5 ADDVALUE 11 NaN NaN NaN 11 ADDVALUE 12 NaN NaN NaN 12 ADDVALUE \
    25 NaN NaN NaN 25 ADDVALUE";

/// `[ ticks values bucketcount bucketspan lastbucket ]` of the series on top of the stack.
const SHAPE: &str = "DUP TICKLIST SWAP DUP VALUES SWAP DUP BUCKETCOUNT SWAP DUP BUCKETSPAN SWAP LASTBUCKET 5 ->LIST";

fn longs(values: &[i64]) -> Value {
    Value::list(values.iter().copied().map(Value::Long).collect())
}

fn doubles(values: &[f64]) -> Value {
    Value::list(values.iter().copied().map(Value::Double).collect())
}

fn shape(ticks: &[i64], values: Value, count: i64, span: i64, last: i64) -> Value {
    Value::list(vec![
        longs(ticks),
        values,
        Value::Long(count),
        Value::Long(span),
        Value::Long(last),
    ])
}

#[test]
fn bucketize_derives_the_missing_parameters() {
    // `lastbucket 0`: the last tick aligned on the span; `bucketcount 0`: as many as needed.
    assert_top(
        &format!("[ {M} bucketizer.sum 0 10 0 ] BUCKETIZE 0 GET {SHAPE}"),
        shape(&[30, 20, 10], longs(&[25, 23, 8]), 3, 10, 30),
    );
    assert_top(
        &format!("[ {M} bucketizer.sum 30 10 0 ] BUCKETIZE 0 GET {SHAPE}"),
        shape(&[30, 20, 10], longs(&[25, 23, 8]), 3, 10, 30),
    );
    assert_top(
        &format!("[ {M} bucketizer.sum 12 10 0 ] BUCKETIZE 0 GET {SHAPE}"),
        shape(&[12, 2], longs(&[28, 3]), 2, 10, 12),
    );
    // `bucketspan 0`: derived from the count (rounded up); `-1`: so that the first tick
    // starts the first bucket.
    assert_top(
        &format!("[ {M} bucketizer.sum 0 0 3 ] BUCKETIZE 0 GET {SHAPE}"),
        shape(&[25, 16, 7], longs(&[25, 23, 8]), 3, 9, 25),
    );
    assert_top(
        &format!("[ {M} bucketizer.sum 0 -1 3 ] BUCKETIZE 0 GET {SHAPE}"),
        shape(&[25, 13, 1], longs(&[25, 30, 1]), 3, 12, 25),
    );
    assert_top(
        &format!("[ {M} bucketizer.sum 0 -1 1 ] BUCKETIZE 0 GET {SHAPE}"),
        shape(&[25], longs(&[55]), 1, 24, 25),
    );
    assert_top(
        &format!("[ {M} bucketizer.sum 0 0 5 ] BUCKETIZE 0 GET {SHAPE}"),
        shape(&[25, 15, 5], longs(&[25, 23, 8]), 5, 5, 25),
    );
    // The alignment of the last bucket may call for one more bucket; duplicate ticks at an
    // explicit last bucket all belong to it.
    assert_top(
        &format!(
            "[ NEWGTS 8 NaN NaN NaN 8 ADDVALUE 15 NaN NaN NaN 15 ADDVALUE bucketizer.sum 0 10 0 ] BUCKETIZE 0 GET {SHAPE}"
        ),
        shape(&[20, 10], longs(&[15, 8]), 2, 10, 20),
    );
    assert_stack(
        "[ NEWGTS 10 NaN NaN NaN 'a' ADDVALUE 20 NaN NaN NaN 'b' ADDVALUE 20 NaN NaN NaN 'c' ADDVALUE 30 NaN NaN NaN 'd' ADDVALUE \
         bucketizer.count 20 10 0 ] BUCKETIZE 0 GET DUP TICKLIST SWAP VALUES",
        &[longs(&[20, 10]), longs(&[2, 1])],
    );
    // Explicit parameters, aligned or not.
    assert_top(
        &format!("[ {M} bucketizer.sum 25 10 2 ] BUCKETIZE 0 GET {SHAPE}"),
        shape(&[25, 15], longs(&[25, 23]), 2, 10, 25),
    );
    assert_top(
        &format!("[ {M} bucketizer.sum 23 10 2 ] BUCKETIZE 0 GET {SHAPE}"),
        shape(&[13], longs(&[28]), 2, 10, 23),
    );
    assert_stack(
        &format!(
            "[ {M} bucketizer.sum 100 10 2 ] BUCKETIZE 0 GET SIZE [ {M} bucketizer.sum -10 10 2 ] BUCKETIZE 0 GET SIZE"
        ),
        &[Value::Long(0), Value::Long(0)],
    );
    // Empty series.
    assert_top("[ NEWGTS bucketizer.sum 0 10 0 ] BUCKETIZE 0 GET SIZE", 0);
    assert_stack(
        "[ NEWGTS bucketizer.sum 0 10 3 ] BUCKETIZE 0 GET DUP BUCKETCOUNT SWAP LASTBUCKET",
        &[Value::Long(3), Value::Long(i64::MIN)],
    );
    assert_fails(
        "[ NEWGTS bucketizer.sum 0 0 3 ] BUCKETIZE",
        "Undefined bucket span, check your GTS timestamps.",
    );
    assert_fails(
        &format!("[ {M} bucketizer.sum 0 0 0 ] BUCKETIZE"),
        "One of bucketspan or bucketcount must be different from zero.",
    );
    assert_fails(
        &format!("[ {M} bucketizer.sum 0 -5 0 ] BUCKETIZE"),
        "/ by zero",
    );
    assert_fails(
        &format!("2 MAXBUCKETS [ {M} bucketizer.sum 0 10 0 ] BUCKETIZE"),
        "Bucket count (3) would exceed maximum value of 2. Consider raising the limit or using capabilities.",
    );
    assert_fails(
        &format!("[ {M} bucketizer.sum 0 1 -1 ] BUCKETIZE"),
        "Bucket count (-1) would exceed maximum value of",
    );
    // The input is sorted in place; `NULL` only sets the parameters of the input itself.
    assert_top(
        &format!(
            "[ {M} 'g' STORE $g RSORT DROP $g bucketizer.sum 0 10 0 ] BUCKETIZE DROP $g TICKLIST"
        ),
        longs(&[1, 2, 5, 11, 12, 25]),
    );
    assert_top(
        &format!("[ {M} NULL 0 10 0 ] BUCKETIZE 0 GET {SHAPE}"),
        shape(
            &[1, 2, 5, 11, 12, 25],
            longs(&[1, 2, 5, 11, 12, 25]),
            3,
            10,
            30,
        ),
    );
    let mut stack = exec(&format!(
        "[ {M} 'g' STORE $g NULL 0 10 0 ] BUCKETIZE 0 GET $g"
    ));
    let original = stack.pop().unwrap();
    assert!(stack.pop().unwrap().same_object(&original));
    // Lists of series, nested or not.
    assert_stack(
        &format!(
            "[ {M} [ {SERIES} ] bucketizer.sum 0 10 0 ] BUCKETIZE SIZE [ [ {SERIES} {M} ] bucketizer.sum 0 10 0 ] BUCKETIZE SIZE"
        ),
        &[Value::Long(2), Value::Long(2)],
    );
    // Any aggregator with the bucketizer role is accepted.
    assert_top(
        &format!("[ {M} mapper.sum 0 10 0 ] BUCKETIZE 0 GET VALUES"),
        longs(&[25, 23, 8]),
    );
    assert_fails("1 BUCKETIZE", "BUCKETIZE expects a list as input.");
    assert_fails(
        &format!("[ {M} bucketizer.sum 0 10 ] BUCKETIZE"),
        "BUCKETIZE needs a list of at least 5 parameters as input.",
    );
    assert_fails(
        "[ 1 bucketizer.sum 0 10 0 ] BUCKETIZE",
        "BUCKETIZE expects a list of Geo Time Series as first parameter.",
    );
    assert_fails(
        "[ [ 1 ] bucketizer.sum 0 10 0 ] BUCKETIZE",
        "BUCKETIZE expects a list of Geo Time Series as first parameter.",
    );
    assert_fails(
        &format!("[ {M} 1 0 10 0 ] BUCKETIZE"),
        "BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
    );
    assert_fails(
        &format!("[ {M} 'x' MATCHER 0 10 0 ] BUCKETIZE"),
        "BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
    );
    assert_fails(
        &format!("[ {M} bucketizer.sum 'x' 10 0 ] BUCKETIZE"),
        "BUCKETIZE expects lastbucket, bucketspan and bucketcount as last 3 parameters.",
    );
    assert_fails("BUCKETIZE", "Empty stack.");
}

#[test]
fn bucketizers_aggregate_each_bucket() {
    assert_top(
        &format!("[ {M} bucketizer.mean 0 10 0 ] BUCKETIZE 0 GET {SHAPE}"),
        shape(
            &[30, 20, 10],
            doubles(&[25.0, 11.5, 2.666_666_666_666_666_5]),
            3,
            10,
            30,
        ),
    );
    assert_stack(
        &format!("[ {M} bucketizer.first 0 10 0 ] BUCKETIZE 0 GET DUP TICKLIST SWAP VALUES"),
        &[longs(&[30, 20, 10]), longs(&[25, 11, 1])],
    );
    assert_stack(
        &format!("[ {M} bucketizer.last 0 10 0 ] BUCKETIZE 0 GET DUP TICKLIST SWAP VALUES"),
        &[longs(&[30, 20, 10]), longs(&[25, 12, 5])],
    );
    assert_stack(
        &format!(
            "[ {M} bucketizer.min 0 10 0 ] BUCKETIZE 0 GET VALUES [ {M} bucketizer.max 0 10 0 ] BUCKETIZE 0 GET VALUES [ {M} bucketizer.count 0 10 0 ] BUCKETIZE 0 GET VALUES"
        ),
        &[longs(&[25, 11, 1]), longs(&[25, 12, 5]), longs(&[1, 2, 3])],
    );
    // Locations and elevations: the most recent datapoint for `sum`, the mean for `mean`,
    // the extremum's for `min` / `max`.
    assert_stack(
        &format!(
            "[ {SERIES} bucketizer.sum 0 10 0 ] BUCKETIZE 0 GET DUP ELEVATIONS SWAP LOCATIONS"
        ),
        &[
            Value::list(vec![
                Value::Long(5),
                Value::Long(35),
                Value::Double(f64::NAN),
            ]),
            doubles(&[f64::NAN, 47.999_999_988_824_13, f64::NAN]),
            doubles(&[f64::NAN, 1.999_999_964_609_742_2, f64::NAN]),
        ],
    );
    assert_stack(
        &format!(
            "[ {SERIES} bucketizer.mean 100 100 1 ] BUCKETIZE 0 GET DUP ELEVATIONS SWAP DUP LOCATIONS 2 ->LIST SWAP VALUES"
        ),
        &[
            Value::list(vec![Value::Long(20)]),
            Value::list(vec![
                doubles(&[47.999_999_988_824_13]),
                doubles(&[1.999_999_964_609_742_2]),
            ]),
            doubles(&[2.0]),
        ],
    );
    assert_stack(
        &format!(
            "[ {SERIES} bucketizer.min 100 100 1 ] BUCKETIZE 0 GET DUP ELEVATIONS SWAP VALUES [ {SERIES} bucketizer.max 100 100 1 ] BUCKETIZE 0 GET DUP ELEVATIONS SWAP VALUES"
        ),
        &[
            Value::list(vec![Value::Double(f64::NAN)]),
            longs(&[1]),
            Value::list(vec![Value::Long(5)]),
            longs(&[3]),
        ],
    );
    // Strings, booleans, doubles, ties (the earliest wins) and mixed numeric types.
    assert_stack(
        "[ NEWGTS 1 NaN NaN NaN 'b' ADDVALUE 2 NaN NaN NaN 'a' ADDVALUE bucketizer.min 0 10 0 ] BUCKETIZE 0 GET VALUES \
         [ NEWGTS 1 NaN NaN NaN 'b' ADDVALUE 2 NaN NaN NaN 'a' ADDVALUE bucketizer.max 0 10 0 ] BUCKETIZE 0 GET VALUES",
        &[
            Value::list(vec![Value::from("a")]),
            Value::list(vec![Value::from("b")]),
        ],
    );
    assert_stack(
        "[ NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE bucketizer.min 0 10 0 ] BUCKETIZE 0 GET DUP TICKLIST SWAP VALUES \
         [ NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE bucketizer.max 0 10 0 ] BUCKETIZE 0 GET VALUES",
        &[
            longs(&[10]),
            Value::list(vec![Value::Boolean(false)]),
            Value::list(vec![Value::Boolean(true)]),
        ],
    );
    assert_stack(
        "[ NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE 3 NaN NaN NaN true ADDVALUE bucketizer.and 0 10 0 ] BUCKETIZE 0 GET VALUES \
         [ NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE bucketizer.or 0 10 0 ] BUCKETIZE 0 GET VALUES \
         [ NEWGTS 1 NaN NaN NaN false ADDVALUE bucketizer.or 0 10 0 ] BUCKETIZE 0 GET VALUES",
        &[
            Value::list(vec![Value::Boolean(false)]),
            Value::list(vec![Value::Boolean(true)]),
            Value::list(vec![Value::Boolean(false)]),
        ],
    );
    assert_stack(
        "[ NEWGTS 1 NaN NaN NaN 2 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 3 NaN NaN NaN 1 ADDVALUE 4 NaN NaN NaN 1 ADDVALUE bucketizer.min 0 10 0 ] BUCKETIZE 0 GET VALUES \
         [ NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE 2 NaN NaN NaN 1.5 ADDVALUE 3 NaN NaN NaN 2 ADDVALUE bucketizer.max 0 10 0 ] BUCKETIZE 0 GET VALUES \
         [ NEWGTS 1 NaN NaN NaN 2.5 ADDVALUE 2 NaN NaN NaN 2.5 ADDVALUE bucketizer.min 0 10 0 ] BUCKETIZE 0 GET VALUES",
        &[longs(&[1]), doubles(&[2.0]), doubles(&[2.5])],
    );
    // Non numeric series produce nothing for `sum` and `mean`.
    assert_stack(
        "[ NEWGTS 1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'b' ADDVALUE bucketizer.sum 0 10 0 ] BUCKETIZE 0 GET SIZE \
         [ NEWGTS 1 NaN NaN NaN 'a' ADDVALUE bucketizer.mean 0 10 0 ] BUCKETIZE 0 GET SIZE",
        &[Value::Long(0), Value::Long(0)],
    );
    assert_stack(
        "[ NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE bucketizer.sum 0 10 0 ] BUCKETIZE 0 GET VALUES \
         [ NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE bucketizer.mean 0 10 0 ] BUCKETIZE 0 GET VALUES",
        &[doubles(&[3.5]), doubles(&[1.75])],
    );
    assert_top("bucketizer.sum TYPEOF", "AGGREGATOR");
    assert_top(
        "[ bucketizer.sum ] SNAPSHOT",
        "[] 'bucketizer.sum' FUNCREF +! ",
    );
    assert_top("bucketizer.sum 'f' STORE $f $f ==", true);
}

#[test]
fn a_macro_bucketizer_returns_a_datapoint() {
    // On the stack: `tick lat lon elevation value`, or as a list.
    assert_top(
        &format!("[ {M} <% TICKLIST 0 GET NaN NaN NaN 'x' %> 0 10 0 ] BUCKETIZE 0 GET {SHAPE}"),
        shape(
            &[30, 20, 10],
            Value::list(vec![Value::from("x"), Value::from("x"), Value::from("x")]),
            3,
            10,
            30,
        ),
    );
    assert_stack(
        &format!(
            "[ {M} <% TICKLIST 0 GET 48.0 2.0 7 'x' 5 ->LIST %> 0 10 0 ] BUCKETIZE 0 GET DUP ELEVATIONS SWAP LOCATIONS"
        ),
        &[
            longs(&[7, 7, 7]),
            doubles(&[47.999_999_988_824_13; 3]),
            doubles(&[1.999_999_964_609_742_2; 3]),
        ],
    );
    assert_top(
        &format!("[ {M} <% VALUES SIZE 0 SWAP 2 ->LIST %> 0 10 0 ] BUCKETIZE 0 GET VALUES"),
        longs(&[1, 2, 3]),
    );
    // The sub-series carries the metadata of the result, and the macro sees each bucket.
    assert_top(
        &format!(
            "[ {SERIES} <% DUP NAME SWAP DUP LABELS 'l' GET SWAP VALUES 0 GET TOSTRING + + 0 SWAP 2 ->LIST %> 0 10 0 ] BUCKETIZE 0 GET VALUES"
        ),
        Value::list(vec![
            Value::from("gv3"),
            Value::from("gv2"),
            Value::from("gv1"),
        ]),
    );
    // `NULL` (alone or last in a list) produces nothing and is left on the stack.
    assert_stack(
        &format!("[ {M} <% DROP NULL %> 0 10 0 ] BUCKETIZE 0 GET SIZE DEPTH"),
        &[
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Long(0),
            Value::Long(4),
        ],
    );
    assert_stack(
        &format!("[ {M} <% DROP [ NULL ] %> 0 10 0 ] BUCKETIZE 0 GET SIZE DEPTH"),
        &[
            Value::list(vec![Value::Null]),
            Value::list(vec![Value::Null]),
            Value::list(vec![Value::Null]),
            Value::Long(0),
            Value::Long(4),
        ],
    );
    assert_stack(
        &format!("[ {M} <% DROP 1 2.0 3.0 4 5 %> 0 10 0 ] BUCKETIZE 0 GET ELEVATIONS"),
        &[longs(&[4, 4, 4])],
    );
    assert_fails(
        &format!("[ {M} <% NAME %> 0 10 0 ] BUCKETIZE"),
        "Empty stack.",
    );
    assert_fails(
        &format!("[ {M} <% DROP 1 2.0 3.0 4.0 5 %> 0 10 0 ] BUCKETIZE"),
        "Macro MUST return an elevations which is NaN if it is of type DOUBLE.",
    );
    assert_fails(
        &format!("[ {M} <% DROP 1.0 2.0 3.0 4 5 %> 0 10 0 ] BUCKETIZE"),
        "Macro MUST return a tick of type LONG.",
    );
    assert_fails(
        &format!("[ {M} <% DROP 1 2 3.0 4 5 %> 0 10 0 ] BUCKETIZE"),
        "Macro MUST return a latitude and a longitude which are of type DOUBLE.",
    );
    assert_fails(
        &format!("[ {M} <% DROP 1 NaN NaN 'e' 5 %> 0 10 0 ] BUCKETIZE"),
        "Macro MUST return an elevation which is either NaN or LONG.",
    );
    assert_fails(
        &format!("[ {M} <% DROP [ 1 2 3 4 5 6 ] %> 0 10 0 ] BUCKETIZE"),
        "Expected a list of 1 to 5 elements, got 6",
    );
    assert_fails(
        &format!("[ {M} <% DROP 'oops' MSGFAIL %> 0 10 0 ] BUCKETIZE"),
        "oops",
    );
    let mut stack = test_stack();
    assert!(
        stack
            .exec_checked(&format!("[ {M} <% DROP %> 0 10 0 ] BUCKETIZE"))
            .is_err()
    );
    // Too few results on the stack, whichever is missing.
    for leftover in ["1", "1 2", "1 2 3", "1 2 3 4"] {
        assert_fails(
            &format!("[ {M} <% DROP {leftover} %> 0 10 0 ] BUCKETIZE"),
            "Empty stack.",
        );
    }
    // A failing aggregator aborts the framework.
    assert_fails(
        &format!("[ {M} test.failing 0 10 0 ] BUCKETIZE"),
        "test.failing failed.",
    );
}

#[test]
fn every_registered_aggregator_can_be_built() {
    for (name, entry) in crate::aggregators::basic::FUNCTIONS {
        let crate::functions::Entry::Object(build) = entry else {
            panic!("{name} is not an object");
        };
        let object = build(name);
        assert_eq!(object.name(), *name);
        assert_eq!(object.type_name(), "AGGREGATOR");
        assert_eq!(
            object.as_aggregator().unwrap().roles(),
            crate::aggregators::Roles::ALL
        );
        assert_top(&format!("{name} TYPEOF"), "AGGREGATOR");
    }
}
