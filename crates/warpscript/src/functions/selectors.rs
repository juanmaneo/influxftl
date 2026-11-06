// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `PARSESELECTOR` and `TOSELECTOR`: from a `class{labels}` selector to its parts, and from a
//! Geo Time Series to the selector that selects it; `METAMATCH`, and the `ATTRDELTA` /
//! `ATTRSKIP` flags of the metadata updates.

use super::Entry;
use crate::error::{Error, Result};
use crate::selector::{MetadataSelectorMatcher, build_selector, parse_selector};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PARSESELECTOR", Entry::Stack(parseselector)),
    ("TOSELECTOR", Entry::Stack(toselector)),
    ("METAMATCH", Entry::Stack(metamatch)),
    (
        "ATTRDELTA",
        Entry::Stack(|s| attribute_flag(s, "ATTRDELTA", ATTRIBUTE_ATTRIBUTES_DELTA)),
    ),
    (
        "ATTRSKIP",
        Entry::Stack(|s| attribute_flag(s, "ATTRSKIP", ATTRIBUTE_ATTRIBUTES_SKIP)),
    ),
];

/// `WarpScriptStack.ATTRIBUTE_ATTRIBUTES_DELTA`: `META` updates the attributes instead of
/// replacing them.
pub const ATTRIBUTE_ATTRIBUTES_DELTA: &str = "attributes.delta";
/// `WarpScriptStack.ATTRIBUTE_ATTRIBUTES_SKIP`: `UPDATE` does not store the attributes.
pub const ATTRIBUTE_ATTRIBUTES_SKIP: &str = "attributes.skip";

/// `ATTRDELTA` / `ATTRSKIP`: set the flag from a boolean, or push it for `NULL`.
fn attribute_flag(stack: &mut Stack, name: &str, attribute: &str) -> Result<()> {
    match stack.pop()? {
        Value::Boolean(b) => {
            stack.set_attribute(attribute, Some(Value::Boolean(b)));
        }
        Value::Null => {
            let set = matches!(stack.attribute(attribute), Some(Value::Boolean(true)));
            stack.push(Value::Boolean(set));
        }
        _ => {
            return Err(Error::new(format!(
                "{name} invalid parameter, expected a BOOLEAN or NULL."
            )));
        }
    }
    Ok(())
}

/// `METAMATCH`: whether the metadata of a series or encoder (or of each of a list) matches a
/// `class{labels}` or `class{labels}{attributes}` selector.
fn metamatch(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "METAMATCH";
    let Value::String(selector) = stack.pop()? else {
        return Err(Error::new(format!("{NAME} expects a STRING selector.")));
    };
    let matcher = MetadataSelectorMatcher::new(&selector)?;
    let matches = |value: &Value| match value {
        Value::Gts(gts) => Some(matcher.matches(&gts.borrow().metadata)),
        Value::Encoder(encoder) => Some(matcher.matches(&encoder.borrow().metadata)),
        _ => None,
    };
    let top = stack.pop()?;
    let result = match &top {
        Value::List(list) => {
            let mut results = Vec::with_capacity(list.borrow().len());
            for element in list.borrow().iter() {
                let Some(matched) = matches(element) else {
                    return Err(Error::new(format!(
                        "{NAME} can only be applied to a LIST containing GTS or ENCODER elements."
                    )));
                };
                results.push(Value::Boolean(matched?));
            }
            Value::list(results)
        }
        other => match matches(other) {
            Some(matched) => Value::Boolean(matched?),
            None => {
                return Err(Error::new(format!(
                    "{NAME} operates on a GTS or ENCODER or a list thereof."
                )));
            }
        },
    };
    stack.push(result);
    Ok(())
}

/// `PARSESELECTOR`: push the class selector then the map of labels selectors.
fn parseselector(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PARSESELECTOR";
    let Value::String(selector) = stack.pop()? else {
        return Err(Error::new(format!(
            "{NAME} expects a String on top of the stack."
        )));
    };
    let wrap = |cause: Error| Error::with_cause(format!("{NAME} could not parse input."), cause);
    let (class, labels) = match parse_selector(&selector) {
        None => return Err(wrap(Error::new(" invalid syntax for selector."))),
        // The `IllegalArgumentException` of a bad percent-encoding is not wrapped.
        Some(Err(err)) if err.is_runtime() => return Err(err),
        Some(Err(err)) => return Err(wrap(err)),
        Some(Ok(parts)) => parts,
    };
    stack.push(Value::from(class));
    stack.push(Value::map(
        labels
            .into_iter()
            .map(|(k, v)| (Value::from(k), Value::from(v)))
            .collect(),
    ));
    Ok(())
}

/// `TOSELECTOR`: the selector of a series, or of each series of a list.
fn toselector(stack: &mut Stack) -> Result<()> {
    let selector = |value: &Value| match value {
        Value::Gts(gts) => Ok(Value::from(build_selector(&gts.borrow().metadata))),
        Value::Encoder(encoder) => Ok(Value::from(build_selector(&encoder.borrow().metadata))),
        _ => Err(Error::new(
            "TOSELECTOR expects a GeoTimeSeries, a GTSEncoder or a list thereof on top of the stack.",
        )),
    };
    let result = match stack.pop()? {
        Value::List(list) => Value::list(
            list.borrow()
                .iter()
                .map(selector)
                .collect::<Result<Vec<Value>>>()?,
        ),
        other => selector(&other)?,
    };
    stack.push(result);
    Ok(())
}
