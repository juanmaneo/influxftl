// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the order statistics aggregators. The expected values were checked against
//! Warp 10 3.5.0 (`warp10io/warp10:3.5.0-alpine`).

use super::order::{Mad, Median, Percentile};
use super::tests::{Window, bucketized, doubles, longs};
use super::{Aggregated, Aggregator};
use crate::geoxp;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::tests::{assert_fails, assert_stack, assert_top};
use crate::value::Value;

/// The `[ [ lats ] [ lons ] ]` and `[ elevations ]` of the series on top of the stack.
const GEO: &str = "DUP LOCATIONS 2 ->LIST SWAP ELEVATIONS";

fn geo(lat: f64, lon: f64, elevation: i64) -> [Value; 2] {
    [
        Value::list(vec![
            Value::list(vec![Value::Double(lat)]),
            Value::list(vec![Value::Double(lon)]),
        ]),
        Value::list(vec![Value::Long(elevation)]),
    ]
}

/// `datapoints` (`tick lat lon elev value` each) bucketized as a whole by `bucketizer`.
fn located(datapoints: &[&str], bucketizer: &str) -> String {
    let mut script = String::from("[ NEWGTS");
    for datapoint in datapoints {
        script.push_str(&format!(" {datapoint} ADDVALUE"));
    }
    script.push_str(&format!(" {bucketizer} 0 100 0 ] BUCKETIZE 0 GET"));
    script
}

#[test]
fn median_of_a_series() {
    assert_top(
        &format!("{} VALUES", bucketized("1 2 3 4", "bucketizer.median")),
        Value::list(doubles(&[2.5])),
    );
    assert_top(
        &format!("{} VALUES", bucketized("1 2 3", "mapper.median")),
        Value::list(doubles(&[2.0])),
    );
    // Values are stored as the series' type: LONG here.
    assert_top(
        &format!("{} VALUES", bucketized("1 2.5", "reducer.median")),
        Value::list(doubles(&[1.5])),
    );
    assert_top(
        &format!("{} VALUES", bucketized("1 NaN 2.0", "bucketizer.median")),
        Value::list(doubles(&[1.0])),
    );
    assert_fails(
        &format!("{} VALUES", bucketized("'a' 'b'", "bucketizer.median")),
        "bucketizer.median can only operate on numeric Geo Time Series.",
    );
    assert_fails(
        &format!("{} VALUES", bucketized("'a'", "bucketizer.median")),
        "class java.lang.String cannot be cast to class java.lang.Number",
    );
    // Even count: the centroid of the two central locations, the mean of their elevations,
    // or the one there is.
    assert_stack(
        &format!(
            "{} DUP VALUES SWAP {GEO}",
            located(
                &[
                    "10 48.0 2.0 100 1",
                    "20 49.0 3.0 200 2",
                    "30 NaN NaN NaN 3",
                    "40 NaN NaN NaN 4"
                ],
                "bucketizer.median"
            )
        ),
        &[
            Value::list(doubles(&[2.5])),
            Value::list(vec![
                Value::list(doubles(&[48.999_999_971_129])),
                Value::list(doubles(&[2.999_999_988_824_129])),
            ]),
            Value::list(longs(&[200])),
        ],
    );
    assert_stack(
        &format!(
            "{} DUP VALUES SWAP {GEO}",
            located(
                &[
                    "10 48.0 2.0 100 1",
                    "20 NaN NaN NaN 2",
                    "30 NaN NaN 300 3",
                    "40 49.0 3.0 NaN 4"
                ],
                "bucketizer.median"
            )
        ),
        &[
            Value::list(doubles(&[2.5])),
            Value::list(vec![
                Value::list(doubles(&[f64::NAN])),
                Value::list(doubles(&[f64::NAN])),
            ]),
            Value::list(longs(&[300])),
        ],
    );
    assert_stack(
        &format!(
            "{} DUP VALUES SWAP {GEO}",
            located(
                &[
                    "10 48.0 2.0 100 1",
                    "20 48.5 2.5 200 2",
                    "30 49.0 3.0 300 3",
                    "40 49.5 3.5 400 4"
                ],
                "bucketizer.median"
            )
        ),
        &[
            Value::list(doubles(&[2.5])),
            Value::list(vec![
                Value::list(doubles(&[48.749_999_944_120_646])),
                Value::list(doubles(&[2.749_999_919_906_258_6])),
            ]),
            Value::list(longs(&[250])),
        ],
    );
    // Odd count: the median datapoint's.
    assert_stack(
        &format!(
            "{} DUP TICKLIST SWAP DUP VALUES SWAP {GEO}",
            located(
                &["10 48.0 2.0 100 1", "20 NaN NaN NaN 2", "30 NaN NaN 300 3"],
                "bucketizer.median"
            )
        ),
        &[
            Value::list(longs(&[100])),
            Value::list(doubles(&[2.0])),
            Value::list(vec![
                Value::list(doubles(&[f64::NAN])),
                Value::list(doubles(&[f64::NAN])),
            ]),
            Value::list(vec![Value::Double(f64::NAN)]),
        ],
    );
    assert_top("bucketizer.median TYPEOF", "AGGREGATOR");
}

