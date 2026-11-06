// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Encoders and wrapped series: `NEWENCODER`, the `WRAP` family (`WRAP`, `WRAPRAW`,
//! `WRAPFAST`, `WRAPOPT`, `WRAPRAWOPT`, `WRAPMV`, `WRAPMV!`) and `UNWRAP`, `UNWRAPEMPTY`,
//! `UNWRAPSIZE`, `UNWRAPENCODER`, the conversions `->ENCODER`, `ENCODER->`, `ASENCODERS`,
//! `->ENCODERS`, `->GTS`, `OPTIMIZE` and `PARSEVALUE`, on the `GTSWrapper` Thrift structure of `thrift.rs`
//! and the encoders of `encoder.rs`.

use std::cell::RefCell;
use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::gts::element_or_list;
use indexmap::IndexMap;

use crate::encoder::{
    DEFAULT_COMP_RATIO_THRESHOLD, EncodedValue, GtsDecoder, GtsEncoder, encoder_to_wrapper,
    fullsort_encoder, gts_to_wrapper, wrapper_to_decoder, wrapper_to_encoder, wrapper_to_gts,
};
use crate::encoding::{opb64_decode, opb64_encode};
use crate::error::{Error, Result};
use crate::geoxp;
use crate::gts::{GeoTimeSerie, Metadata, NO_ELEVATION, NO_LOCATION, Values};
use crate::selector::MetadataSelectorMatcher;
use crate::stack::Stack;
use crate::thrift::{GtsWrapper, deserialize_wrapper, serialize_wrapper};
use crate::value::Value;
use crate::valueparse::parse_value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("NEWENCODER", Entry::Stack(newencoder)),
    ("WRAP", Entry::Builder(wrap)),
    ("WRAPRAW", Entry::Builder(wrap)),
    ("WRAPFAST", Entry::Builder(wrap)),
    ("WRAPOPT", Entry::Builder(wrap)),
    ("WRAPRAWOPT", Entry::Builder(wrap)),
    ("WRAPMV", Entry::Builder(wrap)),
    ("WRAPMV!", Entry::Builder(wrap)),
    ("UNWRAP", Entry::Builder(unwrap)),
    ("UNWRAPEMPTY", Entry::Builder(unwrap)),
    ("UNWRAPSIZE", Entry::Builder(unwrap)),
    ("UNWRAPENCODER", Entry::Builder(unwrapencoder)),
    ("->ENCODER", Entry::Builder(toencoder)),
    ("ENCODER->", Entry::Builder(encoderto)),
    ("ASENCODERS", Entry::Builder(asencoders)),
    ("->ENCODERS", Entry::Builder(toencoders)),
    ("->GTS", Entry::Builder(togts)),
    ("OPTIMIZE", Entry::Builder(optimize)),
    ("PARSEVALUE", Entry::Stack(parsevalue)),
    ("GOLDWRAP", Entry::Stack(goldwrap)),
];

/// The fully sorted encoder of the wrapper `bytes` of `GOLDWRAP`.
fn gold_encoder(bytes: &[u8], reversed: bool) -> Result<GtsEncoder> {
    let wrapper = deserialize_wrapper(bytes).map_err(|err| {
        wrap_error(
            "GOLDWRAP",
            "encountered an error while manipulating GTS Wrapper.",
            err,
        )
    })?;
    fullsort_encoder(&wrapper_to_encoder(&wrapper)?, reversed, 0)
}

