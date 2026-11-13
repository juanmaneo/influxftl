// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Java-compatible textual conversions used all over the runtime.
//!
//! * [`java_double_to_string`] — `Double.toString` formatting (`1.0`, `1.0E7`, `NaN`…);
//! * [`is_long`] / [`is_double`] — Warp 10's `UnsafeString` number detection;
//! * [`parse_long`], [`parse_radix_wrapping`] — `Long.valueOf` / `BigInteger.longValue()`;
//! * [`url_encode`] / [`url_decode`] — `WarpURLEncoder` / `WarpURLDecoder`.

use crate::error::Result;
use crate::error::fault::NumberFormatError;

/// `Double.toString(d)`: shortest digits that round-trip, plain notation in `[1e-3, 1e7)`,
/// computerized scientific notation (`d.dddE±n`) otherwise, always at least one fractional digit.
pub fn java_double_to_string(d: f64) -> String {
    if let Some(special) = special_to_string(d.is_nan(), d.is_infinite(), d == 0.0, d) {
        return special;
    }
    let (digits, exponent) = shortest_digits(&closest_shortest(d.abs()));
    let (digits, exponent) = match digits.parse::<u32>() {
        Ok(digit) if digits.len() == 1 && d.abs() < f64::MIN_POSITIVE => {
            let (two_digits, exponent) = closest_two_digits(
                d.abs().to_bits(),
                1074,
                |text| {
                    text.parse::<f64>()
                        .is_ok_and(|c| c.to_bits() == d.abs().to_bits())
                },
                digit,
                exponent,
            );
            (two_digits.to_string(), exponent)
        }
        _ => (digits, exponent),
    };
    layout(d < 0.0, &digits, exponent)
}

/// `Float.toString(f)`: the same notation as [`java_double_to_string`], for the shortest digits
/// that round-trip as a `float`.
pub fn java_float_to_string(f: f32) -> String {
    if let Some(special) = special_to_string(f.is_nan(), f.is_infinite(), f == 0.0, f64::from(f)) {
        return special;
    }
    let (digits, exponent) = shortest_digits(&closest_shortest(f.abs()));
    let (digits, exponent) = match digits.parse::<u32>() {
        Ok(digit) if digits.len() == 1 && f.abs() < f32::MIN_POSITIVE => {
            let (two_digits, exponent) = closest_two_digits(
                u64::from(f.abs().to_bits()),
                149,
                |text| {
                    text.parse::<f32>()
                        .is_ok_and(|c| c.to_bits() == f.abs().to_bits())
                },
                digit,
                exponent,
            );
            (two_digits.to_string(), exponent)
        }
        _ => (digits, exponent),
    };
    layout(f < 0.0, &digits, exponent)
}

/// The text of a `NaN`, an infinity or a zero (`value` is the number, as a double), `None` for
/// any other number.
fn special_to_string(nan: bool, infinite: bool, zero: bool, value: f64) -> Option<String> {
    let text = if nan {
        "NaN"
    } else if infinite {
        if value > 0.0 { "Infinity" } else { "-Infinity" }
    } else if zero {
        if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
    } else {
        return None;
    };
    Some(text.to_owned())
}

/// The scientific text of the shortest decimal that reads back as `value`. When several decimals
/// of that length do, Java takes the one closest to `value`, and the one with an even last digit
/// when two are equally close (`28.4765625f` is `28.476562`), where the shortest digits of Rust
/// go to the upper one.
fn closest_shortest<T>(value: T) -> String
where
    T: std::fmt::LowerExp + std::str::FromStr + PartialEq + Copy,
{
    let shortest = format!("{value:e}");
    let digits = shortest
        .chars()
        .take_while(|c| *c != 'e')
        .filter(char::is_ascii_digit)
        .count();
    let closest = format!("{value:.*e}", digits - 1);
    if closest.parse::<T>().is_ok_and(|read| read == value) {
        closest
    } else {
        shortest
    }
}

/// The digits and the decimal exponent of a number written `d[.ddd]e[-]x` (Rust's `{:e}`).
fn shortest_digits(scientific: &str) -> (String, i32) {
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((scientific, "0"));
    (
        mantissa.chars().filter(|c| *c != '.').collect(),
        decimal_value(exponent) as i32,
    )
}

