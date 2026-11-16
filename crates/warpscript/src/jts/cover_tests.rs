// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! What the oracle tables of `GEO.WKT` and the like cannot say: the failures that need a
//! resolution of 32 (a script stops at 30), a limit that a script cannot lower, and the
//! numbers that are not finite.

use super::cover::{
    Budget, MAX_PENDING_CELLS, Precision, cover_cells, cover_geometry, optimal_resolution, to_units,
};
use super::geometry::{Coord, Geometry, Polygon, Ring};
use crate::error::fault::ResourceError;
use crate::error::geometry::GeometryError;
use crate::error::{Error, Result};
use crate::finite::NotFinite;
use crate::geoxp::{DEGREES_PER_LAT_UNIT, DEGREES_PER_LON_UNIT};

const ROOMY: Budget = Budget {
    cells: 1_000_000,
    pending: MAX_PENDING_CELLS,
};

fn at(x: f64, y: f64) -> Coord {
    Coord::new(x, y, None).expect("finite coordinate")
}

fn point(x: f64, y: f64) -> Geometry {
    Geometry::Point(Some(at(x, y)))
}

fn rectangle(west: f64, south: f64, east: f64, north: f64) -> Geometry {
    let shell = Ring::new(vec![
        at(west, south),
        at(west, north),
        at(east, north),
        at(east, south),
        at(west, south),
    ])
    .expect("closed ring");
    Geometry::Polygon(Polygon::new(shell, Vec::new()).expect("polygon"))
}

/// The failure of a cover, whatever it was to give.
fn failure<T>(cover: Result<T>) -> Result<()> {
    cover.map(|_| ())
}

fn cell_count(geometry: &Geometry, max_resolution: u32, inside: bool) -> usize {
    cover_geometry(geometry, 2, max_resolution, inside, ROOMY)
        .expect("a cover")
        .expect("within the limits")
        .cell_count()
}

#[test]
fn the_tolerated_error_must_be_finite() {
    for (bad, error) in [
        (f64::NAN, NotFinite::Nan),
        (f64::INFINITY, NotFinite::PositiveInfinity),
        (f64::NEG_INFINITY, NotFinite::NegativeInfinity),
    ] {
        assert_eq!(
            failure(cover_cells(
                &point(1.0, 2.0),
                Precision::ErrorFraction(bad),
                false,
                false,
                10
            )),
            Err(Error::NotFinite(error))
        );
    }
}

#[test]
fn a_tolerated_distance_that_overflows_is_refused() {
    let bounds = [0, 0, 1 << 31, 1 << 31];
    assert_eq!(
        failure(optimal_resolution(&bounds, f64::MAX)),
        Err(Error::NotFinite(NotFinite::PositiveInfinity))
    );
    assert_eq!(
        failure(optimal_resolution(&bounds, -f64::MAX)),
        Err(Error::NotFinite(NotFinite::NegativeInfinity))
    );
    assert_eq!(optimal_resolution(&bounds, 0.0), Ok(32));
    assert_eq!(optimal_resolution(&bounds, 1.0), Ok(2));
}

#[test]
fn a_cover_whose_tolerated_distance_overflows_is_refused() {
    assert_eq!(
        failure(cover_cells(
            &rectangle(0.0, 0.0, 1.0, 1.0),
            Precision::ErrorFraction(f64::MAX),
            false,
            false,
            10
        )),
        Err(Error::NotFinite(NotFinite::PositiveInfinity))
    );
}

#[test]
fn degrees_are_the_units_of_the_planisphere() {
    assert_eq!(to_units(-180.0, 180.0, DEGREES_PER_LON_UNIT), 0);
    assert_eq!(to_units(0.0, 180.0, DEGREES_PER_LON_UNIT), 1 << 31);
    assert_eq!(to_units(180.0, 180.0, DEGREES_PER_LON_UNIT), 0xFFFF_FFFF);
    assert_eq!(to_units(90.0, 90.0, DEGREES_PER_LAT_UNIT), 0xFFFF_FFFF);
    // Just below the planisphere is the unit before the first, further below is further.
    assert_eq!(to_units(-180.000_000_01, 180.0, DEGREES_PER_LON_UNIT), -1);
    assert!(to_units(-200.0, 180.0, DEGREES_PER_LON_UNIT) < -1);
}

