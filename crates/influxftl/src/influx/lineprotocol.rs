// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! InfluxDB line protocol for the datapoints and the attributes of series.

use std::collections::BTreeMap;

use warpscript::finite::ensure_finite;
use warpscript::gts::{GeoTimeSerie, Values};

use super::error::InfluxError;
use super::schema::{
    ELEVATION, HHCODE, META_ATTRIBUTES, META_CLASS, META_DELETED, META_LABELS, META_TABLE,
    RESERVED_LABELS, VALUE_TYPE, type_columns,
};
use super::sql::nanoseconds;

/// The text with the characters of `special` (and the backslash) escaped.
fn escape(text: &str, special: &[char]) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c == '\\' || special.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A name or a value that sits between delimiters: it cannot be empty or hold a line break.
fn delimited(text: &str, special: &[char]) -> Result<String, InfluxError> {
    if text.is_empty() || text.contains(['\n', '\r']) {
        return Err(InfluxError::Unencodable(text.to_owned()));
    }
    Ok(escape(text, special))
}

/// A measurement (a class), which cannot start with the `#` of a comment either, nor be the
/// table of the attributes.
pub fn measurement(class: &str) -> Result<String, InfluxError> {
    if class == META_TABLE {
        return Err(InfluxError::ReservedClass(class.to_owned()));
    }
    if class.starts_with('#') {
        return Err(InfluxError::Unencodable(class.to_owned()));
    }
    delimited(class, &[',', ' '])
}

/// A tag key or a tag value.
pub fn tag(text: &str) -> Result<String, InfluxError> {
    delimited(text, &[',', '=', ' '])
}

/// A string field value, quoted.
pub fn string_field(text: &str) -> String {
    format!("\"{}\"", escape(text, &['"']))
}

/// The start of the lines of a series: its measurement and its tags, sorted by name.
///
/// # Errors
///
/// A class or a label that line protocol cannot carry, a label that is a column of the
/// datapoints, a label without a value.
pub fn series_key<'a>(
    class: &str,
    labels: impl IntoIterator<Item = (&'a String, &'a String)>,
) -> Result<String, InfluxError> {
    let mut tags = BTreeMap::new();
    for (name, value) in labels {
        if RESERVED_LABELS.contains(&name.as_str()) {
            return Err(InfluxError::ReservedLabel(name.clone()));
        }
        if value.is_empty() {
            return Err(InfluxError::EmptyLabelValue(name.clone()));
        }
        tags.insert(tag(name)?, tag(value)?);
    }
    let mut key = measurement(class)?;
    for (name, value) in tags {
        key.push_str(&format!(",{name}={value}"));
    }
    Ok(key)
}

/// The lines of the datapoints of a series, in the order of its ticks.
///
/// # Errors
///
/// A series that [`series_key`] refuses, a double that is not finite or a tick whose
/// nanoseconds do not fit 64 bits.
pub fn series_lines(class: &str, gts: &GeoTimeSerie) -> Result<Vec<String>, InfluxError> {
    let key = series_key(class, &gts.metadata.labels)?;
    let values: Vec<String> = match &gts.values {
        Values::Undefined => Vec::new(),
        Values::Long(values) => values.iter().map(|long| format!("{long}i")).collect(),
        Values::Double(values) => values
            .iter()
            .map(|&double| Ok(format!("{:?}", ensure_finite(double)?)))
            .collect::<Result<_, InfluxError>>()?,
        Values::Boolean(values) => values.iter().map(bool::to_string).collect(),
        Values::String(values) => values.iter().map(|text| string_field(text)).collect(),
    };
    let Some((marker, column)) = type_columns(gts.type_of()) else {
        return Ok(Vec::new());
    };
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let tick = gts.tick_at(index);
            let time = nanoseconds(tick).ok_or(InfluxError::TickOutOfRange(tick))?;
            Ok(format!(
                "{key} {VALUE_TYPE}=\"{marker}\",{column}={value},{HHCODE}={}i,{ELEVATION}={}i {time}",
                gts.location_at(index),
                gts.elevation_at(index),
            ))
        })
        .collect()
}

/// The line of the attributes of a series, as of `time` (nanoseconds); `labels` and
/// `attributes` are JSON objects.
///
/// # Errors
///
/// A class that line protocol cannot carry.
pub fn meta_line(
    class: &str,
    labels: &str,
    attributes: &str,
    deleted: bool,
    time: i64,
) -> Result<String, InfluxError> {
    Ok(format!(
        "{META_TABLE},{META_CLASS}={},{META_LABELS}={} {META_ATTRIBUTES}={},{META_DELETED}={deleted} {time}",
        tag(class)?,
        tag(labels)?,
        string_field(attributes),
    ))
}

#[cfg(test)]
#[path = "lineprotocol_tests.rs"]
mod lineprotocol_tests;