/// The Java layout of a number of the given `digits` and decimal `exponent`.
fn layout(negative: bool, digits: &str, exponent: i32) -> String {
    let sign = if negative { "-" } else { "" };
    if (-3..7).contains(&exponent) {
        if exponent >= 0 {
            let int_len = (exponent + 1) as usize;
            if digits.len() <= int_len {
                let zeros = "0".repeat(int_len - digits.len());
                format!("{sign}{digits}{zeros}.0")
            } else {
                let (int, frac) = digits.split_at(int_len);
                format!("{sign}{int}.{frac}")
            }
        } else {
            let zeros = "0".repeat((-exponent - 1) as usize);
            format!("{sign}0.{zeros}{digits}")
        }
    } else {
        let (first, rest) = digits.split_at(1);
        let rest = if rest.is_empty() { "0" } else { rest };
        format!("{sign}{first}.{rest}E{exponent}")
    }
}

/// When the shortest decimal that rounds to a subnormal has a single digit, `Double.toString`
/// and `Float.toString` pick instead, among all decimals of one or two digits that round to
/// it, the one closest to it. For a normal number the single digit followed by `0` is already
/// that decimal; only the subnormals have several candidates (`Double.MIN_VALUE` prints as
/// `4.9E-324`, not `5.0E-324`). Returns the two digits (`10..=99`) and the exponent of the
/// first one.
///
/// `bits` is the representation of the subnormal, that is `bits * 2^-unit`; `round_trips` tells
/// whether a decimal text reads back as that number.
fn closest_two_digits(
    bits: u64,
    unit: i32,
    round_trips: impl Fn(&str) -> bool,
    digit: u32,
    exponent: i32,
) -> (u32, i32) {
    // Measure everything in that unit, where `j * 10^x` is `j * 2^(unit + x) / 5^-x`. The f64
    // evaluation is accurate to ~1e-13 while candidates are at least 0.2 units apart, and no
    // candidate can tie exactly.
    let target = bits as f64;
    let distance =
        |j: u32, x: i32| (f64::from(j) * 2f64.powi(unit + x) / 5f64.powi(-x) - target).abs();
    let mut best = (distance(digit * 10, exponent - 1), digit * 10, exponent);
    for x in [exponent - 2, exponent - 1] {
        for j in 10u32..=99 {
            if round_trips(&format!("{j}e{x}")) && distance(j, x) < best.0 {
                best = (distance(j, x), j, x + 1);
            }
        }
    }
    (best.1, best.2)
}

/// `UnsafeString.isLong`: optional leading spaces, optional sign, one or more ASCII digits.
pub fn is_long(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    if i == bytes.len() {
        return false;
    }
    if bytes[i] == b'+' || bytes[i] == b'-' {
        i += 1;
    }
    let mut has_digits = false;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            return false;
        }
        has_digits = true;
        i += 1;
    }
    has_digits
}

/// `UnsafeString.isDouble`: optional leading spaces, optional sign, then `NaN`, `Infinity` or
/// digits with at most one `.`, one `e`/`E` (before any exponent sign) and one exponent sign;
/// a bare integer is **not** a double.
pub fn is_double(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    if i == bytes.len() {
        return false;
    }
    if bytes[i] == b'+' || bytes[i] == b'-' {
        i += 1;
        if i >= bytes.len() {
            return false;
        }
    }
    let rest = &bytes[i..];
    if rest == b"NaN" || rest == b"Infinity" {
        return true;
    }
    if !rest[0].is_ascii_digit() {
        return false;
    }
    let (mut ne, mut nsign, mut ndot) = (0, 0, 0);
    for &ch in rest {
        match ch {
            b'0'..=b'9' => {}
            b'e' | b'E' => {
                if nsign != 0 {
                    return false;
                }
                ne += 1;
            }
            b'+' | b'-' => {
                if ne == 0 {
                    return false;
                }
                nsign += 1;
            }
            b'.' => {
                if ne != 0 || nsign != 0 {
                    return false;
                }
                ndot += 1;
            }
            _ => return false,
        }
    }
    if ne > 1 || nsign > 1 || ndot > 1 {
        return false;
    }
    ne != 0 || ndot != 0
}

