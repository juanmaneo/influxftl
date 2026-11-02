// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Datapoint selection and ordering of Geo Time Series: `DEDUP`, `VALUEDEDUP`, `COMPACT`,
//! `RANGECOMPACT`, `ONLYBUCKETS`, `EMPTY`, `NONEMPTY`, `COMMONTICKS`, `VALUESORT`,
//! `RVALUESORT`, `LASTSORT`, `METASORT`, `INTEGRATE`, `RESETS`, `MONOTONIC` and `ATBUCKET`.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::Entry;
use super::args::fail;
use super::functional::java_double_compare;
use super::gts::{gts_or_list, long_value, tuple_at_index};
use super::misc::TIME_UNITS_PER_MS;
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Gts, Metadata, Values};
use crate::stack::Stack;
use crate::value::{List, Value};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("DEDUP", Entry::Stack(dedup)),
    ("VALUEDEDUP", Entry::Stack(valuededup)),
    ("COMPACT", Entry::Stack(compact)),
    ("RANGECOMPACT", Entry::Stack(rangecompact)),
    ("ONLYBUCKETS", Entry::Stack(onlybuckets)),
    ("EMPTY", Entry::Stack(empty)),
    ("NONEMPTY", Entry::Stack(nonempty)),
    ("COMMONTICKS", Entry::Stack(commonticks)),
    ("VALUESORT", Entry::Stack(valuesort)),
    ("RVALUESORT", Entry::Stack(rvaluesort)),
    ("LASTSORT", Entry::Stack(lastsort)),
    ("METASORT", Entry::Stack(metasort)),
    ("INTEGRATE", Entry::Stack(integrate)),
    ("RESETS", Entry::Stack(resets)),
    ("MONOTONIC", Entry::Stack(monotonic)),
    ("ATBUCKET", Entry::Stack(atbucket)),
];

/// A list of series as a value.
fn series_list(series: Vec<GeoTimeSerie>) -> Value {
    Value::list(
        series
            .into_iter()
            .map(|gts| Value::Gts(gts.shared()))
            .collect(),
    )
}

/// The series of a list, flattening nested lists of series and ignoring other elements
/// (`MERGE`, `RESETS`, `EMPTY`…); a non series inside a nested list fails with `message`.
fn flatten_series(name: &str, params: &[Value], message: &str) -> Result<Vec<Gts>> {
    let mut series = Vec::new();
    for param in params {
        match param {
            Value::Gts(gts) => series.push(gts.clone()),
            Value::List(list) => {
                for element in list.borrow().iter() {
                    let Value::Gts(gts) = element else {
                        return Err(fail(name, message));
                    };
                    series.push(gts.clone());
                }
            }
            _ => {}
        }
    }
    Ok(series)
}

// -----------------------------------------------------------------------------------------
// DEDUP, VALUEDEDUP, COMPACT, RANGECOMPACT, ONLYBUCKETS
// -----------------------------------------------------------------------------------------

/// `GTSHelper.dedup`: a copy without duplicate ticks, keeping the last datapoint of each
/// tick in storage order (and the sort flags of the input).
fn dedup_gts(gts: &GeoTimeSerie) -> GeoTimeSerie {
    let n = gts.size();
    if n < 2 {
        return gts.clone();
    }
    let mut remaining: HashMap<i64, usize> = HashMap::new();
    for &tick in &gts.ticks {
        *remaining.entry(tick).or_insert(0) += 1;
    }
    if remaining.values().all(|&count| count == 1) {
        return gts.clone();
    }
    let mut kept = Vec::with_capacity(n);
    for (i, tick) in gts.ticks.iter().enumerate() {
        let count = remaining.entry(*tick).or_insert(0);
        if *count > 1 {
            *count -= 1;
            continue;
        }
        kept.push(i);
    }
    let mut out = gts.select(&kept);
    out.sorted = gts.sorted;
    out.reversed = gts.reversed;
    out
}

