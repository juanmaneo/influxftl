// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The seasonal analysis of bucketized series: the cycle sub-series (`GTSHelper.subCycleSerie`)
//! the seasonal outlier tests and `STL` split a series into.

use crate::error::series::SeriesError;
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, java_binary_search_range};

/// Java's `int` division: truncated, and a failure on a zero divisor.
pub(crate) fn int_div(dividend: i32, divisor: i32) -> Result<i32> {
    if divisor == 0 {
        return Err(Error::division_by_zero());
    }
    Ok(dividend.wrapping_div(divisor))
}

/// `GTSHelper.subCycleSerie` (overwriting): the datapoints of a bucketized series that are
/// `buckets_per_period` buckets apart, from the bucket `lastbucket` back. The result is
/// bucketized with a span of a period; its datapoints come from the most recent to the oldest.
///
/// # Errors
///
/// A series that is not bucketized, a `lastbucket` that is not one of its buckets, or no
/// bucket per period.
pub(crate) fn sub_cycle_serie(
    gts: &mut GeoTimeSerie,
    lastbucket: i64,
    buckets_per_period: i32,
) -> Result<GeoTimeSerie> {
    if !gts.is_bucketized() {
        return Err(SeriesError::NotBucketized.into());
    }
    let distance = gts.lastbucket.wrapping_sub(lastbucket);
    if distance.wrapping_rem(gts.bucketspan) != 0 {
        return Err(SeriesError::CycleOffBucket.into());
    }
    // The buckets of the series that precede `lastbucket`, in the `int` Java computes them in.
    let skipped = distance.wrapping_div(gts.bucketspan) as i32;
    let remaining = (gts.bucketcount as i32)
        .wrapping_sub(skipped)
        .wrapping_sub(1);
    let count = int_div(remaining, buckets_per_period)?.wrapping_add(1);
    let mut cycle = GeoTimeSerie::with_buckets(
        lastbucket,
        i64::from(count),
        gts.bucketspan.wrapping_mul(i64::from(buckets_per_period)),
    );
    if gts.size() == 0 || !cycle.is_bucketized() {
        return Ok(cycle);
    }
    gts.sort(false);
    // The ticks only go down: what is left to search shrinks.
    let mut limit = gts.size() as i64;
    for bucket in 0..cycle.bucketcount {
        let tick = cycle
            .lastbucket
            .wrapping_sub(bucket.wrapping_mul(cycle.bucketspan));
        let found = java_binary_search_range(&gts.ticks, 0, limit, tick);
        if let Ok(idx) = usize::try_from(found) {
            cycle.set_value(
                tick,
                gts.location_at(idx),
                gts.elevation_at(idx),
                &gts.value_at(idx),
                true,
            );
            limit = found;
        }
    }
    Ok(cycle)
}
