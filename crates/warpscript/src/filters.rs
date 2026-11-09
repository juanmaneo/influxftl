// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The filters of `FILTER` (`WarpScriptFilterFunction`): `filter.byclass`, `filter.bylabels`
//! / `byattr` / `bylabelsattr`, `filter.bysize`, `filter.last.*`, `filter.any.*` /
//! `filter.all.*`, `MACROFILTER`, `filter.byselector`, `filter.bymetadata` and
//! `filter.latencies`. A filter receives the labels of an equivalence class and its series,
//! one list per collection, and returns the series it retains.

use std::cell::Ref;
use std::cmp::Ordering;
use std::rc::Rc;

use indexmap::IndexMap;

use crate::error::{Class, Error, Result};
use crate::functions::Entry;
use crate::functions::functional::java_double_compare;

use crate::gts::{GeoTimeSerie, Gts, Metadata, NO_ELEVATION, NO_LOCATION};
use crate::macros::Macro;
use crate::selector::{MetadataSelectorMatcher, Selector, build_selector};
use crate::snapshot::value_snapshot;
use crate::stack::Stack;
use crate::value::{FunctionObject, Value, ValueKind};

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("filter.byclass", Entry::Builder(byclass_builder)),
    (
        "filter.bylabels",
        Entry::Builder(|s, n| bylabels_builder(s, n, true, false)),
    ),
    (
        "filter.byattr",
        Entry::Builder(|s, n| bylabels_builder(s, n, false, true)),
    ),
    (
        "filter.bylabelsattr",
        Entry::Builder(|s, n| bylabels_builder(s, n, true, true)),
    ),
    ("filter.bysize", Entry::Builder(bysize_builder)),
    (
        "filter.last.eq",
        Entry::Builder(|s, n| last_builder(s, n, Comparison::Eq)),
    ),
    (
        "filter.last.ge",
        Entry::Builder(|s, n| last_builder(s, n, Comparison::Ge)),
    ),
    (
        "filter.last.gt",
        Entry::Builder(|s, n| last_builder(s, n, Comparison::Gt)),
    ),
    (
        "filter.last.le",
        Entry::Builder(|s, n| last_builder(s, n, Comparison::Le)),
    ),
    (
        "filter.last.lt",
        Entry::Builder(|s, n| last_builder(s, n, Comparison::Lt)),
    ),
    (
        "filter.last.ne",
        Entry::Builder(|s, n| last_builder(s, n, Comparison::Ne)),
    ),
    (
        "filter.any.eq",
        Entry::Builder(|s, n| any_builder(s, n, Comparison::Eq, false)),
    ),
    (
        "filter.any.ge",
        Entry::Builder(|s, n| any_builder(s, n, Comparison::Ge, false)),
    ),
    (
        "filter.any.gt",
        Entry::Builder(|s, n| any_builder(s, n, Comparison::Gt, false)),
    ),
    (
        "filter.any.le",
        Entry::Builder(|s, n| any_builder(s, n, Comparison::Le, false)),
    ),
    (
        "filter.any.lt",
        Entry::Builder(|s, n| any_builder(s, n, Comparison::Lt, false)),
    ),
    (
        "filter.any.ne",
        Entry::Builder(|s, n| any_builder(s, n, Comparison::Ne, false)),
    ),
    // `filter.all.<op>` retains the series no value of which passes the opposite test.
    (
        "filter.all.ne",
        Entry::Builder(|s, n| any_builder(s, n, Comparison::Eq, true)),
    ),
    (
        "filter.all.lt",
        Entry::Builder(|s, n| any_builder(s, n, Comparison::Ge, true)),
    ),
    (
        "filter.all.le",
        Entry::Builder(|s, n| any_builder(s, n, Comparison::Gt, true)),
    ),
    (
        "filter.all.gt",
        Entry::Builder(|s, n| any_builder(s, n, Comparison::Le, true)),
    ),
    (
        "filter.all.ge",
        Entry::Builder(|s, n| any_builder(s, n, Comparison::Lt, true)),
    ),
    (
        "filter.all.eq",
        Entry::Builder(|s, n| any_builder(s, n, Comparison::Ne, true)),
    ),
    ("MACROFILTER", Entry::Builder(macrofilter_builder)),
    ("filter.byselector", Entry::Builder(byselector_builder)),
    ("filter.bymetadata", Entry::Builder(bymetadata_builder)),
    ("filter.latencies", Entry::Builder(latencies_builder)),
];

