// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! HHCodes and geocells: `->HHCODE`, `->HHCODELONG`, `->GTSHHCODE`, `->GTSHHCODELONG`,
//! `HHCODE->`, `GTSHHCODE->`, `->GEOCELL`, `GEOCELL->`, the `HHCODE.*` neighbours, bounding
//! box and center, `HAVERSINE` and `COPYGEO`.

use std::rc::Rc;

use super::Entry;
use super::args::{fail, fail_with_cause};
use super::gts::gts_or_list_try;
use crate::error::{Error, Result};
use crate::geoxp::{
    delta_hhcode, from_geoxp_point, hhcode_bbox, hhcode_center, hhcode_to_string, to_geocell,
    to_geoxp_point,
};
use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION};
use crate::stack::Stack;
use crate::value::Value;

/// What an `HHCODE.*` function computes.
#[derive(Clone, Copy)]
enum Action {
    /// A neighbour, `(latitude, longitude)` cells away.
    Delta(i64, i64),
    Bbox,
    Center,
}

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "->HHCODE",
        Entry::Stack(|s| tohhcode(s, "->HHCODE", true, false)),
    ),
    (
        "->HHCODELONG",
        Entry::Stack(|s| tohhcode(s, "->HHCODELONG", false, false)),
    ),
    (
        "->GTSHHCODE",
        Entry::Stack(|s| tohhcode(s, "->GTSHHCODE", true, true)),
    ),
    (
        "->GTSHHCODELONG",
        Entry::Stack(|s| tohhcode(s, "->GTSHHCODELONG", false, true)),
    ),
    ("HHCODE->", Entry::Stack(|s| hhcodeto(s, "HHCODE->", false))),
    (
        "GTSHHCODE->",
        Entry::Stack(|s| hhcodeto(s, "GTSHHCODE->", true)),
    ),
    ("->GEOCELL", Entry::Stack(togeocell)),
    ("GEOCELL->", Entry::Stack(geocellto)),
    (
        "HHCODE.BBOX",
        Entry::Stack(|s| hhcodefunc(s, "HHCODE.BBOX", Action::Bbox)),
    ),
    (
        "HHCODE.CENTER",
        Entry::Stack(|s| hhcodefunc(s, "HHCODE.CENTER", Action::Center)),
    ),
    (
        "HHCODE.NORTH",
        Entry::Stack(|s| hhcodefunc(s, "HHCODE.NORTH", Action::Delta(1, 0))),
    ),
    (
        "HHCODE.SOUTH",
        Entry::Stack(|s| hhcodefunc(s, "HHCODE.SOUTH", Action::Delta(-1, 0))),
    ),
    (
        "HHCODE.EAST",
        Entry::Stack(|s| hhcodefunc(s, "HHCODE.EAST", Action::Delta(0, 1))),
    ),
    (
        "HHCODE.WEST",
        Entry::Stack(|s| hhcodefunc(s, "HHCODE.WEST", Action::Delta(0, -1))),
    ),
    (
        "HHCODE.NORTH.EAST",
        Entry::Stack(|s| hhcodefunc(s, "HHCODE.NORTH.EAST", Action::Delta(1, 1))),
    ),
    (
        "HHCODE.NORTH.WEST",
        Entry::Stack(|s| hhcodefunc(s, "HHCODE.NORTH.WEST", Action::Delta(1, -1))),
    ),
    (
        "HHCODE.SOUTH.EAST",
        Entry::Stack(|s| hhcodefunc(s, "HHCODE.SOUTH.EAST", Action::Delta(-1, 1))),
    ),
    (
        "HHCODE.SOUTH.WEST",
        Entry::Stack(|s| hhcodefunc(s, "HHCODE.SOUTH.WEST", Action::Delta(-1, -1))),
    ),
    ("HAVERSINE", Entry::Stack(haversine)),
    ("COPYGEO", Entry::Stack(copygeo)),
];

/// The 16 lowercase hexadecimal digits of an HHCode (`Hex.encodeHexString(Longs.toByteArray)`).
fn hex16(hhcode: i64) -> String {
    format!("{hhcode:016x}")
}

/// `Long.parseUnsignedLong(s, 16)` of a 16 character string.
fn parse_unsigned_hex(s: &str) -> Result<i64> {
    let invalid = || {
        Error::runtime(
            "java.lang.NumberFormatException",
            format!("For input string: \"{s}\" under radix 16"),
        )
    };
    let digits = match s.strip_prefix('+') {
        Some(rest) => rest,
        None if s.starts_with('-') => {
            return Err(Error::runtime(
                "java.lang.NumberFormatException",
                format!("Illegal leading minus sign on unsigned string {s}."),
            ));
        }
        None => s,
    };
    let mut value: u64 = 0;
    for c in digits.chars() {
        let digit = c.to_digit(16).ok_or_else(invalid)?;
        value = (value << 4) | u64::from(digit);
    }
    Ok(value as i64)
}

