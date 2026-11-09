// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Operations on `GEOSHAPE`s: `GEO.UNION`, `GEO.INTERSECTION`, `GEO.DIFFERENCE`,
//! `GEO.INTERSECTS`, `GEO.WITHIN`, `GEO.OPTIMIZE`, `GEO.NORMALIZE`, `GEOSHIFT`,
//! `GEO.REGEXP`, `GEOSPLIT`, `BBOX`, `GEO.COVER` and `GEO.COVER.RL`.

use std::collections::BTreeSet;
use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::gts::{decoded, element_or_list, gts_or_list};
use crate::coverage::Coverage;
use crate::error::{Error, Result};
use crate::geoshape::GeoShape;
use crate::geoxp::from_geoxp_point;
use crate::gts::{GeoTimeSerie, NO_LOCATION};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "GEO.UNION",
        Entry::Stack(|s| fold(s, "GEO.UNION", GeoShape::union)),
    ),
    (
        "GEO.INTERSECTION",
        Entry::Stack(|s| fold(s, "GEO.INTERSECTION", |a, b| Ok(a.intersection(b)))),
    ),
    ("GEO.DIFFERENCE", Entry::Stack(difference)),
    ("GEO.INTERSECTS", Entry::Stack(intersects)),
    ("GEO.WITHIN", Entry::Stack(within)),
    ("GEO.OPTIMIZE", Entry::Stack(optimize)),
    ("GEO.NORMALIZE", Entry::Stack(normalize)),
    ("GEOSHIFT", Entry::Stack(geoshift)),
    ("GEO.REGEXP", Entry::Stack(georegexp)),
    ("GEOSPLIT", Entry::Stack(geosplit)),
    ("BBOX", Entry::Stack(bbox)),
    ("GEO.COVER", Entry::Stack(|s| cover(s, "GEO.COVER", false))),
    (
        "GEO.COVER.RL",
        Entry::Stack(|s| cover(s, "GEO.COVER.RL", true)),
    ),
];

/// A `GEOSHAPE` stack object.
fn shape_value(shape: GeoShape) -> Value {
    Value::GeoShape(Rc::new(shape))
}

/// `GeoUnion` and `GeoIntersection`: combine the 2 shapes on top of the stack or the shapes
/// of the list on top of the stack (no shape for an empty list).
fn fold(
    stack: &mut Stack,
    name: &str,
    combine: impl Fn(&GeoShape, &GeoShape) -> Result<GeoShape>,
) -> Result<()> {
    const MESSAGE: &str = "expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.";
    let top = stack.pop()?;
    let combined = if let Value::List(list) = &top {
        let list = list.borrow();
        let mut shapes = list.iter();
        let Some(first) = shapes.next() else {
            stack.push(shape_value(GeoShape::default()));
            return Ok(());
        };
        let Value::GeoShape(first) = first else {
            return Err(fail(name, MESSAGE));
        };
        let mut combined = Rc::clone(first);
        for shape in shapes {
            let Value::GeoShape(shape) = shape else {
                return Err(fail(name, MESSAGE));
            };
            combined = Rc::new(combine(&combined, shape)?);
        }
        combined
    } else {
        let (Value::GeoShape(top), Value::GeoShape(second)) = (top, stack.pop()?) else {
            return Err(fail(name, MESSAGE));
        };
        Rc::new(combine(&top, &second)?)
    };
    stack.push(Value::GeoShape(combined));
    Ok(())
}

/// `GEO.DIFFERENCE`: the shape under the top one, without the top one.
fn difference(stack: &mut Stack) -> Result<()> {
    let (Value::GeoShape(subtracted), Value::GeoShape(shape)) = (stack.pop()?, stack.pop()?) else {
        return Err(fail(
            "GEO.DIFFERENCE",
            "expects two GeoShape instances as the top 2 elements of the stack.",
        ));
    };
    stack.push(shape_value(shape.subtraction(&subtracted)?));
    Ok(())
}

/// The locations of a Geo Time Series.
fn locations(gts: &GeoTimeSerie) -> impl Iterator<Item = i64> {
    (0..gts.size())
        .map(|idx| gts.location_at(idx))
        .filter(|location| *location != NO_LOCATION)
}

/// `GTSStackFunction` of a shape on top of the stack: `op` of each Geo Time Series.
fn on_series(
    stack: &mut Stack,
    name: &str,
    op: impl Fn(&GeoShape, &GeoTimeSerie) -> bool,
) -> Result<()> {
    let Value::GeoShape(shape) = stack.pop()? else {
        return Err(fail(name, "expects a geo shape on top of the stack."));
    };
    gts_or_list(stack, name, |gts| Value::Boolean(op(&shape, &gts.borrow())))
}