/// `GOLDWRAP` of one element (lists are handled recursively): the fully sorted datapoints,
/// with the class name and the sorted labels and attributes only, as a compressed wrapper.
/// A series is fully sorted in place, as in Warp 10.
fn goldwrap_element(element: &Value, reversed: bool) -> Result<Value> {
    const NAME: &str = "GOLDWRAP";
    let sorted = match element {
        Value::Gts(gts) => {
            let mut gts = gts.borrow_mut();
            gts.fullsort(reversed);
            let mut encoder = GtsEncoder::new(0);
            encoder.metadata = gts.metadata.clone();
            encoder.encode_optimized(&gts);
            encoder
        }
        Value::Encoder(encoder) => fullsort_encoder(&encoder.borrow(), reversed, 0)?,
        Value::String(s) => {
            let ascii: String = s
                .chars()
                .map(|c| if c.is_ascii() { c } else { '?' })
                .collect();
            gold_encoder(&opb64_decode(&ascii)?, reversed)?
        }
        Value::Bytes(bytes) => gold_encoder(bytes, reversed)?,
        Value::List(list) => {
            let results = list
                .borrow()
                .iter()
                .map(|e| goldwrap_element(e, reversed))
                .collect::<Result<Vec<_>>>()?;
            return Ok(Value::list(results));
        }
        _ => {
            return Err(fail(
                NAME,
                "can only handle GTS, ENCODERs and list thereof.",
            ));
        }
    };
    let sorted_map = |map: &IndexMap<String, String>| {
        let mut entries: Vec<(String, String)> =
            map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
        entries.into_iter().collect()
    };
    let metadata = Metadata {
        name: sorted.metadata.name.clone(),
        labels: sorted_map(&sorted.metadata.labels),
        attributes: sorted_map(&sorted.metadata.attributes),
        ..Metadata::default()
    };
    let mut gold = sorted;
    gold.metadata = metadata;
    let wrapper = encoder_to_wrapper(&gold, true, 1.0, true);
    Ok(Value::bytes(serialize_wrapper(&wrapper)))
}

/// `GOLDWRAP`: a canonical (`[ reversed ]`) wrapper of series, encoders or wrappers.
fn goldwrap(stack: &mut Stack) -> Result<()> {
    let reversed = match stack.peek()? {
        Value::Boolean(b) => {
            let b = *b;
            stack.pop().ok();
            b
        }
        _ => false,
    };
    let top = stack.pop()?;
    let result = goldwrap_element(&top, reversed)?;
    stack.push(result);
    Ok(())
}

/// `NEWENCODER`: an empty encoder named `""`.
fn newencoder(stack: &mut Stack) -> Result<()> {
    let mut encoder = GtsEncoder::new(0);
    encoder.metadata.name = Some(String::new());
    stack.push(Value::Encoder(Rc::new(RefCell::new(encoder))));
    Ok(())
}

/// `WRAP` and its variants, `(opt, compress, raw, mv)` from the registration of each name.
fn wrap(stack: &mut Stack, name: &'static str) -> Result<()> {
    let (opt, compress, raw, mv) = match name {
        "WRAPRAW" => (false, true, true, false),
        "WRAPFAST" => (false, false, true, false),
        "WRAPOPT" => (true, true, false, false),
        "WRAPRAWOPT" => (true, true, true, false),
        "WRAPMV" => (true, true, true, true),
        "WRAPMV!" => (true, false, true, true),
        _ => (false, true, false, false),
    };
    let compratio = if opt {
        1.0
    } else {
        DEFAULT_COMP_RATIO_THRESHOLD
    };
    element_or_list(stack, |element| {
        let mut wrapper = match element {
            Value::Gts(gts) => gts_to_wrapper(&gts.borrow(), compress, compratio, opt, true),
            Value::Encoder(encoder) => {
                encoder_to_wrapper(&encoder.borrow(), compress, compratio, true)
            }
            _ => {
                return Err(fail(
                    name,
                    "expects a Geo Time Series, a GTSEncoder or a list thereof.",
                ));
            }
        };
        if mv {
            wrapper.metadata = None;
            wrapper.count = None;
        }
        let bytes = serialize_wrapper(&wrapper);
        Ok(if raw {
            Value::bytes(bytes)
        } else {
            Value::string(opb64_encode(&bytes))
        })
    })
}

