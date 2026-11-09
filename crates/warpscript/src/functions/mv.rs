// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Multivariate values (wrapped series or encoders stored as values): `->MVSTRING`,
//! `MVTICKSPLIT`, `MVINDEXSPLIT` and the `MVEXTRACT` family (`MVVALUES`, `MVLOCATIONS`,
//! `MVELEVATIONS`, `MVTICKS`, `MVHHCODES`).

use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::rc::Rc;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use super::Entry;
use super::args::fail;
use super::gts::element_or_list;
use super::gts_reduce::java_hash_order;
use crate::encoder::{
    DEFAULT_COMP_RATIO_THRESHOLD, EncodedValue, GtsEncoder, encoder_to_wrapper, gts_to_wrapper,
    latin1_bytes, wrapper_to_decoder, wrapper_to_encoder,
};
use crate::encoding::opb64_decode;
use crate::error::Result;
use crate::format::{java_double_to_string, url_encode};
use crate::geoxp::from_geoxp_point;
use crate::gts::{GeoTimeSerie, Metadata, NO_ELEVATION, NO_LOCATION};
use crate::stack::Stack;
use crate::thrift::{GtsWrapper, deserialize_wrapper};
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("->MVSTRING", Entry::Stack(tomvstring)),
    (
        "MVTICKSPLIT",
        Entry::Stack(|s| mvsplit(s, "MVTICKSPLIT", true)),
    ),
    (
        "MVINDEXSPLIT",
        Entry::Stack(|s| mvsplit(s, "MVINDEXSPLIT", false)),
    ),
    (
        "MVVALUES",
        Entry::Stack(|s| mvextract(s, "MVVALUES", Element::Value)),
    ),
    (
        "MVLOCATIONS",
        Entry::Stack(|s| mvextract(s, "MVLOCATIONS", Element::LatLon)),
    ),
    (
        "MVELEVATIONS",
        Entry::Stack(|s| mvextract(s, "MVELEVATIONS", Element::Elevation)),
    ),
    (
        "MVTICKS",
        Entry::Stack(|s| mvextract(s, "MVTICKS", Element::Tick)),
    ),
    (
        "MVHHCODES",
        Entry::Stack(|s| mvextract(s, "MVHHCODES", Element::Location)),
    ),
];

/// A datapoint: tick, location, elevation and value.
type Point = (i64, i64, i64, EncodedValue);

/// The datapoints of an encoder, with their binary values.
fn encoder_points(encoder: &GtsEncoder) -> Result<Vec<Point>> {
    super::gts::decoded(encoder)
}

/// The datapoints of a series.
fn gts_points(gts: &GeoTimeSerie) -> Vec<Point> {
    (0..gts.size())
        .map(|i| {
            (
                gts.tick_at(i),
                gts.location_at(i),
                gts.elevation_at(i),
                EncodedValue::from_value(&gts.value_at(i)).unwrap_or(EncodedValue::Null),
            )
        })
        .collect()
}

/// `TDeserializer.deserialize` of a value that may be a wrapper: the byte arrays, and the
/// strings (as ISO-8859-1) when `strings` is set. `None` when the value is not one of those
/// or is not a wrapper (a `TException`); the runtime exceptions of the deserializer escape.
fn as_wrapper(value: &EncodedValue, strings: bool) -> Result<Option<GtsWrapper>> {
    let bytes = match value {
        EncodedValue::Bytes(bytes) => bytes.to_vec(),
        EncodedValue::String(s) if strings => latin1_bytes(s),
        _ => return Ok(None),
    };
    match deserialize_wrapper(&bytes) {
        Ok(wrapper) => Ok(Some(wrapper)),
        // Not a wrapper: the value is a plain string or byte array, not a multivariate value.
        Err(err) if err.is_malformed_input() => Ok(None),
        Err(err) => Err(err.into()),
    }
}

// -----------------------------------------------------------------------------------------
// ->MVSTRING
// -----------------------------------------------------------------------------------------

