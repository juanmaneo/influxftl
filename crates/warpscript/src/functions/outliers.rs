// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Outlier tests (`GTSOutliersHelper`): `THRESHOLDTEST`, `ZSCORETEST`, `GRUBBSTEST` and
//! `ESDTEST`.

#![allow(clippy::float_cmp)] // Java `==` on the deviations

use super::Entry;
use super::args::fail;
use super::functional::java_double_compare;
use super::gts::gts_or_list_try;
use super::gts_stats::musigma;
use crate::distribution::TDistribution;
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Values};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("THRESHOLDTEST", Entry::Stack(thresholdtest)),
    ("ZSCORETEST", Entry::Stack(zscoretest)),
    ("GRUBBSTEST", Entry::Stack(grubbstest)),
    ("ESDTEST", Entry::Stack(esdtest)),
];

/// `GTSOutliersHelper.doubleCheck`: the values of a `DOUBLE` series.
pub(super) fn double_values(gts: &GeoTimeSerie) -> Result<&[f64]> {
    match &gts.values {
        Values::Double(values) => Ok(values),
        _ => Err(Error::usage("GTS", "must be of type Double.")),
    }
}

/// `GTSOutliersHelper.median`: the median of non empty values (`Arrays.sort` order).
#[allow(clippy::manual_midpoint)] // Java's `(a + b) / 2`
fn median(values: &[f64]) -> f64 {
    let n = values.len();
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| java_double_compare(*a, *b));
    if n.is_multiple_of(2) {
        (sorted[n / 2] + sorted[n / 2 - 1]) / 2.0
    } else {
        sorted[n / 2]
    }
}

/// `GTSOutliersHelper.madsigma` of non empty values: the mean and standard deviation, or the
/// median and the median absolute deviation (scaled by `1 / 0.6745`, the standard deviation
/// when it is 0).
fn madsigma(values: &[f64], use_median: bool) -> (f64, f64) {
    if !use_median {
        return musigma(values, true);
    }
    let m = median(values);
    let deviations: Vec<f64> = values.iter().map(|v| (v - m).abs()).collect();
    let mad = median(&deviations);
    let sigma = if mad == 0.0 {
        musigma(values, true).1
    } else {
        mad / 0.6745
    };
    (m, sigma)
}

/// The index of the largest `|z|` (the first one), `0` when none exceeds `-∞`.
fn most_deviant(values: &[f64], m: f64, std: f64) -> (usize, f64) {
    let mut max = f64::NEG_INFINITY;
    let mut index = 0;
    for (i, v) in values.iter().enumerate() {
        let z = ((v - m) / std).abs();
        if z > max {
            max = z;
            index = i;
        }
    }
    (index, max)
}

/// The list of `LONG` ticks.
fn ticks(ticks: Vec<i64>) -> Value {
    Value::list(ticks.into_iter().map(Value::Long).collect())
}

/// `THRESHOLDTEST`: the ticks of the values at least the threshold.
fn thresholdtest(stack: &mut Stack) -> Result<()> {
    let Value::Double(threshold) = stack.pop()? else {
        return Err(fail(
            "THRESHOLDTEST",
            "expects a threshold (a DOUBLE) on top of the stack.",
        ));
    };
    gts_or_list_try(stack, "THRESHOLDTEST", |gts| {
        let gts = gts.borrow();
        let values = double_values(&gts)?;
        Ok(ticks(
            values
                .iter()
                .zip(&gts.ticks)
                .filter(|(v, _)| **v >= threshold)
                .map(|(_, t)| *t)
                .collect(),
        ))
    })
}

/// Pop the optional `DOUBLE` level of `ZSCORETEST` / `GRUBBSTEST` / `ESDTEST` (`default` when
/// the top is the flag) then the `BOOLEAN` flag.
fn pop_level_and_flag(
    stack: &mut Stack,
    name: &str,
    default: f64,
    level_message: &str,
    flag_message: &str,
) -> Result<(f64, bool)> {
    let level = match stack.pop()? {
        Value::Double(level) => level,
        Value::Boolean(flag) => return Ok((default, flag)),
        _ => return Err(fail(name, level_message)),
    };
    match stack.pop()? {
        Value::Boolean(flag) => Ok((level, flag)),
        _ => Err(fail(name, flag_message)),
    }
}

/// `ZSCORETEST`: the ticks whose (modified) z-score is at least the threshold.
fn zscoretest(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ZSCORETEST";
    let (threshold, mad) = pop_level_and_flag(
        stack,
        NAME,
        3.5,
        "expects a threshold level (a DOUBLE) or a flag (a BOOLEAN) on top of the stack.",
        "expects a flag (a BOOLEAN) that indicates wether to use modified z-score below the THRESHOLD level.",
    )?;
    gts_or_list_try(stack, NAME, |gts| {
        Ok(ticks(zscore(&gts.borrow(), mad, threshold)?))
    })
}

