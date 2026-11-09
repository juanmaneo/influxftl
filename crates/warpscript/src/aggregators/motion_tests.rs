// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the change and motion aggregators. The expected values were checked against
//! Warp 10 3.5.0 (`warp10io/warp10:3.5.0-alpine`).

use super::motion::{Kind, Motion};
use super::tests::Window;
use super::{Aggregated, Aggregator, Roles};
use crate::geoxp;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::tests::{assert_top, fail};
use crate::value::Value;

/// A located series with a datapoint without location nor elevation.
const LOCATED: &str = "NEWGTS 1000000 48.0 2.0 100 10 ADDVALUE 3000000 NaN NaN NaN 15 ADDVALUE \
    4000000 48.5 2.5 250 12 ADDVALUE 7000000 49.0 1.0 50 30 ADDVALUE \
    9000000 47.0 -1.0 250 31 ADDVALUE";
/// A `DOUBLE` series without locations.
const DOUBLES: &str = "NEWGTS 1000000 NaN NaN NaN 1.5 ADDVALUE 3000000 NaN NaN NaN 2.0 ADDVALUE \
    5000000 NaN NaN NaN 2.0 ADDVALUE 6000000 NaN NaN NaN 1.0 ADDVALUE";

/// The ticks, values, latitudes, longitudes and elevations of `series` processed by
/// `[ series mapper parameters ] framework`, as JSON.
fn processed(series: &str, mapper: &str, parameters: &str, framework: &str) -> String {
    format!(
        "[ {series} {mapper} {parameters} ] {framework} 0 GET 'r' STORE \
         $r TICKLIST $r VALUES $r LOCATIONS $r ELEVATIONS 5 ->LIST ->JSON"
    )
}

/// The ticks and values of `series` mapped by `mapper` with a pre-window of 1.
fn mapped(series: &str, mapper: &str) -> String {
    format!("[ {series} {mapper} 1 0 0 ] MAP 0 GET 'r' STORE $r TICKLIST $r VALUES 2 ->LIST ->JSON")
}

const TICKS: &str = "[1000000,3000000,4000000,7000000,9000000]";
const POSITIONS: &str = "[NaN,48.49999995902181,48.999999971129,46.99999996460974,46.99999996460974],\
    [NaN,2.4999999348074198,0.9999999403953552,-1.000000024214387,-1.000000024214387],\
    [NaN,250,50,250,250]";
const AT_TICK: &str = "[47.99999998882413,NaN,48.49999995902181,48.999999971129,46.99999996460974],\
    [1.9999999646097422,NaN,2.4999999348074198,0.9999999403953552,-1.000000024214387],\
    [100,NaN,250,50,250]";

#[test]
fn maps_of_a_located_series() {
    for (mapper, expected) in [
        (
            "mapper.delta",
            format!("[{TICKS},[5,2,15,19,1],{POSITIONS}]"),
        ),
        (
            "mapper.rate",
            format!("[{TICKS},[2.5,0.6666666666666666,3.75,3.8,0.5],{POSITIONS}]"),
        ),
        (
            "mapper.highest",
            format!(
                "[{TICKS},[10,12,12,31,31],\
                 [47.99999998882413,48.49999995902181,48.49999995902181,46.99999996460974,46.99999996460974],\
                 [1.9999999646097422,2.4999999348074198,2.4999999348074198,-1.000000024214387,-1.000000024214387],\
                 [100,250,250,250,250]]"
            ),
        ),
        (
            "mapper.lowest",
            format!(
                "[{TICKS},[10,10,30,30,30],\
                 [47.99999998882413,47.99999998882413,48.999999971129,48.999999971129,48.999999971129],\
                 [1.9999999646097422,1.9999999646097422,0.9999999403953552,0.9999999403953552,0.9999999403953552],\
                 [100,100,50,50,50]]"
            ),
        ),
        (
            "mapper.hdist",
            format!(
                "[{TICKS},[0.0,66750.62247027809,123145.73088714827,390549.1027920258,267403.3719048775],{AT_TICK}]"
            ),
        ),
        (
            "mapper.hspeed",
            format!(
                "[{TICKS},[0.0,22250.207490092696,30786.432721787067,78109.82055840516,133701.68595243874],{AT_TICK}]"
            ),
        ),
        (
            "mapper.vdist",
            format!("[{TICKS},[0.0,0.15,0.2,0.4,0.2],{AT_TICK}]"),
        ),
        (
            "mapper.vspeed",
            "[[3000000,7000000,9000000],[0.049999999999999996,0.0,0.1],\
             [NaN,48.999999971129,46.99999996460974],[NaN,0.9999999403953552,-1.000000024214387],\
             [NaN,50,250]]"
                .to_owned(),
        ),
        (
            "mapper.truecourse",
            "[[3000000,7000000,9000000],[315.0887664708421,112.45267013895736,134.7458228375483],\
             [NaN,48.999999971129,46.99999996460974],[NaN,0.9999999403953552,-1.000000024214387],\
             [NaN,50,250]]"
                .to_owned(),
        ),
    ] {
        assert_top(&processed(LOCATED, mapper, "1 1 0", "MAP"), expected);
    }
}

