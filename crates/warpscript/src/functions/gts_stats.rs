// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Statistics and value-based transforms of Geo Time Series: `STANDARDIZE`, `NORMALIZE`,
//! `ISONORMALIZE`, `ZSCORE`, `SKEWNESS`, `KURTOSIS`, `MODE`, `VALUESPLIT`, `VALUEHISTOGRAM`,
//! `QUANTIZE` and `LOCSTRINGS`.

use indexmap::IndexMap;

use super::Entry;
use super::args::fail;
use super::functional::java_double_compare;
use super::gts::{element_or_list, gts_or_list, gts_or_list_try};
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, NO_LOCATION, Type, Values};
use crate::jmath;
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("STANDARDIZE", Entry::Stack(standardize)),
    ("NORMALIZE", Entry::Stack(normalize)),
    ("ISONORMALIZE", Entry::Stack(isonormalize)),
    ("ZSCORE", Entry::Stack(zscore)),
    ("SKEWNESS", Entry::Stack(skewness)),
    ("KURTOSIS", Entry::Stack(kurtosis)),
    ("MODE", Entry::Stack(mode)),
    ("VALUESPLIT", Entry::Stack(valuesplit)),
    ("VALUEHISTOGRAM", Entry::Stack(valuehistogram)),
    ("QUANTIZE", Entry::Stack(quantize)),
    ("LOCSTRINGS", Entry::Stack(locstrings)),
];

/// The values of a numeric series as doubles (`((Number) value).doubleValue()`), `None` for
/// a series that is not numeric.
fn doubles(gts: &GeoTimeSerie) -> Option<Vec<f64>> {
    match &gts.values {
        Values::Long(v) => Some(v.iter().map(|&l| l as f64).collect()),
        Values::Double(v) => Some(v.clone()),
        _ => None,
    }
}

/// A copy of `gts` (metadata, bucketization, sort flags) whose values are the given doubles,
/// datapoint by datapoint.
fn with_doubles(gts: &GeoTimeSerie, values: impl Iterator<Item = f64>) -> GeoTimeSerie {
    let mut out = gts.clone_empty();
    for (i, value) in values.enumerate() {
        out.set_value(
            gts.tick_at(i),
            gts.location_at(i),
            gts.elevation_at(i),
            &Value::Double(value),
            false,
        );
    }
    out.sorted = gts.sorted;
    out.reversed = gts.reversed;
    out
}

/// `GTSHelper.musigma`: the mean and standard deviation (Bessel-corrected on request) of
/// the values of a numeric series, in plain double arithmetic.
fn musigma(values: &[f64], bessel: bool) -> (f64, f64) {
    let n = values.len() as f64;
    let mut sum = 0.0;
    let mut sumsq = 0.0;
    for &v in values {
        sum += v;
        sumsq += v * v;
    }
    let mut variance = sumsq / n - sum * sum / (n * n);
    if bessel && values.len() > 1 {
        variance = variance * n / (n - 1.0);
    }
    (sum / n, variance.sqrt())
}

// -----------------------------------------------------------------------------------------
// STANDARDIZE, NORMALIZE, ISONORMALIZE
// -----------------------------------------------------------------------------------------

/// `GTSHelper.standardize`: `(x - mean) / sd` (Bessel-corrected `sd`, no division when it is
/// `0`); a clone of a non numeric or empty series.
fn standardize_gts(gts: &GeoTimeSerie) -> GeoTimeSerie {
    let Some(values) = doubles(gts) else {
        return gts.clone();
    };
    if values.is_empty() {
        return gts.clone();
    }
    let (mean, sd) = musigma(&values, true);
    with_doubles(
        gts,
        values.into_iter().map(|v| {
            let centered = v - mean;
            if sd == 0.0 { centered } else { centered / sd }
        }),
    )
}

