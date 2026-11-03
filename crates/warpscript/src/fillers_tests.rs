// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the fillers that `functions::gts_fill_tests` (generated from the answers of
//! Warp 10 3.5.0) cannot express: the runaway fills of macro fillers, which Warp 10 stops at
//! its default limits.

use crate::fillers::FillerRef;
use crate::tests::{assert_fails, top};
use crate::value::Value;

const G: &str = "NEWGTS 'c' RENAME 30 NaN NaN NaN 3 ADDVALUE 50 48.0 -4.5 100 5 ADDVALUE \
    80 NaN NaN 200 8 ADDVALUE [ SWAP bucketizer.last 100 10 10 ] BUCKETIZE 0 GET";
const U: &str = "NEWGTS 'u' RENAME 10 NaN NaN NaN 1.0 ADDVALUE 20 1.0 1.0 7 2.0 ADDVALUE \
    40 2.0 3.0 9 4.0 ADDVALUE 70 NaN NaN NaN 1.0 ADDVALUE";
const V: &str = "NEWGTS 'v' RENAME 5 NaN NaN NaN 1.0 ADDVALUE 20 NaN NaN NaN 5.0 ADDVALUE \
    30 NaN NaN NaN 6.0 ADDVALUE";

/// A macro filler adds its datapoints to the series it fills, which the bucket and cross
/// fills keep reading: they only stop at a limit (like Warp 10's default 100000 operations).
#[test]
fn macro_fillers_run_away() {
    for script in [
        format!("1000 MAXOPS [ {G} <% 'tick' STORE 'g' STORE $tick 1000 * %> ] FILL"),
        format!("1000 MAXOPS {G} 'orig' STORE [ $orig <% SWAP DROP 2 * %> ] FILL"),
        format!("1000 MAXOPS {U} {V} <% SWAP NAME SWAP 2 ->LIST ' ' JOIN %> FILL"),
    ] {
        assert_fails(&script, "exceeded maximum of 1000.");
    }
}

/// Only filler objects and macros are fillers.
#[test]
fn filler_ref() {
    assert!(FillerRef::of(&Value::Long(1)).is_none());
    assert!(FillerRef::of(&top("mapper.max")).is_none());
    assert!(FillerRef::of(&top("filler.next")).is_some());
    assert!(FillerRef::of(&top("<% %>")).is_some());
    let Value::Function(filler) = top("filler.next") else {
        panic!("not a function");
    };
    assert_eq!(filler.name(), "filler.next");
}
