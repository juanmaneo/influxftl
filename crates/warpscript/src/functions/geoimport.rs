// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The geometry functions: `GEO.WKT`, `GEO.WKB`, `GEO.JSON` (and their `.UNIFORM` forms),
//! which turn a geometry into a `GEOSHAPE`, and `->WKT`, `->WKB`, `->GEOJSON`, which turn a
//! geometry, or a `GEOSHAPE`, into a text.

use std::rc::Rc;

use super::Entry;
use super::args::{fail, fail_with_cause};
use crate::error::geometry::GeometryError;
use crate::error::{Error, Result};
use crate::geoshape::GeoShape;
use crate::jts::cover::{Precision, cover_cells};
use crate::jts::geometry::Geometry;
use crate::jts::outline::shape_to_geojson;
use crate::jts::{geojson, wkb, wkt};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "GEO.WKT",
        Entry::Stack(|s| import(s, "GEO.WKT", Format::Wkt, false)),
    ),
    (
        "GEO.WKT.UNIFORM",
        Entry::Stack(|s| import(s, "GEO.WKT.UNIFORM", Format::Wkt, true)),
    ),
    (
        "GEO.WKB",
        Entry::Stack(|s| import(s, "GEO.WKB", Format::Wkb, false)),
    ),
    (
        "GEO.WKB.UNIFORM",
        Entry::Stack(|s| import(s, "GEO.WKB.UNIFORM", Format::Wkb, true)),
    ),
    (
        "GEO.JSON",
        Entry::Stack(|s| import(s, "GEO.JSON", Format::GeoJson, false)),
    ),
    (
        "GEO.JSON.UNIFORM",
        Entry::Stack(|s| import(s, "GEO.JSON.UNIFORM", Format::GeoJson, true)),
    ),
    ("->WKT", Entry::Stack(towkt)),
    ("->WKB", Entry::Stack(towkb)),
    ("->GEOJSON", Entry::Stack(togeojson)),
];

/// A format a `GeoImporter` reads.
#[derive(Clone, Copy)]
enum Format {
    Wkt,
    Wkb,
    GeoJson,
}

impl Format {
    /// How the type of the input is worded in the messages (`GeoWKB` has a leading space,
    /// which Warp 10 keeps).
    fn input_type(self) -> &'static str {
        match self {
            Format::Wkt => "a WKT STRING",
            Format::Wkb => " GeoWKB BYTES",
            Format::GeoJson => "a GeoJSON STRING",
        }
    }

    /// The geometry the input reads as (`None` when the reader returns `null`), `None` when the
    /// input is not of the type the format reads.
    fn read(self, input: &Value) -> Option<Result<Option<Geometry>>> {
        match (self, input) {
            (Format::Wkt, Value::String(text)) => Some(wkt::read(text)),
            (Format::Wkb, Value::Bytes(bytes)) => Some(wkb::read(bytes).map(Some)),
            (Format::GeoJson, Value::String(text)) => Some(geojson::read(text).map(Some)),
            _ => None,
        }
    }
}

/// `GeoImporter`: the shape that covers the geometry in the `STRING` or `BYTES` of the third
/// level of the stack, to the precision (a `DOUBLE` error percentage or an even `LONG`
/// resolution) of the second one, inside it only when the top is `true`.
fn import(stack: &mut Stack, name: &str, format: Format, uniform: bool) -> Result<()> {
    let inside = stack.pop()?;
    let precision = stack.pop()?;
    let input = stack.pop()?;
    let type_error = || {
        fail(
            name,
            &format!(
                "expects {}, a DOUBLE error percentage or a LONG resolution (even number between 2 and 30) and a BOOLEAN.",
                format.input_type()
            ),
        )
    };
    let (Some(read), Value::Boolean(inside)) = (format.read(&input), inside) else {
        return Err(type_error());
    };
    let precision = match precision {
        Value::Double(fraction) => Precision::ErrorFraction(fraction),
        Value::Long(resolution) if (2..=30).contains(&resolution) && resolution % 2 == 0 => {
            Precision::Resolution(resolution as u32)
        }
        Value::Long(_) => {
            return Err(fail(
                name,
                "expects the resolution to be an even number between 2 and 30",
            ));
        }
        _ => return Err(type_error()),
    };
    let geometry = read.map_err(|err| {
        fail_with_cause(
            name,
            &format!("expects valid {}.", format.input_type()),
            err,
        )
    })?;
    // `intValue()` of the limit: it wraps, and a negative limit allows no cell.
    let max_cells = (stack.limits.max_geocells as i32).max(0);
    // A reader that returns `null` gives the `NullPointerException` of the cover.
    let geometry = geometry.ok_or(Error::NullPointer)?;
    let Some(cells) = cover_cells(&geometry, precision, uniform, inside, max_cells as usize)?
    else {
        return Err(fail(
            name,
            &format!(
                "reached the maximum number of cells in a geographic shape (warpscript.maxgeocells={max_cells})."
            ),
        ));
    };
    stack.push(Value::GeoShape(Rc::new(GeoShape::from_cells(cells))));
    Ok(())
}

