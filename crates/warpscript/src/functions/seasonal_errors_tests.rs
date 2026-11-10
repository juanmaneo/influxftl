// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The seasonal tests given series that scripts cannot build: a filled series whose ticks
//! are not those of its buckets.

use crate::error::Error;
use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION};
use crate::tests::test_stack;
use crate::value::Value;

/// A series of 10 buckets of 10 ticks ending at 100, one `DOUBLE` value at each given tick.
fn series(ticks: impl Iterator<Item = i64>) -> Value {
    let mut gts = GeoTimeSerie::with_buckets(100, 10, 10);
    for tick in ticks {
        gts.set_value(
            tick,
            NO_LOCATION,
            NO_ELEVATION,
            &Value::Double(tick as f64),
            false,
        );
    }
    Value::Gts(gts.shared())
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
fn a_piece_without_value_has_no_median() {
    // 2 pieces of 4 buckets cover the ticks 30 to 100: all the values are before.
    assert_eq!(
        failure(
            vec![
                series(1..=10),
                Value::Long(2),
                Value::Long(2),
                Value::Long(1)
            ],
            "HYBRIDTEST2"
        ),
        Error::index_out_of_bounds(0, 0)
    );
}