/// A wrapped series as given to the `UNWRAP` functions.
enum Wrapped {
    /// OPB64 text.
    Text(Rc<str>),
    /// Raw bytes.
    Raw(Rc<[u8]>),
}

impl Wrapped {
    fn of(value: &Value) -> Option<Wrapped> {
        match value {
            Value::String(s) => Some(Wrapped::Text(Rc::clone(s))),
            Value::Bytes(b) => Some(Wrapped::Raw(Rc::clone(b))),
            _ => None,
        }
    }

    /// The deserialized wrapper. A string is decoded from OPB64 (its non-ASCII characters
    /// are `?`, as `getBytes(US_ASCII)` makes them); Thrift errors are the cause of
    /// `<name> <what>`.
    fn read(&self, name: &str, what: &str) -> Result<GtsWrapper> {
        let bytes = match self {
            Wrapped::Text(s) => {
                let ascii: String = s
                    .chars()
                    .map(|c| if c.is_ascii() { c } else { '?' })
                    .collect();
                opb64_decode(&ascii)?
            }
            Wrapped::Raw(b) => b.to_vec(),
        };
        deserialize_wrapper(&bytes).map_err(|err| wrap_error(name, what, err))
    }
}

/// Java's `catch (TException | IOException e)`: the checked exceptions become the cause of
/// `<name> <what>`, the runtime ones go through.
fn wrap_error(name: &str, what: &str, err: Error) -> Error {
    if err.is_runtime() {
        err
    } else {
        Error::with_cause(format!("{name} {what}"), err)
    }
}

/// `UNWRAP`, `UNWRAPEMPTY` (the series without their datapoints) and `UNWRAPSIZE` (the
/// number of datapoints): a string or byte array, or a list thereof.
fn unwrap(stack: &mut Stack, name: &'static str) -> Result<()> {
    let invalid = || {
        fail(
            name,
            "operates on a string or byte array or a list thereof.",
        )
    };
    let top = stack.pop()?;
    let elements = match &top {
        Value::List(list) => list.borrow().clone(),
        other => vec![other.clone()],
    };
    let inputs = elements
        .iter()
        .map(Wrapped::of)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid)?;
    let mut outputs = Vec::with_capacity(inputs.len());
    for input in &inputs {
        let wrapper = input.read(name, "failed to unwrap GTS.")?;
        outputs.push(if name == "UNWRAPSIZE" {
            Value::Long(wrapper.count())
        } else {
            let gts = wrapper_to_gts(&wrapper, name == "UNWRAPEMPTY")?;
            Value::Gts(gts.shared())
        });
    }
    stack.push(match top {
        Value::List(_) => Value::list(outputs),
        _ => outputs.pop().unwrap_or(Value::Null),
    });
    Ok(())
}

/// `UNWRAPENCODER`: the encoder of a wrapped series.
fn unwrapencoder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let input = Wrapped::of(&stack.pop()?)
        .ok_or_else(|| fail(name, "operates on a string or byte array."))?;
    let what = "failed to unwrap encoder.";
    let wrapper = input.read(name, what)?;
    let encoder = wrapper_to_encoder(&wrapper).map_err(|err| wrap_error(name, what, err))?;
    stack.push(Value::Encoder(Rc::new(RefCell::new(encoder))));
    Ok(())
}

/// The shared encoder value of `encoder`.
fn encoder_value(encoder: GtsEncoder) -> Value {
    Value::Encoder(Rc::new(RefCell::new(encoder)))
}

