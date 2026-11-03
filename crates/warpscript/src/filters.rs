// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The filters of `FILTER` (`WarpScriptFilterFunction`): `filter.byclass`, `filter.bylabels`
//! / `byattr` / `bylabelsattr`, `filter.bysize`, `filter.last.*`, `filter.any.*` /
//! `filter.all.*` and `MACROFILTER`. A filter receives the labels of an equivalence class and
//! its series, one list per collection, and returns the series it retains.

use std::cmp::Ordering;
use std::rc::Rc;

use indexmap::IndexMap;

use crate::error::{Error, Result};
use crate::functions::Entry;
use crate::functions::functional::java_double_compare;
use crate::functions::strings::CompiledPattern;
use crate::gts::{GeoTimeSerie, Gts};
use crate::macros::Macro;
use crate::snapshot::value_snapshot;
use crate::stack::Stack;
use crate::value::{FunctionObject, Value};

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

/// A class, label or attribute selector: `=value` or `value` (exact) or `~regexp`.
enum Selector {
    Exact(String),
    Regex(CompiledPattern),
}

impl Selector {
    /// Parse a selector.
    fn parse(selector: &str) -> Result<Selector> {
        Ok(if let Some(exact) = selector.strip_prefix('=') {
            Selector::Exact(exact.to_owned())
        } else if let Some(regex) = selector.strip_prefix('~') {
            Selector::Regex(CompiledPattern::compile(regex)?)
        } else {
            Selector::Exact(selector.to_owned())
        })
    }

    /// Whether `value` matches entirely (`Matcher.matches()`).
    fn matches(&self, value: &str) -> Result<bool> {
        Ok(match self {
            Selector::Exact(exact) => exact == value,
            Selector::Regex(regex) => !regex.full_match(value)?.is_empty(),
        })
    }
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
            return Err(Error::class_cast(other.java_class(), "java.lang.String"));
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
        return Err(Error::new(format!(
            "{name} expects two LONG objects as arguments."
        )));
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
        other => Err(Error::class_cast(other.java_class(), "java.lang.Number")),
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
        _ => return Err(Error::new(format!("{name} threshold type is invalid."))),
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
            return Err(Error::new(format!(
                "{} macro should return a list of Geo Time Series onto the stack.",
                self.name
            )));
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
        return Err(Error::new(format!(
            "{name} expects a macro on top of the stack."
        )));
    };
    push(stack, name, MacroFilter { name, wrapped });
    Ok(())
}