#[test]
fn median_on_windows_with_nulls_and_mixed_types() {
    let median = Median {
        name: "reducer.median",
        forbid_nulls: false,
    };
    let forbid = Median {
        name: "reducer.median.forbid-nulls",
        forbid_nulls: true,
    };
    let with_null = Window::of(&[Value::Null, Value::Long(3), Value::Long(1)]);
    assert_eq!(with_null.value(&median), Value::Double(2.0));
    assert_eq!(
        Window::of(&[Value::Long(1), Value::Null, Value::Null, Value::Long(2)]).value(&median),
        Value::Double(1.5)
    );
    assert_eq!(
        with_null.error(&forbid),
        "reducer.median.forbid-nulls cannot compute median of null values."
    );
    // The single non-null datapoint, with its location and elevation.
    let single = Window {
        ticks: vec![1, 2],
        locations: vec![7, 8],
        elevations: vec![70, 80],
        values: vec![Value::Null, Value::Long(5)],
    };
    assert_eq!(
        single.apply(&median).unwrap(),
        Aggregated {
            tick: 100,
            location: 8,
            elevation: 80,
            value: Value::Double(5.0),
        }
    );
    assert_eq!(
        Window::of(&[Value::Null, Value::from("a")]).error(&median),
        "class java.lang.String cannot be cast to class java.lang.Number"
    );
    // No value at all: Warp 10 reads `indices[-1]`.
    assert_eq!(
        Window::of(&[Value::Null, Value::Null]).error(&median),
        "Index -1 out of bounds for length 2"
    );
    assert_eq!(
        Window::of(&[]).error(&median),
        "Index -1 out of bounds for length 0"
    );
    assert_eq!(
        Window::of(&[Value::Long(1), Value::from("a")]).error(&median),
        "reducer.median can only operate on numeric Geo Time Series."
    );
    assert_eq!(
        Window::of(&[Value::Long(1), Value::Boolean(true), Value::Null]).error(&median),
        "reducer.median can only operate on numeric Geo Time Series."
    );
    // Mixed LONG / DOUBLE windows compare exactly, NaN last.
    assert_eq!(
        Window::of(&[Value::Long(1), Value::Double(2.5), Value::Long(2)]).value(&median),
        Value::Double(2.0)
    );
    assert_eq!(
        Window::of(&[Value::Double(f64::NAN), Value::Long(1), Value::Double(2.0)]).value(&median),
        Value::Double(2.0)
    );
    assert_eq!(
        Window::of(&[Value::Double(2.0), Value::Long(1), Value::Double(f64::NAN)]).value(&median),
        Value::Double(2.0)
    );
    assert_eq!(
        Window::of(&[Value::Long(1), Value::Double(f64::INFINITY)]).value(&median),
        Value::Double(f64::INFINITY)
    );
    assert_eq!(
        Window::of(&[Value::Double(f64::NEG_INFINITY), Value::Long(1)]).value(&median),
        Value::Double(f64::NEG_INFINITY)
    );
    assert_eq!(
        Window::of(&[Value::Double(-0.0), Value::Double(0.0), Value::Long(1)]).value(&median),
        Value::Double(0.0)
    );
    // Even count: the centroid of two locations, the one elevation there is.
    let even = Window {
        ticks: vec![1, 2, 3, 4],
        locations: vec![
            NO_LOCATION,
            geoxp::to_geoxp_point(48.0, 2.0),
            geoxp::to_geoxp_point(49.0, 3.0),
            NO_LOCATION,
        ],
        elevations: vec![NO_ELEVATION, NO_ELEVATION, 300, NO_ELEVATION],
        values: longs(&[1, 2, 3, 4]),
    };
    let result = even.apply(&median).unwrap();
    assert_eq!(
        geoxp::from_geoxp_point(result.location),
        (48.499_999_959_021_81, 2.499_999_934_807_419_8)
    );
    assert_eq!(result.elevation, 300);
    let low_only = Window {
        elevations: vec![NO_ELEVATION, 200, NO_ELEVATION, NO_ELEVATION],
        locations: vec![
            NO_LOCATION,
            geoxp::to_geoxp_point(48.0, 2.0),
            NO_LOCATION,
            NO_LOCATION,
        ],
        ..even
    };
    let result = low_only.apply(&median).unwrap();
    assert_eq!(result.elevation, 200);
    assert_eq!(result.location, geoxp::to_geoxp_point(48.0, 2.0));
    let high_only = Window {
        elevations: vec![NO_ELEVATION; 4],
        locations: vec![
            NO_LOCATION,
            NO_LOCATION,
            geoxp::to_geoxp_point(49.0, 3.0),
            NO_LOCATION,
        ],
        ..low_only
    };
    let result = high_only.apply(&median).unwrap();
    assert_eq!(result.elevation, NO_ELEVATION);
    assert_eq!(result.location, geoxp::to_geoxp_point(49.0, 3.0));
}

