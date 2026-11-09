// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from GeoXP Lib (Copyright 1999-2019 Mathias Herberts, 2019-2020 iroise.net S.A.S.,
// 2020- SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Coverages: sets of HHCode cells, one set per resolution (`com.geoxp.geo.Coverage`).
//!
//! A cell of resolution `2 * (r + 1)` is stored as the HHCode of its first point, its lower
//! bits cleared. The Java sets are hash sets, but every iteration over them is either
//! followed by a sort or does not depend on the order, so ordered sets behave the same.

use std::collections::BTreeSet;

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

    /// `setAutoDedup`.
    pub fn set_auto_dedup(&mut self, auto_dedup: bool) {
        self.auto_dedup = auto_dedup;
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