/// A filter.
pub trait Filter {
    /// The series retained among those of a class (labels `labels`, one list per collection);
    /// a macro filter may return other values.
    fn filter(
        &self,
        stack: &mut Stack,
        labels: &IndexMap<String, String>,
        series: &[Vec<Gts>],
    ) -> Result<Vec<Value>>;

    /// The snapshot of the filter built by `name` (the Java `toString()`): the WarpScript
    /// source of the parameters the builder consumed, then the name.
    fn snapshot(&self, name: &str) -> String;
}

/// A filter registered under the name of its builder.
struct Named<F: Filter + 'static> {
    name: &'static str,
    filter: F,
}

impl<F: Filter + 'static> FunctionObject for Named<F> {
    fn name(&self) -> &str {
        self.name
    }

    fn type_name(&self) -> &'static str {
        "FILTER"
    }

    fn snapshot(&self) -> String {
        self.filter.snapshot(self.name)
    }

    fn as_filter(&self) -> Option<&dyn Filter> {
        Some(&self.filter)
    }
}

/// Push the filter `filter` built by `name`.
fn push<F: Filter + 'static>(stack: &mut Stack, name: &'static str, filter: F) {
    stack.push(Value::Function(Rc::new(Named { name, filter })));
}

/// The filter of a function object value.
pub fn filter_of(value: &Value) -> Option<&dyn Filter> {
    match value {
        Value::Function(function) => function.as_filter(),
        _ => None,
    }
}

/// The series of all the collections passing `test`, in order.
fn retain(
    series: &[Vec<Gts>],
    mut test: impl FnMut(&GeoTimeSerie) -> Result<bool>,
) -> Result<Vec<Value>> {
    let mut retained = Vec::new();
    for gts in series.iter().flatten() {
        if test(&gts.borrow())? {
            retained.push(Value::Gts(gts.clone()));
        }
    }
    Ok(retained)
}

/// `FilterByClass`: the series whose class name matches.
struct ByClass {
    source: String,
    selector: Selector,
}

impl Filter for ByClass {
    fn filter(
        &self,
        _stack: &mut Stack,
        _labels: &IndexMap<String, String>,
        series: &[Vec<Gts>],
    ) -> Result<Vec<Value>> {
        retain(series, |gts| {
            self.selector.matches(gts.name().unwrap_or_default())
        })
    }

    fn snapshot(&self, name: &str) -> String {
        format!(
            "{} {name}",
            value_snapshot(&Value::string(&self.source)).trim_end()
        )
    }
}

/// `FilterByClass.Builder`: `'selector' filter.byclass`.
fn byclass_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let Value::String(source) = stack.pop()? else {
        return Err(Error::new("Invalid class selector"));
    };
    let selector = Selector::parse(&source)?;
    push(
        stack,
        name,
        ByClass {
            source: source.to_string(),
            selector,
        },
    );
    Ok(())
}

/// `FilterByLabels`: the series whose labels (and / or attributes) match all the selectors.
/// With the absent label support of Warp 10, the selectors `` and `=` (`None` here) require
/// the label to be missing.
struct ByLabels {
    selectors: Vec<(String, String, Option<Selector>)>,
    labels: bool,
    attributes: bool,
}