#[test]
fn mad_of_a_series() {
    assert_top(
        &format!("{} VALUES", bucketized("1 2 3 4 10", "bucketizer.mad")),
        Value::list(longs(&[1])),
    );
    assert_top(
        &format!("{} VALUES", bucketized("1 2 3 4", "mapper.mad")),
        Value::list(longs(&[1])),
    );
    assert_top(
        &format!("{} VALUES", bucketized("1.0 2.0 3.0 4.0", "reducer.mad")),
        Value::list(doubles(&[1.0])),
    );
    // Values are stored as the series' type.
    assert_top(
        &format!("{} VALUES", bucketized("1 2.0", "bucketizer.mad")),
        Value::list(longs(&[0])),
    );
    assert_top(
        &format!("{} VALUES", bucketized("2.0 1", "bucketizer.mad")),
        Value::list(doubles(&[0.5])),
    );
    // Non numeric values: the median is selected like for numbers, then cannot be cast.
    for (values, message) in [
        (
            "true false",
            "Unable to compute median on an even number of non numeric values.",
        ),
        (
            "'a' 'b'",
            "Unable to compute median on an even number of non numeric values.",
        ),
        (
            "true",
            "class java.lang.Boolean cannot be cast to class java.lang.Double",
        ),
        (
            "'a' 'b' 'c'",
            "class java.lang.String cannot be cast to class java.lang.Double",
        ),
        (
            "'a' 'a'",
            "class java.lang.String cannot be cast to class java.lang.Double",
        ),
    ] {
        assert_fails(&bucketized(values, "bucketizer.mad"), message);
    }
    assert_top("bucketizer.mad TYPEOF", "AGGREGATOR");
}