/// `TOMVSTRING.wrapperToString`: the datapoints of a wrapper as a `[ … ]` (compressed) or
/// `[! … ]` multivariate literal.
fn wrapper_to_string(out: &mut String, wrapper: &GtsWrapper) -> Result<()> {
    out.push_str(if wrapper.is_compressed() { "[ " } else { "[! " });
    let mut decoder = wrapper_to_decoder(wrapper)?;
    while decoder.next()? {
        let (ts, location, elevation) =
            (decoder.timestamp(), decoder.location(), decoder.elevation());
        let latlon = (location != NO_LOCATION).then(|| from_geoxp_point(location));
        if ts != 0 || latlon.is_some() || elevation != NO_ELEVATION {
            let _ = write!(out, "{ts}/");
        }
        if let Some((lat, lon)) = latlon {
            let _ = write!(
                out,
                "{}:{}/",
                java_double_to_string(lat),
                java_double_to_string(lon)
            );
        } else if elevation != NO_ELEVATION {
            out.push('/');
        }
        if elevation != NO_ELEVATION {
            let _ = write!(out, "{elevation}/");
        }
        match decoder.binary_value() {
            EncodedValue::Bytes(bytes) => bytes_to_string(out, &bytes)?,
            EncodedValue::String(s) => {
                let _ = write!(out, "'{}'", url_encode(&s));
            }
            EncodedValue::Boolean(b) => out.push(if b { 'T' } else { 'F' }),
            EncodedValue::Decimal(d) => {
                let _ = write!(out, "{d}");
            }
            // `LONG`s and `DOUBLE`s (a decoded datapoint is never `null`).
            other => {
                let _ = write!(out, "{}", other.to_value());
            }
        }
        out.push(' ');
    }
    out.push(']');
    Ok(())
}

/// `TOMVSTRING.bytesToString`: a wrapper, or `b64:` and the URL safe base64 of the bytes.
fn bytes_to_string(out: &mut String, bytes: &[u8]) -> Result<()> {
    match as_wrapper(&EncodedValue::Bytes(Rc::from(bytes)), false)? {
        Some(wrapper) => wrapper_to_string(out, &wrapper),
        None => {
            out.push_str("b64:");
            out.push_str(&URL_SAFE_NO_PAD.encode(bytes));
            Ok(())
        }
    }
}

/// `->MVSTRING`: the multivariate literal of a series, an encoder or a wrapper (bytes or
/// OPB64 string).
fn tomvstring(stack: &mut Stack) -> Result<()> {
    let top = stack.pop()?;
    let mut out = String::new();
    let wrapper = match &top {
        Value::Encoder(encoder) => Ok(encoder_to_wrapper(
            &encoder.borrow(),
            false,
            DEFAULT_COMP_RATIO_THRESHOLD,
            true,
        )),
        Value::Gts(gts) => Ok(gts_to_wrapper(
            &gts.borrow(),
            false,
            DEFAULT_COMP_RATIO_THRESHOLD,
            false,
            true,
        )),
        Value::Bytes(bytes) => Err(bytes.to_vec()),
        Value::String(s) => {
            let ascii: String = s
                .chars()
                .map(|c| if c.is_ascii() { c } else { '?' })
                .collect();
            Err(opb64_decode(&ascii)?)
        }
        _ => {
            return Err(fail(
                "->MVSTRING",
                "operates on a Geo Time Series, ENCODER, STRING or byte array.",
            ));
        }
    };
    // A series or encoder, or the bytes of a wrapper.
    match wrapper {
        Ok(wrapper) => wrapper_to_string(&mut out, &wrapper)?,
        Err(bytes) => bytes_to_string(&mut out, &bytes)?,
    }
    stack.push(out);
    Ok(())
}

// -----------------------------------------------------------------------------------------
// MVEXTRACT
// -----------------------------------------------------------------------------------------

/// The element `MVEXTRACT` extracts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Element {
    Tick,
    Location,
    LatLon,
    Elevation,
    Value,
}

/// `MVEXTRACT.elt`: the element of a datapoint.
fn element_of(element: Element, point: &Point) -> Value {
    let (tick, location, elevation, value) = point;
    match element {
        Element::Value => value.to_value(),
        Element::Tick => Value::Long(*tick),
        Element::LatLon => {
            let (lat, lon) = if *location == NO_LOCATION {
                (f64::NAN, f64::NAN)
            } else {
                from_geoxp_point(*location)
            };
            Value::list(vec![Value::Double(lat), Value::Double(lon)])
        }
        Element::Elevation => Value::Long(*elevation),
        Element::Location => Value::Long(*location),
    }
}

