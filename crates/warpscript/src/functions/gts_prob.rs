// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Value probabilities: `PROB`, `CPROB` (conditional probabilities of `given<sep>event`
//! values) and the `PROBABILITY` function builder.

use std::collections::HashMap;
use std::rc::Rc;

use indexmap::IndexMap;

use super::Entry;
use super::args::fail;
use super::gts::gts_or_list_try;
use super::gts_stats::value_counts;
use super::template::java_trim;
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Gts};
use crate::macros::{Callable, StackFunction, StackObject};
use crate::snapshot::value_snapshot;
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PROB", Entry::Stack(prob)),
    ("CPROB", Entry::Stack(cprob)),
    ("PROBABILITY", Entry::Stack(probability)),
];

/// Set the probability `p` at datapoint `i` of `source` in `result`.
fn set_probability(result: &mut GeoTimeSerie, source: &GeoTimeSerie, i: usize, p: f64) {
    result.set_value(
        source.ticks[i],
        source.location_at(i),
        source.elevation_at(i),
        &Value::Double(p),
        false,
    );
}

/// `GTSHelper.prob`: the probability of the value of each datapoint among the values of the
/// series (empty buckets count as `null` values but not in the total).
fn prob_gts(gts: &GeoTimeSerie) -> Value {
    let histogram = value_counts(gts);
    let n = gts.size();
    let mut result = gts.clone_empty();
    for i in 0..n {
        let count = histogram.get(&gts.value_at(i)).copied().unwrap_or_default();
        set_probability(&mut result, gts, i, count as f64 / n as f64);
    }
    Value::Gts(result.shared())
}

/// `PROB`.
fn prob(stack: &mut Stack) -> Result<()> {
    gts_or_list_try(stack, "PROB", |gts| Ok(prob_gts(&gts.borrow())))
}

/// `GTSHelper.cprob` without separator: the probability of each value.
fn cprob_values(gts: &GeoTimeSerie) -> Value {
    let mut histogram: IndexMap<Value, i64> = IndexMap::new();
    for i in 0..gts.size() {
        *histogram.entry(gts.value_at(i)).or_insert(0) += 1;
    }
    let total = gts.size() as f64;
    let mut result = gts.clone_empty();
    for i in 0..gts.size() {
        let count = histogram.get(&gts.value_at(i)).copied().unwrap_or_default();
        set_probability(&mut result, gts, i, count as f64 / total);
    }
    Value::Gts(result.shared())
}

/// The `given` and `event` parts of a `given<sep>event` value.
fn split_event<'a>(value: &'a str, separator: &str) -> Result<(&'a str, &'a str)> {
    let Some(lastsep) = value.rfind(separator) else {
        return Err(Error::new(
            "Separator not found, unable to isolate given events.",
        ));
    };
    Ok((&value[..lastsep], &value[lastsep + separator.len()..]))
}

/// `GTSHelper.cprob` with a separator: the probability of each event among the values
/// sharing its given events. The series is sorted by value in place.
fn cprob_given(gts: &Gts, separator: &str) -> Result<Value> {
    gts.borrow_mut().value_sort(false);
    let g = gts.borrow();
    let n = g.size();
    let mut result = g.clone_empty();
    // The values all have the type of the series.
    let values: Vec<Rc<str>> = (0..n)
        .filter_map(|i| match g.value_at(i) {
            Value::String(s) => Some(s),
            _ => None,
        })
        .collect();
    if values.len() < n {
        return Err(Error::new(
            "Can only compute conditional probabilities for String Geo Time Series.",
        ));
    }
    let mut idx = 0;
    while idx < n {
        let (given, _) = split_event(&values[idx], separator)?;
        let mut histogram: HashMap<&str, i64> = HashMap::new();
        let mut events = Vec::new();
        for value in &values[idx..] {
            let (given_events, event) = split_event(value, separator)?;
            if given_events != given {
                break;
            }
            *histogram.entry(java_trim(event)).or_insert(0) += 1;
            events.push(event);
        }
        let total = events.len() as f64;
        for (i, event) in (idx..).zip(&events) {
            // Warp 10 counts the trimmed events but looks the untrimmed ones up.
            let Some(count) = histogram.get(event) else {
                return Err(Error::null_pointer());
            };
            set_probability(&mut result, &g, i, *count as f64 / total);
        }
        idx += events.len();
    }
    Ok(Value::Gts(result.shared()))
}

/// `CPROB`: `gts separator CPROB`, the (conditional) probability of the value of each
/// datapoint.
fn cprob(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "CPROB";
    let separator = match stack.pop()? {
        Value::Null => None,
        Value::String(s) => Some(s),
        _ => {
            return Err(fail(
                NAME,
                "expects null or a String separator on top of the stack.",
            ));
        }
    };
    gts_or_list_try(stack, NAME, |gts| match &separator {
        None => Ok(cprob_values(&gts.borrow())),
        Some(separator) => cprob_given(gts, separator),
    })
}

/// The function `PROBABILITY` builds: the probability of the value on top of the stack.
#[derive(Debug)]
struct Probability {
    probabilities: IndexMap<Value, f64>,
}

impl StackObject for Probability {
    fn name(&self) -> &'static str {
        "PROBABILITY"
    }

    fn to_snapshot(&self) -> String {
        let mut out = String::from("{ ");
        for (value, p) in &self.probabilities {
            out.push_str(&value_snapshot(value));
            out.push_str(&value_snapshot(&Value::Double(*p)));
        }
        out.push_str(" } PROBABILITY");
        out
    }

    fn apply(&self, stack: &mut Stack) -> Result<()> {
        let value = stack.pop()?;
        let p = self.probabilities.get(&value).copied().unwrap_or(0.0);
        stack.push(Value::Double(p));
        Ok(())
    }
}

/// `PROBABILITY`: `histogram PROBABILITY`, a function giving the probability of a value from
/// a value histogram (a map of counts).
fn probability(stack: &mut Stack) -> Result<()> {
    let Value::Map(histogram) = stack.pop()? else {
        return Err(fail(
            "PROBABILITY",
            "expects a value histogram on top of the stack.",
        ));
    };
    let histogram = histogram.borrow();
    let count = |value: &Value| -> Result<f64> {
        match value {
            Value::Null => Err(Error::null_pointer()),
            other => other
                .as_f64()
                .ok_or_else(|| Error::class_cast(other.java_class(), "java.lang.Number")),
        }
    };
    let mut total = 0.0;
    let mut counts = Vec::with_capacity(histogram.len());
    for value in histogram.values() {
        let c = count(value)?;
        total += c;
        counts.push(c);
    }
    let probabilities = histogram
        .keys()
        .zip(counts)
        .map(|(key, c)| (key.clone(), c / total))
        .collect();
    stack.push(StackFunction::value(Callable::Object(Rc::new(
        Probability { probabilities },
    ))));
    Ok(())
}
