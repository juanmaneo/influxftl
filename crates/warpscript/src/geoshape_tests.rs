// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from GeoXP Lib (Copyright 1999-2019 Mathias Herberts, 2019-2020 iroise.net S.A.S.,
// 2020- SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of the shapes that scripts cannot build: a geocell without resolution is refused
//! like Warp 10 does (an `ArrayIndexOutOfBoundsException` of the cell array, or a
//! `StringIndexOutOfBoundsException` of its HHCode).

use crate::error::fault::BoundsError;
use crate::geoshape::GeoShape;

/// A shape of one geocell of resolution 2.
fn shape() -> GeoShape {
    GeoShape::from_cells(vec![0x1100_0000_0000_0000])
}

/// A geocell has a resolution, from 1 to 15: this one has none.
fn without_resolution() -> GeoShape {
    GeoShape::from_cells(vec![1])
}

fn no_cell_array_index() -> crate::error::Error {
    BoundsError::ArrayIndex {
        index: -1,
        length: 16,
    }
    .into()
}

#[test]
fn a_union_refuses_a_cell_without_resolution() {
    assert_eq!(
        shape()
            .union(&without_resolution())
            .expect_err("the cell has no resolution"),
        no_cell_array_index()
    );
    assert_eq!(
        without_resolution()
            .union(&shape())
            .expect_err("the cell has no resolution"),
        no_cell_array_index()
    );
}

#[test]
fn a_subtraction_refuses_a_cell_without_resolution() {
    assert_eq!(
        shape()
            .subtraction(&without_resolution())
            .expect_err("the cell has no resolution"),
        no_cell_array_index()
    );
    assert_eq!(
        without_resolution()
            .subtraction(&shape())
            .expect_err("the cell has no resolution"),
        no_cell_array_index()
    );
}

#[test]
fn a_normalization_refuses_a_cell_without_resolution() {
    assert_eq!(
        shape()
            .normalize(&without_resolution())
            .expect_err("the cell has no resolution"),
        no_cell_array_index()
    );
    assert_eq!(
        without_resolution()
            .normalize(&shape())
            .expect_err("the cell has no resolution"),
        no_cell_array_index()
    );
}

#[test]
fn a_regexp_refuses_a_cell_without_resolution() {
    assert_eq!(
        without_resolution()
            .regexp()
            .expect_err("the cell has no resolution"),
        BoundsError::StringSlice {
            begin: 1,
            end: 0,
            length: 16
        }
        .into()
    );
}

#[test]
fn a_cell_without_resolution_overlaps_nothing() {
    assert!(!without_resolution().intersects(&without_resolution()));
    assert_eq!(
        without_resolution().intersection(&shape()).cells(),
        &[] as &[i64]
    );
}

#[test]
fn the_finest_resolution_is_that_of_the_finest_cell() {
    assert_eq!(GeoShape::default().finest_resolution(), 0);
    assert_eq!(shape().finest_resolution(), 1);
    assert_eq!(
        GeoShape::from_cells(vec![0x1100_0000_0000_0000, 0x4abc_0000_0000_0000])
            .finest_resolution(),
        4
    );
}
