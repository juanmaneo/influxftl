// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Geo Time Series construction and inspection: `NEWGTS`, `MAKEGTS`, `ADDVALUE`, `RENAME`, `RELABEL`,
//! `TICKS`, `VALUES`, `SORT`, `ATINDEX`…

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use super::Entry;
use super::args::{compute_and_check_index, fail, fail_with_cause};
use super::misc::TIME_UNITS_PER_MS;
use crate::encoder::{
    DEFAULT_COMP_RATIO_THRESHOLD, EncodedValue, GtsEncoder, encoder_to_wrapper, gts_to_wrapper,
};
use crate::error::series::SeriesError;
use crate::error::{Class, Error, Result};
use crate::geoxp;
use crate::gts::{GeoTimeSerie, Gts, Metadata, NO_ELEVATION, NO_LOCATION};
use crate::stack::Stack;
use crate::thrift::serialize_wrapper;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("NEWGTS", Entry::Stack(newgts)),
    ("MAKEGTS", Entry::Stack(makegts)),
    ("ADDVALUE", Entry::Stack(addvalue)),
    ("SETVALUE", Entry::Stack(setvalue)),
    ("REMOVETICK", Entry::Stack(removetick)),
    ("NAME", Entry::Stack(name)),
    ("RENAME", Entry::Stack(rename)),
    ("LABELS", Entry::Stack(labels)),
    ("RELABEL", Entry::Stack(relabel)),
    ("ATTRIBUTES", Entry::Stack(attributes)),
    ("SETATTRIBUTES", Entry::Stack(setattributes)),
    ("LASTACTIVITY", Entry::Stack(lastactivity)),
    ("TICKS", Entry::Stack(ticks)),
    ("TICKLIST", Entry::Stack(ticklist)),
    ("VALUES", Entry::Stack(values)),
    ("LOCATIONS", Entry::Stack(locations)),
    ("ELEVATIONS", Entry::Stack(elevations)),
    ("SORT", Entry::Stack(sort)),
    ("RSORT", Entry::Stack(rsort)),
    ("FIRSTTICK", Entry::Stack(firsttick)),
    ("LASTTICK", Entry::Stack(lasttick)),
    ("CLONEEMPTY", Entry::Stack(cloneempty)),
    ("ATINDEX", Entry::Stack(atindex)),
    ("ATTICK", Entry::Stack(attick)),
    ("TICKINDEX", Entry::Stack(tickindex)),
    ("BUCKETSPAN", Entry::Stack(bucketspan)),
    ("BUCKETCOUNT", Entry::Stack(bucketcount)),
    ("LASTBUCKET", Entry::Stack(lastbucket)),
];

// -----------------------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------------------

/// `ElementOrListStackFunction.apply`: apply `f` to the top of the stack or, when it is a
/// list, to each of its elements (pushing the list of results).
pub(super) fn element_or_list(
    stack: &mut Stack,
    f: impl Fn(&Value) -> Result<Value>,
) -> Result<()> {
    let top = stack.pop()?;
    let result = match &top {
        Value::List(list) => {
            let results = list.borrow().iter().map(&f).collect::<Result<Vec<_>>>()?;
            Value::list(results)
        }
        other => f(other)?,
    };
    stack.push(result);
    Ok(())
}

/// `GTSStackFunction.apply`: apply `f` to the Geo Time Series on top of the stack or to each
/// element of a list of Geo Time Series; anything else is pushed back before failing.
pub(super) fn gts_or_list(stack: &mut Stack, name: &str, f: impl Fn(&Gts) -> Value) -> Result<()> {
    gts_or_list_try(stack, name, |gts| Ok(f(gts)))
}

