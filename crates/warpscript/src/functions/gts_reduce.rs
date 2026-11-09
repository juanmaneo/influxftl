// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `PARTITION` / `STRICTPARTITION` (`GTSHelper.partition`) and the `REDUCE` / `PREDUCE`
//! framework (`GTSHelper.reduceUnflattened`): the series are grouped into equivalence classes
//! by label values, then the reducer is applied tick by tick to the datapoints of each class.
//!
//! Warp 10 collects the classes in `HashMap`s, so their order is Java's hash order; it is
//! reproduced here from the Java hash codes of the label maps.

use indexmap::IndexMap;

use super::Entry;
use super::gts_frameworks::{Mapped, aggregate};
use crate::aggregators::{Aggregator, AggregatorArgs, aggregator_of};
use crate::error::{Class, Error, Result};
use crate::gts::{GeoTimeSerie, Gts, NO_ELEVATION, NO_LOCATION};
use crate::stack::Stack;
use crate::value::{Value, ValueKind};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "PARTITION",
        Entry::Stack(|s| partition_fn(s, "PARTITION", false)),
    ),
    (
        "STRICTPARTITION",
        Entry::Stack(|s| partition_fn(s, "STRICTPARTITION", true)),
    ),
    ("REDUCE", Entry::Stack(|s| reduce(s, "REDUCE", true))),
    ("PREDUCE", Entry::Stack(|s| reduce(s, "PREDUCE", false))),
];

/// A label map.
pub(super) type Labels = IndexMap<String, String>;

/// `String.hashCode()`.
pub(super) fn java_string_hash(s: &str) -> i32 {
    s.encode_utf16()
        .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(c)))
}

/// `Map.hashCode()` of a label map: the sum of `key.hashCode() ^ value.hashCode()`.
fn java_labels_hash(labels: &Labels) -> i32 {
    labels.iter().fold(0i32, |h, (k, v)| {
        h.wrapping_add(java_string_hash(k) ^ java_string_hash(v))
    })
}

/// The iteration order of a `java.util.HashMap` of initial table size `capacity` (a power
/// of two) into which keys of hash codes `hashes` were put in that order: by bucket (the
/// spread hash modulo the final table size, which doubles whenever the size exceeds three
/// quarters of it), then by insertion. Buckets of eight colliding keys or more, which Java
/// turns into trees, are not modelled.
pub(super) fn java_hash_order(hashes: &[i32], mut capacity: usize) -> Vec<usize> {
    while hashes.len() > capacity * 3 / 4 {
        capacity *= 2;
    }
    let mut order: Vec<usize> = (0..hashes.len()).collect();
    order.sort_by_key(|&i| {
        let h = hashes[i] as u32;
        (h ^ (h >> 16)) as usize & (capacity - 1)
    });
    order
}

/// `HashMap.tableSizeFor`: the table size of `new HashMap(n)`.
pub(super) fn table_size_for(n: usize) -> usize {
    n.max(1).next_power_of_two()
}