/// `GTSHelper.normalize` / `isonormalize`: `(x - min) / (max - min)` (`(x - mean) / (max -
/// min)` for the iso variant), `1.0` for a constant series; a clone of a non numeric or
/// empty series. `LONG` series compute their extrema as longs.
#[allow(clippy::float_cmp)] // Java `==` on the extrema
fn normalize_gts(gts: &GeoTimeSerie, iso: bool) -> GeoTimeSerie {
    if gts.size() == 0 {
        return gts.clone();
    }
    let (constant, range, offsets): (bool, f64, Vec<f64>) = match &gts.values {
        Values::Long(v) => {
            let (mut lmin, mut lmax) = (i64::MAX, i64::MIN);
            let mut sum = 0.0;
            for &value in v {
                lmax = lmax.max(value);
                lmin = lmin.min(value);
                sum += value as f64;
            }
            // `x - min` is a long subtraction (which may wrap), `x - mean` a double one.
            let mean = sum / v.len() as f64;
            let offsets = v
                .iter()
                .map(|&value| {
                    if iso {
                        value as f64 - mean
                    } else {
                        value.wrapping_sub(lmin) as f64
                    }
                })
                .collect();
            (lmin == lmax, lmax.wrapping_sub(lmin) as f64, offsets)
        }
        Values::Double(v) => {
            let (mut dmin, mut dmax) = (f64::INFINITY, f64::NEG_INFINITY);
            let mut sum = 0.0;
            for &value in v {
                if value > dmax {
                    dmax = value;
                }
                if value < dmin {
                    dmin = value;
                }
                sum += value;
            }
            let reference = if iso { sum / v.len() as f64 } else { dmin };
            (
                dmin == dmax,
                dmax - dmin,
                v.iter().map(|&value| value - reference).collect(),
            )
        }
        _ => return gts.clone(),
    };
    with_doubles(
        gts,
        offsets
            .into_iter()
            .map(|offset| if constant { 1.0 } else { offset / range }),
    )
}

fn standardize(stack: &mut Stack) -> Result<()> {
    gts_or_list(stack, "STANDARDIZE", |gts| {
        Value::Gts(standardize_gts(&gts.borrow()).shared())
    })
}

fn normalize(stack: &mut Stack) -> Result<()> {
    gts_or_list(stack, "NORMALIZE", |gts| {
        Value::Gts(normalize_gts(&gts.borrow(), false).shared())
    })
}

fn isonormalize(stack: &mut Stack) -> Result<()> {
    gts_or_list(stack, "ISONORMALIZE", |gts| {
        Value::Gts(normalize_gts(&gts.borrow(), true).shared())
    })
}

// -----------------------------------------------------------------------------------------
// ZSCORE, SKEWNESS, KURTOSIS
// -----------------------------------------------------------------------------------------

/// `GTSOutliersHelper.median` of sorted doubles (`Arrays.sort` order), `None` when there is
/// none.
fn median(values: &[f64]) -> Option<f64> {
    let mut copy = values.to_vec();
    copy.sort_by(|a, b| java_double_compare(*a, *b));
    let n = copy.len();
    if n == 0 {
        return None;
    }
    Some(if n.is_multiple_of(2) {
        // `(a + b) / 2` as Java computes it.
        let (a, b) = (copy[n / 2], copy[n / 2 - 1]);
        #[allow(clippy::manual_midpoint)]
        let median = (a + b) / 2.0;
        median
    } else {
        copy[n / 2]
    })
}

/// `GTSOutliersHelper.zScore`: `(x - m) / s` with the mean and Bessel-corrected standard
/// deviation, or the median and the median absolute deviation (scaled by `0.6745`, falling
/// back to the standard deviation when it is `0`).
fn zscore_gts(gts: &GeoTimeSerie, use_median: bool) -> Result<GeoTimeSerie> {
    let Values::Double(values) = &gts.values else {
        return Err(Error::new("GTS must be of type Double."));
    };
    let values = values.clone();
    let (m, std) = if use_median {
        // Warp 10 reads the median of an empty series anyway.
        let Some(med) = median(&values) else {
            return Err(Error::new("Index 0 out of bounds for length 0"));
        };
        let deviations: Vec<f64> = values.iter().map(|v| (v - med).abs()).collect();
        let mad = median(&deviations).unwrap_or(f64::NAN);
        let scale = if mad == 0.0 {
            musigma(&values, true).1
        } else {
            mad / 0.6745
        };
        (med, scale)
    } else {
        musigma(&values, true)
    };
    if std == 0.0 {
        return Err(Error::new(format!(
            "{} Deviation is null",
            if use_median {
                "Median Absolute"
            } else {
                "Standard"
            }
        )));
    }
    Ok(with_doubles(gts, values.into_iter().map(|v| (v - m) / std)))
}

