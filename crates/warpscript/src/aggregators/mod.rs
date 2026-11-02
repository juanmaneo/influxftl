// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The aggregators of the `BUCKETIZE`, `MAP` and `REDUCE` frameworks
//! (`WarpScriptAggregatorFunction`): function objects applied to a window of datapoints
//! (`bucketizer.sum`, `mapper.mean`, `reducer.max`…).

pub mod basic;
#[cfg(test)]
mod basic_tests;

use std::rc::Rc;

use indexmap::IndexMap;

use crate::error::Result;
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
    /// `args[7]`: the window parameters (`[ 0 -bucketspan start end ]` for bucketizers,
    /// `[ prewindow postwindow start end ]` for mappers), when the framework provides them.
    pub window: Option<[i64; 4]>,
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

    fn as_aggregator(&self) -> Option<&dyn Aggregator> {
        Some(&self.aggregator)
    }
}

/// The value's aggregator, when it is an aggregator function object.
pub fn aggregator_of(value: &Value) -> Option<&dyn Aggregator> {
    match value {
        Value::Function(function) => function.as_aggregator(),
        _ => None,
    }
}
