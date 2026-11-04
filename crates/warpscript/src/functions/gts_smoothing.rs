// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Downsampling and smoothing of Geo Time Series: `LTTB` / `TLTTB` (Largest Triangle Three
//! Buckets) and `SINGLEEXPONENTIALSMOOTHING` / `DOUBLEEXPONENTIALSMOOTHING`.

use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::gts::gts_or_list_try;
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Gts, Type};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("LTTB", Entry::Stack(lttb)),
    ("TLTTB", Entry::Stack(tlttb)),
    (
        "SINGLEEXPONENTIALSMOOTHING",
        Entry::Stack(single_exponential_smoothing),
    ),
    (
        "DOUBLEEXPONENTIALSMOOTHING",
        Entry::Stack(double_exponential_smoothing),
    ),
];

/// `((Number) valueAtIndex(gts, idx)).doubleValue()` on a numeric series.
fn number_at(gts: &GeoTimeSerie, idx: usize) -> f64 {
    gts.value_at(idx).as_f64().unwrap_or_default()
}

// -----------------------------------------------------------------------------------------
// LTTB
// -----------------------------------------------------------------------------------------

/// The `[start, end)` index range of the buckets of the time based `LTTB`: pairs of
/// boundaries (inclusive end) of the buckets of `timebucket` time units, the first and last
/// datapoints alone in theirs.
fn time_buckets(ticks: &[i64], timebucket: i64) -> Result<Vec<i32>> {
    let n = ticks.len();
    let lowerts = ticks[0].wrapping_add(1);
    let mut buckets = vec![0];
    let mut lastbucket = 0i64;
    for (i, tick) in ticks.iter().enumerate().take(n - 1).skip(1) {
        if timebucket == 0 {
            return Err(Error::runtime("java.lang.ArithmeticException", "/ by zero"));
        }
        let bucket = 1i64.wrapping_add(tick.wrapping_sub(lowerts).wrapping_div(timebucket));
        if bucket != lastbucket {
            buckets.push(i as i32 - 1);
            buckets.push(i as i32);
            lastbucket = bucket;
        }
    }
    buckets.push(n as i32 - 2);
    buckets.push(n as i32 - 1);
    buckets.push(n as i32 - 1);
    Ok(buckets)
}

