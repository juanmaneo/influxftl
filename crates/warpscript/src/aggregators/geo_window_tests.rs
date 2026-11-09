// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The mappers of locations applied directly to the windows `MAP` cannot produce: empty ones
//! and `null` values.

use std::rc::Rc;

use super::Aggregated;
use super::geo::{Approximate, Check, ShapeMapper};
use super::tests::Window;
use crate::error::Error;
use crate::error::fault::BoundsError;
use crate::geoshape::GeoShape;
use crate::geoxp::{hhcode_center, to_geocell, to_geoxp_point};
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::value::Value;

fn paris() -> i64 {
    to_geoxp_point(48.85, 2.35)
}

/// The shape of the cell of resolution 2 holding Paris.
fn around_paris() -> Rc<GeoShape> {
    Rc::new(GeoShape::from_cells(vec![to_geocell(paris(), 2)]))
}

fn shape_mapper(check: Check) -> ShapeMapper {
    ShapeMapper {
        name: "mapper.geo.x",
        check,
        shape: around_paris(),
    }
}

/// A window of one datapoint (tick 1) at `location`, with an elevation of 7.
fn located(location: i64, value: Value) -> Window {
    Window {
        locations: vec![location],
        elevations: vec![7],
        ..Window::of(&[value])
    }
}

fn array_index() -> Error {
    BoundsError::ArrayIndex {
        index: 0,
        length: 0,
    }
    .into()
}

#[test]
fn an_empty_window_approximates_nothing() {
    let mapper = Approximate {
        name: "mapper.geo.approximate",
        resolution: 4,
    };
    assert_eq!(
        Window::of(&[]).apply(&mapper).expect_err("empty window"),
        array_index()
    );
}

#[test]
fn a_window_without_position_fails() {
    let window = Window {
        locations: vec![],
        elevations: vec![],
        ..Window::of(&[Value::Long(1)])
    };
    for check in [Check::Within, Check::Outside, Check::Fence] {
        assert_eq!(
            window.apply(&shape_mapper(check)).expect_err("no position"),
            array_index()
        );
    }
}

#[test]
fn an_approximation_keeps_the_elevation_and_the_first_value() {
    let mapper = Approximate {
        name: "mapper.geo.approximate",
        resolution: 4,
    };
    assert_eq!(
        located(paris(), Value::Long(5))
            .apply(&mapper)
            .expect("apply should succeed"),
        Aggregated {
            tick: 100,
            location: hhcode_center(paris(), 4),
            elevation: 7,
            value: Value::Long(5),
        }
    );
    assert_eq!(
        located(NO_LOCATION, Value::Long(5))
            .apply(&mapper)
            .expect("apply should succeed"),
        Aggregated {
            tick: 100,
            location: NO_LOCATION,
            elevation: 7,
            value: Value::Null,
        }
    );
    // The datapoint has a location, but the window no value.
    let valueless = Window {
        values: vec![],
        ..located(paris(), Value::Null)
    };
    assert_eq!(
        valueless.apply(&mapper).expect("apply should succeed"),
        Aggregated {
            tick: 100,
            location: paris(),
            elevation: 7,
            value: Value::Null,
        }
    );
}

#[test]
fn an_empty_window_keeps_its_tick() {
    for check in [Check::Within, Check::Outside, Check::Fence] {
        assert_eq!(
            Window::of(&[])
                .apply(&shape_mapper(check))
                .expect("apply should succeed"),
            Aggregated {
                tick: 100,
                location: NO_LOCATION,
                elevation: NO_ELEVATION,
                value: Value::Null,
            }
        );
    }
}

#[test]
fn a_null_value_is_never_reported() {
    for (check, location, expected) in [
        (Check::Within, paris(), Value::Null),
        (Check::Outside, to_geoxp_point(-10.0, -10.0), Value::Null),
        (Check::Fence, paris(), Value::Boolean(true)),
        (
            Check::Fence,
            to_geoxp_point(-10.0, -10.0),
            Value::Boolean(false),
        ),
    ] {
        let result = located(location, Value::Null)
            .apply(&shape_mapper(check))
            .expect("apply should succeed");
        assert_eq!(result.value, expected, "{check:?}");
        assert_eq!(result.location, location);
        assert_eq!(result.elevation, 7);
    }
}

#[test]
fn a_value_is_reported_on_the_right_side_of_the_shape() {
    let outside = to_geoxp_point(-10.0, -10.0);
    for (check, location, expected) in [
        (Check::Within, paris(), Value::Long(5)),
        (Check::Within, outside, Value::Null),
        (Check::Within, NO_LOCATION, Value::Null),
        (Check::Outside, paris(), Value::Null),
        (Check::Outside, outside, Value::Long(5)),
        (Check::Outside, NO_LOCATION, Value::Null),
        (Check::Fence, NO_LOCATION, Value::Null),
    ] {
        let result = located(location, Value::Long(5))
            .apply(&shape_mapper(check))
            .expect("apply should succeed");
        assert_eq!(result.value, expected, "{check:?} {location}");
        assert_eq!(result.tick, 100);
    }
}
