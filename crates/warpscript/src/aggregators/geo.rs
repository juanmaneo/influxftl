// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The mappers of locations: `mapper.geo.approximate` moves a location to the center of its
//! cell, `mapper.geo.within`, `mapper.geo.outside` and `mapper.geo.fence` check it against a
//! `GEOSHAPE`.

use std::rc::Rc;

use super::{Aggregated, Aggregator, AggregatorArgs, Named, Roles};
use crate::error::fault::BoundsError;
use crate::error::{Error, Result};
use crate::functions::Entry;
use crate::geoshape::GeoShape;
use crate::geoxp::hhcode_center;
use crate::gts::NO_LOCATION;
use crate::snapshot::value_snapshot;
use crate::stack::Stack;
use crate::value::Value;

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "mapper.geo.approximate",
        Entry::Builder(approximate_builder),
    ),
    (
        "mapper.geo.within",
        Entry::Builder(|s, n| {
            shape_builder(
                s,
                n,
                Check::Within,
                "expects a Geo Shape on top of the stack.",
            )
        }),
    ),
    (
        "mapper.geo.outside",
        Entry::Builder(|s, n| {
            shape_builder(
                s,
                n,
                Check::Outside,
                "expects a Geo Shape on top of the stack.",
            )
        }),
    ),
    (
        "mapper.geo.fence",
        Entry::Builder(|s, n| shape_builder(s, n, Check::Fence, "expects a Geo Shape.")),
    ),
];

/// The location and elevation of the first datapoint of a window.
///
/// # Errors
///
/// [`BoundsError::ArrayIndex`] for a window without datapoint, as the Java reads the first
/// element of its arrays.
fn first_position(args: &AggregatorArgs) -> Result<(i64, i64)> {
    match (args.locations.first(), args.elevations.first()) {
        (Some(location), Some(elevation)) => Ok((*location, *elevation)),
        _ => Err(BoundsError::ArrayIndex {
            index: 0,
            length: 0,
        }
        .into()),
    }
}

/// `MapperGeoApproximate`: the location of the datapoint moved to the center of its cell.
pub struct Approximate {
    /// The registered name, for the snapshot.
    pub name: &'static str,
    /// The resolution of the cell, even from 2 (coarsest) to 32 (finest).
    pub resolution: i32,
}

impl Aggregator for Approximate {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let (location, elevation) = first_position(args)?;
        // Without a location (or a value), the datapoint has nothing to report.
        Ok(match args.values.first() {
            Some(value) if location != NO_LOCATION => Aggregated {
                tick: args.tick,
                location: hhcode_center(location, self.resolution),
                elevation,
                value: value.clone(),
            },
            _ => Aggregated {
                tick: args.tick,
                location,
                elevation,
                value: Value::Null,
            },
        })
    }

    fn parameters(&self) -> Option<String> {
        Some(self.resolution.to_string())
    }
}

/// `Number.intValue()`: a `double` saturates, an integer keeps its low 32 bits.
fn int_value(value: &Value) -> Option<i32> {
    match value {
        Value::Double(d) => Some(*d as i32),
        other => other.as_i64().map(|l| l as i32),
    }
}

/// `MapperGeoApproximate.Builder`: `resolution mapper.geo.approximate`.
fn approximate_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let message = "expects an even resolution between 2 (coarsest) and 32 (finest).";
    let resolution = int_value(&stack.pop()?)
        .filter(|resolution| (2..=32).contains(resolution) && resolution % 2 == 0)
        .ok_or_else(|| Error::usage(name, message))?;
    stack.push(Value::Function(Named::object(
        name,
        Approximate { name, resolution },
    )));
    Ok(())
}

/// What a [`ShapeMapper`] does with a datapoint located in or out of its shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// `MapperGeoWithin`: the value of the datapoints located in the shape.
    Within,
    /// `MapperGeoOutside`: the value of the datapoints located out of the shape.
    Outside,
    /// `MapperGeoFence`: whether the datapoint is located in the shape.
    Fence,
}

/// `MapperGeoWithin`, `MapperGeoOutside` and `MapperGeoFence`: check the location of the
/// datapoint against a shape.
pub struct ShapeMapper {
    /// The registered name, for the snapshot.
    pub name: &'static str,
    /// What to report.
    pub check: Check,
    /// The shape.
    pub shape: Rc<GeoShape>,
}

impl Aggregator for ShapeMapper {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        // An empty window keeps the tick, but nothing else.
        let Some(value) = args.values.first() else {
            return Ok(Aggregated {
                tick: args.tick,
                ..Aggregated::none()
            });
        };
        let (location, elevation) = first_position(args)?;
        let value = if location == NO_LOCATION {
            Value::Null
        } else {
            let contained = self.shape.contains_point(location);
            let reported = !matches!(value, Value::Null);
            match self.check {
                Check::Fence => Value::Boolean(contained),
                Check::Within if contained && reported => value.clone(),
                Check::Outside if !contained && reported => value.clone(),
                Check::Within | Check::Outside => Value::Null,
            }
        };
        Ok(Aggregated {
            tick: args.tick,
            location,
            elevation,
            value,
        })
    }

    fn parameters(&self) -> Option<String> {
        let packed = value_snapshot(&Value::from(self.shape.pack()));
        Some(format!("{} GEOUNPACK", packed.trim_end()))
    }
}

/// The builders of the [`ShapeMapper`]s: `shape mapper.geo.within`.
fn shape_builder(stack: &mut Stack, name: &'static str, check: Check, message: &str) -> Result<()> {
    let Value::GeoShape(shape) = stack.pop()? else {
        return Err(Error::usage(name, message));
    };
    stack.push(Value::Function(Named::object(
        name,
        ShapeMapper { name, check, shape },
    )));
    Ok(())
}
