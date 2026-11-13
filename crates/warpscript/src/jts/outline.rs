// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from GeoXP Lib (Copyright 1999-2019 Mathias Herberts, 2019-2020 iroise.net S.A.S.,
// 2020- SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! A `GEOSHAPE` as a GeoJSON `MultiPolygon`: what `->GEOJSON` makes of a shape
//! (`TOGEOJSON`, `CoverageHelper.toGeoJSON`).
//!
//! The outline of the cells of a cluster is traced from the segments that no two cells share.
//! Warp 10 does that with hash maps whose iteration order decides where each ring starts and
//! which way it runs, so [`JavaHashMap`] stands for them here, and the coordinates are
//! `float`s, printed as `Float.toString` does.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::coverage::{Coverage, MIN_RESOLUTION};
use crate::error::Result;
use crate::format::{java_double_to_string, java_float_to_string};
use crate::geoxp::{DEGREES_PER_LAT_UNIT, DEGREES_PER_LON_UNIT, hhcode_bbox, stable_split_hhcode};
use crate::javamap::{JavaHashMap, long_hash, ordered};

/// `TOGEOJSON` of a shape: one polygon per cell when `all_cells`, else the outline of each
/// cluster of cells.
///
/// # Errors
///
/// [`BoundsError::ArrayIndex`](crate::error::fault::BoundsError::ArrayIndex) for a geocell
/// without resolution, which no shape holds.
pub fn shape_to_geojson(cells: &[i64], all_cells: bool) -> Result<String> {
    if all_cells {
        return Ok(cells_to_geojson(cells));
    }
    let mut coverage = Coverage::from_cells(cells)?;
    coverage.dedup();
    coverage.optimize(0, MIN_RESOLUTION);
    let clusters = coverage
        .clusters()
        .iter()
        .map(|cluster| polygon_json(&rings(&cluster.to_geocells(30))))
        .collect::<Vec<_>>();
    Ok(format!(
        "{{\"type\":\"MultiPolygon\",\"coordinates\":[{}]}}",
        clusters.join(",")
    ))
}

/// A polygon per cell, counterclockwise from the south west.
fn cells_to_geojson(cells: &[i64]) -> String {
    let polygons = cells
        .iter()
        .map(|cell| {
            let resolution = ((*cell as u64 >> 60) as i32) << 1;
            let [south, west, north, east] = hhcode_bbox(cell << 4, resolution);
            let [south, west, north, east] = [south, west, north, east].map(java_double_to_string);
            format!(
                "[[[{west},{south}],[{east},{south}],[{east},{north}],[{west},{north}],[{west},{south}]]]"
            )
        })
        .collect::<Vec<_>>();
    format!(
        "{{\"type\":\"MultiPolygon\",\"coordinates\":[{}]}}",
        polygons.join(",")
    )
}

/// A vertex, `(latitude, longitude)`, rounded to a `float`.
type Vertex = (f32, f32);

/// The `(latitude, longitude)` units of a vertex, packed in a `long` (`latitude << 32 |
/// longitude`) as the hash maps hold them.
fn pack(lat: u32, lon: u32) -> i64 {
    (i64::from(lat) << 32) | i64::from(lon)
}

/// A segment along a line of the grid: from `start` to `end` units.
type Segment = (u32, u32);

/// `HHCodeHelper.toLat` of a unit, rounded to a `float`.
fn to_lat(units: i64) -> f32 {
    (DEGREES_PER_LAT_UNIT * units as f64 - 90.0) as f32
}

/// `HHCodeHelper.toLon` of a unit, rounded to a `float`.
fn to_lon(units: i64) -> f32 {
    (DEGREES_PER_LON_UNIT * units as f64 - 180.0) as f32
}

/// The segments of `Segment` ordered as the Java comparator does: by start, then longest first
/// (a max-heap of `Reverse` hands out the smallest first).
#[derive(Default)]
struct Queue(BinaryHeap<Reverse<(u32, Reverse<u32>)>>);

impl Queue {
    fn add(&mut self, start: u32, end: u32) {
        self.0.push(Reverse((start, Reverse(end))));
    }

    fn poll(&mut self) -> Option<Segment> {
        self.0
            .pop()
            .map(|Reverse((start, Reverse(end)))| (start, end))
    }
}

/// A segment as the `Long` a `HashSet<Long>` holds (`start << 32 | end`).
fn segment_key(segment: Segment) -> i64 {
    pack(segment.0, segment.1)
}

/// The segment a `Long` of a `HashSet<Long>` stands for.
fn key_segment(key: i64) -> Segment {
    ((key as u64 >> 32) as u32, key as u32)
}

/// `CoverageHelper.getSegments`: for each line of the grid (a longitude, or a latitude when
/// `swap`), the segments along it that border exactly one cell.
fn segments(cells: &[i64], swap: bool) -> JavaHashMap<u32, JavaHashMap<i64, ()>> {
    let mut lines: JavaHashMap<u32, Queue> = JavaHashMap::new().with_order(ordered);
    for cell in cells {
        let level = ((*cell as u64) >> 60) as u32;
        let (lat, lon) = stable_split_hhcode(cell << 4, level * 2);
        let (along, across) = if swap { (lon, lat) } else { (lat, lon) };
        // `Coverage.CELL_SIZE_BY_RES`; `1 << (32 - resolution)`.
        let size = 1i64 << (32 - level * 2);
        let limit = |units: i64| (units + size).min(0xFFFF_FFFF) as u32;
        let (start, end) = (along as u32, limit(along));
        for line in [across as u32, limit(across)] {
            lines.change_or_put(long_hash(i64::from(line)), line, Queue::default, |queue| {
                queue.add(start, end);
            });
        }
    }
    let mut borders = JavaHashMap::new().with_order(ordered);
    for (_, line, queue) in lines.into_entries() {
        let set = border_set(queue);
        if !set.is_empty() {
            borders.put(long_hash(i64::from(line)), line, set);
        }
    }
    borders
}

