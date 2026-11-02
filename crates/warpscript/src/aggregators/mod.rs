// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The aggregators of the `BUCKETIZE`, `MAP` and `REDUCE` frameworks
//! (`WarpScriptAggregatorFunction`): function objects applied to a window of datapoints
//! (`bucketizer.sum`, `mapper.mean`, `reducer.max`…).

pub mod basic;
#[cfg(test)]
mod basic_tests;
pub mod join;
#[cfg(test)]
mod join_tests;
pub mod order;
#[cfg(test)]
mod order_tests;
pub mod stats;
#[cfg(test)]
mod stats_tests;
#[cfg(test)]
mod tests;

use std::cmp::Ordering;
use std::rc::Rc;

use indexmap::IndexMap;

use crate::error::{Error, Result};
use crate::functions::functional::java_double_compare;
use crate::functions::operators::compare_long_double;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::value::{FunctionObject, Value};

/// The arguments of an aggregator (`WarpScriptAggregatorFunction.apply(Object[] args)`):
/// the tick the result is for, the names and labels of the series involved, and the
/// datapoints of the window as parallel arrays (`null` values are `Value::Null`).
pub struct AggregatorArgs<'a> {
    /// `args[0]`: the tick of the result (the bucket end, the current tick…).
    pub tick: i64,
    /// `args[1]`: the class names of the series (one per series for reducers).
    pub names: &'a [String],
    /// `args[2]`: the labels of the series.
    pub labels: &'a [IndexMap<String, String>],
    /// `args[3]`: the ticks of the datapoints.
    pub ticks: &'a [i64],
    /// `args[4]`: their locations (`NO_LOCATION` when absent).
    pub locations: &'a [i64],
    /// `args[5]`: their elevations (`NO_ELEVATION` when absent).
    pub elevations: &'a [i64],
    /// `args[6]`: their values.
    pub values: &'a [Value],
    /// `args[7]`: the window parameters, when the framework provides them.
    pub window: Option<Window>,
}

/// The window parameters of a bucketizer or mapper application (`args[7]`, a `long[]`):
/// `[ 0 -bucketspan start end ]` for `BUCKETIZE`, `[ prewindow postwindow start stop tickidx ]`
/// for `MAP`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// The pre-window parameter (`0` for `BUCKETIZE`).
    pub prewindow: i64,
    /// The post-window parameter (the negated bucket span for `BUCKETIZE`).
    pub postwindow: i64,
    /// The first tick of the window.
    pub start: i64,
    /// The last tick of the window.
    pub stop: i64,
    /// `MAP` only: the index of the current tick in the window (`-1` when it holds no
    /// datapoint at that tick); `BUCKETIZE` provides no fifth element.
    pub tick_index: Option<i64>,
}

/// The result of an aggregator: `{ tick, location, elevation, value }` (`Object[4]`).
#[derive(Clone, Debug, PartialEq)]
pub struct Aggregated {
    /// The tick of the result (ignored by the frameworks that impose their own).
    pub tick: i64,
    /// The location of the result.
    pub location: i64,
    /// The elevation of the result.
    pub elevation: i64,
    /// The value, `Value::Null` when there is none.
    pub value: Value,
}

impl Aggregated {
    /// The "no result" tuple most aggregators return on an empty window:
    /// `{ Long.MAX_VALUE, NO_LOCATION, NO_ELEVATION, null }`.
    pub fn none() -> Self {
        Aggregated {
            tick: i64::MAX,
            location: NO_LOCATION,
            elevation: NO_ELEVATION,
            value: Value::Null,
        }
    }
}

/// The roles an aggregator may play (the `WarpScriptBucketizerFunction`,
/// `WarpScriptMapperFunction` and `WarpScriptReducerFunction` interfaces of its class).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Roles {
    /// Accepted by `BUCKETIZE`.
    pub bucketizer: bool,
    /// Accepted by `MAP`.
    pub mapper: bool,
    /// Accepted by `REDUCE`.
    pub reducer: bool,
}

impl Roles {
    /// All three roles, the common case of the shared aggregator classes.
    pub const ALL: Roles = Roles {
        bucketizer: true,
        mapper: true,
        reducer: true,
    };
}

/// An aggregator function object.
pub trait Aggregator {
    /// The roles of the aggregator.
    fn roles(&self) -> Roles;

    /// Aggregate a window of datapoints.
    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated>;

    /// The WarpScript source of the parameters a builder consumed to produce this aggregator
    /// (`true` for `true bucketizer.var`), which the snapshot puts before the name like the
    /// Java `toString()`; `None` for the parameterless aggregators (`'name' FUNCREF`).
    fn parameters(&self) -> Option<String> {
        None
    }
}

