// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! What the oracle tables of `GEO.WKB` and `->WKB` cannot say: the counts and the nesting that
//! only a crafted input gives, the flags of the geometry type, the byte orders, and the failures
//! of the writer a script reaches through a geometry it built.

use super::geometry::{Coord, Geometry};
use super::wkb::{read, write};
use crate::error::Error;
use crate::error::fault::{BoundsError, ResourceError};
use crate::error::geometry::GeometryError;
use crate::finite::NotFinite;
use crate::tests::on_big_stack;

/// The geometry type of a point with elevations (`0x80000000`).
const WITH_Z: u32 = 0x8000_0000;

/// The geometry type of a point with an SRID (`0x20000000`).
const WITH_SRID: u32 = 0x2000_0000;

/// A big endian header: the byte order, then the geometry type.
fn header(kind: u32) -> Vec<u8> {
    let mut bytes = vec![0];
    bytes.extend(kind.to_be_bytes());
    bytes
}

/// A big endian count or flag.
fn int(value: i32) -> [u8; 4] {
    value.to_be_bytes()
}

/// A big endian ordinate.
fn double(value: f64) -> [u8; 8] {
    value.to_be_bytes()
}

/// A count, then the two dimensional points.
fn points(points: &[(f64, f64)]) -> Vec<u8> {
    let count = i32::try_from(points.len()).expect("a short list of points");
    let mut bytes = int(count).to_vec();
    for (x, y) in points {
        bytes.extend(double(*x));
        bytes.extend(double(*y));
    }
    bytes
}

