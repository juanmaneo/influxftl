// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `join`: the concatenation of the string representations of the values, built by a stack
//! function consuming the separator.

use super::{Aggregated, Aggregator, AggregatorArgs, Named, Roles, latest};
use crate::error::{Error, Result};
use crate::format::url_encode;
use crate::functions::Entry;
use crate::stack::Stack;
use crate::value::Value;

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "bucketizer.join",
        Entry::Builder(|s, n| join_builder(s, n, true, false, None)),
    ),
    (
        "mapper.join",
        Entry::Builder(|s, n| join_builder(s, n, true, false, None)),
    ),
    (
        "reducer.join",
        Entry::Builder(|s, n| join_builder(s, n, true, false, None)),
    ),
    (
        "reducer.join.forbid-nulls",
        Entry::Builder(|s, n| join_builder(s, n, false, false, None)),
    ),
    (
        "reducer.join.nonnull",
        Entry::Builder(|s, n| join_builder(s, n, false, false, None)),
    ),
    (
        "reducer.join.urlencoded",
        Entry::Builder(|s, n| join_builder(s, n, false, true, Some(""))),
    ),
];

/// `Join`: the values rendered with `toString()` and joined by the separator; location and
/// elevation of the most recent datapoint.
#[derive(Clone)]
pub struct Join {
    /// The separator.
    pub separator: String,
    /// Whether `null` values are skipped rather than making the result `null`.
    pub ignore_nulls: bool,
    /// Whether the rendered values are percent-encoded.
    pub urlencode: bool,
    /// What a `null` value contributes, when anything.
    pub null_string: Option<&'static str>,
}

impl Aggregator for Join {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.ticks.is_empty() {
            return Ok(Aggregated::none());
        }
        let (location, elevation) = latest(args);
        let mut joined = String::new();
        let mut has_nulls = false;
        let mut first = true;
        for value in args.values {
            if value.is_null() {
                has_nulls = true;
                if let Some(null_string) = self.null_string {
                    if !first {
                        joined.push_str(&self.separator);
                    }
                    joined.push_str(null_string);
                }
                // A skipped null still ends the "first" element.
                first = false;
                continue;
            }
            if !first {
                joined.push_str(&self.separator);
            }
            first = false;
            if self.urlencode {
                joined.push_str(&url_encode(&value.to_string()));
            } else {
                joined.push_str(&value.to_string());
            }
        }
        let value = if has_nulls && !self.ignore_nulls {
            Value::Null
        } else {
            Value::from(joined)
        };
        Ok(Aggregated {
            tick: 0,
            location,
            elevation,
            value,
        })
    }

    fn parameters(&self) -> Option<String> {
        Some(format!("'{}'", url_encode(&self.separator)))
    }
}

/// `Join.Builder`: `separator name` pushes the configured aggregator.
fn join_builder(
    stack: &mut Stack,
    name: &'static str,
    ignore_nulls: bool,
    urlencode: bool,
    null_string: Option<&'static str>,
) -> Result<()> {
    let Value::String(separator) = stack.pop()? else {
        return Err(Error::new(format!("{name} expects a STRING seperator.")));
    };
    stack.push(Value::Function(Named::object(
        name,
        Join {
            separator: separator.to_string(),
            ignore_nulls,
            urlencode,
            null_string,
        },
    )));
    Ok(())
}