/// `DEDUP`: remove the duplicate ticks of a Geo Time Series or of every series of a (nested)
/// list, keeping the last value of each tick.
fn dedup(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "DEDUP";
    fn apply(value: &Value) -> Result<Value> {
        match value {
            Value::Gts(gts) => Ok(Value::Gts(dedup_gts(&gts.borrow()).shared())),
            Value::List(list) => {
                let items = list.borrow().clone();
                let mut result = Vec::with_capacity(items.len());
                for item in &items {
                    result.push(apply(item)?);
                }
                Ok(Value::list(result))
            }
            _ => Err(fail(
                NAME,
                "expects a Geo Time Series instance or a list thereof under an optional macro or mapper.",
            )),
        }
    }
    let top = stack.pop()?;
    if matches!(top, Value::Macro(_) | Value::Function(_)) {
        // `GTSHelper.map` with the macro or mapper: the `MAP` framework.
        return Err(fail(
            NAME,
            "with a macro or mapper requires the MAP framework, which is not supported yet.",
        ));
    }
    let result = apply(&top)?;
    stack.push(result);
    Ok(())
}

/// `VALUEDEDUP`: sort a Geo Time Series by value (in place) and keep, for each distinct
/// value, its first (`true`) or last datapoint.
fn valuededup(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "VALUEDEDUP";
    let Value::Boolean(first) = stack.pop()? else {
        return Err(fail(NAME, "expects a boolean on top of the stack."));
    };
    gts_or_list(stack, NAME, |gts| {
        let mut gts = gts.borrow_mut();
        gts.value_sort(false);
        let n = gts.size();
        let mut kept = Vec::new();
        let mut current: Option<Value> = None;
        let mut first_occurrence = 0;
        let mut last_occurrence = 0;
        for idx in 0..n {
            let value = gts.value_at(idx);
            if current.as_ref() == Some(&value) {
                last_occurrence = idx;
                continue;
            }
            if current.is_some() {
                kept.push(if first {
                    first_occurrence
                } else {
                    last_occurrence
                });
            }
            first_occurrence = idx;
            last_occurrence = idx;
            current = Some(value);
        }
        // Warp 10 always records the last run (an empty series yields a `null` value, which
        // `setValue` ignores).
        if current.is_some() {
            kept.push(if first {
                first_occurrence
            } else {
                last_occurrence
            });
        }
        Value::Gts(gts.select(&kept).shared())
    })
}

/// `GTSHelper.compact`: a sorted copy without the datapoints repeating the value, location
/// and elevation of the previous one; the last datapoint is always kept, and so is the last
/// datapoint of each run when `preserve_ranges` is set.
fn compact_gts(gts: &GeoTimeSerie, preserve_ranges: bool) -> GeoTimeSerie {
    let mut clone = gts.clone();
    clone.sort(false);
    let n = clone.size();
    if n <= 2 {
        return clone;
    }
    let same = |i: usize, j: usize| {
        clone.location_at(i) == clone.location_at(j)
            && clone.elevation_at(i) == clone.elevation_at(j)
            && clone.value_at(i) == clone.value_at(j)
    };
    let mut kept = vec![0];
    let mut idx = 0;
    while idx < n - 1 {
        let mut offset = 0;
        while idx + 1 + offset < n - 1 && same(idx + 1 + offset, idx) {
            offset += 1;
        }
        if preserve_ranges && idx + 1 + offset == n - 1 && same(n - 1, idx) {
            offset += 1;
        }
        if preserve_ranges && offset > 0 {
            kept.push(idx + offset);
        }
        if idx + offset + 1 < n {
            kept.push(idx + offset + 1);
        }
        idx += offset + 1;
    }
    clone.select(&kept)
}

/// `COMPACT` / `RANGECOMPACT`: compact a Geo Time Series or each of a list.
fn compact_fn(stack: &mut Stack, name: &str, preserve_ranges: bool) -> Result<()> {
    gts_or_list(stack, name, |gts| {
        Value::Gts(compact_gts(&gts.borrow(), preserve_ranges).shared())
    })
}

fn compact(stack: &mut Stack) -> Result<()> {
    compact_fn(stack, "COMPACT", false)
}

fn rangecompact(stack: &mut Stack) -> Result<()> {
    compact_fn(stack, "RANGECOMPACT", true)
}

