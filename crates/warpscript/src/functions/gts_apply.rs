// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `APPLY` / `PAPPLY` (`GTSHelper.partitionAndApplyUnflattened`, `applyNAryFunction`): the
//! series of several collections are partitioned by label values and an `op.*` operation is
//! applied tick by tick to the series of each class, one per collection (a collection of a
//! single series takes part in every class).

use std::rc::Rc;

use super::Entry;
use super::gts_reduce::{Labels, classes_value, partition};
use crate::aggregators::{Aggregator, AggregatorArgs, aggregator_of};
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Gts, NO_ELEVATION, NO_LOCATION};
use crate::macros::Macro;
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("APPLY", Entry::Stack(|s| apply(s, "APPLY", true))),
    ("PAPPLY", Entry::Stack(|s| apply(s, "PAPPLY", false))),
];

/// The bucketization of the result of `applyNAryFunction` (`lastbucket`, `bucketcount`,
/// `bucketspan`, all zero when not bucketized): the bucketized series (the others before the
/// first bucketized one are skipped, one after it cancels the bucketization) must have the
/// same span and congruent last buckets.
fn result_buckets(subseries: &[Vec<Gts>]) -> (i64, i64, i64) {
    let (mut lastbucket, mut firstbucket, mut bucketspan) = (0i64, i64::MAX, 0i64);
    for gts in subseries.iter().flatten() {
        let gts = gts.borrow();
        let bucketized = gts.is_bucketized();
        if !bucketized && bucketspan > 0 {
            return (0, 0, 0);
        }
        if !bucketized {
            continue;
        }
        if bucketspan > 0 && gts.bucketspan != bucketspan {
            return (0, 0, 0);
        } else if bucketspan == 0 {
            bucketspan = gts.bucketspan;
            lastbucket = gts.lastbucket;
        }
        if gts.lastbucket.wrapping_rem(bucketspan) != lastbucket.wrapping_rem(bucketspan) {
            return (0, 0, 0);
        }
        lastbucket = lastbucket.max(gts.lastbucket);
        firstbucket = firstbucket.min(
            gts.lastbucket
                .wrapping_sub(gts.bucketcount.wrapping_mul(bucketspan)),
        );
    }
    if bucketspan == 0 {
        return (0, 0, 0);
    }
    // `(int) ((lastbucket - firstbucket) / bucketspan)`
    let bucketcount = lastbucket
        .wrapping_sub(firstbucket)
        .wrapping_div(bucketspan) as i32;
    (lastbucket, i64::from(bucketcount), bucketspan)
}

/// `GTSHelper.applyNAryFunction`: the operation applied at each tick of the series of a class
/// to their values (`null` for the series without a datapoint at that tick), the index of the
/// collection of each series in place of its tick. The series are sorted in place; the result
/// is named after the last series.
fn apply_nary(op: &dyn Aggregator, labels: &Labels, subseries: &[Vec<Gts>]) -> Result<Gts> {
    let (lastbucket, bucketcount, bucketspan) = result_buckets(subseries);
    let mut result = GeoTimeSerie::with_buckets(lastbucket, bucketcount, bucketspan);
    result.metadata.name = Some(String::new());
    result.metadata.labels.clone_from(labels);
    let mut data: Vec<GeoTimeSerie> = Vec::new();
    let mut collections: Vec<i64> = Vec::new();
    for (i, members) in subseries.iter().enumerate() {
        for gts in members {
            gts.borrow_mut().sort(false);
            let gts = gts.borrow();
            result.metadata.name.clone_from(&gts.metadata.name);
            data.push(gts.clone());
            collections.push(i as i64);
        }
    }
    let names: Vec<String> = data
        .iter()
        .map(|gts| gts.name().unwrap_or_default().to_owned())
        .collect();
    let mut all_labels: Vec<Labels> = data.iter().map(|gts| gts.metadata.labels.clone()).collect();
    all_labels.push(labels.clone());
    let n = data.len();
    let mut idx = vec![0usize; n];
    let (mut locations, mut elevations, mut values) =
        (vec![0; n], vec![0; n], vec![Value::Null; n]);
    loop {
        let smallest = (0..n)
            .filter(|&k| idx[k] < data[k].size())
            .map(|k| data[k].ticks[idx[k]])
            .min()
            .unwrap_or(i64::MAX);
        if smallest == i64::MAX {
            break;
        }
        for k in 0..n {
            let gts = &data[k];
            if idx[k] < gts.size() && gts.ticks[idx[k]] == smallest {
                locations[k] = gts.location_at(idx[k]);
                elevations[k] = gts.elevation_at(idx[k]);
                values[k] = gts.value_at(idx[k]);
                idx[k] += 1;
            } else {
                locations[k] = NO_LOCATION;
                elevations[k] = NO_ELEVATION;
                values[k] = Value::Null;
            }
        }
        let aggregated = op.apply(&AggregatorArgs {
            tick: smallest,
            names: &names,
            labels: &all_labels,
            ticks: &collections,
            locations: &locations,
            elevations: &elevations,
            values: &values,
            window: None,
        })?;
        if !aggregated.value.is_null() {
            result.set_value(
                smallest,
                aggregated.location,
                aggregated.elevation,
                &aggregated.value,
                false,
            );
        }
    }
    Ok(result.shared())
}

