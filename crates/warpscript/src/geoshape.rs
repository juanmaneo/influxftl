// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from GeoXP Lib (Copyright 1999-2019 Mathias Herberts, 2019-2020 iroise.net S.A.S.,
// 2020- SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `GEOSHAPE`: a set of geocells (`GeoXPLib.GeoXPShape`).
//!
//! A geocell packs the resolution (`1` to `15`, half the number of bits of each coordinate
//! divided by two) in its 4 most significant bits and the leading 60 bits of the HHCode of
//! the cell in the others. The cells of a shape are sorted as signed longs, which is why
//! the cells of resolutions 8 to 15 come before those of resolutions 1 to 7.

use std::ops::Range;

use crate::coverage::{Coverage, MAX_RESOLUTION, MIN_RESOLUTION};
use crate::encoder::{
    DEFAULT_COMP_RATIO_THRESHOLD, EncodedValue, GtsEncoder, encoder_to_wrapper, wrapper_to_decoder,
};
use crate::encoding::opb64_encode;
use crate::error::Result;
use crate::error::fault::BoundsError;
use crate::geoxp::{build_hhcode, stable_split_hhcode};
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::jmath;
use crate::thrift::{deserialize_wrapper, serialize_wrapper};

/// The thresholds that merge a single child into its parent, at every resolution.
const MERGE_ANY_CHILD: i64 = 0x1111_1111_1111_1111;

/// `GeoXPShapeHelper.RESOLUTION_MASKS`: the bits of a geocell that identify its cell at the
/// resolution `res` (1 to 15), the `res` leading hexadecimal digits under the resolution.
const fn resolution_mask(res: usize) -> i64 {
    0x0fff_ffff_ffff_ffff & (-1i64 << (60 - 4 * res))
}

/// The geocells of a shape that have each resolution, as the ranges of their indices (the
/// cells are sorted as signed longs, so those of the resolutions 8 to 15 come first).
struct Resolutions<'a> {
    cells: &'a [i64],
    ranges: [Range<usize>; 16],
}

/// The cells of two shapes that overlap a cell of the other.
struct Overlap {
    /// Whether checking found an overlap.
    found: bool,
    from_self: Vec<bool>,
    from_other: Vec<bool>,
}

impl<'a> Resolutions<'a> {
    fn new(cells: &'a [i64]) -> Resolutions<'a> {
        let mut ranges: [Range<usize>; 16] = std::array::from_fn(|_| 0..0);
        for (index, cell) in cells.iter().enumerate() {
            let range = &mut ranges[((*cell as u64) >> 60) as usize];
            if range.start == range.end {
                range.start = index;
            }
            range.end = index + 1;
        }
        Resolutions { cells, ranges }
    }

    /// Mark the cells of `other` that are in a cell of this shape (of the resolution `res`)
    /// not marked in `included`, or that contain one. Stops at the first one when only
    /// checking. `true` when a cell was marked.
    fn mark_overlaps(
        &self,
        other: &Resolutions<'_>,
        res: usize,
        included: &[bool],
        included_other: &mut [bool],
        check: bool,
    ) -> bool {
        for index in self.ranges[res].clone() {
            if included[index] {
                continue;
            }
            let cell = self.cells[index] & resolution_mask(res);
            for subres in res..16 {
                let range = other.ranges[subres].clone();
                let subcell = cell | ((subres as i64) << 60);
                let first =
                    range.start + other.cells[range.clone()].partition_point(|c| *c < subcell);
                let candidates = other.cells[first..range.end]
                    .iter()
                    .zip(&mut included_other[first..range.end]);
                for (other_cell, included) in candidates {
                    if cell != other_cell & resolution_mask(res) {
                        break;
                    }
                    if check {
                        return true;
                    }
                    *included = true;
                }
            }
        }
        false
    }
}

/// The geocells of a `GEOSHAPE`.
#[must_use]
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct GeoShape {
    cells: Vec<i64>,
}

impl GeoShape {
    /// `GeoXPLib.fromCells`: the shape of `cells`, which the caller has sorted.
    pub fn from_cells(cells: Vec<i64>) -> GeoShape {
        GeoShape { cells }
    }

    /// `GeoXPLib.getCells`.
    pub fn cells(&self) -> &[i64] {
        &self.cells
    }

    /// `GEOPACK.pack`: the cells as the timestamps of a gzipped `GTSWrapper`, in order
    /// preserving base 64.
    pub fn pack(&self) -> String {
        let mut encoder = GtsEncoder::new(0);
        for cell in &self.cells {
            encoder.add_value(
                *cell,
                NO_LOCATION,
                NO_ELEVATION,
                &EncodedValue::Boolean(true),
            );
        }
        let wrapper = encoder_to_wrapper(&encoder, true, DEFAULT_COMP_RATIO_THRESHOLD, true);
        opb64_encode(&serialize_wrapper(&wrapper))
    }

    /// `new Coverage(getCells(shape))`.
    fn coverage(&self) -> Result<Coverage> {
        Coverage::from_cells(&self.cells)
    }

