// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the statistical aggregators. The expected values were checked against Warp 10
//! 3.5.0 (`warp10io/warp10:3.5.0-alpine`).

use super::Aggregated;
use super::stats::{CircularMean, Rms, StandardDeviation, Variance};
use super::tests::{Window, bucketized, doubles, longs};
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::tests::{assert_fails, assert_stack, assert_top};
use crate::value::Value;

/// Two located and elevated datapoints, bucketized as a whole.
fn located(bucketizer: &str) -> String {
    format!(
        "[ NEWGTS 10 48.0 2.0 100 1 ADDVALUE 20 49.0 3.0 200 2 ADDVALUE {bucketizer} 0 100 0 ] BUCKETIZE 0 GET"
    )
}

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

#[test]
fn rms_is_the_root_mean_square_typed_by_the_first_value() {
    assert_top(
        &format!("{} VALUES", bucketized("1 2 3", "bucketizer.rms")),
        Value::list(longs(&[2])),
    );
    assert_top(
        &format!("{} VALUES", bucketized("1.0 2.0 3.0", "bucketizer.rms")),
        Value::list(doubles(&[2.160_246_899_469_287])),
    );
    assert_top(
        &format!("{} VALUES", bucketized("1.5 2.5", "mapper.rms")),
        Value::list(doubles(&[2.061_552_812_808_830_3])),
    );
    assert_top(
        &format!("{} VALUES", bucketized("7", "reducer.rms")),
        Value::list(longs(&[7])),
    );
    // A string in a LONG series is stored as 0.
    assert_top(
        &format!("{} VALUES", bucketized("1 'b'", "bucketizer.rms")),
        Value::list(longs(&[0])),
    );
    // Strings have no RMS.
    assert_top(
        &format!("{} SIZE", bucketized("'a' 'b'", "bucketizer.rms")),
        0,
    );
    // Location and elevation of the most recent datapoint.
    assert_stack(
        &format!("{} {GEO}", located("bucketizer.rms")),
        &geo(48.999_999_971_129, 2.999_999_988_824_129, 200),
    );
    assert_top("bucketizer.rms TYPEOF", "AGGREGATOR");
    assert_top(
        "reducer.rms.exclude-nulls SNAPSHOT",
        "'reducer.rms.exclude-nulls' FUNCREF ",
    );
}

#[test]
fn rms_on_windows_with_nulls_and_mixed_types() {
    let poison = Rms {
        ignore_nulls: false,
    };
    let ignore = Rms { ignore_nulls: true };
    let with_null = Window::of(&[Value::Long(1), Value::Null, Value::Long(3)]);
    assert_eq!(with_null.apply(&poison).unwrap(), Aggregated::none());
    // sqrt((1 + 9) / 2) = 2.236… cast to a LONG.
    assert_eq!(with_null.value(&ignore), Value::Long(2));
    let all_null = Window::of(&[Value::Null, Value::Null]);
    assert_eq!(all_null.apply(&ignore).unwrap(), Aggregated::none());
    assert_eq!(Window::of(&[]).apply(&ignore).unwrap(), Aggregated::none());
    assert_eq!(
        Window::of(&[Value::Boolean(true)]).apply(&ignore).unwrap(),
        Aggregated::none()
    );
    // The first value fixes the type, the following ones must be numbers.
    assert_eq!(
        Window::of(&[Value::Long(3), Value::Double(4.0)]).value(&ignore),
        Value::Long(3)
    );
    assert_eq!(
        Window::of(&[Value::Double(3.0), Value::Long(4)]).value(&ignore),
        Value::Double(3.535_533_905_932_737_7)
    );
    assert_eq!(
        Window::of(&[Value::Long(1), Value::from("a")]).error(&ignore),
        "class java.lang.String cannot be cast to class java.lang.Number"
    );
    let latest = Window {
        ticks: vec![2, 1],
        locations: vec![7, 8],
        elevations: vec![70, 80],
        values: longs(&[1, 2]),
    };
    assert_eq!(
        latest.apply(&ignore).unwrap(),
        Aggregated {
            tick: 100,
            location: 7,
            elevation: 70,
            value: Value::Long(1),
        }
    );
}

