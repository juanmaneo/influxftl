// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The GML and KML readers and writer on the geometries the oracle tables cannot build by
//! themselves: the elevations, the round trips, and the depth of the elements.

use super::geometry::{Coord, Geometry, Line, Polygon, Ring};
use super::gml::{Dialect, read, write};
use crate::error::fault::ResourceError;
use crate::error::gml::GmlError;
use crate::error::{Class, Error};

fn at(x: f64, y: f64, z: Option<f64>) -> Coord {
    Coord::new(x, y, z).expect("finite coordinate")
}

fn line(coords: Vec<Coord>) -> Line {
    Line::new(coords).expect("a line of two coordinates")
}

fn square(from: f64, to: f64) -> Ring {
    Ring::closed(vec![
        at(from, from, None),
        at(to, from, None),
        at(to, to, None),
        at(from, to, None),
    ])
}

#[test]
fn the_elevation_of_a_position_is_kept() {
    assert_eq!(
        read(
            "<gml:Point><gml:coordinates>1,2,3</gml:coordinates></gml:Point>",
            Dialect::Gml
        ),
        Ok(Geometry::Point(Some(at(1.0, 2.0, Some(3.0)))))
    );
}

/// The collections are not in it: Warp 10 refuses the GML of a collection that its writer
/// puts (a `NullPointerException` on the `geometryMember`), and so does this reader.
#[test]
fn geometries_are_read_back_from_what_the_writer_puts() {
    let geometries = [
        Geometry::Point(Some(at(1.0, 2.0, Some(3.0)))),
        Geometry::LineString(line(vec![at(1.0, 2.0, None), at(3.0, 4.0, None)])),
        Geometry::Polygon(
            Polygon::new(square(0.0, 10.0), vec![square(2.0, 4.0)]).expect("a polygon with a hole"),
        ),
        Geometry::MultiPoint(vec![Some(at(1.0, 2.0, None)), Some(at(3.0, 4.0, None))]),
        Geometry::MultiLineString(vec![line(vec![at(1.0, 2.0, None), at(3.0, 4.0, None)])]),
    ];
    for geometry in &geometries {
        let text = write(geometry, Dialect::Gml).expect("a GML text");
        assert_eq!(read(&text, Dialect::Gml).as_ref(), Ok(geometry), "{text}");
    }
    for geometry in geometries.iter().take(3) {
        let text = write(geometry, Dialect::Kml).expect("a KML text");
        assert_eq!(read(&text, Dialect::Kml).as_ref(), Ok(geometry), "{text}");
    }
}

#[test]
fn elements_nested_too_deep_end_in_a_stack_overflow() {
    assert!(!matches!(
        read(&"<a>".repeat(511), Dialect::Gml),
        Err(Error::ResourceExhausted(ResourceError::StackOverflow))
    ));
    assert_eq!(
        read(&"<a>".repeat(600), Dialect::Gml),
        Err(ResourceError::StackOverflow.into())
    );
}

/// A hole that is not a ring is refused when the polygon is made: the oracle's answer is an
/// `ArrayStoreException` whose message depends on its JIT, so only the class is checked.
#[test]
fn a_hole_that_is_not_a_ring_is_an_array_store() {
    let text = "<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:innerBoundaryIs></gml:Polygon>";
    assert_eq!(
        read(text, Dialect::Gml),
        Err(GmlError::Store(Class::Point).into())
    );
}

/// A token that lacks an ordinate of a sequence of more dimensions reads `NaN` in Warp 10 (the
/// oracle keeps it); the geometries refuse it (`docs/DIVERGENCES.md`, section 37).
#[test]
fn a_missing_ordinate_of_a_sequence_is_not_finite() {
    for coordinates in ["1,2 3", "1,2,3 4,5"] {
        let text = format!(
            "<gml:LineString><gml:coordinates>{coordinates}</gml:coordinates></gml:LineString>"
        );
        assert!(
            matches!(read(&text, Dialect::Gml), Err(Error::NotFinite(_))),
            "{text}"
        );
    }
}

/// A `NaN` abscissa of a coordinate: Warp 10 keeps it, the geometries refuse it
/// (`docs/DIVERGENCES.md`, section 37).
#[test]
fn a_coordinate_with_a_nan_abscissa_is_not_finite() {
    assert!(matches!(
        read(
            "<gml:Point><gml:coord><gml:X>NaN</gml:X><gml:Y>1</gml:Y></gml:coord></gml:Point>",
            Dialect::Gml
        ),
        Err(Error::NotFinite(_))
    ));
}

/// A linear ring of coordinates and a coordinate sequence: the sequence is refused as an array
/// store. The oracle's message depends on its JIT, so only the class is checked.
#[test]
fn a_linear_ring_of_coordinates_and_a_sequence_is_an_array_store() {
    let text = "<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>";
    assert_eq!(
        read(text, Dialect::Gml),
        Err(GmlError::Store(Class::ArraySequence).into())
    );
}

/// An ordinate given where a point is expected: the oracle's answer is a `ClassCastException`
/// whose message depends on its JIT, so the variant is checked here.
#[test]
fn an_ordinate_is_not_a_point() {
    assert_eq!(
        read("<gml:Point><gml:X>1</gml:X></gml:Point>", Dialect::Gml),
        Err(GmlError::Cast {
            from: Class::Double,
            to: Class::CoordinateSequence,
        }
        .into())
    );
}

/// A document whose root is not a geometry: the oracle's answer is a `ClassCastException`
/// whose message depends on its JIT, so the variant is checked here.
#[test]
fn a_root_that_is_not_a_geometry_is_a_cast() {
    assert_eq!(
        read("<gml:X>1</gml:X>", Dialect::Gml),
        Err(GmlError::Cast {
            from: Class::Double,
            to: Class::Geometry,
        }
        .into())
    );
}