#[test]
fn mad_takes_the_median_location_and_elevation() {
    // Three datapoints, two located: the sorted locations end with the marker, which stays.
    let two_of_three = &["10 48.0 2.0 100 1", "20 49.0 3.0 200 2", "30 NaN NaN NaN 3"];
    assert_stack(
        &format!("{} {GEO}", located(two_of_three, "bucketizer.mad")),
        &geo(48.999_999_971_129, 2.999_999_988_824_129, 150),
    );
    // Four datapoints, two located: the central pair includes the marker's coordinates.
    let two_of_four = &[
        "10 48.0 2.0 100 1",
        "20 49.0 3.0 200 2",
        "30 NaN NaN NaN 3",
        "40 NaN NaN NaN 4",
    ];
    assert_stack(
        &format!("{} {GEO}", located(two_of_four, "bucketizer.mad")),
        &geo(-20.500_000_035_390_258, -69.441_176_522_523_16, 100),
    );
    // Southern / western locations sort before the marker, which gets removed.
    let negative = &[
        "10 -48.0 -2.0 100 1",
        "20 -49.0 -3.0 200 2",
        "30 NaN NaN NaN 3",
        "40 NaN NaN NaN 4",
    ];
    assert_stack(
        &format!("{} {GEO}", located(negative, "bucketizer.mad")),
        &geo(-49.000_000_013_038_516, -3.000_000_072_643_161, 100),
    );
    assert_stack(
        &format!("{} {GEO}", located(&negative[..3], "bucketizer.mad")),
        &geo(-48.500_000_042_840_84, -2.500_000_102_445_483, 150),
    );
    // More markers than locations: the binary search lands on a marker that is not the
    // first one, so the median is computed over leftovers (Warp 10 does the same).
    let two_of_six = &[
        "10 48.0 2.0 100 1",
        "20 49.0 3.0 200 2",
        "30 NaN NaN NaN 3",
        "40 NaN NaN NaN 4",
        "50 NaN NaN NaN 5",
        "60 NaN NaN NaN 6",
    ];
    assert_stack(
        &format!("{} {GEO}", located(two_of_six, "bucketizer.mad")),
        &geo(f64::NAN, f64::NAN, -4_611_686_018_427_387_854),
    );
    assert_stack(
        &format!("{} {GEO}", located(&two_of_six[..5], "bucketizer.mad")),
        &geo(f64::NAN, f64::NAN, -4_611_686_018_427_387_854),
    );
    let all_located = &[
        "10 48.0 2.0 100 1",
        "20 49.0 3.0 200 2",
        "30 50.0 4.0 300 3",
    ];
    assert_stack(
        &format!("{} {GEO}", located(all_located, "bucketizer.mad")),
        &geo(48.999_999_971_129, 2.999_999_988_824_129, 200),
    );
}

#[test]
fn mad_on_windows_with_nulls_and_mixed_types() {
    assert_eq!(
        Window::of(&[Value::Null, Value::Long(1), Value::Long(2), Value::Long(3)]).value(&Mad),
        Value::Long(1)
    );
    assert_eq!(
        Window::of(&[Value::Null, Value::Null]).apply(&Mad).unwrap(),
        Aggregated {
            tick: 100,
            location: NO_LOCATION,
            elevation: NO_ELEVATION,
            value: Value::Null,
        }
    );
    assert_eq!(Window::of(&[Value::Long(5)]).value(&Mad), Value::Long(0));
    assert_eq!(
        Window::of(&[]).error(&Mad),
        "Index 0 out of bounds for length 0"
    );
    // `Arrays.sort` cannot compare two classes.
    assert_eq!(
        Window::of(&[Value::Long(1), Value::Long(2), Value::Double(2.0)]).error(&Mad),
        "class java.lang.Long cannot be cast to class java.lang.Double"
    );
    assert_eq!(
        Window::of(&[Value::Double(2.0), Value::Long(1)]).error(&Mad),
        "class java.lang.Double cannot be cast to class java.lang.Long"
    );
    assert_eq!(
        Window::of(&[Value::from("a"), Value::Boolean(true)]).error(&Mad),
        "class java.lang.String cannot be cast to class java.lang.Boolean"
    );
    assert_eq!(
        Window::of(&[Value::list(Vec::new())]).error(&Mad),
        "class java.util.ArrayList cannot be cast to class java.lang.Comparable"
    );
    // Doubles order with NaN last; equal extrema (`Double.equals`) short-circuit.
    assert_eq!(
        Window::of(&doubles(&[1.0, f64::NAN, 2.0])).value(&Mad),
        Value::Double(1.0)
    );
    assert!(matches!(
        Window::of(&doubles(&[f64::NAN, f64::NAN])).value(&Mad),
        Value::Double(d) if d.is_nan()
    ));
    assert_eq!(
        Window::of(&doubles(&[-0.0, 0.0])).value(&Mad),
        Value::Double(0.0)
    );
    // Wrapping arithmetic: the median is `(MIN + MAX) / 2 = 0`, the residuals `|MIN|` (which
    // wraps to `MIN`) and `MAX`, whose median is 0 again.
    assert_eq!(
        Window::of(&longs(&[i64::MIN, i64::MAX])).value(&Mad),
        Value::Long(0)
    );
    assert_eq!(
        Window::of(&[Value::Boolean(true), Value::Boolean(true)]).error(&Mad),
        "class java.lang.Boolean cannot be cast to class java.lang.Double"
    );
}

