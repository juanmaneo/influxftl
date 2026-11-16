// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The seasonal analysis of bucketized series: the cycle sub-series (`GTSHelper.subCycleSerie`)
//! the seasonal outlier tests split a series into, and the Seasonal-Trend decomposition based
//! on Loess (`GTSHelper.stl`, <http://www.wessa.net/download/stl.pdf>).
//!
//! `GTSHelper.stl` works on the arrays of Geo Time Series, larger than the series they hold,
//! and shares them between the steps (the low-pass filtering borrows the arrays of the trend,
//! the residuals those of the robustness weights). A series with missing values depends on
//! this, so the decomposition is ported onto explicit arrays ([`Track`]) that behave the same.

use std::num::NonZeroI32;

use crate::error::fault::{BoundsError, ResourceError};
use crate::error::series::SeriesError;
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Type, Values, binary_search_range};
use crate::lowess::{Grid, Lowess, Samples, pointwise_lowess, smooth};

/// Java's `int` division: truncated, and a failure on a zero divisor.
pub(crate) fn int_div(dividend: i32, divisor: i32) -> Result<i32> {
    if divisor == 0 {
        return Err(Error::division_by_zero());
    }
    Ok(dividend.wrapping_div(divisor))
}

/// The bucketization of a cycle sub-series: the buckets of the series that are a period
/// apart, from `lastbucket` back.
#[derive(Clone, Copy, Debug)]
struct CycleGrid {
    lastbucket: i64,
    bucketspan: i64,
    bucketcount: i64,
}

impl CycleGrid {
    /// The cycle that ends `skipped` buckets before the last bucket of the series.
    fn behind(series: Grid, skipped: i32, period: NonZeroI32) -> CycleGrid {
        // The buckets that precede the end of the cycle, in the `int` Java computes them in.
        let remaining = (series.bucketcount as i32)
            .wrapping_sub(skipped)
            .wrapping_sub(1);
        CycleGrid {
            lastbucket: series
                .lastbucket
                .wrapping_sub(i64::from(skipped).wrapping_mul(series.bucketspan)),
            bucketspan: series.bucketspan.wrapping_mul(i64::from(period.get())),
            bucketcount: i64::from(remaining.wrapping_div(period.get()).wrapping_add(1)),
        }
    }

    /// The cycle of a bucketized series that ends at its bucket `lastbucket`.
    fn of(series: Grid, lastbucket: i64, buckets_per_period: i32) -> Result<CycleGrid> {
        let distance = series.lastbucket.wrapping_sub(lastbucket);
        if distance.wrapping_rem(series.bucketspan) != 0 {
            return Err(SeriesError::CycleOffBucket.into());
        }
        let skipped = distance.wrapping_div(series.bucketspan) as i32;
        let period = NonZeroI32::new(buckets_per_period).ok_or_else(Error::division_by_zero)?;
        Ok(CycleGrid {
            lastbucket,
            ..CycleGrid::behind(series, skipped, period)
        })
    }

    /// Whether the cycle is a bucketized series.
    fn is_bucketized(&self) -> bool {
        self.bucketcount != 0 && self.bucketspan != 0 && self.lastbucket != 0
    }

    /// The indexes in `ticks` (in ascending order) of the datapoints at the buckets of the
    /// cycle, the most recent first. A bucket is searched below the previous datapoint only.
    fn hits(&self, ticks: &[i64]) -> Vec<usize> {
        if ticks.is_empty() || !self.is_bucketized() {
            return Vec::new();
        }
        let mut hits = Vec::new();
        let mut limit = ticks.len() as i64;
        for bucket in 0..self.bucketcount {
            let tick = self
                .lastbucket
                .wrapping_sub(bucket.wrapping_mul(self.bucketspan));
            let found = binary_search_range(ticks, 0, limit, tick);
            if let Ok(idx) = usize::try_from(found) {
                hits.push(idx);
                limit = found;
            }
        }
        hits
    }
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
    let grid = CycleGrid::of(
        Grid {
            lastbucket: gts.lastbucket,
            bucketspan: gts.bucketspan,
            bucketcount: gts.bucketcount,
        },
        lastbucket,
        buckets_per_period,
    )?;
    let mut cycle = GeoTimeSerie::with_buckets(grid.lastbucket, grid.bucketcount, grid.bucketspan);
    if gts.size() == 0 {
        return Ok(cycle);
    }
    gts.sort(false);
    for idx in grid.hits(&gts.ticks) {
        cycle.set_value(
            gts.tick_at(idx),
            gts.location_at(idx),
            gts.elevation_at(idx),
            &gts.value_at(idx),
            true,
        );
    }
    Ok(cycle)
}