impl ByLabels {
    /// Whether the series matches all the selectors.
    fn accepts(&self, gts: &GeoTimeSerie) -> Result<bool> {
        let (labels, attributes) = (&gts.metadata.labels, &gts.metadata.attributes);
        for (name, _, selector) in &self.selectors {
            let in_labels = self.labels && labels.contains_key(name);
            let in_attributes = self.attributes && attributes.contains_key(name);
            let Some(selector) = selector else {
                if in_labels || in_attributes {
                    return Ok(false);
                }
                continue;
            };
            let value = if in_labels {
                &labels[name]
            } else if in_attributes {
                &attributes[name]
            } else {
                return Ok(false);
            };
            if !selector.matches(value)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl Filter for ByLabels {
    fn filter(
        &self,
        _stack: &mut Stack,
        _labels: &IndexMap<String, String>,
        series: &[Vec<Gts>],
    ) -> Result<Vec<Value>> {
        retain(series, |gts| self.accepts(gts))
    }

    /// `{ 'k' 'v''k2' 'v2'} name`: Java puts no space between the entries nor before `}`.
    fn snapshot(&self, name: &str) -> String {
        let mut out = String::from("{ ");
        for (name, source, _) in &self.selectors {
            out.push_str(value_snapshot(&Value::string(name)).trim_end());
            out.push(' ');
            out.push_str(value_snapshot(&Value::string(source)).trim_end());
        }
        out.push_str("} ");
        out.push_str(name);
        out
    }
}

/// `FilterByLabels.Builder`: `{ selectors } filter.bylabels`.
fn bylabels_builder(
    stack: &mut Stack,
    name: &'static str,
    labels: bool,
    attributes: bool,
) -> Result<()> {
    let Value::Map(map) = stack.pop()? else {
        return Err(Error::new("Invalid labels/attributes selector"));
    };
    let mut selectors = Vec::new();
    for (key, value) in map.borrow().iter() {
        let (Value::String(key), Value::String(source)) = (key, value) else {
            let other = if matches!(key, Value::String(_)) {
                value
            } else {
                key
            };
            return Err(Error::class_cast(other.kind(), ValueKind::String));
        };
        let selector = if source.is_empty() || &**source == "=" {
            None
        } else {
            Some(Selector::parse(source)?)
        };
        selectors.push((key.to_string(), source.to_string(), selector));
    }
    push(
        stack,
        name,
        ByLabels {
            selectors,
            labels,
            attributes,
        },
    );
    Ok(())
}

/// `FilterBySize`: the series of a number of values within `[min, max]` (`int`s).
struct BySize {
    min: i32,
    max: i32,
}

impl Filter for BySize {
    fn filter(
        &self,
        _stack: &mut Stack,
        _labels: &IndexMap<String, String>,
        series: &[Vec<Gts>],
    ) -> Result<Vec<Value>> {
        retain(series, |gts| {
            let size = gts.size() as i64;
            Ok(size >= i64::from(self.min) && size <= i64::from(self.max))
        })
    }

    fn snapshot(&self, name: &str) -> String {
        format!("{} {} {name}", self.min, self.max)
    }
}

/// `FilterBySize.Builder`: `min max filter.bysize`.
fn bysize_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let max = stack.pop()?;
    let min = stack.pop()?;
    let (Value::Long(min), Value::Long(max)) = (min, max) else {
        return Err(Error::usage(name, "expects two LONG objects as arguments."));
    };
    push(
        stack,
        name,
        BySize {
            min: min as i32,
            max: max as i32,
        },
    );
    Ok(())
}

/// The comparison of a threshold with a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Comparison {
    Eq,
    Ge,
    Gt,
    Le,
    Lt,
    Ne,
}

impl Comparison {
    /// Whether `threshold.compareTo(value)` (`ordering`) satisfies the comparison of the value
    /// with the threshold (`value >= threshold` for `Ge`…).
    fn verify(self, ordering: Ordering) -> bool {
        match self {
            Comparison::Eq => ordering == Ordering::Equal,
            Comparison::Ge => ordering != Ordering::Greater,
            Comparison::Gt => ordering == Ordering::Less,
            Comparison::Le => ordering != Ordering::Less,
            Comparison::Lt => ordering == Ordering::Greater,
            Comparison::Ne => ordering != Ordering::Equal,
        }
    }
}

/// `threshold.compareTo(value)` as the `filter.last.*` / `filter.any.*` filters compute it (a
/// `LONG` or `DOUBLE` threshold casts the value to a `Number`); `None` for a `BOOLEAN`
/// threshold, which only tests equality.
fn compare_threshold(threshold: &Value, value: &Value) -> Result<Option<Ordering>> {
    let number = || match value {
        Value::Long(_) | Value::Double(_) => Ok(value),
        other => Err(Error::class_cast(other.kind(), Class::Number)),
    };
    Ok(Some(match threshold {
        Value::Long(t) => {
            let v = number()?;
            t.cmp(&v.as_i64().unwrap_or_default())
        }
        Value::Double(t) => {
            let v = number()?;
            java_double_compare(*t, v.as_f64().unwrap_or_default())
        }
        Value::String(t) => t.encode_utf16().cmp(value.to_string().encode_utf16()),
        _ => return Ok(None),
    }))
}

/// `GTSHelper.getLastValue`: the value at the last bucket of a bucketized series, otherwise
/// at its most recent tick (the first of equal ones); `None` when there is none.
fn last_value(gts: &GeoTimeSerie) -> Option<Value> {
    if gts.size() == 0 {
        return None;
    }
    let index = if gts.is_bucketized() {
        gts.ticks.iter().position(|&tick| tick == gts.lastbucket)?
    } else {
        let mut index = 0;
        for (i, &tick) in gts.ticks.iter().enumerate() {
            if tick > gts.ticks[index] {
                index = i;
            }
        }
        index
    };
    Some(gts.value_at(index))
}

/// `FilterLast*`: the series whose last value compares as requested with the threshold.
struct Last {
    threshold: Value,
    comparison: Comparison,
}

impl Filter for Last {
    fn filter(
        &self,
        _stack: &mut Stack,
        _labels: &IndexMap<String, String>,
        series: &[Vec<Gts>],
    ) -> Result<Vec<Value>> {
        retain(series, |gts| {
            let Some(last) = last_value(gts) else {
                return Ok(false);
            };
            Ok(match compare_threshold(&self.threshold, &last)? {
                Some(ordering) => self.comparison.verify(ordering),
                None => (self.threshold == last) == (self.comparison == Comparison::Eq),
            })
        })
    }