/// [`gts_or_list`] for an operation that can fail.
pub(super) fn gts_or_list_try(
    stack: &mut Stack,
    name: &str,
    f: impl Fn(&Gts) -> Result<Value>,
) -> Result<()> {
    let top = stack.pop()?;
    let result = match &top {
        Value::Gts(gts) => f(gts)?,
        Value::List(list) => {
            let mut results = Vec::with_capacity(list.borrow().len());
            for element in list.borrow().iter() {
                let Value::Gts(gts) = element else {
                    stack.push(top.clone());
                    return Err(fail(name, "can only operate on Geo Time Series instances."));
                };
                results.push(f(gts)?);
            }
            Value::list(results)
        }
        _ => {
            stack.push(top.clone());
            return Err(fail(name, "can only operate on Geo Time Series instances."));
        }
    };
    stack.push(result);
    Ok(())
}

/// Java `((Number) o).longValue()`.
pub(super) fn long_value(value: &Value) -> Option<i64> {
    value.as_i64()
}

/// A map of `STRING` (or `null`) keys and values as the `(name, value)` changes `RELABEL` and
/// `SETATTRIBUTES` apply.
fn string_entries<'a>(
    entries: impl Iterator<Item = (&'a Value, &'a Value)>,
) -> Option<Vec<(Option<&'a str>, Option<&'a str>)>> {
    let mut changes = Vec::new();
    for (key, value) in entries {
        let key = match key {
            Value::Null => None,
            Value::String(s) => Some(&**s),
            _ => return None,
        };
        let value = match value {
            Value::Null => None,
            Value::String(s) => Some(&**s),
            _ => return None,
        };
        changes.push((key, value));
    }
    Some(changes)
}

/// `ATINDEX.getTupleAtIndex`: `[ tick lat lon elevation value ]`, with `NaN` for the absent
/// location and elevation, or `[ NaN NaN NaN NaN null ]` when `idx` is out of range.
pub(super) fn tuple_at_index(gts: &GeoTimeSerie, idx: Option<usize>) -> Value {
    let Some(idx) = idx else {
        return Value::list(vec![
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Null,
        ]);
    };
    let (lat, lon) = match gts.location_at(idx) {
        NO_LOCATION => (f64::NAN, f64::NAN),
        location => geoxp::from_geoxp_point(location),
    };
    let elevation = match gts.elevation_at(idx) {
        NO_ELEVATION => Value::Double(f64::NAN),
        elevation => Value::Long(elevation),
    };
    Value::list(vec![
        Value::Long(gts.tick_at(idx)),
        Value::Double(lat),
        Value::Double(lon),
        elevation,
        gts.value_at(idx),
    ])
}

// -----------------------------------------------------------------------------------------
// Construction
// -----------------------------------------------------------------------------------------

/// `NEWGTS`: an empty Geo Time Series named `""`.
fn newgts(stack: &mut Stack) -> Result<()> {
    let mut gts = GeoTimeSerie::new();
    gts.metadata.name = Some(String::new());
    stack.push(Value::Gts(gts.shared()));
    Ok(())
}

/// `MAKEGTS`: a series built from lists of ticks, latitudes, longitudes, elevations and values,
/// as long as the longest list. Missing ticks follow the previous one, missing values repeat the
/// last one, missing elevations and locations are absent.
fn makegts(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MAKEGTS";
    let mut pop_list = |what: &str| match stack.pop()? {
        Value::List(list) => Ok(list.borrow().clone()),
        _ => Err(fail(NAME, &format!("expects a list of {what}"))),
    };
    let values = pop_list("values on top of the stack.")?;
    let elevations = pop_list("elevations below the list of values.")?;
    let longitudes = pop_list("longitudes below the list of elevations.")?;
    let latitudes = pop_list("latitudes below the list of longitudes.")?;
    let ticks = pop_list("ticks below the list of latitudes.")?;
    // `((Number) o)`: a `NullPointerException` for `null`, a `ClassCastException` otherwise.
    let number = |value: &Value| match value {
        Value::Null => Err(Error::null_pointer()),
        _ if value.is_number() => Ok(value.clone()),
        _ => Err(Error::class_cast(value.kind(), Class::Number)),
    };
    let len = [&values, &elevations, &longitudes, &latitudes, &ticks]
        .iter()
        .map(|list| list.len())
        .max()
        .unwrap_or_default();
    let mut gts = GeoTimeSerie::new();
    let mut lasttick = -1i64;
    for i in 0..len {
        let value = values
            .get(i)
            .or_else(|| values.last())
            .ok_or_else(|| Error::index_out_of_bounds(-1, 0))?;
        let elevation = match elevations.get(i) {
            Some(elevation) => number(elevation)?.as_i64().unwrap_or_default(),
            None => NO_ELEVATION,
        };
        let location = match (latitudes.get(i), longitudes.get(i)) {
            (Some(lat), Some(lon)) => {
                let lat = number(lat)?.as_f64().unwrap_or_default();
                let lon = number(lon)?.as_f64().unwrap_or_default();
                geoxp::to_geoxp_point(lat, lon)
            }
            _ => NO_LOCATION,
        };
        lasttick = match ticks.get(i) {
            Some(tick) => number(tick)?.as_i64().unwrap_or_default(),
            None => lasttick.wrapping_add(1),
        };
        gts.set_value(lasttick, location, elevation, value, false);
    }
    gts.metadata.name = Some(String::new());
    stack.push(Value::Gts(gts.shared()));
    Ok(())
}

