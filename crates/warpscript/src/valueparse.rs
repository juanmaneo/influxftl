// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `GTSHelper.parseValue`: the value syntax of the Warp 10 input format, used by `PARSEVALUE`.
//!
//! Quoted strings (URL-decoded), `T` / `F` / `true` / `false`, `HH:lat:lon` (a GeoXP point),
//! `Q:w:x:y:z` (a quaternion), `b64:` and `hex:` byte arrays, longs, decimals (`BigDecimal`
//! for inputs of at most 15 characters made of digits, signs and dots) and doubles, and the
//! nested encoders `[ … ]` / `[! … ]` (uncompressed) of `ts/lat:lon/elev/value` tokens,
//! returned as the serialized wrapper. Offsets are counted in UTF-16 units like Java's.

use crate::encoder::{
    DEFAULT_COMP_RATIO_THRESHOLD, Decimal, EncodedValue, GtsEncoder, encoder_to_wrapper,
};
use crate::encoding::hex_decode;
use crate::error::Error;
use crate::format::{is_long, parse_long, url_decode};
use crate::functions::operators::parse_java_double;
use crate::functions::quaternions::to_quaternion;
use crate::geoxp::to_geoxp_point;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::thrift::serialize_wrapper;

/// A `java.text.ParseException`: a message, the offset of the error and its cause.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// The message (`getMessage()`).
    pub message: String,
    /// The offset of the error in the parsed text (`getErrorOffset()`), in UTF-16 units.
    pub offset: usize,
    /// The exception that caused this one.
    pub cause: Option<Error>,
}

impl ParseError {
    fn new(message: impl Into<String>, offset: usize) -> Self {
        ParseError {
            message: message.into(),
            offset,
            cause: None,
        }
    }

    fn caused(message: &str, offset: usize, cause: Error) -> Self {
        ParseError {
            message: message.to_owned(),
            offset,
            cause: Some(cause),
        }
    }

    /// The error as a script error: the message with its cause.
    pub fn into_error(self) -> Error {
        match self.cause {
            Some(cause) => Error::with_cause(self.message, cause),
            None => Error::new(self.message),
        }
    }
}

/// A failure inside `parseValue`: a `ParseException`, or any other exception, which the
/// outermost `catch` turns into `Cannot parse value.` at offset 0.
enum Failure {
    Parse(ParseError),
    Other(Error),
}

impl From<ParseError> for Failure {
    fn from(error: ParseError) -> Self {
        Failure::Parse(error)
    }
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Failure::Other(error)
    }
}

const SPACE: u16 = b' ' as u16;
const SLASH: u16 = b'/' as u16;
const COLON: u16 = b':' as u16;
const OPEN: u16 = b'[' as u16;
const CLOSE: u16 = b']' as u16;

/// `GTSHelper.parseValue` of `text`.
pub fn parse_value(text: &str) -> Result<EncodedValue, ParseError> {
    let units: Vec<u16> = text.encode_utf16().collect();
    parse_units(&units)
}

fn parse_units(s: &[u16]) -> Result<EncodedValue, ParseError> {
    parse(s).map_err(|failure| match failure {
        Failure::Parse(error) => error,
        Failure::Other(cause) => ParseError::caused("Cannot parse value.", 0, cause),
    })
}

/// The string of UTF-16 units.
fn text(s: &[u16]) -> String {
    String::from_utf16_lossy(s)
}

/// `substring(from, to)` for bounds known to be valid.
fn sub(s: &[u16], from: usize, to: usize) -> &[u16] {
    s.get(from..to).unwrap_or_default()
}

/// `charAt(i)` for an index known to be valid.
fn at(s: &[u16], i: usize) -> u16 {
    s.get(i).copied().unwrap_or_default()
}

/// `charAt(i)`, failing like Java past the end.
fn char_at(s: &[u16], i: usize) -> Result<u16, Error> {
    s.get(i).copied().ok_or_else(|| {
        Error::runtime(
            "java.lang.StringIndexOutOfBoundsException",
            format!("Index {i} out of bounds for length {}", s.len()),
        )
    })
}

fn starts_with(s: &[u16], prefix: &str) -> bool {
    s.starts_with(&prefix.encode_utf16().collect::<Vec<u16>>())
}

/// `indexOf(c, from)`.
fn index_of(s: &[u16], c: u16, from: usize) -> Option<usize> {
    s.iter()
        .skip(from)
        .position(|&u| u == c)
        .map(|pos| pos + from)
}

/// `Long.parseLong` of a token (which holds no space): an optional sign and digits.
fn java_parse_long(s: &str) -> Result<i64, Error> {
    if is_long(s) {
        parse_long(s)
    } else {
        Err(Error::new(format!("For input string: \"{s}\"")))
    }
}

