// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The datastore functions: `FETCH` (and its typed variants), `FIND`, `FINDSETS`, `FINDSTATS`,
//! `METASET`, `UPDATE`, `META`, `METADIFF` and `DELETE`. They parse their parameters like Warp 10's and
//! work on the [`Datastore`] connected to the stack; a script that has none fails with
//! [`DatastoreError::NotConnected`].
//!
//! The runtime has no Warp 10 token (InfluxDB authenticates the requests): the `token`
//! parameter has to be there, as it is in a script written for Warp 10, and is not read. The
//! parameters that depend on Warp 10's storage (`metaset`, `gts`, `extra`, `typeattr`,
//! `selpairs`, `active.after`, `quiet.after`, `sample`, `wtimestamp`, `ttl`, `showuuid`,
//! `encoders`) fail with [`DatastoreError::Unsupported`]; `docs/DATASTORE.md` has the details.

use std::collections::BTreeSet;
use std::sync::Arc;

use indexmap::IndexMap;

use super::Entry;
use super::args::{fail, fail_with_cause};
use super::compression::gzip_vec;
use super::crypto::{Kek, aes_wrap};
use super::misc::TIME_UNITS_PER_MS;
use super::runtime::current_time;
use super::selectors::parse_selector_text;
use super::strings::base64_decode;
use crate::datastore::{
    Datastore, DeleteRequest, FetchRequest, RangeNames, Selector, compute_time_range, get_timestamp,
};
use crate::encoder::GtsEncoder;
use crate::encoding::{hex_decode, opb64_encode};
use crate::error::datastore::DatastoreError;
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Metadata, Type};
use crate::stack::Stack;
use crate::thrift::{MetaSet, serialize_metaset};
use crate::value::{Value, ValueKind};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("FETCH", Entry::Stack(|s| fetch(s, "FETCH", None))),
    (
        "FETCHLONG",
        Entry::Stack(|s| fetch(s, "FETCHLONG", Some(Type::Long))),
    ),
    (
        "FETCHDOUBLE",
        Entry::Stack(|s| fetch(s, "FETCHDOUBLE", Some(Type::Double))),
    ),
    (
        "FETCHSTRING",
        Entry::Stack(|s| fetch(s, "FETCHSTRING", Some(Type::String))),
    ),
    (
        "FETCHBOOLEAN",
        Entry::Stack(|s| fetch(s, "FETCHBOOLEAN", Some(Type::Boolean))),
    ),
    ("FIND", Entry::Stack(|s| find(s, "FIND", false))),
    ("FINDSETS", Entry::Stack(|s| find(s, "FINDSETS", true))),
    ("FINDSTATS", Entry::Stack(findstats)),
    ("METASET", Entry::Stack(metaset)),
    ("UPDATE", Entry::Stack(update)),
    ("META", Entry::Stack(|s| meta(s, "META", false))),
    ("METADIFF", Entry::Stack(|s| meta(s, "METADIFF", true))),
    ("DELETE", Entry::Stack(delete)),
];

/// The parameters of a function, a map.
type Params = IndexMap<Value, Value>;

fn get<'a>(params: &'a Params, key: &str) -> Option<&'a Value> {
    params.get(&Value::from(key))
}

fn has(params: &Params, key: &str) -> bool {
    params.contains_key(&Value::from(key))
}

/// The datastore of the stack.
fn datastore(stack: &Stack, name: &str) -> Result<Arc<dyn Datastore>> {
    stack.datastore().ok_or_else(|| {
        DatastoreError::NotConnected {
            function: name.to_owned(),
        }
        .into()
    })
}

/// The failure of a parameter this runtime does not support.
fn unsupported(name: &str, parameter: &str) -> Error {
    DatastoreError::Unsupported {
        function: name.to_owned(),
        parameter: parameter.to_owned(),
    }
    .into()
}