/// `GTSHelper.lttb`: downsample a numeric series to `threshold` datapoints, keeping the
/// first and the last ones and, in each bucket, the one forming the largest triangle with
/// the previously retained datapoint and the average of the next bucket. The buckets hold
/// the same number of datapoints, or span the same time when `timebased`. Small, `STRING` and
/// `BOOLEAN` series are returned as they are; the others are sorted in place.
fn lttb_gts(gts: &Gts, threshold: i32, timebased: bool) -> Result<Value> {
    let (size, kind) = {
        let g = gts.borrow();
        (g.size() as i32, g.type_of())
    };
    if size <= threshold.wrapping_sub(2) || matches!(kind, Type::String | Type::Boolean) {
        return Ok(Value::Gts(Rc::clone(gts)));
    }
    if threshold < 3 {
        return Err(Error::new("Threshold MUST be >= 3."));
    }
    let mut threshold = threshold;
    let bucketsize = f64::from(size) / f64::from(threshold - 1);
    gts.borrow_mut().sort(false);
    let g = gts.borrow();
    let ticks = &g.ticks;
    let n = size as usize;
    let span = ticks[n - 1].wrapping_sub(ticks[0]).wrapping_sub(2);
    let timebucket = (span as f64 / f64::from(threshold - 2)).ceil() as i64;
    let buckets = if timebased {
        let buckets = time_buckets(ticks, timebucket)?;
        threshold = buckets.len() as i32 / 2;
        buckets
    } else {
        Vec::new()
    };
    let bucket = |k: i32| buckets[k as usize];
    let mut sampled = g.clone_empty();
    sampled.copy_datapoint(&g, 0);
    // Warp 10 never moves the reference datapoint: every triangle starts at the first one.
    let refvalue = number_at(&g, 0);
    let reftick = ticks[0] as f64;
    for i in 0..threshold - 2 {
        // The range of the next bucket, whose average is the third vertex of the triangles.
        let (first, last) = if timebased {
            (bucket(2 * (i + 2)), bucket(2 * (i + 2) + 1) + 1)
        } else {
            let first = 1 + (f64::from(i + 1) * bucketsize).floor() as i32;
            let last = 1 + (f64::from(i + 2) * bucketsize).floor() as i32;
            (first.min(size - 1), last.min(size - 1))
        };
        let mut ticksum = 0.0;
        let mut valuesum = 0.0;
        for j in first..last {
            ticksum += ticks[j as usize] as f64;
            valuesum += number_at(&g, j as usize);
        }
        let count = f64::from(last - first + 1);
        let tickavg = ticksum / count;
        let valueavg = valuesum / count;
        // The range of the current bucket.
        let (first, last) = if timebased {
            (bucket(2 * (i + 1)), bucket(2 * (i + 1) + 1) + 1)
        } else {
            let first = 1 + (f64::from(i) * bucketsize).floor() as i32;
            let last = 1 + (f64::from(i + 1) * bucketsize).floor() as i32;
            (
                if first >= size - 1 { size - 2 } else { first },
                last.min(size - 1),
            )
        };
        let mut maxarea = -1.0;
        let mut nextref = None;
        for j in first..last {
            let tick = ticks[j as usize] as f64;
            let value = number_at(&g, j as usize);
            let area = 0.5
                * ((reftick - tickavg) * (value - refvalue)
                    - (reftick - tick) * (valueavg - refvalue))
                    .abs();
            if area > maxarea {
                maxarea = area;
                nextref = Some(j as usize);
            }
        }
        if let Some(nextref) = nextref {
            sampled.copy_datapoint(&g, nextref);
        }
    }
    sampled.copy_datapoint(&g, n - 1);
    Ok(Value::Gts(sampled.shared()))
}

/// `LTTB` and `TLTTB`: `gts threshold LTTB`.
fn lttb_impl(stack: &mut Stack, name: &str, timebased: bool) -> Result<()> {
    let Value::Long(threshold) = stack.pop()? else {
        return Err(fail(name, "expects a threshold on top of the stack."));
    };
    gts_or_list_try(stack, name, |gts| {
        lttb_gts(gts, threshold as i32, timebased)
    })
}

/// `LTTB`: buckets of the same number of datapoints.
fn lttb(stack: &mut Stack) -> Result<()> {
    lttb_impl(stack, "LTTB", false)
}

/// `TLTTB`: buckets of the same time span.
fn tlttb(stack: &mut Stack) -> Result<()> {
    lttb_impl(stack, "TLTTB", true)
}

// -----------------------------------------------------------------------------------------
// Exponential smoothing
// -----------------------------------------------------------------------------------------

/// Pop a smoothing factor, which must be a number strictly between `0` and `1`.
fn pop_factor(stack: &mut Stack, missing: impl FnOnce() -> Error, what: &str) -> Result<f64> {
    let Some(factor) = stack.pop()?.as_f64() else {
        return Err(missing());
    };
    check_factor(factor, what)?;
    Ok(factor)
}

/// Check that a smoothing factor lies strictly between `0` and `1` (`NaN` passes, as in
/// Warp 10).
fn check_factor(factor: f64, what: &str) -> Result<()> {
    if factor <= 0.0 || factor >= 1.0 {
        return Err(Error::new(format!(
            "{what} should be strictly between 0.0 and 1.0."
        )));
    }
    Ok(())
}