/// `->ENCODER`: an encoder holding the datapoints of a list of wrapped series, Geo Time Series
/// (whose metadata is lost) and `[ tick (lat lon) (elevation) value ]` lists.
fn toencoder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let Value::List(list) = stack.pop()? else {
        return Err(fail(name, "expects a list on top of the stack."));
    };
    let invalid = || fail(name, "encountered an invalid element.");
    let mut encoder = GtsEncoder::new(0);
    for element in list.borrow().iter() {
        if let Some(wrapped) = Wrapped::of(element) {
            let wrapper = wrapped.read(name, "failed to unwrap encoder.")?;
            let mut decoder = wrapper_to_decoder(&wrapper)?;
            decoder.next()?;
            let what = "cannot get encoder from wrapper.";
            let other = decoder
                .encoder()
                .map_err(|err| wrap_error(name, what, err))?;
            encoder.merge(&other)?;
            continue;
        }
        let elements = match element {
            Value::Gts(gts) => {
                encoder.encode(&gts.borrow());
                continue;
            }
            Value::List(elements) => elements.borrow().clone(),
            _ => return Err(invalid()),
        };
        let Some(Value::Long(tick)) = elements.first() else {
            return Err(if (2..=5).contains(&elements.len()) {
                fail(name, "encountered an invalid timestamp.")
            } else {
                invalid()
            });
        };
        let number = |idx: usize| {
            elements
                .get(idx)
                .and_then(Value::as_f64)
                .ok_or_else(invalid)
        };
        let elevation = |idx: usize| -> Result<i64> {
            let elev = elements
                .get(idx)
                .filter(|v| v.is_number())
                .ok_or_else(invalid)?;
            Ok(if elev.as_f64().is_some_and(f64::is_nan) {
                NO_ELEVATION
            } else {
                elev.as_i64().unwrap_or(NO_ELEVATION)
            })
        };
        let location = || -> Result<i64> {
            let (lat, lon) = (number(1)?, number(2)?);
            Ok(if lat.is_nan() || lon.is_nan() {
                NO_LOCATION
            } else {
                geoxp::to_geoxp_point(lat, lon)
            })
        };
        let (location, elevation) = match elements.len() {
            2 => (NO_LOCATION, NO_ELEVATION),
            3 => (NO_LOCATION, elevation(1)?),
            4 => (location()?, NO_ELEVATION),
            5 => (location()?, elevation(3)?),
            _ => return Err(invalid()),
        };
        let value = elements.last().unwrap_or(&Value::Null);
        encoder.add_value(
            *tick,
            location,
            elevation,
            &EncodedValue::from_value(value)?,
        );
    }
    stack.push(encoder_value(encoder));
    Ok(())
}

/// The decoder of an encoder or of a wrapped series (`ENCODER->`, `->ENCODERS`, `->GTS`),
/// `None` for any other value.
fn decoder_of(value: &Value, name: &str) -> Result<Option<GtsDecoder>> {
    if let Value::Encoder(encoder) = value {
        return Ok(Some(encoder.borrow().decoder()));
    }
    let Some(wrapped) = Wrapped::of(value) else {
        return Ok(None);
    };
    let wrapper = wrapped.read(name, "failed to unwrap encoder.")?;
    wrapper_to_decoder(&wrapper).map(Some)
}

/// The decoder of the value on top of the stack for `ENCODER->` and `->ENCODERS`.
fn pop_decoder(stack: &mut Stack, name: &str) -> Result<GtsDecoder> {
    decoder_of(&stack.pop()?, name)?
        .ok_or_else(|| fail(name, "operates on a string, byte array or encoder."))
}

/// A map of strings as a WarpScript map.
fn string_map(map: &IndexMap<String, String>) -> Value {
    Value::map(
        map.iter()
            .map(|(k, v)| (Value::from(k.as_str()), Value::from(v.as_str())))
            .collect(),
    )
}

