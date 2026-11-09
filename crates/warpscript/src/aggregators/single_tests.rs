// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the single datapoint mappers. The expected values were checked against Warp 10
//! 3.5.0 (`warp10io/warp10:3.5.0-alpine`).

use indexmap::IndexMap;

use super::single::{DateTime, Field, Kind, Single, TimeZone};
use super::tests::{Window, doubles, longs, series};
use super::{Aggregated, Aggregator, AggregatorArgs};
use crate::functions::time::Zone;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::tests::{assert_fails, assert_top};
use crate::value::Value;

const ALL_KINDS: [Kind; 16] = [
    Kind::Abs,
    Kind::Ceil,
    Kind::Floor,
    Kind::Round,
    Kind::Tanh,
    Kind::Sigmoid,
    Kind::Finite,
    Kind::ToBoolean,
    Kind::ToLong,
    Kind::ToDouble,
    Kind::ToString,
    Kind::Latitude,
    Kind::Longitude,
    Kind::Elevation,
    Kind::GeoClear,
    Kind::Tick,
];

fn mapper(kind: Kind) -> Single {
    Single {
        name: "mapper.x",
        kind,
    }
}

/// The value `kind` maps `value` to.
fn mapped(kind: Kind, value: Value) -> Value {
    Window::of(&[value]).value(&mapper(kind))
}

/// The values of the series of `values` mapped by `mapper`.
fn map_values(values: &str, mapper: &str) -> String {
    format!("[ {} {mapper} 0 0 0 ] MAP 0 GET VALUES", series(values))
}

fn strings(values: &[&str]) -> Vec<Value> {
    values.iter().map(Value::string).collect()
}

#[test]
fn empty_and_larger_windows() {
    let year = DateTime {
        zone: TimeZone::Zone(Zone::UTC),
        field: Field::Year,
        name: "mapper.x",
    };
    let mappers: Vec<&dyn Aggregator> = vec![&year];
    let singles: Vec<Single> = ALL_KINDS.into_iter().map(mapper).collect();
    for mapper in mappers
        .into_iter()
        .chain(singles.iter().map(|s| s as &dyn Aggregator))
    {
        assert_eq!(
            Window::of(&[]).apply(mapper).unwrap(),
            Aggregated {
                tick: 0,
                ..Aggregated::none()
            }
        );
        assert_eq!(
            Window::of(&longs(&[1, 2])).error(mapper),
            "mapper.x can only be applied to a single value."
        );
    }
    assert_fails(
        "[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE mapper.abs 1 0 0 ] MAP",
        "MAP was given invalid parameters. (mapper.abs can only be applied to a single value.)",
    );
    assert_fails(
        &format!("[ {} mapper.abs 0 10 0 ] BUCKETIZE", series("1")),
        "BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
    );
    assert_fails(
        &format!("[ {} 'UTC' mapper.year 0 10 0 ] BUCKETIZE", series("1")),
        "BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
    );
}

#[test]
fn arithmetic_mappers() {
    assert_top(
        &map_values("-3 5", "mapper.abs"),
        Value::list(longs(&[3, 5])),
    );
    assert_top(
        &map_values("-2.5 1.5", "mapper.abs"),
        Value::list(doubles(&[2.5, 1.5])),
    );
    assert_eq!(
        mapped(Kind::Abs, Value::Long(i64::MIN)),
        Value::Long(i64::MIN)
    );
    assert_top(
        &map_values("1 -2", "mapper.ceil"),
        Value::list(doubles(&[1.0, -2.0])),
    );
    assert_top(
        &map_values("1.5 -0.5", "mapper.ceil"),
        Value::list(doubles(&[2.0, -0.0])),
    );
    assert!(
        mapped(Kind::Ceil, Value::Double(-0.5))
            .as_f64()
            .unwrap()
            .is_sign_negative()
    );
    assert_top(
        &map_values("1.5 -1.5", "mapper.floor"),
        Value::list(doubles(&[1.0, -2.0])),
    );
    assert_top(
        &map_values("3", "mapper.floor"),
        Value::list(doubles(&[3.0])),
    );
    for (value, expected) in [
        (Value::Long(123_456_789), 123_456_792),
        (Value::Long(i64::MAX), 2_147_483_647),
        (Value::Long(i64::MIN), -2_147_483_648),
        (Value::Double(2.5), 3),
        (Value::Double(-2.5), -2),
        (Value::Double(f64::NAN), 0),
        (Value::Double(f64::INFINITY), i64::MAX),
    ] {
        assert_eq!(mapped(Kind::Round, value), Value::Long(expected));
    }
    assert_eq!(mapped(Kind::Tanh, Value::Long(0)), Value::Double(0.0));
    assert_eq!(
        mapped(Kind::Tanh, Value::Double(1.0)),
        Value::Double(1.0f64.tanh())
    );
    assert_eq!(mapped(Kind::Sigmoid, Value::Long(0)), Value::Double(0.5));
    assert_eq!(
        mapped(Kind::Sigmoid, Value::Double(2.0)),
        Value::Double(1.0 / (1.0 + (-2.0f64).exp()))
    );
    for kind in [
        Kind::Abs,
        Kind::Ceil,
        Kind::Floor,
        Kind::Round,
        Kind::Tanh,
        Kind::Sigmoid,
    ] {
        for value in [Value::string("a"), Value::Boolean(true)] {
            assert_eq!(
                Window::of(&[value]).error(&mapper(kind)),
                "mapper.x can only be applied to LONG or DOUBLE values."
            );
        }
    }
    assert_fails(
        &map_values("'a'", "mapper.abs"),
        "MAP was given invalid parameters. (mapper.abs can only be applied to LONG or DOUBLE values.)",
    );
}