/// A datapoint to add: tick, GeoXP location, elevation and value.
pub(crate) struct Datapoint {
    pub(crate) tick: i64,
    pub(crate) location: i64,
    pub(crate) elevation: i64,
    pub(crate) value: Value,
}

/// `MACROMAPPER.listToObjects`: a datapoint given as `[ tick lat lon elevation value ]`,
/// `[ tick lat lon value ]`, `[ tick elevation value ]`, `[ tick value ]` or `[ value ]`.
pub(crate) fn datapoint_from_list(elements: &[Value]) -> Result<Datapoint> {
    let number = |index: usize| -> Result<f64> {
        let value = &elements[index];
        value
            .as_f64()
            .ok_or_else(|| Error::class_cast(value.kind(), Class::Number))
    };
    let location = |lat: usize, lon: usize| -> Result<i64> {
        let (lat, lon) = (number(lat)?, number(lon)?);
        Ok(if lat.is_nan() || lon.is_nan() {
            NO_LOCATION
        } else {
            geoxp::to_geoxp_point(lat, lon)
        })
    };
    let elevation = |index: usize| -> Result<i64> {
        let elevation = number(index)?;
        Ok(if elevation.is_nan() {
            NO_ELEVATION
        } else {
            long_value(&elements[index]).unwrap_or_default()
        })
    };
    let tick = |index: usize| -> Result<i64> {
        number(index)?;
        Ok(long_value(&elements[index]).unwrap_or_default())
    };
    let (tick, location, elevation, value) = match elements.len() {
        5 => (tick(0)?, location(1, 2)?, elevation(3)?, &elements[4]),
        4 => (tick(0)?, location(1, 2)?, NO_ELEVATION, &elements[3]),
        3 => (tick(0)?, NO_LOCATION, elevation(1)?, &elements[2]),
        2 => (tick(0)?, NO_LOCATION, NO_ELEVATION, &elements[1]),
        1 => (0, NO_LOCATION, NO_ELEVATION, &elements[0]),
        n => {
            return Err(SeriesError::DatapointListSize(n).into());
        }
    };
    Ok(Datapoint {
        tick,
        location,
        elevation,
        value: value.clone(),
    })
}

