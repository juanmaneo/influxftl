// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The mappers of a single datapoint (`mapper.abs`, `mapper.tolong`, `mapper.lat`,
//! `mapper.hour`…): applied by `MAP` with a window of one datapoint, they transform its
//! value, location or elevation and fail on a larger window.

use chrono::{Datelike, Timelike};

use super::{Aggregated, Aggregator, AggregatorArgs, Named, Roles};
use crate::error::fault::IllegalArgumentError;
use crate::error::{Error, Result};
use crate::format::url_encode;
use crate::functions::Entry;
use crate::functions::misc::TIME_UNITS_PER_MS;
use crate::functions::operators::parse_java_double;
use crate::functions::time::{TIME_UNITS_PER_S, Zone};
use crate::geoxp;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::jmath;
use crate::stack::Stack;
use crate::value::{FunctionObject, Value, double_to_long};

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("mapper.abs", Entry::Object(|n| single(n, Kind::Abs))),
    ("mapper.ceil", Entry::Object(|n| single(n, Kind::Ceil))),
    ("mapper.floor", Entry::Object(|n| single(n, Kind::Floor))),
    ("mapper.round", Entry::Object(|n| single(n, Kind::Round))),
    ("mapper.tanh", Entry::Object(|n| single(n, Kind::Tanh))),
    (
        "mapper.sigmoid",
        Entry::Object(|n| single(n, Kind::Sigmoid)),
    ),
    ("mapper.finite", Entry::Object(|n| single(n, Kind::Finite))),
    (
        "mapper.toboolean",
        Entry::Object(|n| single(n, Kind::ToBoolean)),
    ),
    ("mapper.tolong", Entry::Object(|n| single(n, Kind::ToLong))),
    (
        "mapper.todouble",
        Entry::Object(|n| single(n, Kind::ToDouble)),
    ),
    (
        "mapper.tostring",
        Entry::Object(|n| single(n, Kind::ToString)),
    ),
    ("mapper.lat", Entry::Object(|n| single(n, Kind::Latitude))),
    ("mapper.lon", Entry::Object(|n| single(n, Kind::Longitude))),
    ("mapper.elev", Entry::Object(|n| single(n, Kind::Elevation))),
    (
        "mapper.geo.clear",
        Entry::Object(|n| single(n, Kind::GeoClear)),
    ),
    ("mapper.tick", Entry::Object(|n| single(n, Kind::Tick))),
    (
        "mapper.year",
        Entry::Builder(|s, n| date_time_builder(s, n, Field::Year)),
    ),
    (
        "mapper.month",
        Entry::Builder(|s, n| date_time_builder(s, n, Field::Month)),
    ),
    (
        "mapper.day",
        Entry::Builder(|s, n| date_time_builder(s, n, Field::Day)),
    ),
    (
        "mapper.weekday",
        Entry::Builder(|s, n| date_time_builder(s, n, Field::Weekday)),
    ),
    (
        "mapper.hour",
        Entry::Builder(|s, n| date_time_builder(s, n, Field::Hour)),
    ),
    (
        "mapper.minute",
        Entry::Builder(|s, n| date_time_builder(s, n, Field::Minute)),
    ),
    (
        "mapper.second",
        Entry::Builder(|s, n| date_time_builder(s, n, Field::Second)),
    ),
];

/// The single value of the window, `None` for an empty window, failing with `<name> can
/// only be applied to a single value.` on a larger one.
pub(super) fn single_value<'a>(args: &'a AggregatorArgs, name: &str) -> Result<Option<&'a Value>> {
    match args.values {
        [] => Ok(None),
        [value] => Ok(Some(value)),
        _ => Err(Error::new(format!(
            "{name} can only be applied to a single value."
        ))),
    }
}

/// The result of an empty window: `{ 0, NO_LOCATION, NO_ELEVATION, null }`.
pub(super) fn empty() -> Aggregated {
    Aggregated {
        tick: 0,
        ..Aggregated::none()
    }
}

