// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Positions in a collection: the index test of Warp 10, done once and without a conversion
//! error to hide.

/// The position of `index` in a collection of `len` items: `None` when `index` is negative or not
/// below `len`. Warp 10 tests `index < 0 || index >= size` in `long` arithmetic, which is what this
/// does; the cast is exact once the test has passed.
pub fn position(index: i64, len: usize) -> Option<usize> {
    (index >= 0 && (index as u64) < len as u64).then_some(index as usize)
}
