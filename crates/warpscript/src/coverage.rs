// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from GeoXP Lib (Copyright 1999-2019 Mathias Herberts, 2019-2020 iroise.net S.A.S.,
// 2020- SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Coverages: sets of HHCode cells, one set per resolution (`com.geoxp.geo.Coverage`).
//!
//! A cell of resolution `2 * (r + 1)` is stored as the HHCode of its first point, its lower
//! bits cleared. The Java sets are hash sets, but every iteration over them is either
//! followed by a sort or does not depend on the order, so ordered sets behave the same.

use std::collections::{BTreeSet, VecDeque};

use crate::error::Result;
use crate::error::fault::BoundsError;
use crate::geoxp::delta_hhcode;

/// The HHCode bits a cell of each resolution keeps (index `r` for resolution `2 * (r + 1)`).
const PREFIX_MASK: [i64; 16] = {
    let mut masks = [0i64; 16];
    let mut i = 0;
    while i < 16 {
        masks[i] = -1i64 << (60 - 4 * i);
        i += 1;
    }
    masks
};

/// `HHCodeHelper.MIN_RESOLUTION`.
pub const MIN_RESOLUTION: u32 = 2;
/// `HHCodeHelper.MAX_RESOLUTION`.
pub const MAX_RESOLUTION: u32 = 32;

/// `Coverage`: the cells of each of the 16 resolutions (2, 4 … 32).
#[derive(Clone, Debug, Default)]
pub struct Coverage {
    cells: [BTreeSet<i64>; 16],
    /// Thresholds to optimize with as cells are added.
    auto_thresholds: i64,
    /// Whether to optimize as cells are added.
    auto_optimize: bool,
    /// Whether to skip the cells whose parent is already there.
    auto_dedup: bool,
}

impl Coverage {
    /// `new Coverage()`: no cells.
    pub fn new() -> Coverage {
        Coverage::default()
    }

    /// `setAutoThresholds`: optimize with `thresholds` as cells are added.
    pub fn set_auto_thresholds(&mut self, thresholds: i64) {
        self.auto_thresholds = thresholds;
        self.auto_optimize = true;
    }

    /// `setAutoOptimize(true)`: optimize as cells are added, merging 16 children (the
    /// thresholds are those of [`Coverage::new`]).
    pub fn set_auto_optimize(&mut self) {
        self.auto_optimize = true;
    }

    /// `setAutoDedup`.
    pub fn set_auto_dedup(&mut self, auto_dedup: bool) {
        self.auto_dedup = auto_dedup;
    }

    /// `new Coverage(long[] geocells)`: the cells of a shape, as they are (no masking of the
    /// HHCode bits below the resolution).
    ///
    /// # Errors
    ///
    /// [`BoundsError::ArrayIndex`] for a geocell without resolution, which no shape holds.
    pub fn from_cells(geocells: &[i64]) -> Result<Coverage> {
        let mut coverage = Coverage::new();
        for geocell in geocells {
            let level = ((*geocell as u64) >> 60) as usize;
            let cells = level
                .checked_sub(1)
                .and_then(|level| coverage.cells.get_mut(level))
                .ok_or(BoundsError::ArrayIndex {
                    index: -1,
                    length: 16,
                })?;
            cells.insert(*geocell << 4);
        }
        Ok(coverage)
    }

    /// `CoverageHelper.fromGeoCells`: add the cells of a shape one by one (the geocells
    /// without resolution are ignored) and merge them with [`Coverage::optimize`].
    pub fn from_geocells(geocells: &[i64]) -> Coverage {
        let mut coverage = Coverage::new();
        for geocell in geocells {
            coverage.add_cell((((*geocell as u64) >> 60) << 1) as i32, *geocell << 4);
        }
        coverage.optimize(0, MIN_RESOLUTION);
        coverage
    }

    /// `merge`: add the cells of `other`.
    pub fn merge(&mut self, other: &Coverage) {
        for (cells, others) in self.cells.iter_mut().zip(&other.cells) {
            cells.extend(others);
        }
    }