fn parse(s: &[u16]) -> Result<EncodedValue, Failure> {
    let first = char_at(s, 0)?;
    let last = s.last().copied().unwrap_or_default();
    let is = |c: u16, ascii: u8| c == u16::from(ascii);
    if (is(first, b'\'') && is(last, b'\'')) || (is(first, b'"') && is(last, b'"')) {
        if s.len() == 1 {
            return Err(Error::runtime(
                "java.lang.StringIndexOutOfBoundsException",
                "Range [1, 0) out of bounds for length 1",
            )
            .into());
        }
        let decoded = url_decode(&text(sub(s, 1, s.len() - 1)))?;
        return Ok(EncodedValue::String(decoded.into()));
    }
    if (is(first, b't') || is(first, b'T'))
        && (s.len() == 1 || text(s).eq_ignore_ascii_case("true"))
    {
        return Ok(EncodedValue::Boolean(true));
    }
    if (is(first, b'f') || is(first, b'F'))
        && (s.len() == 1 || text(s).eq_ignore_ascii_case("false"))
    {
        return Ok(EncodedValue::Boolean(false));
    }
    if starts_with(s, "HH:") {
        let colon = index_of(s, COLON, 3)
            .ok_or_else(|| ParseError::new("Invalid value for lat,lon conversion to HHCode.", 0))?;
        let lat = parse_java_double(&text(sub(s, 3, colon)))
            .map_err(|cause| ParseError::caused("Cannot parse latitude.", 3, cause))?;
        let lon = parse_java_double(&text(sub(s, colon + 1, s.len())))
            .map_err(|cause| ParseError::caused("Cannot parse longitude.", colon + 1, cause))?;
        return Ok(EncodedValue::Long(to_geoxp_point(lat, lon)));
    }
    if starts_with(s, "Q:") {
        return Ok(EncodedValue::Long(parse_quaternion(s)?));
    }
    if is(first, b'[') {
        return Ok(EncodedValue::Bytes(parse_encoder(s)?.into()));
    }
    if starts_with(s, "b64:") {
        return Ok(EncodedValue::Bytes(
            decode_base64_lenient(&text(sub(s, 4, s.len()))).into(),
        ));
    }
    if starts_with(s, "hex:") {
        return Ok(EncodedValue::Bytes(
            hex_decode(&text(sub(s, 4, s.len())))?.into(),
        ));
    }
    let string = text(s);
    if is(first, b':') {
        // No custom `ValueEncoder` is registered.
        return Err(ParseError::new(format!("Unsupported value '{string}'"), 0).into());
    }
    if is_long(&string) {
        return Ok(EncodedValue::Long(parse_long(&string)?));
    }
    if s.len() <= 15
        && string
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '+' | '-'))
    {
        return Ok(EncodedValue::Decimal(parse_decimal(&string)?));
    }
    Ok(EncodedValue::Double(parse_java_double(&string)?))
}

/// `Q:w:x:y:z`: the packed quaternion of four finite components.
fn parse_quaternion(s: &[u16]) -> Result<i64, ParseError> {
    let component = |from: usize, to: usize| {
        parse_java_double(&text(sub(s, from, to)))
            .map_err(|cause| ParseError::caused("Cannot parse quaternion component.", from, cause))
    };
    let mut idx = 2;
    let mut parts = [0.0; 3];
    for part in &mut parts {
        let colon = index_of(s, COLON, idx).ok_or_else(|| {
            ParseError::new("Invalid value for Quaternion, expected Q:w:x:y:z", 0)
        })?;
        *part = component(idx, colon)?;
        idx = colon + 1;
    }
    let [qw, qx, qy] = parts;
    let qz = component(idx, s.len())?;
    if ![qw, qx, qy, qz].iter().all(|v| v.is_finite()) {
        return Err(ParseError::new(
            "Quaternion values require finite elements.",
            0,
        ));
    }
    Ok(to_quaternion(qw, qx, qy, qz))
}