#[test]
fn percentile_types_follow_hyndman_and_fan() {
    const TEN: &str = "1 2 3 4 5 6 7 8 9 10";
    let expected_33: [Value; 9] = [
        Value::Long(4),
        Value::Double(4.0),
        Value::Long(3),
        Value::Double(3.300_000_000_000_000_3),
        Value::Double(3.800_000_000_000_000_3),
        Value::Double(3.630_000_000_000_000_3),
        Value::Double(3.97),
        Value::Double(3.743_333_333_333_333_6),
        Value::Double(3.757_500_000_000_000_3),
    ];
    let expected_25_75_90: [[f64; 3]; 9] = [
        [3.0, 8.0, 9.0],
        [3.0, 8.0, 9.5],
        [2.0, 8.0, 9.0],
        [2.5, 7.5, 9.0],
        [3.0, 8.0, 9.5],
        [2.75, 8.25, 9.9],
        [3.25, 7.75, 9.1],
        [
            2.916_666_666_666_666_5,
            8.083_333_333_333_334,
            9.633_333_333_333_333,
        ],
        [2.9375, 8.0625, 9.6],
    ];
    let expected_three: [[f64; 3]; 9] = [
        [1.0, 3.0, 2.0],
        [1.0, 3.0, 2.0],
        [1.0, 2.0, 2.0],
        [1.0, 2.25, 1.5],
        [1.25, 2.75, 2.0],
        [1.0, 3.0, 2.0],
        [1.5, 2.5, 2.0],
        [1.166_666_666_666_666_7, 2.833_333_333_333_333_5, 2.0],
        [1.1875, 2.8125, 2.0],
    ];
    let expected_halves: [[f64; 3]; 9] = [
        [1.5, 3.5, 2.5],
        [1.5, 3.5, 2.5],
        [1.5, 2.5, 2.5],
        [1.5, 2.75, 2.0],
        [1.75, 3.25, 2.5],
        [1.5, 3.5, 2.5],
        [2.0, 3.0, 2.5],
        [1.666_666_666_666_667, 3.333_333_333_333_333, 2.5],
        [1.6875, 3.3125, 2.5],
    ];
    let value =
        |values: &str, bucketizer: &str| format!("{} VALUES 0 GET", bucketized(values, bucketizer));
    for kind in 1..=9 {
        let percentile = |p: &str| format!("{p} 'type{kind}' bucketizer.percentile");
        assert_top(
            &value(TEN, &percentile("33")),
            expected_33[kind - 1].clone(),
        );
        // The lowest and highest values for ranks 0 and 100, 50 by every type.
        let lowest = if kind == 1 || kind == 3 {
            Value::Long(1)
        } else {
            Value::Double(1.0)
        };
        let highest = if kind == 1 || kind == 3 {
            Value::Long(10)
        } else {
            Value::Double(10.0)
        };
        let half = match kind {
            1 | 3 => Value::Long(5),
            4 => Value::Double(5.0),
            _ => Value::Double(5.5),
        };
        assert_top(&value(TEN, &percentile("0")), lowest);
        assert_top(&value(TEN, &percentile("100")), highest);
        assert_top(&value(TEN, &percentile("50")), half);
        for (i, p) in ["25", "75", "90"].iter().enumerate() {
            let expected = expected_25_75_90[kind - 1][i];
            let expected = if kind == 1 || kind == 3 {
                Value::Long(expected as i64)
            } else {
                Value::Double(expected)
            };
            assert_top(&value(TEN, &percentile(p)), expected);
        }
        for (i, p) in ["25", "75", "50"].iter().enumerate() {
            let expected = expected_three[kind - 1][i];
            let expected = if kind == 1 || kind == 3 {
                Value::Long(expected as i64)
            } else {
                Value::Double(expected)
            };
            assert_top(&value("1 2 3", &percentile(p)), expected);
            assert_top(
                &value("1.5 2.5 3.5", &percentile(p)),
                Value::Double(expected_halves[kind - 1][i]),
            );
        }
    }
    // Types 2 and 3 around their half-way special cases, type 4 at an exact rank.
    for (bucketizer, expected) in [
        ("100 'type3' bucketizer.percentile", Value::Long(4)),
        ("0 'type3' bucketizer.percentile", Value::Long(1)),
        ("12.5 'type3' bucketizer.percentile", Value::Long(1)),
        ("37.5 'type3' bucketizer.percentile", Value::Long(2)),
        ("50 'type2' bucketizer.percentile", Value::Double(2.5)),
        ("100 'type2' bucketizer.percentile", Value::Double(4.0)),
        ("100 'type4' bucketizer.percentile", Value::Double(4.0)),
        ("75 'type4' bucketizer.percentile", Value::Double(3.0)),
    ] {
        assert_top(&value("1 2 3 4", bucketizer), expected);
    }
    // A NaN rank is accepted and selects the lowest value.
    assert_top(&value("3 1 2", "NaN bucketizer.percentile"), Value::Long(1));
    assert_top(
        &value("3 1 2", "NaN 'type7' bucketizer.percentile"),
        Value::Double(1.0),
    );
}