/// The segments of a line left once the parts that two cells share are cancelled, those that
/// follow each other joined.
fn border_set(mut queue: Queue) -> JavaHashMap<i64, ()> {
    let mut kept = Queue::default();
    let mut open: Option<Segment> = None;
    while let Some((start, end)) = queue.poll() {
        let Some((open_start, open_end)) = open else {
            open = Some((start, end));
            continue;
        };
        if (start, end) == (open_start, open_end) {
            // Both sides have a cell there: not a border.
            open = None;
        } else if end <= open_end {
            // Inside the open segment (the queue hands out the segments by start): it is cut.
            if start > open_start {
                queue.add(open_start, start);
            }
            if end < open_end {
                queue.add(end, open_end);
            }
            open = None;
        } else if start < open_end {
            // Overlaps the end of the open segment.
            if start > open_start {
                queue.add(open_start, start);
            }
            queue.add(open_end, end);
            open = None;
        } else {
            kept.add(open_start, open_end);
            open = Some((start, end));
        }
    }
    if let Some((start, end)) = open {
        kept.add(start, end);
    }
    let mut set = JavaHashMap::new().with_order(ordered);
    let mut joined: Option<Segment> = None;
    while let Some((start, end)) = kept.poll() {
        joined = match joined {
            Some((joined_start, joined_end)) if joined_end == start => Some((joined_start, end)),
            Some(done) => {
                set.put(long_hash(segment_key(done)), segment_key(done), ());
                Some((start, end))
            }
            None => Some((start, end)),
        };
    }
    if let Some(done) = joined {
        set.put(long_hash(segment_key(done)), segment_key(done), ());
    }
    set
}

/// `CoverageHelper.toEnvelope`: the rings that outline the cells (geocells of the resolutions 2
/// to 30), each as its vertices, the last one back at the first.
fn rings(cells: &[i64]) -> Vec<Vec<Vertex>> {
    let mut graph: JavaHashMap<i64, Vec<i64>> = JavaHashMap::new().with_order(ordered);
    let mut connect = |from: i64, to: i64| {
        for (a, b) in [(from, to), (to, from)] {
            graph.change_or_put(long_hash(a), a, Vec::new, |neighbors| neighbors.push(b));
        }
    };
    for (_, longitude, set) in segments(cells, false).iter() {
        for (_, segment, ()) in set.iter() {
            let (start, end) = key_segment(*segment);
            connect(pack(start, *longitude), pack(end, *longitude));
        }
    }
    for (_, latitude, set) in segments(cells, true).iter() {
        for (_, segment, ()) in set.iter() {
            let (start, end) = key_segment(*segment);
            connect(pack(*latitude, start), pack(*latitude, end));
        }
    }
    trace(graph)
        .into_iter()
        .map(|ring| {
            ring.into_iter()
                .map(|vertex| {
                    (
                        to_lat(i64::from((vertex as u64 >> 32) as u32)),
                        to_lon(vertex & 0xFFFF_FFFF),
                    )
                })
                .collect()
        })
        .collect()
}

/// Walk the segments of a graph (a vertex and the vertices it is joined to) into rings, as
/// `CoverageHelper.toEnvelope` does: start from the first vertex of the table, take the last
/// neighbor of each vertex, close the ring when back at the start.
fn trace(mut graph: JavaHashMap<i64, Vec<i64>>) -> Vec<Vec<i64>> {
    let mut rings = Vec::new();
    while let Some(first) = graph.first().map(|(_, key, _)| *key) {
        let mut ring = vec![first];
        let mut vertex = first;
        // A vertex without a segment left ends the ring short of its start.
        while let Some(next) = graph.get_mut(long_hash(vertex), &vertex).and_then(Vec::pop) {
            if let Some(neighbors) = graph.get_mut(long_hash(next), &next) {
                if let Some(position) = neighbors.iter().position(|n| *n == vertex) {
                    neighbors.remove(position);
                }
                if neighbors.is_empty() {
                    graph.remove(long_hash(next), &next);
                }
            }
            if graph
                .get(long_hash(vertex), &vertex)
                .is_some_and(Vec::is_empty)
            {
                graph.remove(long_hash(vertex), &vertex);
            }
            ring.push(next);
            if next == first {
                break;
            }
            vertex = next;
        }
        rings.push(ring);
    }
    rings
}

/// The coordinates of the polygon whose rings are given: `[[[lon,lat],...],...]`. A ring turns
/// the way the sign of its area says, the first one the other way round.
fn polygon_json(rings: &[Vec<Vertex>]) -> String {
    let rings = rings
        .iter()
        .enumerate()
        .map(|(index, ring)| {
            let area = ring.windows(2).fold(0.0f64, |area, pair| {
                let ((lat, lon), (next_lat, next_lon)) = (pair[0], pair[1]);
                area + f64::from((next_lon - lon) * (next_lat + lat))
            });
            let reversed = (area < 0.0) != (index == 0);
            let points: Vec<String> = if reversed {
                ring.iter().rev().copied().map(point_json).collect()
            } else {
                ring.iter().copied().map(point_json).collect()
            };
            format!("[{}]", points.join(","))
        })
        .collect::<Vec<_>>();
    format!("[{}]", rings.join(","))
}

/// `[lon,lat]`.
fn point_json((lat, lon): Vertex) -> String {
    format!(
        "[{},{}]",
        java_float_to_string(lon),
        java_float_to_string(lat)
    )
}
