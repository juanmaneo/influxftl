// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from GeoXP Lib (Copyright 1999-2019 Mathias Herberts, 2019-2020 iroise.net S.A.S.,
// 2020- SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The cover of a geometry by geo cells: the `GEOSHAPE` of `GEO.WKT`, `GEO.WKB` and `GEO.JSON`
//! (`JTSHelper.coverGeometry`, `GeoXPLib.toGeoXPShape`).
//!
//! The cover splits the 16 cells of the resolution 2 breadth first. A cell that does not meet
//! the geometry is dropped, one that is inside it (or, at the finest resolution, that meets it)
//! is kept, any other is split in 16. The predicates are those of JTS, `RectangleIntersects`,
//! `Geometry.covers` and `Geometry.intersects` included, with their limits: JTS refuses a
//! `GeometryCollection` where it needs the topology of the geometry.

// The limits of the planisphere are compared exactly, as Java does.
#![allow(clippy::float_cmp)]

use std::collections::VecDeque;
use std::f64::consts::PI;

use geo::{Intersects, Relate};

use super::geometry::{Coord, Envelope, Geometry, Polygon};
use crate::coverage::{Coverage, MIN_RESOLUTION};
use crate::error::fault::ResourceError;
use crate::error::geometry::GeometryError;
use crate::error::{Error, Result};
use crate::finite::ensure_finite;
use crate::geoxp::{
    DEGREES_PER_LAT_UNIT, DEGREES_PER_LON_UNIT, orthodromic_distance_units, stable_split_hhcode,
};

/// What fixes the finest resolution of a cover.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Precision {
    /// The resolution: even, `2` to `32` (`30` at most from a script).
    Resolution(u32),
    /// The tolerated error, as a fraction of the diagonal of the bounding box of the geometry.
    ErrorFraction(f64),
}

/// `GeoXPLib.toGeoXPShape` and `toUniformGeoXPShape`: the geocells that cover `geometry`, the
/// cells of a uniform cover all having the same resolution, `None` when the cover needs more
/// than `max_cells` cells.
///
/// # Errors
///
/// [`GeometryError::CollectionArgument`] where JTS refuses a `GeometryCollection`,
/// [`NotFinite`](crate::finite::NotFinite) for an error that is not finite or a distance that
/// is not.
pub fn cover_cells(
    geometry: &Geometry,
    precision: Precision,
    uniform: bool,
    inside: bool,
    max_cells: usize,
) -> Result<Option<Vec<i64>>> {
    let resolution = match precision {
        Precision::Resolution(resolution) => resolution,
        Precision::ErrorFraction(fraction) => {
            optimal_resolution(&bounding_box(geometry), ensure_finite(fraction)?)?
        }
    };
    let min_resolution = if uniform { resolution } else { MIN_RESOLUTION };
    let Some(mut coverage) =
        cover_geometry(
            geometry,
            min_resolution,
            resolution,
            inside,
            Budget {
                cells: max_cells,
                pending: MAX_PENDING_CELLS,
            },
        )?
    else {
        return Ok(None);
    };
    if !uniform {
        coverage.optimize(0, MIN_RESOLUTION);
    }
    Ok(Some(coverage.to_geocells(resolution as i32)))
}

/// The envelope of a geometry without a point (`Envelope.setToNull`).
const NULL_ENVELOPE: Envelope = Envelope {
    min_x: 0.0,
    max_x: -1.0,
    min_y: 0.0,
    max_y: -1.0,
};

/// `HHCodeHelper.getBoundingBox`: the `(latitude, longitude)` units of the south west and north
/// east corners of the envelope.
fn bounding_box(geometry: &Geometry) -> [i64; 4] {
    let envelope = geometry.envelope().unwrap_or(NULL_ENVELOPE);
    [
        to_units(envelope.min_y, 90.0, DEGREES_PER_LAT_UNIT),
        to_units(envelope.min_x, 180.0, DEGREES_PER_LON_UNIT),
        to_units(envelope.max_y, 90.0, DEGREES_PER_LAT_UNIT),
        to_units(envelope.max_x, 180.0, DEGREES_PER_LON_UNIT),
    ]
}