    /// `dedup`: drop the cells under a coarser cell.
    pub fn dedup(&mut self) {
        for level in (0..16).rev() {
            let (coarser, finer) = self.cells.split_at_mut(level);
            finer[0].retain(|hhcode| {
                !coarser
                    .iter()
                    .zip(&PREFIX_MASK)
                    .any(|(cells, mask)| cells.contains(&(hhcode & mask)))
            });
        }
    }

    /// `removeCell`: remove the cell of the resolution `2 * (level + 1)` holding `hhcode`.
    fn remove_cell(&mut self, level: usize, hhcode: i64) {
        self.cells[level].remove(&(hhcode & PREFIX_MASK[level]));
    }

    /// `splitTo`: replace the cell holding `hhcode` by its children, and those by theirs,
    /// down to the resolution of `level`, when a coarser cell covers `hhcode`.
    fn split_to(&mut self, level: usize, hhcode: i64) {
        while let Some(covered) =
            (0..=level).find(|r| self.cells[*r].contains(&(hhcode & PREFIX_MASK[*r])))
        {
            if covered == level {
                return;
            }
            let parent = hhcode & PREFIX_MASK[covered];
            self.cells[covered].remove(&parent);
            let child_shift = 60 - 4 * (covered + 1);
            for child in 0..16i64 {
                self.cells[covered + 1]
                    .insert((parent | (child << child_shift)) & PREFIX_MASK[covered + 1]);
            }
        }
    }

    /// `Coverage.normalize(a, b)`: split the cells of each coverage so that wherever one
    /// has a cell, the other has cells of that resolution only.
    pub fn normalize(a: &mut Coverage, b: &mut Coverage) {
        for level in 0..16 {
            for hhcode in &a.cells[level] {
                b.split_to(level, *hhcode);
            }
            for hhcode in &b.cells[level] {
                a.split_to(level, *hhcode);
            }
        }
    }

    /// `Coverage.minus(a, b, false)`: remove from `self` the cells of `other`. Both are
    /// optimized and normalized in the process.
    pub fn minus(&mut self, other: &mut Coverage) {
        self.optimize(0, MIN_RESOLUTION);
        other.optimize(0, MIN_RESOLUTION);
        Coverage::normalize(self, other);
        for (level, cells) in other.cells.iter().enumerate() {
            for hhcode in cells {
                self.remove_cell(level, *hhcode);
            }
        }
    }

    /// `parent` replaces its 16 children at `level`.
    fn replace_children(&mut self, level: usize, parent: i64) {
        self.cells[level - 1].insert(parent);
        for offset in 0..16 {
            self.cells[level].remove(&(parent | (offset << (4 * (15 - level)))));
        }
    }

    /// `optimize(thresholds, minresolution, MAX_RESOLUTION, 0)`: from the finest resolution up
    /// to `min_resolution`, replace the children of a parent by the parent when there are at
    /// least as many as the threshold of their resolution (a nibble of `thresholds`, 0
    /// meaning all 16). `min_resolution` is at least 2: nothing is coarser than the
    /// resolution 2.
    ///
    /// The Java scans the sorted cells of a resolution by groups of siblings and counts the
    /// last cell with the group before it, whatever its parent: that is kept, as Warp 10
    /// optimizes shapes that way.
    pub fn optimize(&mut self, thresholds: i64, min_resolution: u32) {
        let coarsest = ((min_resolution >> 1) as usize).max(1);
        for level in (coarsest..16).rev() {
            let threshold = (thresholds >> (4 * (15 - level))) & 0xf;
            let merges = |children: i64| (threshold > 0 && children >= threshold) || children == 16;
            let sorted: Vec<i64> = self.cells[level].iter().copied().collect();
            let mut parent_cell = 0i64;
            let mut last_parent = 0i64;
            let mut children = 0i64;
            for (index, hhcode) in sorted.iter().enumerate() {
                let last = index + 1 == sorted.len();
                parent_cell = hhcode & PREFIX_MASK[level - 1];
                if index == 0 {
                    last_parent = parent_cell;
                    children = 0;
                }
                if last_parent != parent_cell || last {
                    if last {
                        children += 1;
                    }
                    if merges(children) {
                        self.replace_children(level, last_parent);
                    }
                    if !last {
                        last_parent = parent_cell;
                    }
                    children = 1;
                } else {
                    children += 1;
                }
            }
            if last_parent != parent_cell && merges(children) {
                self.replace_children(level, parent_cell);
            }
        }
        // A cell two resolutions under a cell already there is redundant.
        for level in 0..14 {
            if self.cells[level].is_empty() {
                continue;
            }
            let (coarser, finer) = self.cells.split_at_mut(level + 2);
            let coarser = &coarser[level];
            finer[0].retain(|hhcode| !coarser.contains(&(hhcode & PREFIX_MASK[level])));
        }
    }

