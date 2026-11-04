// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Local and global polynomial regression on Geo Time Series: `LOWESS` / `RLOWESS`
//! (`GTSHelper.rlowess` and `pointwise_lowess`) and `POLYFIT`.

// The single letter names are those of the Java code and of the LOWESS literature.
#![allow(clippy::many_single_char_names)]

use super::Entry;
use super::args::fail;
use super::gts::gts_or_list_try;
use crate::curvefit::{WeightedPoint, coefficient_count, polynomial_fit};
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Gts, Type, Values, java_binary_search_range};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("LOWESS", Entry::Stack(lowess)),
    ("RLOWESS", Entry::Stack(rlowess)),
    ("POLYFIT", Entry::Stack(polyfit)),
];

/// `((Number) valueAtIndex(gts, idx)).doubleValue()` on a numeric series.
fn number_at(gts: &GeoTimeSerie, idx: usize) -> f64 {
    gts.value_at(idx).as_f64().unwrap_or_default()
}

/// `Math.abs(a - b)` on `long`s, as a `double`.
fn distance(a: i64, b: i64) -> f64 {
    a.wrapping_sub(b).wrapping_abs() as f64
}

/// `GTSHelper.pointwise_lowess`: the local weighted regression of degree `p` at `tick` over
/// the `q` datapoints closest to it, `idx` being the first one at or after `tick`. `rho`
/// holds the robustness weights.
fn pointwise_lowess(
    gts: &GeoTimeSerie,
    idx: i64,
    tick: i64,
    q: i64,
    p: i32,
    rho: Option<&[f64]>,
) -> Result<f64> {
    let n = gts.size() as i64;
    let tick_at = |k: i64| gts.tick_at(k as usize);
    // The `q` nearest datapoints lie strictly between `j` and `i`.
    let mut i = idx;
    let mut j = idx - 1;
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
            let u = distance(gts.ticks[k], tick) / maxdist;
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
            ctick += w * gts.ticks[k] as f64;
            cvalue += w * number_at(gts, k);
        }
        ctick /= wsum;
        cvalue /= wsum;
        let mut covar = 0.0;
        let mut var = 0.0;
        for (k, w) in range.zip(&weights) {
            let dt = gts.ticks[k] as f64 - ctick;
            covar += w * dt * (number_at(gts, k) - cvalue);
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
                x: gts.ticks[k] as f64,
                y: number_at(gts, k),
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
#[derive(Clone, Copy)]
struct Lowess {
    /// The bandwidth: the number of nearest neighbours.
    q: i32,
    /// The number of robustifying iterations.
    r: i32,
    /// The radius (in time units) within which the regression is interpolated.
    d: i64,
    /// The degree of the local polynomials.
    p: i32,
}

/// `GTSHelper.rlowess`: the robust locally weighted regression of a numeric series, at each
/// of its ticks (each of its buckets when bucketized). The series is sorted in place.
#[allow(clippy::too_many_lines)]
fn rlowess_gts(gts: &Gts, params: Lowess) -> Result<Value> {
    let Lowess { q, r, d, p } = params;
    if !matches!(gts.borrow().type_of(), Type::Long | Type::Double) {
        return Err(Error::new("Can only smooth numeric Geo Time Series."));
    }
    if q < 1 {
        return Err(Error::new("Bandwidth parameter must be greater than 0"));
    }
    if r < 0 {
        return Err(Error::new(
            "Robustness parameter must be greater or equal to 0",
        ));
    }
    // Warp 10 also rejects a negative radius here, which its callers never pass.
    if p < 1 {
        return Err(Error::new(
            "Degree of polynomial fit must be greater than 0",
        ));
    }
    if p > 9 {
        return Err(Error::new(
            "Degree of polynomial fit should remain small (lower than 10)",
        ));
    }
    gts.borrow_mut().sort(false);
    let g = gts.borrow();
    let n = g.size();
    let bucketized = g.is_bucketized();
    if g.bucketcount - n as i64 > 500_000 {
        return Err(Error::new("More than 500000 missing values"));
    }
    let mut previous = -1;
    for &tick in &g.ticks {
        if previous == tick {
            return Err(Error::new("Can't be applied on GTS with duplicate ticks"));
        }
        previous = tick;
    }
    // The ticks to estimate (`tickIterator`): those of the series, or every bucket.
    let ticks: Vec<i64> = if bucketized {
        (0..g.bucketcount)
            .rev()
            .map(|b| g.lastbucket.wrapping_sub(b.wrapping_mul(g.bucketspan)))
            .collect()
    } else {
        g.ticks.clone()
    };
    let size = ticks.len();
    // Java's array is a little larger than the number of estimates.
    let mut smoothed = vec![0.0; size + size / 10 + 2];
    let nvalues = i64::from(q).min(size as i64);
    let mut rho = (r > 0).then(|| vec![1.0; n]);
    let mut residual = vec![0.0; if r > 0 { n } else { 0 }];
    let last_index = n as i64 - 1;
    let tick_at = |k: i64| g.tick_at(k as usize);
    let mut r_iter = 0;
    while r_iter < r.wrapping_add(1) {
        let robust = r_iter < r;
        let rho_now = rho.as_deref();
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
                smoothed[ridx] = pointwise_lowess(&g, idx, tick, nvalues, p, rho_now)?;
                if robust && tick == tick_at(idx) {
                    residual[idx as usize] = (number_at(&g, idx as usize) - smoothed[ridx]).abs();
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
                smoothed[ridx - 1] = pointwise_lowess(&g, idx, last_skipped, nvalues, p, rho_now)?;
                if robust && last_skipped == tick_at(idx) {
                    residual[idx as usize] =
                        (number_at(&g, idx as usize) - smoothed[ridx - 1]).abs();
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
                        let sidx = java_binary_search_range(&g.ticks, idx_last, idx, skipped);
                        if sidx > 0 {
                            residual[sidx as usize] =
                                (number_at(&g, sidx as usize) - smoothed[ridx_s]).abs();
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
        if let (true, Some(rho)) = (robust, rho.as_mut()) {
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
    let mut result = g.clone_empty();
    let complete = !bucketized
        || (n as i64 == g.bucketcount
            && g.lastbucket == g.ticks[n - 1]
            && g.lastbucket
                .wrapping_sub(g.bucketspan.wrapping_mul(g.bucketcount - 1))
                == g.ticks[0]);
    if complete {
        result.locations.clone_from(&g.locations);
        result.elevations.clone_from(&g.elevations);
    } else if g.locations.is_some() || g.elevations.is_some() {
        // Warp 10 reads the ticks of the (empty) result to place the locations.
        return Err(Error::null_pointer());
    }
    result.ticks = ticks;
    result.values = Values::Double(smoothed);
    result.sorted = false;
    Ok(Value::Gts(result.shared()))
}

/// Pop an argument of `RLOWESS` / `LOWESS`, which must be a `LONG`.
fn pop_long(stack: &mut Stack, name: &str, message: &str) -> Result<i64> {
    match stack.pop()? {
        Value::Long(l) => Ok(l),
        _ => Err(fail(name, message)),
    }
}

/// `LOWESS`: `gts q LOWESS`, the local linear regression over the `q` nearest neighbours.
fn lowess(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "LOWESS";
    let q = pop_long(
        stack,
        NAME,
        "expects a bandwidth (a LONG) on top of the stack.",
    )? as i32;
    gts_or_list_try(stack, NAME, |gts| {
        rlowess_gts(
            gts,
            Lowess {
                q,
                r: 0,
                d: 0,
                p: 1,
            },
        )
    })
}

/// `RLOWESS`: `gts q r d p RLOWESS`, with `r` robustifying iterations, the regression
/// interpolated within `d` time units of the last one and local polynomials of degree `p`.
fn rlowess(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "RLOWESS";
    let p = pop_long(
        stack,
        NAME,
        "expects a degree (a LONG) for the polynomial fit on top of the stack.",
    )? as i32;
    let d = pop_long(
        stack,
        NAME,
        "expects a delta radius (a LONG in µs) within which LOWESS is computed only once, in 2nd position from the top of the stack.",
    )?;
    let r = pop_long(
        stack,
        NAME,
        "expects a number of robustness iteration (a LONG) in 3rd position from the top of the stack.",
    )? as i32;
    let q = pop_long(
        stack,
        NAME,
        "expects a bandwidth (a LONG) in 4th position from the top of the stack.",
    )? as i32;
    // A negative radius was meant as a fraction of the time span, but Warp 10 truncates
    // `10^d` to zero first.
    let d = d.max(0);
    gts_or_list_try(stack, NAME, |gts| rlowess_gts(gts, Lowess { q, r, d, p }))
}

/// `POLYFIT`: `gts degree (useBucketId) POLYFIT`, the coefficients (constant term first) of
/// the least-squares polynomial through the datapoints, against their ticks or their bucket
/// indexes.
fn polyfit(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "POLYFIT";
    let mut top = stack.pop()?;
    let mut use_bucket_id = false;
    if let Value::Boolean(b) = top {
        use_bucket_id = b;
        top = stack.pop()?;
    }
    let Value::Long(degree) = top else {
        return Err(fail(
            NAME,
            "expects a polynomial degree on top if the stack.",
        ));
    };
    let count = coefficient_count(degree as i32)?;
    let Value::Gts(gts) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a Geo Time Series below the degree of the polynomial.",
        ));
    };
    let g = gts.borrow().clone();
    let n = g.size();
    if n == 0 {
        return Err(fail(NAME, "operates on a non empty Geo Time Series."));
    }
    if use_bucket_id && !g.is_bucketized() {
        return Err(fail(
            NAME,
            "use of bucket id is only possible for bucketized Geo Time Series.",
        ));
    }
    if !matches!(g.type_of(), Type::Long | Type::Double) {
        return Err(fail(NAME, "can only operate on numerical Geo Time Series."));
    }
    let firsttick = g.first_tick();
    let points: Vec<WeightedPoint> = (0..n)
        .map(|i| {
            let tick = g.ticks[i];
            let x = if use_bucket_id {
                tick.wrapping_sub(firsttick).wrapping_div(g.bucketspan) as i32 as f64
            } else {
                tick as f64
            };
            WeightedPoint {
                weight: 1.0,
                x,
                y: number_at(&g, i),
            }
        })
        .collect();
    // Warp 10 means to limit the iterations to twice the number of datapoints, but drops the
    // fitter `withMaxIterations` returns: the limit stays `Integer.MAX_VALUE`.
    let coefficients = polynomial_fit(&points, count, i32::MAX)?;
    stack.push(Value::list(
        coefficients.into_iter().map(Value::Double).collect(),
    ));
    Ok(())
}