/// `HHCodeHelper.toLongLat` and `toLongLon`: the units of a latitude (`limit` 90) or a
/// longitude (`limit` 180), the last one for the limit itself and the one before the first for
/// the degrees that are below the planisphere.
fn to_units(degrees: f64, limit: f64, degrees_per_unit: f64) -> i64 {
    let shifted = degrees + limit;
    let units = if degrees == limit {
        0xFFFF_FFFF
    } else {
        (shifted / degrees_per_unit) as i64
    };
    if shifted < 0.0 && units == 0 {
        units - 1
    } else {
        units
    }
}

/// `HHCodeHelper.getOptimalResolution`: the finest resolution (even, `2` to `32`) whose cells
/// have a diagonal that is not above `error_fraction` times the diagonal of the box (the
/// `(latitude, longitude)` units of its south west and north east corners).
///
/// # Errors
///
/// [`NotFinite`](crate::finite::NotFinite) for a distance, or a tolerated error, that is not
/// finite (Java compares with `NaN` and goes on to the resolution 2).
fn optimal_resolution(bounds: &[i64; 4], error_fraction: f64) -> Result<u32> {
    let [south, west, north, east] = *bounds;
    let diagonal = ensure_finite(orthodromic_distance_units(south, west, north, east))?;
    let tolerated = ensure_finite(diagonal * error_fraction)?;
    let mut resolution = 32;
    while resolution > 2 {
        let lon_span = PI * 2.0 / (1u64 << resolution) as f64;
        let lat_span = PI / (1u64 << resolution) as f64;
        let (from_lat, to_lat) = (-lat_span / 2.0, lat_span / 2.0);
        let haversine = ((from_lat - to_lat) / 2.0).sin().powi(2)
            + from_lat.cos() * to_lat.cos() * (-lon_span / 2.0).sin().powi(2);
        let cell_diagonal = 2.0 * haversine.sqrt().asin() * 57.295_779_513_082_32 * 111_120.0;
        if cell_diagonal > tolerated {
            break;
        }
        resolution -= 2;
    }
    Ok(resolution)
}

/// The cells a cover may leave to split at once. The cover splits breadth first, so a fine
/// resolution over a large area queues an ever growing number of cells: Warp 10 dies of
/// `Java heap space` once its heap is full (somewhere between the 43 million cells of a uniform
/// cover of a 0.1 degree square at the resolution 24, which it answers after 90 seconds, and
/// the 690 million of the resolution 26, which it does not).
pub const MAX_PENDING_CELLS: usize = 1 << 26;

/// What a cover may use.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    /// The cells it may return (`warpscript.maxgeocells`).
    pub cells: usize,
    /// The cells it may queue to split, [`MAX_PENDING_CELLS`] for a script.
    pub pending: usize,
}

/// `JTSHelper.coverGeometry`: the cells of the resolutions `min_resolution` to `max_resolution`
/// that cover `geometry` (those inside it only when `inside`), `None` when more than
/// `budget.cells` would be needed.
///
/// # Errors
///
/// [`GeometryError::CollectionArgument`] where JTS refuses a `GeometryCollection`,
/// [`ResourceError::OutOfMemory`] when more than `budget.pending` cells wait to be split.
pub fn cover_geometry(
    geometry: &Geometry,
    min_resolution: u32,
    max_resolution: u32,
    inside: bool,
    budget: Budget,
) -> Result<Option<Coverage>> {
    let max_cells = budget.cells;
    let Some(target) = Target::new(geometry) else {
        // No cell meets a geometry without a point, but a limit of no cell is reached at once.
        return Ok((max_cells > 0).then(Coverage::new));
    };
    let mut coverage = Coverage::new();
    let mut queue: VecDeque<i64> = (0..16).map(|top| (1 << 60) | (top << 56)).collect();
    let mut count = 0;
    while count < max_cells
        && let Some(geocell) = queue.pop_front()
    {
        let resolution = (((geocell as u64) >> 60) as u32) << 1;
        let hhcode = geocell << 4;
        let cell = cell_bounds(hhcode, resolution);
        if !target.meets_rectangle(&cell) {
            continue;
        }
        if max_resolution == resolution && !inside {
            coverage.add_cell(resolution as i32, hhcode);
            count += 1;
            continue;
        }
        if target.covers(&cell, &cell_polygon(&cell))? && resolution >= min_resolution {
            coverage.add_cell(resolution as i32, hhcode);
            count += 1;
            continue;
        }
        if max_resolution == resolution {
            continue;
        }
        if resolution == 30 {
            for sub in sub_geo_cells(geocell) {
                let ring = cell_bounds(sub, 32);
                let shape = cell_ring(&ring);
                if (target.intersects(&ring, &shape)? && !inside) || target.covers(&ring, &shape)? {
                    coverage.add_cell(32, sub);
                    count += 1;
                }
            }
            continue;
        }
        if queue.len() + 16 > budget.pending {
            return Err(ResourceError::OutOfMemory.into());
        }
        queue.extend(sub_geo_cells(geocell));
    }
    Ok(queue.is_empty().then_some(coverage))
}