    fn snapshot(&self, name: &str) -> String {
        format!("{} {name}", value_snapshot(&self.threshold).trim_end())
    }
}

/// `FilterLast*.Builder`: `threshold filter.last.<op>` (a `BOOLEAN` threshold for `eq` and
/// `ne` only).
fn last_builder(stack: &mut Stack, name: &'static str, comparison: Comparison) -> Result<()> {
    let threshold = stack.pop()?;
    let boolean = matches!(comparison, Comparison::Eq | Comparison::Ne);
    match threshold {
        Value::Long(_) | Value::Double(_) | Value::String(_) => {}
        Value::Boolean(_) if boolean => {}
        _ => return Err(Error::new("Invalid threshold type.")),
    }
    push(
        stack,
        name,
        Last {
            threshold,
            comparison,
        },
    );
    Ok(())
}

/// `FilterAny`: the series one value of which compares as requested with the threshold (none
/// when complemented, for `filter.all.*`).
struct Any {
    threshold: Value,
    comparison: Comparison,
    complement: bool,
}

impl Filter for Any {
    fn filter(
        &self,
        _stack: &mut Stack,
        _labels: &IndexMap<String, String>,
        series: &[Vec<Gts>],
    ) -> Result<Vec<Value>> {
        retain(series, |gts| {
            let mut found = false;
            for i in 0..gts.size() {
                let value = gts.value_at(i);
                found = match compare_threshold(&self.threshold, &value)? {
                    Some(ordering) => self.comparison.verify(ordering),
                    None => (self.threshold == value) != (self.comparison == Comparison::Ne),
                };
                if found {
                    break;
                }
            }
            Ok(found != self.complement)
        })
    }