/// `MVEXTRACT.mvextract`: the elements of the datapoints, the wrapped values replaced by the
/// elements of their own datapoints (with the element of the datapoint holding them, but for
/// values). The strings of a series may be wrappers, those of an encoder are not.
fn extract(points: Vec<Point>, from_encoder: bool, element: Element) -> Result<Value> {
    let mut values = Vec::with_capacity(points.len());
    for point in points {
        match as_wrapper(&point.3, !from_encoder)? {
            Some(wrapper) => {
                let encoder = wrapper_to_encoder(&wrapper)?;
                let nested = extract(encoder_points(&encoder)?, true, element)?;
                if element == Element::Value {
                    values.push(nested);
                } else {
                    values.push(Value::list(vec![element_of(element, &point), nested]));
                }
            }
            None => values.push(element_of(element, &point)),
        }
    }
    Ok(Value::list(values))
}

/// `MVVALUES`, `MVLOCATIONS`, `MVELEVATIONS`, `MVTICKS` and `MVHHCODES`.
fn mvextract(stack: &mut Stack, name: &str, element: Element) -> Result<()> {
    element_or_list(stack, |value| match value {
        Value::Gts(gts) => extract(gts_points(&gts.borrow()), false, element),
        Value::Encoder(encoder) => extract(encoder_points(&encoder.borrow())?, true, element),
        _ => Err(fail(
            name,
            "can only be applied on Geo Time Series or GTS Encoders.",
        )),
    })
}

// -----------------------------------------------------------------------------------------
// MVTICKSPLIT / MVINDEXSPLIT
// -----------------------------------------------------------------------------------------

/// The datapoints of the wrapped values `MVSPLIT` keeps.
struct Selection {
    ticks: HashSet<i64>,
    ranges: Vec<(i64, i64)>,
    check: bool,
    include_zero: bool,
    maxindex: i64,
}

impl Selection {
    fn keeps(&self, index: i64) -> bool {
        !self.check
            || self.ticks.contains(&index)
            || self
                .ranges
                .iter()
                .any(|&(low, high)| index >= low && index <= high)
    }

    /// Parse a list of ticks and `[ low high ]` ranges.
    fn parse(name: &str, list: &[Value]) -> Result<Selection> {
        const MESSAGE: &str = "expects ticks or ranges of ticks (LONGs).";
        let mut selection = Selection {
            ticks: HashSet::new(),
            ranges: Vec::new(),
            check: true,
            include_zero: false,
            maxindex: i64::MIN,
        };
        for element in list {
            match element {
                Value::Long(tick) => {
                    selection.ticks.insert(*tick);
                    selection.include_zero |= *tick == 0;
                    selection.maxindex = selection.maxindex.max(*tick);
                }
                Value::List(range) => {
                    let (low, high) = match range.borrow().as_slice() {
                        [Value::Long(a), Value::Long(b)] => (*a.min(b), *a.max(b)),
                        _ => return Err(fail(name, MESSAGE)),
                    };
                    selection.ranges.push((low, high));
                    selection.maxindex = selection.maxindex.max(high);
                }
                _ => return Err(fail(name, MESSAGE)),
            }
        }
        Ok(selection)
    }
}

/// The encoders of the split of one input, by index, in insertion order.
struct Split<'a> {
    metadata: &'a Metadata,
    encoders: indexmap::IndexMap<i64, GtsEncoder>,
}

impl Split<'_> {
    fn add(&mut self, index: i64, point: (i64, i64, i64, &EncodedValue)) {
        let metadata = self.metadata;
        self.encoders
            .entry(index)
            .or_insert_with(|| {
                let mut encoder = GtsEncoder::new(0);
                encoder.metadata = metadata.clone();
                encoder
            })
            .add_value(point.0, point.1, point.2, point.3);
    }
}

/// Split the datapoints of one input.
fn split_input(
    split: &mut Split<'_>,
    points: Vec<Point>,
    selection: &Selection,
    bytick: bool,
) -> Result<()> {
    for (ts, location, elevation, value) in points {
        // Unlike `MVEXTRACT`, the strings of encoders are tried too.
        match as_wrapper(&value, true)? {
            Some(wrapper) => {
                let mut decoder = wrapper_to_decoder(&wrapper)?;
                let mut index = 0i64;
                while decoder.next()? {
                    if bytick {
                        index = decoder.timestamp();
                    }
                    if selection.keeps(index) {
                        let dlocation = decoder.location();
                        let delevation = decoder.elevation();
                        split.add(
                            index,
                            (
                                ts,
                                if dlocation == NO_LOCATION {
                                    location
                                } else {
                                    dlocation
                                },
                                if delevation == NO_ELEVATION {
                                    elevation
                                } else {
                                    delevation
                                },
                                &decoder.binary_value(),
                            ),
                        );
                    }
                    index = index.wrapping_add(1);
                    if !bytick && selection.check && index > selection.maxindex {
                        break;
                    }
                }
            }
            None if selection.include_zero => {
                split.add(0, (ts, location, elevation, &value));
            }
            None => {}
        }
    }
    Ok(())
}