#[test]
fn percentile_keeps_or_interpolates_the_datapoint() {
    // Type 1 selects a datapoint with its location and elevation.
    assert_stack(
        &format!(
            "{} DUP VALUES SWAP {GEO}",
            located(
                &["10 48.0 2.0 100 1", "20 49.0 3.0 200 2", "30 NaN NaN NaN 3"],
                "50 'type1' bucketizer.percentile"
            )
        ),
        &[
            Value::list(longs(&[2])),
            Value::list(vec![
                Value::list(doubles(&[48.999_999_971_129])),
                Value::list(doubles(&[2.999_999_988_824_129])),
            ]),
            Value::list(longs(&[200])),
        ],
    );
    // Type 7 interpolates the value, the location and the elevation (when both exist).
    for (datapoints, lat, lon, elevation) in [
        (
            ["10 48.0 2.0 100 1", "20 49.0 3.0 200 2"],
            48.249_999_973_922_97,
            2.249_999_949_708_581,
            125,
        ),
        (
            ["10 48.0 2.0 100 1", "20 NaN NaN NaN 2"],
            47.999_999_988_824_13,
            1.999_999_964_609_742_2,
            100,
        ),
        (
            ["10 NaN NaN NaN 1", "20 49.0 3.0 200 2"],
            48.999_999_971_129,
            2.999_999_988_824_129,
            200,
        ),
    ] {
        assert_stack(
            &format!(
                "{} DUP VALUES SWAP {GEO}",
                located(&datapoints, "25 'type7' bucketizer.percentile")
            ),
            &[
                Value::list(doubles(&[1.25])),
                Value::list(vec![
                    Value::list(doubles(&[lat])),
                    Value::list(doubles(&[lon])),
                ]),
                Value::list(longs(&[elevation])),
            ],
        );
    }
    // The interpolated tick (`BUCKETIZE` imposes the bucket's).
    let two = Window {
        ticks: vec![10, 20],
        locations: vec![NO_LOCATION, NO_LOCATION],
        elevations: vec![NO_ELEVATION, NO_ELEVATION],
        values: longs(&[1, 2]),
    };
    assert_eq!(
        two.apply(&Percentile::new("mapper.percentile", 25.0, 7, false))
            .unwrap(),
        Aggregated {
            tick: 13,
            location: NO_LOCATION,
            elevation: NO_ELEVATION,
            value: Value::Double(1.25),
        }
    );
    // A single non numeric value: kept as is by type 1, uncastable for the others.
    assert_top(
        &format!(
            "{} VALUES",
            bucketized("'a'", "25 'type1' bucketizer.percentile")
        ),
        Value::list(vec![Value::from("a")]),
    );
    assert_fails(
        &bucketized("'a'", "25 'type7' bucketizer.percentile"),
        "class java.lang.String cannot be cast to class java.lang.Number",
    );
    assert_fails(
        &bucketized("'a' 'b'", "50 bucketizer.percentile"),
        "java.lang.RuntimeException: bucketizer.percentile can only operate on numeric Geo Time Series. (bucketizer.percentile can only operate on numeric Geo Time Series.)",
    );
}

