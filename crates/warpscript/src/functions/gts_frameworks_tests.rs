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

/// A `LONG` series with three datapoints at ticks 1, 2 and 5.
const THREE: &str =
    "NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 5 NaN NaN NaN 5 ADDVALUE";

/// `[ ticks values ]` of the first mapped series.
const MAPPED: &str = "MAP 0 GET DUP TICKLIST SWAP VALUES 2 ->LIST";

fn mapped(ticks: &[i64], values: &[i64]) -> Value {
    Value::list(vec![longs(ticks), longs(values)])
}

#[test]
fn map_slides_a_window_over_the_ticks() {
    // Windows in ticks (positive) or in time (negative), before and after each tick.
    assert_top(
        &format!("[ {M} mapper.sum 1 0 0 ] {MAPPED}"),
        mapped(&[1, 2, 5, 11, 12, 25], &[1, 3, 7, 16, 23, 37]),
    );
    assert_top(
        &format!("[ {M} mapper.sum 0 1 0 ] MAP 0 GET VALUES"),
        longs(&[3, 7, 16, 23, 37, 25]),
    );
    assert_top(
        &format!("[ {M} mapper.sum -3 0 0 ] MAP 0 GET VALUES"),
        longs(&[1, 3, 7, 11, 23, 25]),
    );
    assert_top(
        &format!("[ {M} mapper.sum 0 -3 0 ] MAP 0 GET VALUES"),
        longs(&[3, 7, 5, 23, 12, 25]),
    );
    // Occurrences: that many applications from the first tick, from the last when negative
    // (the result is then sorted backwards); every `step` ticks.
    assert_top(
        &format!("[ {M} mapper.sum 1 0 2 ] {MAPPED}"),
        mapped(&[1, 2], &[1, 3]),
    );
    assert_top(
        &format!("[ {M} mapper.sum 1 0 -2 ] {MAPPED}"),
        mapped(&[25, 12], &[37, 23]),
    );
    assert_top(
        &format!("[ {M} mapper.sum 1 0 0 2 ] {MAPPED}"),
        mapped(&[1, 5, 12], &[1, 7, 23]),
    );
    assert_top(
        &format!("[ {M} mapper.sum 1 0 -2 2 ] {MAPPED}"),
        mapped(&[25, 11], &[37, 16]),
    );
    // Backwards, the post window reaches the ticks mapped before.
    assert_top(
        &format!("[ {THREE} mapper.sum 0 1 -2 ] {MAPPED}"),
        mapped(&[5, 2], &[5, 7]),
    );
    assert_top(
        &format!("[ {THREE} mapper.sum 0 2 -3 ] {MAPPED}"),
        mapped(&[5, 2, 1], &[5, 7, 8]),
    );
    assert_top(
        &format!("[ {THREE} mapper.sum 1 1 -3 ] {MAPPED}"),
        mapped(&[5, 2, 1], &[7, 8, 3]),
    );
    assert_top(
        &format!("[ {M} mapper.sum 0 0 -9223372036854775808 ] {MAPPED}"),
        mapped(&[25, 12, 11, 5, 2, 1], &[25, 12, 11, 5, 2, 1]),
    );
    // Repeated ticks map one datapoint each, over the same window.
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE 1 NaN NaN NaN 2 ADDVALUE 1 NaN NaN NaN 3 ADDVALUE mapper.sum 0 0 0 ] MAP 0 GET DUP TICKLIST SWAP VALUES 2 ->LIST",
        mapped(&[1, 1, 1], &[6, 6, 6]),
    );
    // Windows larger than `Integer.MAX_VALUE` are capped, the step is an `int`.
    assert_top(
        &format!("[ {THREE} mapper.sum 99999999999 0 0 ] MAP 0 GET VALUES"),
        longs(&[1, 3, 8]),
    );
    assert_top(
        &format!("[ {THREE} mapper.sum 1 0 0 9999999999 ] MAP 0 GET VALUES"),
        longs(&[1]),
    );
    assert_top(
        &format!("[ {THREE} mapper.sum 1 0 0 4294967297 ] MAP 0 GET VALUES"),
        longs(&[1, 3, 7]),
    );
    // The shared aggregators map whatever their registered family.
    assert_top(
        &format!("[ {THREE} bucketizer.sum 1 0 0 ] MAP 0 GET VALUES"),
        longs(&[1, 3, 7]),
    );
    assert_top(
        &format!("[ {THREE} reducer.sum 1 0 0 ] MAP 0 GET VALUES"),
        longs(&[1, 3, 7]),
    );
    // The input series is sorted in place, the result keeps the name and labels.
    assert_stack(
        &format!(
            "{SERIES} 'g' STORE [ $g mapper.sum 0 0 0 ] MAP 0 GET DUP NAME SWAP LABELS $g TICKLIST"
        ),
        &[
            Value::from("g"),
            Value::map(indexmap::IndexMap::from([(
                Value::from("l"),
                Value::from("v"),
            )])),
            longs(&[10, 20, 30]),
        ],
    );
    assert_top(
        "NEWGTS 2 NaN NaN NaN 2 ADDVALUE 1 NaN NaN NaN 1 ADDVALUE 'g' STORE [ $g mapper.sum 0 0 -1 ] MAP DROP $g TICKLIST",
        longs(&[1, 2]),
    );
    assert_top("[ NEWGTS mapper.sum 0 0 0 ] MAP 0 GET SIZE", 0);
    // Series and lists of series, flattened in the result.
    assert_top(
        &format!("[ {THREE} [ {THREE} ] mapper.sum 1 0 0 ] MAP SIZE"),
        2,
    );
    assert_top(
        &format!("[ [ {THREE} ] [ {THREE} ] ] {{ 'mapper' mapper.sum }} MAP SIZE"),
        2,
    );
}

