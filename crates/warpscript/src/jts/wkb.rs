// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Well-Known Binary: the reader behind `GEO.WKB` and the writer behind `->WKB`.
//!
//! Both do what the JTS 1.13 `WKBReader` and `WKBWriter` of Warp 10 do, quirks included: a
//! reader that runs past the end of its input reads zeros, a line string of one point and a
//! ring that is not valid are completed instead of refused, an SRID is read and dropped, the
//! writer is big endian, two dimensional, and does not write an empty point.

use super::geometry::{Coord, Geometry, Line, Polygon, Ring};
use crate::error::Result;
use crate::error::fault::{BoundsError, ResourceError};
use crate::error::geometry::GeometryError;

/// The most elements (coordinates, rings, geometries) a count may announce. A JVM runs out of
/// memory somewhere past that, at a limit that depends on its settings, while the input is
/// read as zeros whatever its length.
const MAX_ELEMENTS: usize = 1 << 24;

/// The deepest nesting of geometry collections the reader follows (a JVM stack overflows
/// somewhere past that, at a depth that depends on its settings).
const MAX_DEPTH: usize = 512;

/// The flag of the geometry type that says that elevations are present (`0x80000000`).
const HAS_Z: u32 = 0x8000_0000;

/// The flag of the geometry type that says that an SRID follows (`0x20000000`).
const HAS_SRID: u32 = 0x2000_0000;

/// The geometry type codes.
mod kind {
    pub const POINT: u8 = 1;
    pub const LINE_STRING: u8 = 2;
    pub const POLYGON: u8 = 3;
    pub const MULTI_POINT: u8 = 4;
    pub const MULTI_LINE_STRING: u8 = 5;
    pub const MULTI_POLYGON: u8 = 6;
    pub const COLLECTION: u8 = 7;
}

/// The reader: the bytes, the position, and the byte order and dimension of the geometry
/// being read (each geometry has its own header).
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    little_endian: bool,
    has_z: bool,
}

impl Reader<'_> {
    /// The next byte; zero past the end (`ByteArrayInStream` fills what it cannot read with
    /// zeros).
    fn byte(&mut self) -> u8 {
        let byte = self.bytes.get(self.position).map_or(0, |byte| *byte);
        self.position = self.position.saturating_add(1);
        byte
    }

    fn array<const N: usize>(&mut self) -> [u8; N] {
        std::array::from_fn(|_| self.byte())
    }

    /// An `int` in the byte order of the geometry.
    fn int(&mut self) -> i32 {
        let bytes = self.array();
        if self.little_endian {
            i32::from_le_bytes(bytes)
        } else {
            i32::from_be_bytes(bytes)
        }
    }

    /// A `double` in the byte order of the geometry.
    fn double(&mut self) -> f64 {
        let bytes = self.array();
        if self.little_endian {
            f64::from_le_bytes(bytes)
        } else {
            f64::from_be_bytes(bytes)
        }
    }

    /// An element count.
    fn count(&mut self) -> Result<usize> {
        let count = self.int();
        size(count)
    }

    fn coordinate(&mut self) -> Result<Coord> {
        let x = self.double();
        let y = self.double();
        let z = self.has_z.then(|| self.double());
        Coord::new(x, y, z)
    }

    fn coordinates(&mut self) -> Result<Vec<Coord>> {
        let count = self.count()?;
        (0..count).map(|_| self.coordinate()).collect()
    }

    fn point(&mut self) -> Result<Coord> {
        self.coordinate()
    }

    fn line(&mut self) -> Result<Line> {
        Ok(Line::extended(self.coordinates()?))
    }

    fn ring(&mut self) -> Result<Ring> {
        Ok(Ring::closed(self.coordinates()?))
    }

    /// The shell is always read, the holes only when the polygon announces more than one ring.
    fn polygon(&mut self) -> Result<Polygon> {
        let rings = self.int();
        let holes = if rings > 1 { size(rings - 1)? } else { 0 };
        let shell = self.ring()?;
        let holes = (0..holes).map(|_| self.ring()).collect::<Result<_>>()?;
        Polygon::new(shell, holes)
    }

    /// The members of a multi-geometry, each with its own header and of the type `pick` takes.
    fn members<T>(
        &mut self,
        name: &'static str,
        depth: usize,
        pick: impl Fn(Geometry) -> Option<T>,
    ) -> Result<Vec<T>> {
        let count = self.count()?;
        (0..count)
            .map(|_| {
                let member = self.geometry(depth + 1)?;
                pick(member).ok_or_else(|| GeometryError::WkbMemberType(name).into())
            })
            .collect()
    }

    /// `readGeometry`.
    fn geometry(&mut self, depth: usize) -> Result<Geometry> {
        if depth > MAX_DEPTH {
            return Err(ResourceError::StackOverflow.into());
        }
        // Only 1 is little endian: any other byte is big endian.
        self.little_endian = self.byte() == 1;
        let header = self.int() as u32;
        self.has_z = header & HAS_Z != 0;
        if header & HAS_SRID != 0 {
            // The SRID is not kept.
            self.int();
        }
        Ok(match (header & 0xff) as u8 {
            kind::POINT => Geometry::Point(Some(self.point()?)),
            kind::LINE_STRING => Geometry::LineString(self.line()?),
            kind::POLYGON => Geometry::Polygon(self.polygon()?),
            kind::MULTI_POINT => {
                let points = self.members("MultiPoint", depth, |member| match member {
                    Geometry::Point(point) => Some(point),
                    _ => None,
                })?;
                Geometry::MultiPoint(points)
            }
            kind::MULTI_LINE_STRING => {
                let lines = self.members("MultiLineString", depth, |member| match member {
                    Geometry::LineString(line) => Some(line),
                    _ => None,
                })?;
                Geometry::MultiLineString(lines)
            }
            kind::MULTI_POLYGON => {
                let polygons = self.members("MultiPolygon", depth, |member| match member {
                    Geometry::Polygon(polygon) => Some(polygon),
                    _ => None,
                })?;
                Geometry::MultiPolygon(polygons)
            }
            kind::COLLECTION => Geometry::Collection(self.members("", depth, Some)?),
            other => return Err(GeometryError::UnknownWkbType(other).into()),
        })
    }
}

