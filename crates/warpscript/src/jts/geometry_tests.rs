// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

use super::geometry::{Coord, Envelope, Geometry, Line, Polygon, Ring};
use crate::error::geometry::GeometryError;
use crate::error::{Error, Result};
use crate::finite::NotFinite;

fn at(x: f64, y: f64) -> Coord {
    Coord::new(x, y, None).expect("finite coordinate")
}

fn square() -> Ring {
    Ring::new(vec![
        at(0.0, 0.0),
        at(0.0, 2.0),
        at(2.0, 2.0),
        at(2.0, 0.0),
        at(0.0, 0.0),
    ])
    .expect("closed ring")
}

#[test]
fn coordinates_must_be_finite() {
    for (bad, error) in [
        (f64::NAN, NotFinite::Nan),
        (f64::INFINITY, NotFinite::PositiveInfinity),
        (f64::NEG_INFINITY, NotFinite::NegativeInfinity),
    ] {
        for (x, y, z) in [(bad, 1.0, None), (1.0, bad, None), (1.0, 2.0, Some(bad))] {
            assert_eq!(Coord::new(x, y, z), Err(Error::NotFinite(error)));
        }
    }
    let coord = Coord::new(1.0, 2.0, Some(3.0)).expect("finite coordinate");
    assert_eq!((coord.x, coord.y, coord.z), (1.0, 2.0, Some(3.0)));
}

#[test]
fn lines_have_no_point_or_two() {
    assert_eq!(
        Line::new(vec![at(1.0, 2.0)]),
        Err(GeometryError::LineStringTooShort.into())
    );
    assert!(
        Line::new(Vec::new())
            .expect("empty line")
            .coords()
            .is_empty()
    );
    let line = Line::new(vec![at(1.0, 2.0), at(3.0, 4.0)]).expect("line");
    assert_eq!(line.coords().len(), 2);
    // The WKB reader repeats a single point instead of refusing it.
    assert_eq!(
        Line::extended(vec![at(1.0, 2.0)]).coords(),
        &[at(1.0, 2.0), at(1.0, 2.0)]
    );
    assert_eq!(
        Line::extended(vec![at(1.0, 2.0), at(3.0, 4.0)])
            .coords()
            .len(),
        2
    );
    assert!(Line::extended(Vec::new()).coords().is_empty());
}

#[test]
fn rings_are_closed_and_have_no_point_or_four() {
    assert_eq!(
        Ring::new(vec![at(1.0, 2.0)]),
        Err(GeometryError::LineStringTooShort.into())
    );
    assert_eq!(
        Ring::new(vec![at(0.0, 0.0), at(0.0, 1.0), at(1.0, 1.0), at(1.0, 0.0)]),
        Err(GeometryError::RingNotClosed.into())
    );
    assert_eq!(
        Ring::new(vec![at(0.0, 0.0), at(1.0, 1.0), at(0.0, 0.0)]),
        Err(GeometryError::RingTooShort(3).into())
    );
    assert!(
        Ring::new(Vec::new())
            .expect("empty ring")
            .coords()
            .is_empty()
    );
    assert_eq!(square().coords().len(), 5);
    // An elevation does not make a ring open.
    let up = Coord::new(0.0, 0.0, Some(1.0)).expect("coordinate");
    assert_eq!(
        Ring::new(vec![at(0.0, 0.0), at(0.0, 1.0), at(1.0, 1.0), up])
            .expect("closed by position")
            .coords()
            .len(),
        4
    );
    assert_eq!(
        Ring::new(vec![
            up,
            at(0.0, 1.0),
            at(1.0, 1.0),
            at(1.0, 0.0),
            at(0.0, 0.0)
        ])
        .expect("closed by position")
        .coords()
        .len(),
        5
    );
}