    /// `GeoXPLib.fromCells(coverage.toGeoCells(MAX_RESOLUTION), false)`.
    fn from_coverage(coverage: &Coverage) -> GeoShape {
        GeoShape::from_cells(coverage.to_geocells(MAX_RESOLUTION as i32))
    }

    /// `GeoXPLib.union`: the cells of both shapes, those under another cell dropped and the
    /// cells merged where they fill their parent.
    pub fn union(&self, other: &GeoShape) -> Result<GeoShape> {
        let mut union = self.coverage()?;
        union.merge(&other.coverage()?);
        union.dedup();
        union.optimize(0, MIN_RESOLUTION);
        Ok(GeoShape::from_coverage(&union))
    }

    /// `GeoXPLib.subtraction`: this shape without the cells of `other`.
    pub fn subtraction(&self, other: &GeoShape) -> Result<GeoShape> {
        let mut shape = self.coverage()?;
        shape.minus(&mut other.coverage()?);
        shape.optimize(0, MIN_RESOLUTION);
        Ok(GeoShape::from_coverage(&shape))
    }

    /// `GeoXPShapeHelper.intersection`: the cells of both shapes that are in a cell of the
    /// other shape or contain one, those of each shape then sorted together (a cell that is
    /// in both shapes is in the result twice).
    pub fn intersection(&self, other: &GeoShape) -> GeoShape {
        let overlap = self.overlap(other, false);
        let mut cells: Vec<i64> = self
            .cells
            .iter()
            .zip(&overlap.from_self)
            .chain(other.cells.iter().zip(&overlap.from_other))
            .filter_map(|(cell, included)| included.then_some(*cell))
            .collect();
        cells.sort_unstable();
        GeoShape::from_cells(cells)
    }

    /// `GeoXPShapeHelper.intersects`: whether the intersection of both shapes has cells.
    pub fn intersects(&self, other: &GeoShape) -> bool {
        self.overlap(other, true).found
    }

    /// The cells of each shape that overlap a cell of the other, resolution after
    /// resolution; when only `check`ing, whether there is one.
    fn overlap(&self, other: &GeoShape, check: bool) -> Overlap {
        let (cells, other_cells) = (
            Resolutions::new(&self.cells),
            Resolutions::new(&other.cells),
        );
        let mut overlap = Overlap {
            found: false,
            from_self: vec![false; self.cells.len()],
            from_other: vec![false; other.cells.len()],
        };
        for res in 1..16 {
            if cells.mark_overlaps(
                &other_cells,
                res,
                &overlap.from_self,
                &mut overlap.from_other,
                check,
            ) || other_cells.mark_overlaps(
                &cells,
                res,
                &overlap.from_other,
                &mut overlap.from_self,
                check,
            ) {
                overlap.found = true;
                break;
            }
        }
        overlap
    }

    /// `Coverage.contains(geocells, hhcode)`: whether a cell of the shape holds the point.
    pub fn contains_point(&self, hhcode: i64) -> bool {
        (1..16usize).any(|res| {
            let key = ((res as i64) << 60) | ((hhcode >> 4) & resolution_mask(res));
            self.cells.binary_search(&key).is_ok()
        })
    }

    /// `GeoXPLib.limitResolution(shape, resolution, MAX_RESOLUTION)` (`GEO.OPTIMIZE` with a
    /// resolution): the cells finer than `resolution` (even, 2 to 30) merged into their
    /// parents.
    pub fn limit_resolution(&self, resolution: u32) -> GeoShape {
        let mut coverage = Coverage::from_geocells(&self.cells);
        coverage.optimize(MERGE_ANY_CHILD, resolution);
        GeoShape::from_cells(coverage.to_geocells(30))
    }

    /// `GEO.OPTIMIZE` without resolution: the cells merged where they fill their parent.
    pub fn optimized(&self) -> GeoShape {
        let mut coverage = Coverage::from_geocells(&self.cells);
        coverage.optimize(0, MIN_RESOLUTION);
        GeoShape::from_cells(coverage.to_geocells(30))
    }

    /// The finest resolution of the cells (1 to 15), 0 for no cell.
    pub fn finest_resolution(&self) -> usize {
        self.cells
            .iter()
            .map(|cell| ((*cell as u64) >> 60) as usize)
            .max()
            .unwrap_or(0)
    }