/// What a [`Single`] mapper computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `MapperAbs`: `Math.abs` of a number (`Long.MIN_VALUE` staying itself).
    Abs,
    /// `MapperCeil`: `Math.ceil` of a `DOUBLE`, a `LONG` converted to a `DOUBLE`.
    Ceil,
    /// `MapperFloor`: `Math.floor` of a `DOUBLE`, a `LONG` converted to a `DOUBLE`.
    Floor,
    /// `MapperRound`: `Math.round` of a number.
    Round,
    /// `MapperTanh`: the hyperbolic tangent of a number.
    Tanh,
    /// `MapperSigmoid`: the logistic function of a number.
    Sigmoid,
    /// `MapperFinite`: drops the infinite and `NaN` doubles.
    Finite,
    /// `MapperToBoolean`: the value as a `BOOLEAN`.
    ToBoolean,
    /// `MapperToLong`: the value as a `LONG`.
    ToLong,
    /// `MapperToDouble`: the value as a `DOUBLE`.
    ToDouble,
    /// `MapperToString`: the value as a `STRING`.
    ToString,
    /// `MapperGeoLatitude`: the latitude of the datapoint as its value.
    Latitude,
    /// `MapperGeoLongitude`: the longitude of the datapoint as its value.
    Longitude,
    /// `MapperGeoElevation`: the elevation of the datapoint as its value.
    Elevation,
    /// `MapperGeoClearPosition`: the datapoint without location nor elevation.
    GeoClear,
    /// `MapperTick`: the tick of the datapoint as its value.
    Tick,
}

/// A mapper of a single datapoint.
pub struct Single {
    /// The registered name, for the error messages.
    pub name: &'static str,
    /// What the mapper computes.
    pub kind: Kind,
}

/// The [`Single`] mapper `kind` registered as `name`.
fn single(name: &'static str, kind: Kind) -> std::rc::Rc<dyn FunctionObject> {
    Named::object(name, Single { name, kind })
}

impl Single {
    /// The error of the arithmetic mappers given a non numeric value.
    fn not_numeric(&self) -> Error {
        Error::new(format!(
            "{} can only be applied to LONG or DOUBLE values.",
            self.name
        ))
    }

    /// The value as a double for the arithmetic mappers.
    fn number(&self, value: &Value) -> Result<f64> {
        match value {
            Value::Long(l) => Ok(*l as f64),
            Value::Double(d) => Ok(*d),
            _ => Err(self.not_numeric()),
        }
    }

    /// The new value of the datapoint (`value` at `tick`, `location`, `elevation`).
    fn map(&self, tick: i64, location: i64, elevation: i64, value: &Value) -> Result<Value> {
        Ok(match (self.kind, value) {
            (Kind::Abs, Value::Long(l)) => Value::Long(l.wrapping_abs()),
            (Kind::Abs, Value::Double(d)) => Value::Double(d.abs()),
            (Kind::Ceil | Kind::Floor, Value::Long(l)) => Value::Double(*l as f64),
            (Kind::Ceil, Value::Double(d)) => Value::Double(d.ceil()),
            (Kind::Floor, Value::Double(d)) => Value::Double(d.floor()),
            // `Math.round(float)` on the `long` widened to a `float`, giving an `int`.
            (Kind::Round, Value::Long(l)) => Value::Long(i64::from((*l as f32) as i32)),
            (Kind::Round, Value::Double(d)) => Value::Long(jmath::round(*d)),
            (Kind::Abs | Kind::Ceil | Kind::Floor | Kind::Round, _) => {
                return Err(self.not_numeric());
            }
            (Kind::Tanh, value) => Value::Double(self.number(value)?.tanh()),
            (Kind::Sigmoid, value) => Value::Double(1.0 / (1.0 + (-self.number(value)?).exp())),
            (Kind::Finite, Value::Double(d)) if !d.is_finite() => Value::Null,
            (Kind::Finite | Kind::GeoClear, value) => value.clone(),
            (Kind::ToBoolean, value) => to_boolean(value),
            (Kind::ToLong, value) => to_long(value),
            (Kind::ToDouble, value) => to_double(value),
            (Kind::ToString, Value::Null) => Value::Null,
            (Kind::ToString, value) => Value::string(value.to_string()),
            (Kind::Latitude | Kind::Longitude, _) if location == NO_LOCATION => Value::Null,
            (Kind::Latitude, _) => Value::Double(geoxp::from_geoxp_point(location).0),
            (Kind::Longitude, _) => Value::Double(geoxp::from_geoxp_point(location).1),
            (Kind::Elevation, _) if elevation == NO_ELEVATION => Value::Null,
            (Kind::Elevation, _) => Value::Long(elevation),
            (Kind::Tick, _) => Value::Long(tick),
        })
    }
}