/// A `LONG` parameter, when set.
fn long_param(name: &str, params: &Params, key: &str) -> Result<Option<i64>> {
    match get(params, key) {
        None => Ok(None),
        Some(Value::Long(long)) => Ok(Some(*long)),
        Some(_) => Err(fail(name, &format!("Invalid type for parameter '{key}'."))),
    }
}

/// A `LONG` parameter that must not be below `minimum`, when set.
fn bounded_param(name: &str, params: &Params, key: &str, minimum: i64) -> Result<Option<i64>> {
    let value = long_param(name, params, key)?;
    if value.is_some_and(|value| value < minimum) {
        return Err(fail(
            name,
            &format!("Parameter '{key}' must be >= {minimum}."),
        ));
    }
    Ok(value)
}

/// The selectors a map of parameters gives: `selectors`, `selector`, or `class` and `labels`.
/// `None` when it has none of them.
fn selectors_param(name: &str, params: &Params) -> Result<Option<Vec<Selector>>> {
    if let Some(selectors) = get(params, "selectors") {
        let Value::List(list) = selectors else {
            return Err(fail(name, "Invalid parameter 'selectors'"));
        };
        let selectors = list
            .borrow()
            .iter()
            .map(|selector| selector_of_text(&selector.to_string()))
            .collect::<Result<Vec<_>>>()?;
        return Ok(Some(selectors));
    }
    if let Some(selector) = get(params, "selector") {
        return Ok(Some(vec![selector_of_text(&selector.to_string())?]));
    }
    match (get(params, "class"), get(params, "labels")) {
        (Some(class), Some(labels)) => Ok(Some(vec![selector_of(name, class, labels)?])),
        _ => Ok(None),
    }
}

/// The selector of the text `class{labels}`.
fn selector_of_text(text: &str) -> Result<Selector> {
    let (class, labels) = parse_selector_text(text)?;
    Ok(Selector { class, labels })
}

/// The selector of a class selector and a map of labels selectors.
fn selector_of(name: &str, class: &Value, labels: &Value) -> Result<Selector> {
    let Value::Map(labels) = labels else {
        return Err(fail(name, "label selector must be a map."));
    };
    let mut selectors = IndexMap::new();
    for (key, value) in labels.borrow().iter() {
        let (Value::String(key), Value::String(value)) = (key, value) else {
            return Err(fail(
                name,
                "keys and values of label selector must be STRINGs.",
            ));
        };
        selectors.insert(key.to_string(), value.to_string());
    }
    let class = match class {
        Value::String(class) => class.to_string(),
        other => other.to_string(),
    };
    Ok(Selector {
        class,
        labels: selectors,
    })
}

// -----------------------------------------------------------------------------------------
// FETCH
// -----------------------------------------------------------------------------------------

/// The parameters of a fetch.
struct FetchParams {
    request: FetchRequest,
    /// The type given as a parameter.
    value_type: Option<Type>,
}

/// The map of parameters a list `[ token class labels end timespan ]` or
/// `[ token class labels start end ]` stands for.
fn fetch_list(name: &str, list: &[Value]) -> Result<Params> {
    let [token, class, labels, third, fourth] = list else {
        return Err(fail(name, "expects a list with 5 elements."));
    };
    let mut map = IndexMap::from([
        (Value::from("token"), token.clone()),
        (Value::from("class"), class.clone()),
        (Value::from("labels"), labels.clone()),
    ]);
    match (third, fourth) {
        (Value::Long(_), Value::Long(_)) => {
            map.insert(Value::from("end"), third.clone());
            map.insert(Value::from("timespan"), fourth.clone());
        }
        (Value::String(_), Value::String(_)) => {
            map.insert(Value::from("start"), third.clone());
            map.insert(Value::from("end"), fourth.clone());
        }
        _ => {
            return Err(fail(
                name,
                "expects 'start' and 'end' to be Strings or 'end' and 'timespan' to be Longs.",
            ));
        }
    }
    Ok(map)
}