#[test]
fn bucketized_located_series() {
    for (mapper, values, positions) in [
        (
            "mapper.delta",
            "[1,2]",
            "[46.99999996460974,48.49999995902181],[-1.000000024214387,2.4999999348074198],[250,250]",
        ),
        (
            "mapper.rate",
            "[0.5,0.6666666666666666]",
            "[46.99999996460974,48.49999995902181],[-1.000000024214387,2.4999999348074198],[250,250]",
        ),
        (
            "mapper.highest",
            "[31,12]",
            "[46.99999996460974,48.49999995902181],[-1.000000024214387,2.4999999348074198],[250,250]",
        ),
        (
            "mapper.lowest",
            "[30,10]",
            "[48.999999971129,47.99999998882413],[0.9999999403953552,1.9999999646097422],[50,100]",
        ),
        (
            "mapper.hdist",
            "[267403.3719048775,66750.62247027809]",
            "[NaN,NaN],[NaN,NaN],[NaN,NaN]",
        ),
        (
            "mapper.hspeed",
            "[133701.68595243874,22250.207490092696]",
            "[NaN,NaN],[NaN,NaN],[NaN,NaN]",
        ),
        (
            "mapper.vdist",
            "[0.2,0.15]",
            "[NaN,NaN],[NaN,NaN],[NaN,NaN]",
        ),
        (
            "mapper.vspeed",
            "[0.1,0.049999999999999996]",
            "[NaN,NaN],[NaN,NaN],[NaN,NaN]",
        ),
        (
            "mapper.truecourse",
            "[134.7458228375483,315.0887664708421]",
            "[NaN,NaN],[NaN,NaN],[NaN,NaN]",
        ),
    ] {
        assert_top(
            &processed(LOCATED, mapper, "0 5000000 0", "BUCKETIZE"),
            format!("[[10000000,5000000],{values},{positions}]"),
        );
    }
}

#[test]
fn maps_of_doubles_without_positions() {
    for (mapper, expected) in [
        (
            "mapper.delta",
            "[[1000000,3000000,5000000,6000000],[0.0,0.5,0.0,-1.0]]",
        ),
        (
            "mapper.rate",
            "[[1000000,3000000,5000000,6000000],[0.0,0.25,0.0,-1.0]]",
        ),
        ("mapper.highest", "[[],[]]"),
        ("mapper.lowest", "[[],[]]"),
        (
            "mapper.hdist",
            "[[1000000,3000000,5000000,6000000],[0.0,0.0,0.0,0.0]]",
        ),
        (
            "mapper.hspeed",
            "[[1000000,3000000,5000000,6000000],[0.0,0.0,0.0,0.0]]",
        ),
        (
            "mapper.vdist",
            "[[1000000,3000000,5000000,6000000],[0.0,0.0,0.0,0.0]]",
        ),
        ("mapper.vspeed", "[[],[]]"),
        ("mapper.truecourse", "[[],[]]"),
    ] {
        assert_top(&mapped(DOUBLES, mapper), expected);
    }
}

#[test]
fn java_quirks() {
    // Non numeric values give no result.
    assert_top(
        &mapped(
            "NEWGTS 1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'b' ADDVALUE",
            "mapper.delta",
        ),
        "[[],[]]",
    );
    assert_top(
        &mapped(
            "NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE",
            "mapper.rate",
        ),
        "[[],[]]",
    );
    // `long` arithmetic wraps.
    assert_top(
        &mapped(
            "NEWGTS 1 NaN NaN NaN -9223372036854775808 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE",
            "mapper.delta",
        ),
        "[[1,2],[0,-9223372036854775807]]",
    );
    assert_top(
        &mapped(
            "NEWGTS 1 NaN NaN -9223372036854775807 1 ADDVALUE 2 NaN NaN 9223372036854775807 1 ADDVALUE",
            "mapper.vdist",
        ),
        "[[1,2],[0.0,0.002]]",
    );
    // A single datapoint at `Long.MAX_VALUE` has no first tick for `getFirstLastTicks`.
    for mapper in ["mapper.delta", "mapper.rate", "mapper.vspeed"] {
        let message = fail(&format!(
            "[ NEWGTS 9223372036854775807 NaN NaN NaN 1 ADDVALUE {mapper} 0 0 0 ] MAP"
        ))
        .to_string();
        assert!(
            message.contains("(Index -1 out of bounds for length 1)"),
            "{mapper}: {message}"
        );
    }
    assert_top(
        "[ NEWGTS 9223372036854775807 NaN NaN NaN 'a' ADDVALUE mapper.delta 0 0 0 ] MAP 0 GET VALUES",
        Value::list(vec![]),
    );
}

fn aggregator(kind: Kind) -> Motion {
    Motion { kind }
}

