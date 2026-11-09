// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The fillers of `FILL`: `filler.value`, `filler.next`, `filler.previous`,
//! `filler.interpolate`, `filler.trend`, `filler.linear`, `filler.akima`, `filler.spline`,
//! `filler.newton`, `filler.lowess`, `filler.rlowess` and `MACROFILLER`, plus the macros
//! `FILL` accepts as fillers (`FILL.fillerFromMacro`).
//!
//! A filler plays one or both of two roles:
//! - **windowed** (`WarpScriptFillerFunction`): it computes the datapoint filling a tick from
//!   the datapoints around it (a `pre` and a `post` window) and the datapoint at that tick;
//! - **single value** (`WarpScriptSingleValueFillerFunction`): it fills the ticks one by one,
//!   usually from a function precomputed on the whole series (`Precomputable`).

use std::any::Any;
use std::rc::Rc;

use crate::aggregators::double_value;
use crate::error::fault::IllegalArgumentError;
use crate::error::{Error, Result};
use crate::format::{java_double_to_string, url_encode};
use crate::functions::Entry;
use crate::functions::gts::datapoint_from_list;
use crate::geoxp::{from_geoxp_point, to_geoxp_point};
use crate::gts::{GeoTimeSerie, Gts, Metadata, NO_ELEVATION, NO_LOCATION, Values};
use crate::interpolation::{
    NewtonForm, PolynomialSplineFunction, akima, divided_difference, linear, loess, spline,
};
use crate::jmath::round;
use crate::macros::Macro;
use crate::snapshot::value_snapshot;
use crate::stack::Stack;
use crate::value::{FunctionObject, Value};

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("filler.value", Entry::Builder(value_builder)),
    ("filler.next", Entry::Object(|n| object(n, Kind::Next))),
    (
        "filler.previous",
        Entry::Object(|n| object(n, Kind::Previous)),
    ),
    (
        "filler.interpolate",
        Entry::Object(|n| object(n, Kind::Interpolate)),
    ),
    ("filler.trend", Entry::Object(|n| object(n, Kind::Trend))),
    (
        "filler.linear",
        Entry::Object(|n| object(n, Kind::Function(Interpolator::Linear))),
    ),
    (
        "filler.akima",
        Entry::Object(|n| object(n, Kind::Function(Interpolator::Akima))),
    ),
    (
        "filler.spline",
        Entry::Object(|n| object(n, Kind::Function(Interpolator::Spline))),
    ),
    (
        "filler.newton",
        Entry::Object(|n| object(n, Kind::Function(Interpolator::Newton))),
    ),
    (
        "filler.lowess",
        Entry::Builder(|s, n| lowess_builder(s, n, false)),
    ),
    (
        "filler.rlowess",
        Entry::Builder(|s, n| lowess_builder(s, n, true)),
    ),
    ("MACROFILLER", Entry::Builder(macrofiller_builder)),
];

/// A filler function object.
pub struct Filler {
    name: &'static str,
    kind: Kind,
}

/// The fillers.
enum Kind {
    Value(FillValue),
    Next,
    Previous,
    Interpolate,
    Trend,
    Function(Interpolator),
    Macro(MacroFiller),
}

/// The interpolating fillers (single value only).
pub enum Interpolator {
    /// `filler.linear`.
    Linear,
    /// `filler.akima`.
    Akima,
    /// `filler.spline`.
    Spline,
    /// `filler.newton`.
    Newton,
    /// `filler.lowess` and `filler.rlowess`.
    Lowess(Lowess),
}

/// `FillerValue`: a constant datapoint.
#[derive(Clone)]
pub struct FillValue {
    location: i64,
    elevation: i64,
    value: Value,
}

/// `FillerLowess` / `FillerRlowess` (with robustness iterations).
pub struct Lowess {
    bandwidth: i64,
    robustness: Option<i32>,
    accuracy: f64,
}

/// `MACROFILLER.MacroFillerWrapper`.
pub struct MacroFiller {
    wrapped: Rc<Macro>,
    pre: i32,
    post: i32,
}

fn object(name: &'static str, kind: Kind) -> Rc<dyn FunctionObject> {
    Rc::new(Filler { name, kind })
}

fn push(stack: &mut Stack, name: &'static str, kind: Kind) {
    stack.push(Value::Function(object(name, kind)));
}

impl FunctionObject for Filler {
    fn name(&self) -> &str {
        self.name
    }