#[test]
fn map_at_output_ticks() {
    for (params, ticks, values) in [
        ("1 0 [ 3 10 ]", vec![3, 10], vec![2, 5]),
        ("1 0 [ 10 3 ]", vec![10, 3], vec![5, 2]),
        ("0 1 [ 2 3 ]", vec![2, 3], vec![7, 5]),
        ("0 1 [ 5 2 ]", vec![5, 2], vec![5, 7]),
        ("1 1 [ 3 1 ]", vec![3, 1], vec![7, 3]),
        ("1 0 [ 3 2 1 ]", vec![3, 2, 1], vec![2, 3, 1]),
        ("1 0 [ 2 ]", vec![2], vec![3]),
        ("1 0 [ 2 2 ]", vec![2, 2], vec![3, 3]),
    ] {
        assert_top(
            &format!("[ {THREE} mapper.sum {params} ] {MAPPED}"),
            mapped(&ticks, &values),
        );
    }
    assert_top(&format!("[ {THREE} mapper.sum 1 0 [ ] ] MAP 0 GET SIZE"), 0);
    for ticks in ["[ 1 3 2 ]", "[ 1 'a' ]", "[ 3 1 2 ]"] {
        assert_fails(
            &format!("[ {THREE} mapper.sum 1 0 {ticks} ] MAP"),
            "MAP expects the occurrences parameter to be a LONG or a sorted LIST of LONG.",
        );
    }
}