/// `GTSHelper.onlybuckets`: a copy of a bucketized series without the datapoints that are
/// not on a bucket boundary, each removed datapoint replaced by the last remaining one
/// (keeping the sort flags of the input); a clone of a series that is not bucketized.
fn onlybuckets_gts(gts: &GeoTimeSerie) -> GeoTimeSerie {
    if !gts.is_bucketized() {
        return gts.clone();
    }
    let is_bucket = |tick: i64| {
        let delta = gts.lastbucket.wrapping_sub(tick);
        let q = delta.wrapping_div(gts.bucketspan);
        delta.wrapping_rem(gts.bucketspan) == 0 && q >= 0 && q < gts.bucketcount
    };
    let mut kept: Vec<usize> = (0..gts.size()).collect();
    let mut values = kept.len();
    let mut i = 0;
    while i < values {
        if is_bucket(gts.ticks[kept[i]]) {
            i += 1;
            continue;
        }
        while values - 1 > i && !is_bucket(gts.ticks[kept[values - 1]]) {
            values -= 1;
        }
        if values - 1 != i {
            kept[i] = kept[values - 1];
            i += 1;
        }
        values -= 1;
    }
    kept.truncate(values);
    let mut out = gts.select(&kept);
    out.sorted = gts.sorted;
    out.reversed = gts.reversed;
    out
}

/// `ONLYBUCKETS`: remove the datapoints that are not on a bucket boundary.
fn onlybuckets(stack: &mut Stack) -> Result<()> {
    gts_or_list(stack, "ONLYBUCKETS", |gts| {
        Value::Gts(onlybuckets_gts(&gts.borrow()).shared())
    })
}

// -----------------------------------------------------------------------------------------
// EMPTY, NONEMPTY, COMMONTICKS
// -----------------------------------------------------------------------------------------

/// `EMPTY` / `NONEMPTY`: the series of a list (nested lists included) without / with
/// datapoints.
fn filter_by_size(stack: &mut Stack, name: &str, keep_empty: bool) -> Result<()> {
    let Value::List(params) = stack.pop()? else {
        return Err(fail(name, "operates on a list of Geo Time Series."));
    };
    let series = flatten_series(name, &params.borrow(), "expects a list of Geo Time Series.")?;
    let result = series
        .into_iter()
        .filter(|gts| (gts.borrow().size() == 0) == keep_empty)
        .map(Value::Gts)
        .collect();
    stack.push(Value::list(result));
    Ok(())
}

fn empty(stack: &mut Stack) -> Result<()> {
    filter_by_size(stack, "EMPTY", true)
}

fn nonempty(stack: &mut Stack) -> Result<()> {
    filter_by_size(stack, "NONEMPTY", false)
}

/// `GTSHelper.commonTicks`: for each series (sorted in place), a copy with the datapoints
/// whose tick appears in every series, found by a sweep led by the shortest series.
fn common_ticks(series: &[Gts]) -> Result<Vec<GeoTimeSerie>> {
    if series.len() == 1 {
        return Ok(vec![series[0].borrow().clone()]);
    }
    for gts in series {
        gts.borrow_mut().sort(false);
    }
    let sorted: Vec<std::cell::Ref<'_, GeoTimeSerie>> = series.iter().map(|g| g.borrow()).collect();
    // The leader is the series with the least datapoints; Warp 10 dies on an empty list.
    let Some((leader, minticks)) = sorted
        .iter()
        .enumerate()
        .map(|(i, gts)| (i, gts.size()))
        .reduce(|best, candidate| {
            if candidate.1 < best.1 {
                candidate
            } else {
                best
            }
        })
    else {
        return Err(Error::new("Index -1 out of bounds for length 0"));
    };
    let mut result: Vec<GeoTimeSerie> = sorted.iter().map(|gts| gts.clone_empty()).collect();
    if minticks == 0 {
        return Ok(result);
    }
    let mut idx = vec![0usize; sorted.len()];
    loop {
        let target = sorted[leader].ticks[idx[leader]];
        let mut matched = true;
        for i in 0..sorted.len() {
            if i == leader {
                continue;
            }
            let serie = &sorted[i];
            while idx[i] < serie.size() && serie.ticks[idx[i]] < target {
                idx[i] += 1;
            }
            if idx[i] >= serie.size() {
                return Ok(result);
            }
            if serie.ticks[idx[i]] > target {
                let bound = serie.ticks[idx[i]];
                let lead = &sorted[leader];
                while idx[leader] < lead.size() && lead.ticks[idx[leader]] < bound {
                    idx[leader] += 1;
                }
                if idx[leader] >= lead.size() {
                    return Ok(result);
                }
                matched = false;
                break;
            }
        }
        if matched {
            for i in 0..sorted.len() {
                let serie = &sorted[i];
                let mut tidx = idx[i];
                while tidx < serie.size() && serie.ticks[tidx] == target {
                    result[i].copy_datapoint(serie, tidx);
                    tidx += 1;
                }
                if tidx < serie.size() {
                    tidx -= 1;
                }
                idx[i] = tidx;
            }
            idx[leader] += 1;
            if idx[leader] >= sorted[leader].size() {
                return Ok(result);
            }
        }
    }
}