/// `ZSCORE`: standardize a `DOUBLE` series by its mean and standard deviation (`false`) or
/// by its median and median absolute deviation (`true`).
fn zscore(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ZSCORE";
    let Value::Boolean(use_median) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a flag (a BOOLEAN) on top of the stack indicating whether to normalize by the mean (F), or by the median (T).",
        ));
    };
    gts_or_list_try(stack, NAME, |gts| {
        Ok(Value::Gts(zscore_gts(&gts.borrow(), use_median)?.shared()))
    })
}

/// `GTSHelper.standardizedMoment`: the mean of `((x - mean) / sd) ^ moment`, `None` for a
/// series that is not numeric; the squares of `LONG` values are computed as longs.
fn standardized_moment(moment: i32, gts: &GeoTimeSerie, bessel: bool) -> Option<f64> {
    let n = gts.size() as f64;
    let mut sum = 0.0;
    let mut sumsq = 0.0;
    let values: Vec<f64> = match &gts.values {
        Values::Long(v) => {
            for &value in v {
                sum += value as f64;
                sumsq += value.wrapping_mul(value) as f64;
            }
            v.iter().map(|&value| value as f64).collect()
        }
        Values::Double(v) => {
            for &value in v {
                sum += value;
                sumsq += value * value;
            }
            v.clone()
        }
        _ => return None,
    };
    let mean = sum / n;
    let mut variance = sumsq / n - sum * sum / (n * n);
    if gts.size() > 1 && bessel {
        variance = variance * n / (n - 1.0);
    }
    let sd = variance.sqrt();
    let mut moment_value = 0.0;
    for value in values {
        moment_value += jmath::pow((value - mean) / sd, f64::from(moment));
    }
    Some(moment_value / n)
}

/// `SKEWNESS` / `KURTOSIS`: the third / fourth standardized moment of a numeric series.
fn moment_fn(stack: &mut Stack, name: &str, moment: i32, what: &str) -> Result<()> {
    let Value::Boolean(bessel) = stack.pop()? else {
        return Err(fail(
            name,
            "expects a boolean on top of the stack to determine if Bessel's correction should be applied or not.",
        ));
    };
    gts_or_list_try(stack, name, |gts| {
        let gts = gts.borrow();
        if gts.size() == 0 {
            return Err(fail(
                name,
                &format!("can only compute {what} for non empty series."),
            ));
        }
        let Some(value) = standardized_moment(moment, &gts, bessel) else {
            return Err(fail(
                name,
                &format!("can only compute {what} for numerical series."),
            ));
        };
        Ok(Value::Double(value))
    })
}

fn skewness(stack: &mut Stack) -> Result<()> {
    moment_fn(stack, "SKEWNESS", 3, "skewness")
}

fn kurtosis(stack: &mut Stack) -> Result<()> {
    moment_fn(stack, "KURTOSIS", 4, "kurtosis")
}

// -----------------------------------------------------------------------------------------
// MODE, VALUESPLIT, VALUEHISTOGRAM
// -----------------------------------------------------------------------------------------

/// `MODE`: the most frequent values of a Geo Time Series (value-sorted in place), as a list.
fn mode(stack: &mut Stack) -> Result<()> {
    gts_or_list(stack, "MODE", |gts| {
        let mut gts = gts.borrow_mut();
        let n = gts.size();
        let mut modes = Vec::new();
        if n == 0 {
            return Value::list(modes);
        }
        gts.value_sort(false);
        let mut count = 0usize;
        let mut mode_count = 0usize;
        let mut last_value: Option<Value> = None;
        let mut record = |value: Value, count: usize, modes: &mut Vec<Value>| {
            if count > mode_count {
                mode_count = count;
                modes.clear();
                modes.push(value);
            } else if count == mode_count {
                modes.push(value);
            }
        };
        for i in 0..n {
            let value = gts.value_at(i);
            match &last_value {
                Some(last) if *last != value => {
                    record(last.clone(), count, &mut modes);
                    count = 1;
                    last_value = Some(value);
                }
                Some(_) => count += 1,
                None => {
                    count = 1;
                    last_value = Some(value);
                }
            }
            if i == n - 1 {
                record(last_value.clone().unwrap_or(Value::Null), count, &mut modes);
            }
        }
        Value::list(modes)
    })
}