/// `ENCODER->`: the name, labels, attributes and `[ tick lat lon elevation value ]` datapoints
/// of an encoder or wrapped series, `NaN` standing for a missing location or elevation.
fn encoderto(stack: &mut Stack, name: &'static str) -> Result<()> {
    let mut decoder = pop_decoder(stack, name)?;
    let mut elements = Vec::new();
    while decoder.next()? {
        let (lat, lon) = match decoder.location() {
            NO_LOCATION => (f64::NAN, f64::NAN),
            location => geoxp::from_geoxp_point(location),
        };
        let elevation = match decoder.elevation() {
            NO_ELEVATION => Value::Double(f64::NAN),
            elevation => Value::Long(elevation),
        };
        elements.push(Value::list(vec![
            Value::Long(decoder.timestamp()),
            Value::Double(lat),
            Value::Double(lon),
            elevation,
            decoder.binary_value().to_value(),
        ]));
    }
    let metadata = &decoder.metadata;
    stack.push(metadata.name.as_deref().map_or(Value::Null, Value::from));
    stack.push(string_map(&metadata.labels));
    stack.push(string_map(&metadata.attributes));
    stack.push(Value::list(elements));
    Ok(())
}

/// `ASENCODERS`: Geo Time Series (their doubles optimized) and wrapped series as encoders,
/// encoders left as they are.
fn asencoders(stack: &mut Stack, name: &'static str) -> Result<()> {
    element_or_list(stack, |element| {
        if let Some(wrapped) = Wrapped::of(element) {
            let wrapper = wrapped.read(name, "failed to unwrap encoder.")?;
            let what = "failed to convert wrapper to encoder.";
            let encoder =
                wrapper_to_encoder(&wrapper).map_err(|err| wrap_error(name, what, err))?;
            return Ok(encoder_value(encoder));
        }
        match element {
            Value::Gts(gts) => {
                let gts = gts.borrow();
                let mut encoder = GtsEncoder::new(0);
                encoder.encode_optimized(&gts);
                encoder.metadata = gts.metadata.clone();
                Ok(encoder_value(encoder))
            }
            Value::Encoder(_) => Ok(element.clone()),
            _ => Err(fail(
                name,
                "expects a Geo Time Series or a list thereof on top on the stack.",
            )),
        }
    })
}

/// The `->ENCODERS` / `->GTS` type of a value (`DOUBLE` for a delete marker).
fn type_name(value: &EncodedValue) -> &'static str {
    match value {
        EncodedValue::Long(_) => "LONG",
        EncodedValue::Boolean(_) => "BOOLEAN",
        EncodedValue::String(_) => "STRING",
        EncodedValue::Bytes(_) => "BINARY",
        EncodedValue::Null | EncodedValue::Double(_) | EncodedValue::Decimal(_) => "DOUBLE",
    }
}

/// `->ENCODERS`: the datapoints of an encoder or wrapped series split by value type, in a map
/// from `LONG`, `DOUBLE`, `BOOLEAN`, `STRING` and `BINARY` to encoders.
fn toencoders(stack: &mut Stack, name: &'static str) -> Result<()> {
    let mut decoder = pop_decoder(stack, name)?;
    let mut encoders: IndexMap<&'static str, GtsEncoder> = IndexMap::new();
    let failed = |err| {
        Error::with_cause(
            format!("{name} encountered an exception during conversion."),
            err,
        )
    };
    while decoder.next().map_err(failed)? {
        let value = decoder.binary_value();
        let encoder = encoders.entry(type_name(&value)).or_insert_with(|| {
            let mut encoder = GtsEncoder::new(0);
            encoder.metadata = decoder.metadata.clone();
            encoder
        });
        encoder.add_value(
            decoder.timestamp(),
            decoder.location(),
            decoder.elevation(),
            &value,
        );
    }
    stack.push(Value::map(
        encoders
            .into_iter()
            .map(|(kind, encoder)| (Value::from(kind), encoder_value(encoder)))
            .collect(),
    ));
    Ok(())
}

/// The value types of `->GTS`, in the order of its output maps.
const TYPES: [&str; 5] = ["LONG", "DOUBLE", "BOOLEAN", "STRING", "BINARY"];

