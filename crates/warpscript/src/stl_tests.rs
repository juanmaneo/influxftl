// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the cycle sub-series, on the arguments scripts cannot give.

use crate::error::Error;
use crate::error::series::SeriesError;
use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION};
use crate::stl::{int_div, sub_cycle_serie};
use crate::value::Value;

/// A series of buckets `0, 10…100` (last bucket 100, 11 buckets) holding a value of its
/// tick over ten, at the given ticks.
fn bucketized(ticks: &[i64]) -> GeoTimeSerie {
    let mut gts = GeoTimeSerie::with_buckets(100, 11, 10);
    for &tick in ticks {
        gts.set_value(
            tick,
            NO_LOCATION,
            NO_ELEVATION,
            &Value::Double(tick as f64 / 10.0),
            false,
        );
    }
    gts
}

#[test]
fn integer_division_is_javas() {
    assert_eq!(int_div(7, 2).expect("a quotient"), 3);
    assert_eq!(int_div(-7, 2).expect("a quotient"), -3);
    assert_eq!(int_div(i32::MIN, -1).expect("a quotient"), i32::MIN);
    assert_eq!(int_div(1, 0), Err(Error::division_by_zero()));
}

#[test]
fn a_cycle_goes_back_from_the_last_bucket_by_periods() {
    let mut gts = bucketized(&[0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100]);
    let cycle = sub_cycle_serie(&mut gts, 90, 3).expect("a cycle");
    assert_eq!(cycle.ticks, [90, 60, 30, 0]);
    assert_eq!(cycle.values.doubles(), [9.0, 6.0, 3.0, 0.0]);
    assert_eq!(
        (cycle.lastbucket, cycle.bucketcount, cycle.bucketspan),
        (90, 4, 30)
    );
}

#[test]
fn a_cycle_skips_the_buckets_without_a_value() {
    let mut gts = bucketized(&[100, 70, 40, 10]);
    let cycle = sub_cycle_serie(&mut gts, 100, 3).expect("a cycle");
    assert_eq!(cycle.ticks, [100, 70, 40, 10]);
    let cycle = sub_cycle_serie(&mut gts, 90, 3).expect("a cycle");
    assert_eq!(cycle.size(), 0);
}

#[test]
fn a_cycle_of_no_value_is_empty() {
    let mut gts = bucketized(&[]);
    let cycle = sub_cycle_serie(&mut gts, 100, 3).expect("a cycle");
    assert_eq!(cycle.size(), 0);
    assert_eq!(
        (cycle.lastbucket, cycle.bucketcount, cycle.bucketspan),
        (100, 4, 30)
    );
}

#[test]
fn a_cycle_ending_at_the_origin_is_not_bucketized() {
    // A last bucket of 0 does not make a bucketized series: no datapoint is searched.
    let mut gts = bucketized(&[0, 10, 20]);
    let cycle = sub_cycle_serie(&mut gts, 0, 3).expect("a cycle");
    assert_eq!(cycle.size(), 0);
}

#[test]
fn a_series_that_is_not_bucketized_has_no_cycle() {
    let mut gts = GeoTimeSerie::new();
    assert_eq!(
        sub_cycle_serie(&mut gts, 100, 3).expect_err("no cycle"),
        SeriesError::NotBucketized.into()
    );
}

#[test]
fn a_cycle_starts_on_a_bucket() {
    let mut gts = bucketized(&[0, 10]);
    assert_eq!(
        sub_cycle_serie(&mut gts, 95, 3).expect_err("no cycle"),
        SeriesError::CycleOffBucket.into()
    );
}

#[test]
fn a_cycle_has_buckets() {
    let mut gts = bucketized(&[0, 10]);
    assert_eq!(
        sub_cycle_serie(&mut gts, 100, 0).expect_err("no cycle"),
        Error::division_by_zero()
    );
}