#[test]
fn percentile_builder() {
    assert_top(
        "50 'type7' bucketizer.percentile SNAPSHOT",
        "50.0 'type7' bucketizer.percentile ",
    );
    assert_top(
        "50 'TYPE7' mapper.percentile SNAPSHOT",
        "50.0 'type7' mapper.percentile ",
    );
    assert_top(
        "50 reducer.percentile SNAPSHOT",
        "50.0 'type1' reducer.percentile ",
    );
    assert_top(
        "50.5 reducer.percentile.forbid-nulls SNAPSHOT",
        "50.5 'type1' reducer.percentile.forbid-nulls ",
    );
    assert_top(
        "NaN bucketizer.percentile SNAPSHOT",
        "NaN 'type1' bucketizer.percentile ",
    );
    assert_top("50 bucketizer.percentile TYPEOF", "AGGREGATOR");
    for kind in ["'typeA'", "'type0'"] {
        assert_fails(
            &format!("50 {kind} bucketizer.percentile"),
            "bucketizer.percentile expect percentile type to be 'typeX' where X is a integer and 0<X<10.",
        );
    }
    for kind in ["'abc'", "'type'", "'type10'", "'x'"] {
        assert_fails(
            &format!("50 {kind} bucketizer.percentile"),
            "bucketizer.percentile expect percentile type to be 'typeX' where 0<X<10.",
        );
    }
    assert_fails("'type2' bucketizer.percentile", "Empty stack");
    assert_fails("bucketizer.percentile", "Empty stack");
    assert_fails(
        "true 'type2' bucketizer.percentile",
        "Invalid parameter for bucketizer.percentile",
    );
    for rank in ["101", "-1", "100.5"] {
        assert_fails(
            &format!("{rank} bucketizer.percentile"),
            "Invalid percentile for bucketizer.percentile, MUST be between 0 and 100.",
        );
    }
    // The constructor clamps the rank like Warp 10's.
    assert_eq!(
        Percentile::new("p", 150.0, 1, false).parameters(),
        Some("100.0 'type1'".to_owned())
    );
    assert_eq!(
        Percentile::new("p", -5.0, 9, false).parameters(),
        Some("0.0 'type9'".to_owned())
    );
}

#[test]
fn percentile_on_windows_with_nulls_and_mixed_types() {
    let median = Percentile::new("reducer.percentile", 50.0, 1, false);
    let forbid = Percentile::new("reducer.percentile.forbid-nulls", 50.0, 1, true);
    let with_null = Window::of(&[Value::Null, Value::Long(3), Value::Long(1), Value::Long(2)]);
    assert_eq!(with_null.value(&median), Value::Long(2));
    assert_eq!(
        with_null.error(&forbid),
        "reducer.percentile.forbid-nulls cannot compute percentile of null values."
    );
    assert_eq!(
        Window::of(&[Value::Null]).apply(&median).unwrap(),
        Aggregated {
            tick: 0,
            ..Aggregated::none()
        }
    );
    assert_eq!(Window::of(&[]).apply(&median).unwrap(), Aggregated::none());
    assert_eq!(
        Window::of(&[Value::Null, Value::from("a")]).value(&median),
        Value::from("a")
    );
    assert_eq!(
        Window::of(&[Value::Null, Value::from("a")]).error(&Percentile::new(
            "reducer.percentile",
            50.0,
            7,
            false
        )),
        "class java.lang.String cannot be cast to class java.lang.Number"
    );
    assert_eq!(
        Window::of(&[Value::Long(1), Value::from("a")]).error(&median),
        "java.lang.RuntimeException: reducer.percentile can only operate on numeric Geo Time Series. (reducer.percentile can only operate on numeric Geo Time Series.)"
    );
    assert_eq!(
        Window::of(&[Value::Long(1), Value::Double(2.5), Value::Long(2)]).value(&Percentile::new(
            "reducer.percentile",
            50.0,
            7,
            false
        )),
        Value::Double(2.0)
    );
    // Types outside 1..=9 only exist when built directly.
    assert_eq!(
        Window::of(&longs(&[1, 2])).error(&Percentile::new("mapper.percentile", 50.0, 10, false)),
        "mapper.percentile given invalid type."
    );
}