/// `GEO.INTERSECTS`: whether 2 shapes intersect, or whether each Geo Time Series has a
/// location in the shape.
fn intersects(stack: &mut Stack) -> Result<()> {
    if let [.., Value::GeoShape(second), Value::GeoShape(first)] = stack.elements() {
        let intersects = first.intersects(second);
        stack.discard_top(2);
        stack.push(Value::Boolean(intersects));
        return Ok(());
    }
    on_series(stack, "GEO.INTERSECTS", |shape, gts| {
        locations(gts).any(|location| shape.contains_point(location))
    })
}

/// `GEO.WITHIN`: whether all the locations of each Geo Time Series are in the shape (and
/// there is one).
fn within(stack: &mut Stack) -> Result<()> {
    on_series(stack, "GEO.WITHIN", |shape, gts| {
        let mut locations = locations(gts).peekable();
        locations.peek().is_some() && locations.all(|location| shape.contains_point(location))
    })
}

/// `GEO.OPTIMIZE`: the cells of a shape merged, or finer than a resolution (even, 2 to 30)
/// merged into that resolution.
fn optimize(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "GEO.OPTIMIZE";
    let Value::Long(resolution) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a maximum resolution on top of the stack.",
        ));
    };
    // `intValue()`: the resolution is the low 32 bits.
    let resolution = resolution as i32;
    if resolution != 0 && (!(2..=30).contains(&resolution) || resolution & 1 != 0) {
        return Err(fail(
            NAME,
            "expects a maximum resolution which is an even number between 2 and 30 or 0.",
        ));
    }
    let Value::GeoShape(shape) = stack.pop()? else {
        return Err(fail(NAME, "operates on a shape."));
    };
    let optimized = if resolution == 0 {
        shape.optimized()
    } else {
        // 2 to 30.
        shape.limit_resolution(resolution as u32)
    };
    stack.push(shape_value(optimized));
    Ok(())
}

/// `GEO.NORMALIZE`: split the cells of the 2 shapes on top of the stack to a common
/// resolution.
fn normalize(stack: &mut Stack) -> Result<()> {
    const MESSAGE: &str = "operates on two GEOSHAPE instances.";
    let Value::GeoShape(b) = stack.pop()? else {
        return Err(fail("GEO.NORMALIZE", MESSAGE));
    };
    let Value::GeoShape(a) = stack.pop()? else {
        return Err(fail("GEO.NORMALIZE", MESSAGE));
    };
    let (a, b) = a.normalize(&b)?;
    stack.push(shape_value(a));
    stack.push(shape_value(b));
    Ok(())
}

/// `GEOSHIFT`: move a shape by a latitude and longitude delta, rounded to a resolution.
fn geoshift(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "GEOSHIFT";
    let Value::Long(resolution) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a resolution, even number between 0 and 30.",
        ));
    };
    // `intValue()`: the resolution is the low 32 bits.
    let resolution = resolution as i32;
    if !(0..=30).contains(&resolution) || resolution % 2 != 0 {
        return Err(fail(
            NAME,
            "expects a resolution, even number between 0 and 30.",
        ));
    }
    let resolution = (resolution >> 1) as usize;
    let delta_lon = stack
        .pop()?
        .as_f64()
        .filter(|d| !(*d < -360.0 || *d > 360.0));
    let Some(delta_lon) = delta_lon else {
        return Err(fail(
            NAME,
            "expects a longitude delta between -360.0 and 360.0.",
        ));
    };
    let delta_lat = stack
        .pop()?
        .as_f64()
        .filter(|d| !(*d < -180.0 || *d > 180.0));
    let Some(delta_lat) = delta_lat else {
        return Err(fail(
            NAME,
            "expects a latitude delta between -180.0 and 180.0.",
        ));
    };
    let mut top = stack.pop()?;
    // A meridian that is not a number (`NaN` or an infinity once wrapped) is no meridian.
    let mut meridian = None;
    if let Some(meridian_degrees) = top.as_f64() {
        let wrapped = (((meridian_degrees + 180.0) % 360.0) + 360.0) % 360.0;
        meridian = Some(wrapped).filter(|wrapped| !wrapped.is_nan());
        top = stack.pop()?;
    }
    let Value::GeoShape(shape) = top else {
        return Err(fail(NAME, "operates on a GEOSHAPE."));
    };
    let finest = shape.finest_resolution();
    let resolution = match resolution {
        0 => finest,
        resolution if resolution > finest => {
            return Err(Error::usage(
                NAME,
                format!("cannot use resolution finer than {}", finest << 1),
            ));
        }
        resolution => resolution,
    };
    stack.push(shape_value(
        shape.shift(resolution, meridian, delta_lat, delta_lon),
    ));
    Ok(())
}

