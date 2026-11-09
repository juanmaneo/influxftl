// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `SMARTPARSE`: Geo Time Series from the named groups of a regular expression.

use std::rc::Rc;

use fancy_regex::{Captures, Regex};
use indexmap::IndexMap;

use super::Entry;
use super::args::fail;
use super::gts::gts_or_list_try;
use super::misc::TIME_UNITS_PER_MS;
use super::operators::parse_java_double;
use super::strings::{compile_regex, regex_error};
use super::time::TIME_UNITS_PER_S;
use crate::error::Result;
use crate::error::fault::BoundsError;
use crate::geoxp::to_geoxp_point;
use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION};
use crate::javamap::JavaHashMap;
use crate::jmath;
use crate::pickle::string_hash;
use crate::stack::Stack;
use crate::unpickle::parse_long;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[("SMARTPARSE", Entry::Stack(smartparse))];

const NAME: &str = "SMARTPARSE";

/// The compiled regular expression and the roles of its named groups.
struct Parser {
    regex: Regex,
    values: Vec<String>,
    labels: Vec<String>,
    timestamps: Vec<String>,
    lat: Option<String>,
    lon: Option<String>,
    elev: Option<String>,
}

/// A series being built, with its Java `Metadata.hashCode`.
struct Series {
    hash: i32,
    gts: GeoTimeSerie,
}

fn smartparse(stack: &mut Stack) -> Result<()> {
    // A STRING text under the regexp, as opposed to a GTS or a list of GTS.
    let text = match (stack.get(0)?, stack.get(1)?) {
        (Value::String(_), Value::String(text)) => Some(Rc::clone(text)),
        _ => None,
    };
    let parser = parser(stack)?;
    if let Some(text) = text {
        // The text sits right under the regexp that `parser` popped, so this cannot fail.
        let _ = stack.pop();
        let mut results = Vec::new();
        parser.parse(&text, 0, &mut results)?;
        stack.push(series_list(results));
        return Ok(());
    }
    gts_or_list_try(stack, NAME, |gts| {
        let gts = gts.borrow();
        let mut results = Vec::new();
        for idx in 0..gts.size() {
            let text = gts.value_at(idx).to_string();
            parser.parse(&text, gts.tick_at(idx), &mut results)?;
        }
        Ok(series_list(results))
    })
}

/// `retrieveParameters`: the regular expression on top of the stack.
fn parser(stack: &mut Stack) -> Result<Parser> {
    // `smartparse` checked that two elements are on the stack, so the pop cannot fail.
    let Value::String(source) = stack.pop().unwrap_or(Value::Null) else {
        return Err(fail(NAME, "expects a STRING regexp."));
    };
    let regex = compile_regex(&source).map_err(|err| regex_error(&err))?;
    let mut parser = Parser {
        regex,
        values: Vec::new(),
        labels: Vec::new(),
        timestamps: Vec::new(),
        lat: None,
        lon: None,
        elev: None,
    };
    // The group names as written in the pattern, `(?<name>`.
    let bytes = source.as_bytes();
    let mut i = 0;
    while let Some(pos) = source[i..].find("(?<") {
        let start = i + pos + 3;
        let len = bytes[start..]
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric())
            .count();
        i = start;
        let name = &source[start..start + len];
        if len == 0 || !bytes[start].is_ascii_alphabetic() || bytes.get(start + len) != Some(&b'>')
        {
            continue;
        }
        match name {
            _ if name.starts_with('V') => parser.values.push(name.to_owned()),
            _ if name.starts_with('L') => parser.labels.push(name.to_owned()),
            _ if name.starts_with('T') => parser.timestamps.push(name.to_owned()),
            "lat" => parser.lat = Some(name.to_owned()),
            "lon" => parser.lon = Some(name.to_owned()),
            "elev" => parser.elev = Some(name.to_owned()),
            _ => {}
        }
    }
    Ok(parser)
}

/// The value of the named group `name`, when it participated in the match.
fn group<'t>(caps: &Captures<'t, str>, name: &str) -> Option<&'t str> {
    caps.name(name).map(|m| m.as_str())
}

/// `Double.parseDouble`, failing with a `NumberFormatException`.
fn parse_double(text: &str) -> Result<f64> {
    Ok(parse_java_double(text)?)
}

