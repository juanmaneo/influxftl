// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The seasonal tests given series that scripts cannot build: a filled series whose ticks
//! are not those of its buckets.

use indexmap::IndexMap;

use crate::error::Error;
use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION};
use crate::stack::Stack;
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

/// The stack after `script`, run over the given arguments (bottom first).
fn run(arguments: Vec<Value>, script: &str) -> Stack {
    let mut stack = test_stack();
    for argument in arguments {
        stack.push(argument);
    }
    stack.exec(script).expect("the script should succeed");
    stack
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

use super::seasonal::stl_residuals;
use crate::error::series::SeriesError;

/// A series of `DOUBLE` values at the ticks `0, 10…`.
fn doubles(values: &[f64]) -> GeoTimeSerie {
    let mut gts = GeoTimeSerie::new();
    for (i, value) in values.iter().enumerate() {
        gts.set_value(
            10 * i as i64,
            NO_LOCATION,
            NO_ELEVATION,
            &Value::Double(*value),
            false,
        );
    }
    gts
}

#[test]
fn residuals_are_less_the_seasonal_and_the_trend() {
    let (gts, seasonal, trend) = (
        doubles(&[1.0, 2.0, 3.0]),
        doubles(&[0.5, 0.25, 0.125]),
        doubles(&[0.25, 0.25, 0.25]),
    );
    assert_eq!(
        stl_residuals(&gts, &seasonal, &trend).expect("residuals"),
        [0.25, 1.5, 2.625]
    );
}

#[test]
fn a_tick_the_seasonal_series_lacks_has_no_residual() {
    let (gts, seasonal) = (doubles(&[1.0, 2.0, 3.0]), doubles(&[0.5, 0.25]));
    assert_eq!(
        stl_residuals(&gts, &seasonal, &seasonal).expect_err("a missing tick"),
        SeriesError::TickNotSeasonal {
            method: "STLESDTest".to_owned(),
            tick: 20
        }
        .into()
    );
}

#[test]
fn a_seasonal_series_longer_than_the_series_is_read_past_its_values() {
    // The seasonal series has the ticks 0 and 10, the series only the value at 10.
    let mut gts = GeoTimeSerie::new();
    gts.set_value(10, NO_LOCATION, NO_ELEVATION, &Value::Double(1.0), false);
    let seasonal = doubles(&[0.5, 0.25]);
    assert_eq!(
        stl_residuals(&gts, &seasonal, &seasonal).expect_err("past the values"),
        Error::index_out_of_bounds(1, 1)
    );
}

/// A series of 24 buckets (a period of 4, a spike), `DOUBLE` values at the ticks `0, 10…230`.
fn seasonal_series() -> Value {
    let mut gts = GeoTimeSerie::with_buckets(230, 24, 10);
    for i in 0..24_i64 {
        let spike = if i == 9 { 12.0 } else { 0.0 };
        let value =
            f64::from([3, 1, 4, 1][(i % 4) as usize]) + 0.1 * f64::from((i % 3) as i32) + spike;
        gts.set_value(
            10 * i,
            NO_LOCATION,
            NO_ELEVATION,
            &Value::Double(value),
            false,
        );
    }
    Value::Gts(gts.shared())
}

/// The map `{ 'PERIOD' period }`.
fn period_map(period: i64) -> Value {
    Value::map(IndexMap::from([(
        Value::from("PERIOD"),
        Value::Long(period),
    )]))
}

#[test]
fn a_period_in_the_map_is_accepted_when_it_is_the_one_of_the_test() {
    // Warp 10 fails on the `Long` of a script (it casts it to the `Integer` it puts itself).
    for (function, arguments) in [
        ("STLESDTEST", vec![Value::Long(4), Value::Long(3)]),
        (
            "HYBRIDTEST",
            vec![Value::Long(4), Value::Long(2), Value::Long(3)],
        ),
    ] {
        let mut all = vec![seasonal_series()];
        all.extend(arguments);
        all.push(period_map(4));
        let mut stack = run(all, function);
        assert!(matches!(stack.pop(), Ok(Value::List(_))), "{function}");
    }
}

#[test]
fn a_period_in_the_map_is_not_another_one_than_the_one_of_the_test() {
    for (function, arguments) in [
        ("STLESDTEST", vec![Value::Long(4), Value::Long(3)]),
        (
            "HYBRIDTEST",
            vec![Value::Long(4), Value::Long(2), Value::Long(3)],
        ),
    ] {
        let mut all = vec![seasonal_series()];
        all.extend(arguments);
        all.push(period_map(5));
        assert_eq!(
            failure(all, function),
            SeriesError::PeriodMismatch.into(),
            "{function}"
        );
    }
}