/// The most buckets (and periods) a decomposition allocates for: beyond, Java runs out of
/// memory.
const MAX_BUCKETS: i64 = 1 << 25;

/// A series held in arrays larger than itself, like the arrays of a `GeoTimeSerie`: it has
/// `size` datapoints, the first of its arrays; the rest of an array is spare room (or what a
/// shorter series left there).
#[derive(Clone, Debug)]
struct Track {
    ticks: Vec<i64>,
    values: Vec<f64>,
    /// `GeoTimeSerie.values`: the number of datapoints.
    size: usize,
    lastbucket: i64,
    bucketspan: i64,
    bucketcount: i64,
    /// Whether the datapoints are known to be in ascending order of ticks.
    sorted: bool,
}

impl Track {
    /// A series of the given ticks and zeros for values, in arrays of `capacity` (of the
    /// bucketization `grid`).
    fn new(ticks: &[i64], capacity: usize, grid: Grid) -> Track {
        let mut padded = vec![0; capacity];
        padded[..ticks.len()].copy_from_slice(ticks);
        Track {
            ticks: padded,
            values: vec![0.0; capacity],
            size: ticks.len(),
            lastbucket: grid.lastbucket,
            bucketspan: grid.bucketspan,
            bucketcount: grid.bucketcount,
            sorted: false,
        }
    }

    fn grid(&self) -> Grid {
        Grid {
            lastbucket: self.lastbucket,
            bucketspan: self.bucketspan,
            bucketcount: self.bucketcount,
        }
    }

    /// The datapoints, as the samples of a regression.
    fn samples(&self) -> Samples<'_> {
        Samples {
            ticks: &self.ticks[..self.size],
            values: &self.values[..self.size],
        }
    }

    /// `GTSHelper.sort`: the datapoints in ascending order of ticks.
    fn sort(&mut self) {
        if self.sorted {
            return;
        }
        let mut order: Vec<usize> = (0..self.size).collect();
        order.sort_by_key(|&idx| self.ticks[idx]);
        let ticks: Vec<i64> = order.iter().map(|&idx| self.ticks[idx]).collect();
        let values: Vec<f64> = order.iter().map(|&idx| self.values[idx]).collect();
        self.ticks[..self.size].copy_from_slice(&ticks);
        self.values[..self.size].copy_from_slice(&values);
        self.sorted = true;
    }

    /// `GTSHelper.valueAtTick`: the value at `tick`, once the series is sorted.
    fn value_at_tick(&mut self, tick: i64) -> Option<f64> {
        if self.size == 0 {
            return None;
        }
        self.sort();
        let found = binary_search_range(&self.ticks, 0, self.size as i64, tick);
        usize::try_from(found).ok().map(|idx| self.values[idx])
    }

    /// `GTSHelper.setValue` with `overwrite`: the value of a tick, which is added when the
    /// series has no datapoint at it.
    fn set_value(&mut self, tick: i64, value: f64) {
        let ticks = &self.ticks[..self.size];
        let found = if self.sorted {
            // The first of the datapoints at the tick.
            let first = ticks.partition_point(|&t| t < tick);
            (ticks.get(first) == Some(&tick)).then_some(first)
        } else {
            ticks.iter().position(|&t| t == tick)
        };
        let idx = found.unwrap_or(self.size);
        if idx == self.size {
            // The flag survives the datapoints that extend the order.
            let ascending = self.sorted || self.size == 1;
            self.sorted = ticks.last().is_none_or(|&last| ascending && last <= tick);
            if idx == self.ticks.len() {
                self.ticks.push(tick);
                self.values.push(value);
            }
            self.size += 1;
        }
        self.ticks[idx] = tick;
        self.values[idx] = value;
    }

    /// `GTSHelper.firsttick` of a bucketized series: its first bucket.
    fn first_tick(&self) -> i64 {
        self.lastbucket.wrapping_sub(
            self.bucketcount
                .wrapping_sub(1)
                .wrapping_mul(self.bucketspan),
        )
    }

    /// The array a regression returns (`Math.max(sizehint, 1.1 * size)` doubles) in place of
    /// the values.
    fn install(&mut self, smoothed: Vec<f64>) {
        let capacity = self
            .values
            .len()
            .max(smoothed.len() + smoothed.len() / 10 + 1);
        self.values = smoothed;
        self.values.resize(capacity, 0.0);
    }

    /// The Geo Time Series of the datapoints.
    fn into_series(self, template: &GeoTimeSerie, name: &str) -> GeoTimeSerie {
        let mut series = template.clone_empty();
        let prefix = match template.name() {
            Some(name) if !name.is_empty() => format!("{name}_"),
            _ => String::new(),
        };
        series.metadata.name = Some(format!("{prefix}{name}"));
        series.lastbucket = self.lastbucket;
        series.bucketspan = self.bucketspan;
        series.bucketcount = self.bucketcount;
        let mut ticks = self.ticks;
        ticks.truncate(self.size);
        let mut values = self.values;
        values.truncate(self.size);
        series.ticks = ticks;
        series.values = Values::Double(values);
        series
    }
}