#[test]
fn a_geometry_without_a_point_has_no_cell() {
    for geometry in [Geometry::Point(None), Geometry::Collection(Vec::new())] {
        let cover = cover_geometry(&geometry, 2, 8, false, ROOMY)
            .expect("a cover")
            .expect("within the limits");
        assert_eq!(cover.cell_count(), 0);
        assert_eq!(
            cover_cells(&geometry, Precision::ErrorFraction(0.1), false, false, 10),
            Ok(Some(Vec::new()))
        );
        // A limit of no cell is reached before the first one.
        assert_eq!(
            cover_cells(&geometry, Precision::Resolution(8), false, false, 0),
            Ok(None)
        );
    }
}

#[test]
fn a_limit_of_no_cell_is_reached_at_once() {
    assert_eq!(
        cover_cells(
            &rectangle(0.0, 0.0, 1.0, 1.0),
            Precision::Resolution(4),
            false,
            false,
            0
        ),
        Ok(None)
    );
}

#[test]
fn the_cells_waiting_to_be_split_are_limited() {
    let square = rectangle(0.0, 0.0, 1.0, 1.0);
    let narrow = Budget {
        cells: 1_000,
        pending: 16,
    };
    assert_eq!(
        failure(cover_geometry(&square, 2, 10, false, narrow)),
        Err(ResourceError::OutOfMemory.into())
    );
    assert!(cover_geometry(&square, 2, 10, false, ROOMY).is_ok_and(|cover| cover.is_some()));
}

fn multipoint(points: Vec<Option<Coord>>) -> Geometry {
    Geometry::MultiPoint(points)
}

#[test]
fn an_empty_point_goes_unnoticed_when_a_point_sits_on_the_corner_of_the_cell() {
    let corner = multipoint(vec![Some(at(-180.0, -90.0)), None]);
    let cells = cover_cells(&corner, Precision::Resolution(8), false, false, 100);
    assert_eq!(cells.expect("a cover").expect("within the limits").len(), 1);
    // The cells of the resolution 32 under it have that corner too.
    assert_eq!(cell_count(&corner, 32, false), 1);
}

#[test]
fn an_empty_point_is_a_null_pointer_where_the_topology_is_needed() {
    // A cell that the envelope covers.
    let covered = multipoint(vec![Some(at(-180.0, -90.0)), Some(at(0.0, 0.0)), None]);
    assert_eq!(
        failure(cover_cells(
            &covered,
            Precision::Resolution(6),
            false,
            false,
            1_000
        )),
        Err(Error::NullPointer)
    );
    // A cell of the resolution 32 that the envelope meets.
    let near = multipoint(vec![Some(at(0.0, 0.0)), None]);
    assert_eq!(
        failure(cover_geometry(&near, 2, 32, false, ROOMY)),
        Err(Error::NullPointer)
    );
}

#[test]
fn a_geometry_collection_is_refused_where_the_topology_is_needed() {
    let refused = Err(Error::Geometry(GeometryError::CollectionArgument));
    // A cell that the envelope covers.
    let covered = Geometry::Collection(vec![point(-180.0, -90.0), point(-90.0, -45.0)]);
    assert_eq!(
        failure(cover_cells(
            &covered,
            Precision::Resolution(4),
            false,
            false,
            1_000
        )),
        refused
    );
    // A cell of the resolution 32 that the envelope meets: the envelope is the first one.
    let cell = Geometry::Collection(vec![
        point(-180.0, -90.0),
        point(-180.0 + DEGREES_PER_LON_UNIT, -90.0 + DEGREES_PER_LAT_UNIT),
    ]);
    assert_eq!(failure(cover_geometry(&cell, 2, 32, false, ROOMY)), refused);
}

#[test]
fn the_cells_of_the_resolution_32_of_a_rectangle_are_those_it_meets_or_covers() {
    let tiny = rectangle(0.0, 0.0, 0.000_001, 0.000_001);
    let touching = cell_count(&tiny, 32, false);
    let inside = cell_count(&tiny, 32, true);
    assert!(inside > 0, "{inside}");
    assert!(touching > inside, "{touching} > {inside}");
}