#[test]
fn windows_frameworks_cannot_produce() {
    let all = [
        Kind::Delta,
        Kind::Rate,
        Kind::Highest,
        Kind::Lowest,
        Kind::HDist,
        Kind::HSpeed,
        Kind::VDist,
        Kind::VSpeed,
        Kind::TrueCourse,
    ];
    for kind in all {
        assert_eq!(
            Window::of(&[])
                .apply(&aggregator(kind))
                .expect("apply should succeed"),
            Aggregated::none()
        );
        assert_eq!(aggregator(kind).roles(), Roles::ALL);
    }
    // Reducer windows: `null`s and unsorted ticks.
    let window = Window {
        ticks: vec![5, 1, 9],
        locations: vec![NO_LOCATION; 3],
        elevations: vec![NO_ELEVATION; 3],
        values: vec![Value::Long(3), Value::Null, Value::Long(10)],
    };
    assert_eq!(
        window.error(&aggregator(Kind::Delta)),
        "NullPointerException"
    );
    let window = Window {
        values: vec![Value::Long(3), Value::string("a"), Value::Double(10.5)],
        ..window
    };
    assert_eq!(
        window.error(&aggregator(Kind::Rate)),
        "class java.lang.String cannot be cast to class java.lang.Number"
    );
    let window = Window {
        values: vec![Value::Long(3), Value::Double(1.5), Value::Long(10)],
        ..window
    };
    assert_eq!(window.value(&aggregator(Kind::Delta)), Value::Long(9));
    assert_eq!(
        window
            .apply(&aggregator(Kind::Rate))
            .expect("apply should succeed"),
        Aggregated {
            tick: 8,
            location: NO_LOCATION,
            elevation: NO_ELEVATION,
            value: Value::Double(9.0 / 8e-6),
        }
    );
    // Equal ticks: the rate is the plain delta; elevations equal: the latest is the highest,
    // the earliest the lowest.
    let window = Window {
        ticks: vec![4, 4],
        locations: vec![1, 2],
        elevations: vec![7, 7],
        values: vec![Value::Double(1.0), Value::Double(3.0)],
    };
    assert_eq!(window.value(&aggregator(Kind::Rate)), Value::Double(0.0));
    assert_eq!(
        window
            .apply(&aggregator(Kind::Highest))
            .expect("apply should succeed")
            .location,
        1
    );
    assert_eq!(
        window
            .apply(&aggregator(Kind::Lowest))
            .expect("apply should succeed")
            .location,
        1
    );
    let window = Window {
        ticks: vec![4, 6],
        ..window
    };
    assert_eq!(
        window
            .apply(&aggregator(Kind::Highest))
            .expect("apply should succeed")
            .location,
        2
    );
    assert_eq!(
        window
            .apply(&aggregator(Kind::Lowest))
            .expect("apply should succeed")
            .location,
        1
    );
    // `Lowest` never picks a datapoint at `Long.MAX_VALUE` of elevation `Long.MAX_VALUE`.
    let window = Window {
        ticks: vec![i64::MAX],
        locations: vec![1],
        elevations: vec![i64::MAX],
        values: vec![Value::Long(1)],
    };
    assert_eq!(
        window
            .apply(&aggregator(Kind::Lowest))
            .expect("apply should succeed"),
        Aggregated::none()
    );
    assert_eq!(
        window
            .apply(&aggregator(Kind::Highest))
            .expect("apply should succeed")
            .value,
        Value::Long(1)
    );
    // A tick at `Long.MIN_VALUE` alone has no last tick.
    let window = Window {
        ticks: vec![i64::MIN],
        ..window
    };
    for kind in [Kind::Delta, Kind::Rate, Kind::VSpeed] {
        assert_eq!(
            window.error(&aggregator(kind)),
            "Index -1 out of bounds for length 1"
        );
    }
    let window = Window {
        ticks: vec![i64::MAX],
        values: vec![Value::Double(1.0)],
        ..window
    };
    assert_eq!(
        window.error(&aggregator(Kind::Delta)),
        "Index -1 out of bounds for length 1"
    );
}

#[test]
fn distances_and_courses() {
    let paris = geoxp::to_geoxp_point(48.0, 2.0);
    let east = geoxp::to_geoxp_point(48.0, 3.0);
    // The longitude delta the shorter way round the antimeridian.
    let west = geoxp::to_geoxp_point(0.0, -179.5);
    let far_east = geoxp::to_geoxp_point(0.0, 179.5);
    assert!((geoxp::loxodromic_distance(west, far_east) - 111_120.0).abs() < 1.0);
    assert!(geoxp::loxodromic_distance(paris, east) > 74_000.0);
    // Williams' formula takes the longitudes positive westwards: heading east gives 270.
    for (from, to, expected) in [
        ("48.0 2.0", "48.0 3.0", "[0.0,270.2489764400462]"),
        ("48.0 3.0", "48.0 2.0", "[0.0,89.75121321694503]"),
    ] {
        assert_top(
            &format!(
                "[ NEWGTS 1 {from} NaN 1 ADDVALUE 2 {to} NaN 1 ADDVALUE mapper.truecourse 1 0 0 ] \
                 MAP 0 GET VALUES ->JSON"
            ),
            expected,
        );
    }
    assert_top("mapper.delta SNAPSHOT", "'mapper.delta' FUNCREF ");
}