/// `ADDVALUE` / `SETVALUE`: pop a value, an elevation, a longitude, a latitude and a tick (or
/// a single list thereof), then the Geo Time Series to add the datapoint to; `SETVALUE`
/// overwrites the value of an existing tick.
fn add_value(stack: &mut Stack, name: &str, overwrite: bool) -> Result<()> {
    let top = stack.pop()?;
    let datapoint = match &top {
        Value::List(list) => datapoint_from_list(&list.borrow())?,
        Value::Long(_)
        | Value::Double(_)
        | Value::BigDecimal(_)
        | Value::Counter(_)
        | Value::String(_)
        | Value::Boolean(_)
        | Value::Bytes(_)
        | Value::Gts(_)
        | Value::Encoder(_) => {
            let elevation = stack.pop()?;
            if !elevation.is_number() {
                return Err(fail(name, "expects the elevation to be numeric or NaN."));
            }
            let elevation = match elevation {
                Value::Double(d) if d.is_nan() => NO_ELEVATION,
                other => long_value(&other).unwrap_or_default(),
            };
            let Some(longitude) = stack.pop()?.as_f64() else {
                return Err(fail(name, "expects the longitude to be numeric or NaN."));
            };
            let Some(latitude) = stack.pop()?.as_f64() else {
                return Err(fail(name, "expects the latitude to be numeric or NaN."));
            };
            let location = if latitude.is_nan() || longitude.is_nan() {
                NO_LOCATION
            } else {
                geoxp::to_geoxp_point(latitude, longitude)
            };
            let Some(tick) = long_value(&stack.pop()?) else {
                return Err(fail(name, "expects the tick to be numeric."));
            };
            Datapoint {
                tick,
                location,
                elevation,
                value: top.clone(),
            }
        }
        _ => {
            return Err(fail(
                name,
                "expects a LONG, DOUBLE, STRING, byte array or BOOLEAN value or a Geo Time Series or ENCODER.",
            ));
        }
    };
    let target = stack.pop()?;
    // Series and encoders are stored as the bytes of their (compressed, uncounted) wrapper.
    let value = match &datapoint.value {
        Value::Gts(value) => Value::Bytes(
            serialize_wrapper(&gts_to_wrapper(
                &value.borrow(),
                true,
                DEFAULT_COMP_RATIO_THRESHOLD,
                false,
                false,
            ))
            .into(),
        ),
        Value::Encoder(value) => Value::Bytes(
            serialize_wrapper(&encoder_to_wrapper(
                &value.borrow(),
                true,
                DEFAULT_COMP_RATIO_THRESHOLD,
                false,
            ))
            .into(),
        ),
        other => other.clone(),
    };
    match &target {
        Value::Gts(gts) => gts.borrow_mut().set_value(
            datapoint.tick,
            datapoint.location,
            datapoint.elevation,
            &value,
            overwrite,
        ),
        Value::Encoder(encoder) => encoder.borrow_mut().add_value(
            datapoint.tick,
            datapoint.location,
            datapoint.elevation,
            &EncodedValue::from_value(&value)?,
        ),
        _ => {
            return Err(fail(
                name,
                "operates on a single Geo Time Series or GTS Encoder.",
            ));
        }
    }
    stack.push(target);
    Ok(())
}

fn addvalue(stack: &mut Stack) -> Result<()> {
    add_value(stack, "ADDVALUE", false)
}

fn setvalue(stack: &mut Stack) -> Result<()> {
    add_value(stack, "SETVALUE", true)
}

