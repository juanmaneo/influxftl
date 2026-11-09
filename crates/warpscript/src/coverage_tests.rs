// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from GeoXP Lib (Copyright 1999-2019 Mathias Herberts, 2019-2020 iroise.net S.A.S.,
// 2020- SenX S.A.S., Apache-2.0), ported to Rust and modified.

use crate::coverage::Coverage;

/// The geocell of resolution 2 holding the HHCode `0x1100_0000_0000_0000`.
const PARENT: i64 = 0x1100_0000_0000_0000;

#[test]
fn nothing_is_optimized_at_resolution_2() {
    let mut coverage = Coverage::new();
    coverage.set_auto_thresholds(2);
    coverage.add_cell(2, 0x1000_0000_0000_0000);
    coverage.add_cell(2, 0x2000_0000_0000_0000);
    assert_eq!(
        coverage.to_geocells(32),
        vec![0x1100_0000_0000_0000, 0x1200_0000_0000_0000]
    );
}

#[test]
fn enough_children_are_replaced_by_their_parent() {
    let mut coverage = Coverage::new();
    // Two children are enough at resolution 4.
    coverage.set_auto_thresholds(2 << 56);
    coverage.add_cell(4, 0x1000_0000_0000_0000);
    assert_eq!(coverage.to_geocells(32).len(), 1);
    coverage.add_cell(4, 0x1100_0000_0000_0000);
    assert_eq!(coverage.to_geocells(32), vec![PARENT]);
}

#[test]
fn cells_of_other_parents_are_not_enough() {
    let mut coverage = Coverage::new();
    coverage.set_auto_thresholds(2 << 56);
    coverage.add_cell(4, 0x1000_0000_0000_0000);
    // As many cells as the threshold, but not under the same parent.
    coverage.add_cell(4, 0x2000_0000_0000_0000);
    assert_eq!(coverage.to_geocells(32).len(), 2);
}

#[test]
fn a_zero_threshold_means_sixteen_children() {
    let mut coverage = Coverage::new();
    coverage.set_auto_thresholds(0);
    for offset in 0..15 {
        coverage.add_cell(4, 0x1000_0000_0000_0000 | (offset << 56));
    }
    assert_eq!(coverage.to_geocells(32).len(), 15);
    coverage.add_cell(4, 0x1000_0000_0000_0000 | (15 << 56));
    assert_eq!(coverage.to_geocells(32).len(), 1);
}

#[test]
fn deduplication_skips_cells_under_a_cell_already_there() {
    let mut coverage = Coverage::new();
    coverage.set_auto_dedup(true);
    coverage.add_cell(2, 0x1000_0000_0000_0000);
    coverage.add_cell(4, 0x1100_0000_0000_0000);
    coverage.add_cell(34, 0x1000_0000_0000_0000);
    assert_eq!(
        coverage.to_geocells(32),
        vec![1 << 60 | 0x0100_0000_0000_0000]
    );
}