    fn snapshot(&self, name: &str) -> String {
        format!("{} {name}", value_snapshot(&self.threshold).trim_end())
    }
}

/// `FilterAny.Builder`: `threshold filter.any.<op>` (a `BOOLEAN` threshold for the `eq` and
/// `ne` comparisons only).
fn any_builder(
    stack: &mut Stack,
    name: &'static str,
    comparison: Comparison,
    complement: bool,
) -> Result<()> {
    let threshold = stack.pop()?;
    let boolean = matches!(comparison, Comparison::Eq | Comparison::Ne);
    match threshold {
        Value::Long(_) | Value::Double(_) | Value::String(_) => {}
        Value::Boolean(_) if boolean => {}
        _ => return Err(Error::usage(name, "threshold type is invalid.")),
    }
    push(
        stack,
        name,
        Any {
            threshold,
            comparison,
            complement,
        },
    );
    Ok(())
}

/// `MACROFILTER.MacroFilterWrapper`: a macro receiving the class labels and the lists of
/// series, and leaving the list of retained series.
struct MacroFilter {
    name: &'static str,
    wrapped: Rc<Macro>,
}

impl Filter for MacroFilter {
    fn filter(
        &self,
        stack: &mut Stack,
        labels: &IndexMap<String, String>,
        series: &[Vec<Gts>],
    ) -> Result<Vec<Value>> {
        stack.push(Value::map(
            labels
                .iter()
                .map(|(k, v)| (Value::from(k.as_str()), Value::from(v.as_str())))
                .collect(),
        ));
        stack.push(Value::list(
            series
                .iter()
                .map(|members| Value::list(members.iter().cloned().map(Value::Gts).collect()))
                .collect(),
        ));
        stack.exec_macro(&self.wrapped)?;
        let Value::List(result) = stack.pop()? else {
            return Err(Error::usage(
                self.name,
                "macro should return a list of Geo Time Series onto the stack.",
            ));
        };
        let retained = result.borrow().clone();
        Ok(retained)
    }

    /// `<% <% … %> MACROFILTER %> EVAL`.
    fn snapshot(&self, name: &str) -> String {
        format!("<% {} {name} %> EVAL", self.wrapped.snapshot(true))
    }
}

/// `MACROFILTER`: `macro MACROFILTER`.
fn macrofilter_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let Value::Macro(wrapped) = stack.pop()? else {
        return Err(Error::usage(name, "expects a macro on top of the stack."));
    };
    push(stack, name, MacroFilter { name, wrapped });
    Ok(())
}

/// `FilterBySelector`: the series whose metadata match a `class{labels}{attributes}` selector.
struct BySelector {
    source: String,
    matcher: MetadataSelectorMatcher,
}

impl Filter for BySelector {
    fn filter(
        &self,
        _stack: &mut Stack,
        _labels: &IndexMap<String, String>,
        series: &[Vec<Gts>],
    ) -> Result<Vec<Value>> {
        retain(series, |gts| self.matcher.matches(&gts.metadata))
    }

    fn snapshot(&self, name: &str) -> String {
        format!(
            "{} {name}",
            value_snapshot(&Value::string(&self.source)).trim_end()
        )
    }
}

/// `FilterBySelector.Builder`: `'selector' filter.byselector`.
fn byselector_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let Value::String(source) = stack.pop()? else {
        return Err(Error::usage(name, "expects a STRING selector"));
    };
    let matcher = MetadataSelectorMatcher::new(&source)?;
    push(
        stack,
        name,
        BySelector {
            source: source.to_string(),
            matcher,
        },
    );
    Ok(())
}

/// `FilterByMetadata`: the series whose metadata (name, labels, attributes and ids) equal those
/// of one of the reference series.
struct ByMetadata {
    metadatas: Vec<Metadata>,
}

impl Filter for ByMetadata {
    fn filter(
        &self,
        _stack: &mut Stack,
        _labels: &IndexMap<String, String>,
        series: &[Vec<Gts>],
    ) -> Result<Vec<Value>> {
        retain(series, |gts| Ok(self.metadatas.contains(&gts.metadata)))
    }