    fn type_name(&self) -> &'static str {
        "FILLER"
    }

    fn snapshot(&self) -> String {
        let name = self.name;
        match &self.kind {
            Kind::Value(v) => {
                let (lat, lon) = from_geoxp_point(v.location);
                format!(
                    "[ {}{}{}{}] {name}",
                    value_snapshot(&Value::Double(lat)),
                    value_snapshot(&Value::Double(lon)),
                    value_snapshot(&Value::Long(v.elevation)),
                    value_snapshot(&v.value)
                )
            }
            Kind::Function(Interpolator::Lowess(l)) => {
                let robustness = l
                    .robustness
                    .map(|r| value_snapshot(&Value::Long(r.into())))
                    .unwrap_or_default();
                format!(
                    "{}{robustness}{} {name}",
                    value_snapshot(&Value::Long(l.bandwidth)),
                    value_snapshot(&Value::Double(l.accuracy))
                )
            }
            Kind::Macro(m) => format!(
                "<% {} {} {} {name} %> EVAL",
                m.wrapped.snapshot(true),
                m.pre,
                m.post
            ),
            _ => format!("'{}' FUNCREF", url_encode(name)),
        }
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }
}

/// `filler.value`: `[ lat lon elev value ] filler.value` or `value filler.value`.
fn value_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let top = stack.pop()?;
    let (location, elevation, value) = if let Value::List(list) = &top {
        let datapoint = list.borrow().clone();
        if datapoint.len() != 4 {
            return Err(Error::new(format!(
                "{name} expects a LIST of 4 parameters: latitude longitude elevation value"
            )));
        }
        let Some(lat) = datapoint[0].as_f64() else {
            return Err(Error::new(format!(
                "{name} expects the latitude to be a NUMBER"
            )));
        };
        let Some(lon) = datapoint[1].as_f64() else {
            return Err(Error::new(format!(
                "{name} expects the longitude to be a NUMBER"
            )));
        };
        let elevation = match datapoint[2] {
            Value::Long(elevation) => elevation,
            Value::Double(d) if d.is_nan() => NO_ELEVATION,
            _ => {
                return Err(Error::new(format!(
                    "{name} expects the elevation to be a LONG or NAN"
                )));
            }
        };
        let location = if lat.is_nan() || lon.is_nan() {
            NO_LOCATION
        } else {
            to_geoxp_point(lat, lon)
        };
        (location, elevation, datapoint[3].clone())
    } else {
        (NO_LOCATION, NO_ELEVATION, top)
    };
    if !matches!(
        value,
        Value::Long(_) | Value::Double(_) | Value::Boolean(_) | Value::String(_) | Value::Bytes(_)
    ) {
        return Err(Error::new(format!(
            "{name} expects the value to be either a LONG, a DOUBLE, a BOOLEAN, a STRING or a BYTEARRAY"
        )));
    }
    push(
        stack,
        name,
        Kind::Value(FillValue {
            location,
            elevation,
            value,
        }),
    );
    Ok(())
}

/// `filler.lowess` (`bandwidth [accuracy]`) and `filler.rlowess` (`bandwidth robustness
/// [accuracy]`).
fn lowess_builder(stack: &mut Stack, name: &'static str, robust: bool) -> Result<()> {
    let mut accuracy = 1e-12;
    let mut top = stack.pop()?;
    if let Value::Double(d) = top {
        accuracy = d;
        top = stack.pop()?;
    }
    let mut robustness = None;
    if robust {
        let Value::Long(r) = top else {
            return Err(Error::new(format!(
                "{name} expects a number of robustifying iterations (LONG), but instead got a {}",
                top.type_name()
            )));
        };
        robustness = Some(r as i32);
        top = stack.pop()?;
    }
    let Value::Long(bandwidth) = top else {
        return Err(Error::new(format!(
            "{name} expects a bandwidth as a number of data points (LONG), but instead got a {}",
            top.type_name()
        )));
    };
    if bandwidth <= 1 {
        return Err(Error::new(format!(
            "{name} expects a bandwidth > 1, instead got {bandwidth}"
        )));
    }
    if let Some(r) = robustness.filter(|r| *r <= 0) {
        return Err(Error::new(format!(
            "{name} expects a positive robustness, instead got {r}"
        )));
    }
    if accuracy <= 0.0 {
        return Err(Error::new(format!(
            "{name} expects a positive accuracy, instead got {}",
            java_double_to_string(accuracy)
        )));
    }
    push(
        stack,
        name,
        Kind::Function(Interpolator::Lowess(Lowess {
            bandwidth,
            robustness,
            accuracy,
        })),
    );
    Ok(())
}