#[test]
fn map_on_a_bucketized_series() {
    let bucketized = format!("[ {M} bucketizer.sum 0 10 0 ] BUCKETIZE 0 GET");
    assert_stack(
        &format!(
            "[ {bucketized} mapper.sum 1 0 0 ] MAP 0 GET DUP TICKLIST SWAP DUP VALUES SWAP BUCKETCOUNT"
        ),
        &[longs(&[10, 20, 30]), longs(&[8, 31, 48]), Value::Long(3)],
    );
    // A window wider than the bucket count reaches the beginning of the series.
    assert_top(
        &format!("[ {bucketized} mapper.sum 5 0 0 ] MAP 0 GET VALUES"),
        longs(&[8, 31, 56]),
    );
    assert_top(
        &format!("[ {bucketized} mapper.sum 1 0 -2 ] {MAPPED}"),
        mapped(&[30, 20], &[48, 31]),
    );
    // Output ticks unbucketize the result.
    assert_stack(
        &format!(
            "[ {bucketized} mapper.sum 0 5 [ 5 15 ] ] MAP 0 GET DUP TICKLIST SWAP DUP VALUES SWAP BUCKETCOUNT"
        ),
        &[longs(&[5, 15]), longs(&[56, 48]), Value::Long(0)],
    );
    assert_top(
        &format!("[ {bucketized} mapper.sum 1 1 [ 15 5 ] ] {MAPPED}"),
        mapped(&[15, 5], &[31, 8]),
    );
    // Named outputs keep the bucketization, unless output ticks were given.
    assert_stack(
        &format!(
            "[ {bucketized} <% DROP {{ 'a' [ 5 ] }} %> 1 0 0 ] MAP 0 GET DUP BUCKETCOUNT SWAP DUP TICKLIST SWAP VALUES"
        ),
        &[Value::Long(3), longs(&[10, 20, 30]), longs(&[5, 5, 5])],
    );
    assert_stack(
        &format!(
            "[ {bucketized} <% DROP {{ 'a' [ 5 ] }} %> 1 0 [ 20 ] ] MAP 0 GET DUP BUCKETCOUNT SWAP DUP TICKLIST SWAP VALUES"
        ),
        &[Value::Long(0), longs(&[20]), longs(&[5])],
    );
    // An empty bucketized series: one empty window per bucket.
    assert_stack(
        "[ [ NEWGTS bucketizer.sum 100 10 3 ] BUCKETIZE 0 GET mapper.count 0 0 0 ] MAP 0 GET DUP TICKLIST SWAP VALUES",
        &[longs(&[]), longs(&[])],
    );
}