/// `GEO.REGEXP`: the regular expression matching the HHCodes of a shape.
fn georegexp(stack: &mut Stack) -> Result<()> {
    let Value::GeoShape(shape) = stack.pop()? else {
        return Err(fail(
            "GEO.REGEXP",
            "expects a GeoShape instance on top of the stack.",
        ));
    };
    stack.push(Value::from(shape.regexp()?));
    Ok(())
}

/// `GEOSPLIT`: the clusters of touching cells of a shape, as a list of shapes.
fn geosplit(stack: &mut Stack) -> Result<()> {
    let Value::GeoShape(shape) = stack.pop()? else {
        return Err(fail("GEOSPLIT", "operates on a GEOSHAPE."));
    };
    let shapes = shape.split()?.into_iter().map(shape_value).collect();
    stack.push(Value::list(shapes));
    Ok(())
}

/// `GTSHelper.bbox`: `[ minlat minlon maxlat maxlon ]` of the locations of a series, the
/// whole world for a series without locations array.
fn bounding_box(gts: &GeoTimeSerie) -> Value {
    let Some(locations) = &gts.locations else {
        return Value::list(vec![
            Value::Double(-90.0),
            Value::Double(-180.0),
            Value::Double(90.0),
            Value::Double(180.0),
        ]);
    };
    let (mut min_lat, mut min_lon, mut max_lat, mut max_lon) =
        (90.0f64, 180.0f64, -90.0f64, -180.0f64);
    for location in locations
        .iter()
        .take(gts.size())
        .filter(|l| **l != NO_LOCATION)
    {
        let (lat, lon) = from_geoxp_point(*location);
        min_lat = min_lat.min(lat);
        max_lat = max_lat.max(lat);
        min_lon = min_lon.min(lon);
        max_lon = max_lon.max(lon);
    }
    Value::list(vec![
        Value::Double(min_lat),
        Value::Double(min_lon),
        Value::Double(max_lat),
        Value::Double(max_lon),
    ])
}

/// `BBOX`: the bounding box of the locations of a series or of each series of a list.
fn bbox(stack: &mut Stack) -> Result<()> {
    gts_or_list(stack, "BBOX", |gts| bounding_box(&gts.borrow()))
}

/// The locations of a series or an encoder, in storage order or, for a polyline, in tick
/// order (a series is then sorted in place, an encoder decoded to a sorted series).
fn cover_locations(name: &str, element: &Value, polyline: bool) -> Result<Vec<i64>> {
    match element {
        Value::Gts(gts) if polyline => {
            let mut gts = gts.borrow_mut();
            gts.sort(false);
            Ok(locations(&gts).collect())
        }
        Value::Gts(gts) => Ok(locations(&gts.borrow()).collect()),
        Value::Encoder(encoder) if polyline => {
            let mut gts = encoder.borrow().decoder().decode()?;
            gts.sort(false);
            Ok(locations(&gts).collect())
        }
        Value::Encoder(encoder) => Ok(decoded(&encoder.borrow())?
            .into_iter()
            .map(|(_, location, ..)| location)
            .filter(|location| *location != NO_LOCATION)
            .collect()),
        _ => Err(fail(
            name,
            // The Java message has a no-break space between "Series" and "or".
            "can only operate on Geo Time Series\u{a0}or GTS Encoders.",
        )),
    }
}

/// `GEO.COVER` and `GEO.COVER.RL`: the shape of the cells of a resolution (even, 2 to 30)
/// holding the locations of a series or an encoder, or of each of a list; with `polyline`
/// (rhumb lines), the cells the line through the locations, in tick order, goes through.
fn cover(stack: &mut Stack, name: &str, polyline: bool) -> Result<()> {
    let resolution = match stack.pop()? {
        Value::Long(resolution) if (2..=30).contains(&resolution) && resolution % 2 == 0 => {
            resolution as u32
        }
        _ => {
            return Err(fail(
                name,
                "expects a resolution (even number from 2 to 30).",
            ));
        }
    };
    // The leading bits of an HHCode a cell of this resolution keeps, and its resolution.
    let mask = (u64::MAX >> (64 - 2 * resolution)) << (64 - 2 * resolution);
    let prefix = u64::from(resolution >> 1) << 60;
    element_or_list(stack, |element| {
        let locations = cover_locations(name, element, polyline)?;
        let cells: BTreeSet<i64> = if polyline {
            Coverage::cover_polyline(&locations, resolution)?
                .to_geocells(resolution as i32)
                .into_iter()
                .collect()
        } else {
            locations
                .into_iter()
                .map(|location| (((location as u64 & mask) >> 4) | prefix) as i64)
                .collect()
        };
        Ok(shape_value(GeoShape::from_cells(
            cells.into_iter().collect(),
        )))
    })
}