/// `VALUESPLIT`: split a Geo Time Series (value-sorted in place) into one series per
/// distinct value, labelled with the value; a `BOOLEAN` series yields its `false` and `true`
/// halves (the empty one left out).
fn valuesplit(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "VALUESPLIT";
    let Value::String(label) = stack.pop()? else {
        return Err(fail(NAME, "expects a label name on top of the stack."));
    };
    gts_or_list(stack, NAME, |gts| {
        let mut gts = gts.borrow_mut();
        let mut series: Vec<GeoTimeSerie> = Vec::new();
        if gts.type_of() == Type::Boolean {
            // `GTSHelper.booleanGTSSplit`
            let mut halves = [gts.clone_empty(), gts.clone_empty()];
            for i in 0..gts.size() {
                let half = usize::from(gts.value_at(i) == Value::Boolean(true));
                halves[half].copy_datapoint(&gts, i);
            }
            for (half, text) in halves.iter_mut().zip(["false", "true"]) {
                half.metadata
                    .labels
                    .insert(label.to_string(), text.to_owned());
            }
            let [falses, trues] = halves;
            if falses.size() != 0 {
                series.push(falses);
            }
            if trues.size() != 0 {
                series.push(trues);
            }
        } else {
            gts.value_sort(false);
            let mut last_value: Option<Value> = None;
            for i in 0..gts.size() {
                let value = gts.value_at(i);
                if last_value.as_ref() != Some(&value) {
                    let mut split = gts.clone_empty();
                    split
                        .metadata
                        .labels
                        .insert(label.to_string(), value.to_string());
                    series.push(split);
                }
                let current = series.len() - 1;
                series[current].copy_datapoint(&gts, i);
                last_value = Some(value);
            }
        }
        Value::list(
            series
                .into_iter()
                .map(|serie| Value::Gts(serie.shared()))
                .collect(),
        )
    })
}

/// `GTSHelper.valueHistogram`: the number of occurrences of each value, in order of first
/// appearance, plus the number of empty buckets under the `null` key for a bucketized
/// series.
fn value_histogram(gts: &GeoTimeSerie) -> Value {
    let mut occurrences: IndexMap<Value, i64> = IndexMap::new();
    for i in 0..gts.size() {
        *occurrences.entry(gts.value_at(i)).or_insert(0) += 1;
    }
    if gts.is_bucketized() && gts.bucketcount != gts.size() as i64 {
        occurrences.insert(Value::Null, gts.bucketcount.wrapping_sub(gts.size() as i64));
    }
    Value::map(
        occurrences
            .into_iter()
            .map(|(value, count)| (value, Value::Long(count)))
            .collect(),
    )
}

/// `VALUEHISTOGRAM`: the histogram of the values of a Geo Time Series or of each of a list.
fn valuehistogram(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "VALUEHISTOGRAM";
    let top = stack.pop()?;
    let result = match &top {
        Value::Gts(gts) => value_histogram(&gts.borrow()),
        Value::List(list) => {
            let mut histograms = Vec::with_capacity(list.borrow().len());
            for element in list.borrow().iter() {
                let Value::Gts(gts) = element else {
                    stack.push(top.clone());
                    return Err(fail(
                        NAME,
                        "can only operate on Geo Time Series or GTS Encoder instances.",
                    ));
                };
                histograms.push(value_histogram(&gts.borrow()));
            }
            Value::list(histograms)
        }
        _ => {
            stack.push(top.clone());
            return Err(fail(NAME, "can only operate on Geo Time Series instances."));
        }
    };
    stack.push(result);
    Ok(())
}

// -----------------------------------------------------------------------------------------
// QUANTIZE, LOCSTRINGS
// -----------------------------------------------------------------------------------------