#[test]
fn variance_and_standard_deviation_builders() {
    for (name, expected) in [
        ("true bucketizer.var", 1.666_666_666_666_666_7),
        ("false bucketizer.var", 1.25),
        ("true bucketizer.var.welford", 1.666_666_666_666_666_7),
        ("false mapper.var.welford", 1.25),
        ("true reducer.sd", 1.290_994_448_735_805_6),
        ("false bucketizer.sd.welford", 1.118_033_988_749_895),
        ("true mapper.sd", 1.290_994_448_735_805_6),
    ] {
        assert_top(
            &format!("{} VALUES", bucketized("1 2 3 4", name)),
            Value::list(doubles(&[expected])),
        );
    }
    // A single value has a zero variance, with or without Bessel's correction.
    assert_top(
        &format!("{} VALUES", bucketized("7", "true bucketizer.var")),
        Value::list(doubles(&[0.0])),
    );
    assert_top(
        &format!("{} VALUES", bucketized("1 'b'", "true bucketizer.var")),
        Value::list(doubles(&[0.5])),
    );
    assert_top(
        &format!("{} SIZE", bucketized("'a' 'b'", "true bucketizer.sd")),
        0,
    );
    assert_stack(
        &format!("{} {GEO}", located("true bucketizer.var")),
        &geo(48.999_999_971_129, 2.999_999_988_824_129, 200),
    );
    // The objects snapshot as the builder call that produced them.
    assert_top("true bucketizer.var SNAPSHOT", "true bucketizer.var ");
    assert_top(
        "false reducer.sd.welford SNAPSHOT",
        "false reducer.sd.welford ",
    );
    assert_top("true bucketizer.sd TYPEOF", "AGGREGATOR");
    assert_top(
        "<% true bucketizer.var %> SNAPSHOT",
        "<% true bucketizer.var %> ",
    );
    assert_top("true 'bucketizer.var' FUNCREF EVAL TYPEOF", "AGGREGATOR");
    assert_top(
        "'bucketizer.var' FUNCREF SNAPSHOT",
        "'bucketizer.var' FUNCREF ",
    );
    assert_top("<% true bucketizer.var %> EVAL TYPEOF", "AGGREGATOR");
    assert_fails(
        "'x' bucketizer.var",
        "bucketizer.var expects a boolean parameter to determine whether or not to apply Bessel's correction.",
    );
    assert_fails(
        "1 reducer.sd.forbid-nulls",
        "reducer.sd.forbid-nulls expects a boolean parameter to determine whether or not to apply Bessel's correction.",
    );
    assert_fails("bucketizer.var", "Empty stack");
    assert_fails(
        "<% 'x' bucketizer.var %> EVAL",
        "Exception at 'bucketizer.var' in section '[TOP]'",
    );
    assert_fails(
        "true bucketizer.var TOSTRING",
        "TOSTRING can only operate on numeric, boolean or string values.",
    );
}

#[test]
fn variance_on_windows_with_nulls_and_mixed_types() {
    let naive = Variance {
        use_bessel: false,
        forbid_nulls: false,
        welford: false,
    };
    let forbid = Variance {
        use_bessel: false,
        forbid_nulls: true,
        welford: false,
    };
    let welford = Variance {
        use_bessel: false,
        forbid_nulls: false,
        welford: true,
    };
    let welford_forbid = Variance {
        use_bessel: true,
        forbid_nulls: true,
        welford: true,
    };
    let with_null = Window::of(&[Value::Long(1), Value::Null, Value::Long(3)]);
    assert_eq!(with_null.value(&naive), Value::Double(1.0));
    assert_eq!(with_null.apply(&forbid).unwrap(), Aggregated::none());
    assert_eq!(
        with_null.apply(&welford_forbid).unwrap(),
        Aggregated::none()
    );
    // Welford divides by the index in the window, so a skipped null skews the result
    // (Warp 10 does the same).
    let leading_null = Window::of(&[Value::Null, Value::Long(1), Value::Long(2)]);
    assert_eq!(leading_null.value(&welford), Value::Double(1.0));
    assert_eq!(
        leading_null.value(&Variance {
            use_bessel: true,
            ..welford
        }),
        Value::Double(2.0)
    );
    for variance in [&naive, &welford] {
        assert_eq!(Window::of(&[]).apply(variance).unwrap(), Aggregated::none());
        assert_eq!(
            Window::of(&[Value::from("a"), Value::Long(1)])
                .apply(variance)
                .unwrap(),
            Aggregated::none()
        );
        assert_eq!(
            Window::of(&[Value::Long(2), Value::Double(4.0)]).value(variance),
            Value::Double(1.0)
        );
    }
    // A later non numeric value: a class cast for the naive formula, no result for Welford.
    let mixed = Window::of(&[Value::Long(1), Value::Boolean(true)]);
    assert_eq!(
        mixed.error(&naive),
        "class java.lang.Boolean cannot be cast to class java.lang.Number"
    );
    assert_eq!(mixed.apply(&welford).unwrap(), Aggregated::none());
    let sd = StandardDeviation {
        variance: Variance {
            use_bessel: true,
            forbid_nulls: false,
            welford: false,
        },
    };
    assert_eq!(
        Window::of(&longs(&[1, 2, 3, 4])).value(&sd),
        Value::Double(1.290_994_448_735_805_6)
    );
    assert_eq!(mixed.error(&sd), mixed.error(&naive));
    assert_eq!(
        with_null.value(&sd),
        Value::Double(std::f64::consts::SQRT_2)
    );
    assert_eq!(
        Window::of(&[Value::from("a")]).apply(&sd).unwrap(),
        Aggregated::none()
    );
    // Location and elevation of the most recent non-null datapoint.
    let latest = Window {
        ticks: vec![3, 2, 1],
        locations: vec![7, 8, 9],
        elevations: vec![70, 80, 90],
        values: vec![Value::Null, Value::Long(1), Value::Long(2)],
    };
    assert_eq!(
        latest.apply(&sd).unwrap(),
        Aggregated {
            tick: 0,
            location: 8,
            elevation: 80,
            value: Value::Double(std::f64::consts::FRAC_1_SQRT_2),
        }
    );
}