/// An array length the input announces.
fn size(count: i32) -> Result<usize> {
    let count = usize::try_from(count).map_err(|_| BoundsError::NegativeSize(i64::from(count)))?;
    if count > MAX_ELEMENTS {
        return Err(ResourceError::OutOfMemory.into());
    }
    Ok(count)
}

/// Read a geometry from WKB.
///
/// # Errors
///
/// [`GeometryError::UnknownWkbType`] for a type that is not one of the 7 of WKB,
/// [`GeometryError::WkbMemberType`] for a member that does not belong in a multi-geometry,
/// [`BoundsError::NegativeSize`] for a negative count, [`ResourceError::OutOfMemory`] for
/// a count past 2^24 and [`ResourceError::StackOverflow`] for collections nested too deep, the
/// `IllegalArgumentException` of a polygon that has holes and no shell, and
/// [`NotFinite`](crate::finite::NotFinite) for an ordinate that is `NaN` or infinite.
pub fn read(bytes: &[u8]) -> Result<Geometry> {
    Reader {
        bytes,
        position: 0,
        little_endian: false,
        has_z: false,
    }
    .geometry(0)
}

/// `WKBWriter.write`: the geometry as big endian, two dimensional WKB.
///
/// # Errors
///
/// [`GeometryError::EmptyPointWkb`] for an empty point, alone or in a multi-geometry.
pub fn write(geometry: &Geometry) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    write_geometry(geometry, &mut out)?;
    Ok(out)
}

/// The byte order (big endian) and type of a geometry.
fn write_header(kind: u8, out: &mut Vec<u8>) {
    out.push(0);
    out.extend_from_slice(&u32::from(kind).to_be_bytes());
}

fn write_count(count: usize, out: &mut Vec<u8>) {
    out.extend_from_slice(&(count as u32).to_be_bytes());
}

fn write_coordinate(coord: &Coord, out: &mut Vec<u8>) {
    out.extend_from_slice(&coord.x.to_be_bytes());
    out.extend_from_slice(&coord.y.to_be_bytes());
}

fn write_coordinates(coords: &[Coord], out: &mut Vec<u8>) {
    write_count(coords.len(), out);
    for coord in coords {
        write_coordinate(coord, out);
    }
}

fn write_point(point: Option<&Coord>, out: &mut Vec<u8>) -> Result<()> {
    let point = point.ok_or(GeometryError::EmptyPointWkb)?;
    write_header(kind::POINT, out);
    write_coordinate(point, out);
    Ok(())
}

fn write_line(line: &Line, out: &mut Vec<u8>) {
    write_header(kind::LINE_STRING, out);
    write_coordinates(line.coords(), out);
}

fn write_polygon(polygon: &Polygon, out: &mut Vec<u8>) {
    write_header(kind::POLYGON, out);
    write_count(polygon.holes().len() + 1, out);
    write_coordinates(polygon.shell().coords(), out);
    for hole in polygon.holes() {
        write_coordinates(hole.coords(), out);
    }
}

fn write_geometry(geometry: &Geometry, out: &mut Vec<u8>) -> Result<()> {
    match geometry {
        Geometry::Point(point) => write_point(point.as_ref(), out)?,
        Geometry::LineString(line) => write_line(line, out),
        Geometry::LinearRing(ring) => {
            write_header(kind::LINE_STRING, out);
            write_coordinates(ring.coords(), out);
        }
        Geometry::Polygon(polygon) => write_polygon(polygon, out),
        Geometry::MultiPoint(points) => {
            write_header(kind::MULTI_POINT, out);
            write_count(points.len(), out);
            for point in points {
                write_point(point.as_ref(), out)?;
            }
        }
        Geometry::MultiLineString(lines) => {
            write_header(kind::MULTI_LINE_STRING, out);
            write_count(lines.len(), out);
            for line in lines {
                write_line(line, out);
            }
        }
        Geometry::MultiPolygon(polygons) => {
            write_header(kind::MULTI_POLYGON, out);
            write_count(polygons.len(), out);
            for polygon in polygons {
                write_polygon(polygon, out);
            }
        }
        Geometry::Collection(elements) => {
            write_header(kind::COLLECTION, out);
            write_count(elements.len(), out);
            for element in elements {
                write_geometry(element, out)?;
            }
        }
    }
    Ok(())
}