/// `Long.valueOf(s)` for a string accepted by [`is_long`]; overflow is an error like Java's
/// `NumberFormatException`.
pub fn parse_long(s: &str) -> Result<i64> {
    let trimmed = s.trim_start_matches(' ');
    let digits = trimmed.strip_prefix('+').unwrap_or(trimmed);
    digits
        .parse::<i64>()
        .map_err(|_| NumberFormatError::InvalidInput(s.to_owned()).into())
}

/// `Double.valueOf(s)` for a string accepted by [`is_double`].
pub fn parse_double(s: &str) -> Result<f64> {
    let trimmed = s.trim_start_matches(' ');
    let unsigned = trimmed
        .strip_prefix('+')
        .or_else(|| trimmed.strip_prefix('-'))
        .unwrap_or(trimmed);
    let magnitude = match unsigned {
        "NaN" => f64::NAN,
        "Infinity" => f64::INFINITY,
        other => other
            .parse::<f64>()
            .map_err(|_| NumberFormatError::InvalidInput(s.to_owned()))?,
    };
    Ok(if trimmed.starts_with('-') {
        -magnitude
    } else {
        magnitude
    })
}

/// Parse `digits` (optional sign, then digits in `radix` 2 or 16) into a `long`, wrapping on
/// overflow exactly like `new BigInteger(s, radix).longValue()`.
pub fn parse_radix_wrapping(digits: &str, radix: u32) -> Result<i64> {
    let bytes = digits.as_bytes();
    let (negative, start) = match bytes.first() {
        Some(b'-') => (true, 1),
        Some(b'+') => (false, 1),
        _ => (false, 0),
    };
    if start >= bytes.len() {
        return Err(NumberFormatError::InvalidInput(digits.to_owned()).into());
    }
    let mut acc: u64 = 0;
    for &b in &bytes[start..] {
        let digit = (b as char)
            .to_digit(radix)
            .ok_or_else(|| NumberFormatError::InvalidInput(digits.to_owned()))?;
        acc = acc
            .wrapping_mul(u64::from(radix))
            .wrapping_add(u64::from(digit));
    }
    let value = acc as i64;
    Ok(if negative {
        value.wrapping_neg()
    } else {
        value
    })
}

/// `WarpURLEncoder.encode`: `java.net.URLEncoder` (UTF-8, uppercase hex) with `+` rendered
/// as `%20`. Letters, digits, `.`, `-`, `*` and `_` are left untouched.
pub fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        if b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'*' || b == b'_' {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX_UPPER[(b >> 4) as usize] as char);
            out.push(HEX_UPPER[(b & 0xF) as usize] as char);
        }
    }
    out
}

const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";

/// `SNAPSHOT.appendProcessedString`: the readable string encoding, which only escapes `%`, `'`
/// and control characters (as `%` + high nibble in decimal + low nibble in lowercase hex, the
/// way Warp 10 does it).
pub fn snapshot_encode_readable(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if c == '%' || c == '\'' || c < ' ' {
            let code = c as u32;
            out.push_str(&format!("%{}{:x}", code >> 4, code & 0xF));
        } else {
            out.push(c);
        }
    }
    out
}

/// `WarpURLDecoder.decode`: `java.net.URLDecoder.decode` applied only when the input contains a
/// `%`, with the `+`s escaped first so that they stay `+`s. Invalid sequences raise the
/// `IllegalArgumentException` of `URLDecoder`.
pub fn url_decode(input: &str) -> Result<String> {
    if !input.contains('%') {
        return Ok(input.to_owned());
    }
    crate::functions::strings::java_url_decode(&input.replace('+', "%2B"))
}

/// The value of a text of ASCII decimal digits, optionally signed, such as the exponents the
/// number formatters of this crate print. It cannot fail: a character that is not a digit
/// counts as zero and the arithmetic wraps, as Java's does.
pub(crate) fn decimal_value(text: &str) -> i64 {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let magnitude = digits.bytes().fold(0i64, |acc, byte| {
        acc.wrapping_mul(10)
            .wrapping_add(i64::from(byte.wrapping_sub(b'0')))
    });
    if negative {
        magnitude.wrapping_neg()
    } else {
        magnitude
    }
}