/// `GTSHelper.partition`: the equivalence classes of `series` by the values of `bylabels`
/// (all the labels when `None`), each keyed by the labels its members have in common, in
/// Java's hash order.
pub(super) fn partition(series: &[Gts], bylabels: Option<&[String]>) -> Vec<(Labels, Vec<Gts>)> {
    // (class key, common labels, members), in insertion order.
    let mut classes: Vec<(Labels, Labels, Vec<Gts>)> = Vec::new();
    let mut lookup: std::collections::HashMap<Vec<(String, String)>, usize> =
        std::collections::HashMap::new();
    for gts in series {
        let labels = gts.borrow().metadata.labels.clone();
        let key: Labels = match bylabels {
            None => labels.clone(),
            Some(names) => {
                let mut key = Labels::new();
                for name in names {
                    if let Some(value) = labels.get(name) {
                        key.insert(name.clone(), value.clone());
                    }
                }
                key
            }
        };
        let mut canonical: Vec<(String, String)> =
            key.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        canonical.sort();
        match lookup.get(&canonical) {
            Some(&index) => {
                let (_, common, members) = &mut classes[index];
                members.push(gts.clone());
                // Only the labels every member has, with the same value, are common.
                common.retain(|k, v| labels.get(k) == Some(v));
            }
            None => {
                lookup.insert(canonical, classes.len());
                classes.push((key, labels, vec![gts.clone()]));
            }
        }
    }
    // `classes` iterates in hash order of the class keys (a default `HashMap`), the result
    // (`new HashMap(classes.size())`) in hash order of the common labels.
    let key_hashes: Vec<i32> = classes.iter().map(|c| java_labels_hash(&c.0)).collect();
    let mut by_key: Vec<Option<(Labels, Labels, Vec<Gts>)>> =
        classes.into_iter().map(Some).collect();
    let ordered: Vec<(Labels, Vec<Gts>)> = java_hash_order(&key_hashes, 16)
        .into_iter()
        .filter_map(|i| by_key[i].take())
        .map(|(_, common, members)| (common, members))
        .collect();
    rehash(ordered)
}

/// The label names of a partitioning list. Java casts them to `String` while partitioning the
/// first series, so a list holding another type fails only when there are series.
fn label_names(labels: &[Value], has_series: bool) -> Result<Vec<String>> {
    labels
        .iter()
        .filter_map(|label| match label {
            Value::String(name) => Some(Ok(name.to_string())),
            _ if !has_series => None,
            other => Some(Err(Error::class_cast(other.kind(), ValueKind::String))),
        })
        .collect()
}

/// The entries put, in this order, into a `new HashMap(entries.size())`, in its iteration
/// order.
fn rehash<T>(entries: Vec<(Labels, T)>) -> Vec<(Labels, T)> {
    let hashes: Vec<i32> = entries.iter().map(|(k, _)| java_labels_hash(k)).collect();
    let capacity = table_size_for(entries.len());
    let mut slots: Vec<Option<(Labels, T)>> = entries.into_iter().map(Some).collect();
    java_hash_order(&hashes, capacity)
        .into_iter()
        .filter_map(|i| slots[i].take())
        .collect()
}

/// A label map as a WarpScript map.
pub(super) fn labels_value(labels: &Labels) -> Value {
    Value::map(
        labels
            .iter()
            .map(|(k, v)| (Value::from(k.as_str()), Value::from(v.as_str())))
            .collect(),
    )
}

/// The equivalence classes as a map of label maps to lists of series.
pub(super) fn classes_value(classes: Vec<(Labels, Vec<Gts>)>) -> Value {
    Value::map(
        classes
            .into_iter()
            .map(|(labels, members)| {
                (
                    labels_value(&labels),
                    Value::list(members.into_iter().map(Value::Gts).collect()),
                )
            })
            .collect(),
    )
}