/// `MapperToBoolean`: a number is `true` when it is not zero (`NaN` included), a string when
/// it is not empty; other values give no result.
#[allow(clippy::float_cmp)]
fn to_boolean(value: &Value) -> Value {
    match value {
        Value::Long(l) => Value::Boolean(*l != 0),
        // `0.0D != d` as Java computes it: `-0.0` is zero, `NaN` is not.
        Value::Double(d) => Value::Boolean(*d != 0.0),
        Value::Boolean(b) => Value::Boolean(*b),
        Value::String(s) => Value::Boolean(!s.is_empty()),
        _ => Value::Null,
    }
}

/// `MapperToLong`: a `DOUBLE` truncated (saturating, `NaN` giving 0), a `BOOLEAN` as 1 or 0,
/// a `STRING` parsed by `Long.valueOf` (no result when it is not a decimal `long`).
fn to_long(value: &Value) -> Value {
    match value {
        Value::Long(l) => Value::Long(*l),
        Value::Double(d) => Value::Long(double_to_long(*d)),
        Value::Boolean(b) => Value::Long(i64::from(*b)),
        // Rust's grammar is `Long.valueOf`'s for ASCII digits: optional sign, no spaces.
        Value::String(s) => s.parse::<i64>().map_or(Value::Null, Value::Long),
        _ => Value::Null,
    }
}

/// `MapperToDouble`: a `LONG` converted, a `BOOLEAN` as 1.0 or 0.0, a `STRING` parsed by
/// `Double.valueOf` (no result when it is not a number).
fn to_double(value: &Value) -> Value {
    match value {
        Value::Long(l) => Value::Double(*l as f64),
        Value::Double(d) => Value::Double(*d),
        Value::Boolean(b) => Value::Double(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => parse_java_double(s).map_or(Value::Null, Value::Double),
        _ => Value::Null,
    }
}

impl Aggregator for Single {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let Some(value) = single_value(args, self.name)? else {
            return Ok(empty());
        };
        let (mut location, mut elevation) = (args.locations[0], args.elevations[0]);
        let value = self.map(args.tick, location, elevation, value)?;
        if matches!(
            self.kind,
            Kind::Latitude | Kind::Longitude | Kind::Elevation | Kind::GeoClear
        ) {
            location = NO_LOCATION;
            elevation = NO_ELEVATION;
        }
        Ok(Aggregated {
            tick: args.tick,
            location,
            elevation,
            value,
        })
    }
}

/// The date-time field a [`DateTime`] mapper extracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    /// `MapperYear`.
    Year,
    /// `MapperMonthOfYear`: 1 for January.
    Month,
    /// `MapperDayOfMonth`.
    Day,
    /// `MapperDayOfWeek`: 1 for Monday.
    Weekday,
    /// `MapperHourOfDay`.
    Hour,
    /// `MapperMinuteOfHour`.
    Minute,
    /// `MapperSecondOfMinute`: a `DOUBLE` including the fraction of second of the tick.
    Second,
}

/// The time zone of a [`DateTime`] mapper (`DateTimeZone.forID` / `forOffsetMillis`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeZone {
    /// A zone given by its identifier, or the zero offset (`UTC`).
    Zone(Zone),
    /// A non zero offset in milliseconds, below a day.
    Offset(i32),
}