#[test]
fn closed_completes_a_ring_instead_of_refusing_it() {
    let a = at(0.0, 0.0);
    let b = at(0.0, 1.0);
    let c = at(1.0, 1.0);
    let d = at(1.0, 0.0);
    assert!(Ring::closed(Vec::new()).coords().is_empty());
    assert_eq!(Ring::closed(vec![a]).coords(), &[a, a, a, a]);
    assert_eq!(Ring::closed(vec![a, b]).coords(), &[a, b, a, a]);
    assert_eq!(Ring::closed(vec![a, b, c]).coords(), &[a, b, c, a]);
    assert_eq!(Ring::closed(vec![a, b, c, d]).coords(), &[a, b, c, d, a]);
    assert_eq!(Ring::closed(vec![a, b, c, a]).coords(), &[a, b, c, a]);
}

#[test]
fn polygons_have_a_shell_before_holes() {
    let empty = || Ring::new(Vec::new()).expect("empty ring");
    assert_eq!(
        Polygon::new(empty(), vec![square()]),
        Err(GeometryError::HolesWithoutShell.into())
    );
    assert!(Polygon::new(empty(), vec![empty()]).is_ok());
    let polygon = Polygon::new(square(), vec![square()]).expect("polygon");
    assert_eq!(polygon.shell(), &square());
    assert_eq!(polygon.holes().len(), 1);
}

#[test]
fn rectangles_go_round_their_envelope() {
    let rectangle = |points: &[(f64, f64)]| {
        let ring = Ring::new(points.iter().map(|(x, y)| at(*x, *y)).collect()).expect("ring");
        Polygon::new(ring, Vec::new()).expect("polygon")
    };
    assert!(square_polygon().is_rectangle());
    // A shell that goes through the corners in the other direction is one too.
    assert!(
        rectangle(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0), (0.0, 0.0)]).is_rectangle()
    );
    // Crossing corners, a triangle, a point off the corners, a hole.
    assert!(
        !rectangle(&[(0.0, 0.0), (2.0, 2.0), (2.0, 0.0), (0.0, 2.0), (0.0, 0.0)]).is_rectangle()
    );
    assert!(!rectangle(&[(0.0, 0.0), (0.0, 2.0), (2.0, 0.0), (0.0, 0.0)]).is_rectangle());
    assert!(
        !rectangle(&[(0.0, 0.0), (0.0, 2.0), (1.0, 2.0), (2.0, 0.0), (0.0, 0.0)]).is_rectangle()
    );
    assert!(
        !rectangle(&[(0.0, 0.0), (0.0, 2.0), (2.0, 3.0), (2.0, 0.0), (0.0, 0.0)]).is_rectangle()
    );
    let holed = Polygon::new(square(), vec![square()]).expect("polygon");
    assert!(!holed.is_rectangle());
    let empty = Polygon::new(Ring::new(Vec::new()).expect("ring"), Vec::new()).expect("polygon");
    assert!(!empty.is_rectangle());
}

fn square_polygon() -> Polygon {
    Polygon::new(square(), Vec::new()).expect("polygon")
}

#[test]
fn envelopes_cover_and_meet() {
    assert_eq!(Envelope::of(&[]), None);
    let envelope = Envelope::of(&[at(1.0, 5.0), at(-2.0, 3.0), at(0.0, 7.0)]).expect("envelope");
    assert_eq!(
        envelope,
        Envelope {
            min_x: -2.0,
            max_x: 1.0,
            min_y: 3.0,
            max_y: 7.0
        }
    );
    let inside = Envelope::of(&[at(0.0, 4.0), at(1.0, 6.0)]).expect("envelope");
    let apart = Envelope::of(&[at(2.0, 4.0), at(3.0, 6.0)]).expect("envelope");
    let touching = Envelope::of(&[at(1.0, 7.0), at(3.0, 9.0)]).expect("envelope");
    assert!(envelope.covers(&inside) && !inside.covers(&envelope));
    assert!(envelope.intersects(&inside) && envelope.intersects(&touching));
    assert!(!envelope.covers(&touching));
    for side in [
        Envelope {
            min_x: 2.0,
            ..apart
        },
        Envelope {
            max_x: -3.0,
            min_x: -4.0,
            ..apart
        },
        Envelope {
            min_y: 8.0,
            max_y: 9.0,
            ..inside
        },
        Envelope {
            min_y: 0.0,
            max_y: 2.0,
            ..inside
        },
    ] {
        assert!(!envelope.intersects(&side), "{side:?}");
    }
    // Not covered on each side in turn.
    for wider in [
        Envelope {
            min_x: -3.0,
            ..inside
        },
        Envelope {
            max_x: 2.0,
            ..inside
        },
        Envelope {
            min_y: 2.0,
            ..inside
        },
        Envelope {
            max_y: 8.0,
            ..inside
        },
    ] {
        assert!(!envelope.covers(&wider), "{wider:?}");
    }
}

