// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Locally weighted regression: `GTSHelper.pointwise_lowess` and the robust, fast
//! `GTSHelper.rlowess`, over the ticks and the values of a series held in slices. `LOWESS`,
//! `RLOWESS` and `STL` share them.

// The single letter names are those of the Java code and of the LOWESS literature.
#![allow(clippy::many_single_char_names)]

use crate::curvefit::{WeightedPoint, polynomial_fit};
use crate::error::series::SeriesError;
use crate::error::{Error, Result};
use crate::gts::binary_search_range;

/// The datapoints of a series: its ticks, in ascending order, and its values.
#[derive(Clone, Copy, Debug)]
pub struct Samples<'a> {
    /// The ticks.
    pub ticks: &'a [i64],
    /// The values, one per tick.
    pub values: &'a [f64],
}

impl Samples<'_> {
    /// `GTSHelper.tickAtIndex`: `Long.MIN_VALUE` when out of range.
    fn tick_at(&self, idx: i64) -> i64 {
        // A negative index wraps around to one that is out of range as well.
        self.ticks.get(idx as usize).copied().unwrap_or(i64::MIN)
    }
}

/// The buckets of a bucketized series.
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    /// The last bucket.
    pub lastbucket: i64,
    /// The span of a bucket.
    pub bucketspan: i64,
    /// The number of buckets.
    pub bucketcount: i64,
}

impl Grid {
    /// The ticks of the buckets, the oldest first (`tickIterator`).
    pub fn ticks(&self) -> Vec<i64> {
        (0..self.bucketcount)
            .rev()
            .map(|bucket| {
                self.lastbucket
                    .wrapping_sub(bucket.wrapping_mul(self.bucketspan))
            })
            .collect()
    }

    /// Whether datapoints at `ticks` fill every bucket, from the first to the last.
    pub fn is_filled_by(&self, ticks: &[i64]) -> bool {
        match (ticks.first(), ticks.last()) {
            (Some(&first), Some(&last)) => {
                ticks.len() as i64 == self.bucketcount
                    && self.lastbucket == last
                    && self
                        .lastbucket
                        .wrapping_sub(self.bucketspan.wrapping_mul(self.bucketcount - 1))
                        == first
            }
            _ => false,
        }
    }
}

/// `Math.abs(a - b)` on `long`s, as a `double`.
fn distance(a: i64, b: i64) -> f64 {
    a.wrapping_sub(b).wrapping_abs() as f64
}

/// `GTSHelper.pointwise_lowess`: the local weighted regression of degree `p` at `tick` over
/// the `q` datapoints closest to it, `idx` being the first one at or after `tick` (or, when
/// `reversed`, the last one at or before it). `rho` holds the robustness weights.
pub fn pointwise_lowess(
    samples: Samples<'_>,
    idx: i64,
    tick: i64,
    q: i64,
    p: i32,
    rho: Option<&[f64]>,
    reversed: bool,
) -> Result<f64> {
    if rho.is_some_and(|rho| rho.len() < samples.ticks.len()) {
        return Err(SeriesError::IncoherentLowessArrays.into());
    }
    let n = samples.ticks.len() as i64;
    let tick_at = |k: i64| samples.tick_at(k);
    // The `q` nearest datapoints lie strictly between `j` and `i`.
    let mut i = idx;
    let mut j = idx - 1;
    if reversed {
        i += 1;
        j += 1;
    }
    let mut count = 0;
    while count < q {
        let idist = if i < n {
            tick_at(i).wrapping_sub(tick).wrapping_abs()
        } else {
            i64::MAX
        };
        let jdist = if j >= 0 {
            tick_at(j).wrapping_sub(tick).wrapping_abs()
        } else {
            i64::MAX
        };
        if idist == i64::MAX && jdist == i64::MAX {
            break;
        }
        if idist < jdist {
            i += 1;
        } else {
            j -= 1;
        }
        count += 1;
    }
    // `j` never goes below `-1` (Warp 10 guards against it all the same).
    let jmax = distance(tick_at(j + 1), tick);
    let imax = if i <= 0 {
        0.0
    } else {
        distance(tick_at(i - 1), tick)
    };
    let mut maxdist = crate::jmath::max(jmax, imax);
    if q > n {
        maxdist = (maxdist * q as f64) / n as f64;
    }
    let range = (j + 1) as usize..i as usize;
    let mut weights = Vec::with_capacity(range.len());
    let mut wsum = 0.0;
    for k in range.clone() {
        let weight = if maxdist == 0.0 {
            1.0
        } else {
            let u = distance(samples.ticks[k], tick) / maxdist;
            if u >= 1.0 {
                0.0
            } else {
                let w = 1.0 - u * u * u;
                // All robustness weights at zero must still allow a regression.
                let rho = rho.map_or(1.0, |rho| if rho[k] == 0.0 { 0.000_001 } else { rho[k] });
                rho * w * w * w
            }
        };
        weights.push(weight);
        wsum += weight;
    }
    let beta = if p == 1 {
        let mut ctick = 0.0;
        let mut cvalue = 0.0;
        for (k, w) in range.clone().zip(&weights) {
            ctick += w * samples.ticks[k] as f64;
            cvalue += w * samples.values[k];
        }
        ctick /= wsum;
        cvalue /= wsum;
        let mut covar = 0.0;
        let mut var = 0.0;
        for (k, w) in range.zip(&weights) {
            let dt = samples.ticks[k] as f64 - ctick;
            covar += w * dt * (samples.values[k] - cvalue);
            var += w * dt * dt;
        }
        covar /= wsum;
        var /= wsum;
        let beta1 = if var == 0.0 { 0.0 } else { covar / var };
        vec![cvalue - ctick * beta1, beta1]
    } else {
        let points: Vec<WeightedPoint> = range
            .zip(&weights)
            .map(|(k, &weight)| WeightedPoint {
                weight,
                x: samples.ticks[k] as f64,
                y: samples.values[k],
            })
            .collect();
        polynomial_fit(&points, p as usize + 1, i32::MAX)?
    };
    let mut estimated = beta[0];
    let mut tmp = 1.0;
    for coefficient in &beta[1..] {
        tmp *= tick as f64;
        estimated += tmp * coefficient;
    }
    Ok(estimated)
}