/// `new BigDecimal(s)` for a string of digits, signs and dots, with the JDK messages.
fn parse_decimal(s: &str) -> Result<Decimal, Error> {
    let (negative, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let mut unscaled = 0i64;
    let mut scale = 0;
    let mut dot = false;
    let mut digits = false;
    for c in body.chars() {
        if let Some(digit) = c.to_digit(10) {
            unscaled = unscaled.wrapping_mul(10).wrapping_add(i64::from(digit));
            digits = true;
            scale += i32::from(dot);
        } else if c == '.' && !dot {
            dot = true;
        } else if c == '.' {
            return Err(Error::new(
                "Character array contains more than one decimal point.",
            ));
        } else {
            return Err(Error::new(format!(
                "Character {c} is neither a decimal digit number, decimal point, nor \"e\" notation exponential mark."
            )));
        }
    }
    if !digits {
        return Err(Error::new("No digits found."));
    }
    Ok(Decimal {
        unscaled: if negative { -unscaled } else { unscaled },
        scale,
    })
}

/// Commons Codec's `Base64.decodeBase64`: characters outside both alphabets are skipped,
/// decoding stops at the first `=`, and incomplete trailing bits are dropped.
fn decode_base64_lenient(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut n = 0;
    for c in text.chars() {
        let value = match c {
            '=' => break,
            'A'..='Z' => u32::from(c) - u32::from('A'),
            'a'..='z' => u32::from(c) - u32::from('a') + 26,
            '0'..='9' => u32::from(c) - u32::from('0') + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            _ => continue,
        };
        acc = acc << 6 | value;
        n += 1;
        if n == 4 {
            out.extend([(acc >> 16) as u8, (acc >> 8) as u8, acc as u8]);
            acc = 0;
            n = 0;
        }
    }
    match n {
        2 => out.push((acc >> 4) as u8),
        3 => out.extend([(acc >> 10) as u8, (acc >> 2) as u8]),
        _ => {}
    }
    out
}

/// The end of the value starting at `from`: past its matching `]` (or at `end_value` when
/// unbalanced) when it starts with `[`, else `end`.
fn value_end(s: &[u16], from: usize, end: usize, end_value: usize) -> usize {
    if at(s, from) != OPEN {
        return end;
    }
    let mut closing = from + 1;
    let mut opening = 1;
    while closing < end_value && opening > 0 {
        match at(s, closing) {
            CLOSE => opening -= 1,
            OPEN => opening += 1,
            _ => {}
        }
        closing += 1;
    }
    closing
}

/// The value of `s[from..to]`, its errors shifted to offsets in `s`.
fn nested(s: &[u16], from: usize, to: usize) -> Result<EncodedValue, ParseError> {
    parse_units(sub(s, from, to)).map_err(|mut error| {
        error.offset += from;
        error
    })
}

/// `[ token … ]` (`[! … ]` for an uncompressed wrapper), each token being `value`,
/// `ts/value`, `ts/lat:lon/value`, `ts/hhcode/value` or `ts/lat:lon/elev/value` (fields may
/// be empty): the serialized wrapper of the encoder of the tokens, without count.
fn parse_encoder(s: &[u16]) -> Result<Vec<u8>, Failure> {
    let mut encoder = GtsEncoder::new(0);
    let mut start = 1;
    let mut compress = true;
    if char_at(s, 1)? == u16::from(b'!') {
        compress = false;
        start = 2;
    }
    // The closing bracket, ignoring trailing spaces.
    let mut end_value = s.len() - 1;
    loop {
        if end_value < start {
            return Err(ParseError::new("Missing closing bracket.", 0).into());
        }
        match at(s, end_value) {
            CLOSE => break,
            SPACE => end_value -= 1,
            _ => return Err(ParseError::new("Missing closing bracket.", 0).into()),
        }
    }
    let find = |from: usize, end: usize, c: u16| {
        let mut idx = from;
        while idx < end && at(s, idx) != c {
            idx += 1;
        }
        idx
    };
    while start < end_value {
        while start < end_value && at(s, start) == SPACE {
            start += 1;
        }
        if start >= end_value {
            break;
        }
        let mut end = start + 1;
        while end < end_value && at(s, end) != SPACE {
            end += 1;
        }
        let mut location = NO_LOCATION;
        let mut elevation = NO_ELEVATION;
        let mut slash = find(start + 1, end, SLASH);
        if slash == end {
            end = value_end(s, start, end, end_value);
            encoder.add_value(0, location, elevation, &nested(s, start, end)?);
            start = end;
            continue;
        }
        let ts = java_parse_long(&text(sub(s, start, slash)))?;
        start = slash + 1;
        slash = find(start, end, SLASH);
        if slash == end {
            end = value_end(s, start, end, end_value);
            encoder.add_value(ts, location, elevation, &nested(s, start, end)?);
            start = end;
            continue;
        }
        let colon = find(start, slash, COLON);
        if colon == slash {
            if colon > start {
                location = java_parse_long(&text(sub(s, start, colon)))?;
            }
        } else {
            let lat = parse_java_double(&text(sub(s, start, colon)))?;
            let lon = parse_java_double(&text(sub(s, colon + 1, slash)))?;
            location = to_geoxp_point(lat, lon);
        }
        start = slash + 1;
        slash = find(start, end, SLASH);
        if slash == end {
            end = value_end(s, start, end, end_value);
            encoder.add_value(ts, location, elevation, &nested(s, start, end)?);
            start = end;
            continue;
        }
        if slash > start {
            elevation = java_parse_long(&text(sub(s, start, slash)))?;
        }
        end = value_end(s, slash + 1, end, end_value);
        encoder.add_value(ts, location, elevation, &nested(s, slash + 1, end)?);
        start = end + 1;
    }
    let wrapper = encoder_to_wrapper(&encoder, compress, DEFAULT_COMP_RATIO_THRESHOLD, false);
    Ok(serialize_wrapper(&wrapper))
}
