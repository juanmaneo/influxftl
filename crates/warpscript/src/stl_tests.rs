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

use crate::error::fault::ResourceError;
use crate::gts::Values;
use crate::stl::Stl;

/// The parameters of a decomposition of a period of 4 buckets with the defaults of `STL`.
fn decomposition() -> Stl {
    Stl {
        period: 4,
        inner: 2,
        outer: 0,
        ns: 7,
        ds: 1,
        js: 0,
        nl: 5,
        dl: 1,
        jl: 0,
        nt: 7,
        dt: 1,
        jt: 0,
        np: 0,
        dp: 2,
        jp: 0,
    }
}

#[test]
fn a_series_with_too_many_missing_values_is_refused() {
    let mut gts = GeoTimeSerie::with_buckets(6_000_000, 600_001, 10);
    for tick in [0, 10, 6_000_000] {
        gts.set_value(tick, NO_LOCATION, NO_ELEVATION, &Value::Double(1.0), false);
    }
    assert_eq!(
        decomposition().decompose(&mut gts).expect_err("too sparse"),
        SeriesError::LowessMissingValues.into()
    );
}

#[test]
fn a_series_without_a_value_has_no_decomposition() {
    // No script makes an empty series of a known type.
    let mut gts = GeoTimeSerie::with_buckets(100, 11, 10);
    gts.values = Values::Double(Vec::new());
    assert_eq!(
        decomposition().decompose(&mut gts).expect_err("no value"),
        SeriesError::StlNoValue("3.0".to_owned()).into()
    );
}

#[test]
fn a_decomposition_has_a_memory_limit() {
    let series = || {
        let mut gts = GeoTimeSerie::with_buckets(100, 11, 10);
        gts.set_value(100, NO_LOCATION, NO_ELEVATION, &Value::Double(1.0), false);
        gts
    };
    let huge = Stl {
        period: i32::MAX,
        ..decomposition()
    };
    assert_eq!(
        huge.decompose(&mut series()).expect_err("too long"),
        ResourceError::ArrayTooLong.into()
    );
    let large = Stl {
        period: 100_000_000,
        ..decomposition()
    };
    assert_eq!(
        large.decompose(&mut series()).expect_err("too large"),
        ResourceError::OutOfMemory.into()
    );
    let wide = Stl {
        ns: i32::MAX,
        ..decomposition()
    };
    assert_eq!(
        wide.decompose(&mut series()).expect_err("too wide"),
        ResourceError::ArrayTooLong.into()
    );
}

/// A bucketized series of 16 values (a period of 4 and a `NaN`), at the ticks `0, 10…150`.
fn series_with_nan() -> GeoTimeSerie {
    let mut gts = GeoTimeSerie::with_buckets(150, 16, 10);
    for i in 0..16 {
        let value = if i == 5 {
            f64::NAN
        } else {
            f64::from([3, 1, 4, 1][i % 4]) + 0.25 * f64::from(i as i32 % 3)
        };
        gts.set_value(
            10 * i as i64,
            NO_LOCATION,
            NO_ELEVATION,
            &Value::Double(value),
            false,
        );
    }
    gts
}

#[test]
fn a_regression_of_a_series_with_a_nan_fails_in_each_smoothing() {
    // Warp 10 loops 2^31 times before it fails on a `NaN` (minutes for each of the cases).
    let message = "illegal state: maximal count (2,147,483,647) exceeded: iterations";
    for (smoothing, params) in [
        (
            "seasonal",
            Stl {
                ds: 2,
                ..decomposition()
            },
        ),
        (
            "low-pass",
            Stl {
                dl: 2,
                ..decomposition()
            },
        ),
        (
            "trend",
            Stl {
                dt: 2,
                ..decomposition()
            },
        ),
        (
            "post seasonal",
            Stl {
                np: 5,
                dp: 2,
                ..decomposition()
            },
        ),
    ] {
        let failure = params
            .decompose(&mut series_with_nan())
            .expect_err("the fit should fail");
        assert!(
            failure.to_string().contains(message),
            "{smoothing}: {failure}"
        );
    }
}

#[test]
fn a_decomposition_has_a_period() {
    assert_eq!(
        Stl {
            period: 0,
            ..decomposition()
        }
        .decompose(&mut series_with_nan())
        .expect_err("no period"),
        Error::division_by_zero()
    );
}

#[test]
fn a_series_that_claims_to_be_sorted_is_not_searched_past_its_order() {
    // Not reachable from WarpScript: the ticks are out of order but flagged as sorted, so that
    // the binary search of the first step misses a tick.
    let mut gts = GeoTimeSerie::with_buckets(150, 16, 10);
    for tick in [
        0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150,
    ] {
        gts.set_value(tick, NO_LOCATION, NO_ELEVATION, &Value::Double(1.0), false);
    }
    gts.ticks.swap(0, 15);
    gts.sorted = true;
    gts.reversed = false;
    assert_eq!(
        decomposition()
            .decompose(&mut gts)
            .expect_err("misses a tick"),
        Error::index_out_of_bounds(-17, 24)
    );
}
