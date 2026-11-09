// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `GEOSHAPE`: `->GEOSHAPE`, `GEOSHAPE->`, `->GEOHASH`, `GEOHASH->`, `GEOPACK` and
//! `GEOUNPACK`.

use std::collections::BTreeSet;
use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::geo::{geocell_prefix, hex16, hh_and_res_for};
use crate::encoding::opb64_decode;
use crate::error::{Error, Result};
use crate::geohash::{from_geocells_optimized, from_hhcode, to_geocells, to_hhcode};
use crate::geoshape::GeoShape;
use crate::geoxp::{from_geoxp_point, to_geocell, to_geoxp_point};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("->GEOSHAPE", Entry::Stack(togeoshape)),
    ("GEOSHAPE->", Entry::Stack(geoshapeto)),
    ("->GEOHASH", Entry::Stack(togeohash)),
    ("GEOHASH->", Entry::Stack(geohashto)),
    ("GEOPACK", Entry::Stack(geopack)),
    ("GEOUNPACK", Entry::Stack(geounpack)),
];

/// The `GEOSHAPE` stack object of `cells`, which are sorted.
fn shape_value(cells: Vec<i64>) -> Value {
    Value::GeoShape(Rc::new(GeoShape::from_cells(cells)))
}

/// `->GEOSHAPE`: the shape of a list of geocells (`LONG`) and HHCode prefixes (`STRING` or
/// `BYTES`).
fn togeoshape(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "->GEOSHAPE";
    const MESSAGE: &str = "expects a LIST of LONG,STRING or BYTES representing geocells.";
    let Value::List(list) = stack.pop()? else {
        return Err(fail(NAME, MESSAGE));
    };
    let mut geocells = BTreeSet::new();
    for cell in list.borrow().iter() {
        let geocell = match cell {
            Value::Long(geocell) => *geocell,
            Value::String(_) | Value::Bytes(_) => {
                let (hhcode, resolution) = hh_and_res_for(
                    NAME,
                    cell,
                    "expects the given list to contain valid STRING or BYTES HHCodes.",
                )?;
                to_geocell(hhcode, resolution)
            }
            _ => return Err(fail(NAME, MESSAGE)),
        };
        if (geocell as u64) >> 60 == 0 {
            return Err(fail(
                NAME,
                "expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ));
        }
        geocells.insert(geocell);
    }
    stack.push(shape_value(geocells.into_iter().collect()));
    Ok(())
}

/// `GEOSHAPE->`: the geocells of a shape, as `LONG`s or (with `true` on top) as HHCode
/// prefixes.
fn geoshapeto(stack: &mut Stack) -> Result<()> {
    let mut top = stack.pop()?;
    let mut tostring = false;
    if let Value::Boolean(flag) = top {
        tostring = flag;
        top = stack.pop()?;
    }
    let Value::GeoShape(shape) = top else {
        return Err(fail("GEOSHAPE->", "expects a GEOSHAPE."));
    };
    let cells = shape
        .cells()
        .iter()
        .map(|cell| {
            if tostring {
                Value::from(geocell_prefix(*cell))
            } else {
                Value::Long(*cell)
            }
        })
        .collect();
    stack.push(Value::list(cells));
    Ok(())
}

/// `->GEOHASH`: the GeoHashes covering a shape, or the GeoHash of an HHCode or of a latitude
/// and longitude.
fn togeohash(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "->GEOHASH";
    match stack.pop()? {
        Value::GeoShape(shape) => {
            let geohashes = from_geocells_optimized(shape.cells());
            stack.push(Value::list(
                geohashes.into_iter().map(Value::from).collect(),
            ));
        }
        Value::Long(hhcode) => stack.push(Value::from(from_hhcode(hhcode, 32))),
        Value::Double(lon) => {
            let Value::Double(lat) = stack.pop()? else {
                return Err(fail(
                    NAME,
                    "expects a latitude and a longitude on the stack.",
                ));
            };
            stack.push(Value::from(from_hhcode(to_geoxp_point(lat, lon), 32)));
        }
        _ => {
            return Err(fail(
                NAME,
                "operates on a HHCode, a GEOSHAPE or a latitude and longitude.",
            ));
        }
    }
    Ok(())
}

/// `GEOHASH->`: the shape covering a list of GeoHashes, or the latitude and longitude of a
/// GeoHash (its HHCode, as a string or a `LONG`, with a boolean on top).
fn geohashto(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "GEOHASH->";
    const MESSAGE: &str = "expects a GeoHash to be a string.";
    let mut top = stack.pop()?;
    if let Value::List(list) = &top {
        let mut geohashes = Vec::new();
        for geohash in list.borrow().iter() {
            let Value::String(geohash) = geohash else {
                return Err(fail(NAME, MESSAGE));
            };
            geohashes.push(geohash.to_lowercase());
        }
        stack.push(shape_value(to_geocells(&geohashes)?));
        return Ok(());
    }
    let mut hhcode_string = None;
    if let Value::Boolean(flag) = top {
        hhcode_string = Some(flag);
        top = stack.pop()?;
    }
    let Value::String(geohash) = top else {
        return Err(fail(NAME, MESSAGE));
    };
    let hhcode = to_hhcode(&geohash.to_lowercase())?;
    match hhcode_string {
        Some(true) => stack.push(Value::from(hex16(hhcode))),
        Some(false) => stack.push(Value::Long(hhcode)),
        None => {
            let (lat, lon) = from_geoxp_point(hhcode);
            stack.push(Value::Double(lat));
            stack.push(Value::Double(lon));
        }
    }
    Ok(())
}

/// `GEOPACK`: the packed form of a shape.
fn geopack(stack: &mut Stack) -> Result<()> {
    let Value::GeoShape(shape) = stack.pop()? else {
        return Err(fail("GEOPACK", "expects a shape on top of the stack."));
    };
    stack.push(Value::from(shape.pack()));
    Ok(())
}

/// `GEOUNPACK`: the shape of a packed one (a string, or the raw bytes).
fn geounpack(stack: &mut Stack) -> Result<()> {
    let serialized = match stack.pop()? {
        Value::String(packed) => {
            // `getBytes(US_ASCII)`: the other characters are `?`.
            let ascii: String = packed
                .chars()
                .map(|c| if c.is_ascii() { c } else { '?' })
                .collect();
            opb64_decode(&ascii)?
        }
        Value::Bytes(bytes) => bytes.to_vec(),
        _ => {
            return Err(fail(
                "GEOUNPACK",
                "expects a packed shape on top of the stack.",
            ));
        }
    };
    // A `TException` is the cause of the `WarpScriptException(te)` which reports it.
    let shape = GeoShape::unpack(&serialized).map_err(|err| {
        if err.is_runtime() {
            err
        } else {
            Error::wrapped(err)
        }
    })?;
    stack.push(shape_value(shape.cells().to_vec()));
    Ok(())
}