/// `REMOVETICK`: a copy of the Geo Time Series without the datapoints at the given tick (or
/// collection of ticks).
fn removetick(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "REMOVETICK";
    const EXPECTS: &str = "expects a tick (LONG) or a collection thereof on top of the stack.";
    let ticks: BTreeSet<i64> = match stack.pop()? {
        Value::Long(tick) => BTreeSet::from([tick]),
        Value::List(list) => list
            .borrow()
            .iter()
            .map(|v| match v {
                Value::Long(tick) => Ok(*tick),
                _ => Err(fail(NAME, EXPECTS)),
            })
            .collect::<Result<_>>()?,
        Value::Set(set) => set
            .borrow()
            .iter()
            .map(|v| match v {
                Value::Long(tick) => Ok(*tick),
                _ => Err(fail(NAME, EXPECTS)),
            })
            .collect::<Result<_>>()?,
        _ => return Err(fail(NAME, EXPECTS)),
    };
    let Value::Gts(gts) = stack.pop()? else {
        return Err(fail(NAME, "operates on a Geo Time Series."));
    };
    let gts = gts.borrow();
    let mut pruned = gts.clone_empty();
    for i in 0..gts.size() {
        let tick = gts.tick_at(i);
        if ticks.contains(&tick) {
            continue;
        }
        pruned.set_value(
            tick,
            gts.location_at(i),
            gts.elevation_at(i),
            &gts.value_at(i),
            false,
        );
    }
    stack.push(Value::Gts(pruned.shared()));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// Metadata
// -----------------------------------------------------------------------------------------

/// `NAME`: the class name of a Geo Time Series.
fn name(stack: &mut Stack) -> Result<()> {
    let name = pop_metadata(stack, "NAME", |metadata| {
        metadata.name.as_deref().map_or(Value::Null, Value::from)
    })?;
    stack.push(name);
    Ok(())
}

/// `f` applied to the metadata of a Geo Time Series or of an encoder, `None` for any other
/// value.
fn with_metadata<T>(value: &Value, f: impl FnOnce(&mut Metadata) -> T) -> Option<T> {
    match value {
        Value::Gts(gts) => Some(f(&mut gts.borrow_mut().metadata)),
        Value::Encoder(encoder) => Some(f(&mut encoder.borrow_mut().metadata)),
        _ => None,
    }
}

/// [`with_metadata`] on the value popped from the stack, which must be a Geo Time Series or
/// an encoder.
fn pop_metadata<T>(stack: &mut Stack, name: &str, f: impl FnOnce(&mut Metadata) -> T) -> Result<T> {
    with_metadata(&stack.pop()?, f).ok_or_else(|| {
        fail(
            name,
            "expects a Geo Time Series or encoder instance on top of the stack.",
        )
    })
}

/// `RENAME`: set the class name of a Geo Time Series (or of each of a list); a name starting
/// with `+` is appended to the current one.
fn rename(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "RENAME";
    let Value::String(new_name) = stack.pop()? else {
        return Err(fail(NAME, "expects a STRING name."));
    };
    element_or_list(stack, |element| match element {
        Value::Gts(gts) => {
            gts.borrow_mut().rename(&new_name);
            Ok(element.clone())
        }
        Value::Encoder(encoder) => {
            encoder.borrow_mut().metadata.rename(&new_name);
            Ok(element.clone())
        }
        _ => Err(fail(
            NAME,
            "expects a GeoTimeSeries, a GTSEncoder or a list thereof under the new name.",
        )),
    })
}

/// The labels or attributes of the Geo Time Series on top of the stack, as a new map.
fn metadata_map(stack: &mut Stack, name: &str, attributes: bool) -> Result<()> {
    let entries = pop_metadata(stack, name, |metadata| {
        let map = if attributes {
            &metadata.attributes
        } else {
            &metadata.labels
        };
        map.iter()
            .map(|(k, v)| (Value::from(k.as_str()), Value::from(v.as_str())))
            .collect()
    })?;
    stack.push(Value::map(entries));
    Ok(())
}

fn labels(stack: &mut Stack) -> Result<()> {
    metadata_map(stack, "LABELS", false)
}

fn attributes(stack: &mut Stack) -> Result<()> {
    metadata_map(stack, "ATTRIBUTES", true)
}

/// `RELABEL`: apply a map of label changes to a Geo Time Series (or each of a list): a `null`
/// key replaces all the labels, a `null` or empty value removes a label.
fn relabel(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "RELABEL";
    let Value::Map(map) = stack.pop()? else {
        return Err(fail(NAME, "expects a map of labels as parameter."));
    };
    let map = map.borrow();
    let Some(changes) = string_entries(map.iter()) else {
        return Err(fail(NAME, "keys and values MUST be STRING or NULL"));
    };
    element_or_list(stack, |element| {
        with_metadata(element, |metadata| metadata.relabel(&changes))
            .map(|()| element.clone())
            .ok_or_else(|| {
                fail(
                    NAME,
                    "expects a Geo Time Series, a GTSEncoder or a list thereof under the labels \
                     parameter.",
                )
            })
    })
}

/// `SETATTRIBUTES`: apply a map of attribute changes to a Geo Time Series (or each of a
/// list), with the conventions of `RELABEL`.
fn setattributes(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "SETATTRIBUTES";
    let Value::Map(map) = stack.pop()? else {
        return Err(fail(NAME, "expects a map of attributes as parameter."));
    };
    let map = map.borrow();
    element_or_list(stack, |element| {
        if !matches!(element, Value::Gts(_) | Value::Encoder(_)) {
            return Err(fail(
                NAME,
                "expects a GeoTimeSeries, a GTSEncoder or a list thereof under the attribute map.",
            ));
        }
        // Unlike `RELABEL`, the entries are checked for each series, `null` keys skipped.
        let Some(changes) = string_entries(map.iter().filter(|(k, _)| !k.is_null())) else {
            return Err(fail(
                NAME,
                "attribute key and value MUST be of type String.",
            ));
        };
        with_metadata(element, |metadata| {
            if map.contains_key(&Value::Null) {
                metadata.attributes.clear();
            }
            metadata.update_attributes(&changes);
        });
        Ok(element.clone())
    })
}

/// `LASTACTIVITY`: the last activity timestamp of a Geo Time Series, in time units.
fn lastactivity(stack: &mut Stack) -> Result<()> {
    let last_activity = pop_metadata(stack, "LASTACTIVITY", |metadata| metadata.last_activity)?;
    stack.push(Value::Long(last_activity.wrapping_mul(TIME_UNITS_PER_MS)));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// Datapoints
// -----------------------------------------------------------------------------------------

/// `TICKS`: the sorted distinct ticks of a Geo Time Series or of a list of them.
fn ticks(stack: &mut Stack) -> Result<()> {
    const MESSAGE: &str = "expects a Geo Time Series or a list thereof on top of the stack.";
    let mut ticks = BTreeSet::new();
    let mut collect = |value: &Value| -> Result<()> {
        let Value::Gts(gts) = value else {
            return Err(fail("TICKS", MESSAGE));
        };
        ticks.extend(gts.borrow().ticks.iter().copied());
        Ok(())
    };
    match stack.pop()? {
        Value::List(list) => {
            for element in list.borrow().iter() {
                collect(element)?;
            }
        }
        other => collect(&other)?,
    }
    stack.push(Value::list(ticks.into_iter().map(Value::Long).collect()));
    Ok(())
}

/// `TICKLIST`: the ticks of a Geo Time Series (or of each of a list), in storage order.
fn ticklist(stack: &mut Stack) -> Result<()> {
    element_or_list(stack, |element| match element {
        Value::Gts(gts) => Ok(Value::list(
            gts.borrow()
                .ticks
                .iter()
                .copied()
                .map(Value::Long)
                .collect(),
        )),
        Value::Encoder(encoder) => Ok(Value::list(
            decoded(&encoder.borrow())?
                .into_iter()
                .map(|point| Value::Long(point.0))
                .collect(),
        )),
        _ => Err(fail(
            "TICKLIST",
            "expects a GeoTimeSeries, a GTSEncoder or a list thereof on top of the stack.",
        )),
    })
}

/// The datapoints of an encoder, `(tick, location, elevation, binary value)`.
pub(crate) fn decoded(encoder: &GtsEncoder) -> Result<Vec<(i64, i64, i64, EncodedValue)>> {
    let mut decoder = encoder.decoder();
    let mut points = Vec::new();
    while decoder.next()? {
        points.push((
            decoder.timestamp(),
            decoder.location(),
            decoder.elevation(),
            decoder.binary_value(),
        ));
    }
    Ok(points)
}

/// `VALUES`: the values of a Geo Time Series (or of each of a list), in storage order.
fn values(stack: &mut Stack) -> Result<()> {
    element_or_list(stack, |element| match element {
        Value::Gts(gts) => {
            let gts = gts.borrow();
            Ok(Value::list(
                (0..gts.size()).map(|i| gts.value_at(i)).collect(),
            ))
        }
        Value::Encoder(encoder) => Ok(Value::list(
            decoded(&encoder.borrow())?
                .into_iter()
                .map(|point| point.3.to_value())
                .collect(),
        )),
        _ => Err(fail(
            "VALUES",
            "expects a GeoTimeSeries, a GTSEncoder or a list thereof on top of the stack.",
        )),
    })
}

/// `LOCATIONS`: the latitudes then the longitudes of the datapoints (`NaN` when absent).
fn locations(stack: &mut Stack) -> Result<()> {
    let Value::Gts(gts) = stack.pop()? else {
        return Err(fail(
            "LOCATIONS",
            "expects a Geo Time Series on top of the stack.",
        ));
    };
    let gts = gts.borrow();
    let (latitudes, longitudes): (Vec<Value>, Vec<Value>) = (0..gts.size())
        .map(|i| match gts.location_at(i) {
            NO_LOCATION => (Value::Double(f64::NAN), Value::Double(f64::NAN)),
            location => {
                let (lat, lon) = geoxp::from_geoxp_point(location);
                (Value::Double(lat), Value::Double(lon))
            }
        })
        .unzip();
    stack.push(Value::list(latitudes));
    stack.push(Value::list(longitudes));
    Ok(())
}

/// `ELEVATIONS`: the elevations of the datapoints (`NaN` when absent).
fn elevations(stack: &mut Stack) -> Result<()> {
    gts_or_list(stack, "ELEVATIONS", |gts| {
        let gts = gts.borrow();
        Value::list(
            (0..gts.size())
                .map(|i| match gts.elevation_at(i) {
                    NO_ELEVATION => Value::Double(f64::NAN),
                    elevation => Value::Long(elevation),
                })
                .collect(),
        )
    })
}

/// `SORT` / `RSORT`: sort the datapoints by tick, in place.
fn sort_gts(stack: &mut Stack, name: &str, reversed: bool) -> Result<()> {
    gts_or_list(stack, name, |gts| {
        gts.borrow_mut().sort(reversed);
        Value::Gts(gts.clone())
    })
}

fn sort(stack: &mut Stack) -> Result<()> {
    sort_gts(stack, "SORT", false)
}

fn rsort(stack: &mut Stack) -> Result<()> {
    sort_gts(stack, "RSORT", true)
}

/// `FIRSTTICK` / `LASTTICK`: the first (last) tick of a Geo Time Series, or the earliest
/// (latest) over a list of them.
fn boundary_tick(stack: &mut Stack, name: &str, first: bool) -> Result<()> {
    const MESSAGE: &str = "expects a Geo Time Series or a list thereof on top of the stack.";
    let tick_of = |gts: &Gts| {
        let gts = gts.borrow();
        if first {
            gts.first_tick()
        } else {
            gts.last_tick()
        }
    };
    let tick = match stack.pop()? {
        Value::Gts(gts) => tick_of(&gts),
        Value::List(list) => {
            let mut best = if first { i64::MAX } else { i64::MIN };
            for element in list.borrow().iter() {
                let Value::Gts(gts) = element else {
                    return Err(fail(name, MESSAGE));
                };
                let tick = tick_of(gts);
                if (first && tick < best) || (!first && tick > best) {
                    best = tick;
                }
            }
            best
        }
        _ => return Err(fail(name, MESSAGE)),
    };
    stack.push(Value::Long(tick));
    Ok(())
}

fn firsttick(stack: &mut Stack) -> Result<()> {
    boundary_tick(stack, "FIRSTTICK", true)
}

fn lasttick(stack: &mut Stack) -> Result<()> {
    boundary_tick(stack, "LASTTICK", false)
}

/// `CLONEEMPTY`: a Geo Time Series with the same metadata and bucketization but no datapoint.
fn cloneempty(stack: &mut Stack) -> Result<()> {
    element_or_list(stack, |element| match element {
        Value::Gts(gts) => Ok(Value::Gts(gts.borrow().clone_empty().shared())),
        Value::Encoder(encoder) => Ok(Value::Encoder(Rc::new(RefCell::new(
            encoder.borrow().clone_empty(),
        )))),
        _ => Err(fail(
            "CLONEEMPTY",
            "expects a GeoTimeSeries, a GTSEncoder or a list thereof on top of the stack.",
        )),
    })
}

/// `ATINDEX`: the `[ tick lat lon elevation value ]` datapoint at an index (negative indexes
/// count from the end) of a Geo Time Series or of each of a list.
fn atindex(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ATINDEX";
    let Some(idx) = long_value(&stack.pop()?) else {
        return Err(fail(NAME, "expects an index on top of the stack."));
    };
    let idx = idx as i32;
    element_or_list(stack, |element| match element {
        Value::Gts(gts) => {
            let gts = gts.borrow();
            let index = compute_and_check_index(idx, gts.size() as i32)?;
            Ok(tuple_at_index(&gts, usize::try_from(index).ok()))
        }
        _ => Err(fail(
            NAME,
            "expects a Geo Time Series instance or a list thereof under the index.",
        )),
    })
}

/// `ATTICK`: the `[ tick lat lon elevation value ]` datapoint at a tick (`NaN`s and `null`
/// when the tick is absent); the series is sorted as a side effect.
fn attick(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ATTICK";
    let Some(tick) = long_value(&stack.pop()?) else {
        return Err(fail(NAME, "expects a tick."));
    };
    gts_or_list(stack, NAME, |gts| {
        let mut gts = gts.borrow_mut();
        // `GTSHelper.indexAtTick`: sort, then binary search.
        let index = if gts.size() == 0 {
            None
        } else {
            gts.sort(false);
            crate::gts::binary_search(&gts.ticks, tick)
        };
        tuple_at_index(&gts, index)
    })
}

/// `TICKINDEX`: a copy of the Geo Time Series whose ticks are replaced by their index.
fn tickindex(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "TICKINDEX";
    element_or_list(stack, |element| match element {
        Value::Gts(gts) => {
            let mut indexed = gts.borrow().clone();
            for (i, tick) in indexed.ticks.iter_mut().enumerate() {
                *tick = i as i64;
            }
            indexed.bucketcount = 0;
            indexed.bucketspan = 0;
            indexed.lastbucket = 0;
            Ok(Value::Gts(indexed.shared()))
        }
        // A new encoder, without the metadata of the original.
        Value::Encoder(encoder) => {
            let mut indexed = GtsEncoder::new(0);
            let points = decoded(&encoder.borrow()).map_err(|err| {
                fail_with_cause(NAME, "caught an exception while converting encoder.", err)
            })?;
            for (i, (_, location, elevation, value)) in points.into_iter().enumerate() {
                indexed.add_value(i as i64, location, elevation, &value);
            }
            Ok(Value::Encoder(Rc::new(RefCell::new(indexed))))
        }
        // Warp 10 wraps every failure in the message of its encoder conversion.
        _ => Err(fail_with_cause(
            NAME,
            "caught an exception while converting encoder.",
            fail(
                NAME,
                "can only operate on Geo Time Series or GTS Encoder instances.",
            ),
        )),
    })
}

// -----------------------------------------------------------------------------------------
// Bucketization parameters
// -----------------------------------------------------------------------------------------

/// `BUCKETSPAN` / `BUCKETCOUNT` / `LASTBUCKET`: a bucketization parameter, `0` for a series
/// that is not bucketized.
fn bucket_parameter(stack: &mut Stack, name: &str, get: fn(&GeoTimeSerie) -> i64) -> Result<()> {
    let Value::Gts(gts) = stack.pop()? else {
        return Err(fail(name, "expects a Geo Time Series on top of the stack."));
    };
    let gts = gts.borrow();
    let value = if gts.is_bucketized() { get(&gts) } else { 0 };
    stack.push(Value::Long(value));
    Ok(())
}

fn bucketspan(stack: &mut Stack) -> Result<()> {
    bucket_parameter(stack, "BUCKETSPAN", |gts| gts.bucketspan)
}

fn bucketcount(stack: &mut Stack) -> Result<()> {
    bucket_parameter(stack, "BUCKETCOUNT", |gts| gts.bucketcount)
}

fn lastbucket(stack: &mut Stack) -> Result<()> {
    bucket_parameter(stack, "LASTBUCKET", |gts| gts.lastbucket)
}