#[test]
fn map_with_a_macro() {
    // The macro receives each window as a series carrying the result's metadata and returns
    // `tick lat lon elevation value`, a datapoint list, or named lists.
    assert_top(
        &format!("[ {THREE} <% 'x' STORE $x LASTTICK NaN NaN NaN $x SIZE %> 1 0 0 ] {MAPPED}"),
        mapped(&[1, 2, 5], &[1, 2, 2]),
    );
    assert_top(
        &format!("[ {THREE} <% 'x' STORE [ $x SIZE ] %> 1 0 0 ] {MAPPED}"),
        mapped(&[1, 2, 5], &[1, 2, 2]),
    );
    assert_stack(
        &format!(
            "{SERIES} {{ 'at' 'tr' }} SETATTRIBUTES 'g' STORE [ $g <% 'x' STORE [ $x NAME $x LABELS ->JSON + $x ATTRIBUTES ->JSON + ] %> 1 0 0 ] MAP 0 GET DUP VALUES SWAP NAME"
        ),
        &[
            Value::list(vec![Value::from("g{\"l\":\"v\"}{\"at\":\"tr\"}"); 3]),
            Value::from("g"),
        ],
    );
    // The tick override takes the returned tick (duplicates included).
    assert_top(
        &format!("[ {THREE} <% 'x' STORE [ 100 $x SIZE ] %> 1 0 0 1 true ] {MAPPED}"),
        mapped(&[100, 100, 100], &[1, 2, 2]),
    );
    assert_top(
        &format!("[ {THREE} <% 'x' STORE [ 100 $x SIZE ] %> 1 0 0 1 false ] {MAPPED}"),
        mapped(&[1, 2, 5], &[1, 2, 2]),
    );
    // A single-element list has an `Integer` tick, which the override cannot cast.
    for result in ["[ $x SIZE ]", "{ 'a' [ $x SIZE ] }"] {
        assert_fails(
            &format!("[ {THREE} <% 'x' STORE {result} %> 1 0 0 1 true ] MAP"),
            "class java.lang.Integer cannot be cast to class java.lang.Long",
        );
    }
    assert_top(
        &format!("[ {THREE} <% 'x' STORE {{ 'a' [ 100 $x SIZE ] }} %> 1 0 0 1 true ] {MAPPED}"),
        mapped(&[100, 100, 100], &[1, 2, 2]),
    );
    // Named outputs: one series per key, sorted by name, nested when there are several.
    assert_stack(
        &format!(
            "[ {THREE} <% 'x' STORE {{ 'a' [ $x SIZE ] 'b' [ 7 ] }} %> 1 0 0 ] MAP 0 GET DUP 0 GET DUP NAME SWAP VALUES ROT 1 GET DUP NAME SWAP VALUES"
        ),
        &[
            Value::from("a"),
            longs(&[1, 2, 2]),
            Value::from("b"),
            longs(&[7, 7, 7]),
        ],
    );
    assert_stack(
        &format!(
            "[ {THREE} <% 'x' STORE {{ 'a' [ 1 ] }} %> 1 0 0 ] MAP 0 GET DUP NAME SWAP DUP TICKLIST SWAP VALUES"
        ),
        &[Value::from("a"), longs(&[1, 2, 5]), longs(&[1, 1, 1])],
    );
    // Once a map is returned, the plain results of the other ticks are dropped.
    assert_stack(
        &format!(
            "[ {THREE} <% 'x' STORE $x SIZE 1 == <% {{ 'a' [ 1 ] }} %> <% [ 2 ] %> IFTE %> 1 0 0 ] MAP 0 GET DUP NAME SWAP DUP TICKLIST SWAP VALUES"
        ),
        &[Value::from("a"), longs(&[1]), longs(&[1])],
    );
    assert_stack(
        &format!(
            "[ {THREE} <% 'x' STORE $x SIZE 1 == <% {{ 'a' [ 1 ] }} %> <% {{ 'b' [ 2 ] 'a' [ 3 ] }} %> IFTE %> 1 0 0 ] MAP 0 GET DUP 0 GET DUP NAME SWAP VALUES ROT 1 GET DUP NAME SWAP DUP TICKLIST SWAP VALUES"
        ),
        &[
            Value::from("a"),
            longs(&[1, 3, 3]),
            Value::from("b"),
            longs(&[2, 5]),
            longs(&[2, 2]),
        ],
    );
    assert_top(
        &format!("[ {THREE} <% DROP {{}} %> 1 0 0 ] MAP"),
        Value::list(vec![]),
    );
    assert_top(
        &format!("[ {THREE} <% DROP {{ 1 [ 5 ] }} %> 1 0 0 ] MAP 0 GET NAME"),
        "1",
    );
    // `null` values produce nothing; a list on top is the result, whatever lies below.
    assert_top(
        &format!("[ {THREE} <% DROP [ NULL ] %> 1 0 0 ] MAP 0 GET SIZE"),
        0,
    );
    assert_stack(
        &format!(
            "[ {THREE} <% DROP {{ 'a' [ NULL ] 'b' [ 1 ] }} %> 1 0 0 ] MAP 0 GET DUP 0 GET DUP NAME SWAP SIZE ROT 1 GET SIZE"
        ),
        &[Value::from("a"), Value::Long(0), Value::Long(3)],
    );
    assert_top(
        &format!("[ {THREE} <% DROP 7 NaN NaN NaN NULL %> 1 0 0 ] MAP 0 GET SIZE"),
        0,
    );
    assert_stack(
        &format!("[ {THREE} <% DROP 7 NaN NaN NaN [ 1 ] %> 1 0 0 ] MAP 0 GET SIZE DEPTH"),
        &[
            Value::Long(7),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Long(7),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Long(7),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Long(3),
            Value::Long(13),
        ],
    );
    // Java runtime exceptions escape `MAP` unwrapped (the interpreter still locates the
    // statement), WarpScript ones are wrapped, STOP stops.
    for (mapper, class, message) in [
        (
            "<% 'x' STORE $x SIZE %>",
            "io.warp10.script.InformativeEmptyStackException",
            "Empty stack.",
        ),
        (
            "<% DROP { 'a' 1 } %>",
            "java.lang.ClassCastException",
            "class java.lang.Long cannot be cast to class java.util.List",
        ),
        (
            "<% DROP [ 'x' 1 ] %>",
            "java.lang.ClassCastException",
            "class java.lang.String cannot be cast to class java.lang.Number",
        ),
        (
            "<% DROP 'boom' MSGFAIL %>",
            "io.warp10.script.WarpScriptException",
            "MAP was given invalid parameters. (Exception at 'MSGFAIL' in section '[TOP]' (MSGFAIL boom))",
        ),
        (
            "<% DROP %>",
            "io.warp10.script.InformativeEmptyStackException",
            "Empty stack.",
        ),
        (
            "<% DROP [ 1 2 3 4 5 6 ] %>",
            "io.warp10.script.WarpScriptException",
            "MAP was given invalid parameters. (Expected a list of 1 to 5 elements, got 6)",
        ),
        (
            "<% DROP { 'a' [ 1 2 3 4 5 6 ] } %>",
            "io.warp10.script.WarpScriptException",
            "MAP was given invalid parameters. (Expected a list of 1 to 5 elements, got 6)",
        ),
        (
            "test.failing",
            "io.warp10.script.WarpScriptException",
            "MAP was given invalid parameters. (test.failing failed.)",
        ),
    ] {
        let script = format!("[ {THREE} {mapper} 1 0 0 ] MAP");
        let err = crate::tests::fail(&script);
        let text = err.to_string();
        assert!(
            text.starts_with("Line #1 (Exception at '")
                && text.ends_with(&format!("=>MAP<=' in section [TOP] ({message}))")),
            "{text}\n{script}"
        );
        assert_top(
            &format!("<% {script} %> <% ERROR 1 GET 'type' GET %> <% %> TRY"),
            format!("class {class}"),
        );
    }
    assert_stack(&format!("[ {THREE} <% DROP STOP %> 1 0 0 ] MAP"), &[]);
    // `ERROR` reports the Java class of a runtime exception.
    assert_top(
        "<% [ NEWGTS 1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'b' ADDVALUE bucketizer.median 0 10 0 ] BUCKETIZE %> <% ERROR 1 GET 'type' GET %> <% %> TRY",
        "class java.lang.RuntimeException",
    );
}