#[test]
fn collections_refuse_elements_that_are_missing() {
    assert_eq!(
        Geometry::collection(vec![Some(Geometry::Point(None)), None]),
        Err(GeometryError::NullElement.into())
    );
    assert_eq!(
        Geometry::collection(vec![Some(Geometry::Point(None))]),
        Ok(Geometry::Collection(vec![Geometry::Point(None)]))
    );
}

fn every_kind() -> Vec<(Geometry, bool, bool)> {
    let line = Line::new(vec![at(1.0, 2.0), at(3.0, 4.0)]).expect("line");
    let no_line = Line::new(Vec::new()).expect("line");
    let no_ring = Ring::new(Vec::new()).expect("ring");
    let no_polygon = Polygon::new(no_ring.clone(), Vec::new()).expect("polygon");
    vec![
        (Geometry::Point(Some(at(1.0, 2.0))), false, true),
        (Geometry::Point(None), true, false),
        (Geometry::LineString(line.clone()), false, true),
        (Geometry::LineString(no_line.clone()), true, false),
        (Geometry::LinearRing(square()), false, true),
        (Geometry::LinearRing(no_ring), true, false),
        (Geometry::Polygon(square_polygon()), false, true),
        (Geometry::Polygon(no_polygon.clone()), true, false),
        (
            Geometry::MultiPoint(vec![None, Some(at(1.0, 2.0))]),
            false,
            true,
        ),
        (Geometry::MultiPoint(vec![None]), true, false),
        (
            Geometry::MultiLineString(vec![no_line.clone(), line]),
            false,
            true,
        ),
        (Geometry::MultiLineString(vec![no_line]), true, false),
        (
            Geometry::MultiPolygon(vec![no_polygon.clone(), square_polygon()]),
            false,
            true,
        ),
        (Geometry::MultiPolygon(vec![no_polygon]), true, false),
        (
            Geometry::Collection(vec![Geometry::Point(Some(at(1.0, 2.0)))]),
            false,
            true,
        ),
        (
            Geometry::Collection(vec![Geometry::Point(None)]),
            true,
            false,
        ),
        (Geometry::Collection(Vec::new()), true, false),
    ]
}

#[test]
fn emptiness_and_envelope_agree() {
    for (geometry, empty, has_envelope) in every_kind() {
        assert_eq!(geometry.is_empty(), empty, "{geometry:?}");
        assert_eq!(geometry.envelope().is_some(), has_envelope, "{geometry:?}");
    }
}

#[test]
fn envelope_of_a_multi_geometry_merges_its_parts() {
    let points = Geometry::MultiPoint(vec![Some(at(1.0, 2.0)), None, Some(at(-3.0, 8.0))]);
    assert_eq!(
        points.envelope(),
        Some(Envelope {
            min_x: -3.0,
            max_x: 1.0,
            min_y: 2.0,
            max_y: 8.0
        })
    );
}

#[test]
fn results_carry_the_typed_errors() {
    let result: Result<Line> = Line::new(vec![at(0.0, 0.0)]);
    assert!(matches!(
        result,
        Err(Error::Geometry(GeometryError::LineStringTooShort))
    ));
}