/// `COMMONTICKS`: keep, in each series of a list, the datapoints whose tick is in every
/// series.
fn commonticks(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "COMMONTICKS";
    const EXPECTS: &str = "operates on a list of GTS.";
    let Value::List(list) = stack.pop()? else {
        return Err(fail(NAME, EXPECTS));
    };
    let series = list
        .borrow()
        .iter()
        .map(|element| match element {
            Value::Gts(gts) => Ok(gts.clone()),
            _ => Err(fail(NAME, EXPECTS)),
        })
        .collect::<Result<Vec<_>>>()?;
    stack.push(series_list(common_ticks(&series)?));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// VALUESORT, RVALUESORT, LASTSORT, METASORT
// -----------------------------------------------------------------------------------------

/// `VALUESORT` / `RVALUESORT`: sort a Geo Time Series (or each of a list) by value, in place.
fn value_sort_fn(stack: &mut Stack, name: &str, reversed: bool) -> Result<()> {
    gts_or_list(stack, name, |gts| {
        gts.borrow_mut().value_sort(reversed);
        Value::Gts(gts.clone())
    })
}

fn valuesort(stack: &mut Stack) -> Result<()> {
    value_sort_fn(stack, "VALUESORT", false)
}

fn rvaluesort(stack: &mut Stack) -> Result<()> {
    value_sort_fn(stack, "RVALUESORT", true)
}

/// Java `String.compareTo`: by UTF-16 code units.
fn java_string_compare(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Compare two maps of strings like `MetadataTextComparator` does for labels and attributes:
/// the sorted keys pairwise, each key then its value, the shorter map first.
fn compare_string_maps(
    a: &indexmap::IndexMap<String, String>,
    b: &indexmap::IndexMap<String, String>,
) -> Ordering {
    let mut keys_a: Vec<&String> = a.keys().collect();
    let mut keys_b: Vec<&String> = b.keys().collect();
    keys_a.sort_by(|x, y| java_string_compare(x, y));
    keys_b.sort_by(|x, y| java_string_compare(x, y));
    for (key_a, key_b) in keys_a.iter().zip(&keys_b) {
        let comp = java_string_compare(key_a, key_b);
        if comp != Ordering::Equal {
            return comp;
        }
        let comp = java_string_compare(&a[*key_a], &b[*key_b]);
        if comp != Ordering::Equal {
            return comp;
        }
    }
    a.len().cmp(&b.len())
}

/// `MetadataTextComparator` without fields: name, then labels, then attributes.
fn compare_metadata(a: &Metadata, b: &Metadata) -> Ordering {
    match (&a.name, &b.name) {
        (None, Some(_)) => return Ordering::Less,
        (Some(_), None) => return Ordering::Greater,
        (Some(name_a), Some(name_b)) => {
            let comp = java_string_compare(name_a, name_b);
            if comp != Ordering::Equal {
                return comp;
            }
        }
        (None, None) => {}
    }
    if a.labels.is_empty() != b.labels.is_empty() {
        return a.labels.len().cmp(&b.labels.len());
    }
    let comp = compare_string_maps(&a.labels, &b.labels);
    if comp != Ordering::Equal {
        return comp;
    }
    if a.attributes.is_empty() || b.attributes.is_empty() {
        return a.attributes.len().cmp(&b.attributes.len());
    }
    compare_string_maps(&a.attributes, &b.attributes)
}

/// `MetadataTextComparator.compareWithFields`: field by field, a `null` field standing for
/// the name, the others looked up in the labels (overlaid on the attributes on request);
/// a missing field sorts first.
fn compare_metadata_fields(
    a: &Metadata,
    b: &Metadata,
    fields: &[Option<String>],
    consider_attributes: bool,
) -> Ordering {
    let lookup = |metadata: &Metadata, field: &Option<String>| -> Option<String> {
        let Some(field) = field else {
            return metadata.name.clone();
        };
        metadata
            .labels
            .get(field)
            .or_else(|| {
                if consider_attributes {
                    metadata.attributes.get(field)
                } else {
                    None
                }
            })
            .cloned()
    };
    for field in fields {
        match (lookup(a, field), lookup(b, field)) {
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(value_a), Some(value_b)) => {
                let comp = java_string_compare(&value_a, &value_b);
                if comp != Ordering::Equal {
                    return comp;
                }
            }
            (None, None) => {}
        }
    }
    Ordering::Equal
}

/// The list on top of the stack (left in place) and its Geo Time Series, for the in-place
/// sorts.
fn peek_series(stack: &mut Stack, name: &str) -> Result<(List, Vec<Gts>)> {
    const EXPECTS: &str = "operates on a list of Geo Time Series.";
    let top = stack.pop()?;
    stack.push(top.clone());
    let Value::List(list) = top else {
        return Err(fail(name, EXPECTS));
    };
    let series = list
        .borrow()
        .iter()
        .map(|element| match element {
            Value::Gts(gts) => Ok(gts.clone()),
            _ => Err(fail(name, EXPECTS)),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((list, series))
}

/// Replace the elements of `list` by `series`.
fn store_series(list: &List, series: Vec<Gts>) {
    *list.borrow_mut() = series.into_iter().map(Value::Gts).collect();
}

/// `LASTSORT.LAST_COMPARATOR`: by most recent value, then most recent tick first, then
/// metadata; empty series last. Every compared series is sorted in reverse order in place.
fn compare_last(a: &Gts, b: &Gts) -> Ordering {
    if a.borrow().size() == 0 {
        return if b.borrow().size() == 0 {
            compare_metadata(&a.borrow().metadata, &b.borrow().metadata)
        } else {
            Ordering::Greater
        };
    }
    if b.borrow().size() == 0 {
        return Ordering::Less;
    }
    a.borrow_mut().sort(true);
    b.borrow_mut().sort(true);
    let (a, b) = (a.borrow(), b.borrow());
    let comp = match (a.value_at(0), b.value_at(0)) {
        (Value::Long(x), Value::Long(y)) => x.cmp(&y),
        (Value::Double(x), Value::Double(y)) => java_double_compare(x, y),
        (Value::String(x), Value::String(y)) => java_string_compare(&x, &y),
        (Value::Boolean(x), Value::Boolean(y)) => x.cmp(&y),
        (Value::Long(x), Value::Double(y)) => java_double_compare(x as f64, y),
        (Value::Double(x), Value::Long(y)) => java_double_compare(x, y as f64),
        (x, y) => java_string_compare(&x.to_string(), &y.to_string()),
    };
    if comp != Ordering::Equal {
        return comp;
    }
    let comp = b.ticks[0].cmp(&a.ticks[0]);
    if comp != Ordering::Equal {
        return comp;
    }
    compare_metadata(&a.metadata, &b.metadata)
}

/// `LASTSORT`: sort a list of Geo Time Series (in place, left on the stack) by their most
/// recent value.
fn lastsort(stack: &mut Stack) -> Result<()> {
    let (list, mut series) = peek_series(stack, "LASTSORT")?;
    series.sort_by(compare_last);
    store_series(&list, series);
    Ok(())
}

/// `METASORT`: sort a list of Geo Time Series (in place, left on the stack) by metadata,
/// along a list of fields (`null` for the name) or, when the list is empty, by name, labels
/// and attributes; a boolean on top lets the fields also match attributes.
fn metasort(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "METASORT";
    let mut top = stack.pop()?;
    let mut consider_attributes = false;
    if let Value::Boolean(b) = top {
        consider_attributes = b;
        top = stack.pop()?;
    }
    let Value::List(fields) = top else {
        return Err(fail(
            NAME,
            "expects a list of fields to consider for sorting on top of the stack.",
        ));
    };
    let fields = fields
        .borrow()
        .iter()
        .map(|field| match field {
            Value::Null => Ok(None),
            Value::String(s) => Ok(Some(s.to_string())),
            _ => Err(fail(NAME, "expects a list of fields (strings).")),
        })
        .collect::<Result<Vec<_>>>()?;
    let (list, mut series) = peek_series(stack, NAME)?;
    if fields.is_empty() {
        series.sort_by(|a, b| compare_metadata(&a.borrow().metadata, &b.borrow().metadata));
    } else {
        series.sort_by(|a, b| {
            compare_metadata_fields(
                &a.borrow().metadata,
                &b.borrow().metadata,
                &fields,
                consider_attributes,
            )
        });
    }
    store_series(&list, series);
    Ok(())
}

// -----------------------------------------------------------------------------------------
// INTEGRATE, RESETS, MONOTONIC
// -----------------------------------------------------------------------------------------

/// Java `((Number) value).doubleValue()` on a datapoint value.
fn number_value(value: &Value) -> Result<f64> {
    value.as_f64().ok_or_else(|| {
        Error::new(format!(
            "class {} cannot be cast to class java.lang.Number",
            value.java_class()
        ))
    })
}

/// `GTSHelper.integrate`: the series (sorted in place) integrated as rates of change per
/// second from an initial value; an empty series yields the initial value at
/// `Long.MIN_VALUE`.
fn integrate_gts(gts: &mut GeoTimeSerie, initial_value: f64) -> Result<GeoTimeSerie> {
    let mut integrated = gts.clone_empty();
    gts.sort(false);
    let mut value = initial_value;
    integrated.set_value(
        gts.tick_at(0),
        gts.location_at(0),
        gts.elevation_at(0),
        &Value::Double(value),
        false,
    );
    for i in 1..gts.size() {
        let delta_t =
            gts.ticks[i].wrapping_sub(gts.ticks[i - 1]) as f64 / (TIME_UNITS_PER_MS * 1000) as f64;
        let rate_of_change = number_value(&gts.value_at(i - 1))?;
        value += rate_of_change * delta_t;
        integrated.set_value(
            gts.ticks[i],
            gts.location_at(i),
            gts.elevation_at(i),
            &Value::Double(value),
            false,
        );
    }
    Ok(integrated)
}

/// `INTEGRATE`: integrate a Geo Time Series (or each of a list) from an initial value.
fn integrate(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "INTEGRATE";
    let Some(initial_value) = stack.pop()?.as_f64() else {
        return Err(fail(
            NAME,
            "expects a numeric initial value on top of the stack.",
        ));
    };
    super::gts::gts_or_list_try(stack, NAME, |gts| {
        Ok(Value::Gts(
            integrate_gts(&mut gts.borrow_mut(), initial_value)?.shared(),
        ))
    })
}

/// `GTSHelper.compensateResets`: a sorted copy of a numeric series where every reset (a
/// value lower, or higher when `reset_higher`, than the previous one) adds the previous
/// value to an offset applied to the following values. Warp 10 reads the first value of
/// the copy before looking at its size, hence the failure on a numeric series without
/// datapoint.
fn compensate_resets(gts: &GeoTimeSerie, reset_higher: bool) -> Result<GeoTimeSerie> {
    let mut filled = gts.clone();
    filled.sort(false);
    match &mut filled.values {
        Values::Long(values) => {
            let Some(&first) = values.first() else {
                return Err(Error::new("Index 0 out of bounds for length 0"));
            };
            let mut last = first;
            let mut offset = 0i64;
            for value in values.iter_mut().skip(1) {
                let v = *value;
                if (!reset_higher && v < last) || (reset_higher && v > last) {
                    offset = offset.wrapping_add(last);
                }
                last = v;
                *value = v.wrapping_add(offset);
            }
        }
        Values::Double(values) => {
            let Some(&first) = values.first() else {
                return Err(Error::new("Index 0 out of bounds for length 0"));
            };
            let mut last = first;
            let mut offset = 0.0;
            for value in values.iter_mut().skip(1) {
                let v = *value;
                if (!reset_higher && v < last) || (reset_higher && v > last) {
                    offset += last;
                }
                last = v;
                *value = v + offset;
            }
        }
        _ => {}
    }
    Ok(filled)
}

/// `RESETS`: compensate the resets of a Geo Time Series or of every series of a (nested)
/// list; the boolean tells whether a reset is a jump up rather than down.
fn resets(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "RESETS";
    let Value::Boolean(decreasing) = stack.pop()? else {
        return Err(fail(NAME, "expects a boolean as parameter."));
    };
    let top = stack.pop()?;
    match &top {
        Value::Gts(gts) => {
            stack.push(Value::Gts(
                compensate_resets(&gts.borrow(), decreasing)?.shared(),
            ));
        }
        Value::List(params) => {
            let series = flatten_series(
                NAME,
                &params.borrow(),
                "expects a list of Geo Time Series as first parameter.",
            )?;
            let result = series
                .iter()
                .map(|gts| compensate_resets(&gts.borrow(), decreasing))
                .collect::<Result<Vec<_>>>()?;
            stack.push(series_list(result));
        }
        _ => {
            return Err(fail(NAME, "operates on a Geo Time Series or list thereof."));
        }
    }
    Ok(())
}

/// `MONOTONIC`: the running maximum (or minimum when the boolean is `true`) of the values of
/// a Geo Time Series (sorted in place), or of each of a list.
fn monotonic(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MONOTONIC";
    let Value::Boolean(decreasing) = stack.pop()? else {
        return Err(fail(NAME, "expects a boolean on top of the stack."));
    };
    gts_or_list(stack, NAME, |gts| {
        let mut gts = gts.borrow_mut();
        gts.sort(false);
        let mut monotonic = gts.clone_empty();
        let mut lvalue = if decreasing { i64::MAX } else { i64::MIN };
        let mut dvalue = if decreasing {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
        let mut svalue: Option<String> = if decreasing {
            None
        } else {
            Some(String::new())
        };
        let mut bvalue = decreasing;
        for i in 0..gts.size() {
            let value = match gts.value_at(i) {
                Value::Long(lv) => {
                    if (decreasing && lv < lvalue) || (!decreasing && lv > lvalue) {
                        lvalue = lv;
                    }
                    Value::Long(lvalue)
                }
                Value::Double(dv) => {
                    if (decreasing && dv < dvalue) || (!decreasing && dv > dvalue) {
                        dvalue = dv;
                    }
                    Value::Double(dvalue)
                }
                Value::String(sv) => {
                    let better = match &svalue {
                        None => true,
                        Some(current) => {
                            let comp = java_string_compare(&sv, current);
                            if decreasing {
                                comp == Ordering::Less
                            } else {
                                comp == Ordering::Greater
                            }
                        }
                    };
                    if better {
                        svalue = Some(sv.to_string());
                    }
                    Value::from(svalue.clone().unwrap_or_default())
                }
                other => {
                    let bv = other == Value::Boolean(true);
                    if bv != decreasing {
                        bvalue = bv;
                    }
                    Value::Boolean(bvalue)
                }
            };
            monotonic.set_value(
                gts.tick_at(i),
                gts.location_at(i),
                gts.elevation_at(i),
                &value,
                false,
            );
        }
        Value::Gts(monotonic.shared())
    })
}

// -----------------------------------------------------------------------------------------
// ATBUCKET
// -----------------------------------------------------------------------------------------

/// `ATBUCKET`: the `[ tick lat lon elevation value ]` datapoint at a bucket index of a
/// bucketized Geo Time Series (sorted in place), `NaN`s and `null` when the bucket is empty.
fn atbucket(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ATBUCKET";
    let Some(idx) = long_value(&stack.pop()?) else {
        return Err(fail(NAME, "expects an index on top of the stack."));
    };
    let idx = idx as i32;
    let Value::Gts(gts) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a Geo Time Series instance on top of the stack.",
        ));
    };
    let mut gts = gts.borrow_mut();
    if !gts.is_bucketized() {
        return Err(fail(
            NAME,
            "expects the Geo Time Series on top of the stack to be bucketized.",
        ));
    }
    let bucketcount = gts.bucketcount as i32;
    if idx >= bucketcount {
        return Err(fail(
            NAME,
            &format!("cannot retrieve bucket {idx} of a GTS with {bucketcount} buckets."),
        ));
    }
    gts.sort(false);
    let tick = gts.lastbucket.wrapping_sub(
        i64::from(bucketcount.wrapping_sub(1).wrapping_sub(idx)).wrapping_mul(gts.bucketspan),
    );
    let index = crate::gts::java_binary_search(&gts.ticks, tick);
    let tuple = tuple_at_index(&gts, index);
    stack.push(tuple);
    Ok(())
}