#[test]
fn finite_mapper() {
    assert_eq!(mapped(Kind::Finite, Value::Double(f64::NAN)), Value::Null);
    assert_eq!(
        mapped(Kind::Finite, Value::Double(f64::NEG_INFINITY)),
        Value::Null
    );
    assert_eq!(mapped(Kind::Finite, Value::Double(1.5)), Value::Double(1.5));
    assert_eq!(mapped(Kind::Finite, Value::Long(3)), Value::Long(3));
    assert_eq!(mapped(Kind::Finite, Value::string("a")), Value::string("a"));
    assert_top(
        &map_values("1.0 NaN Infinity 2.0", "mapper.finite"),
        Value::list(doubles(&[1.0, 2.0])),
    );
}

#[test]
fn conversion_mappers() {
    for (value, expected) in [
        (Value::Long(0), false),
        (Value::Long(-3), true),
        (Value::Double(0.0), false),
        (Value::Double(-0.0), false),
        (Value::Double(f64::NAN), true),
        (Value::Double(0.5), true),
        (Value::Boolean(true), true),
        (Value::Boolean(false), false),
        (Value::string(""), false),
        (Value::string("false"), true),
    ] {
        assert_eq!(
            mapped(Kind::ToBoolean, value.clone()),
            Value::Boolean(expected),
            "{value:?}"
        );
    }
    for (value, expected) in [
        (Value::Long(7), Value::Long(7)),
        (Value::Double(-2.9), Value::Long(-2)),
        (Value::Double(f64::NAN), Value::Long(0)),
        (Value::Double(1e300), Value::Long(i64::MAX)),
        (Value::Boolean(true), Value::Long(1)),
        (Value::Boolean(false), Value::Long(0)),
        (Value::string("+5"), Value::Long(5)),
        (Value::string("-12"), Value::Long(-12)),
        (Value::string(" 5"), Value::Null),
        (Value::string("0x10"), Value::Null),
        (Value::string("1.5"), Value::Null),
    ] {
        assert_eq!(mapped(Kind::ToLong, value.clone()), expected, "{value:?}");
    }
    for (value, expected) in [
        (Value::Long(7), Value::Double(7.0)),
        (Value::Double(-2.5), Value::Double(-2.5)),
        (Value::Boolean(true), Value::Double(1.0)),
        (Value::Boolean(false), Value::Double(0.0)),
        (Value::string(" 5"), Value::Double(5.0)),
        (Value::string("+5"), Value::Double(5.0)),
        (Value::string("1e3"), Value::Double(1000.0)),
        (Value::string("-Infinity"), Value::Double(f64::NEG_INFINITY)),
        (Value::string("1d"), Value::Double(1.0)),
        (Value::string(" 2.5 "), Value::Double(2.5)),
        (Value::string("abc"), Value::Null),
        (Value::string(""), Value::Null),
        (Value::string("0x10"), Value::Null),
        (Value::string("nan"), Value::Null),
    ] {
        assert_eq!(mapped(Kind::ToDouble, value.clone()), expected, "{value:?}");
    }
    assert!(
        mapped(Kind::ToDouble, Value::string("NaN"))
            .as_f64()
            .unwrap()
            .is_nan()
    );
    for (value, expected) in [
        (Value::Double(-2.5), "-2.5"),
        (Value::Double(-0.0), "-0.0"),
        (Value::Double(f64::NAN), "NaN"),
        (Value::Double(f64::INFINITY), "Infinity"),
        (Value::Double(1e21), "1.0E21"),
        (Value::Boolean(true), "true"),
        (Value::Long(-4), "-4"),
        (Value::string("s"), "s"),
    ] {
        assert_eq!(mapped(Kind::ToString, value), Value::string(expected));
    }
    // Values a series cannot hold give no result.
    for kind in [
        Kind::ToBoolean,
        Kind::ToLong,
        Kind::ToDouble,
        Kind::ToString,
    ] {
        assert_eq!(mapped(kind, Value::Null), Value::Null);
    }
    for kind in [Kind::ToBoolean, Kind::ToLong, Kind::ToDouble] {
        assert_eq!(mapped(kind, Value::list(vec![])), Value::Null);
    }
    // Each type in its own series: a series converts its values to the type of the first.
    assert_top(
        &map_values("'a' ''", "mapper.toboolean"),
        Value::list(vec![Value::Boolean(true), Value::Boolean(false)]),
    );
    assert_top(
        &map_values("'+5' ' 5' '-12'", "mapper.tolong"),
        Value::list(longs(&[5, -12])),
    );
    assert_top(
        &map_values("'1d' 'abc' '1e3'", "mapper.todouble"),
        Value::list(doubles(&[1.0, 1000.0])),
    );
    assert_top(
        &map_values("1.0E21 -0.0", "mapper.tostring"),
        Value::list(strings(&["1.0E21", "-0.0"])),
    );
}