/// `MVTICKSPLIT` / `MVINDEXSPLIT`: split the wrapped values of series or encoders into one
/// encoder per tick (or index) of the wrapped datapoints.
fn mvsplit(stack: &mut Stack, name: &str, bytick: bool) -> Result<()> {
    const INPUT: &str = "operates on Geo Time Series or ENCODER or a list thereof.";
    let mut top = stack.pop()?;
    let renaming = match &top {
        Value::Map(map) => {
            let map = map.borrow().clone();
            top = stack.pop()?;
            map
        }
        _ => indexmap::IndexMap::new(),
    };
    let mut selection = Selection {
        ticks: HashSet::new(),
        ranges: Vec::new(),
        check: false,
        include_zero: false,
        maxindex: i64::MIN,
    };
    if let Value::List(list) = &top {
        let elements = list.borrow().clone();
        let series = elements
            .iter()
            .all(|e| matches!(e, Value::Gts(_) | Value::Encoder(_)));
        if !series {
            selection = Selection::parse(name, &elements)?;
            top = stack.pop()?;
        }
    } else {
        selection.include_zero = true;
    }
    let (inputs, list_input) = match &top {
        Value::Gts(_) | Value::Encoder(_) => (vec![top.clone()], false),
        Value::List(list) => (list.borrow().clone(), true),
        _ => return Err(fail(name, INPUT)),
    };
    // The `HashMap` of the encoders is cleared between inputs: it keeps its table size.
    let mut capacity = 16usize;
    let mut outputs = Vec::with_capacity(inputs.len());
    for input in &inputs {
        let (metadata, points) = match input {
            Value::Encoder(encoder) => {
                let encoder = encoder.borrow();
                (encoder.metadata.clone(), encoder_points(&encoder)?)
            }
            Value::Gts(gts) => {
                let gts = gts.borrow();
                (gts.metadata.clone(), gts_points(&gts))
            }
            _ => return Err(fail(name, INPUT)),
        };
        let mut split = Split {
            metadata: &metadata,
            encoders: indexmap::IndexMap::new(),
        };
        split_input(&mut split, points, &selection, bytick)?;
        let mut encoders = split.encoders;
        for (index, encoder) in &mut encoders {
            let new_name = renaming
                .get(&Value::Long(*index))
                .filter(|v| !matches!(v, Value::Null))
                .or_else(|| {
                    renaming
                        .get(&Value::from(index.to_string()))
                        .filter(|v| !matches!(v, Value::Null))
                });
            let old = encoder
                .metadata
                .name
                .clone()
                .unwrap_or_else(|| "null".to_owned());
            encoder.metadata.name = Some(match new_name {
                None => format!("{old}:{index}"),
                Some(new_name) => {
                    let new_name = new_name.to_string();
                    match new_name.strip_prefix('+') {
                        Some(suffix) => format!("{old}{suffix}"),
                        None => new_name,
                    }
                }
            });
        }
        let hashes: Vec<i32> = encoders
            .keys()
            .map(|index| (index ^ (index >> 32)) as i32)
            .collect();
        let mut slots: Vec<Option<GtsEncoder>> = encoders.into_values().map(Some).collect();
        let ordered: Vec<Value> = java_hash_order(&hashes, capacity)
            .into_iter()
            .filter_map(|i| slots[i].take())
            .map(|encoder| Value::Encoder(Rc::new(RefCell::new(encoder))))
            .collect();
        while hashes.len() > capacity * 3 / 4 {
            capacity *= 2;
        }
        outputs.push(Value::list(ordered));
    }
    let result = if list_input {
        Value::list(outputs)
    } else {
        outputs.pop().unwrap_or(Value::Null)
    };
    stack.push(result);
    Ok(())
}