    /// `GEOSHIFT`: the shape moved by `delta_lat` and `delta_lon` degrees, rounded to cells of
    /// `resolution` (1 to 15, at most the finest of the shape): the cells of that resolution
    /// or finer move and the coarser ones are split to it first. With a `meridian`, a cell
    /// further than a quarter of the equator from it moves the opposite way in latitude (a
    /// shape moved over a pole).
    pub fn shift(
        &self,
        resolution: usize,
        meridian: Option<f64>,
        delta_lat: f64,
        delta_lon: f64,
    ) -> GeoShape {
        // The Java computes the units with `int` shifts, which wrap at 32 bits.
        let bits = (resolution << 1) as u32;
        let cells_around = 1i32 << bits;
        let unit = i64::from(1i32.wrapping_shl(32 - bits));
        let quarter_equator = i64::from((cells_around >> 2).wrapping_shl(32 - bits));
        let cell_lat = 180.0 / f64::from(cells_around);
        let cell_lon = 360.0 / f64::from(cells_around);
        let long_meridian = meridian.map_or(0, |meridian| jmath::round(meridian / cell_lon) * unit);
        let long_delta_lat = jmath::round(delta_lat / cell_lat) * unit;
        let long_delta_lon = jmath::round(delta_lon / cell_lon) * unit;

        let mut coverage = Coverage::new();
        coverage.set_auto_optimize();
        let mut add_shifted = |res: usize, hhcode: i64| {
            let resolution = (res << 1) as u32;
            let (lat, lon) = stable_split_hhcode(hhcode, resolution);
            let factor = if meridian.is_some() && (lon - long_meridian).abs() > quarter_equator {
                -1
            } else {
                1
            };
            let shifted = build_hhcode(lat + factor * long_delta_lat, lon + long_delta_lon);
            coverage.add_cell(resolution as i32, shifted);
        };
        let mut coarser = Vec::new();
        for cell in &self.cells {
            let res = ((*cell as u64) >> 60) as usize;
            if res >= resolution {
                add_shifted(res, *cell << 4);
            } else {
                coarser.push(*cell);
            }
        }
        while let Some(cell) = coarser.pop() {
            let res = ((cell as u64) >> 60) as usize;
            let hhcode = cell << 4;
            if res >= resolution {
                add_shifted(res, hhcode);
                continue;
            }
            for offset in 0..16i64 {
                let child = (hhcode | (offset << (60 - 4 * res))) as u64 >> 4;
                coarser.push((((res + 1) as i64) << 60) | child as i64);
            }
        }
        GeoShape::from_coverage(&coverage)
    }

    /// `GEO.NORMALIZE`: both shapes with their cells split to the finest resolution of the
    /// cells they overlap.
    pub fn normalize(&self, other: &GeoShape) -> Result<(GeoShape, GeoShape)> {
        let (mut a, mut b) = (self.coverage()?, other.coverage()?);
        Coverage::normalize(&mut a, &mut b);
        Ok((GeoShape::from_coverage(&a), GeoShape::from_coverage(&b)))
    }

    /// `HHCodeHelper.geocellsToRegexp`: a regular expression for the HHCodes (in hexadecimal)
    /// of the cells. The cells of resolutions 1 to 7, which come first in the order of the
    /// regular expression, then those of resolutions 8 to 15.
    pub fn regexp(&self) -> Result<String> {
        let mut regexps = Vec::new();
        let mut current = String::new();
        let mut last_prefix: Option<String> = None;
        let coarse = self
            .cells
            .iter()
            .filter(|cell| **cell >= 0x1000_0000_0000_0000);
        let fine = self
            .cells
            .iter()
            .filter(|cell| **cell < 0x1000_0000_0000_0000);
        for cell in coarse.chain(fine) {
            let res = ((*cell as u64) >> 60) as usize;
            let hex = format!("{:016x}", *cell as u64 | 0xf000_0000_0000_0000);
            // The only hexadecimal digit of the resolution 2 is the one that is listed.
            if res == 1 {
                if current.is_empty() {
                    current.push('[');
                }
                current.push_str(&hex[1..2]);
                continue;
            }
            let prefix = hex.get(1..res).ok_or(BoundsError::StringSlice {
                begin: 1,
                end: res,
                length: 16,
            })?;
            if last_prefix.as_deref() != Some(prefix) {
                if current.len() > 1 {
                    current.push_str("].*");
                    regexps.push(std::mem::take(&mut current));
                }
                current.push_str(prefix);
                current.push('[');
            }
            current.push_str(&hex[res..=res]);
            last_prefix = Some(prefix.to_owned());
        }
        current.push_str("].*");
        regexps.push(current);
        Ok(regexps.join("|"))
    }

    /// `GEOUNPACK`: the shape of a serialized `GTSWrapper` holding geocells as timestamps.
    /// The timestamps whose resolution is 0 are not cells and are dropped; the cells are
    /// sorted and deduplicated.
    pub fn unpack(serialized: &[u8]) -> Result<GeoShape> {
        let wrapper = deserialize_wrapper(serialized)?;
        let mut decoder = wrapper_to_decoder(&wrapper)?;
        // `(int) wrapper.getCount()` sizes the array the cells are read into.
        let count = wrapper.count() as i32;
        if count < 0 {
            return Err(BoundsError::NegativeSize(i64::from(count)).into());
        }
        let mut cells = Vec::new();
        while decoder.next()? {
            let cell = decoder.timestamp();
            if cell as u64 >> 60 != 0 {
                cells.push(cell);
            }
        }
        cells.sort_unstable();
        cells.dedup();
        Ok(GeoShape { cells })
    }
}
