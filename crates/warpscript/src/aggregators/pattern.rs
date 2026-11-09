// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The regular expression mappers: `'regexp' mapper.regexp.match` (the first value of the
//! window the pattern matches entirely) and `'regexp' 'replacement' mapper.regexp.replace`
//! (`String.replaceAll` on the value), with the Java pattern support of `MATCH` / `REPLACE`.

use std::rc::Rc;

use super::single::{empty, single_value};
use super::{Aggregated, Aggregator, AggregatorArgs, Named, Roles};
use crate::error::{Error, Result};
use crate::functions::Entry;
use crate::functions::strings::{CompiledPattern, regex_replace};
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::stack::Stack;
use crate::value::{Value, ValueKind};

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("mapper.regexp.match", Entry::Builder(match_builder)),
    ("mapper.regexp.replace", Entry::Builder(replace_builder)),
];

/// `(String) value` handed to `Pattern.matcher`: the `NullPointerException` of a `null`, the
/// `ClassCastException` of another type.
fn string_of(value: &Value) -> Result<&str> {
    match value {
        Value::String(s) => Ok(s),
        Value::Null => Err(Error::null_pointer()),
        other => Err(Error::class_cast(other.kind(), ValueKind::String)),
    }
}

/// `MapperRegExpMatch`: the first datapoint of the window whose value the pattern matches
/// entirely (`Matcher.matches()`), at the current tick with its location and elevation.
pub struct RegExpMatch {
    /// The compiled pattern.
    pub pattern: CompiledPattern,
}

impl Aggregator for RegExpMatch {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        for (i, value) in args.values.iter().enumerate() {
            if !self.pattern.full_match(string_of(value)?)?.is_empty() {
                return Ok(Aggregated {
                    tick: args.tick,
                    location: args.locations[i],
                    elevation: args.elevations[i],
                    value: value.clone(),
                });
            }
        }
        Ok(Aggregated {
            tick: args.tick,
            location: NO_LOCATION,
            elevation: NO_ELEVATION,
            value: Value::Null,
        })
    }
}

/// `Pattern.compile` in a builder: `<name> expects a valid regular expression.`, caused by
/// the syntax error.
fn compile(source: &str, name: &str) -> Result<CompiledPattern> {
    CompiledPattern::compile(source).map_err(|err| {
        Error::with_cause(format!("{name} expects a valid regular expression."), err)
    })
}

/// `MapperRegExpMatch.Builder`: `'regexp' mapper.regexp.match`.
fn match_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let Value::String(source) = stack.pop()? else {
        return Err(Error::new(format!(
            "{name} expects a regular expression STRING."
        )));
    };
    let pattern = compile(&source, name)?;
    stack.push(Value::Function(Named::object(
        name,
        RegExpMatch { pattern },
    )));
    Ok(())
}

/// `MapperRegExpReplace`: every match of the pattern in the single value of the window
/// replaced (`Matcher.replaceAll`).
pub struct RegExpReplace {
    /// The registered name, for the error messages.
    pub name: &'static str,
    /// The pattern source (validated by the builder).
    pub source: Rc<str>,
    /// The replacement, with Java's `$n` / `${name}` / `\` syntax.
    pub replacement: Rc<str>,
}

impl Aggregator for RegExpReplace {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let Some(value) = single_value(args, self.name)? else {
            return Ok(empty());
        };
        let replaced = regex_replace(string_of(value)?, &self.source, &self.replacement, true)
            .map_err(|err| {
                // `Matcher.appendReplacement` throws runtime exceptions.
                let message = err.message();
                let class = if message.starts_with("No group ") {
                    "java.lang.IndexOutOfBoundsException"
                } else {
                    "java.lang.IllegalArgumentException"
                };
                Error::runtime(class, message)
            })?;
        Ok(Aggregated {
            tick: args.tick,
            location: args.locations[0],
            elevation: args.elevations[0],
            value: Value::string(replaced),
        })
    }
}

/// `MapperRegExpReplace.Builder`: `'regexp' 'replacement' mapper.regexp.replace`.
fn replace_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let Value::String(replacement) = stack.pop()? else {
        return Err(Error::new(format!("{name} expects a replacement STRING.")));
    };
    let Value::String(source) = stack.pop()? else {
        return Err(Error::new(format!(
            "{name} expects a regular expression STRING."
        )));
    };
    compile(&source, name)?;
    stack.push(Value::Function(Named::object(
        name,
        RegExpReplace {
            name,
            source,
            replacement,
        },
    )));
    Ok(())
}