/// A registered aggregator: the name it was registered under and its implementation.
pub struct Named<A: Aggregator + 'static> {
    name: &'static str,
    aggregator: A,
}

impl<A: Aggregator + 'static> Named<A> {
    /// Wrap `aggregator` under its registered name.
    pub fn object(name: &'static str, aggregator: A) -> Rc<dyn FunctionObject> {
        Rc::new(Named { name, aggregator })
    }
}

impl<A: Aggregator + 'static> FunctionObject for Named<A> {
    fn name(&self) -> &str {
        self.name
    }

    fn type_name(&self) -> &'static str {
        "AGGREGATOR"
    }

    fn snapshot(&self) -> String {
        named_snapshot(self.name, self.aggregator.parameters())
    }

    fn as_aggregator(&self) -> Option<&dyn Aggregator> {
        Some(&self.aggregator)
    }
}

/// The snapshot of the aggregator `name` built with `parameters` (`true bucketizer.var`), or
/// the reference snapshot of a parameterless one (`'bucketizer.sum' FUNCREF`).
fn named_snapshot(name: &str, parameters: Option<String>) -> String {
    match parameters {
        Some(parameters) => format!("{parameters} {name}"),
        None => format!("'{}' FUNCREF", crate::format::url_encode(name)),
    }
}

/// The value's aggregator, when it is an aggregator function object.
pub fn aggregator_of(value: &Value) -> Option<&dyn Aggregator> {
    match value {
        Value::Function(function) => function.as_aggregator(),
        _ => None,
    }
}

/// Java `((Number) value).longValue()` / `doubleValue()` on a datapoint value: the
/// `ClassCastException` of a non numeric value.
pub(crate) fn class_cast(value: &Value) -> Error {
    Error::class_cast(value.java_class(), "java.lang.Number")
}

/// `((Number) value).longValue()`.
pub(crate) fn long_value(value: &Value) -> Result<i64> {
    value.as_i64().ok_or_else(|| class_cast(value))
}

/// `((Number) value).doubleValue()`.
pub(crate) fn double_value(value: &Value) -> Result<f64> {
    value.as_f64().ok_or_else(|| class_cast(value))
}

/// The location and elevation of the most recent datapoint of the window (the way `Sum`,
/// `Count`, `RMS`, `Join`… pick theirs).
pub(crate) fn latest(args: &AggregatorArgs) -> (i64, i64) {
    let mut location = NO_LOCATION;
    let mut elevation = NO_ELEVATION;
    let mut timestamp = i64::MIN;
    for i in 0..args.values.len() {
        if args.ticks[i] > timestamp {
            location = args.locations[i];
            elevation = args.elevations[i];
            timestamp = args.ticks[i];
        }
    }
    (location, elevation)
}

/// A numeric datapoint value (`Long` or `Double`), for the comparisons of `EQ.compare`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Num {
    /// A `LONG`.
    Long(i64),
    /// A `DOUBLE`.
    Double(f64),
}

impl Num {
    /// The numeric value, `None` for `null` and non numeric values.
    pub(crate) fn of(value: &Value) -> Option<Num> {
        match value {
            Value::Long(l) => Some(Num::Long(*l)),
            Value::Double(d) => Some(Num::Double(*d)),
            _ => None,
        }
    }

    /// The value as a `f64` (`Number.doubleValue()`).
    pub(crate) fn as_f64(self) -> f64 {
        match self {
            Num::Long(l) => l as f64,
            Num::Double(d) => d,
        }
    }

    /// `EQ.compare(Number, Number)`: `Long.compare` on two `LONG`s, `Double.compareTo` on two
    /// `DOUBLE`s (`-0.0 < 0.0`, `NaN` greatest) and the exact `BigDecimal` comparison of a
    /// `LONG` with a `DOUBLE`. Warp 10 fails on a `LONG` compared with a `NaN` or infinite
    /// `DOUBLE` (`Infinite or NaN`); here infinities order naturally and `NaN` is the greatest
    /// value, so that the comparison is a total order.
    pub(crate) fn compare(self, other: Num) -> Ordering {
        match (self, other) {
            (Num::Long(x), Num::Long(y)) => x.cmp(&y),
            (Num::Double(x), Num::Double(y)) => java_double_compare(x, y),
            (Num::Long(l), Num::Double(d)) => compare_long_nan_double(l, d),
            (Num::Double(d), Num::Long(l)) => compare_long_nan_double(l, d).reverse(),
        }
    }
}

/// A `LONG` against a `DOUBLE` that may be `NaN` (then the greatest).
fn compare_long_nan_double(l: i64, d: f64) -> Ordering {
    if d.is_nan() {
        Ordering::Less
    } else {
        compare_long_double(l, d)
    }
}