#[test]
fn circular_mean_builder_and_aggregation() {
    assert_top(
        &format!(
            "{} VALUES",
            bucketized("90 180", "360 bucketizer.mean.circular")
        ),
        Value::list(doubles(&[135.0])),
    );
    assert_top(
        &format!(
            "{} VALUES",
            bucketized("350 10", "360 mapper.mean.circular")
        ),
        Value::list(doubles(&[-1.614_809_932_057_922e-15])),
    );
    assert_top(
        &format!(
            "{} VALUES",
            bucketized("1 'b'", "360 reducer.mean.circular")
        ),
        Value::list(doubles(&[0.5])),
    );
    assert_top(
        &format!("{} SIZE", bucketized("'b'", "360 bucketizer.mean.circular")),
        0,
    );
    // The mean location and elevation.
    assert_stack(
        &format!(
            "[ NEWGTS 10 48.0 2.0 100 90 ADDVALUE 20 49.0 3.0 200 180 ADDVALUE 360 bucketizer.mean.circular 0 100 0 ] BUCKETIZE 0 GET DUP VALUES SWAP {GEO}"
        ),
        &[
            Value::list(doubles(&[135.0])),
            Value::list(vec![
                Value::list(doubles(&[48.499_999_959_021_81])),
                Value::list(doubles(&[2.499_999_934_807_419_8])),
            ]),
            Value::list(longs(&[150])),
        ],
    );
    assert_top(
        "360 bucketizer.mean.circular SNAPSHOT",
        "360.0 bucketizer.mean.circular ",
    );
    assert_top(
        "2.5 reducer.mean.circular.exclude-nulls SNAPSHOT",
        "2.5 reducer.mean.circular.exclude-nulls ",
    );
    for period in ["'x'", "0", "-1", "NaN", "+Infinity"] {
        assert_fails(
            &format!("{period} bucketizer.mean.circular"),
            "bucketizer.mean.circular expects a finite, strictly positive, numeric 'period' parameter.",
        );
    }
    assert_fails("bucketizer.mean.circular", "Empty stack");
}

#[test]
fn circular_mean_on_windows_with_nulls_and_mixed_types() {
    let forbid = CircularMean {
        period: 360.0,
        forbid_nulls: true,
    };
    let exclude = CircularMean {
        period: 360.0,
        forbid_nulls: false,
    };
    let with_null = Window::of(&[Value::Long(90), Value::Null, Value::Long(180)]);
    assert_eq!(with_null.apply(&forbid).unwrap(), Aggregated::none());
    assert_eq!(with_null.value(&exclude), Value::Double(135.0));
    // All nulls: a NaN mean.
    let all_null = Window::of(&[Value::Null]);
    assert!(matches!(all_null.value(&exclude), Value::Double(d) if d.is_nan()));
    assert_eq!(Window::of(&[]).apply(&exclude).unwrap(), Aggregated::none());
    assert_eq!(
        Window::of(&[Value::from("a"), Value::Long(1)])
            .apply(&exclude)
            .unwrap(),
        Aggregated::none()
    );
    assert_eq!(
        Window::of(&[Value::Long(90), Value::from("a")]).error(&exclude),
        "class java.lang.String cannot be cast to class java.lang.Number"
    );
    assert_eq!(
        Window::of(&[Value::Double(90.0), Value::Long(180)]).value(&exclude),
        Value::Double(135.0)
    );
    // The mean location and elevation of the non-null datapoints (the null's are skipped).
    let located = Window {
        ticks: vec![1, 2, 3, 4],
        locations: vec![
            crate::geoxp::to_geoxp_point(48.0, 2.0),
            crate::geoxp::to_geoxp_point(0.0, 0.0),
            NO_LOCATION,
            crate::geoxp::to_geoxp_point(49.0, 3.0),
        ],
        elevations: vec![100, 1000, NO_ELEVATION, 200],
        values: vec![Value::Long(0), Value::Null, Value::Long(0), Value::Long(0)],
    };
    let result = located.apply(&exclude).unwrap();
    assert_eq!(
        crate::geoxp::from_geoxp_point(result.location),
        (48.499_999_959_021_81, 2.499_999_934_807_419_8)
    );
    assert_eq!(result.elevation, 150);
    assert_eq!(result.value, Value::Double(0.0));
}