/// `Arrays.binarySearch(double[], double)`: `Double.compare` order, `-(insertion) - 1` when
/// absent.
fn java_binary_search_doubles(bounds: &[f64], key: f64) -> i64 {
    let mut low = 0i64;
    let mut high = bounds.len() as i64 - 1;
    while low <= high {
        let mid = (low + high) >> 1;
        let mid_val = bounds[mid as usize];
        if mid_val < key {
            low = mid + 1;
        } else if mid_val > key {
            high = mid - 1;
        } else {
            let (mid_bits, key_bits) = (mid_val.to_bits() as i64, key.to_bits() as i64);
            match mid_bits.cmp(&key_bits) {
                std::cmp::Ordering::Equal => return mid,
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid - 1,
            }
        }
    }
    -(low + 1)
}

/// `GTSHelper.quantize`: replace each value of a numeric series by the rank of the interval
/// (between consecutive bounds) it falls in, or by the value given for that rank.
fn quantize_gts(
    gts: &GeoTimeSerie,
    bounds: &[f64],
    values: Option<&[Value]>,
) -> Result<GeoTimeSerie> {
    let Some(values_as_doubles) = doubles(gts) else {
        return Err(Error::new("Can only quantify numeric Geo Time Series."));
    };
    let mut quantified = gts.clone_empty();
    for (i, v) in values_as_doubles.into_iter().enumerate() {
        let insertion = java_binary_search_doubles(bounds, v);
        let rank = if insertion >= 0 {
            insertion
        } else {
            -(1 + insertion)
        };
        let value = match values {
            Some(values) => values[rank as usize].clone(),
            None => Value::Long(rank),
        };
        quantified.set_value(
            gts.tick_at(i),
            gts.location_at(i),
            gts.elevation_at(i),
            &value,
            false,
        );
    }
    Ok(quantified)
}

/// `QUANTIZE`: quantize a Geo Time Series (or each of a list) along a list of bounds, the
/// ranks optionally mapped to a list of values (one more than the bounds).
fn quantize(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "QUANTIZE";
    let Value::List(rank_to_value) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a list of target values on top of the stack.",
        ));
    };
    let Value::List(bounds_list) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a list of bounds under the top of the stack.",
        ));
    };
    let rank_to_value = rank_to_value.borrow().clone();
    let bounds_list = bounds_list.borrow();
    if !rank_to_value.is_empty() && rank_to_value.len() != bounds_list.len() + 1 {
        return Err(fail(
            NAME,
            &format!(
                "expected {} values but got {}",
                bounds_list.len() + 1,
                rank_to_value.len()
            ),
        ));
    }
    let mut bounds = Vec::with_capacity(bounds_list.len());
    for (i, bound) in bounds_list.iter().enumerate() {
        let bound = bound.as_f64().ok_or_else(|| {
            Error::new(format!(
                "class {} cannot be cast to class java.lang.Number",
                bound.java_class()
            ))
        })?;
        if !bound.is_finite() {
            return Err(fail(NAME, "expects the bounds to be finite."));
        }
        if i > 0 && bound <= bounds[i - 1] {
            return Err(fail(NAME, "identified unordered or duplicate bounds."));
        }
        bounds.push(bound);
    }
    let values = if rank_to_value.is_empty() {
        None
    } else {
        Some(rank_to_value.as_slice())
    };
    element_or_list(stack, |element| {
        let Value::Gts(gts) = element else {
            return Err(fail(
                NAME,
                "expects a Geo Time Series instance or a list thereof.",
            ));
        };
        Ok(Value::Gts(
            quantize_gts(&gts.borrow(), &bounds, values)?.shared(),
        ))
    })
}

/// `LOCSTRINGS`: the locations of a Geo Time Series (sorted in place) as 16-digit
/// hexadecimal HHCodes (`""` without location).
fn locstrings(stack: &mut Stack) -> Result<()> {
    let Value::Gts(gts) = stack.pop()? else {
        return Err(fail(
            "LOCSTRINGS",
            "expects a Geo Time Series on top of the stack.",
        ));
    };
    let mut gts = gts.borrow_mut();
    gts.sort(false);
    let locations = (0..gts.size())
        .map(|i| match gts.location_at(i) {
            NO_LOCATION => Value::from(""),
            location => Value::from(format!("{location:016x}")),
        })
        .collect();
    stack.push(Value::list(locations));
    Ok(())
}
