// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `STRICTMAPPER` and `STRICTREDUCER`: wrappers that only apply a mapper to windows of a given
//! size or timespan, and a reducer to ticks where no series lacks a value.

use std::rc::Rc;

use super::{Aggregated, Aggregator, AggregatorArgs, Num, Roles};
use crate::error::{Error, Result};
use crate::functions::Entry;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::macros::{Callable, Macro, StackObject, Statement};
use crate::stack::Stack;
use crate::value::{FunctionObject, Value};

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("STRICTMAPPER", Entry::Builder(strict_mapper_builder)),
    ("STRICTREDUCER", Entry::Builder(strict_reducer_builder)),
];

/// The "no result" tuple of a wrapper whose condition does not hold.
fn nothing(args: &AggregatorArgs) -> Aggregated {
    Aggregated {
        tick: args.tick,
        location: NO_LOCATION,
        elevation: NO_ELEVATION,
        value: Value::Null,
    }
}

/// The bounds of `STRICTMAPPER`: a positive bound is a number of values, a negative one a
/// timespan (its opposite), `0` no bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    /// The minimum (inclusive).
    pub min: i64,
    /// The maximum (inclusive).
    pub max: i64,
}

impl Bounds {
    /// Whether `count` values over `timespan` are within the bounds.
    fn admit(self, count: i64, timespan: i64) -> bool {
        let Bounds { min, max } = self;
        !(min > 0 && count < min
            || max > 0 && count > max
            || min < 0 && timespan < -min
            || max < 0 && timespan > -max)
    }
}

/// `STRICTMAPPER.StringentMapper`: the mapper applied only to the windows within the bounds
/// (their timespan running from their first to their last tick, inclusive).
pub struct StrictMapper {
    /// The name of the function that built the wrapper.
    pub name: &'static str,
    /// The bounds.
    pub bounds: Bounds,
    /// The wrapped mapper.
    pub mapper: Rc<dyn FunctionObject>,
}

impl Aggregator for StrictMapper {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        match self.delegate(args) {
            Some(mapper) => mapper.apply(args),
            None => Ok(nothing(args)),
        }
    }

    fn delegate(&self, args: &AggregatorArgs) -> Option<&dyn Aggregator> {
        let ticks = args.ticks;
        let timespan = match (ticks.first(), ticks.last()) {
            (Some(first), Some(last)) => last.wrapping_sub(*first).wrapping_add(1),
            _ => 0,
        };
        self.bounds
            .admit(ticks.len() as i64, timespan)
            .then(|| self.mapper.as_aggregator())
            .flatten()
    }
}

impl FunctionObject for StrictMapper {
    fn name(&self) -> &str {
        self.name
    }

    fn type_name(&self) -> &'static str {
        "AGGREGATOR"
    }

    fn snapshot(&self) -> String {
        format!(
            "<% {} {} {} {} %> EVAL",
            self.mapper.snapshot(),
            self.bounds.min,
            self.bounds.max,
            self.name
        )
    }

    fn as_aggregator(&self) -> Option<&dyn Aggregator> {
        Some(self)
    }
}

/// `STRICTMAPPER.StringentMacroAsMapperFunction`: compiled into a macro, it replaces a series
/// outside the bounds by `[ NULL ]` (no result for a macro mapper) and runs the macro on the
/// others.
#[derive(Debug)]
pub struct StrictMacro {
    /// The name of the function that built it.
    pub name: &'static str,
    /// The bounds.
    pub bounds: Bounds,
    /// The wrapped macro.
    pub mapper: Rc<Macro>,
}

impl StackObject for StrictMacro {
    fn name(&self) -> &str {
        self.name
    }

    /// The Java `toString()`.
    fn to_snapshot(&self) -> String {
        format!(
            "<% {} {} {} {} %> EVAL",
            self.mapper.snapshot(true),
            self.bounds.min,
            self.bounds.max,
            self.name
        )
    }