/// `PARTITION` / `STRICTPARTITION`: `[ series ] [ labels ]|NULL PARTITION`. The strict form
/// keys the classes by the partitioning labels only.
fn partition_fn(stack: &mut Stack, name: &str, strict: bool) -> Result<()> {
    let bylabels = match stack.pop()? {
        Value::Null => None,
        Value::List(list) => Some(list.borrow().clone()),
        other => {
            return Err(Error::with_cause(
                format!("{name} expects a list of labels or null on the top of the stack."),
                Error::class_cast(other.kind(), Class::List),
            ));
        }
    };
    let params = match stack.pop()? {
        Value::List(params) => params,
        // `params.size()` on the `null` the cast lets through.
        Value::Null => return Err(Error::null_pointer()),
        other => {
            return Err(Error::with_cause(
                format!(
                    "{name} expects a list of Geo Time Series instances under the top of the stack."
                ),
                Error::class_cast(other.kind(), Class::List),
            ));
        }
    };
    let mut series = Vec::new();
    for param in params.borrow().iter() {
        match param {
            Value::Gts(gts) => series.push(gts.clone()),
            Value::List(list) => {
                for item in list.borrow().iter() {
                    let Value::Gts(gts) = item else {
                        return Err(Error::usage(
                            name,
                            "expects a list of Geo Time Series as first parameter.",
                        ));
                    };
                    series.push(gts.clone());
                }
            }
            // Other elements are ignored.
            _ => {}
        }
    }
    let bylabels = bylabels
        .map(|labels| label_names(&labels, !series.is_empty()))
        .transpose()?;
    let mut classes = partition(&series, bylabels.as_deref());
    if let (true, Some(bylabels)) = (strict, &bylabels) {
        // The class keys restricted to the partitioning labels, in a default `HashMap`.
        let restricted: Vec<(Labels, Vec<Gts>)> = classes
            .into_iter()
            .map(|(labels, members)| {
                let key: Labels = bylabels
                    .iter()
                    .filter_map(|name| labels.get(name).map(|value| (name.clone(), value.clone())))
                    .collect();
                (key, members)
            })
            .collect();
        let hashes: Vec<i32> = restricted
            .iter()
            .map(|(k, _)| java_labels_hash(k))
            .collect();
        let mut slots: Vec<Option<(Labels, Vec<Gts>)>> = restricted.into_iter().map(Some).collect();
        classes = java_hash_order(&hashes, 16)
            .into_iter()
            .filter_map(|i| slots[i].take())
            .collect();
    }
    stack.push(classes_value(classes));
    Ok(())
}

/// `GTSHelper.commonAttributes`: the attributes (key and value) all the series have, none as
/// soon as one series has none.
fn common_attributes(series: &[Gts]) -> Labels {
    let mut common = Labels::new();
    for (i, gts) in series.iter().enumerate() {
        let attributes = &gts.borrow().metadata.attributes;
        if attributes.is_empty() {
            common.clear();
            break;
        }
        if i == 0 {
            common.clone_from(attributes);
        } else {
            common.retain(|k, v| attributes.get(k) == Some(v));
            if common.is_empty() {
                break;
            }
        }
    }
    common
}

/// The bucketization of the result of a class (`lastbucket`, `bucketcount`, `bucketspan`):
/// the series must all be bucketized with the same span and congruent last buckets; the
/// result covers all their buckets. `None` otherwise.
fn result_buckets(series: &[Gts]) -> Option<(i64, i64, i64)> {
    let (mut lastbucket, mut startbucket, mut bucketspan) = (i64::MIN, i64::MAX, 0i64);
    for gts in series {
        let gts = gts.borrow();
        if !gts.is_bucketized() {
            return None;
        }
        if bucketspan == 0 {
            bucketspan = gts.bucketspan;
        } else if bucketspan != gts.bucketspan {
            return None;
        }
        if lastbucket == i64::MIN {
            lastbucket = gts.lastbucket;
        }
        if lastbucket.wrapping_rem(bucketspan) != gts.lastbucket.wrapping_rem(gts.bucketspan) {
            return None;
        }
        lastbucket = lastbucket.max(gts.lastbucket);
        startbucket = startbucket.min(
            gts.lastbucket
                .wrapping_sub(gts.bucketcount.wrapping_mul(gts.bucketspan)),
        );
    }
    // An empty class is not bucketized either.
    (bucketspan != 0).then(|| {
        // `(int) ((lastbucket - startbucket) / bucketspan)`
        let bucketcount = lastbucket
            .wrapping_sub(startbucket)
            .wrapping_div(bucketspan) as i32;
        (lastbucket, i64::from(bucketcount), bucketspan)
    })
}