/// `TOGEOJSON.toGeometry`: the geometry in a `BYTES` (WKB) or a `STRING` (GeoJSON, GML, KML or
/// WKT, told by the first characters), `None` when the reader returns `null`.
///
/// # Errors
///
/// [`GeometryError::UnknownFormat`] for any other value.
fn to_geometry(object: &Value) -> Result<Option<Geometry>> {
    match object {
        Value::Bytes(bytes) => wkb::read(bytes).map(Some),
        Value::String(text) => {
            // `String.trim`: every character up to the space.
            let text = text.trim_matches(|c| c <= ' ');
            if text.starts_with('{') {
                geojson::read(text).map(Some)
            } else if text.starts_with('<') {
                // GML (`<gml`) and KML: not read yet.
                Err(GeometryError::Unsupported.into())
            } else {
                wkt::read(text)
            }
        }
        _ => Err(GeometryError::UnknownFormat.into()),
    }
}

/// Read an object as a geometry and write it, the failures worded as Warp 10 does: `expected`
/// for a value that is not a geometry text, `was given invalid input.` for the others.
fn write_object(
    stack: &mut Stack,
    name: &str,
    expected: &str,
    object: &Value,
    write: fn(Option<Geometry>) -> Result<Value>,
) -> Result<()> {
    let written = to_geometry(object).and_then(write);
    match written {
        Ok(value) => {
            stack.push(value);
            Ok(())
        }
        Err(err @ Error::Geometry(GeometryError::UnknownFormat)) => {
            Err(fail_with_cause(name, expected, err))
        }
        Err(err) => Err(fail_with_cause(name, "was given invalid input.", err)),
    }
}

/// What `->WKT` and `->WKB` do: a `GEOSHAPE`, or a `BOOLEAN` and a `GEOSHAPE`, is made a
/// GeoJSON first (as `->GEOJSON` does), then the object on top is read and written.
fn export(
    stack: &mut Stack,
    name: &str,
    expected: &str,
    write: fn(Option<Geometry>) -> Result<Value>,
) -> Result<()> {
    let mut object = stack.pop()?;
    if matches!(object, Value::GeoShape(_) | Value::Boolean(_)) {
        object = Value::from(shape_to_text(stack, name, object)?);
    }
    write_object(stack, name, expected, &object, write)
}

/// `TOGEOJSON` on a `GEOSHAPE`, or on a `BOOLEAN` (every cell) and the `GEOSHAPE` under it:
/// `top` is the value popped, the shape is under it when it is a `BOOLEAN`.
fn shape_to_text(stack: &mut Stack, name: &str, top: Value) -> Result<String> {
    let (all_cells, shape) = match top {
        Value::Boolean(flag) => (flag, stack.pop()?),
        shape => (false, shape),
    };
    let Value::GeoShape(shape) = shape else {
        return Err(fail(
            name,
            "operates on a GEOSHAPE when first given a BOOLEAN.",
        ));
    };
    shape_to_geojson(shape.cells(), all_cells)
}

/// `->WKT`: the WKT of a `GEOSHAPE` or of a geometry.
fn towkt(stack: &mut Stack) -> Result<()> {
    export(
        stack,
        "->WKT",
        "expects a GEOSHAPE, a GeoJSON STRING, a GML STRING, a KML STRING or WKB BYTES.",
        |geometry| {
            Ok(Value::from(wkt::write(
                &geometry.ok_or(Error::NullPointer)?,
            )?))
        },
    )
}

/// `->WKB`: the WKB of a `GEOSHAPE` or of a geometry.
fn towkb(stack: &mut Stack) -> Result<()> {
    export(
        stack,
        "->WKB",
        "expects a GEOSHAPE, a WKT STRING, a GML STRING, a KML STRING or a GeoJSON STRING.",
        |geometry| {
            let geometry = geometry.ok_or(GeometryError::UnknownGeometryType)?;
            Ok(Value::Bytes(Rc::from(wkb::write(&geometry)?)))
        },
    )
}

/// `->GEOJSON`: the GeoJSON of a `GEOSHAPE` or of a geometry.
fn togeojson(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "->GEOJSON";
    let object = stack.pop()?;
    if matches!(object, Value::GeoShape(_) | Value::Boolean(_)) {
        let json = shape_to_text(stack, NAME, object)?;
        stack.push(Value::from(json));
        return Ok(());
    }
    write_object(
        stack,
        NAME,
        "expects a GEOSHAPE, a WKT STRING, a GML STRING, a KML STRING or WKB BYTES.",
        &object,
        |geometry| {
            Ok(Value::from(geojson::write(
                &geometry.ok_or(Error::NullPointer)?,
            )?))
        },
    )
}