/// The ticks, values, latitudes, longitudes and elevations of a located series mapped by
/// `mapper`, as JSON.
fn located(mapper: &str) -> String {
    format!(
        "NEWGTS 1 48.0 2.0 100 7 ADDVALUE 2 NaN NaN NaN 8 ADDVALUE 3 NaN NaN 5 9 ADDVALUE 'g' STORE \
         [ $g {mapper} 0 0 0 ] MAP 0 GET 'r' STORE \
         $r TICKLIST $r VALUES $r LOCATIONS $r ELEVATIONS 5 ->LIST ->JSON"
    )
}

#[test]
fn geo_and_tick_mappers() {
    for (mapper, expected) in [
        ("mapper.lat", "[[1],[47.99999998882413],[NaN],[NaN],[NaN]]"),
        ("mapper.lon", "[[1],[1.9999999646097422],[NaN],[NaN],[NaN]]"),
        (
            "mapper.elev",
            "[[1,3],[100,5],[NaN,NaN],[NaN,NaN],[NaN,NaN]]",
        ),
        (
            "mapper.geo.clear",
            "[[1,2,3],[7,8,9],[NaN,NaN,NaN],[NaN,NaN,NaN],[NaN,NaN,NaN]]",
        ),
        (
            "mapper.tick",
            "[[1,2,3],[1,2,3],[47.99999998882413,NaN,NaN],[1.9999999646097422,NaN,NaN],[100,NaN,5]]",
        ),
    ] {
        assert_top(&located(mapper), expected);
    }
}

#[test]
fn registration() {
    for (name, entry) in super::single::FUNCTIONS {
        let parameter = if matches!(entry, crate::functions::Entry::Builder(_)) {
            "'UTC'"
        } else {
            ""
        };
        assert_top(&format!("{parameter} {name} TYPEOF"), "AGGREGATOR");
        let snapshot = if parameter.is_empty() {
            format!("'{name}' FUNCREF ")
        } else {
            format!("'UTC' {name} ")
        };
        assert_top(&format!("{parameter} {name} SNAPSHOT"), snapshot);
        let roles = crate::tests::top(&format!("{parameter} {name}"));
        let roles = super::aggregator_of(&roles).unwrap().roles();
        assert!(
            roles.mapper && !roles.bucketizer && !roles.reducer,
            "{name}"
        );
    }
}

/// `<timezone> <mapper>` applied to a series with datapoints at the ticks below.
fn date_time(timezone: &str, mapper: &str) -> String {
    format!(
        "NEWGTS -86400000001 NaN NaN NaN 'x' ADDVALUE -1500 NaN NaN NaN 'x' ADDVALUE \
         1609459199999999 NaN NaN NaN 'x' ADDVALUE 1609459200000000 NaN NaN NaN 'x' ADDVALUE \
         1625097600123456 NaN NaN NaN 'x' ADDVALUE \
         'g' STORE [ $g {timezone} {mapper} 0 0 0 ] MAP 0 GET VALUES"
    )
}

