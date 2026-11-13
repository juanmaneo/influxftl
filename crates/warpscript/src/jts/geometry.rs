// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The geometries the readers build and the writers print.
//!
//! The model is the one of the JTS Topology Suite 1.13 that Warp 10 bundles, as far as a
//! script can tell: a point may be empty, a line string has no point or at least two, a
//! linear ring is closed and has no point or at least four, a polygon has no holes without a
//! shell, a coordinate has an optional elevation. The types check those invariants when they
//! are built, with the failure JTS raises.

// Positions are compared exactly, as JTS does (`equals2D`, the corners of a rectangle): the
// coordinates are finite by construction.
#![allow(clippy::float_cmp)]

use crate::error::Result;
use crate::error::geometry::GeometryError;
use crate::finite::ensure_finite;

/// A position: `x` is the longitude, `y` the latitude, `z` the optional elevation (`NaN` in JTS
/// when it is absent).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coord {
    /// The abscissa.
    pub x: f64,
    /// The ordinate.
    pub y: f64,
    /// The elevation, when the geometry has one.
    pub z: Option<f64>,
}

impl Coord {
    /// A coordinate of finite ordinates.
    ///
    /// # Errors
    ///
    /// [`NotFinite`](crate::finite::NotFinite) for a `NaN` or infinite ordinate: JTS accepts
    /// them, but the library does not let them in a geometry.
    pub fn new(x: f64, y: f64, z: Option<f64>) -> Result<Coord> {
        Ok(Coord {
            x: ensure_finite(x)?,
            y: ensure_finite(y)?,
            z: z.map(ensure_finite).transpose()?,
        })
    }

    /// `equals2D`: the same position, whatever the elevation.
    fn same_position(&self, other: &Coord) -> bool {
        self.x == other.x && self.y == other.y
    }
}

/// A line string (`LineString`): no point or at least two.
#[derive(Clone, Debug, PartialEq)]
pub struct Line(Vec<Coord>);

impl Line {
    /// The line string through `coords`.
    ///
    /// # Errors
    ///
    /// [`GeometryError::LineStringTooShort`] for a single point.
    pub fn new(coords: Vec<Coord>) -> Result<Line> {
        if coords.len() == 1 {
            return Err(GeometryError::LineStringTooShort.into());
        }
        Ok(Line(coords))
    }

    /// The line through `coords`, a single point being repeated to make two (what the WKB reader
    /// does with a line of one point, that `LineString` refuses).
    pub fn extended(mut coords: Vec<Coord>) -> Line {
        if let [point] = coords.as_slice() {
            coords.push(*point);
        }
        Line(coords)
    }

    /// The points of the line.
    pub fn coords(&self) -> &[Coord] {
        &self.0
    }
}

/// A closed line string (`LinearRing`): no point or at least four.
#[derive(Clone, Debug, PartialEq)]
pub struct Ring(Vec<Coord>);

impl Ring {
    /// The ring through `coords`.
    ///
    /// # Errors
    ///
    /// [`GeometryError::LineStringTooShort`] for a single point, [`GeometryError::RingNotClosed`]
    /// when the last point is not the first and [`GeometryError::RingTooShort`] for 2 or 3
    /// points.
    pub fn new(coords: Vec<Coord>) -> Result<Ring> {
        if coords.len() == 1 {
            return Err(GeometryError::LineStringTooShort.into());
        }
        if let (Some(first), Some(last)) = (coords.first(), coords.last())
            && !first.same_position(last)
        {
            return Err(GeometryError::RingNotClosed.into());
        }
        if (1..4).contains(&coords.len()) {
            return Err(GeometryError::RingTooShort(coords.len()).into());
        }
        Ok(Ring(coords))
    }

    /// The ring through `coords`, closed and completed with the first point if it is not valid
    /// (what the WKB reader does: `CoordinateSequences.ensureValidRing`).
    pub fn closed(mut coords: Vec<Coord>) -> Ring {
        if let Some(first) = coords.first().copied() {
            let closed = coords.last().is_some_and(|last| first.same_position(last));
            match coords.len() {
                1..=3 => coords.resize(4, first),
                _ if !closed => coords.push(first),
                _ => {}
            }
        }
        Ring(coords)
    }

    /// The points of the ring.
    pub fn coords(&self) -> &[Coord] {
        &self.0
    }
}

/// A polygon: a shell and the holes in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Polygon {
    shell: Ring,
    holes: Vec<Ring>,
}

impl Polygon {
    /// The polygon of a `shell` and its `holes`.
    ///
    /// # Errors
    ///
    /// [`GeometryError::HolesWithoutShell`] for an empty shell and a hole that is not.
    pub fn new(shell: Ring, holes: Vec<Ring>) -> Result<Polygon> {
        if shell.coords().is_empty() && holes.iter().any(|hole| !hole.coords().is_empty()) {
            return Err(GeometryError::HolesWithoutShell.into());
        }
        Ok(Polygon { shell, holes })
    }

    /// The exterior ring.
    pub fn shell(&self) -> &Ring {
        &self.shell
    }

    /// The interior rings.
    pub fn holes(&self) -> &[Ring] {
        &self.holes
    }