/// A new (possibly bucketized) result series.
fn new_result(buckets: Option<(i64, i64, i64)>) -> GeoTimeSerie {
    match buckets {
        Some((lastbucket, bucketcount, bucketspan)) => {
            GeoTimeSerie::with_buckets(lastbucket, bucketcount, bucketspan)
        }
        None => GeoTimeSerie::new(),
    }
}

/// `GTSHelper.reduceUnflattened` for one class: the reducer applied to the datapoints of the
/// series at each of their ticks (`null` for the series without a datapoint at that tick).
/// The series are sorted in place.
fn reduce_class(
    stack: &mut Stack,
    reducer: &dyn Aggregator,
    class_labels: &Labels,
    series: &[Gts],
    override_tick: bool,
) -> Result<Vec<Gts>> {
    let buckets = result_buckets(series);
    let mut result = new_result(buckets);
    result.metadata.labels.clone_from(class_labels);
    result.metadata.attributes = common_attributes(series);
    let mut result_name: Option<Option<String>> = None;
    for gts in series {
        gts.borrow_mut().sort(false);
        let name = gts.borrow().metadata.name.clone();
        result_name = match result_name {
            // A `null` first name lets the next one decide, as in Java.
            None | Some(None) => Some(name),
            Some(current) if current == name => Some(current),
            Some(_) => Some(Some(String::new())),
        };
    }
    result.metadata.name = result_name.flatten();
    // Copies, so that no borrow is held while a macro reducer (which may reach them) runs.
    let data: Vec<GeoTimeSerie> = series.iter().map(|gts| gts.borrow().clone()).collect();
    let names: Vec<String> = data
        .iter()
        .map(|gts| gts.name().unwrap_or_default().to_owned())
        .collect();
    let mut labels: Vec<Labels> = data.iter().map(|gts| gts.metadata.labels.clone()).collect();
    labels.push(class_labels.clone());
    let n = data.len();
    let mut idx = vec![0usize; n];
    let mut single = false;
    let mut multiple: Vec<(String, GeoTimeSerie)> = Vec::new();
    let (mut ticks, mut locations, mut elevations, mut values) =
        (vec![0; n], vec![0; n], vec![0; n], vec![Value::Null; n]);
    loop {
        let smallest = (0..n)
            .filter(|&i| idx[i] < data[i].size())
            .map(|i| data[i].ticks[idx[i]])
            .min()
            .unwrap_or(i64::MAX);
        // A tick at `Long.MAX_VALUE` is never the smallest.
        if smallest == i64::MAX {
            break;
        }
        for i in 0..n {
            let gts = &data[i];
            if idx[i] < gts.size() && gts.ticks[idx[i]] == smallest {
                ticks[i] = smallest;
                locations[i] = gts.location_at(idx[i]);
                elevations[i] = gts.elevation_at(idx[i]);
                values[i] = gts.value_at(idx[i]);
                idx[i] += 1;
            } else {
                ticks[i] = i64::MIN;
                locations[i] = NO_LOCATION;
                elevations[i] = NO_ELEVATION;
                values[i] = Value::Null;
            }
        }
        let args = AggregatorArgs {
            tick: smallest,
            names: &names,
            labels: &labels,
            ticks: &ticks,
            locations: &locations,
            elevations: &elevations,
            values: &values,
            window: None,
        };
        match aggregate(stack, reducer, &args)? {
            Mapped::Single(reduced) => {
                single = true;
                let datapoint = &reduced.datapoint;
                if !datapoint.value.is_null() {
                    result.set_value(
                        reduced.tick(override_tick, smallest)?,
                        datapoint.location,
                        datapoint.elevation,
                        &datapoint.value,
                        false,
                    );
                }
            }
            Mapped::Multiple(outputs) => {
                for (key, reduced) in outputs {
                    let position = match multiple.iter().position(|(k, _)| *k == key) {
                        Some(position) => position,
                        None => {
                            let mut gts = new_result(buckets);
                            gts.metadata.name = Some(key.clone());
                            gts.metadata.labels.clone_from(class_labels);
                            multiple.push((key, gts));
                            multiple.len() - 1
                        }
                    };
                    let datapoint = &reduced.datapoint;
                    if !datapoint.value.is_null() {
                        multiple[position].1.set_value(
                            reduced.tick(override_tick, smallest)?,
                            datapoint.location,
                            datapoint.elevation,
                            &datapoint.value,
                            false,
                        );
                    }
                }
            }
        }
    }
    let mut results = Vec::new();
    if single {
        results.push(result.shared());
    }
    // Named outputs in `String.compareTo` order (a `TreeMap` in Warp 10).
    multiple.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
    results.extend(multiple.into_iter().map(|(_, gts)| gts.shared()));
    Ok(results)
}