impl TimeZone {
    /// `DateTimeZone.forOffsetMillis`: offsets of a day or more are a runtime error.
    pub fn for_offset_millis(millis: i32) -> Result<TimeZone> {
        const MAX_OFFSET: u32 = 86_399_999;
        if millis.unsigned_abs() > MAX_OFFSET {
            return Err(IllegalArgumentError::MillisOutOfRange(millis).into());
        }
        Ok(if millis == 0 {
            TimeZone::Zone(Zone::UTC)
        } else {
            TimeZone::Offset(millis)
        })
    }

    /// `DateTimeZone.getID()`: `+HH:mm`, `+HH:mm:ss` or `+HH:mm:ss.SSS` for an offset.
    pub fn id(self) -> String {
        match self {
            TimeZone::Zone(zone) => zone.id(),
            TimeZone::Offset(millis) => {
                let sign = if millis < 0 { '-' } else { '+' };
                let millis = millis.unsigned_abs();
                let (hours, minutes) = (millis / 3_600_000, millis / 60_000 % 60);
                let (seconds, fraction) = (millis / 1000 % 60, millis % 1000);
                let mut id = format!("{sign}{hours:02}:{minutes:02}");
                if seconds != 0 || fraction != 0 {
                    id.push_str(&format!(":{seconds:02}"));
                }
                if fraction != 0 {
                    id.push_str(&format!(".{fraction:03}"));
                }
                id
            }
        }
    }
}

/// `MapperDateTime`: a field of the date-time of the tick in a time zone.
pub struct DateTime {
    /// The time zone.
    pub zone: TimeZone,
    /// The field.
    pub field: Field,
    /// The registered name, for the error messages.
    pub name: &'static str,
}

impl Aggregator for DateTime {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if single_value(args, self.name)?.is_none() {
            return Ok(empty());
        }
        let tick = args.tick;
        // Truncated towards zero, as Java divides.
        let millis = tick / TIME_UNITS_PER_MS;
        let local = match self.zone {
            TimeZone::Zone(zone) => zone.local_at(millis)?,
            TimeZone::Offset(offset) => Zone::UTC.local_at(millis + i64::from(offset))?,
        };
        let value = match self.field {
            Field::Year => Value::Long(i64::from(local.year())),
            Field::Month => Value::Long(i64::from(local.month())),
            Field::Day => Value::Long(i64::from(local.day())),
            Field::Weekday => Value::Long(i64::from(local.weekday().number_from_monday())),
            Field::Hour => Value::Long(i64::from(local.hour())),
            Field::Minute => Value::Long(i64::from(local.minute())),
            Field::Second => Value::Double(
                f64::from(local.second())
                    + (tick % TIME_UNITS_PER_S) as f64 / TIME_UNITS_PER_S as f64,
            ),
        };
        Ok(Aggregated {
            tick,
            location: args.locations[0],
            elevation: args.elevations[0],
            value,
        })
    }

    fn parameters(&self) -> Option<String> {
        Some(format!("'{}'", url_encode(&self.zone.id())))
    }
}

/// `MapperDateTime` builders: `timezone mapper.hour` pushes the mapper of the hour in
/// `timezone`, a zone identifier or an offset in milliseconds (`Number.intValue()`).
fn date_time_builder(stack: &mut Stack, name: &'static str, field: Field) -> Result<()> {
    let zone = match stack.pop()? {
        Value::String(id) => TimeZone::Zone(Zone::for_id(Some(&id))?),
        // `intValue()`: a `LONG` wraps, a `DOUBLE` is truncated (saturating, `NaN` giving 0).
        Value::Long(l) => TimeZone::for_offset_millis(l as i32)?,
        Value::Double(d) => TimeZone::for_offset_millis(d as i32)?,
        _ => {
            return Err(Error::new(format!(
                "{name} expects a STRING timezone or a NUMBER millisecond offset."
            )));
        }
    };
    stack.push(Value::Function(Named::object(
        name,
        DateTime { zone, field, name },
    )));
    Ok(())
}