/// `HHCODEFUNC.hhAndRes`: the HHCode and resolution of a `LONG` (32), a hexadecimal string
/// (twice its length, right padded with zeros) or a byte array (four times its length).
fn hh_and_res(value: &Value) -> Result<(i64, i32)> {
    match value {
        Value::Long(hhcode) => Ok((*hhcode, 32)),
        Value::String(s) => {
            let len = s.encode_utf16().count();
            if len > 16 {
                return Err(Error::new(
                    "Hexadecimal HHCode string of must be of length <= 16",
                ));
            }
            let padded = format!("{s}{}", "0".repeat(16 - len));
            Ok((parse_unsigned_hex(&padded)?, (len * 2) as i32))
        }
        Value::Bytes(bytes) => {
            if bytes.len() > 8 {
                return Err(Error::new("Byte array HHCode must be of length <= 8"));
            }
            let mut padded = [0u8; 8];
            padded[..bytes.len()].copy_from_slice(bytes);
            Ok((i64::from_be_bytes(padded), (bytes.len() * 4) as i32))
        }
        _ => Err(Error::new(
            "HHCode must be represented as a long, a string or a byte array.",
        )),
    }
}

/// [`hh_and_res`] for `name`: its script errors become the cause of `<name> <message>`, the
/// `NumberFormatException` of a bad hexadecimal string goes through.
fn hh_and_res_for(name: &str, value: &Value, message: &str) -> Result<(i64, i32)> {
    hh_and_res(value).map_err(|err| {
        if err.is_runtime() {
            err
        } else {
            fail_with_cause(name, message, err)
        }
    })
}

/// `->HHCODE` and friends: the HHCode of a latitude and longitude, as a string or a `LONG`;
/// with the Geo Time Series convention, `NaN NaN` is no location.
fn tohhcode(stack: &mut Stack, name: &str, tostring: bool, gts_convention: bool) -> Result<()> {
    let lon = stack.pop()?;
    let lat = stack.pop()?;
    let (Some(lat), Some(lon)) = (lat.as_f64(), lon.as_f64()) else {
        return Err(fail(
            name,
            "expects a latitude and a longitude on the stack.",
        ));
    };
    let hhcode = if gts_convention && (lat.is_nan() || lon.is_nan()) {
        if lat.is_nan() != lon.is_nan() {
            return Err(fail(
                name,
                "expects latitude and longitude to both be NaN or both be not NaN",
            ));
        }
        NO_LOCATION
    } else {
        to_geoxp_point(lat, lon)
    };
    stack.push(if tostring {
        Value::from(hex16(hhcode))
    } else {
        Value::Long(hhcode)
    });
    Ok(())
}

/// `HHCODE->` / `GTSHHCODE->`: the latitude and longitude of an HHCode (`NaN NaN` for no
/// location with the Geo Time Series convention).
fn hhcodeto(stack: &mut Stack, name: &str, gts_convention: bool) -> Result<()> {
    let top = stack.pop()?;
    let (hhcode, _) = hh_and_res_for(name, &top, "was given unexpected arguments.")?;
    let (lat, lon) = if gts_convention && hhcode == NO_LOCATION {
        (f64::NAN, f64::NAN)
    } else {
        from_geoxp_point(hhcode)
    };
    stack.push(Value::Double(lat));
    stack.push(Value::Double(lon));
    Ok(())
}

/// `->GEOCELL`: the geocell of an HHCode prefix (string or bytes) of an even resolution.
fn togeocell(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "->GEOCELL";
    const MESSAGE: &str = "expects a STRING or a BYTES.";
    let top = stack.pop()?;
    if !matches!(top, Value::String(_) | Value::Bytes(_)) {
        return Err(fail(NAME, MESSAGE));
    }
    let (hhcode, resolution) = hh_and_res_for(NAME, &top, MESSAGE)?;
    let geocell = to_geocell(hhcode, resolution);
    if (geocell as u64) >> 60 == 0 {
        return Err(fail(
            NAME,
            "expects the geocell resolution to be even and between 2 and 30, inclusive.",
        ));
    }
    stack.push(Value::Long(geocell));
    Ok(())
}

/// `GEOCELL->`: the HHCode prefix of a geocell (`Long.toHexString(geocell).substring(1, n)`).
fn geocellto(stack: &mut Stack) -> Result<()> {
    let Value::Long(geocell) = stack.pop()? else {
        return Err(fail("GEOCELL->", "expects a LONG geocell."));
    };
    let hex = format!("{:x}", geocell as u64);
    // The hexadecimal form of a geocell has 16 digits whenever its resolution is not 0.
    let end = ((geocell as u64) >> 60) as usize + 1;
    stack.push(Value::from(hex.get(1..end).unwrap_or_default().to_owned()));
    Ok(())
}