    /// `[ 'selector' PARSESELECTOR NEWGTS SWAP RELABEL SWAP RENAME … ] name`.
    fn snapshot(&self, name: &str) -> String {
        let mut out = String::from("[ ");
        for metadata in &self.metadatas {
            out.push_str(value_snapshot(&Value::string(build_selector(metadata))).trim_end());
            out.push_str(" PARSESELECTOR NEWGTS SWAP RELABEL SWAP RENAME ");
        }
        out.push_str("] ");
        out.push_str(name);
        out
    }
}

/// `FilterByMetadata.Builder`: `[ series ] filter.bymetadata`.
fn bymetadata_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let invalid = || Error::usage(name, "expects a list of GTS on top of the stack.");
    let Value::List(list) = stack.pop()? else {
        return Err(invalid());
    };
    let mut metadatas: Vec<Metadata> = Vec::new();
    for element in list.borrow().iter() {
        let Value::Gts(gts) = element else {
            return Err(invalid());
        };
        let metadata = &gts.borrow().metadata;
        if !metadatas.contains(metadata) {
            metadatas.push(metadata.clone());
        }
    }
    push(stack, name, ByMetadata { metadatas });
    Ok(())
}

/// `LatencyFilter`: not a filter per se. The values of the series are packet hashes; the
/// single series of the first collection is the uplink, the series of the others downlinks.
/// Each uplink packet is matched with the same hashes on the downlinks received between `min`
/// and `max` time units later, producing the series selected by `options` (latencies,
/// matches, bitset of the downlinks…) at the ticks of the uplink.
struct Latencies {
    min: i64,
    max: i64,
    /// The `toString` of each option.
    options: Vec<String>,
}

/// The outputs of a [`Latencies`] filter, in the order Warp 10 returns them.
const LATENCY_OPTIONS: [&str; 8] = [
    "uplink.latency.min",
    "uplink.latency.max",
    "downlink.latency.min",
    "downlink.latency.max",
    "downlink.matches",
    "downlinks.bitset",
    "downlinks.totalmatches",
    "downlinks.withmatches",
];

/// `((Number) value).longValue()` of a packet hash.
fn hash_of(value: &Value) -> Result<i64> {
    value
        .as_i64()
        .ok_or_else(|| Error::class_cast(value.kind(), Class::Number))
}

/// A `cloneEmpty` of `gts` named after it with `suffix`.
fn latency_output(gts: &GeoTimeSerie, suffix: &str) -> GeoTimeSerie {
    let mut out = gts.clone_empty();
    out.metadata.name = Some(format!("{}:{suffix}", gts.name().unwrap_or("null")));
    out
}

impl Latencies {
    fn wants(&self, option: &str) -> bool {
        self.options.iter().any(|o| o == option)
    }
}

