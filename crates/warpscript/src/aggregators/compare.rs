// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `CompareTo`, built by the `MapperCompareTo` functions (`5 mapper.gt`, `'a' mapper.eq`,
//! `1000 mapper.le.tick`, `48.5 mapper.ge.lat`…): the earliest datapoint of the window whose
//! value, tick, location or elevation compares as requested with a reference.

use std::cmp::Ordering;
use std::rc::Rc;

use super::{Aggregated, Aggregator, AggregatorArgs, Named, Num, Roles};
use crate::error::{Error, Result};
use crate::functions::Entry;
use crate::functions::functional::double_compare;
use crate::geoxp;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::snapshot::value_snapshot;
use crate::stack::Stack;
use crate::value::{Value, double_to_long};

/// A registration of a `MapperCompareTo` builder.
macro_rules! compare_to {
    ($name:literal, $compared:ident, $comparison:ident) => {
        (
            $name,
            Entry::Builder(|s, n| {
                compare_builder(s, n, Compared::$compared, Comparison::$comparison)
            }),
        )
    };
}

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    compare_to!("mapper.gt", Value, Gt),
    compare_to!("mapper.ge", Value, Ge),
    compare_to!("mapper.eq", Value, Eq),
    compare_to!("mapper.ne", Value, Ne),
    compare_to!("mapper.le", Value, Le),
    compare_to!("mapper.lt", Value, Lt),
    compare_to!("mapper.gt.tick", Tick, Gt),
    compare_to!("mapper.ge.tick", Tick, Ge),
    compare_to!("mapper.eq.tick", Tick, Eq),
    compare_to!("mapper.ne.tick", Tick, Ne),
    compare_to!("mapper.le.tick", Tick, Le),
    compare_to!("mapper.lt.tick", Tick, Lt),
    compare_to!("mapper.gt.lat", Lat, Gt),
    compare_to!("mapper.ge.lat", Lat, Ge),
    compare_to!("mapper.eq.lat", Lat, Eq),
    compare_to!("mapper.ne.lat", Lat, Ne),
    compare_to!("mapper.le.lat", Lat, Le),
    compare_to!("mapper.lt.lat", Lat, Lt),
    compare_to!("mapper.gt.lon", Lon, Gt),
    compare_to!("mapper.ge.lon", Lon, Ge),
    compare_to!("mapper.eq.lon", Lon, Eq),
    compare_to!("mapper.ne.lon", Lon, Ne),
    compare_to!("mapper.le.lon", Lon, Le),
    compare_to!("mapper.lt.lon", Lon, Lt),
    compare_to!("mapper.gt.hhcode", HhCode, Gt),
    compare_to!("mapper.ge.hhcode", HhCode, Ge),
    compare_to!("mapper.eq.hhcode", HhCode, Eq),
    compare_to!("mapper.ne.hhcode", HhCode, Ne),
    compare_to!("mapper.le.hhcode", HhCode, Le),
    compare_to!("mapper.lt.hhcode", HhCode, Lt),
    compare_to!("mapper.gt.elev", Elev, Gt),
    compare_to!("mapper.ge.elev", Elev, Ge),
    compare_to!("mapper.eq.elev", Elev, Eq),
    compare_to!("mapper.ne.elev", Elev, Ne),
    compare_to!("mapper.le.elev", Elev, Le),
    compare_to!("mapper.lt.elev", Elev, Lt),
];

/// What a `CompareTo` compares (`CompareTo.Compared`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compared {
    /// The tick.
    Tick,
    /// The location as a raw HHCode.
    HhCode,
    /// The latitude of the location.
    Lat,
    /// The longitude of the location.
    Lon,
    /// The elevation.
    Elev,
    /// The value.
    Value,
}

/// The comparison of a `CompareTo` (`CompareTo.Comparison`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    /// Greater than the reference.
    Gt,
    /// Greater than or equal to the reference.
    Ge,
    /// Equal to the reference.
    Eq,
    /// Not equal to the reference.
    Ne,
    /// Less than or equal to the reference.
    Le,
    /// Less than the reference.
    Lt,
}

impl Comparison {
    /// Whether `ordering` (of the compared item relative to the reference) satisfies the
    /// comparison.
    fn holds(self, ordering: Ordering) -> bool {
        match self {
            Comparison::Gt => ordering == Ordering::Greater,
            Comparison::Ge => ordering != Ordering::Less,
            Comparison::Eq => ordering == Ordering::Equal,
            Comparison::Ne => ordering != Ordering::Equal,
            Comparison::Le => ordering != Ordering::Greater,
            Comparison::Lt => ordering == Ordering::Less,
        }
    }
}

/// The compared item and the reference it is compared with, checked by the builder.
#[derive(Clone, Debug, PartialEq)]
pub enum Test {
    /// The tick, against a `LONG`.
    Tick(i64),
    /// The HHCode, against a `LONG`.
    HhCode(i64),
    /// The elevation, against a `LONG`.
    Elevation(i64),
    /// The latitude, against a number.
    Latitude(f64),
    /// The longitude, against a number.
    Longitude(f64),
    /// A numeric value truncated to a `long`, against a `LONG`.
    Long(i64),
    /// A numeric value, against a `DOUBLE` (`Double.compare`).
    Double(f64),
    /// The `toString()` of the value, against a `STRING` (`String.compareTo`).
    String(Rc<str>),
    /// A boolean value, against a `BOOLEAN`.
    Boolean(bool),
}

