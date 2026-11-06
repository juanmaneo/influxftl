// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `PARSESELECTOR` and `TOSELECTOR`: from a `class{labels}` selector to its parts, and from a
//! Geo Time Series to the selector that selects it.

use super::Entry;
use crate::error::{Error, Result};
use crate::selector::{build_selector, parse_selector};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PARSESELECTOR", Entry::Stack(parseselector)),
    ("TOSELECTOR", Entry::Stack(toselector)),
];

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