/// The parameters of `APPLY` around the operation: the collections of series, the
/// partitioning labels and the optional validator macro.
struct Params {
    collections: Vec<Vec<Gts>>,
    bylabels: Option<Vec<String>>,
    validator: Option<Rc<Macro>>,
}

/// Parse `[ [ series ]+ labels|NULL op (validator) ]`, the operation being at `opidx`.
fn parse(params: &[Value], opidx: usize, name: &str) -> Result<Params> {
    let labels_error = || {
        Error::new(format!(
            "{name} expects a list of label names under the operation."
        ))
    };
    let labelsidx = opidx
        .checked_sub(1)
        .filter(|&i| i >= 1)
        .ok_or_else(labels_error)?;
    let bylabels = match &params[labelsidx] {
        Value::Null => None,
        Value::List(list) => Some(list.borrow().clone()),
        Value::Set(set) => Some(set.borrow().iter().cloned().collect()),
        _ => return Err(labels_error()),
    };
    let bylabels = bylabels
        .map(|labels| {
            labels
                .iter()
                .map(|label| match label {
                    Value::String(s) => Ok(s.to_string()),
                    _ => Err(Error::new(format!(
                        "{name} expects a list of label names as penultimate parameter."
                    ))),
                })
                .collect::<Result<Vec<String>>>()
        })
        .transpose()?;
    let not_series = || {
        Error::new(format!(
            "{name} expects lists of Geo Time Series as first parameters."
        ))
    };
    let mut lists = Vec::with_capacity(labelsidx);
    for param in &params[..labelsidx] {
        let Value::List(list) = param else {
            return Err(not_series());
        };
        lists.push(list.clone());
    }
    let mut collections = Vec::with_capacity(labelsidx);
    for list in lists {
        let mut series = Vec::new();
        for item in list.borrow().iter() {
            let Value::Gts(gts) = item else {
                return Err(not_series());
            };
            series.push(gts.clone());
        }
        collections.push(series);
    }
    let validator = match params.get(opidx + 1) {
        Some(Value::Macro(m)) => Some(m.clone()),
        _ => None,
    };
    Ok(Params {
        collections,
        bylabels,
        validator,
    })
}

/// `APPLY` / `PAPPLY`: `APPLY` pushes the list of the results of all the classes, `PAPPLY`
/// the map of the labels of each class to its results.
fn apply(stack: &mut Stack, name: &str, flatten: bool) -> Result<()> {
    let Value::List(params) = stack.pop()? else {
        return Err(Error::new(format!("{name} expects a list as input.")));
    };
    let params = params.borrow().clone();
    if params.len() < 3 {
        return Err(Error::new(format!("{name} expects at least 3 parameters.")));
    }
    let Some((opidx, op)) = params
        .iter()
        .enumerate()
        .find_map(|(i, p)| aggregator_of(p).filter(|a| a.nary()).map(|a| (i, a)))
    else {
        return Err(Error::new(format!(
            "{name} expects an operation in the parameter list."
        )));
    };
    let Params {
        collections,
        bylabels,
        validator,
    } = parse(&params, opidx, name)?;
    // The series of the collections of more than one series are partitioned (each once);
    // when there are none, the first collection is.
    let mut all: Vec<Gts> = Vec::new();
    for collection in collections.iter().filter(|c| c.len() > 1) {
        for gts in collection {
            if !all.iter().any(|g| Rc::ptr_eq(g, gts)) {
                all.push(gts.clone());
            }
        }
    }
    if all.is_empty() {
        all.clone_from(&collections[0]);
    }
    // Java uses the class and labels ids to find the series; they are unset afterwards.
    for gts in collections.iter().flatten() {
        let mut gts = gts.borrow_mut();
        gts.metadata.class_id = 0;
        gts.metadata.labels_id = 0;
    }
    let classes = partition(&all, bylabels.as_deref());
    let mut results = Vec::with_capacity(classes.len());
    for (labels, members) in classes {
        let subseries: Vec<Vec<Gts>> = collections
            .iter()
            .map(|collection| {
                if collection.len() == 1 {
                    collection.clone()
                } else {
                    members
                        .iter()
                        .filter(|gts| collection.iter().any(|g| Rc::ptr_eq(g, gts)))
                        .cloned()
                        .collect()
                }
            })
            .collect();
        let mut proceed = true;
        if let Some(validator) = &validator {
            stack.push(Value::list(
                subseries
                    .iter()
                    .map(|members| Value::list(members.iter().cloned().map(Value::Gts).collect()))
                    .collect(),
            ));
            stack.push(super::gts_reduce::labels_value(&labels));
            stack.exec_macro(validator)?;
            proceed = stack.pop()? == Value::Boolean(true);
        }
        let mut result = Vec::new();
        if proceed {
            result.push(apply_nary(op, &labels, &subseries)?);
        }
        results.push((labels, result));
    }
    if flatten {
        stack.push(Value::list(
            results
                .into_iter()
                .flat_map(|(_, results)| results.into_iter().map(Value::Gts))
                .collect(),
        ));
    } else {
        stack.push(classes_value(results));
    }
    Ok(())
}