    /// `addCell(resolution, hhcode)`: add the cell of `resolution` (even, 2 to 32) holding
    /// `hhcode`. Other resolutions are ignored.
    pub fn add_cell(&mut self, resolution: i32, hhcode: i64) {
        let r = (resolution >> 1) - 1;
        if r & !0xf != 0 {
            return;
        }
        let r = r as usize;
        let hhcode = hhcode & PREFIX_MASK[r];
        // Java scans the parents from the finest or the coarsest depending on `r`; either
        // way the cell is skipped when any parent is there.
        if self.auto_dedup
            && (0..r).any(|parent| self.cells[parent].contains(&(hhcode & PREFIX_MASK[parent])))
        {
            return;
        }
        self.cells[r].insert(hhcode);
        if self.auto_optimize {
            let mut resolution = resolution;
            while self.optimize_cell(hhcode, resolution, self.auto_thresholds) {
                resolution -= 2;
            }
        }
    }

    /// `optimizeCell`: replace the children of the parent of `hhcode` by the parent when at
    /// least as many as the threshold of `resolution` are there. `true` when the parent was
    /// added, so that the parent may be optimized in turn.
    fn optimize_cell(&mut self, hhcode: i64, resolution: i32, thresholds: i64) -> bool {
        // Nothing coarser than resolution 2 (and no odd resolution is ever optimized).
        if resolution < 4 {
            return false;
        }
        let r = ((resolution >> 1) - 1) as usize;
        // A threshold of 0 stands for 16, which does not fit in 4 bits.
        let threshold = match (thresholds >> (4 * (15 - r))) & 0xf {
            0 => 16,
            threshold => threshold,
        };
        if (self.cells[r].len() as i64) < threshold {
            return false;
        }
        let parent = hhcode & PREFIX_MASK[r - 1];
        let child = |offset: i64| parent | (offset << (4 * (15 - r)));
        let count = (0..16)
            .filter(|offset| self.cells[r].contains(&child(*offset)))
            .count() as i64;
        if count < threshold {
            return false;
        }
        self.cells[r - 1].insert(parent);
        for offset in 0..16 {
            self.cells[r].remove(&child(offset));
        }
        true
    }

