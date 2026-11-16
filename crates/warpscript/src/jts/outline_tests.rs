// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The walk of the outline of a shape over graphs that `rings` does not build: its segments
//! are not all joined both ways. Java fails on them, the walk ends there.

use super::outline::{shape_to_geojson, trace};
use crate::error::Error;
use crate::error::fault::BoundsError;
use crate::javamap::{JavaHashMap, long_hash, ordered};

/// A graph whose vertices are in the table in the order of their (small) numbers.
fn graph(vertices: &[(i64, &[i64])]) -> JavaHashMap<i64, Vec<i64>> {
    let mut graph = JavaHashMap::new().with_order(ordered);
    for (vertex, neighbors) in vertices {
        graph.put(long_hash(*vertex), *vertex, neighbors.to_vec());
    }
    graph
}

#[test]
fn a_graph_without_vertex_has_no_ring() {
    assert_eq!(trace(graph(&[])), Vec::<Vec<i64>>::new());
}

#[test]
fn each_vertex_takes_its_last_neighbor() {
    let triangle: &[(i64, &[i64])] = &[(1, &[2, 3]), (2, &[1, 3]), (3, &[1, 2])];
    assert_eq!(trace(graph(triangle)), [[1, 3, 2, 1]]);
}

#[test]
fn each_component_is_a_ring() {
    let two: &[(i64, &[i64])] = &[
        (1, &[2, 3]),
        (2, &[1, 3]),
        (3, &[1, 2]),
        (5, &[6, 7]),
        (6, &[5, 7]),
        (7, &[5, 6]),
    ];
    assert_eq!(trace(graph(two)), [vec![1, 3, 2, 1], vec![5, 7, 6, 5]]);
}

#[test]
fn a_ring_may_pass_through_its_start_before_it_ends() {
    // Two triangles that share the vertex 1.
    let bow: &[(i64, &[i64])] = &[
        (1, &[2, 3, 4, 5]),
        (2, &[1, 3]),
        (3, &[1, 2]),
        (4, &[1, 5]),
        (5, &[1, 4]),
    ];
    assert_eq!(trace(graph(bow)), [vec![1, 5, 4, 1], vec![1, 3, 2, 1]]);
}

#[test]
fn a_neighbor_that_is_no_vertex_ends_the_ring() {
    assert_eq!(trace(graph(&[(1, &[2])])), [[1, 2]]);
}

#[test]
fn a_segment_that_is_not_joined_back_is_walked_once() {
    assert_eq!(trace(graph(&[(1, &[2]), (2, &[3])])), [[1, 2, 3]]);
}

#[test]
fn a_vertex_joined_to_itself_closes_its_ring() {
    assert_eq!(trace(graph(&[(1, &[1])])), [[1, 1]]);
}

#[test]
fn a_vertex_without_neighbor_ends_the_ring_and_goes() {
    assert_eq!(trace(graph(&[(1, &[2]), (2, &[])])), [[1, 2]]);
    assert_eq!(trace(graph(&[(1, &[])])), [[1]]);
}

#[test]
fn a_geocell_without_resolution_is_no_shape() {
    assert!(matches!(
        shape_to_geojson(&[0], false),
        Err(Error::Bounds(BoundsError::ArrayIndex { .. }))
    ));
}
