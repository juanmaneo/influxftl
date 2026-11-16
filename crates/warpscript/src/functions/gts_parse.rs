// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `PARSE`: Geo Time Series from text in the GTS input format.

use super::Entry;
use super::args::fail;
use super::runtime::current_time;
use crate::encoder::GtsEncoder;
use crate::error::{Error, Result};
use crate::gtsparse::{LineError, parse_line};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[("PARSE", Entry::Stack(parse))];

/// `BufferedReader.readLine`: lines ended by `\n`, `\r` or `\r\n`.
fn split_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find(['\n', '\r']) {
            Some(end) => {
                lines.push(&rest[..end]);
                let skip = if rest[end..].starts_with("\r\n") {
                    2
                } else {
                    1
                };
                rest = &rest[end + skip..];
            }
            None => {
                lines.push(rest);
                rest = "";
            }
        }
    }
    lines
}

/// The series of an encoder.
fn series_of(encoder: &GtsEncoder) -> Value {
    Value::Gts(encoder.decoder().decode().unwrap_or_default().shared())
}

/// `PARSE`: the series of the lines of a text (blank lines and `#` comments skipped).
fn parse(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PARSE";
    let Value::String(text) = stack.pop()? else {
        return Err(fail(NAME, "expects a String on top of the stack."));
    };
    let now = current_time();
    let mut series = Vec::new();
    let mut current: Option<GtsEncoder> = None;
    for line in split_lines(&text) {
        let line = line.trim_matches(|c: char| c <= ' ');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parsed = parse_line(current.as_ref(), line, now).map_err(|err| match err {
            LineError::Parse(pe) => Error::with_cause(
                format!("{NAME} could not parse at index {} in '{line}'", pe.offset),
                pe.into_error(),
            ),
            LineError::Other(err) => err,
        })?;
        if let Some((name, labels)) = parsed.metadata {
            let same = current.as_ref().is_some_and(|c| {
                c.metadata.name.as_deref() == Some(name.as_str()) && c.metadata.labels == labels
            });
            if !same {
                if let Some(previous) = current.take() {
                    series.push(series_of(&previous));
                }
                let mut encoder = GtsEncoder::new(0);
                encoder.metadata.name = Some(name);
                encoder.metadata.labels = labels;
                current = Some(encoder);
            }
        }
        // A line without metadata continues the current series (`parse_line` checks there is
        // one): the default is never inserted.
        let encoder = current.get_or_insert(GtsEncoder::new(0));
        if let Some(attributes) = parsed.attributes {
            encoder.metadata.attributes = attributes;
        }
        let (tick, location, elevation, value) = parsed.point;
        encoder.add_value(tick, location, elevation, &value);
    }
    if let Some(encoder) = current {
        series.push(series_of(&encoder));
    }
    stack.push(Value::list(series));
    Ok(())
}