/// A cycle sub-series, in ascending order of ticks, to smooth.
#[derive(Clone, Debug)]
struct Cycle {
    ticks: Vec<i64>,
    values: Vec<f64>,
    lastbucket: i64,
    bucketspan: i64,
    bucketcount: i64,
}

impl Cycle {
    /// The cycle of a series that ends `skipped` buckets before its last bucket, a period apart,
    /// extended by a bucket at both ends; the datapoints are read at the indexes of its sorted
    /// `ticks` from `values` (those of the series, or the robustness weights).
    fn behind(
        ticks: &[i64],
        values: &[f64],
        grid: Grid,
        skipped: i32,
        period: NonZeroI32,
    ) -> Cycle {
        let cycle = CycleGrid::behind(grid, skipped, period);
        let mut hits = cycle.hits(ticks);
        // The datapoints come the most recent first: they are smoothed the other way.
        hits.reverse();
        Cycle {
            ticks: hits.iter().map(|&idx| ticks[idx]).collect(),
            values: hits.iter().map(|&idx| values[idx]).collect(),
            lastbucket: cycle.lastbucket.wrapping_add(cycle.bucketspan),
            bucketspan: cycle.bucketspan,
            bucketcount: cycle.bucketcount.wrapping_add(2),
        }
    }

    fn is_bucketized(&self) -> bool {
        self.bucketcount != 0 && self.bucketspan != 0 && self.lastbucket != 0
    }
}

/// The robustness weights of the cycles of a loop, in the array Java reuses from a cycle to the
/// next one: what a cycle leaves past its own datapoints stays there for the next cycle to
/// read, and the array only exists once a cycle has a datapoint.
#[derive(Debug)]
struct CycleWeights {
    /// The array, once allocated.
    array: Option<Vec<f64>>,
    /// The size Java gives it at first.
    hint: usize,
}

impl CycleWeights {
    fn new(hint: usize) -> CycleWeights {
        CycleWeights { array: None, hint }
    }

    /// Store the weights of a cycle, the most recent datapoint first, over those of the
    /// previous one. The array is large enough for the cycles of the series: Java's growth
    /// when it is not never happens, and only the length it ends up with could tell.
    fn fill(&mut self, weights: impl Iterator<Item = f64>) {
        for (position, weight) in weights.enumerate() {
            let array = self.array.get_or_insert_with(|| vec![0.0; self.hint]);
            array.resize(array.len().max(position + 1), 0.0);
            array[position] = weight;
        }
    }
}