/// `FETCH.paramsFromMap` and what follows it: the request of a map of parameters.
#[allow(clippy::too_many_lines)]
fn fetch_params(name: &str, map: &mut Params) -> Result<FetchParams> {
    for parameter in ["metaset", "gts", "extra", "typeattr", "selpairs"] {
        if has(map, parameter) {
            return Err(unsupported(name, parameter));
        }
    }
    if !has(map, "token") {
        return Err(fail(name, "Missing 'token' parameter"));
    }
    let Some(selectors) = selectors_param(name, map)? else {
        return Err(fail(
            name,
            "Missing 'metaset', 'gts', 'selector', 'selectors' or 'class' and 'labels' parameters.",
        ));
    };
    // A negative timespan is a count.
    if let Some(Value::Long(timespan)) = get(map, "timespan")
        && *timespan < 0
    {
        if has(map, "count") {
            return Err(fail(
                name,
                "cannot be given both 'count' and negative 'timespan'.",
            ));
        }
        let timespan = if *timespan == i64::MIN {
            *timespan + 1
        } else {
            *timespan
        };
        map.insert(Value::from("count"), Value::Long(-timespan));
        map.shift_remove(&Value::from("timespan"));
    }
    let count = match get(map, "count") {
        None => None,
        Some(Value::Long(count)) => Some(*count),
        Some(other) => return Err(Error::class_cast(other.kind(), ValueKind::Long)),
    };
    let names = RangeNames {
        start: "start",
        end: "end",
        timespan: "timespan",
        count: "count",
    };
    let range = compute_time_range(
        get(map, "start"),
        get(map, "end"),
        get(map, "timespan"),
        get(map, "count"),
        names,
        current_time(),
    )
    .map_err(|err| Error::with_cause(format!("{name} given invalid parameters."), err))?;
    let value_type = match get(map, "type").map(ToString::to_string) {
        None => None,
        Some(text) => Some(match text.to_ascii_uppercase().as_str() {
            "LONG" => Type::Long,
            "DOUBLE" => Type::Double,
            "STRING" => Type::String,
            "BOOLEAN" => Type::Boolean,
            _ => return Err(fail(name, "Invalid value for parameter 'type'.")),
        }),
    };
    let truth = |key: &str| matches!(get(map, key), Some(Value::Boolean(true)));
    if truth("wtimestamp") && truth("ttl") {
        return Err(fail(
            name,
            "Only one of 'wtimestamp' and 'ttl' can be set to true.",
        ));
    }
    for parameter in ["wtimestamp", "ttl", "showuuid", "encoders"] {
        if truth(parameter) {
            return Err(unsupported(name, parameter));
        }
    }
    for parameter in ["active.after", "quiet.after"] {
        if long_param(name, map, parameter)?.is_some() {
            return Err(unsupported(name, parameter));
        }
    }
    let boundary = long_param(name, map, "boundary")?;
    let boundary_pre = long_param(name, map, "boundary.pre")?
        .or(boundary)
        .unwrap_or(0);
    let boundary_post = long_param(name, map, "boundary.post")?
        .or(boundary)
        .unwrap_or(0);
    let skip = bounded_param(name, map, "skip", 0)?.unwrap_or(0);
    let step = bounded_param(name, map, "step", 1)?.unwrap_or(1);
    let timestep = bounded_param(name, map, "timestep", 1)?;
    match get(map, "sample") {
        None => {}
        Some(Value::Double(sample)) if *sample > 0.0 && *sample <= 1.0 => {
            return Err(unsupported(name, "sample"));
        }
        Some(Value::Double(_)) => {
            return Err(fail(
                name,
                "Parameter 'sample' must be in the range ( 0.0, 1.0 ].",
            ));
        }
        Some(_) => return Err(fail(name, "Invalid type for parameter 'sample'.")),
    }
    if let Some(priority) = get(map, "priority")
        && !matches!(priority, Value::List(_))
    {
        return Err(fail(
            name,
            "Invalid type for parameter 'priority', expected a LIST.",
        ));
    }
    let series_skip = bounded_param(name, map, "gskip", 0)?.unwrap_or(0);
    let series_count = bounded_param(name, map, "gcount", 0)?.unwrap_or(i64::MAX);
    Ok(FetchParams {
        request: FetchRequest {
            selectors,
            start: range.start,
            end: range.end,
            count,
            value_type,
            boundary_pre,
            boundary_post,
            keep_empty: truth("keepempty"),
            skip,
            step,
            timestep,
            series_skip,
            series_count,
        },
        value_type,
    })
}