    /// `isRectangle`: no holes and a shell of 5 points that are the corners of the envelope,
    /// in an order that goes round it.
    pub fn is_rectangle(&self) -> bool {
        let shell = self.shell.coords();
        let (Some(envelope), true, true) =
            (Envelope::of(shell), self.holes.is_empty(), shell.len() == 5)
        else {
            return false;
        };
        let on_corners = shell.iter().all(|c| {
            (c.x == envelope.min_x || c.x == envelope.max_x)
                && (c.y == envelope.min_y || c.y == envelope.max_y)
        });
        let going_round = shell
            .windows(2)
            .all(|pair| (pair[0].x != pair[1].x) != (pair[0].y != pair[1].y));
        on_corners && going_round
    }
}

/// A bounding box (`Envelope`) of at least one point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Envelope {
    /// The smallest abscissa.
    pub min_x: f64,
    /// The largest abscissa.
    pub max_x: f64,
    /// The smallest ordinate.
    pub min_y: f64,
    /// The largest ordinate.
    pub max_y: f64,
}

impl Envelope {
    /// The envelope of `coords`, or `None` for no point.
    pub fn of(coords: &[Coord]) -> Option<Envelope> {
        coords.iter().fold(None, |envelope, c| {
            Some(match envelope {
                None => Envelope {
                    min_x: c.x,
                    max_x: c.x,
                    min_y: c.y,
                    max_y: c.y,
                },
                Some(e) => Envelope {
                    min_x: e.min_x.min(c.x),
                    max_x: e.max_x.max(c.x),
                    min_y: e.min_y.min(c.y),
                    max_y: e.max_y.max(c.y),
                },
            })
        })
    }

    /// The smallest envelope that holds `self` and `other`.
    fn union(self, other: Envelope) -> Envelope {
        Envelope {
            min_x: self.min_x.min(other.min_x),
            max_x: self.max_x.max(other.max_x),
            min_y: self.min_y.min(other.min_y),
            max_y: self.max_y.max(other.max_y),
        }
    }

    /// `covers`: `other` is inside `self`, the edges included.
    pub fn covers(&self, other: &Envelope) -> bool {
        other.min_x >= self.min_x
            && other.max_x <= self.max_x
            && other.min_y >= self.min_y
            && other.max_y <= self.max_y
    }

    /// `intersects`: the envelopes share a point, the edges included.
    pub fn intersects(&self, other: &Envelope) -> bool {
        !(other.min_x > self.max_x
            || other.max_x < self.min_x
            || other.min_y > self.max_y
            || other.max_y < self.min_y)
    }
}

/// A geometry of the types JTS reads and writes.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    /// `Point`, empty without coordinate.
    Point(Option<Coord>),
    /// `LineString`.
    LineString(Line),
    /// `LinearRing`.
    LinearRing(Ring),
    /// `Polygon`.
    Polygon(Polygon),
    /// `MultiPoint`: its points may be empty.
    MultiPoint(Vec<Option<Coord>>),
    /// `MultiLineString`.
    MultiLineString(Vec<Line>),
    /// `MultiPolygon`.
    MultiPolygon(Vec<Polygon>),
    /// `GeometryCollection`.
    Collection(Vec<Geometry>),
}

impl Geometry {
    /// The geometry collection of `elements`, which are all geometries.
    pub fn collection(elements: Vec<Option<Geometry>>) -> Result<Geometry> {
        elements
            .into_iter()
            .map(|element| element.ok_or(GeometryError::NullElement))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map(Geometry::Collection)
            .map_err(Into::into)
    }

    /// `isEmpty`: no coordinate at all.
    pub fn is_empty(&self) -> bool {
        match self {
            Geometry::Point(point) => point.is_none(),
            Geometry::LineString(line) => line.coords().is_empty(),
            Geometry::LinearRing(ring) => ring.coords().is_empty(),
            Geometry::Polygon(polygon) => polygon.shell().coords().is_empty(),
            Geometry::MultiPoint(points) => points.iter().all(Option::is_none),
            Geometry::MultiLineString(lines) => lines.iter().all(|line| line.coords().is_empty()),
            Geometry::MultiPolygon(polygons) => polygons
                .iter()
                .all(|polygon| polygon.shell().coords().is_empty()),
            Geometry::Collection(elements) => elements.iter().all(Geometry::is_empty),
        }
    }

    /// `getEnvelopeInternal`: the bounding box, `None` for an empty geometry.
    pub fn envelope(&self) -> Option<Envelope> {
        match self {
            Geometry::Point(point) => point.and_then(|c| Envelope::of(&[c])),
            Geometry::LineString(line) => Envelope::of(line.coords()),
            Geometry::LinearRing(ring) => Envelope::of(ring.coords()),
            Geometry::Polygon(polygon) => Envelope::of(polygon.shell().coords()),
            Geometry::MultiPoint(points) => {
                merged(points.iter().map(|p| p.and_then(|c| Envelope::of(&[c]))))
            }
            Geometry::MultiLineString(lines) => {
                merged(lines.iter().map(|line| Envelope::of(line.coords())))
            }
            Geometry::MultiPolygon(polygons) => merged(
                polygons
                    .iter()
                    .map(|polygon| Envelope::of(polygon.shell().coords())),
            ),
            Geometry::Collection(elements) => merged(elements.iter().map(Geometry::envelope)),
        }
    }
}

/// The envelope of the envelopes that exist.
fn merged(envelopes: impl Iterator<Item = Option<Envelope>>) -> Option<Envelope> {
    envelopes.flatten().reduce(Envelope::union)
}