/// `GTSHelper.lowess_stl`: smooth a cycle sub-series, and set the result at each of its
/// buckets in `to`; a `neighbours` below zero sets the mean of the cycle instead. Only one
/// bucket in `jump + 1` is estimated, the others are interpolated.
fn lowess_stl(
    from: &Cycle,
    to: &mut Track,
    neighbours: i32,
    degree: i32,
    jump: i32,
    rho: Option<&[f64]>,
) -> Result<()> {
    if !from.is_bucketized() {
        return Err(SeriesError::StlCycleNotBucketized.into());
    }
    let bucket_tick = |bucket: i64| {
        from.lastbucket
            .wrapping_sub(bucket.wrapping_mul(from.bucketspan))
    };
    if neighbours < 0 {
        let sum = from.values.iter().fold(0.0, |sum, value| sum + value);
        let mean = sum / from.values.len() as f64;
        for bucket in 0..from.bucketcount {
            to.set_value(bucket_tick(bucket), mean);
        }
        return Ok(());
    }
    let samples = Samples {
        ticks: &from.ticks,
        values: &from.values,
    };
    // Estimate from the most recent bucket back, a bucket in `step`, ending at the oldest.
    let step = i64::from(jump) + 1;
    let intervals = from.bucketcount.wrapping_sub(1);
    let rest = intervals.wrapping_rem(step);
    let count = intervals.wrapping_div(step);
    let mut ticks: Vec<i64> = (0..=count)
        .map(|j| bucket_tick(j.wrapping_mul(step).wrapping_add(rest)))
        .collect();
    // The most recent buckets, which the steps from the oldest one skipped, are estimated last.
    if rest != 0 {
        ticks.push(from.lastbucket);
    }
    let estimates = ticks
        .iter()
        .map(|&tick| {
            // The last datapoint at or before the tick.
            let before = from.ticks.partition_point(|&t| t <= tick);
            pointwise_lowess(
                samples,
                before as i64 - 1,
                tick,
                i64::from(neighbours),
                degree,
                rho,
                true,
            )
        })
        .collect::<Result<Vec<f64>>>()?;
    for (&tick, &estimated) in ticks.iter().zip(&estimates).take(count as usize + 1) {
        to.set_value(tick, estimated);
    }
    // Interpolate the buckets skipped between two estimates.
    for j in 0..count {
        let right = j.wrapping_mul(step).wrapping_add(rest);
        let left = (j + 1).wrapping_mul(step).wrapping_add(rest);
        let denom = (left - right) as f64;
        let (at_right, at_left) = (estimates[j as usize], estimates[j as usize + 1]);
        for r in 1..step {
            let middle = r.wrapping_add(j.wrapping_mul(step)).wrapping_add(rest);
            let alpha = (middle - right) as f64 / denom;
            let interpolated = alpha * at_left + (1.0 - alpha) * at_right;
            to.set_value(bucket_tick(middle), interpolated);
        }
    }
    if rest != 0 {
        let estimated = estimates[count as usize + 1];
        to.set_value(from.lastbucket, estimated);
        let denom = rest as f64;
        let at_left = estimates[0];
        for r in 1..rest {
            let alpha = r as f64 / denom;
            let interpolated = alpha * at_left + (1.0 - alpha) * estimated;
            to.set_value(bucket_tick(r), interpolated);
        }
    }
    Ok(())
}

/// The parameters of a decomposition: the length of a period, the number of inner and outer
/// loops, and the bandwidth (`n`), degree (`d`) and jump (`j`) of the regressions of the
/// seasonal (`s`), low-pass (`l`), trend (`t`) and post-seasonal (`p`) smoothings.
#[derive(Clone, Copy, Debug)]
pub struct Stl {
    /// The number of buckets of a period.
    pub period: i32,
    /// The number of inner loops (the precision).
    pub inner: i32,
    /// The number of outer loops (the robustness).
    pub outer: i32,
    /// The bandwidth of the seasonal smoothing (negative: the mean of the cycle).
    pub ns: i32,
    /// The degree of the seasonal smoothing.
    pub ds: i32,
    /// The jump of the seasonal smoothing.
    pub js: i32,
    /// The bandwidth of the low-pass filtering.
    pub nl: i32,
    /// The degree of the low-pass filtering.
    pub dl: i32,
    /// The jump of the low-pass filtering.
    pub jl: i32,
    /// The bandwidth of the trend smoothing.
    pub nt: i32,
    /// The degree of the trend smoothing.
    pub dt: i32,
    /// The jump of the trend smoothing.
    pub jt: i32,
    /// The bandwidth of the post seasonal smoothing (none when `0`).
    pub np: i32,
    /// The degree of the post seasonal smoothing.
    pub dp: i32,
    /// The jump of the post seasonal smoothing.
    pub jp: i32,
}

/// `Arrays.binarySearch` of a datapoint that has to exist (the index into an array of
/// `length` Java reads at).
fn found(index: i64, length: usize) -> Result<usize> {
    usize::try_from(index).map_err(|_| Error::index_out_of_bounds(index, length))
}