/// The parameters of `GTSHelper.rlowess`.
#[derive(Clone, Copy, Debug)]
pub struct Lowess {
    /// The bandwidth: the number of nearest neighbours.
    pub q: i32,
    /// The number of robustifying iterations.
    pub r: i32,
    /// The radius (in time units) within which the regression is interpolated.
    pub d: i64,
    /// The degree of the local polynomials.
    pub p: i32,
}

impl Lowess {
    /// The checks `GTSHelper.rlowess` makes on its parameters (Warp 10 also rejects a negative
    /// radius, which no caller passes).
    ///
    /// # Errors
    ///
    /// A bandwidth below one, a negative robustness, a degree below one or above nine.
    pub fn validate(&self) -> Result<()> {
        if self.q < 1 {
            return Err(SeriesError::LowessBandwidth.into());
        }
        if self.r < 0 {
            return Err(SeriesError::LowessRobustness.into());
        }
        if self.p < 1 {
            return Err(SeriesError::LowessDegreeTooSmall.into());
        }
        if self.p > 9 {
            return Err(SeriesError::LowessDegreeTooLarge.into());
        }
        Ok(())
    }
}

/// The estimates of `GTSHelper.rlowess`: one per tick of the series, or one per bucket when it
/// is bucketized.
#[derive(Clone, Debug)]
pub struct Smoothed {
    /// The ticks that were estimated.
    pub ticks: Vec<i64>,
    /// The estimates.
    pub values: Vec<f64>,
}