/// `CompareTo`: the earliest datapoint of the window passing the test, at its own tick,
/// location and elevation.
pub struct CompareTo {
    /// The reference as given to the builder, for the snapshot.
    pub reference: Value,
    /// The test.
    pub test: Test,
    /// The comparison.
    pub comparison: Comparison,
}

impl CompareTo {
    /// The ordering of the item of the datapoint `i` relative to the reference, `None` when
    /// the value cannot be compared (the datapoint does not pass).
    fn ordering(&self, args: &AggregatorArgs, i: usize) -> Result<Option<Ordering>> {
        let value = &args.values[i];
        Ok(Some(match &self.test {
            Test::Tick(reference) => args.ticks[i].cmp(reference),
            Test::HhCode(reference) => args.locations[i].cmp(reference),
            Test::Elevation(reference) => args.elevations[i].cmp(reference),
            Test::Latitude(reference) => {
                double_compare(geoxp::from_geoxp_point(args.locations[i]).0, *reference)
            }
            Test::Longitude(reference) => {
                double_compare(geoxp::from_geoxp_point(args.locations[i]).1, *reference)
            }
            Test::Long(reference) => match Num::of(value) {
                Some(Num::Long(l)) => l.cmp(reference),
                Some(Num::Double(d)) => double_to_long(d).cmp(reference),
                None => return Ok(None),
            },
            Test::Double(reference) => match Num::of(value) {
                Some(number) => double_compare(number.as_f64(), *reference),
                None => return Ok(None),
            },
            // `value.toString()` on a `null` (a `REDUCE` window).
            Test::String(_) if value.is_null() => return Err(Error::null_pointer()),
            Test::String(reference) => value
                .to_string()
                .encode_utf16()
                .cmp(reference.encode_utf16()),
            Test::Boolean(reference) => match value {
                Value::Boolean(b) => b.cmp(reference),
                _ => return Ok(None),
            },
        }))
    }
}

impl Aggregator for CompareTo {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let mut found = None;
        let mut tick = i64::MAX;
        // The ticks are not considered to be sorted: the earliest passing datapoint wins.
        for i in 0..args.values.len() {
            if found.is_some() && args.ticks[i] >= tick {
                continue;
            }
            if self
                .ordering(args, i)?
                .is_some_and(|ordering| self.comparison.holds(ordering))
            {
                found = Some(i);
                tick = args.ticks[i];
            }
        }
        Ok(match found {
            Some(i) => Aggregated {
                tick: args.ticks[i],
                location: args.locations[i],
                elevation: args.elevations[i],
                value: args.values[i].clone(),
            },
            None => Aggregated {
                tick: args.tick,
                location: NO_LOCATION,
                elevation: NO_ELEVATION,
                value: Value::Null,
            },
        })
    }

    fn parameters(&self) -> Option<String> {
        Some(value_snapshot(&self.reference).trim_end().to_owned())
    }
}

/// `MapperCompareTo`: `reference name` pushes the `CompareTo` of the reference.
fn compare_builder(
    stack: &mut Stack,
    name: &'static str,
    compared: Compared,
    comparison: Comparison,
) -> Result<()> {
    let reference = stack.pop()?;
    if matches!(reference, Value::Boolean(_))
        && !matches!(comparison, Comparison::Eq | Comparison::Ne)
    {
        return Err(Error::usage(name, "cannot compare with a BOOLEAN."));
    }
    let test = match (compared, &reference) {
        (Compared::Tick, Value::Long(l)) => Test::Tick(*l),
        (Compared::HhCode, Value::Long(l)) => Test::HhCode(*l),
        (Compared::Elev, Value::Long(l)) => Test::Elevation(*l),
        (Compared::Tick | Compared::HhCode | Compared::Elev, _) => {
            return Err(Error::usage(name, "can only compare with a LONG."));
        }
        (Compared::Lat, Value::Long(l)) => Test::Latitude(*l as f64),
        (Compared::Lat, Value::Double(d)) => Test::Latitude(*d),
        (Compared::Lon, Value::Long(l)) => Test::Longitude(*l as f64),
        (Compared::Lon, Value::Double(d)) => Test::Longitude(*d),
        (Compared::Lat | Compared::Lon, _) => {
            return Err(Error::usage(
                name,
                "can only compare with a DOUBLE or a LONG.",
            ));
        }
        (Compared::Value, Value::Long(l)) => Test::Long(*l),
        (Compared::Value, Value::Double(d)) => Test::Double(*d),
        (Compared::Value, Value::String(s)) => Test::String(s.clone()),
        (Compared::Value, Value::Boolean(b)) => Test::Boolean(*b),
        (Compared::Value, _) => {
            return Err(Error::usage(
                name,
                "can only compare with a STRING, a DOUBLE, a LONG or a BOOLEAN.",
            ));
        }
    };
    stack.push(Value::Function(Named::object(
        name,
        CompareTo {
            reference,
            test,
            comparison,
        },
    )));
    Ok(())
}