/// The type map of `->GTS`: the selectors of each type and the name of the type label.
struct TypeMap {
    selectors: Vec<(&'static str, Vec<MetadataSelectorMatcher>)>,
    label: Option<String>,
}

impl TypeMap {
    fn parse(map: &IndexMap<Value, Value>, name: &str) -> Result<TypeMap> {
        let selectors_expected = || {
            fail(
                name,
                "type MAP input must contains selector or list of thereof for each type.",
            )
        };
        let mut selectors = Vec::new();
        let mut label = Some(".type".to_string());
        for (key, value) in map {
            let kind = key.as_str().unwrap_or_default();
            if let Some(kind) = TYPES.iter().find(|t| **t == kind) {
                let sources = match value {
                    Value::String(s) => vec![Value::String(Rc::clone(s))],
                    Value::List(list) => list.borrow().clone(),
                    _ => return Err(selectors_expected()),
                };
                let matchers = sources
                    .iter()
                    .map(|source| {
                        let source = source.as_str().ok_or_else(selectors_expected)?;
                        MetadataSelectorMatcher::new(source)
                    })
                    .collect::<Result<Vec<_>>>()?;
                selectors.push((*kind, matchers));
            } else if kind == "label.type" {
                label = match value {
                    Value::Null => None,
                    Value::String(s) => Some(s.to_string()),
                    _ => {
                        return Err(fail(
                            name,
                            "extra 'label' input in the MAP must be either null or a string.",
                        ));
                    }
                };
            } else {
                return Err(fail(
                    name,
                    "type MAP input must contains valid types as key (LONG, DOUBLE, BOOLEAN, \
                     STRING or BINARY) or 'label.type' to override '.type' label.",
                ));
            }
        }
        Ok(TypeMap { selectors, label })
    }

    /// The type of the first type whose selectors match the series.
    fn enforced(&self, decoder: &GtsDecoder) -> Result<Option<&'static str>> {
        for (kind, matchers) in &self.selectors {
            for matcher in matchers {
                if matcher.matches(&decoder.metadata)? {
                    return Ok(Some(kind));
                }
            }
        }
        Ok(None)
    }

    /// The series of a decoder, typed by the selectors or by its first value, with the type
    /// label.
    fn series(&self, mut decoder: GtsDecoder, name: &str) -> Result<GeoTimeSerie> {
        let mut gts = GeoTimeSerie::new();
        gts.metadata = decoder.metadata.clone();
        let mut enforced = self.enforced(&decoder)?;
        gts.values = match enforced {
            Some("LONG") => Values::Long(Vec::new()),
            Some("DOUBLE") => Values::Double(Vec::new()),
            Some("BOOLEAN") => Values::Boolean(Vec::new()),
            Some(_) => Values::String(Vec::new()),
            None => Values::Undefined,
        };
        let mut first = true;
        while decoder.next()? {
            let value = decoder.binary_value();
            if first && enforced.is_none() && value != EncodedValue::Null {
                enforced = Some(type_name(&value));
            }
            first = false;
            decoder.set_value_on(&mut gts);
        }
        if let Some(label) = &self.label {
            if gts.metadata.labels.contains_key(label) {
                return Err(fail(name, &format!("the input already has label {label}")));
            }
            let kind = enforced.unwrap_or("UNDEFINED");
            gts.metadata.labels.insert(label.clone(), kind.to_string());
        }
        Ok(gts)
    }
}

/// The series of a decoder split by value type, in [`TYPES`] order (delete markers dropped).
fn split_by_type(mut decoder: GtsDecoder) -> Result<Vec<(&'static str, GeoTimeSerie)>> {
    let mut series: [Option<GeoTimeSerie>; 5] = Default::default();
    while decoder.next()? {
        let value = decoder.binary_value();
        if value == EncodedValue::Null {
            continue;
        }
        let kind = type_name(&value);
        let idx = TYPES.iter().position(|t| *t == kind).unwrap_or_default();
        for gts in series.iter_mut().skip(idx).take(1) {
            let gts = gts.get_or_insert_with(|| {
                let mut gts = GeoTimeSerie::new();
                gts.metadata = decoder.metadata.clone();
                gts
            });
            decoder.set_value_on(gts);
        }
    }
    Ok(TYPES
        .into_iter()
        .zip(series)
        .filter_map(|(kind, gts)| gts.map(|gts| (kind, gts)))
        .collect())
}