/// `GTSHelper.rlowess` from the checks of the series on (it has to be sorted): the robust locally
/// weighted regression of a series, at each of its ticks (each of its buckets when it has a
/// `grid`). `bucketcount` is the number of buckets of the series, bucketized or not, which the
/// check of the missing values compares to its datapoints. `rho` holds the initial robustness
/// weights (all `1` when absent).
///
/// # Errors
///
/// A series with too many missing values or with duplicate ticks, or a polynomial fit that
/// fails.
#[allow(clippy::too_many_lines)]
pub fn smooth(
    samples: Samples<'_>,
    grid: Option<Grid>,
    bucketcount: i64,
    params: Lowess,
    rho: Option<&[f64]>,
) -> Result<Smoothed> {
    if bucketcount - samples.ticks.len() as i64 > 500_000 {
        return Err(SeriesError::LowessMissingValues.into());
    }
    let mut previous = -1;
    for &tick in samples.ticks {
        if previous == tick {
            return Err(SeriesError::DuplicateTicks.into());
        }
        previous = tick;
    }
    let Lowess { q, r, d, p } = params;
    let n = samples.ticks.len();
    // The ticks to estimate (`tickIterator`): those of the series, or every bucket.
    let ticks: Vec<i64> = grid.map_or_else(|| samples.ticks.to_vec(), |grid| grid.ticks());
    let size = ticks.len();
    // Java's array is a little larger than the number of estimates.
    let mut smoothed = vec![0.0; size + size / 10 + 2];
    let nvalues = i64::from(q).min(size as i64);
    let mut robustness: Option<Vec<f64>> =
        (r > 0).then(|| rho.map_or_else(|| vec![1.0; n], <[f64]>::to_vec));
    let mut residual = vec![0.0; if r > 0 { n } else { 0 }];
    let last_index = n as i64 - 1;
    let tick_at = |k: i64| samples.tick_at(k);
    let mut r_iter = 0;
    while r_iter < r.wrapping_add(1) {
        let robust = r_iter < r;
        let rho_now = robustness.as_deref().or(rho);
        let mut it = 0usize;
        // The follower walks the ticks again to interpolate the skipped ones (`d > 0` only).
        let mut follower = (d != 0).then_some(0usize);
        let mut idx = 0i64;
        let mut ridx = 0usize;
        let mut ridx_last = 0usize;
        let mut last = d.wrapping_neg().wrapping_sub(1);
        let mut idx_last = 0i64;
        let mut last_skipped = 0i64;
        let mut skip = false;
        let mut resolved = false;
        let mut tick = 0i64;
        while it < size || resolved {
            if resolved {
                resolved = false;
            } else {
                tick = ticks[it];
                it += 1;
            }
            let has_next = it < size;
            if has_next && tick.wrapping_sub(last) <= d {
                // Close to the last estimate: skipped, interpolated later.
                last_skipped = tick;
                skip = true;
                ridx += 1;
            } else if !skip {
                while idx < last_index && tick > tick_at(idx) {
                    idx += 1;
                }
                smoothed[ridx] = pointwise_lowess(samples, idx, tick, nvalues, p, rho_now, false)?;
                if robust && tick == tick_at(idx) {
                    residual[idx as usize] = (samples.values[idx as usize] - smoothed[ridx]).abs();
                }
                if let Some(f) = follower.as_mut() {
                    *f += 1;
                    last = tick;
                    idx_last = idx;
                    ridx_last = ridx;
                }
                ridx += 1;
            } else {
                if !has_next && tick.wrapping_sub(last) <= d {
                    last_skipped = tick;
                    ridx += 1;
                }
                while idx < last_index && last_skipped > tick_at(idx) {
                    idx += 1;
                }
                smoothed[ridx - 1] =
                    pointwise_lowess(samples, idx, last_skipped, nvalues, p, rho_now, false)?;
                if robust && last_skipped == tick_at(idx) {
                    residual[idx as usize] =
                        (samples.values[idx as usize] - smoothed[ridx - 1]).abs();
                }
                // Linear interpolation of the skipped ticks.
                let denom = last_skipped.wrapping_sub(last) as f64;
                // Without a radius, Warp 10 has no follower to read the skipped ticks from.
                let Some(f) = follower.as_mut() else {
                    return Err(Error::null_pointer());
                };
                let mut skipped = ticks.get(*f).copied().unwrap_or_default();
                *f += 1;
                let mut ridx_s = ridx_last + 1;
                while last_skipped > skipped {
                    let alpha = skipped.wrapping_sub(last) as f64 / denom;
                    smoothed[ridx_s] =
                        alpha * smoothed[ridx - 1] + (1.0 - alpha) * smoothed[ridx_last];
                    if robust {
                        let sidx = binary_search_range(samples.ticks, idx_last, idx, skipped);
                        if sidx > 0 {
                            residual[sidx as usize] =
                                (samples.values[sidx as usize] - smoothed[ridx_s]).abs();
                        }
                    }
                    skipped = ticks.get(*f).copied().unwrap_or_default();
                    *f += 1;
                    ridx_s += 1;
                }
                if has_next || tick.wrapping_sub(last) > d {
                    skip = false;
                    resolved = true;
                    last = last_skipped;
                    idx_last = idx;
                    ridx_last = ridx - 1;
                }
            }
        }
        if let (true, Some(rho)) = (robust, robustness.as_mut()) {
            let mut sorted = residual.clone();
            sorted.sort_by(f64::total_cmp);
            let half = n / 2;
            if sorted.is_empty() {
                return Err(Error::index_out_of_bounds(0, 0));
            }
            let median = if n.is_multiple_of(2) {
                // Java's `(a + b) / 2`, which overflows where `midpoint` would not.
                #[allow(clippy::manual_midpoint)]
                let median = (sorted[half] + sorted[half - 1]) / 2.0;
                median
            } else {
                sorted[half]
            };
            let h = 6.0 * median;
            for (rho, residual) in rho.iter_mut().zip(&residual) {
                *rho = if h == 0.0 {
                    1.0
                } else {
                    let u = residual / h;
                    if u >= 1.0 {
                        0.0
                    } else {
                        let v = 1.0 - u * u;
                        v * v
                    }
                };
            }
        }
        r_iter += 1;
    }
    smoothed.truncate(size);
    Ok(Smoothed {
        ticks,
        values: smoothed,
    })
}