#[test]
fn map_checks_its_parameters() {
    // The list form.
    assert_fails(
        "'x' MAP",
        "MAP expects a list as input or a map of parameters on top of a list of GTS.",
    );
    assert_fails(
        "[ mapper.sum 1 0 0 ] MAP",
        "MAP expects Geo Time Series or lists thereof as first parameters.",
    );
    for params in ["", "'x' 1 0 0", "50 bucketizer.percentile TYPEOF 1 0 0"] {
        assert_fails(
            &format!("[ {THREE} {params} ] MAP"),
            "MAP expects a mapper function or a macro after Geo Time Series.",
        );
    }
    for windows in ["'x' 0", "1 'x'"] {
        assert_fails(
            &format!("[ {THREE} mapper.sum {windows} 0 ] MAP"),
            "MAP expects prewindow and postwindow as 2 parameters following the mapper function.",
        );
    }
    assert_fails(
        &format!("[ {THREE} mapper.sum 1 0 'x' ] MAP"),
        "MAP expects occurrences (LONG or LIST of LONG) as the parameter following the postwindow parameter.",
    );
    assert_fails(
        &format!("[ {THREE} mapper.sum 1 0 0 'x' ] MAP"),
        "MAP expects a step parameter that is an integer number.",
    );
    for step in ["0", "2147483648"] {
        assert_fails(
            &format!("[ {THREE} mapper.sum 1 0 0 {step} ] MAP"),
            "MAP expects a step parameter which is strictly positive.",
        );
    }
    assert_fails(
        &format!("[ {THREE} mapper.sum 1 0 0 1 'x' ] MAP"),
        "MAP expects a boolean as 'override tick' parameter.",
    );
    assert_fails(
        &format!("[ {THREE} [ 'x' ] mapper.sum 1 0 0 ] MAP"),
        "MAP operates on lists of Geo Time Series.",
    );
    // The missing windows and occurrences are added to the parameter list itself.
    assert_stack(
        &format!("[ {THREE} mapper.sum ] 'p' STORE $p MAP 0 GET VALUES $p SIZE"),
        &[longs(&[1, 2, 5]), Value::Long(5)],
    );
    // The map form.
    assert_top(
        &format!("{THREE} {{ 'mapper' mapper.sum 'pre' 1 }} MAP 0 GET VALUES"),
        longs(&[1, 3, 7]),
    );
    assert_top(
        &format!("[ {THREE} ] {{ 'mapper' mapper.sum 'pre' 1 'occurrences' -1 }} {MAPPED}"),
        mapped(&[5], &[7]),
    );
    assert_top(
        &format!(
            "{THREE} {{ 'mapper' mapper.sum 'post' 1 'step' 2 'override' false }} MAP 0 GET VALUES"
        ),
        longs(&[3, 5]),
    );
    // A non positive step runs the mapper at every tick, a huge one once.
    for (step, values) in [
        ("0", vec![1, 2, 5]),
        ("-1", vec![1, 2, 5]),
        ("99999999999", vec![1]),
    ] {
        assert_top(
            &format!("{THREE} {{ 'mapper' mapper.sum 'step' {step} }} MAP 0 GET VALUES"),
            longs(&values),
        );
    }
    assert_fails(
        "NEWGTS { 'mapper' mapper.sum 'occurences' 1 } MAP",
        "MAP was given a deprecated parameter with a typo: occurences. Use instead parameter occurrences",
    );
    assert_fails(
        "NEWGTS { 'mapper' mapper.sum 'ticks' 1 } MAP",
        "MAP was given a deprecated parameter: ticks. Consider using occurrences instead (behaviour is slightly different - see documentation)",
    );
    assert_fails("NEWGTS { 'pre' 1 } MAP", "MAP Missing 'mapper' parameter.");
    assert_fails(
        "NEWGTS { 'mapper' mapper.sum 'pre' 'x' } MAP",
        "MAP expects the pre parameter to be a LONG.",
    );
    assert_fails(
        "NEWGTS { 'mapper' mapper.sum 'post' NULL } MAP",
        "MAP expects the post parameter to be a LONG.",
    );
    assert_fails(
        "NEWGTS { 'mapper' mapper.sum 'occurrences' 'x' } MAP",
        "MAP expects the occurrences parameter to be a LONG or a sorted LIST of LONG.",
    );
    assert_fails(
        "NEWGTS { 'mapper' mapper.sum 'step' 'x' } MAP",
        "MAP expects the step parameter to be a LONG.",
    );
    assert_fails(
        "NEWGTS { 'mapper' mapper.sum 'override' 'x' } MAP",
        "MAP expects the override parameter to be a BOOLEAN.",
    );
    // The mapper is only checked when there is a tick to map.
    assert_fails(
        &format!("{THREE} {{ 'mapper' 'x' }} MAP"),
        "MAP was given invalid parameters. (Expected a mapper function.)",
    );
    assert_fails(
        &format!("{THREE} {{ 'mapper' NULL }} MAP"),
        "MAP was given invalid parameters. (Expected a mapper function.)",
    );
    assert_top("NEWGTS { 'mapper' 'x' } MAP 0 GET SIZE", 0);
    for series in ["'x'", "[ 'x' ]", "[ [ 'x' ] ]"] {
        assert_fails(
            &format!("{series} {{ 'mapper' mapper.sum }} MAP"),
            "MAP operates on lists of Geo Time Series.",
        );
    }
    assert_fails("MAP", "Empty stack.");
    assert_fails("{ 'mapper' mapper.sum } MAP", "Empty stack.");
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