    fn apply(&self, stack: &mut Stack) -> Result<()> {
        let top = stack.pop()?;
        let Value::Gts(gts) = &top else {
            stack.push(top);
            return Err(Error::new(format!(
                "{}'s resulting function can only be applied on a GTS.",
                self.name
            )));
        };
        let (size, timespan) = {
            let gts = gts.borrow();
            let size = gts.size() as i64;
            let timespan = if size > 0 {
                gts.last_tick()
                    .wrapping_sub(gts.first_tick())
                    .wrapping_add(1)
            } else {
                0
            };
            (size, timespan)
        };
        if !self.bounds.admit(size, timespan) {
            stack.push(Value::list(vec![Value::Null]));
            return Ok(());
        }
        stack.push(top);
        stack.exec_macro(&self.mapper)
    }
}

/// `STRICTMAPPER`: `mapper|macro min max STRICTMAPPER`.
fn strict_mapper_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    // `((Number) o).longValue()`, `Long.MIN_VALUE` moved one up.
    let bound = |value: Value| Num::of(&value).map(|n| n.long_value().max(i64::MIN + 1));
    let Some(max) = bound(stack.pop()?) else {
        return Err(Error::usage(
            name,
            "expects a maximum (inclusive) number of values or a minimum timespan on top of the stack.",
        ));
    };
    let Some(min) = bound(stack.pop()?) else {
        return Err(Error::usage(
            name,
            "expects a minimum (inclusive) number of values or a minimum timespan below the top of the stack.",
        ));
    };
    if (min > 0 && max >= 0 || min < 0 && max <= 0) && min.abs() > max.abs() {
        return Err(Error::usage(
            name,
            "expects abs(min) <= abs(max) when min and max both express a count or both express a duration",
        ));
    }
    let bounds = Bounds { min, max };
    match stack.pop()? {
        Value::Function(mapper) if mapper.as_aggregator().is_some_and(|a| a.roles().mapper) => {
            stack.push(Value::Function(Rc::new(StrictMapper {
                name,
                bounds,
                mapper,
            })));
        }
        Value::Macro(mapper) => {
            let object = StrictMacro {
                name,
                bounds,
                mapper,
            };
            stack.push(Value::Macro(Rc::new(Macro::from_statements(vec![
                Statement::Call(Callable::Object(Rc::new(object))),
            ]))));
        }
        _ => {
            return Err(Error::usage(
                name,
                "expects a mapper or a macro below the extrema defining the value count range or timespan.",
            ));
        }
    }
    Ok(())
}

/// `STRICTREDUCER.StringentReducer`: the reducer applied only to the ticks where every series
/// has a value.
pub struct StrictReducer {
    /// The name of the function that built the wrapper.
    pub name: &'static str,
    /// The wrapped reducer.
    pub reducer: Rc<dyn FunctionObject>,
}

impl Aggregator for StrictReducer {
    fn roles(&self) -> Roles {
        Roles {
            bucketizer: false,
            mapper: false,
            reducer: true,
        }
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        match self.delegate(args) {
            Some(reducer) => reducer.apply(args),
            None => Ok(nothing(args)),
        }
    }

    fn delegate(&self, args: &AggregatorArgs) -> Option<&dyn Aggregator> {
        if args.values.iter().any(Value::is_null) {
            return None;
        }
        self.reducer.as_aggregator()
    }
}

impl FunctionObject for StrictReducer {
    fn name(&self) -> &str {
        self.name
    }

    fn type_name(&self) -> &'static str {
        "AGGREGATOR"
    }

    fn snapshot(&self) -> String {
        format!("<% {} {} %> EVAL", self.reducer.snapshot(), self.name)
    }

    fn as_aggregator(&self) -> Option<&dyn Aggregator> {
        Some(self)
    }
}

/// `STRICTREDUCER`: `reducer STRICTREDUCER`.
fn strict_reducer_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let reducer = match stack.pop()? {
        Value::Function(reducer) if reducer.as_aggregator().is_some_and(|a| a.roles().reducer) => {
            reducer
        }
        _ => {
            return Err(Error::usage(name, "expects a reducer on top of the stack."));
        }
    };
    stack.push(Value::Function(Rc::new(StrictReducer { name, reducer })));
    Ok(())
}