/// The checks of `GTSHelper.singleExponentialSmoothing` / `doubleExponentialSmoothing`, the
/// empty result series (metadata and bucketization of the input) and the sorted input.
fn smoothing_input(gts: &Gts) -> Result<(GeoTimeSerie, GeoTimeSerie)> {
    let kind = gts.borrow().type_of();
    if kind != Type::Long && kind != Type::Double {
        return Err(Error::new(
            "Can only perform exponential smoothing on numeric Geo Time Series.",
        ));
    }
    if gts.borrow().size() < 2 {
        return Err(Error::new(
            "Can only perform exponential smoothing on Geo Time Series containing at least two values.",
        ));
    }
    gts.borrow_mut().sort(false);
    let input = gts.borrow().clone();
    let smoothed = input.clone_empty();
    Ok((input, smoothed))
}

/// `GTSHelper.singleExponentialSmoothing`: `s(0) = x(0)`, `s(t) = α x(t) + (1 - α) s(t-1)`.
fn single_smoothing_gts(gts: &Gts, alpha: f64) -> Result<Value> {
    let (input, mut s) = smoothing_input(gts)?;
    let mut smoothed = number_at(&input, 0);
    let oneminusalpha = 1.0 - alpha;
    for i in 0..input.size() {
        if i > 0 {
            smoothed = alpha * number_at(&input, i) + oneminusalpha * smoothed;
        }
        s.set_value(
            input.ticks[i],
            input.location_at(i),
            input.elevation_at(i),
            &Value::Double(smoothed),
            false,
        );
    }
    Ok(Value::Gts(s.shared()))
}

/// `GTSHelper.doubleExponentialSmoothing` (Holt's linear method): the smoothed series and the
/// trend estimate series, from the second datapoint on.
fn double_smoothing_gts(gts: &Gts, alpha: f64, beta: f64) -> Result<Value> {
    let (input, mut s) = smoothing_input(gts)?;
    let mut b = s.clone();
    let mut smoothed = number_at(&input, 1);
    let mut bestestimate = smoothed - number_at(&input, 0);
    let oneminusalpha = 1.0 - alpha;
    let oneminusbeta = 1.0 - beta;
    for i in 1..input.size() {
        if i > 1 {
            let newsmoothed =
                alpha * number_at(&input, i) + oneminusalpha * (smoothed + bestestimate);
            bestestimate = beta * (newsmoothed - smoothed) + oneminusbeta * bestestimate;
            smoothed = newsmoothed;
        }
        let (tick, location, elevation) =
            (input.ticks[i], input.location_at(i), input.elevation_at(i));
        s.set_value(tick, location, elevation, &Value::Double(smoothed), false);
        b.set_value(
            tick,
            location,
            elevation,
            &Value::Double(bestestimate),
            false,
        );
    }
    Ok(Value::list(vec![
        Value::Gts(s.shared()),
        Value::Gts(b.shared()),
    ]))
}

/// `SINGLEEXPONENTIALSMOOTHING`: `gts alpha SINGLEEXPONENTIALSMOOTHING`.
fn single_exponential_smoothing(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "SINGLEEXPONENTIALSMOOTHING";
    let alpha = pop_factor(
        stack,
        || {
            fail(
                NAME,
                "expects the 'alpha' parameter (smoothing factor) on the top of the stack.",
            )
        },
        "'alpha' (smoothing factor)",
    )?;
    gts_or_list_try(stack, NAME, |gts| single_smoothing_gts(gts, alpha))
}

/// `DOUBLEEXPONENTIALSMOOTHING`: `gts alpha beta DOUBLEEXPONENTIALSMOOTHING`, a list of the
/// smoothed series and the trend series for each input series.
fn double_exponential_smoothing(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "DOUBLEEXPONENTIALSMOOTHING";
    let Some(beta) = stack.pop()?.as_f64() else {
        return Err(fail(
            NAME,
            "expects the 'beta' parameter (trend factor) on the top of the stack.",
        ));
    };
    let alpha = pop_factor(
        stack,
        || {
            fail(
                NAME,
                "expects the 'alpha' parameter (smoothing factor) at the second level of the stack.",
            )
        },
        "'alpha' (smoothing factor)",
    )?;
    check_factor(beta, "'beta' (trend factor)")?;
    gts_or_list_try(stack, NAME, |gts| double_smoothing_gts(gts, alpha, beta))
}