/// `HHCODEFUNC`: a neighbour, the bounding box or the center of the cell of an HHCode, at
/// its resolution or at the one given on top of the stack.
fn hhcodefunc(stack: &mut Stack, name: &str, action: Action) -> Result<()> {
    let mut top = stack.pop()?;
    let mut resolution_override = None;
    let number = match &top {
        // `Number.intValue()`.
        Value::Long(l) => Some(*l as i32),
        Value::Double(d) => Some(*d as i32),
        _ => None,
    };
    if let Some(resolution) = number {
        if !(0..=32).contains(&resolution) {
            return Err(fail(
                name,
                "expects a resolution which is an long between 0 and 32, inclusive.",
            ));
        }
        resolution_override = Some(resolution);
        top = stack.pop()?;
    }
    let (hhcode, resolution) = hh_and_res_for(name, &top, "was given unexpected arguments.")?;
    let resolution = resolution_override.unwrap_or(resolution);
    match action {
        Action::Delta(lat, lon) => {
            let neighbour = delta_hhcode(hhcode, resolution, lat, lon);
            stack.push(match &top {
                Value::Bytes(_) => Value::bytes(neighbour.to_be_bytes()),
                Value::String(_) => Value::from(hhcode_to_string(neighbour, resolution)),
                _ => Value::Long(neighbour),
            });
        }
        Action::Bbox => {
            for bound in hhcode_bbox(hhcode, resolution) {
                stack.push(Value::Double(bound));
            }
        }
        Action::Center => {
            let (lat, lon) = from_geoxp_point(hhcode_center(hhcode, resolution));
            stack.push(Value::Double(lat));
            stack.push(Value::Double(lon));
        }
    }
    Ok(())
}

/// `HAVERSINE`: the great circle distance in meters between two points (Earth radius
/// 6371 km).
fn haversine(stack: &mut Stack) -> Result<()> {
    const EARTH_RADIUS: f64 = 6_371_000.0;
    let lon2 = stack.pop()?;
    let lat2 = stack.pop()?;
    let lon1 = stack.pop()?;
    let lat1 = stack.pop()?;
    let (Some(lat1), Some(lon1), Some(lat2), Some(lon2)) =
        (lat1.as_f64(), lon1.as_f64(), lat2.as_f64(), lon2.as_f64())
    else {
        return Err(fail(
            "HAVERSINE",
            "expects two sets of lat/lon coordinates.",
        ));
    };
    let (rlat1, rlon1) = (lat1.to_radians(), lon1.to_radians());
    let (rlat2, rlon2) = (lat2.to_radians(), lon2.to_radians());
    let sinphi2 = ((rlat2 - rlat1) / 2.0).sin().powi(2);
    let sinlambda2 = ((rlon2 - rlon1) / 2.0).sin().powi(2);
    let d = 2.0
        * EARTH_RADIUS
        * (sinphi2 + rlat1.cos() * rlat2.cos() * sinlambda2)
            .sqrt()
            .asin();
    stack.push(Value::Double(d));
    Ok(())
}

/// `GTSHelper.setLocationAtIndex` / `setElevationAtIndex`: the array appears on the first
/// value which is not the absent one.
fn set_at(column: &mut Option<Vec<i64>>, size: usize, idx: usize, value: i64, absent: i64) {
    match column {
        Some(values) => values[idx] = value,
        None if value != absent => {
            let mut values = vec![absent; size];
            values[idx] = value;
            *column = Some(values);
        }
        None => {}
    }
}

/// `GTSHelper.copyGeo`: copy the locations and elevations of `from` onto the datapoints of
/// `to` at the same ticks (both sorted in place). The first datapoint of `to` at a tick gets
/// the location of the last datapoint of `from` at that tick.
fn copy_geo(from: &mut GeoTimeSerie, to: &mut GeoTimeSerie) {
    from.sort(false);
    to.sort(false);
    let (from_n, to_n) = (from.size(), to.size());
    let (mut fromidx, mut toidx) = (0, 0);
    while toidx < to_n && fromidx < from_n {
        let totick = to.tick_at(toidx);
        while fromidx < from_n && from.tick_at(fromidx) < totick {
            fromidx += 1;
        }
        if fromidx >= from_n {
            break;
        }
        let fromtick = from.tick_at(fromidx);
        while toidx < to_n && to.tick_at(toidx) < fromtick {
            toidx += 1;
        }
        if toidx >= to_n {
            break;
        }
        if to.tick_at(toidx) == fromtick {
            let location = from.location_at(fromidx);
            let elevation = from.elevation_at(fromidx);
            set_at(&mut to.locations, to_n, toidx, location, NO_LOCATION);
            set_at(&mut to.elevations, to_n, toidx, elevation, NO_ELEVATION);
            fromidx += 1;
        }
    }
}

/// `COPYGEO`: copy the locations and elevations of a series onto a series or each of a list.
fn copygeo(stack: &mut Stack) -> Result<()> {
    let Value::Gts(from) = stack.pop()? else {
        return Err(fail(
            "COPYGEO",
            "expects a Geo Time Series on top of the stack.",
        ));
    };
    gts_or_list_try(stack, "COPYGEO", |to| {
        if Rc::ptr_eq(&from, to) {
            // Warp 10 reads datapoints it has not overwritten yet: a sorted copy reads the same.
            let mut to = to.borrow_mut();
            to.sort(false);
            let mut from = to.clone();
            copy_geo(&mut from, &mut to);
        } else {
            copy_geo(&mut from.borrow_mut(), &mut to.borrow_mut());
        }
        Ok(Value::Gts(Rc::clone(to)))
    })
}