/// `HHCodeHelper.getSubGeoCells`: the 16 cells of the next resolution inside a geocell (the
/// HHCodes of the cells of the resolution 32 for a geocell of the resolution 30).
fn sub_geo_cells(geocell: i64) -> [i64; 16] {
    let level = (geocell as u64) >> 60;
    if level == 15 {
        let hhcode = (geocell << 4) & !0xF;
        std::array::from_fn(|i| hhcode | i as i64)
    } else {
        let next = level + 1;
        let kept = (geocell as u64 & !((1u64 << (60 - 4 * level)) - 1)) & 0x0FFF_FFFF_FFFF_FFFF;
        let prefix = (kept | (next << 60)) as i64;
        std::array::from_fn(|i| prefix | ((i as i64) << (60 - 4 * next)))
    }
}

/// `JTSHelper.hhcodeToLinearRing`: the bounds of the cell of `resolution` that holds `hhcode`.
fn cell_bounds(hhcode: i64, resolution: u32) -> Envelope {
    let (lat, lon) = stable_split_hhcode(hhcode, resolution);
    let south = lat as f64 * DEGREES_PER_LAT_UNIT - 90.0;
    let west = lon as f64 * DEGREES_PER_LON_UNIT - 180.0;
    // `1 << (32 - resolution)` is an `int` in Java; the resolutions here are even and 2 to 32.
    let units = f64::from(1u32 << (32 - resolution));
    Envelope {
        min_x: west,
        max_x: west + DEGREES_PER_LON_UNIT * units,
        min_y: south,
        max_y: south + DEGREES_PER_LAT_UNIT * units,
    }
}

/// The cell as the polygon JTS builds from it.
fn cell_polygon(cell: &Envelope) -> geo::Geometry<f64> {
    geo::Rect::new(
        geo::coord! { x: cell.min_x, y: cell.min_y },
        geo::coord! { x: cell.max_x, y: cell.max_y },
    )
    .to_polygon()
    .into()
}

/// The cell as the linear ring JTS builds from it.
fn cell_ring(cell: &Envelope) -> geo::Geometry<f64> {
    geo::LineString::from(vec![
        (cell.min_x, cell.min_y),
        (cell.min_x, cell.max_y),
        (cell.max_x, cell.max_y),
        (cell.max_x, cell.min_y),
        (cell.min_x, cell.min_y),
    ])
    .into()
}

/// The geometry a cover is of, in the forms its predicates need.
struct Target {
    envelope: Envelope,
    /// `Polygon.isRectangle`.
    rectangle: bool,
    /// Whether it is a `GeometryCollection`, that JTS refuses where it needs the topology.
    collection: bool,
    /// The points, lines and polygons it is made of, collections taken apart.
    parts: geo::GeometryCollection<f64>,
    /// The points of a multi-point that has an empty point, whose topology JTS cannot compute.
    with_empty_point: Option<Vec<Coord>>,
}

impl Target {
    /// The target of a geometry, `None` when it has no point.
    fn new(geometry: &Geometry) -> Option<Target> {
        let envelope = geometry.envelope()?;
        let mut parts = Vec::new();
        take_apart(geometry, &mut parts);
        Some(Target {
            envelope,
            rectangle: matches!(geometry, Geometry::Polygon(polygon) if polygon.is_rectangle()),
            collection: matches!(geometry, Geometry::Collection(_)),
            parts: geo::GeometryCollection::new_from(parts),
            with_empty_point: match geometry {
                Geometry::MultiPoint(points) if points.iter().any(Option::is_none) => {
                    Some(points.iter().flatten().copied().collect())
                }
                _ => None,
            },
        })
    }