impl Filter for Latencies {
    fn filter(
        &self,
        _stack: &mut Stack,
        _labels: &IndexMap<String, String>,
        series: &[Vec<Gts>],
    ) -> Result<Vec<Value>> {
        if series.first().map(Vec::len) != Some(1) {
            return Ok(Vec::new());
        }
        let all: Vec<&Gts> = series.iter().flatten().collect();
        for gts in &all {
            gts.borrow_mut().value_sort(false);
        }
        let all: Vec<Ref<'_, GeoTimeSerie>> = all.iter().map(|gts| gts.borrow()).collect();
        let uplink = &*all[0];
        let downlinks = &all[1..];
        let or_none = |v: i64, none: i64| if v == none { -1 } else { v };
        // One output per option: the uplink ones, then one per downlink.
        let mut uplink_out: Vec<GeoTimeSerie> = LATENCY_OPTIONS
            .iter()
            .map(|option| latency_output(uplink, option))
            .collect();
        let mut downlink_out: Vec<Vec<GeoTimeSerie>> = (2..5)
            .map(|k| {
                downlinks
                    .iter()
                    .map(|gts| latency_output(gts, LATENCY_OPTIONS[k]))
                    .collect()
            })
            .collect();
        let mut indices = vec![0usize; downlinks.len()];
        for up in 0..uplink.size() {
            let hash = hash_of(&uplink.value_at(up))?;
            let tick = uplink.tick_at(up);
            let (mut min, mut max) = (i64::MAX, i64::MIN);
            let (mut total, mut with_matches, mut mask) = (0i64, 0i64, 0i64);
            for (i, gts) in downlinks.iter().enumerate() {
                let mut start = indices[i];
                let (mut down_min, mut down_max, mut matches) = (i64::MAX, i64::MIN, 0i64);
                while indices[i] < gts.size() {
                    let idx = indices[i];
                    let down_hash = hash_of(&gts.value_at(idx))?;
                    let down_tick = gts.tick_at(idx);
                    // A hash too low, or the same hash received too early, is skipped for good;
                    // a higher hash or a later reception ends the matches of this packet.
                    let skip = match down_hash.cmp(&hash) {
                        Ordering::Less => true,
                        Ordering::Greater => break,
                        Ordering::Equal => down_tick < tick.wrapping_add(self.min),
                    };
                    if skip {
                        indices[i] += 1;
                        start = indices[i];
                        continue;
                    }
                    let latency = down_tick.wrapping_sub(tick);
                    if latency > self.max {
                        break;
                    }
                    (min, max) = (min.min(latency), max.max(latency));
                    (down_min, down_max) = (down_min.min(latency), down_max.max(latency));
                    matches += 1;
                    total += 1;
                    if start == indices[i] {
                        with_matches += 1;
                        mask |= 1i64.wrapping_shl(i as u32);
                    }
                    indices[i] += 1;
                }
                let values = [
                    or_none(down_min, i64::MAX),
                    or_none(down_max, i64::MIN),
                    matches,
                ];
                for (out, value) in downlink_out.iter_mut().zip(values) {
                    set(&mut out[i], tick, value);
                }
            }
            let values = [
                or_none(min, i64::MAX),
                or_none(max, i64::MIN),
                mask,
                total,
                with_matches,
            ];
            for (k, value) in [0, 1, 5, 6, 7].into_iter().zip(values) {
                set(&mut uplink_out[k], tick, value);
            }
        }
        let mut result = Vec::new();
        for (k, option) in LATENCY_OPTIONS.iter().enumerate() {
            if !self.wants(option) {
                continue;
            }
            match k {
                2..=4 => result.append(&mut downlink_out[k - 2]),
                _ => result.push(std::mem::take(&mut uplink_out[k])),
            }
        }
        Ok(result
            .into_iter()
            .map(|gts| Value::Gts(gts.shared()))
            .collect())
    }

    /// `min max [ 'option' … ] name`.
    fn snapshot(&self, name: &str) -> String {
        let mut out = format!("{} {} [ ", self.min, self.max);
        for option in &self.options {
            out.push_str(value_snapshot(&Value::string(option)).trim_end());
            out.push(' ');
        }
        out.push_str("] ");
        out.push_str(name);
        out
    }
}

/// `GTSHelper.setValue(gts, tick, value)`.
fn set(gts: &mut GeoTimeSerie, tick: i64, value: i64) {
    gts.set_value(tick, NO_LOCATION, NO_ELEVATION, &Value::Long(value), false);
}

/// `LatencyFilter.Builder`: `min max [ options ] filter.latencies`.
fn latencies_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let Value::List(options) = stack.pop()? else {
        return Err(Error::usage(
            name,
            "expects a list of options on top of the stack.",
        ));
    };
    let Value::Long(max) = stack.pop()? else {
        return Err(Error::usage(
            name,
            "expects a maximum latency under the list of options.",
        ));
    };
    let Value::Long(min) = stack.pop()? else {
        return Err(Error::usage(
            name,
            "expects a minimum latency under the list of options.",
        ));
    };
    let options = options.borrow().iter().map(ToString::to_string).collect();
    push(stack, name, Latencies { min, max, options });
    Ok(())
}