/// `->GTS`: the Geo Time Series of encoders or wrapped series. Without a type map, a map from
/// the value types to the series (or to lists thereof for a list input); with a map of
/// selectors by type, one series per input converted to the selected type (or to the type of
/// its first value), with a type label.
pub(super) fn togts(stack: &mut Stack, name: &'static str) -> Result<()> {
    let mut top = stack.pop()?;
    let type_map = match &top {
        Value::Map(map) => {
            let type_map = TypeMap::parse(&map.borrow(), name)?;
            top = stack.pop()?;
            Some(type_map)
        }
        _ => None,
    };
    let invalid = || {
        fail(
            name,
            "operates on a string, a byte array, an encoder, or list thereof.",
        )
    };
    let (inputs, list_input) = match &top {
        Value::List(list) => (list.borrow().clone(), true),
        other => (vec![other.clone()], false),
    };
    let decoders = inputs
        .iter()
        .map(|input| decoder_of(input, name)?.ok_or_else(invalid))
        .collect::<Result<Vec<_>>>()?;
    let gts_value = |gts: GeoTimeSerie| Value::Gts(gts.shared());
    let result = match type_map {
        Some(type_map) => {
            let series = decoders
                .into_iter()
                .map(|decoder| type_map.series(decoder, name).map(gts_value))
                .collect::<Result<Vec<_>>>()?;
            if list_input {
                Value::list(series)
            } else {
                series.into_iter().next().unwrap_or(Value::Null)
            }
        }
        None => {
            let mut result: IndexMap<Value, Vec<Value>> = IndexMap::new();
            for decoder in decoders {
                for (kind, gts) in split_by_type(decoder)? {
                    result
                        .entry(Value::from(kind))
                        .or_default()
                        .push(gts_value(gts));
                }
            }
            Value::map(
                result
                    .into_iter()
                    .map(|(kind, series)| {
                        if list_input {
                            (kind, Value::list(series))
                        } else {
                            (kind, series.into_iter().next().unwrap_or(Value::Null))
                        }
                    })
                    .collect(),
            )
        }
    };
    stack.push(result);
    Ok(())
}

/// `OPTIMIZE`: shrink the arrays of Geo Time Series and encoders, which have no spare capacity
/// here; the ratio and the elements are only checked.
fn optimize(stack: &mut Stack, name: &'static str) -> Result<()> {
    if !stack.pop()?.is_number() {
        return Err(fail(name, "expects a ratio on top of the stack."));
    }
    element_or_list(stack, |element| match element {
        Value::Gts(_) | Value::Encoder(_) => Ok(element.clone()),
        _ => Err(fail(
            name,
            " operates on a Geo Time Series or encoder instance or a list thereof.",
        )),
    })
}

/// `PARSEVALUE`: the value of a string in the syntax of the input format
/// (`GTSHelper.parseValue`, see `valueparse.rs`), the string being trimmed first. Decimals
/// are pushed as doubles and nested encoders as the bytes of their wrapper.
fn parsevalue(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PARSEVALUE";
    let Value::String(text) = stack.pop()? else {
        return Err(fail(NAME, "operates on a STRING."));
    };
    let text = text.trim_matches(|c: char| c <= ' ');
    let value = parse_value(text).map_err(|error| {
        let message = format!(
            "{NAME} encountered a parse error at index {} for '{text}'.",
            error.offset
        );
        Error::with_cause(message, error.into_error())
    })?;
    // `GTSHelper.parseValue` returns a `BigDecimal` for a decimal.
    stack.push(match value {
        EncodedValue::Decimal(d) => Value::BigDecimal(Rc::new(crate::bigdecimal::BigDecimal::new(
            d.unscaled.into(),
            d.scale,
        ))),
        other => other.to_value(),
    });
    Ok(())
}