impl Parser {
    /// `doParse`: the datapoints of every match of the expression in `text`, added to the
    /// series of `results` (in creation order).
    fn parse(&self, text: &str, mut timestamp: i64, results: &mut Vec<Series>) -> Result<()> {
        for caps in self.regex.captures_iter(text) {
            let caps = caps.map_err(|err| regex_error(&err))?;
            let mut labels = JavaHashMap::new();
            for name in &self.labels {
                if let Some(value) = group(&caps, name) {
                    let key = name[1..].to_owned();
                    labels.put(string_hash(&key), key, value.to_owned());
                }
            }
            let mut location = NO_LOCATION;
            if let (Some(lat), Some(lon)) = (&self.lat, &self.lon)
                && let (Some(lat), Some(lon)) = (group(&caps, lat), group(&caps, lon))
            {
                location = to_geoxp_point(parse_double(lat)?, parse_double(lon)?);
            }
            let mut elevation = NO_ELEVATION;
            if let Some(elev) = self.elev.as_deref().and_then(|name| group(&caps, name)) {
                elevation = jmath::round(parse_double(elev)?);
            }
            for name in &self.timestamps {
                let Some(value) = group(&caps, name) else {
                    continue;
                };
                let ts = if name.starts_with("Ts") {
                    parse_double(value)? * TIME_UNITS_PER_S as f64
                } else if name.starts_with("Tms") {
                    parse_double(value)? * TIME_UNITS_PER_MS as f64
                } else if name.starts_with("Tus") {
                    parse_double(value)? / (1_000_000.0 / TIME_UNITS_PER_S as f64)
                } else if name.starts_with("Tns") {
                    parse_double(value)? / (1_000_000_000.0 / TIME_UNITS_PER_S as f64)
                } else {
                    continue;
                };
                timestamp = ts as i64;
                break;
            }
            for name in &self.values {
                let Some(text) = group(&caps, name) else {
                    continue;
                };
                let value = match name.as_bytes().get(1) {
                    Some(b'L') => Value::Long(parse_long(text)?),
                    Some(b'D') => Value::Double(parse_double(text)?),
                    Some(b'B') => Value::Boolean(text.eq_ignore_ascii_case("true")),
                    Some(b'S') => Value::from(text),
                    Some(_) => continue,
                    None => {
                        return Err(BoundsError::StringIndex {
                            index: 1,
                            length: 1,
                        }
                        .into());
                    }
                };
                let class = name[2..].to_owned();
                // `new HashMap<>(labels)`.
                let mut copy = JavaHashMap::new();
                copy.put_all(labels.iter().cloned().collect());
                let label_map: IndexMap<String, String> = copy
                    .into_entries()
                    .into_iter()
                    .map(|(_, k, v)| (k, v))
                    .collect();
                let series = match results.iter_mut().position(|s| {
                    s.gts.name() == Some(class.as_str()) && s.gts.metadata.labels == label_map
                }) {
                    Some(index) => &mut results[index],
                    None => {
                        let mut gts = GeoTimeSerie::new();
                        gts.metadata.name = Some(class.clone());
                        let hash = metadata_hash(&class, &label_map);
                        gts.metadata.labels = label_map;
                        results.push(Series { hash, gts });
                        let last = results.len() - 1;
                        &mut results[last]
                    }
                };
                series
                    .gts
                    .set_value(timestamp, location, elevation, &value, false);
            }
        }
        Ok(())
    }
}

/// The Thrift `Metadata.hashCode` of a `new Metadata()` given a name and labels.
fn metadata_hash(name: &str, labels: &IndexMap<String, String>) -> i32 {
    const SET: i32 = 131_071;
    const UNSET: i32 = 524_287;
    let labels_hash = labels.iter().fold(0i32, |h, (k, v)| {
        h.wrapping_add(string_hash(k) ^ string_hash(v))
    });
    // name, labels, classId, labelsId, attributes, source, lastActivity
    [
        SET,
        string_hash(name),
        SET,
        labels_hash,
        0,
        0,
        UNSET,
        UNSET,
        UNSET,
    ]
    .iter()
    .fold(1i32, |h, x| h.wrapping_mul(8191).wrapping_add(*x))
}

/// `new ArrayList<>(results.values())`: the series in Java `HashMap` order.
fn series_list(results: Vec<Series>) -> Value {
    let mut map = JavaHashMap::new();
    for (index, series) in results.into_iter().enumerate() {
        map.put(series.hash, index, series.gts);
    }
    Value::list(
        map.into_entries()
            .into_iter()
            .map(|(_, _, gts)| Value::Gts(gts.shared()))
            .collect(),
    )
}