    /// `RectangleIntersects.intersects(cell, geometry)`: the cell and the geometry share a
    /// point.
    fn meets_rectangle(&self, cell: &Envelope) -> bool {
        self.envelope.intersects(cell) && self.any_part_meets(&cell_polygon(cell))
    }

    fn any_part_meets(&self, shape: &geo::Geometry<f64>) -> bool {
        self.parts.iter().any(|part| shape.intersects(part))
    }

    /// `Geometry.covers(shape)` for a `shape` of the bounds `cell`.
    fn covers(&self, cell: &Envelope, shape: &geo::Geometry<f64>) -> Result<bool> {
        if !self.envelope.covers(cell) {
            return Ok(false);
        }
        if self.rectangle {
            return Ok(true);
        }
        self.refuse_collection()?;
        self.locate_corner(cell)?;
        Ok(self.parts.relate(shape).is_covers())
    }

    /// `Geometry.intersects(shape)` for a `shape` of the bounds `cell`.
    fn intersects(&self, cell: &Envelope, shape: &geo::Geometry<f64>) -> Result<bool> {
        if !self.envelope.intersects(cell) {
            return Ok(false);
        }
        if self.rectangle {
            return Ok(self.any_part_meets(shape));
        }
        self.refuse_collection()?;
        self.locate_corner(cell)?;
        Ok(self.parts.relate(shape).is_intersects())
    }

    /// The location of the first corner of a cell in the geometry, that `RelateOp` needs unless
    /// a point sits on that corner: JTS dereferences the missing coordinate of an empty point
    /// of a multi-point there.
    ///
    /// # Errors
    ///
    /// [`Error::NullPointer`] for a multi-point that has an empty point.
    fn locate_corner(&self, cell: &Envelope) -> Result<()> {
        let Some(points) = &self.with_empty_point else {
            return Ok(());
        };
        if points
            .iter()
            .any(|point| point.x == cell.min_x && point.y == cell.min_y)
        {
            return Ok(());
        }
        Err(Error::NullPointer)
    }

    /// `Geometry.checkNotGeometryCollection`.
    fn refuse_collection(&self) -> Result<()> {
        if self.collection {
            return Err(GeometryError::CollectionArgument.into());
        }
        Ok(())
    }
}

fn point(coord: &Coord) -> geo::Point<f64> {
    geo::Point::new(coord.x, coord.y)
}

fn line_string(coords: &[Coord]) -> geo::LineString<f64> {
    coords
        .iter()
        .map(|coord| geo::coord! { x: coord.x, y: coord.y })
        .collect()
}

/// The polygon, `None` when it is empty (its empty holes are dropped).
fn polygon(polygon: &Polygon) -> Option<geo::Polygon<f64>> {
    let holes = polygon
        .holes()
        .iter()
        .filter(|hole| !hole.coords().is_empty())
        .map(|hole| line_string(hole.coords()))
        .collect();
    (!polygon.shell().coords().is_empty())
        .then(|| geo::Polygon::new(line_string(polygon.shell().coords()), holes))
}

/// The non-empty points, lines and polygons of a geometry, appended to `parts`.
fn take_apart(geometry: &Geometry, parts: &mut Vec<geo::Geometry<f64>>) {
    match geometry {
        Geometry::Point(coord) => parts.extend(coord.iter().map(|c| point(c).into())),
        Geometry::LineString(line) => parts.extend(non_empty_line(line.coords())),
        Geometry::LinearRing(ring) => parts.extend(non_empty_line(ring.coords())),
        Geometry::Polygon(shape) => parts.extend(polygon(shape).map(Into::into)),
        Geometry::MultiPoint(points) => {
            parts.extend(points.iter().flatten().map(|c| point(c).into()));
        }
        Geometry::MultiLineString(lines) => {
            parts.extend(
                lines
                    .iter()
                    .filter_map(|line| non_empty_line(line.coords())),
            );
        }
        Geometry::MultiPolygon(polygons) => {
            parts.extend(polygons.iter().filter_map(polygon).map(Into::into));
        }
        Geometry::Collection(elements) => {
            for element in elements {
                take_apart(element, parts);
            }
        }
    }
}

fn non_empty_line(coords: &[Coord]) -> Option<geo::Geometry<f64>> {
    (!coords.is_empty()).then(|| line_string(coords).into())
}
