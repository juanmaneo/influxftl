// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `MACROMAPPER`, `MACROREDUCER` and `MACROBUCKETIZER` (`MACROMAPPER.MacroMapperWrapper`): a
//! macro wrapped into an aggregator the three frameworks accept. The frameworks run the macro
//! on their stack with the arguments as lists (`[ tick [ names ] [ labels ] [ ticks ] [ lats ]
//! [ lons ] [ elevations ] [ values ] ]`), see `gts_frameworks::aggregate`.

use std::rc::Rc;

use super::{Aggregated, Aggregator, AggregatorArgs, Roles};
use crate::error::{Error, Result};
use crate::functions::Entry;
use crate::macros::Macro;
use crate::stack::Stack;
use crate::value::{FunctionObject, Value};

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("MACROMAPPER", Entry::Builder(wrapper_builder)),
    ("MACROREDUCER", Entry::Builder(wrapper_builder)),
    ("MACROBUCKETIZER", Entry::Builder(wrapper_builder)),
];

/// `MacroMapperWrapper`: a macro usable as a mapper, reducer or bucketizer.
pub struct MacroWrapper {
    /// The name of the function that built the wrapper.
    pub name: &'static str,
    /// The wrapped macro.
    pub wrapped: Rc<Macro>,
}

impl Aggregator for MacroWrapper {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    /// The frameworks run [`MacroWrapper::wrapped`] themselves; without a stack there is
    /// nothing to run it on.
    fn apply(&self, _args: &AggregatorArgs) -> Result<Aggregated> {
        Err(Error::usage(
            self.name,
            "can only be applied by BUCKETIZE, MAP or REDUCE.",
        ))
    }

    fn wrapped_macro(&self) -> Option<&Rc<Macro>> {
        Some(&self.wrapped)
    }
}

impl FunctionObject for MacroWrapper {
    fn name(&self) -> &str {
        self.name
    }

    fn type_name(&self) -> &'static str {
        "AGGREGATOR"
    }

    /// `<% <% … %> MACROMAPPER %> EVAL`, like the Java `toString()`.
    fn snapshot(&self) -> String {
        format!("<% {} {} %> EVAL", self.wrapped.snapshot(true), self.name)
    }

    fn as_aggregator(&self) -> Option<&dyn Aggregator> {
        Some(self)
    }
}

/// `MACROMAPPER`: `macro MACROMAPPER` pushes the wrapper of the macro.
fn wrapper_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let Value::Macro(wrapped) = stack.pop()? else {
        return Err(Error::usage(name, "expects a macro on top of the stack."));
    };
    stack.push(Value::Function(Rc::new(MacroWrapper { name, wrapped })));
    Ok(())
}