/// `FETCH`, `FETCHLONG`, `FETCHDOUBLE`, `FETCHSTRING` and `FETCHBOOLEAN`: the Geo Time Series
/// of a list or a map of parameters, as a list. The typed variants only fetch the datapoints
/// of their type.
fn fetch(stack: &mut Stack, name: &str, forced: Option<Type>) -> Result<()> {
    let mut map = match stack.pop()? {
        Value::Map(map) => map.borrow().clone(),
        Value::List(list) => fetch_list(name, &list.borrow())?,
        _ => return Err(fail(name, "expects a map or a list as parameter.")),
    };
    let FetchParams {
        mut request,
        value_type,
    } = fetch_params(name, &mut map)?;
    request.value_type = forced.or(value_type);
    let series = datastore(stack, name)?.fetch(&request)?;
    stack.push(Value::list(
        series
            .into_iter()
            .map(|gts| Value::Gts(gts.shared()))
            .collect(),
    ));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// FIND, FINDSETS and FINDSTATS
// -----------------------------------------------------------------------------------------

/// The arguments of `FIND` and `FINDSTATS`: the selector, and the series to skip and to keep.
struct FindParams {
    selector: Selector,
    skip: i64,
    count: i64,
}

/// The token, class selector and labels selectors on the stack, in a list or not
/// (`$token 'class' { labels } FIND`).
fn find_arguments(stack: &mut Stack, name: &str, sizes: &str, flag: bool) -> Result<FindParams> {
    let top = stack.pop()?;
    let (token, class, labels) = match top {
        Value::List(list) => {
            let list = list.borrow();
            let accepted = if flag {
                list.len() == 3 || list.len() == 4
            } else {
                list.len() == 3
            };
            if !accepted {
                return Err(fail(name, &format!("expects {sizes} parameters.")));
            }
            (list[0].clone(), list[1].clone(), list[2].clone())
        }
        Value::Map(map) if is_map_parameters(&map.borrow()) => {
            return find_map_parameters(name, &map.borrow());
        }
        // The syntax of older versions: `$token 'class' { labels } FIND`.
        labels @ Value::Map(_) => {
            let class = stack.pop()?;
            let token = stack.pop()?;
            (token, class, labels)
        }
        _ => {
            return Err(fail(
                name,
                if flag {
                    "expects a list or map of parameters."
                } else {
                    "expects 3 parameters."
                },
            ));
        }
    };
    if !matches!(labels, Value::Map(_)) {
        return Err(fail(name, "label selector must be a map."));
    }
    if !matches!(class, Value::String(_)) {
        return Err(fail(name, "Class selector must be a string."));
    }
    if !matches!(token, Value::String(_)) {
        return Err(fail(name, "Token must be a string."));
    }
    Ok(FindParams {
        selector: selector_of(name, &class, &labels)?,
        skip: 0,
        count: i64::MAX,
    })
}

/// Whether a map is the parameters of `FIND`: a token and a selector.
fn is_map_parameters(map: &Params) -> bool {
    has(map, "token")
        && (has(map, "selectors")
            || has(map, "selector")
            || (has(map, "class") && has(map, "labels")))
}

/// The parameters of `FIND` given as a map. Several selectors are not kept apart: the series
/// that any of them selects are found.
fn find_map_parameters(name: &str, map: &Params) -> Result<FindParams> {
    for parameter in ["active.after", "quiet.after"] {
        if has(map, parameter) {
            return Err(unsupported(name, parameter));
        }
    }
    let skip = bounded_param(name, map, "gskip", 0)?.unwrap_or(0);
    let count = bounded_param(name, map, "gcount", 0)?.unwrap_or(i64::MAX);
    let mut selectors = selectors_param(name, map)?.unwrap_or_default();
    // A first selector stands for the others in the find, which has a single selector here.
    if selectors.len() != 1 {
        return Err(unsupported(name, "selectors"));
    }
    Ok(FindParams {
        selector: selectors.remove(0),
        skip,
        count,
    })
}

/// `FIND` and `FINDSETS`: the series that a selector selects, as empty Geo Time Series or (for
/// `FINDSETS`) the sets of their classes, of the values of their labels and of their
/// attributes.
fn find(stack: &mut Stack, name: &str, elements: bool) -> Result<()> {
    let params = find_arguments(stack, name, "3 or 4", true)?;
    let metadata = datastore(stack, name)?.find(&[params.selector])?;
    let metadata: Vec<Metadata> = metadata
        .into_iter()
        .skip(params.skip.max(0) as usize)
        .take(params.count.max(0) as usize)
        .collect();
    if !elements {
        stack.push(Value::list(
            metadata
                .into_iter()
                .map(|metadata| {
                    let mut gts = GeoTimeSerie::new();
                    gts.metadata = metadata;
                    Value::Gts(gts.shared())
                })
                .collect(),
        ));
        return Ok(());
    }
    let mut classes = BTreeSet::new();
    let mut labels: IndexMap<String, BTreeSet<String>> = IndexMap::new();
    let mut attributes: IndexMap<String, BTreeSet<String>> = IndexMap::new();
    for meta in &metadata {
        classes.extend(meta.name.clone());
        for (name, value) in &meta.labels {
            labels
                .entry(name.clone())
                .or_default()
                .insert(value.clone());
        }
        for (name, value) in &meta.attributes {
            attributes
                .entry(name.clone())
                .or_default()
                .insert(value.clone());
        }
    }
    let strings = |values: &BTreeSet<String>| {
        Value::list(
            values
                .iter()
                .map(|value| Value::from(value.as_str()))
                .collect(),
        )
    };
    let sets = |sets: &IndexMap<String, BTreeSet<String>>| {
        Value::map(
            sets.iter()
                .map(|(name, values)| (Value::from(name.as_str()), strings(values)))
                .collect(),
        )
    };
    stack.push(strings(&classes));
    stack.push(sets(&labels));
    stack.push(sets(&attributes));
    Ok(())
}

/// The configuration property of the AES key that wraps the sets `METASET` builds.
const METASETS_KEY: &str = "warp.aes.metasets";

/// The AES key of `METASET`, decoded like `KeyStore.decodeKey` decodes the property: a `hex:`
/// or `base64:` prefix. Without a key, or with another prefix, `METASET` is disabled.
fn metasets_key(stack: &Stack, name: &str) -> Result<Kek> {
    let disabled = || {
        fail(
            name,
            &format!("is disabled, as no key is set in '{METASETS_KEY}'."),
        )
    };
    let encoded = stack.config_property(METASETS_KEY).ok_or_else(disabled)?;
    let key = if let Some(hex) = encoded.strip_prefix("hex:") {
        hex_decode(hex).map_err(|cause| {
            fail_with_cause(
                name,
                &format!("the key in '{METASETS_KEY}' is not hexadecimal."),
                cause,
            )
        })?
    } else if let Some(base64) = encoded.strip_prefix("base64:") {
        base64_decode(base64, false).map_err(|cause| {
            fail_with_cause(
                name,
                &format!("the key in '{METASETS_KEY}' is not base64."),
                cause,
            )
        })?
    } else {
        return Err(disabled());
    };
    Kek::new(&key).ok_or_else(|| fail(name, "AES key length not 128/192/256 bits."))
}

/// A bound of a MetaSet: a `LONG`, or `NaN` when the bound is not set.
fn metaset_bound(value: &Value, name: &str, requirement: &str) -> Result<Option<i64>> {
    match value {
        Value::Long(bound) => Ok(Some(*bound)),
        Value::Double(bound) if bound.is_nan() => Ok(None),
        _ => Err(fail(name, requirement)),
    }
}

/// `METASET`: the series that a class selector and labels selectors select, sealed in a
/// MetaSet the way Warp 10 seals it: the Thrift structure, gzipped, wrapped with the AES key and
/// written in OrderPreservingBase64. Its parameters are a list of 7: the token, the class and the
/// labels selectors, the `notbefore`, `notafter` and `maxduration` bounds (or `NaN`) and, on top,
/// the TTL in time units. The token is recorded, not verified: this runtime reads no token.
fn metaset(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "METASET";
    let kek = metasets_key(stack, NAME)?;
    let parameters = match stack.pop()? {
        Value::List(list) => list.borrow().clone(),
        Value::Map(_) => {
            return Err(fail(
                NAME,
                "expects a list of parameters on top of the stack.",
            ));
        }
        _ => return Err(fail(NAME, "expects a list of parameters.")),
    };
    let [token, class, labels, notbefore, notafter, maxduration, ttl] =
        <[Value; 7]>::try_from(parameters).map_err(|_| fail(NAME, "expects 7 parameters."))?;
    let Value::Long(ttl) = ttl else {
        return Err(fail(
            NAME,
            "expects a metaset TTL (in time units) on top of the stack.",
        ));
    };
    let maxduration = metaset_bound(
        &maxduration,
        NAME,
        "expects a maximum duration or NaN below the expiration.",
    )?;
    let notafter = metaset_bound(
        &notafter,
        NAME,
        "expects a 'notafter' parameter below the maximum duration.",
    )?;
    let notbefore = metaset_bound(
        &notbefore,
        NAME,
        "expects a 'notbefore' parameter below 'notafter'.",
    )?;
    let selector = selector_of(NAME, &class, &labels)?;
    if !matches!(class, Value::String(_)) {
        return Err(fail(NAME, "Class selector must be a string."));
    }
    let Value::String(token) = token else {
        return Err(fail(NAME, "Token must be a string."));
    };
    let metadatas = datastore(stack, NAME)?.find(&[selector])?;
    if metadatas.is_empty() {
        return Err(fail(
            NAME,
            "couldn't find any metadata matching the given class and label selectors.",
        ));
    }
    let set = MetaSet {
        // `System.currentTimeMillis() + ttl / TIME_UNITS_PER_MS`, where Java's longs wrap around.
        expiry: (current_time() / TIME_UNITS_PER_MS).wrapping_add(ttl / TIME_UNITS_PER_MS),
        token: token.to_string(),
        metadatas,
        maxduration,
        notbefore,
        notafter,
    };
    let sealed = aes_wrap(&kek, &gzip_vec(&serialize_metaset(&set)));
    stack.push(Value::String(opb64_encode(&sealed).into()));
    Ok(())
}

/// `FINDSTATS`: the number of series that a selector selects, and of their classes, label
/// names and label values (exact, where Warp 10 estimates), as a map.
fn findstats(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "FINDSTATS";
    let params = find_arguments(stack, NAME, "3", false)?;
    let metadata = datastore(stack, NAME)?.find(&[params.selector])?;
    let mut classes = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut values = BTreeSet::new();
    let mut per_label: IndexMap<String, BTreeSet<String>> = IndexMap::new();
    for meta in &metadata {
        classes.extend(meta.name.clone());
        for (name, value) in &meta.labels {
            names.insert(name.clone());
            values.insert(format!("{name}={value}"));
            per_label
                .entry(name.clone())
                .or_default()
                .insert(value.clone());
        }
    }
    let count = |len: usize| Value::Long(len as i64);
    stack.push(Value::map(IndexMap::from([
        (Value::from("gts.estimate"), count(metadata.len())),
        (Value::from("classes.estimate"), count(classes.len())),
        (Value::from("labelnames.estimate"), count(names.len())),
        (Value::from("labelvalues.estimate"), count(values.len())),
        (
            Value::from("per.label.value.estimate"),
            Value::map(
                per_label
                    .iter()
                    .map(|(name, values)| (Value::from(name.as_str()), count(values.len())))
                    .collect(),
            ),
        ),
        (Value::from("error.rate"), Value::Double(0.0)),
    ])));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// UPDATE, META and DELETE
// -----------------------------------------------------------------------------------------

/// The type of a series that holds a value; `None` for a value a series cannot hold.
fn type_of_value(value: &Value) -> Option<Type> {
    match value {
        Value::Long(_) => Some(Type::Long),
        Value::Double(_) => Some(Type::Double),
        Value::Boolean(_) => Some(Type::Boolean),
        Value::String(_) | Value::Bytes(_) => Some(Type::String),
        _ => None,
    }
}

/// The series of an encoder, one for each type of value it holds (a series has a single type).
fn encoder_series(encoder: &GtsEncoder) -> Result<Vec<GeoTimeSerie>> {
    let mut series: Vec<GeoTimeSerie> = Vec::new();
    for (tick, location, elevation, value) in super::gts::decoded(encoder)? {
        let value = value.to_value();
        let Some(value_type) = type_of_value(&value) else {
            // A delete marker holds no value.
            continue;
        };
        let position = match series.iter().position(|gts| gts.type_of() == value_type) {
            Some(position) => position,
            None => {
                let mut gts = GeoTimeSerie::new();
                gts.metadata = encoder.metadata.clone();
                series.push(gts);
                series.len() - 1
            }
        };
        series[position].set_value(tick, location, elevation, &value, false);
    }
    Ok(series)
}

/// `UPDATE`: write Geo Time Series, encoders, or a list of them, which have a name and were
/// renamed, to the datastore.
fn update(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "UPDATE";
    const ELEMENTS: &str = "can only operate on Geo Time Series, encoders or a list thereof.";
    let Value::String(_) = stack.pop()? else {
        return Err(fail(NAME, "expects a token on top of the stack."));
    };
    let elements = match stack.pop()? {
        Value::List(list) => list.borrow().clone(),
        element @ (Value::Gts(_) | Value::Encoder(_)) => vec![element],
        _ => return Err(fail(NAME, ELEMENTS)),
    };
    let mut series = Vec::new();
    for element in &elements {
        match element {
            Value::Gts(gts) => {
                let gts = gts.borrow();
                if gts.size() > 0 {
                    if gts.name().is_none_or(str::is_empty) {
                        return Err(fail(
                            NAME,
                            "can only update Geo Time Series which have a non empty name.",
                        ));
                    }
                    if !gts.renamed {
                        return Err(fail(
                            NAME,
                            "can only update Geo Time Series which have been renamed.",
                        ));
                    }
                    series.push(gts.clone());
                }
            }
            Value::Encoder(encoder) => {
                let encoder = encoder.borrow();
                if encoder.count() > 0 {
                    if encoder.metadata.name.as_deref().is_none_or(str::is_empty) {
                        return Err(fail(
                            NAME,
                            "can only update encoders which have a non empty name.",
                        ));
                    }
                    series.extend(encoder_series(&encoder)?);
                }
            }
            _ => return Err(fail(NAME, ELEMENTS)),
        }
    }
    // There is nothing to write.
    if series.is_empty() {
        return Ok(());
    }
    datastore(stack, NAME)?.update(&series)
}

/// `META` and `METADIFF`: set the attributes of Geo Time Series, encoders, or a list of
/// them, which have a name: they replace the attributes of the series, or (`METADIFF`) are
/// changes to them.
fn meta(stack: &mut Stack, name: &str, delta: bool) -> Result<()> {
    let Value::String(_) = stack.pop()? else {
        return Err(fail(name, "expects a token."));
    };
    let metadata = |element: &Value| match element {
        Value::Gts(gts) => Some(gts.borrow().metadata.clone()),
        Value::Encoder(encoder) => Some(encoder.borrow().metadata.clone()),
        _ => None,
    };
    let metas = match stack.pop()? {
        Value::List(list) => list
            .borrow()
            .iter()
            .map(|element| {
                metadata(element).ok_or_else(|| {
                    fail(
                        name,
                        "can only operate on Geo Time Series, GTS Encoders or a list thereof.",
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?,
        element => vec![metadata(&element).ok_or_else(|| {
            fail(
                name,
                "can only operate on Geo Time Series, GTS Encoders or a list thereof",
            )
        })?],
    };
    if metas.is_empty() {
        return Ok(());
    }
    if metas
        .iter()
        .any(|meta| meta.name.as_deref().is_none_or(str::is_empty))
    {
        return Err(fail(
            name,
            "can only set attributes of Geo Time Series or GTS Encoders which have a non empty name.",
        ));
    }
    datastore(stack, name)?.update_metadata(&metas, delta)
}

/// A time of `DELETE`: `NULL`, a `LONG`, or a `STRING`.
fn delete_time(value: &Value, what: &str) -> Result<Option<i64>> {
    match value {
        Value::Null => Ok(None),
        Value::Long(_) | Value::String(_) => get_timestamp(Some(value), what, current_time()),
        _ => Err(fail(
            "DELETE",
            &format!("expects the {what} timestamp to be a Long, a String or NULL."),
        )),
    }
}

/// `DELETE`: `token selector start end expected DELETE`, delete the datapoints (all of them
/// and the series when `start` and `end` are `NULL`) of the series a selector selects, and
/// push the number of series. `expected` is the most series that may be selected; `NULL`
/// asks for a dry run, which pushes the series selected as empty Geo Time Series.
fn delete(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "DELETE";
    let expected = match stack.pop()? {
        Value::Long(expected) => Some(expected),
        Value::Null => None,
        _ => {
            return Err(fail(
                NAME,
                "expects a LONG count on top of the stack or NULL to indicate a dry run.",
            ));
        }
    };
    let end = delete_time(&stack.pop()?, "end")?;
    let start = delete_time(&stack.pop()?, "start")?;
    if start.is_some() != end.is_some() {
        return Err(fail(
            NAME,
            "expects both start and end timestamps MUST be NULL if one of them is.",
        ));
    }
    let Value::String(selector) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a Geo Time Series selector below the time parameters.",
        ));
    };
    let Value::String(_) = stack.pop()? else {
        return Err(fail(NAME, "expects a token below the selector."));
    };
    let selectors = vec![selector_of_text(&selector)?];
    let datastore = datastore(stack, NAME)?;
    let Some(expected) = expected else {
        let found = datastore.find(&selectors)?;
        stack.push(Value::list(
            found
                .into_iter()
                .map(|metadata| {
                    let mut gts = GeoTimeSerie::new();
                    gts.metadata = metadata;
                    Value::Gts(gts.shared())
                })
                .collect(),
        ));
        return Ok(());
    };
    let selected = datastore.find(&selectors)?.len() as i64;
    if expected < selected {
        return Err(fail(
            NAME,
            &format!(
                "expected at most {expected} Geo Time Series to be deleted but {} would have been deleted instead.",
                expected.saturating_add(1)
            ),
        ));
    }
    let range = start.zip(end);
    let deleted = if selected == 0 {
        0
    } else {
        datastore.delete(&DeleteRequest { selectors, range })?.len() as i64
    };
    stack.push(Value::Long(deleted));
    Ok(())
}