/// `MACROFILLER`: `macro pre post MACROFILLER`.
fn macrofiller_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let Value::Long(post) = stack.pop()? else {
        return Err(Error::new(format!(
            "{name} expects a 'post' window length on top of the stack."
        )));
    };
    let Value::Long(pre) = stack.pop()? else {
        return Err(Error::new(format!(
            "{name} expects a 'pre' window length below the 'post' window length."
        )));
    };
    let Value::Macro(wrapped) = stack.pop()? else {
        return Err(Error::new(format!("{name} operates on a MACRO.")));
    };
    push(
        stack,
        name,
        Kind::Macro(MacroFiller {
            wrapped,
            pre: pre as i32,
            post: post as i32,
        }),
    );
    Ok(())
}

// -----------------------------------------------------------------------------------------
// Roles
// -----------------------------------------------------------------------------------------

/// What `FILL` accepts as a filler: a filler object or a macro.
pub enum FillerRef<'a> {
    /// A filler object.
    Object(&'a Filler),
    /// A macro, wrapped by `FILL.fillerFromMacro`.
    Macro(Rc<Macro>),
}

/// The roles of a filler.
pub enum Roles<'a> {
    /// A windowed filler only.
    Window(Window<'a>),
    /// A single value filler only.
    Single(Single<'a>),
    /// A filler playing both roles.
    Both(Window<'a>, Single<'a>),
}

impl<'a> FillerRef<'a> {
    /// The filler designated by `value`, if any.
    pub fn of(value: &'a Value) -> Option<FillerRef<'a>> {
        match value {
            Value::Function(function) => function
                .as_any()
                .and_then(|any| any.downcast_ref::<Filler>())
                .map(FillerRef::Object),
            Value::Macro(m) => Some(FillerRef::Macro(Rc::clone(m))),
            _ => None,
        }
    }

    /// The roles of the filler.
    pub fn roles(&self) -> Roles<'a> {
        let filler = match self {
            FillerRef::Object(filler) => *filler,
            FillerRef::Macro(m) => return Roles::Single(Single::Macro(Rc::clone(m))),
        };
        let name = filler.name;
        match &filler.kind {
            Kind::Value(v) => Roles::Both(Window::Value(v), Single::Value(v)),
            Kind::Next => Roles::Both(Window::Next, Single::Next),
            Kind::Previous => Roles::Both(Window::Previous, Single::Previous),
            Kind::Interpolate => Roles::Both(Window::Interpolate, Single::Interpolate(name)),
            Kind::Trend => Roles::Window(Window::Trend),
            Kind::Macro(m) => Roles::Window(Window::Macro(m)),
            Kind::Function(f) => Roles::Single(Single::Function(name, f)),
        }
    }
}

// -----------------------------------------------------------------------------------------
// Windowed fillers
// -----------------------------------------------------------------------------------------

/// A datapoint of a window, or the datapoint a windowed filler computes: tick, location,
/// elevation and value, any of which may be `null`.
#[derive(Clone, Debug)]
pub struct Slot {
    /// The tick.
    pub tick: Option<i64>,
    /// The location (`NO_LOCATION` when the datapoint has none).
    pub location: Option<i64>,
    /// The elevation (`NO_ELEVATION` when the datapoint has none).
    pub elevation: Option<i64>,
    /// The value.
    pub value: Value,
}

impl Slot {
    /// The empty slot.
    pub fn empty() -> Slot {
        Slot {
            tick: None,
            location: None,
            elevation: None,
            value: Value::Null,
        }
    }

    /// The datapoint at `idx` of `gts`.
    pub fn at(gts: &GeoTimeSerie, idx: usize) -> Slot {
        Slot {
            tick: Some(gts.tick_at(idx)),
            location: Some(gts.location_at(idx)),
            elevation: Some(gts.elevation_at(idx)),
            value: gts.value_at(idx),
        }
    }
}

/// A windowed filler.
pub enum Window<'a> {
    /// `filler.value`.
    Value(&'a FillValue),
    /// `filler.next`.
    Next,
    /// `filler.previous`.
    Previous,
    /// `filler.interpolate`.
    Interpolate,
    /// `filler.trend`.
    Trend,
    /// `MACROFILLER`.
    Macro(&'a MacroFiller),
}

/// The metadata of the series a windowed filler fills, and of the other series of a cross
/// fill.
pub type Metas<'m> = (&'m Metadata, Option<&'m Metadata>);

impl Window<'_> {
    /// `getPreWindow`.
    pub fn pre(&self) -> i32 {
        match self {
            Window::Previous | Window::Interpolate => 1,
            Window::Trend => 2,
            Window::Macro(m) => m.pre,
            Window::Value(_) | Window::Next => 0,
        }
    }

    /// `getPostWindow`.
    pub fn post(&self) -> i32 {
        match self {
            Window::Next | Window::Interpolate => 1,
            Window::Trend => 2,
            Window::Macro(m) => m.post,
            Window::Value(_) | Window::Previous => 0,
        }
    }

    /// `apply`: the datapoint filling the tick of the current slot, `slots` holding the
    /// `pre` window, the current slot and the `post` window (windows clamped to 0).
    pub fn apply(&self, stack: &mut Stack, metas: Metas<'_>, slots: &[Slot]) -> Result<Slot> {
        match self {
            Window::Value(v) => Ok(Slot {
                tick: slots[0].tick,
                location: Some(v.location),
                elevation: Some(v.elevation),
                value: v.value.clone(),
            }),
            Window::Next => Ok(adjacent(&slots[0], &slots[1])),
            Window::Previous => Ok(adjacent(&slots[1], &slots[0])),
            Window::Interpolate => Ok(interpolate(slots)),
            Window::Trend => trend(slots),
            Window::Macro(m) => m.apply(stack, metas, slots),
        }
    }
}

/// `FillerNext` / `FillerPrevious`: the datapoint `adjacent` at the tick of `current`.
fn adjacent(current: &Slot, adjacent: &Slot) -> Slot {
    if adjacent.tick.is_none() {
        return Slot::empty();
    }
    Slot {
        tick: current.tick,
        ..adjacent.clone()
    }
}

/// The numeric value of a value known to be a number.
fn numeric(value: &Value) -> f64 {
    value.as_f64().unwrap_or_default()
}

/// `((Number) value).doubleValue()`.
fn number(value: &Value) -> Result<f64> {
    match value {
        Value::Null => Err(Error::null_pointer()),
        _ => double_value(value),
    }
}

/// Both results, or the first error.
pub(crate) fn both<A, B>(a: Result<A>, b: Result<B>) -> Result<(A, B)> {
    match (a, b) {
        (Ok(a), Ok(b)) => Ok((a, b)),
        (Err(err), _) | (_, Err(err)) => Err(err),
    }
}

/// `FillerInterpolate.apply`: the linear interpolation between the previous and the next
/// datapoints (none for `STRING` and `BOOLEAN` values), locations interpolated in latitude
/// and longitude, elevations rounded.
fn interpolate(slots: &[Slot]) -> Slot {
    let (prev, current, next) = (&slots[0], &slots[1], &slots[2]);
    let (Some(prevtick), Some(nexttick)) = (prev.tick, next.tick) else {
        return Slot::empty();
    };
    if matches!(prev.value, Value::String(_) | Value::Boolean(_)) {
        return Slot::empty();
    }
    let tick = current.tick.unwrap_or_default();
    let span = nexttick.wrapping_sub(prevtick) as f64;
    let delta = tick.wrapping_sub(prevtick) as f64;
    let prevvalue = numeric(&prev.value);
    let interpolated = prevvalue + (numeric(&next.value) - prevvalue) / span * delta;
    let value = if let Value::Long(_) = prev.value {
        Value::Long(round(interpolated))
    } else {
        Value::Double(interpolated)
    };
    let (prevloc, nextloc) = (
        prev.location.unwrap_or(NO_LOCATION),
        next.location.unwrap_or(NO_LOCATION),
    );
    let location = if prevloc == NO_LOCATION || nextloc == NO_LOCATION {
        NO_LOCATION
    } else {
        let (plat, plon) = from_geoxp_point(prevloc);
        let (nlat, nlon) = from_geoxp_point(nextloc);
        to_geoxp_point(
            plat + delta * ((nlat - plat) / span),
            plon + delta * ((nlon - plon) / span),
        )
    };
    let (prevelev, nextelev) = (
        prev.elevation.unwrap_or(NO_ELEVATION),
        next.elevation.unwrap_or(NO_ELEVATION),
    );
    let elevation = if prevelev == NO_ELEVATION || nextelev == NO_ELEVATION {
        NO_ELEVATION
    } else {
        round(prevelev as f64 + delta * (nextelev.wrapping_sub(prevelev) as f64 / span))
    };
    Slot {
        tick: Some(tick),
        location: Some(location),
        elevation: Some(elevation),
        value,
    }
}

/// `FillerTrend.apply` on the window `prevprev prev current next nextnext`: the value is
/// projected from the trends before and after the tick; the latitude, longitude and elevation
/// are projected the same way (Warp 10 stores the projected elevation, rounded, as the
/// location and never sets the elevation).
fn trend(slots: &[Slot]) -> Result<Slot> {
    let ticks: Vec<Option<i64>> = slots.iter().map(|s| s.tick).collect();
    let tick = ticks[2].unwrap_or_default();
    let values: Vec<Value> = slots.iter().map(|s| s.value.clone()).collect();
    let Some(value) = trend_value(&ticks, &values, tick)? else {
        return Ok(Slot::empty());
    };
    // The recursive calls of Warp 10 project the coordinates of the slots that have some.
    let coordinate = |part: fn((f64, f64)) -> f64| -> Vec<Value> {
        slots
            .iter()
            .map(|s| match s.location.unwrap_or(NO_LOCATION) {
                NO_LOCATION => Value::Null,
                location => Value::Double(part(from_geoxp_point(location))),
            })
            .collect()
    };
    let lat = trend_value(&ticks, &coordinate(|(lat, _)| lat), tick);
    let lon = trend_value(&ticks, &coordinate(|(_, lon)| lon), tick);
    let mut location = match both(lat, lon)? {
        (Some(lat), Some(lon)) => to_geoxp_point(lat, lon),
        _ => NO_LOCATION,
    };
    let elevations: Vec<Value> = slots
        .iter()
        .map(|s| match s.elevation.unwrap_or(NO_ELEVATION) {
            NO_ELEVATION => Value::Null,
            elevation => Value::Long(elevation),
        })
        .collect();
    if let Some(elevation) = trend_value(&ticks, &elevations, tick)? {
        location = round(elevation);
    }
    Ok(Slot {
        tick: Some(tick),
        location: Some(location),
        elevation: Some(NO_ELEVATION),
        value: Value::Double(value),
    })
}

/// The value `FillerTrend` projects at `tick` from the values of the window, `None` when no
/// trend can be computed.
fn trend_value(ticks: &[Option<i64>], values: &[Value], tick: i64) -> Result<Option<f64>> {
    let has = |i: usize| !values[i].is_null();
    let pre = if has(0) && has(1) {
        Some((0, 1))
    } else if has(1) && has(3) {
        Some((1, 3))
    } else if has(3) && has(4) {
        Some((3, 4))
    } else {
        None
    };
    let post = if has(4) && has(3) {
        Some((3, 4))
    } else if has(1) && has(3) {
        Some((1, 3))
    } else if has(0) && has(1) {
        Some((0, 1))
    } else {
        None
    };
    let rate = |pair: Option<(usize, usize)>| -> Result<f64> {
        let Some((a, b)) = pair else {
            return Ok(f64::NAN);
        };
        let (vb, va) = both(number(&values[b]), number(&values[a]))?;
        let span = ticks[b]
            .unwrap_or_default()
            .wrapping_sub(ticks[a].unwrap_or_default());
        Ok((vb - va) / span as f64)
    };
    let (mut prerate, mut postrate) = both(rate(pre), rate(post))?;
    if prerate.is_nan() {
        prerate = postrate;
    }
    if postrate.is_nan() {
        postrate = prerate;
    }
    if prerate.is_nan() {
        return Ok(None);
    }
    Ok(Some(match (ticks[1], ticks[3]) {
        (Some(prev), Some(next)) => {
            let span = next.wrapping_sub(prev);
            let delta = tick.wrapping_sub(prev);
            let alpha = delta as f64 / span as f64;
            let (prevvalue, nextvalue) = both(number(&values[1]), number(&values[3]))?;
            let projected_previous = prevvalue + delta as f64 * prerate;
            let projected_next = nextvalue - span.wrapping_sub(delta) as f64 * postrate;
            alpha * projected_previous + (1.0 - alpha) * projected_next
        }
        (Some(prev), None) => numeric(&values[1]) + prerate * tick.wrapping_sub(prev) as f64,
        (None, next) => {
            numeric(&values[3]) - postrate * next.unwrap_or_default().wrapping_sub(tick) as f64
        }
    }))
}

impl MacroFiller {
    /// `MacroFillerWrapper.apply`: the macro receives a series holding the metadata of the
    /// series to fill, the same for the other series (or `null`), the `pre` window, the
    /// current datapoint, the `post` window and the tick (datapoints as `[ tick lat lon elev
    /// value ]`), and leaves the datapoint to add as a list.
    fn apply(&self, stack: &mut Stack, metas: Metas<'_>, slots: &[Slot]) -> Result<Slot> {
        stack.push(Value::Gts(metadata_gts(metas.0)));
        stack.push(metas.1.map_or(Value::Null, |m| Value::Gts(metadata_gts(m))));
        capacity(self.pre)?;
        let mut ticks = Vec::new();
        let mut ts = 0;
        for (i, slot) in (1..).zip(slots) {
            let tick = tick_list(slot);
            if i == self.pre + 1 {
                stack.push(Value::list(std::mem::take(&mut ticks)));
                stack.push(tick);
                capacity(self.post)?;
                ts = slot.tick.unwrap_or_default();
            } else {
                ticks.push(tick);
            }
        }
        stack.push(Value::list(ticks));
        stack.push(Value::Long(ts));
        stack.exec_macro(&self.wrapped)?;
        let list = match stack.pop()? {
            Value::List(list) => list,
            other => {
                stack.push(other);
                return Err(Error::new(
                    "Expected a [ ts lat lon elev value ] list as result of filler.",
                ));
            }
        };
        let mut elements = list.borrow_mut();
        if elements.len() == 1 {
            elements.insert(0, Value::Long(ts));
        }
        let datapoint = datapoint_from_list(&elements)?;
        Ok(Slot {
            tick: Some(datapoint.tick),
            location: Some(datapoint.location),
            elevation: Some(datapoint.elevation),
            value: datapoint.value,
        })
    }
}

/// `new ArrayList(capacity)`.
fn capacity(capacity: i32) -> Result<()> {
    if capacity < 0 {
        return Err(IllegalArgumentError::Capacity(i64::from(capacity)).into());
    }
    Ok(())
}

/// A new series carrying the name, labels and attributes of `metadata`.
fn metadata_gts(metadata: &Metadata) -> Gts {
    let mut gts = GeoTimeSerie::new();
    gts.metadata = Metadata {
        name: metadata.name.clone(),
        labels: metadata.labels.clone(),
        attributes: metadata.attributes.clone(),
        ..Metadata::default()
    };
    gts.shared()
}

/// A slot as `[ tick lat lon elev value ]` (`NaN` for no location or elevation), or
/// `[ null NaN NaN NaN null ]` when it has no tick or value.
fn tick_list(slot: &Slot) -> Value {
    let (Some(tick), false) = (slot.tick, slot.value.is_null()) else {
        return Value::list(vec![
            Value::Null,
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Null,
        ]);
    };
    let (lat, lon) = match slot.location.unwrap_or(NO_LOCATION) {
        NO_LOCATION => (f64::NAN, f64::NAN),
        location => from_geoxp_point(location),
    };
    let elevation = match slot.elevation.unwrap_or(NO_ELEVATION) {
        NO_ELEVATION => Value::Double(f64::NAN),
        elevation => Value::Long(elevation),
    };
    Value::list(vec![
        Value::Long(tick),
        Value::Double(lat),
        Value::Double(lon),
        elevation,
        slot.value.clone(),
    ])
}

// -----------------------------------------------------------------------------------------
// Single value fillers
// -----------------------------------------------------------------------------------------

/// A single value filler.
pub enum Single<'a> {
    /// `filler.value` (not precomputable).
    Value(&'a FillValue),
    /// `filler.next`.
    Next,
    /// `filler.previous`.
    Previous,
    /// `filler.interpolate` (named).
    Interpolate(&'a str),
    /// An interpolating filler (named).
    Function(&'a str, &'a Interpolator),
    /// A macro (`FILL.fillerFromMacro`).
    Macro(Rc<Macro>),
}

/// The filler of the ticks of a series, as `compute` returns it for that series.
pub enum Ticker {
    /// `filler.value`.
    Value(FillValue),
    /// `filler.next`.
    Next {
        /// The series.
        original: Gts,
        /// Its size.
        count: usize,
        /// Its last tick.
        last_tick: i64,
        /// The index of the first tick not before the last filled tick.
        current: usize,
    },
    /// `filler.previous`.
    Previous {
        /// The series.
        original: Gts,
        /// Its size.
        count: usize,
        /// Its first tick.
        first_tick: i64,
        /// The index of the first tick not before the last filled tick.
        current: usize,
    },
    /// `filler.interpolate`: value, latitude and longitude, elevation functions.
    Interpolate {
        /// The value function.
        value: Option<PolynomialSplineFunction>,
        /// The latitude and longitude functions.
        location: Option<(PolynomialSplineFunction, PolynomialSplineFunction)>,
        /// The elevation function.
        elevation: Option<PolynomialSplineFunction>,
    },
    /// An interpolating filler.
    Function(Option<Function>),
    /// A macro, which fills the series it was computed on.
    Macro {
        /// The series.
        original: Gts,
        /// The macro.
        wrapped: Rc<Macro>,
    },
}

/// An interpolated function.
pub enum Function {
    /// A piecewise polynomial function, defined between its first and last knots.
    Spline(PolynomialSplineFunction),
    /// A polynomial, defined everywhere.
    Newton(NewtonForm),
}

impl Function {
    fn is_valid_point(&self, x: f64) -> bool {
        match self {
            Function::Spline(f) => f.is_valid_point(x),
            Function::Newton(_) => true,
        }
    }

    fn value(&self, x: f64) -> f64 {
        match self {
            Function::Spline(f) => f.value(x),
            Function::Newton(f) => f.value(x),
        }
    }
}

impl Single<'_> {
    /// `Precomputable.compute`: the filler of the ticks of `gts`.
    pub fn compute(&self, gts: &Gts) -> Result<Ticker> {
        match self {
            Single::Value(v) => Ok(Ticker::Value((*v).clone())),
            Single::Next => {
                let mut g = gts.borrow_mut();
                g.sort(false);
                Ok(Ticker::Next {
                    original: Rc::clone(gts),
                    count: g.size(),
                    last_tick: g.ticks.last().copied().unwrap_or(i64::MIN),
                    current: 0,
                })
            }
            Single::Previous => {
                let mut g = gts.borrow_mut();
                g.sort(false);
                Ok(Ticker::Previous {
                    original: Rc::clone(gts),
                    count: g.size(),
                    first_tick: g.tick_at(0),
                    current: 0,
                })
            }
            Single::Interpolate(name) => interpolate_ticker(name, &gts.borrow()),
            Single::Function(name, interpolator) => {
                function_ticker(name, interpolator, &gts.borrow())
            }
            Single::Macro(wrapped) => Ok(Ticker::Macro {
                original: Rc::clone(gts),
                wrapped: Rc::clone(wrapped),
            }),
        }
    }
}

/// The ticks and values of a `DOUBLE` or `LONG` series as doubles, `None` for an empty
/// series.
fn numeric_points(name: &str, gts: &GeoTimeSerie) -> Result<Option<(Vec<f64>, Vec<f64>)>> {
    if gts.size() == 0 {
        return Ok(None);
    }
    let values = match &gts.values {
        Values::Long(v) => v.iter().map(|l| *l as f64).collect(),
        Values::Double(v) => v.clone(),
        Values::Boolean(_) => return Err(type_error(name, "BOOLEAN")),
        _ => return Err(type_error(name, "STRING")),
    };
    Ok(Some((
        gts.ticks.iter().map(|t| *t as f64).collect(),
        values,
    )))
}

fn type_error(name: &str, type_name: &str) -> Error {
    Error::new(format!(
        "{name} expects a GTS of type DOUBLE or LONG, but instead got a GTS of type {type_name}"
    ))
}

/// `FillerInterpolate.compute`: linear interpolations of the values (from 2 datapoints), of
/// the locations and of the elevations (from 1, which Commons Math rejects).
fn interpolate_ticker(name: &str, gts: &GeoTimeSerie) -> Result<Ticker> {
    let Some((x, y)) = numeric_points(name, gts)? else {
        return Ok(Ticker::Interpolate {
            value: None,
            location: None,
            elevation: None,
        });
    };
    let value = if x.len() > 1 {
        Some(linear(&x, &y)?)
    } else {
        None
    };
    let (mut lx, mut lat, mut lon, mut ex, mut ey) = (vec![], vec![], vec![], vec![], vec![]);
    for (i, tick) in x.iter().enumerate() {
        let location = gts.location_at(i);
        if location != NO_LOCATION {
            let (la, lo) = from_geoxp_point(location);
            lx.push(*tick);
            lat.push(la);
            lon.push(lo);
        }
        let elevation = gts.elevation_at(i);
        if elevation != NO_ELEVATION {
            ex.push(*tick);
            ey.push(elevation as f64);
        }
    }
    let location = if lx.is_empty() {
        None
    } else {
        Some(both(linear(&lx, &lat), linear(&lx, &lon))?)
    };
    let elevation = if ex.is_empty() {
        None
    } else {
        Some(linear(&ex, &ey)?)
    };
    Ok(Ticker::Interpolate {
        value,
        location,
        elevation,
    })
}

/// `compute` of the interpolating fillers: the function interpolating the series when it has
/// enough datapoints (Akima from 5, spline and LOESS from 3, falling back to linear from 2).
fn function_ticker(name: &str, interpolator: &Interpolator, gts: &GeoTimeSerie) -> Result<Ticker> {
    let Some((x, y)) = numeric_points(name, gts)? else {
        return Ok(Ticker::Function(None));
    };
    let n = x.len();
    let function = match interpolator {
        Interpolator::Newton if n > 1 => divided_difference(&x, &y).map(Function::Newton),
        Interpolator::Akima if n > 4 => akima(&x, &y).map(Function::Spline),
        Interpolator::Spline if n > 2 => spline(&x, &y).map(Function::Spline),
        Interpolator::Lowess(l) if n > 2 => {
            let ratio = (l.bandwidth as f64 / n as f64 + 1e-12).min(1.0);
            loess(&x, &y, ratio, l.robustness.unwrap_or(0), l.accuracy).map(Function::Spline)
        }
        Interpolator::Newton => return Ok(Ticker::Function(None)),
        _ if n > 1 => linear(&x, &y).map(Function::Spline),
        _ => return Ok(Ticker::Function(None)),
    }?;
    Ok(Ticker::Function(Some(function)))
}

impl Ticker {
    /// `fillTick`: fill `tick` in `filled` (or, for a macro, in the series it was computed
    /// on), with `invalid` (unless `null`) when the filler has no value for it.
    pub fn fill_tick(
        &mut self,
        stack: &mut Stack,
        tick: i64,
        filled: &mut GeoTimeSerie,
        invalid: &Value,
    ) -> Result<()> {
        let t = tick as f64;
        match self {
            Ticker::Value(v) => filled.set_value(tick, v.location, v.elevation, &v.value, false),
            Ticker::Next {
                original,
                count,
                last_tick,
                current,
            } => {
                if *count == 0 || tick >= *last_tick {
                    if tick != *last_tick {
                        set_unless_null(filled, tick, NO_LOCATION, NO_ELEVATION, invalid);
                    }
                    return Ok(());
                }
                let g = original.borrow();
                while tick > g.tick_at(*current) && *current < *count {
                    *current += 1;
                }
                filled.copy_datapoint_at(&g, *current, tick);
            }
            Ticker::Previous {
                original,
                count,
                first_tick,
                current,
            } => {
                if *count == 0 || tick <= *first_tick {
                    if tick != *first_tick {
                        set_unless_null(filled, tick, NO_LOCATION, NO_ELEVATION, invalid);
                    }
                    return Ok(());
                }
                let g = original.borrow();
                while tick > g.tick_at(*current) && *current < *count {
                    *current += 1;
                }
                filled.copy_datapoint_at(&g, current.saturating_sub(1), tick);
            }
            Ticker::Interpolate {
                value,
                location,
                elevation,
            } => {
                let elevation = elevation
                    .as_ref()
                    .filter(|f| f.is_valid_point(t))
                    .map_or(NO_ELEVATION, |f| round(f.value(t)));
                let location = location
                    .as_ref()
                    .filter(|(lat, _)| lat.is_valid_point(t))
                    .map_or(NO_LOCATION, |(lat, lon)| {
                        to_geoxp_point(lat.value(t), lon.value(t))
                    });
                match value.as_ref().filter(|f| f.is_valid_point(t)) {
                    Some(f) => filled.set_value(
                        tick,
                        location,
                        elevation,
                        &Value::Double(f.value(t)),
                        false,
                    ),
                    None => set_unless_null(filled, tick, location, elevation, invalid),
                }
            }
            Ticker::Function(function) => match function.as_ref().filter(|f| f.is_valid_point(t)) {
                Some(f) => filled.set_value(
                    tick,
                    NO_LOCATION,
                    NO_ELEVATION,
                    &Value::Double(f.value(t)),
                    false,
                ),
                None => set_unless_null(filled, tick, NO_LOCATION, NO_ELEVATION, invalid),
            },
            Ticker::Macro { original, wrapped } => {
                stack.push(Value::Gts(Rc::clone(original)));
                stack.push(Value::Long(tick));
                stack.exec_macro(wrapped)?;
                let out = stack.pop()?;
                let value = if out.is_null() { invalid } else { &out };
                set_unless_null(
                    &mut original.borrow_mut(),
                    tick,
                    NO_LOCATION,
                    NO_ELEVATION,
                    value,
                );
            }
        }
        Ok(())
    }
}

/// Add `value` at `tick` unless it is `null`.
fn set_unless_null(
    filled: &mut GeoTimeSerie,
    tick: i64,
    location: i64,
    elevation: i64,
    value: &Value,
) {
    if !value.is_null() {
        filled.set_value(tick, location, elevation, value, false);
    }
}

impl GeoTimeSerie {
    /// Append the location, elevation and value at `idx` of `source` at `tick`.
    fn copy_datapoint_at(&mut self, source: &GeoTimeSerie, idx: usize, tick: i64) {
        self.set_value(
            tick,
            source.location_at(idx),
            source.elevation_at(idx),
            &source.value_at(idx),
            false,
        );
    }
}