impl Stl {
    /// `GTSHelper.stl`: the seasonal and the trend components of a numeric, bucketized series
    /// (sorted by the decomposition).
    ///
    /// # Errors
    ///
    /// A series that is not numeric or not bucketized, with too many missing values, with
    /// locations or elevations (a `NullPointerException` in Warp 10), a decomposition that
    /// finds no value in a cycle, or a regression that fails.
    #[allow(clippy::too_many_lines)]
    pub fn decompose(&self, gts: &mut GeoTimeSerie) -> Result<(GeoTimeSerie, GeoTimeSerie)> {
        if !matches!(gts.type_of(), Type::Double | Type::Long) {
            return Err(SeriesError::StlNotNumeric.into());
        }
        if !gts.is_bucketized() {
            return Err(SeriesError::StlNotBucketized.into());
        }
        gts.sort(false);
        let nonnull = gts.size();
        let values = gts.values.to_doubles();
        let size = i64::from(gts.bucketcount as i32);
        if size - nonnull as i64 > 500_000 {
            return Err(SeriesError::LowessMissingValues.into());
        }
        let bpp = i64::from(self.period);
        let capacity = size + 2 * bpp;
        if capacity > MAX_BUCKETS {
            return Err(if capacity > i64::from(i32::MAX - 2) {
                ResourceError::ArrayTooLong
            } else {
                ResourceError::OutOfMemory
            }
            .into());
        }
        // The scratch array of the regressions of Java is as large as the widest bandwidth.
        let widest = self.ns.max(self.nl).max(self.nt);
        if widest > i32::MAX - 2 {
            return Err(ResourceError::ArrayTooLong.into());
        }
        let capacity = capacity as usize;
        // Java truncates the ticks to the room it has, then reads past it.
        if capacity < nonnull {
            return Err(BoundsError::ArrayRange(nonnull).into());
        }
        let grid = Grid {
            lastbucket: gts.lastbucket,
            bucketspan: gts.bucketspan,
            bucketcount: gts.bucketcount,
        };
        let span = grid.bucketspan;
        let mut seasonal = Track::new(&gts.ticks, capacity, grid);
        let mut trend = Track::new(&gts.ticks, capacity, grid);
        // The robustness weights, which the residuals of the outer loop reuse.
        let mut rho = vec![1.0; nonnull];
        let buckets_per_period =
            NonZeroI32::new(self.period).ok_or_else(Error::division_by_zero)?;
        let period = i64::from(self.period);
        // `new GeoTimeSerie(…, (int) Math.max(1.4 * bucketcount, sizehint) / buckets_per_period)`.
        let hint = ((1.4 * size as f64).max(capacity as f64) as i32)
            .wrapping_div(buckets_per_period.get());
        let hint = hint.max(0) as usize;

        for s in 0..=i64::from(self.outer) {
            for _ in 0..self.inner {
                // Step 1: detrending.
                let mut idx_t = 0;
                for (idx, (&value, &tick)) in values.iter().zip(&gts.ticks).enumerate() {
                    idx_t = binary_search_range(&trend.ticks, idx_t, trend.size as i64, tick);
                    seasonal.values[idx] = value - trend.values[found(idx_t, trend.values.len())?];
                }

                // Step 2: cycle-subseries smoothing.
                let mut weights = CycleWeights::new(hint);
                for c in 0..period {
                    seasonal.sort();
                    let skipped = c as i32;
                    let cycle = Cycle::behind(
                        &seasonal.ticks[..seasonal.size],
                        &seasonal.values[..seasonal.size],
                        seasonal.grid(),
                        skipped,
                        buckets_per_period,
                    );
                    // The weights of the same cycle, read over the first `nonnull` ticks. Warp 10
                    // leaves them from the most recent to the oldest, where the cycle is smoothed
                    // from the oldest to the most recent one.
                    if s > 0 {
                        let grid = CycleGrid::behind(seasonal.grid(), skipped, buckets_per_period);
                        let hits = grid.hits(&seasonal.ticks[..nonnull]);
                        weights.fill(hits.iter().map(|&idx| rho[idx]));
                    }
                    if !cycle.ticks.is_empty() {
                        let cycle_rho = if s > 0 {
                            weights.array.as_deref()
                        } else {
                            Some(&rho[..])
                        };
                        lowess_stl(&cycle, &mut seasonal, self.ns, self.ds, self.js, cycle_rho)?;
                    }
                }
                // The seasonal series now holds a period before and after the series.
                let widening = period.wrapping_mul(span);
                seasonal.lastbucket = seasonal.lastbucket.wrapping_add(widening);
                seasonal.bucketcount = seasonal.bucketcount.wrapping_add(2 * period);

                // Step 3: low-pass filtering of the smoothed cycle-subseries, in the arrays
                // of the trend.
                seasonal.sort();
                let firstbucket = seasonal.first_tick();
                let mut sum = 0.0;
                let mut count = 0_i64;
                for r in 0..period {
                    if let Some(value) =
                        seasonal.value_at_tick(firstbucket.wrapping_add(r.wrapping_mul(span)))
                    {
                        count += 1;
                        sum += value;
                    }
                }
                // 1st moving average, of the width of a period: the sum and the number of the
                // values of each window of the seasonal series, as the window slides.
                let mut windows = vec![(sum, count)];
                let last_average = seasonal.bucketcount - period + 1;
                for r in 1..last_average.max(1) {
                    let first = seasonal
                        .value_at_tick(firstbucket.wrapping_add((r - 1).wrapping_mul(span)));
                    let next = seasonal.value_at_tick(
                        firstbucket.wrapping_add((r + period - 1).wrapping_mul(span)),
                    );
                    match (first, next) {
                        (None, None) => {}
                        (None, Some(next)) => {
                            count += 1;
                            sum += next;
                        }
                        (Some(first), Some(next)) => sum += next - first,
                        (Some(first), None) => {
                            count -= 1;
                            sum -= first;
                        }
                    }
                    windows.push((sum, count));
                }
                // Warp 10 stops at a window without a value (`3.0` for the first one).
                let averages = windows
                    .iter()
                    .enumerate()
                    .map(|(r, &(sum, count))| {
                        if count == 0 {
                            Err(
                                SeriesError::StlNoValue(format!("3.{}", usize::from(r != 0)))
                                    .into(),
                            )
                        } else {
                            Ok(sum / count as f64)
                        }
                    })
                    .collect::<Result<Vec<f64>>>()?;
                trend.values[..averages.len()].copy_from_slice(&averages);
                // 2nd moving average, of the width of a period.
                let width = period as usize;
                let mut sum = trend.values[..width].iter().fold(0.0, |sum, v| sum + v);
                let mut previous = trend.values[0];
                trend.values[0] = sum / period as f64;
                for r in 1..=(seasonal.bucketcount - 2 * period + 1) as usize {
                    sum += trend.values[r + width - 1] - previous;
                    previous = trend.values[r];
                    trend.values[r] = sum / period as f64;
                }
                // 3rd moving average, of the width of 3.
                for r in 0..(seasonal.bucketcount - 2 * period).max(0) as usize {
                    trend.values[r] += trend.values[r + 1] + trend.values[r + 2];
                    trend.values[r] /= 3.0;
                }
                // The trend borrows the arrays: it now has the buckets of the series.
                trend.bucketcount = seasonal.bucketcount - 2 * period;
                trend.lastbucket = seasonal.lastbucket.wrapping_sub(widening);
                trend.size = trend.bucketcount as usize;
                for i in 0..trend.size {
                    trend.ticks[i] = trend.lastbucket.wrapping_sub(
                        (trend.bucketcount - 1 - i as i64).wrapping_mul(trend.bucketspan),
                    );
                }
                trend.regress(
                    Lowess {
                        q: self.nl,
                        r: 0,
                        d: (i64::from(self.jl) + 1).wrapping_mul(trend.bucketspan),
                        p: self.dl,
                    },
                    None,
                )?;

                // Step 4: detrending of the smoothed cycle-subseries.
                seasonal.lastbucket = seasonal.lastbucket.wrapping_sub(widening);
                seasonal.bucketcount -= 2 * period;
                let mut id = 0;
                for r in 0..seasonal.bucketcount {
                    let tick = firstbucket.wrapping_add((r + period).wrapping_mul(span));
                    if let Some(value) = seasonal.value_at_tick(tick) {
                        let r = r as usize;
                        seasonal.values[id] = value - trend.values[r];
                        seasonal.ticks[id] = trend.ticks[r];
                        id += 1;
                    }
                }
                seasonal.size = id;

                // Step 5: deseasonalizing.
                let mut idx_s = 0;
                for (idx, (&value, &tick)) in values.iter().zip(&gts.ticks).enumerate() {
                    idx_s = binary_search_range(&seasonal.ticks, idx_s, seasonal.size as i64, tick);
                    trend.values[idx] =
                        value - seasonal.values[found(idx_s, seasonal.values.len())?];
                }
                trend.size = nonnull;
                trend.lastbucket = grid.lastbucket;
                trend.bucketspan = grid.bucketspan;
                trend.bucketcount = size;

                // Step 6: trend smoothing.
                trend.regress(
                    Lowess {
                        q: self.nt,
                        r: 0,
                        d: (i64::from(self.jt) + 1).wrapping_mul(trend.bucketspan),
                        p: self.dt,
                    },
                    Some(&rho),
                )?;
            }

            // Robustifying operations of the outer loop (but the last time).
            if s < i64::from(self.outer) {
                let mut idx_s = 0;
                let residuals: Vec<f64> = values
                    .iter()
                    .zip(&gts.ticks)
                    .filter_map(|(&value, &tick)| {
                        idx_s =
                            binary_search_range(&seasonal.ticks, idx_s, seasonal.size as i64, tick);
                        // The trend is assumed to hold its datapoints at the same indexes.
                        usize::try_from(idx_s).ok().map(|found| {
                            (value - seasonal.values[found] - trend.values[found]).abs()
                        })
                    })
                    .collect();
                rho[..residuals.len()].copy_from_slice(&residuals);
                let mut sorted = rho.clone();
                sorted.sort_by(f64::total_cmp);
                let half = nonnull / 2;
                let median = if nonnull.is_multiple_of(2) {
                    // Java's `(a + b) / 2`.
                    #[allow(clippy::manual_midpoint)]
                    let median = (sorted[half] + sorted[half - 1]) / 2.0;
                    median
                } else {
                    sorted[half]
                };
                // The bisquare weights of the residuals over 6 medians.
                let h = 6.0 * median;
                for weight in &mut rho {
                    *weight = if h == 0.0 {
                        1.0
                    } else {
                        let u = *weight / h;
                        if u >= 1.0 {
                            0.0
                        } else {
                            let v = 1.0 - u * u;
                            v * v
                        }
                    };
                }
            }
        }

        // Post seasonal smoothing.
        if self.np > 0 {
            seasonal.regress_into_new_series(Lowess {
                q: self.np,
                r: 0,
                d: (i64::from(self.jp) + 1).wrapping_mul(seasonal.bucketspan),
                p: self.dp,
            })?;
        }
        // Warp 10 gives locations and elevations to arrays that it never allocated.
        if gts.locations.is_some() || gts.elevations.is_some() {
            return Err(Error::null_pointer());
        }
        Ok((
            seasonal.into_series(gts, "seasonal"),
            trend.into_series(gts, "trend"),
        ))
    }
}

impl Track {
    /// `GTSHelper.rlowess` in place: the regression of the series replaces its values; a
    /// series with missing values then has a value per bucket.
    fn regress(&mut self, params: Lowess, rho: Option<&[f64]>) -> Result<()> {
        params.validate()?;
        self.sort();
        let grid = self.grid();
        let smoothed = smooth(self.samples(), Some(grid), self.bucketcount, params, rho)?;
        let filled = grid.is_filled_by(&self.ticks[..self.size]);
        self.install(smoothed.values);
        if !filled {
            self.size = smoothed.ticks.len();
        }
        Ok(())
    }

    /// `GTSHelper.rlowess` into a new series: the regression of the series, at each of its
    /// ticks (each of its buckets when it is bucketized), takes the place of its datapoints.
    fn regress_into_new_series(&mut self, params: Lowess) -> Result<()> {
        params.validate()?;
        self.sort();
        let smoothed = smooth(
            self.samples(),
            Some(self.grid()),
            self.bucketcount,
            params,
            None,
        )?;
        self.size = smoothed.ticks.len();
        self.ticks = smoothed.ticks;
        self.values = smoothed.values;
        self.sorted = false;
        Ok(())
    }
}