#[test]
fn date_time_mappers() {
    for (mapper, expected) in [
        ("mapper.year", [1969, 1969, 2020, 2021, 2021]),
        ("mapper.month", [12, 12, 12, 1, 7]),
        ("mapper.day", [31, 31, 31, 1, 1]),
        ("mapper.weekday", [3, 3, 4, 5, 4]),
        ("mapper.hour", [0, 23, 23, 0, 0]),
        ("mapper.minute", [0, 59, 59, 0, 0]),
    ] {
        assert_top(&date_time("'UTC'", mapper), Value::list(longs(&expected)));
    }
    assert_top(
        &date_time("'UTC'", "mapper.second"),
        Value::list(doubles(&[-1.0E-6, 58.9985, 59.999_999, 0.0, 0.123_456])),
    );
    assert_top(
        &date_time("'Europe/Paris'", "mapper.hour"),
        Value::list(longs(&[1, 0, 0, 1, 2])),
    );
    assert_top(
        &date_time("-5400000", "mapper.hour"),
        Value::list(longs(&[22, 22, 22, 22, 22])),
    );
    assert_top(
        &date_time("-5400000", "mapper.minute"),
        Value::list(longs(&[30, 29, 29, 30, 30])),
    );
    assert_top(
        &date_time("1.9", "mapper.hour"),
        Value::list(longs(&[0, 0, 0, 0, 0])),
    );
    assert_top(
        &date_time("1.9", "mapper.second"),
        Value::list(doubles(&[-1.0E-6, -0.0015, 0.999_999, 0.0, 0.123_456])),
    );
    // The location and elevation of the datapoint are kept.
    assert_top(
        "[ NEWGTS 1 48.0 2.0 100 'x' ADDVALUE 0 mapper.year 0 0 0 ] MAP 0 GET ELEVATIONS",
        Value::list(longs(&[100])),
    );
}

#[test]
fn date_time_builders() {
    for (timezone, snapshot) in [
        ("'UTC'", "'UTC'"),
        ("0", "'UTC'"),
        ("'+00:00'", "'UTC'"),
        ("3600000", "'%2B01%3A00'"),
        ("3600500", "'%2B01%3A00%3A00.500'"),
        ("-86399999", "'-23%3A59%3A59.999'"),
        ("3601000.7", "'%2B01%3A00%3A01'"),
        ("'+0100'", "'%2B01%3A00'"),
        ("'Europe/Paris'", "'Europe%2FParis'"),
    ] {
        assert_top(
            &format!("{timezone} mapper.year SNAPSHOT"),
            format!("{snapshot} mapper.year "),
        );
    }
    assert_fails(
        "'utc' mapper.year",
        "The datetime zone id 'utc' is not recognised",
    );
    for (timezone, millis) in [
        ("86400000", "86400000"),
        ("-86400000", "-86400000"),
        ("9999999999", "1410065407"),
        ("1e30", "2147483647"),
    ] {
        assert_fails(
            &format!("{timezone} mapper.year"),
            &format!("(Millis out of range: {millis})"),
        );
    }
    let err = TimeZone::for_offset_millis(i32::MIN).unwrap_err();
    assert_eq!(
        crate::error::compat::exception_class(&err),
        "java.lang.IllegalArgumentException"
    );
    assert_eq!(err.message(), "Millis out of range: -2147483648");
    assert_top("NaN mapper.year SNAPSHOT", "'UTC' mapper.year ");
    assert_fails(
        "[ 'a' ] mapper.year",
        "mapper.year expects a STRING timezone or a NUMBER millisecond offset.",
    );
    assert_fails("mapper.year", "Empty stack");
}

/// Warp 10 (Joda) supports instants beyond the years `chrono` represents; here they are an
/// error.
#[test]
fn date_time_out_of_range() {
    for zone in [TimeZone::Zone(Zone::UTC), TimeZone::Offset(1)] {
        let mapper = DateTime {
            zone,
            field: Field::Year,
            name: "mapper.year",
        };
        let names = ["g".to_owned()];
        let labels = [IndexMap::new()];
        let args = AggregatorArgs {
            tick: i64::MAX,
            names: &names,
            labels: &labels,
            ticks: &[i64::MAX],
            locations: &[NO_LOCATION],
            elevations: &[NO_ELEVATION],
            values: &[Value::Long(1)],
            window: None,
        };
        assert!(
            mapper
                .apply(&args)
                .unwrap_err()
                .to_string()
                .contains("out of range"),
        );
    }
}