/// `REDUCE` / `PREDUCE`: `[ [ series ]+ labels|NULL reducer (override) ] REDUCE`. `REDUCE`
/// pushes the list of the results of all the classes, `PREDUCE` the map of the labels of
/// each class to its results.
fn reduce(stack: &mut Stack, name: &str, flatten: bool) -> Result<()> {
    let Value::List(params) = stack.pop()? else {
        return Err(Error::usage(name, "expects a list as input."));
    };
    let params = params.borrow().clone();
    let size = params.len();
    if size < 3 {
        return Err(Error::usage(name, "expects at least 3 parameters."));
    }
    let not_series = || Error::usage(name, "expects lists of Geo Time Series as first parameter.");
    // All but the last two parameters must be lists, even when the last is the override.
    let mut lists = Vec::with_capacity(size - 2);
    for param in &params[..size - 2] {
        let Value::List(list) = param else {
            return Err(not_series());
        };
        lists.push(list.clone());
    }
    let (mut labels_index, mut reducer_index) = (size - 2, size - 1);
    let mut override_tick = false;
    if let Value::Boolean(b) = params[size - 1] {
        labels_index -= 1;
        reducer_index -= 1;
        override_tick = b;
    }
    let bylabels = match &params[labels_index] {
        Value::Null => None,
        Value::List(list) => Some(list.borrow().clone()),
        Value::Set(set) => Some(set.borrow().iter().cloned().collect()),
        _ => {
            return Err(Error::usage(
                name,
                format!(
                    "expects a list of label names or null as parameter number {}.",
                    labels_index + 1
                ),
            ));
        }
    };
    if bylabels
        .as_ref()
        .is_some_and(|labels| labels.iter().any(|l| !matches!(l, Value::String(_))))
    {
        return Err(Error::usage(
            name,
            format!(
                "expects a list of label names as parameter number {}.",
                labels_index + 1
            ),
        ));
    }
    let reducer_value = &params[reducer_index];
    let Some(reducer) = aggregator_of(reducer_value).filter(|a| a.roles().reducer) else {
        return Err(Error::usage(
            name,
            format!(
                "expects a function as parameter number {}.",
                reducer_index + 1
            ),
        ));
    };
    let mut series = Vec::new();
    for list in &lists[..labels_index] {
        for item in list.borrow().iter() {
            let Value::Gts(gts) = item else {
                return Err(not_series());
            };
            series.push(gts.clone());
        }
    }
    // The label names were checked to be strings.
    let bylabels: Option<Vec<String>> =
        bylabels.map(|labels| labels.iter().map(ToString::to_string).collect());
    let classes = partition(&series, bylabels.as_deref());
    let mut reduced = Vec::with_capacity(classes.len());
    for (labels, members) in classes {
        let results = reduce_class(stack, reducer, &labels, &members, override_tick)?;
        reduced.push((labels, results));
    }
    if flatten {
        stack.push(Value::list(
            reduced
                .into_iter()
                .flat_map(|(_, results)| results.into_iter().map(Value::Gts))
                .collect(),
        ));
    } else {
        stack.push(classes_value(reduced));
    }
    Ok(())
}