    /// `CoverageHelper.clusters`: the groups of cells that touch each other (to the north,
    /// east, south or west, at their own resolution or at a coarser one), each as a
    /// coverage, the first cell of each group (in geocell order) giving its place.
    pub fn clusters(&self) -> Vec<Coverage> {
        let cells = self.to_geocells(MAX_RESOLUTION as i32);
        let mut cluster_ids = vec![0usize; cells.len()];
        let mut current = 1;
        while let Some(first) = cluster_ids.iter().position(|id| *id == 0) {
            cluster_ids[first] = current;
            let mut window = VecDeque::from([cells[first]]);
            while let Some(cell) = window.pop_front() {
                for neighbor in neighbors(cell).iter().rev() {
                    let Ok(index) = cells.binary_search(neighbor) else {
                        continue;
                    };
                    match cluster_ids[index] {
                        0 => {
                            cluster_ids[index] = current;
                            window.push_back(*neighbor);
                        }
                        id if id != current => {
                            // The neighbor belongs to a cluster found earlier: join it.
                            for other in cluster_ids.iter_mut().filter(|other| **other == id) {
                                *other = current;
                            }
                        }
                        _ => {}
                    }
                }
            }
            current += 1;
        }
        // A cluster that was joined to another has no cell left.
        (1..current)
            .filter_map(|id| {
                let members: Vec<&i64> = cells
                    .iter()
                    .zip(&cluster_ids)
                    .filter_map(|(cell, cluster)| (*cluster == id).then_some(cell))
                    .collect();
                if members.is_empty() {
                    return None;
                }
                let mut coverage = Coverage::new();
                for geocell in members {
                    coverage.add_cell((((*geocell as u64) >> 60) << 1) as i32, *geocell << 4);
                }
                Some(coverage)
            })
            .collect()
    }

    /// `toGeoCells`: the geocells of the resolutions up to `finest` (at most 30), sorted as
    /// signed longs.
    pub fn to_geocells(&self, finest: i32) -> Vec<i64> {
        let levels = (finest.min(30) >> 1).max(0) as usize;
        let mut geocells: Vec<i64> = self
            .cells
            .iter()
            .enumerate()
            .take(levels)
            .flat_map(|(i, hhcodes)| {
                hhcodes.iter().map(move |hhcode| {
                    (((i + 1) as i64) << 60) | ((hhcode >> 4) & 0x0fff_ffff_ffff_ffff)
                })
            })
            .collect();
        geocells.sort_unstable();
        geocells
    }
}

/// The top level cells (the digit of the resolution 2) a neighbor to the north, east, south
/// or west must not be in, unless the cell is, for it to be a neighbor: it wrapped around
/// the planisphere.
const WRAPPING_TOP_CELLS: [[u64; 4]; 4] = [
    [0, 1, 4, 5],
    [0xA, 8, 2, 0],
    [0xA, 0xB, 0xE, 0xF],
    [0xF, 0xD, 7, 5],
];

/// The cells `cell` may touch, in the order `CoverageHelper.clusters` looks for them: its
/// north, east, south and west neighbors at its own resolution (the ones that wrapped around
/// are `0`), then the cells of each coarser resolution (from the coarsest) holding those.
fn neighbors(cell: i64) -> Vec<i64> {
    const TOP_CELL: u64 = 0x0F00_0000_0000_0000;
    const RESOLUTION: u64 = 0xF000_0000_0000_0000;
    let resolution = ((cell as u64) >> 60) as i32;
    let hhcode = cell << 4;
    let deltas = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    let own: Vec<i64> = deltas
        .iter()
        .zip(&WRAPPING_TOP_CELLS)
        .map(|((delta_lat, delta_lon), wrapping)| {
            let neighbor = (cell as u64 & RESOLUTION)
                | (delta_hhcode(hhcode, resolution << 1, *delta_lat, *delta_lon) as u64 >> 4);
            let top = (neighbor & TOP_CELL) >> 56;
            if wrapping.contains(&top) && neighbor & TOP_CELL != cell as u64 & TOP_CELL {
                0
            } else {
                neighbor as i64
            }
        })
        .collect();
    let coarser = (1..resolution).flat_map(|level| {
        let shift = 60 - 4 * level;
        own.iter().map(move |neighbor| {
            // The cells that a neighbor that wrapped (0) is replaced by are those of the
            // first HHCode, as they are in Warp 10.
            ((((*neighbor as u64) >> shift) << shift) & 0x0FFF_FFFF_FFFF_FFFF) as i64
                | (i64::from(level) << 60)
        })
    });
    own.iter().copied().chain(coarser).collect()
}
