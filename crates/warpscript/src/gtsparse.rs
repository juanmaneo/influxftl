// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The GTS input format (`GTSHelper.parse`): `TS/LAT:LON/ELEV NAME{LABELS}{ATTRIBUTES} VALUE`,
//! with `=TS/LAT:LON/ELEV VALUE` continuation lines. Offsets are in UTF-16 units, like Java's.

use indexmap::IndexMap;

use crate::encoder::{EncodedValue, GtsEncoder};
use crate::error::Error;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::selector::{SelectorFailure, parse_selector_tokens};
use crate::valueparse::{ParseError, parse_value};

/// One parsed line: the datapoint and, unless it continues the previous series, the class
/// name and labels of its series.
pub struct ParsedLine {
    /// The class name and labels, `None` for a line reusing those of the previous series.
    pub metadata: Option<(String, IndexMap<String, String>)>,
    /// The attributes, when the line has some.
    pub attributes: Option<IndexMap<String, String>>,
    /// The tick, location, elevation and value.
    pub point: (i64, i64, i64, EncodedValue),
}

/// A parse failure: a `ParseException`, or a runtime exception that escapes the parser.
pub enum LineError {
    /// `ParseException` (message and offset).
    Parse(ParseError),
    /// Another exception.
    Other(Error),
}

fn parse_error(message: impl Into<String>, offset: usize) -> LineError {
    LineError::Parse(ParseError {
        message: message.into(),
        offset,
        cause: None,
    })
}

/// `String.indexOf(c, from)` on UTF-16 units.
fn index_of(s: &[u16], c: char, from: usize) -> Option<usize> {
    let c = c as u16;
    s.iter().skip(from).position(|&u| u == c).map(|p| p + from)
}

fn text(s: &[u16]) -> String {
    String::from_utf16_lossy(s)
}

/// `GTSHelper.parseLabels`: `name=value` pairs (a `~` selector is an error at offset 0).
fn parse_labels(s: &str) -> Result<IndexMap<String, String>, LineError> {
    let selectors = parse_selector_tokens(s).map_err(|failure| match failure {
        // `ParseException(token, 0)`, wrapped by the selector parser.
        SelectorFailure::Token(token) => parse_error(&token, 0),
        SelectorFailure::Decode(err) => LineError::Other(err),
    })?;
    let mut labels = IndexMap::new();
    for (name, value) in selectors {
        let Some(value) = value.strip_prefix('=') else {
            return Err(parse_error(value, 0));
        };
        labels.insert(name, value.to_owned());
    }
    Ok(labels)
}

/// [`parse_labels`] of the labels or attributes at `offset`, wrapped as `message`.
fn labels_at(s: &str, offset: usize, message: &str) -> Result<IndexMap<String, String>, LineError> {
    parse_labels(s).map_err(|err| match err {
        LineError::Parse(pe) => LineError::Parse(ParseError {
            message: message.to_owned(),
            offset: pe.offset + offset,
            cause: Some(Error::parse(pe.message, pe.offset)),
        }),
        other @ LineError::Other(_) => other,
    })
}

/// `GTSHelper.parse` of one (trimmed, non empty) line, `last` being the series of the
/// previous line and `now` the current time.
pub fn parse_line(
    last: Option<&GtsEncoder>,
    line: &str,
    now: i64,
) -> Result<ParsedLine, LineError> {
    let s: Vec<u16> = line.encode_utf16().collect();
    let continuation = s.first() == Some(&('=' as u16));
    if continuation && last.is_none() {
        return Err(parse_error("Invalid continuation.", 0));
    }
    let tsoffset = usize::from(continuation);
    let Some(mut idx) = index_of(&s, '/', tsoffset) else {
        return Err(parse_error("Missing timestamp separator.", tsoffset));
    };
    let invalid_timestamp = || parse_error("Invalid timestamp.", tsoffset);
    let timestamp = if tsoffset == idx {
        now
    } else if s[tsoffset] == 'T' as u16 {
        let delta: i64 = text(&s[tsoffset + 1..idx])
            .parse()
            .map_err(|_| invalid_timestamp())?;
        now.wrapping_add(delta)
    } else {
        text(&s[tsoffset..idx])
            .parse()
            .map_err(|_| invalid_timestamp())?
    };
    idx += 1;
    let Some(idx2) = index_of(&s, '/', idx) else {
        return Err(parse_error("Missing location/elevation separator.", idx));
    };
    let mut location = NO_LOCATION;
    if idx == idx2 {
        idx = idx2 + 1;
    } else {
        let latlon = text(&s[idx..idx2]);
        let latlon_len = idx2 - idx;
        idx = idx2 + 1;
        let invalid = || {
            parse_error(
                format!("Invalid location: '{latlon}'."),
                idx - latlon_len - 1,
            )
        };
        location = match latlon.split_once(':') {
            Some((lat, lon)) => {
                let lat =
                    crate::functions::operators::parse_double_text(lat).map_err(|_| invalid())?;
                let lon =
                    crate::functions::operators::parse_double_text(lon).map_err(|_| invalid())?;
                crate::geoxp::to_geoxp_point(lat, lon)
            }
            None => latlon.parse::<i64>().map_err(|_| invalid())?,
        };
    }
    let Some(idx2) = index_of(&s, ' ', idx) else {
        return Err(parse_error(
            if continuation {
                "Missing value."
            } else {
                "Missing GTS name, labels and value."
            },
            idx,
        ));
    };
    let mut elevation = NO_ELEVATION;
    if idx != idx2 {
        let raw = text(&s[idx..idx2]);
        elevation = raw
            .parse()
            .map_err(|_| parse_error(format!("Invalid elevation: '{raw}'."), idx))?;
    }
    idx = idx2 + 1;
    while idx < s.len() && s[idx] == ' ' as u16 {
        idx += 1;
    }
    let brace = if continuation {
        None
    } else {
        index_of(&s, '{', idx)
    };
    let mut metadata = None;
    let mut attributes = None;
    match brace {
        None => {
            if idx >= s.len() {
                return Err(parse_error("Missing value", idx));
            }
            if last.is_none() {
                return Err(parse_error("Missing or invalid GTS name and labels.", idx));
            }
        }
        Some(brace) => {
            let name =
                crate::format::url_decode(&text(&s[idx..brace])).map_err(LineError::Other)?;
            idx = brace + 1;
            let Some(end) = index_of(&s, '}', idx) else {
                return Err(parse_error("Missing end of labels '}'.", s.len() - 1));
            };
            let labels = labels_at(&text(&s[idx..end]), idx, "Invalid label definition.")?;
            idx = end + 1;
            if idx < s.len() && s[idx] == '{' as u16 {
                idx += 1;
                let start = idx;
                while idx < s.len() && s[idx] != '}' as u16 {
                    idx += 1;
                }
                if idx >= s.len() {
                    return Err(parse_error("Missing end of attributes '}'.", s.len() - 1));
                }
                attributes = Some(labels_at(
                    &text(&s[start..idx]),
                    idx,
                    "Invalid attribute definition.",
                )?);
                idx += 1;
            }
            while idx < s.len() && s[idx] == ' ' as u16 {
                idx += 1;
            }
            if idx >= s.len() {
                return Err(parse_error("Missing value.", s.len() - 1));
            }
            metadata = Some((name, labels));
        }
    }
    let value = parse_value(&text(&s[idx..])).map_err(|pe| {
        LineError::Parse(ParseError {
            message: pe.message,
            offset: idx + pe.offset,
            cause: pe.cause,
        })
    })?;
    Ok(ParsedLine {
        metadata,
        attributes,
        point: (timestamp, location, elevation, value),
    })
}