/// `GTSOutliersHelper.zScoreTest`. The median of no value reads `copy[0]` of an empty array.
pub(super) fn zscore(gts: &GeoTimeSerie, mad: bool, threshold: f64) -> Result<Vec<i64>> {
    let values = double_values(gts)?;
    if mad && values.is_empty() {
        return Err(Error::index_out_of_bounds(0, 0));
    }
    // Without values, the mean and deviation are `NaN`.
    let (m, std) = if values.is_empty() {
        (f64::NAN, f64::NAN)
    } else {
        madsigma(values, mad)
    };
    if std == 0.0 {
        return Ok(Vec::new());
    }
    Ok(values
        .iter()
        .zip(&gts.ticks)
        .filter(|(v, _)| ((**v - m) / std).abs() >= threshold)
        .map(|(_, t)| *t)
        .collect())
}

/// `GTSOutliersHelper.grubbsTest`: the tick of the most deviant value when it exceeds the
/// Grubbs critical value at `alpha`.
fn grubbs(gts: &GeoTimeSerie, mad: bool, alpha: f64) -> Result<Vec<i64>> {
    let values = double_values(gts)?;
    let n = values.len();
    if n < 3 {
        return Ok(Vec::new());
    }
    let (m, std) = madsigma(values, mad);
    if std == 0.0 {
        return Ok(Vec::new());
    }
    let (index, max) = most_deviant(values, m, std);
    let nf = n as f64;
    let t = TDistribution::new(nf - 2.0)
        .inverse_cumulative_probability(alpha / (2 * n as i32) as f64)?;
    let critical = (nf - 1.0) * t.abs() / (nf * (nf - 2.0 + t * t)).sqrt();
    Ok(if max > critical {
        vec![gts.ticks[index]]
    } else {
        Vec::new()
    })
}

/// `GRUBBSTEST`: Grubbs' test for one outlier.
fn grubbstest(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "GRUBBSTEST";
    let (alpha, mad) = pop_level_and_flag(
        stack,
        NAME,
        0.05,
        "expects a significance level (a DOUBLE) or a flag (a BOOLEAN) on top of the stack.",
        "expects a flag (a BOOLEAN) that indicates wether to use modified z-score below the significance level.",
    )?;
    gts_or_list_try(stack, NAME, |gts| {
        Ok(ticks(grubbs(&gts.borrow(), mad, alpha)?))
    })
}

/// `GTSOutliersHelper.ESDTest`: the generalized extreme Studentized deviate test for up to
/// `k` outliers. The most deviant value of each round is swapped to the end of the shrinking
/// working copy; the outliers are those of the rounds up to the last one passing the test.
#[allow(clippy::many_single_char_names)]
pub(super) fn esd(gts: &GeoTimeSerie, k: i32, mad: bool, alpha: f64) -> Result<Vec<i64>> {
    let mut values = double_values(gts)?.to_vec();
    let mut ticks = gts.ticks.clone();
    let mut size = values.len();
    let mut passed: Option<usize> = None;
    for j in 0..k.max(0) as usize {
        let n = size;
        if n < 3 {
            break;
        }
        let (m, std) = madsigma(&values[..n], mad);
        if std == 0.0 {
            break;
        }
        let (index, max) = most_deviant(&values[..n], m, std);
        let nf = n as f64;
        let p = 1.0 - alpha / (2 * n as i32) as f64;
        let t = TDistribution::new(nf - 2.0).inverse_cumulative_probability(p)?;
        let lambda = (nf - 1.0) * t / ((nf - 2.0 + t * t) * nf).sqrt();
        if max > lambda {
            passed = Some(j);
        }
        size -= 1;
        ticks.swap(index, size);
        values[index] = values[size];
    }
    let total = gts.ticks.len();
    Ok(passed.map_or_else(Vec::new, |last| {
        (0..=last).map(|j| ticks[total - 1 - j]).collect()
    }))
}

/// `ESDTEST`: the generalized ESD test.
fn esdtest(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ESDTEST";
    let (alpha, mad) = pop_level_and_flag(
        stack,
        NAME,
        0.05,
        "expects a significance level (a DOUBLE)  or a flag (a BOOLEAN) on top of the stack.",
        "expects a flag (a BOOLEAN) that indicates wether to use modified z-score below the SIGNIFICANCE level.",
    )?;
    let Value::Long(k) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects an upper bound of the number of outliers (a LONG) below the flag.",
        ));
    };
    // `intValue()`.
    let k = k as i32;
    gts_or_list_try(stack, NAME, |gts| {
        Ok(ticks(esd(&gts.borrow(), k, mad, alpha)?))
    })
}
