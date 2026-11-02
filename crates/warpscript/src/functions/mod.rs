// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The WarpLib function registry and the shared helpers used to implement functions.
//!
//! Every function group lives in its own module and exposes a `FUNCTIONS` table of
//! `(name, Entry)` pairs. The registry is the union of all tables; names must be unique.

pub mod args;
pub mod capabilities;
pub mod collections;
pub mod compression;
pub mod control;
pub mod crypto;
pub mod functional;
pub mod json;
pub mod limits;
pub mod math;
pub mod misc;
pub mod numerical;
pub mod operators;
pub mod pack;
pub mod quaternions;
pub mod runtime;
pub mod stack_ops;
pub mod stringformat;
pub mod strings;
pub mod symbols;
pub mod template;
pub mod time;

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

use crate::error::{Error, Result};
use crate::macros::StackFn;
use crate::stack::Stack;
use crate::value::FunctionObject;

/// Constructor of a function object (mapper, reducer…) from its registered name.
pub type ObjectFn = fn(&'static str) -> Rc<dyn FunctionObject>;

/// A registry entry.
pub enum Entry {
    /// A stack function: executed as soon as it is encountered (or compiled into a macro).
    Stack(StackFn),
    /// A function object: pushed onto the stack when encountered (`mapper.sum`…).
    Object(ObjectFn),
}

/// All function tables, in registration order.
const TABLES: &[&[(&str, Entry)]] = &[
    stack_ops::FUNCTIONS,
    symbols::FUNCTIONS,
    control::FUNCTIONS,
    misc::FUNCTIONS,
    operators::FUNCTIONS,
    math::FUNCTIONS,
    strings::FUNCTIONS,
    json::FUNCTIONS,
    collections::FUNCTIONS,
    functional::FUNCTIONS,
    pack::FUNCTIONS,
    crypto::FUNCTIONS,
    compression::FUNCTIONS,
    capabilities::FUNCTIONS,
    limits::FUNCTIONS,
    runtime::FUNCTIONS,
    runtime::SNAPSHOT_FUNCTIONS,
    time::FUNCTIONS,
    quaternions::FUNCTIONS,
    stringformat::FUNCTIONS,
    template::FUNCTIONS,
];

static REGISTRY: OnceLock<HashMap<&'static str, &'static Entry>> = OnceLock::new();

/// Function objects registered only in test builds, until the first real ones (mappers,
/// reducers…) are ported.
#[cfg(test)]
pub(crate) mod test_objects {
    use std::rc::Rc;

    use super::Entry;
    use crate::value::FunctionObject;

    /// A function object that does nothing but carry its name.
    pub struct TestObject(&'static str);

    impl FunctionObject for TestObject {
        fn name(&self) -> &str {
            self.0
        }

        fn type_name(&self) -> &'static str {
            "OPERATOR"
        }
    }

    fn build(name: &'static str) -> Rc<dyn FunctionObject> {
        Rc::new(TestObject(name))
    }

    /// The test-only table.
    pub const FUNCTIONS: &[(&str, Entry)] = &[("test.object", Entry::Object(build))];
}

#[cfg(test)]
fn extra_tables() -> &'static [&'static [(&'static str, Entry)]] {
    &[test_objects::FUNCTIONS]
}

#[cfg(not(test))]
fn extra_tables() -> &'static [&'static [(&'static str, Entry)]] {
    &[]
}

/// The registry: WarpScript name → entry.
pub fn registry() -> &'static HashMap<&'static str, &'static Entry> {
    REGISTRY.get_or_init(|| {
        let mut map = HashMap::new();
        for table in TABLES.iter().chain(extra_tables()) {
            for (name, entry) in *table {
                map.insert(*name, entry);
            }
        }
        map
    })
}

/// Look a function up by name; returns the canonical (static) name and the entry.
pub fn lookup(name: &str) -> Option<(&'static str, &'static Entry)> {
    registry().get_key_value(name).map(|(k, v)| (*k, *v))
}

/// Names of every function known to the runtime, sorted.
pub fn registered_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = registry().keys().copied().collect();
    names.sort_unstable();
    names
}

/// Apply the stack function `name` to `stack`.
pub fn call(stack: &mut Stack, name: &str) -> Result<()> {
    match lookup(name) {
        Some((_, Entry::Stack(func))) => func(stack),
        _ => Err(Error::new(format!("Unknown function '{name}'"))),
    }
}

#[cfg(test)]
mod args_tests;
#[cfg(test)]
mod capabilities_tests;
#[cfg(test)]
mod collections_tests;
#[cfg(test)]
mod compression_tests;
#[cfg(test)]
mod control_tests;
#[cfg(test)]
mod crypto_tests;
#[cfg(test)]
mod functional_tests;
#[cfg(test)]
mod json_tests;
#[cfg(test)]
mod limits_tests;
#[cfg(test)]
mod math_tests;
#[cfg(test)]
mod misc_tests;
#[cfg(test)]
mod numerical_tests;
#[cfg(test)]
mod operators_tests;
#[cfg(test)]
mod pack_tests;
#[cfg(test)]
mod quaternions_tests;
#[cfg(test)]
mod runtime_tests;
#[cfg(test)]
mod stack_ops_tests;
#[cfg(test)]
mod stringformat_tests;
#[cfg(test)]
mod strings_tests;
#[cfg(test)]
mod symbols_tests;
#[cfg(test)]
mod template_tests;
#[cfg(test)]
mod time_tests;