/// The concatenation of `parts`.
fn bytes(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// The failure of reading `input`, which must be refused.
fn failure(input: &[u8]) -> Error {
    read(input).expect_err("the input is not valid WKB")
}

/// `depth` collections holding one element each, the innermost one empty.
fn nested_collections(depth: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    for _ in 0..depth {
        bytes.extend(header(7));
        bytes.extend(int(1));
    }
    bytes.extend(header(7));
    bytes.extend(int(0));
    bytes
}

#[test]
fn a_negative_count_is_refused_wherever_it_is_read() {
    let minus_one = || Error::from(BoundsError::NegativeSize(-1));
    // A line, a polygon shell, a polygon hole, a multi-point, a member of a multi-line string,
    // a member of a collection: each reads its count first.
    assert_eq!(failure(&bytes(&[&header(2), &int(-1)])), minus_one());
    assert_eq!(
        failure(&bytes(&[&header(3), &int(1), &int(-1)])),
        minus_one()
    );
    assert_eq!(
        failure(&bytes(&[&header(3), &int(2), &int(0), &int(-1)])),
        minus_one()
    );
    assert_eq!(failure(&bytes(&[&header(4), &int(-1)])), minus_one());
    assert_eq!(
        failure(&bytes(&[&header(5), &int(1), &header(2), &int(-1)])),
        minus_one()
    );
    assert_eq!(failure(&bytes(&[&header(7), &int(-1)])), minus_one());
}

#[test]
fn counts_past_the_limit_are_refused_before_anything_is_read() {
    let out_of_memory = || Error::from(ResourceError::OutOfMemory);
    let past_the_limit = (1 << 24) + 1;
    assert_eq!(
        failure(&bytes(&[&header(2), &int(past_the_limit)])),
        out_of_memory()
    );
    assert_eq!(
        failure(&bytes(&[&header(4), &int(i32::MAX)])),
        out_of_memory()
    );
    // A polygon of 2^24 + 2 rings announces 2^24 + 1 holes.
    assert_eq!(
        failure(&bytes(&[&header(3), &int(past_the_limit + 1)])),
        out_of_memory()
    );
}

#[test]
fn collections_nested_past_512_levels_overflow_the_stack() {
    on_big_stack(nested_collections_body);
}

fn nested_collections_body() {
    read(&nested_collections(512)).expect("512 levels of collections are read");
    assert_eq!(
        failure(&nested_collections(513)),
        Error::from(ResourceError::StackOverflow)
    );
}

#[test]
fn ordinates_must_be_finite() {
    for (bad, error) in [
        (f64::NAN, NotFinite::Nan),
        (f64::INFINITY, NotFinite::PositiveInfinity),
        (f64::NEG_INFINITY, NotFinite::NegativeInfinity),
    ] {
        let point = bytes(&[&header(1), &double(bad), &double(0.0)]);
        assert_eq!(failure(&point), Error::from(error));
        let point_with_z = bytes(&[
            &header(WITH_Z | 1),
            &double(1.0),
            &double(2.0),
            &double(bad),
        ]);
        assert_eq!(failure(&point_with_z), Error::from(error));
        let line = bytes(&[
            &header(2),
            &int(2),
            &double(0.0),
            &double(0.0),
            &double(bad),
            &double(1.0),
        ]);
        assert_eq!(failure(&line), Error::from(error));
    }
}

#[test]
fn elevations_and_srids_are_read_and_the_srid_is_dropped() {
    let point = |x, y, z| Coord::new(x, y, z).expect("finite");
    let with_z = bytes(&[
        &header(WITH_Z | 1),
        &double(1.0),
        &double(2.0),
        &double(3.0),
    ]);
    assert_eq!(
        read(&with_z),
        Ok(Geometry::Point(Some(point(1.0, 2.0, Some(3.0)))))
    );
    let with_srid = bytes(&[
        &header(WITH_SRID | 1),
        &int(4326),
        &double(1.0),
        &double(2.0),
    ]);
    assert_eq!(
        read(&with_srid),
        Ok(Geometry::Point(Some(point(1.0, 2.0, None))))
    );
    let with_both = bytes(&[
        &header(WITH_SRID | WITH_Z | 1),
        &int(4326),
        &double(1.0),
        &double(2.0),
        &double(3.0),
    ]);
    assert_eq!(
        read(&with_both),
        Ok(Geometry::Point(Some(point(1.0, 2.0, Some(3.0)))))
    );
}

#[test]
fn little_endian_geometries_are_read_with_their_own_header() {
    let mut little = vec![1];
    little.extend(1u32.to_le_bytes());
    little.extend(1.5f64.to_le_bytes());
    little.extend((-2.5f64).to_le_bytes());
    assert_eq!(
        read(&little),
        Ok(Geometry::Point(Some(
            Coord::new(1.5, -2.5, None).expect("finite")
        )))
    );
}

#[test]
fn unknown_geometry_types_are_refused() {
    assert_eq!(
        failure(&header(8)),
        Error::from(GeometryError::UnknownWkbType(8))
    );
}

#[test]
fn a_polygon_with_holes_and_no_shell_is_refused() {
    let square = [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
    let holes_without_shell = bytes(&[&header(3), &int(2), &int(0), &points(&square)]);
    assert_eq!(
        failure(&holes_without_shell),
        Error::from(GeometryError::HolesWithoutShell)
    );
}

#[test]
fn members_of_a_multi_geometry_have_its_type() {
    let line = bytes(&[&header(2), &points(&[(0.0, 0.0), (1.0, 1.0)])]);
    let point = bytes(&[&header(1), &double(1.0), &double(2.0)]);
    assert_eq!(
        failure(&bytes(&[&header(4), &int(1), &line])),
        Error::from(GeometryError::WkbMemberType("MultiPoint"))
    );
    assert_eq!(
        failure(&bytes(&[&header(5), &int(1), &point])),
        Error::from(GeometryError::WkbMemberType("MultiLineString"))
    );
    assert_eq!(
        failure(&bytes(&[&header(6), &int(1), &point])),
        Error::from(GeometryError::WkbMemberType("MultiPolygon"))
    );
    assert_eq!(
        failure(&bytes(&[&header(7), &int(1), &header(8)])),
        Error::from(GeometryError::UnknownWkbType(8))
    );
}

#[test]
fn the_writer_refuses_empty_points() {
    let empty = Error::from(GeometryError::EmptyPointWkb);
    assert_eq!(write(&Geometry::Point(None)), Err(empty.clone()));
    assert_eq!(write(&Geometry::MultiPoint(vec![None])), Err(empty.clone()));
    assert_eq!(
        write(&Geometry::Collection(vec![Geometry::Point(None)])),
        Err(empty)
    );
}

#[test]
fn the_writer_is_big_endian_and_two_dimensional() {
    let point = Geometry::Point(Some(Coord::new(1.0, 2.0, Some(3.0)).expect("finite")));
    assert_eq!(
        write(&point),
        Ok(bytes(&[&header(1), &double(1.0), &double(2.0)]))
    );
}
