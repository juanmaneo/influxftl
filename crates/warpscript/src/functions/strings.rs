//! String functions of WarpLib: case, trimming, sub-strings, splitting and joining, URL and
//! binary-to-text encodings (hex, base64, OPB64, binary), charsets, regular expressions
//! (`MATCH`, `MATCHER`, `REPLACE`, `REPLACEALL`, `REOPTALT`), bit sets and the byte
//! serialisations `->LONGBYTES`, `->VARINT` / `VARINT->` and `->RLP` / `RLP->`.
//!
//! # Strings are UTF-16 in Java
//!
//! Java indexes strings by UTF-16 code unit. The index based functions (`SUBSTRING`, the
//! length checks of `SPLIT`, hex and base64 length validation) therefore work on
//! `str::encode_utf16` so that results match Warp 10 for characters outside the Basic
//! Multilingual Plane; a sub-string that cuts a surrogate pair in half yields U+FFFD where
//! Java would carry a lone surrogate.
//!
//! Regular expressions are compiled by the `fancy-regex` crate (Java-style lookaround and
//! backreferences); the syntax differences with `java.util.regex` are listed in
//! `docs/DIVERGENCES.md`.

use std::any::Any;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::rc::Rc;

use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD, URL_SAFE};
use base64::engine::{DecodePaddingMode, Engine};
use fancy_regex::{Captures, Regex};

use super::Entry;
use super::args::{compute_and_check_index, fail, fail_with_cause, pop_list, pop_long, pop_string};
use super::math::entropy_error;
use crate::bitset::BitSet;
use crate::encoding::{hex_decode, hex_encode, opb64_decode, opb64_encode};
use crate::error::{Error, Result};
use crate::format::url_encode;
use crate::stack::Stack;
use crate::value::{FunctionObject, Value};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("TOUPPER", Entry::Stack(toupper)),
    ("TOLOWER", Entry::Stack(tolower)),
    ("TRIM", Entry::Stack(trim)),
    ("SUBSTRING", Entry::Stack(substring)),
    ("SPLIT", Entry::Stack(split)),
    ("JOIN", Entry::Stack(join)),
    ("URLENCODE", Entry::Stack(urlencode)),
    ("URLDECODE", Entry::Stack(urldecode)),
    ("UUID", Entry::Stack(uuid)),
    ("B64TOHEX", Entry::Stack(b64tohex)),
    ("HEXTOB64", Entry::Stack(hextob64)),
    ("BINTOHEX", Entry::Stack(bintohex)),
    ("HEXTOBIN", Entry::Stack(hextobin)),
    ("BIN->", Entry::Stack(binto)),
    ("HEX->", Entry::Stack(hexto)),
    ("B64->", Entry::Stack(b64to)),
    ("B64URL->", Entry::Stack(b64urlto)),
    ("BYTES->", Entry::Stack(bytesto)),
    ("->BYTES", Entry::Stack(tobytes)),
    ("->B64", Entry::Stack(tob64)),
    ("->B64URL", Entry::Stack(tob64url)),
    ("->OPB64", Entry::Stack(toopb64)),
    ("OPB64->", Entry::Stack(opb64to)),
    ("OPB64TOHEX", Entry::Stack(opb64tohex)),
    ("MATCH", Entry::Stack(match_fn)),
    ("MATCHER", Entry::Stack(matcher)),
    ("REPLACE", Entry::Stack(replace)),
    ("REPLACEALL", Entry::Stack(replaceall)),
    ("REOPTALT", Entry::Stack(reoptalt)),
    ("~=", Entry::Stack(almosteq)),
    ("BITGET", Entry::Stack(bitget)),
    ("BITCOUNT", Entry::Stack(bitcount)),
    ("BITSTOBYTES", Entry::Stack(bitstobytes)),
    ("BYTESTOBITS", Entry::Stack(bytestobits)),
    ("->LONGBYTES", Entry::Stack(tolongbytes)),
    ("->VARINT", Entry::Stack(tovarint)),
    ("VARINT->", Entry::Stack(varintto)),
    ("->RLP", Entry::Stack(torlp)),
    ("RLP->", Entry::Stack(rlpto)),
];

const OPERATES_ON_STRING: &str = "operates on a String.";
const OPERATES_ON_STRING_OR_BYTES: &str = "operates on a String or a byte array.";
const OPERATES_ON_BYTES: &str = "operates on a byte array.";

/// The first UTF-16 unit of `c` (the whole character for the BMP, the high surrogate
/// otherwise), which is what Java prints when it reports a `char`.
fn utf16_lead(c: char) -> u16 {
    let mut units = [0u16; 2];
    c.encode_utf16(&mut units)[0]
}

// -----------------------------------------------------------------------------------------
// Case, trimming, sub-strings
// -----------------------------------------------------------------------------------------

/// `String.toUpperCase()` with the root locale (the JVM default locale is not consulted).
fn toupper(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "TOUPPER", OPERATES_ON_STRING)?;
    stack.push(s.to_uppercase());
    Ok(())
}

/// `String.toLowerCase()` with the root locale.
fn tolower(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "TOLOWER", OPERATES_ON_STRING)?;
    stack.push(s.to_lowercase());
    Ok(())
}

/// `String.trim()`: strips the leading and trailing characters at or below U+0020.
fn trim(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "TRIM", OPERATES_ON_STRING)?;
    stack.push(s.trim_matches(|c: char| c <= ' '));
    Ok(())
}

/// The start index of `SUBSTRING`: 0 is accepted as is (so that empty inputs work), anything
/// else goes through `GET.computeAndCheckIndex`.
fn substring_start(index: i32, size: usize) -> Result<usize> {
    if index == 0 {
        Ok(0)
    } else {
        Ok(compute_and_check_index(index, size as i32)? as usize)
    }
}

/// `SUBSTRING`: `STRING|BYTES start SUBSTRING` or `STRING|BYTES start length SUBSTRING`.
/// String indices are UTF-16 code units.
fn substring(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "SUBSTRING";
    let Value::Long(n) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a numeric length or 0-based start index on top of the stack.",
        ));
    };
    let n = n as i32;
    let idx = match stack.pop()? {
        Value::String(s) => {
            let units: Vec<u16> = s.encode_utf16().collect();
            let start = substring_start(n, units.len())?;
            stack.push(String::from_utf16_lossy(&units[start..]));
            return Ok(());
        }
        Value::Bytes(bytes) => {
            let start = substring_start(n, bytes.len())?;
            stack.push(Value::bytes(&bytes[start..]));
            return Ok(());
        }
        Value::Long(idx) => idx as i32,
        _ => {
            return Err(fail(
                NAME,
                "expects a 0-based start index below the length.",
            ));
        }
    };
    match stack.pop()? {
        Value::String(s) => {
            let units: Vec<u16> = s.encode_utf16().collect();
            let len = units.len() as i32;
            let idx = substring_start(idx, units.len())? as i32;
            let end = n.wrapping_add(idx).min(len);
            if end < idx {
                return Err(Error::new(format!(
                    "Range [{idx}, {end}) out of bounds for length {len}"
                )));
            }
            stack.push(String::from_utf16_lossy(&units[idx as usize..end as usize]));
        }
        Value::Bytes(bytes) => {
            let len = bytes.len() as i32;
            let idx = substring_start(idx, bytes.len())? as i32;
            let end = n.wrapping_add(idx).min(len);
            if end < idx {
                return Err(Error::new(format!("{idx} > {end}")));
            }
            stack.push(Value::bytes(&bytes[idx as usize..end as usize]));
        }
        _ => {
            return Err(fail(NAME, "can only operate on strings or byte arrays."));
        }
    }
    Ok(())
}

// -----------------------------------------------------------------------------------------
// Splitting and joining
// -----------------------------------------------------------------------------------------

/// The character of a `STRING` of UTF-16 length 1 (`SPLIT`'s delimiter).
fn single_utf16_char(value: &Value) -> Option<char> {
    let Value::String(s) = value else {
        return None;
    };
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.len_utf16() == 1 => Some(c),
        _ => None,
    }
}

/// `SPLIT`: `STRING delimiter [limit] SPLIT` → list of the parts (at most `limit`).
fn split(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "SPLIT";
    let mut top = stack.pop()?;
    let mut limit = usize::MAX;
    if let Value::Long(l) = top {
        let l = i32::try_from(l).map_err(|_| Error::new("integer overflow"))?;
        if l <= 0 {
            return Err(fail(
                NAME,
                "expects the limit to be a strictly positive integer.",
            ));
        }
        limit = l as usize;
        top = stack.pop()?;
    }
    let delimiter = single_utf16_char(&top)
        .ok_or_else(|| fail(NAME, "expects a string delimiter of length 1."))?;
    let input = pop_string(stack, NAME, OPERATES_ON_STRING)?;
    let parts: Vec<Value> = input.splitn(limit, delimiter).map(Value::string).collect();
    stack.push(Value::list(parts));
    Ok(())
}

/// `JOIN`: `LIST separator JOIN` or `… separator n JOIN` (`n` elements popped from the stack).
fn join(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "JOIN";
    match stack.pop()? {
        Value::String(sep) => {
            let list = pop_list(
                stack,
                NAME,
                "expects a separator and a list of strings to join on the stack.",
            )?;
            let mut out = String::new();
            for item in list.borrow().iter() {
                // Warp 10 only inserts the separator when the buffer is not empty, so leading
                // empty strings do not produce a separator.
                if !out.is_empty() {
                    out.push_str(&sep);
                }
                out.push_str(&item.to_string());
            }
            stack.push(out);
        }
        Value::Long(n) => {
            let sep = pop_string(
                stack,
                NAME,
                "expects the separator string below the number of strings to join.",
            )?;
            let n = n as i32;
            if n < 0 {
                return Err(Error::new("Index out of bound."));
            }
            let items = stack.popn(n as usize)?;
            let mut out = String::new();
            for (i, item) in items.iter().enumerate() {
                if i != 0 {
                    out.push_str(&sep);
                }
                if item.is_null() {
                    return Err(fail(NAME, "cannot join a NULL element."));
                }
                out.push_str(&item.to_string());
            }
            stack.push(out);
        }
        _ => {
            return Err(fail(
                NAME,
                "expects the number of strings to join on top of the stack.",
            ));
        }
    }
    Ok(())
}

// -----------------------------------------------------------------------------------------
// URL encoding
// -----------------------------------------------------------------------------------------

/// `URLENCODE`: `WarpURLEncoder.encode` (UTF-8, `+` encoded as `%20`).
fn urlencode(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "URLENCODE", OPERATES_ON_STRING)?;
    stack.push(url_encode(&s));
    Ok(())
}

/// `URLDECODE`: `java.net.URLDecoder.decode(s, "UTF-8")`, which also turns `+` into a space.
fn urldecode(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "URLDECODE", OPERATES_ON_STRING)?;
    stack.push(java_url_decode(&s)?);
    Ok(())
}

/// A hexadecimal digit of a `%xx` escape (`HexFormat.fromHexDigit`).
fn url_hex_digit(c: char) -> Result<u8> {
    c.to_digit(16).map(|d| d as u8).ok_or_else(|| {
        Error::new(format!(
            "URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: \"{c}\" = {}",
            utf16_lead(c)
        ))
    })
}

/// `java.net.URLDecoder.decode(input, "UTF-8")`: `+` becomes a space, each run of `%xx`
/// escapes is decoded as UTF-8 (malformed bytes become U+FFFD), a `%` without two
/// following characters is an error.
pub(crate) fn java_url_decode(input: &str) -> Result<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut bytes = Vec::new();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        match c {
            '+' => {
                out.push(' ');
                i += 1;
            }
            '%' => {
                bytes.clear();
                loop {
                    match chars.get(i..) {
                        Some(&['%', hi, lo, ..]) => {
                            bytes.push((url_hex_digit(hi)? << 4) | url_hex_digit(lo)?);
                            i += 3;
                        }
                        Some(['%', ..]) => {
                            return Err(Error::new(
                                "URLDecoder: Incomplete trailing escape (%) pattern",
                            ));
                        }
                        _ => break,
                    }
                }
                out.push_str(&decode_utf8_java(&bytes));
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    Ok(out)
}

// -----------------------------------------------------------------------------------------
// UUID
// -----------------------------------------------------------------------------------------

/// `UUID.randomUUID().toString()` built from 128 random bits: version 4, IETF variant.
pub(crate) fn uuid_string(msb: u64, lsb: u64) -> String {
    let msb = (msb & 0xFFFF_FFFF_FFFF_0FFF) | 0x4000;
    let lsb = (lsb & 0x3FFF_FFFF_FFFF_FFFF) | 0x8000_0000_0000_0000;
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        msb >> 32,
        (msb >> 16) & 0xFFFF,
        msb & 0xFFFF,
        lsb >> 48,
        lsb & 0xFFFF_FFFF_FFFF
    )
}

/// Push a random UUID built from `entropy` (128 bits from the operating system).
pub(crate) fn push_uuid(
    stack: &mut Stack,
    entropy: core::result::Result<(u64, u64), getrandom::Error>,
) -> Result<()> {
    let (msb, lsb) = entropy.map_err(entropy_error)?;
    stack.push(uuid_string(msb, lsb));
    Ok(())
}

/// `UUID`: push a random (version 4) UUID.
fn uuid(stack: &mut Stack) -> Result<()> {
    push_uuid(
        stack,
        getrandom::u64().and_then(|msb| getrandom::u64().map(|lsb| (msb, lsb))),
    )
}

// -----------------------------------------------------------------------------------------
// Hexadecimal, binary, base64 and OPB64
// -----------------------------------------------------------------------------------------

/// Lenient decoding like Guava's `BaseEncoding`: padding is optional, trailing bits are
/// ignored.
const LENIENT: GeneralPurposeConfig = GeneralPurposeConfig::new()
    .with_decode_padding_mode(DecodePaddingMode::RequireNone)
    .with_decode_allow_trailing_bits(true);
const STANDARD_LENIENT: GeneralPurpose = GeneralPurpose::new(&alphabet::STANDARD, LENIENT);
const URL_SAFE_LENIENT: GeneralPurpose = GeneralPurpose::new(&alphabet::URL_SAFE, LENIENT);

/// Guava `BaseEncoding.base64().decode` / `base64Url().decode`: any number of trailing `=`
/// is accepted, a length of 1 modulo 4 (in UTF-16 units) is rejected, characters outside
/// the alphabet are errors, bits left over in the last symbol are ignored.
pub(crate) fn base64_decode(text: &str, url_safe: bool) -> Result<Vec<u8>> {
    let trimmed = text.trim_end_matches('=');
    let len = trimmed.encode_utf16().count();
    if len % 4 == 1 {
        return Err(Error::new(format!("Invalid input length {len}")));
    }
    let (engine, alphabet) = if url_safe {
        (&URL_SAFE_LENIENT, alphabet::URL_SAFE.as_str())
    } else {
        (&STANDARD_LENIENT, alphabet::STANDARD.as_str())
    };
    engine
        .decode(trimmed)
        .map_err(|err| base64_decode_error(trimmed, alphabet, &err))
}

/// The Guava error for a decoding failure. Guava reads the input sequentially and reports
/// the first character outside the alphabet (printable ASCII as is, anything else as the
/// hexadecimal UTF-16 unit); the crate's own message is kept for any other failure.
pub(crate) fn base64_decode_error(text: &str, alphabet: &str, err: &base64::DecodeError) -> Error {
    match text.chars().find(|&c| !alphabet.contains(c)) {
        Some(c) if ('!'..='~').contains(&c) => Error::new(format!("Unrecognized character: {c}")),
        Some(c) => Error::new(format!("Unrecognized character: 0x{:x}", utf16_lead(c))),
        None => Error::new(err.to_string()),
    }
}

/// `B64TOHEX`: base64 → lowercase hexadecimal.
fn b64tohex(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "B64TOHEX", OPERATES_ON_STRING)?;
    stack.push(hex_encode(&base64_decode(&s, false)?));
    Ok(())
}

/// `HEXTOB64`: hexadecimal → base64.
fn hextob64(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "HEXTOB64", OPERATES_ON_STRING)?;
    stack.push(STANDARD.encode(hex_decode(&s)?));
    Ok(())
}

/// `BINTOx.apply`: a string of `0` / `1` left-padded to a multiple of 8 bits, as bytes.
fn bin_to_bytes(name: &str, bin: &str) -> Result<Vec<u8>> {
    let missing = (8 - bin.chars().count() % 8) % 8;
    let mut out = Vec::with_capacity(bin.len() / 8 + 1);
    let mut current: u8 = 0;
    for (i, c) in bin.chars().enumerate() {
        current <<= 1;
        match c {
            '1' => current |= 1,
            '0' => {}
            _ => {
                return Err(fail(
                    name,
                    &format!("expects a valid binary representation. \"{bin}\" is invalid."),
                ));
            }
        }
        if (i + missing) % 8 == 7 {
            out.push(current);
            current = 0;
        }
    }
    Ok(out)
}

/// `BINTOHEX`: binary digits → uppercase hexadecimal.
fn bintohex(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "BINTOHEX", OPERATES_ON_STRING)?;
    stack.push(hex_encode(&bin_to_bytes("BINTOHEX", &s)?).to_uppercase());
    Ok(())
}

/// `HEXTOBIN`: hexadecimal digits → binary digits (4 per character).
fn hextobin(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "HEXTOBIN", OPERATES_ON_STRING)?;
    let mut out = String::with_capacity(s.len() * 4);
    for c in s.chars() {
        match c.to_digit(16) {
            Some(d) => out.push_str(&format!("{d:04b}")),
            None => {
                return Err(fail(
                    "HEXTOBIN",
                    &format!("encountered an invalid hex character '{c}'."),
                ));
            }
        }
    }
    stack.push(out);
    Ok(())
}

/// `BIN->`: binary digits → bytes.
fn binto(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "BIN->", OPERATES_ON_STRING)?;
    stack.push(Value::bytes(bin_to_bytes("BIN->", &s)?));
    Ok(())
}

/// `HEX->`: hexadecimal → bytes, an odd number of digits is left-padded with `0`.
fn hexto(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "HEX->", OPERATES_ON_STRING)?;
    let bytes = if s.encode_utf16().count() % 2 == 1 {
        hex_decode(&format!("0{s}"))?
    } else {
        hex_decode(&s)?
    };
    stack.push(Value::bytes(bytes));
    Ok(())
}

/// `B64->`: base64 → bytes, ignoring line breaks and spaces.
fn b64to(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "B64->", OPERATES_ON_STRING)?;
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, '\n' | '\r' | ' '))
        .collect();
    stack.push(Value::bytes(base64_decode(&cleaned, false)?));
    Ok(())
}

/// `B64URL->`: URL-safe base64 → bytes.
fn b64urlto(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "B64URL->", OPERATES_ON_STRING)?;
    stack.push(Value::bytes(base64_decode(&s, true)?));
    Ok(())
}

/// The bytes a `STRING` (UTF-8) or `BYTES` operand stands for.
fn pop_string_or_bytes(stack: &mut Stack, name: &str) -> Result<Rc<[u8]>> {
    match stack.pop()? {
        Value::String(s) => Ok(Rc::from(s.as_bytes())),
        Value::Bytes(b) => Ok(b),
        _ => Err(fail(name, OPERATES_ON_STRING_OR_BYTES)),
    }
}

/// `->B64`: string (UTF-8) or bytes → base64 (padded).
fn tob64(stack: &mut Stack) -> Result<()> {
    let data = pop_string_or_bytes(stack, "->B64")?;
    stack.push(STANDARD.encode(data));
    Ok(())
}

/// `->B64URL`: string (UTF-8) or bytes → URL-safe base64 (padded, like Guava).
fn tob64url(stack: &mut Stack) -> Result<()> {
    let data = pop_string_or_bytes(stack, "->B64URL")?;
    stack.push(URL_SAFE.encode(data));
    Ok(())
}

/// `->OPB64`: string (UTF-8) or bytes → order preserving base64.
fn toopb64(stack: &mut Stack) -> Result<()> {
    let data = pop_string_or_bytes(stack, "->OPB64")?;
    stack.push(opb64_encode(&data));
    Ok(())
}

/// `OPB64->`: order preserving base64 → bytes.
fn opb64to(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "OPB64->", OPERATES_ON_STRING)?;
    stack.push(Value::bytes(opb64_decode(&s)?));
    Ok(())
}

/// `OPB64TOHEX`: order preserving base64 → lowercase hexadecimal.
fn opb64tohex(stack: &mut Stack) -> Result<()> {
    let s = pop_string(stack, "OPB64TOHEX", OPERATES_ON_STRING)?;
    stack.push(hex_encode(&opb64_decode(&s)?));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// Charsets
// -----------------------------------------------------------------------------------------

/// The charsets supported by `->BYTES` / `BYTES->`, looked up by their JDK canonical name or
/// aliases (case-insensitive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Charset {
    /// `UTF-8`.
    Utf8,
    /// `ISO-8859-1`.
    Latin1,
    /// `US-ASCII`.
    Ascii,
    /// `UTF-16`: big endian with a byte order mark when encoding, BOM detection when decoding.
    Utf16,
    /// `UTF-16BE`.
    Utf16Be,
    /// `UTF-16LE`.
    Utf16Le,
}

impl Charset {
    /// `Charset.forName` restricted to the supported charsets.
    pub(crate) fn for_name(name: &str) -> Option<Charset> {
        match name.to_ascii_uppercase().as_str() {
            "UTF-8" | "UTF8" | "UNICODE-1-1-UTF-8" => Some(Charset::Utf8),
            "ISO-8859-1" | "ISO8859-1" | "ISO8859_1" | "ISO_8859-1" | "ISO_8859_1"
            | "ISO_8859-1:1987" | "ISO-IR-100" | "LATIN1" | "L1" | "IBM819" | "IBM-819"
            | "CP819" | "819" | "CSISOLATIN1" | "8859_1" => Some(Charset::Latin1),
            "US-ASCII" | "ASCII" | "US" | "ISO646-US" | "ISO-IR-6" | "ANSI_X3.4-1986"
            | "ANSI_X3.4-1968" | "ISO_646.IRV:1991" | "ISO_646.IRV:1983" | "IBM367" | "CP367"
            | "CSASCII" | "DEFAULT" | "646" | "ASCII7" => Some(Charset::Ascii),
            "UTF-16" | "UTF_16" | "UTF16" | "UNICODE" | "UNICODEBIG" => Some(Charset::Utf16),
            "UTF-16BE" | "UTF_16BE" | "X-UTF-16BE" | "ISO-10646-UCS-2" | "UNICODEBIGUNMARKED" => {
                Some(Charset::Utf16Be)
            }
            "UTF-16LE" | "UTF_16LE" | "X-UTF-16LE" | "UNICODELITTLEUNMARKED" => {
                Some(Charset::Utf16Le)
            }
            _ => None,
        }
    }

    /// `String.getBytes(charset)`: unmappable characters become `?`, `UTF-16` starts with a
    /// byte order mark (unless the string is empty).
    pub(crate) fn encode(self, s: &str) -> Vec<u8> {
        match self {
            Charset::Utf8 => s.as_bytes().to_vec(),
            Charset::Latin1 => s
                .chars()
                .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
                .collect(),
            Charset::Ascii => s
                .chars()
                .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
                .collect(),
            Charset::Utf16 => {
                let mut out = Vec::with_capacity(2 + s.len() * 2);
                if !s.is_empty() {
                    out.extend_from_slice(&[0xFE, 0xFF]);
                }
                out.extend(s.encode_utf16().flat_map(u16::to_be_bytes));
                out
            }
            Charset::Utf16Be => s.encode_utf16().flat_map(u16::to_be_bytes).collect(),
            Charset::Utf16Le => s.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        }
    }

    /// `new String(bytes, charset)`: malformed input is replaced with U+FFFD.
    pub(crate) fn decode(self, bytes: &[u8]) -> String {
        match self {
            Charset::Utf8 => decode_utf8_java(bytes),
            Charset::Latin1 => bytes.iter().map(|&b| char::from(b)).collect(),
            Charset::Ascii => bytes
                .iter()
                .map(|&b| if b < 0x80 { char::from(b) } else { '\u{FFFD}' })
                .collect(),
            Charset::Utf16 => match bytes {
                [0xFE, 0xFF, rest @ ..] => decode_utf16(rest, true),
                [0xFF, 0xFE, rest @ ..] => decode_utf16(rest, false),
                _ => decode_utf16(bytes, true),
            },
            Charset::Utf16Be => decode_utf16(bytes, true),
            Charset::Utf16Le => decode_utf16(bytes, false),
        }
    }
}

/// `new String(bytes, UTF_8)`: like `String::from_utf8_lossy`, except that an encoded
/// surrogate (`ED A0..BF xx`) is one malformed sequence for Java (a single U+FFFD) whereas
/// Rust reports each of its bytes.
pub(crate) fn decode_utf8_java(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut rest = bytes;
    loop {
        match std::str::from_utf8(rest) {
            Ok(valid) => {
                out.push_str(valid);
                return out;
            }
            Err(err) => {
                let valid_up_to = err.valid_up_to();
                out.push_str(&String::from_utf8_lossy(&rest[..valid_up_to]));
                out.push('\u{FFFD}');
                let malformed = &rest[valid_up_to..];
                let skip = match malformed {
                    [0xED, 0xA0..=0xBF, 0x80..=0xBF, ..] => 3,
                    [0xED, 0xA0..=0xBF, ..] => 2,
                    _ => err.error_len().unwrap_or(malformed.len()),
                };
                rest = &malformed[skip..];
            }
        }
    }
}

/// The JDK UTF-16 decoder: a high surrogate must be followed by a low surrogate (otherwise
/// both units are one malformed sequence), a lone low surrogate and a trailing odd byte are
/// malformed, and a high surrogate with fewer than two bytes left swallows the rest.
fn decode_utf16(bytes: &[u8], big_endian: bool) -> String {
    let unit = |pair: &[u8; 2]| {
        if big_endian {
            u16::from_be_bytes(*pair)
        } else {
            u16::from_le_bytes(*pair)
        }
    };
    let (pairs, odd) = bytes.as_chunks::<2>();
    let mut out = String::with_capacity(pairs.len());
    let mut iter = pairs.iter().map(unit);
    while let Some(first) = iter.next() {
        if (0xD800..0xDC00).contains(&first) {
            let Some(second) = iter.next() else {
                out.push('\u{FFFD}');
                return out;
            };
            match char::decode_utf16([first, second]).next() {
                Some(Ok(c)) => out.push(c),
                _ => out.push('\u{FFFD}'),
            }
        } else {
            out.push(char::from_u32(u32::from(first)).unwrap_or('\u{FFFD}'));
        }
    }
    if !odd.is_empty() {
        out.push('\u{FFFD}');
    }
    out
}

/// The charset named by the top of the stack, or the Java `UnsupportedEncodingException`
/// wrapped like `TOBYTES` / `BYTESTO` do.
fn charset_for(name: &str, charset: &str) -> Result<Charset> {
    Charset::for_name(charset).ok_or_else(|| {
        fail_with_cause(
            name,
            &format!("unsupported encoding '{charset}'"),
            Error::new(charset),
        )
    })
}

/// `BYTES->`: `BYTES charset BYTES->` → string.
fn bytesto(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "BYTES->";
    let charset = pop_string(
        stack,
        NAME,
        "operates on a byte array and expects a charset name on top of the stack.",
    )?;
    let Value::Bytes(bytes) = stack.pop()? else {
        return Err(fail(NAME, OPERATES_ON_BYTES));
    };
    let charset = charset_for(NAME, &charset)?;
    stack.push(charset.decode(&bytes));
    Ok(())
}

/// `->BYTES`: `STRING charset ->BYTES` → bytes.
fn tobytes(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "->BYTES";
    let charset = pop_string(
        stack,
        NAME,
        "operates on a String and expects a charset name on top of the stack.",
    )?;
    let s = pop_string(stack, NAME, OPERATES_ON_STRING)?;
    let charset = charset_for(NAME, &charset)?;
    stack.push(Value::bytes(charset.encode(&s)));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// Regular expressions
// -----------------------------------------------------------------------------------------

/// A compiled regular expression on the stack (the Java `Matcher` pushed by `MATCHER`).
pub struct CompiledPattern {
    source: Rc<str>,
    /// The pattern anchored at both ends, which is what `Matcher.matches()` tests.
    full: Regex,
}

/// The message of a regular expression error (`PatternSyntaxException` and the runtime
/// limits of the backtracking engine).
fn regex_error(err: &fancy_regex::Error) -> Error {
    Error::new(err.to_string())
}

/// Rewrite the Java-only `\Q…\E` quoting into escaped literals (`\E` may be omitted, in
/// which case the quotation runs to the end of the pattern).
fn expand_quoting(source: &str) -> Cow<'_, str> {
    if !source.contains("\\Q") {
        return Cow::Borrowed(source);
    }
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while !rest.is_empty() {
        if let Some(quoted) = rest.strip_prefix("\\Q") {
            let (literal, remainder) = quoted.split_once("\\E").unwrap_or((quoted, ""));
            out.push_str(&fancy_regex::escape(literal));
            rest = remainder;
        } else if let Some(escaped) = rest.strip_prefix('\\') {
            // Any other escape is kept with the character it applies to (so that `\\Q` stays
            // an escaped backslash followed by `Q`).
            let len = 1 + escaped.chars().next().map_or(0, char::len_utf8);
            out.push_str(&rest[..len]);
            rest = &rest[len..];
        } else {
            let len = rest.chars().next().map_or(1, char::len_utf8);
            out.push_str(&rest[..len]);
            rest = &rest[len..];
        }
    }
    Cow::Owned(out)
}

/// `Pattern.compile(source)`: a Java pattern compiled by `fancy-regex`.
fn compile_regex(source: &str) -> fancy_regex::Result<Regex> {
    Regex::new(&expand_quoting(source))
}

/// Compile `source` anchored at both ends (`Matcher.matches()` semantics, including the
/// backtracking into alternations that `a|ab` needs to match `ab`).
fn compile_anchored(source: &str) -> fancy_regex::Result<Regex> {
    // Validate the pattern on its own first: wrapping could turn an unbalanced parenthesis
    // into a valid (but different) expression.
    compile_regex(source)?;
    let source = expand_quoting(source);
    // A verbose pattern (`(?x)`) ending with a `#` comment swallows the closing parenthesis;
    // a line break (ignored in verbose mode) ends the comment.
    Regex::new(&format!("\\A(?:{source})\\z"))
        .or_else(|_| Regex::new(&format!("\\A(?:{source}\n)\\z")))
}

impl CompiledPattern {
    /// `Pattern.compile(source)`.
    pub fn compile(source: &str) -> Result<CompiledPattern> {
        let full = compile_anchored(source).map_err(|err| regex_error(&err))?;
        Ok(CompiledPattern {
            source: Rc::from(source),
            full,
        })
    }

    /// The pattern source.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// `Matcher.matches()` followed by `group(0..=groupCount())`: the groups (`NULL` for the
    /// ones that did not participate) when the whole `text` matches, an empty list otherwise.
    pub fn full_match(&self, text: &str) -> Result<Vec<Value>> {
        let Some(caps) = self.full.captures(text).map_err(|err| regex_error(&err))? else {
            return Ok(Vec::new());
        };
        Ok((0..self.full.captures_len())
            .map(|i| {
                caps.get(i)
                    .map_or(Value::Null, |m| Value::string(m.as_str()))
            })
            .collect())
    }
}

impl FunctionObject for CompiledPattern {
    fn name(&self) -> &str {
        &self.source
    }

    fn type_name(&self) -> &'static str {
        "MATCHER"
    }

    /// `'pattern' MATCHER`, with the raw pattern like `SNAPSHOT` does in Warp 10.
    fn snapshot(&self) -> String {
        format!("'{}' MATCHER", self.source)
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }
}

/// The compiled pattern carried by a `MATCHER` value.
fn matcher_of(value: &Value) -> Option<&CompiledPattern> {
    match value {
        Value::Function(f) => f.as_any()?.downcast_ref::<CompiledPattern>(),
        _ => None,
    }
}

/// `MATCH`: `STRING regexp|MATCHER MATCH` → the list of groups when the whole string matches
/// (empty list otherwise).
fn match_fn(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MATCH";
    let regexp = stack.pop()?;
    let compiled;
    let pattern = match &regexp {
        Value::String(source) => {
            compiled = CompiledPattern::compile(source)?;
            &compiled
        }
        other => matcher_of(other).ok_or_else(|| {
            fail(
                NAME,
                "regular expression MUST be a string, a compiled pattern or a matcher.",
            )
        })?,
    };
    let text = pop_string(stack, NAME, "can only be applied to a string.")?;
    let groups = pattern.full_match(&text)?;
    stack.push(Value::list(groups));
    Ok(())
}

/// `MATCHER`: compile a regular expression into a reusable `MATCHER` object.
fn matcher(stack: &mut Stack) -> Result<()> {
    let source = pop_string(stack, "MATCHER", "expects a STRING.")?;
    let compiled = CompiledPattern::compile(&source)?;
    stack.push(Value::Function(Rc::new(compiled)));
    Ok(())
}

/// Append the expansion of a Java replacement string for one match
/// (`Matcher.appendReplacement`): `$n`, `${name}` and `\x` escapes.
fn expand_replacement(
    out: &mut String,
    replacement: &[char],
    caps: &Captures<'_, str>,
    regex: &Regex,
) -> Result<()> {
    let group_count = regex.captures_len().saturating_sub(1);
    let mut cursor = 0;
    while let Some(&c) = replacement.get(cursor) {
        cursor += 1;
        if c == '\\' {
            let Some(&escaped) = replacement.get(cursor) else {
                return Err(Error::new("character to be escaped is missing"));
            };
            out.push(escaped);
            cursor += 1;
        } else if c == '$' {
            let Some(&next) = replacement.get(cursor) else {
                return Err(Error::new(
                    "Illegal group reference: group index is missing",
                ));
            };
            let group = if next == '{' {
                cursor += 1;
                let start = cursor;
                while replacement
                    .get(cursor)
                    .is_some_and(char::is_ascii_alphanumeric)
                {
                    cursor += 1;
                }
                let name: String = replacement[start..cursor].iter().collect();
                if name.is_empty() {
                    return Err(Error::new("named capturing group has 0 length name"));
                }
                if replacement.get(cursor) != Some(&'}') {
                    return Err(Error::new("named capturing group is missing trailing '}'"));
                }
                cursor += 1;
                if name.starts_with(|c: char| c.is_ascii_digit()) {
                    return Err(Error::new(format!(
                        "capturing group name {{{name}}} starts with digit character"
                    )));
                }
                regex
                    .capture_names()
                    .position(|n| n == Some(name.as_str()))
                    .ok_or_else(|| Error::new(format!("No group with name {{{name}}}")))?
            } else {
                let Some(first) = next.to_digit(10) else {
                    return Err(Error::new("Illegal group reference"));
                };
                cursor += 1;
                // The largest legal group number is taken: `$10` is group 10 when it exists,
                // group 1 followed by a literal `0` otherwise.
                let mut group = first as usize;
                while let Some(digit) = replacement.get(cursor).and_then(|c| c.to_digit(10)) {
                    let candidate = group * 10 + digit as usize;
                    if group_count < candidate {
                        break;
                    }
                    group = candidate;
                    cursor += 1;
                }
                group
            };
            if group > group_count {
                return Err(Error::new(format!("No group {group}")));
            }
            if let Some(m) = caps.get(group) {
                out.push_str(m.as_str());
            }
        } else {
            out.push(c);
        }
    }
    Ok(())
}

/// `String.replaceAll` / `String.replaceFirst`: every (or the first) match of `regexp` in
/// `input` is replaced by the expansion of `replacement`. The search resumes right after a
/// match, or one character further after an empty match, like `Matcher.find()`.
pub(crate) fn regex_replace(
    input: &str,
    regexp: &str,
    replacement: &str,
    all: bool,
) -> Result<String> {
    let regex = compile_regex(regexp).map_err(|err| regex_error(&err))?;
    let replacement: Vec<char> = replacement.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut tail = 0;
    let mut from = 0;
    while from <= input.len() {
        let Some(caps) = regex
            .captures_from_pos(input, from)
            .map_err(|err| regex_error(&err))?
        else {
            break;
        };
        let (start, end) = caps.get(0).map_or((from, from), |m| (m.start(), m.end()));
        out.push_str(input.get(tail..start).unwrap_or_default());
        expand_replacement(&mut out, &replacement, &caps, &regex)?;
        tail = end;
        from = if start == end {
            end + input
                .get(end..)
                .and_then(|s| s.chars().next())
                .map_or(1, char::len_utf8)
        } else {
            end
        };
        if !all {
            break;
        }
    }
    out.push_str(input.get(tail..).unwrap_or_default());
    Ok(out)
}

/// The `toString()` of an operand of `REPLACE` / `REPLACEALL` (`null` has none).
fn pop_to_string(stack: &mut Stack, name: &str) -> Result<String> {
    let value = stack.pop()?;
    if value.is_null() {
        return Err(fail(name, "cannot operate on NULL."));
    }
    Ok(value.to_string())
}

/// `REPLACE` / `REPLACEALL`: `input regexp replacement REPLACE`.
fn replace_with(stack: &mut Stack, name: &str, all: bool) -> Result<()> {
    let replacement = pop_to_string(stack, name)?;
    let regexp = pop_to_string(stack, name)?;
    let input = pop_to_string(stack, name)?;
    stack.push(regex_replace(&input, &regexp, &replacement, all)?);
    Ok(())
}

fn replace(stack: &mut Stack) -> Result<()> {
    replace_with(stack, "REPLACE", false)
}

fn replaceall(stack: &mut Stack) -> Result<()> {
    replace_with(stack, "REPLACEALL", true)
}

/// `~=` (`ALMOSTEQ`): `op1 op2 lambda ~=` → `|op1 - op2| <= |lambda|`; two `NaN` are equal.
#[allow(clippy::float_cmp)]
fn almosteq(stack: &mut Stack) -> Result<()> {
    let lambda = stack.pop()?;
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    let (Some(lambda), Some(op2), Some(op1)) = (lambda.as_f64(), op2.as_f64(), op1.as_f64()) else {
        return Err(fail("~=", "only accepts numeric parameters."));
    };
    if op1.is_nan() || op2.is_nan() {
        stack.push(op1.is_nan() && op2.is_nan());
    } else {
        stack.push(lambda.abs() >= (op1 - op2).abs());
    }
    Ok(())
}

// -----------------------------------------------------------------------------------------
// REOPTALT
// -----------------------------------------------------------------------------------------

/// `REOPTALT`: a list of strings → a regular expression matching exactly those strings, with
/// common prefixes and suffixes factored out. Operates on Unicode scalar values where Java
/// uses UTF-16 units (identical results for BMP text).
fn reoptalt(stack: &mut Stack) -> Result<()> {
    let list = pop_list(
        stack,
        "REOPTALT",
        "expects a list of strings on top of the stack.",
    )?;
    let alternatives: Vec<Vec<char>> = list
        .borrow()
        .iter()
        .map(|item| item.to_string().chars().collect())
        .collect();
    stack.push(build_opt_alt(&alternatives));
    Ok(())
}

/// `REOPTALT.escape`: backslash-escape the regex meta characters (the backslash itself is
/// not escaped, like in Warp 10).
fn reoptalt_escape(s: &[char]) -> String {
    let mut out = String::with_capacity(s.len());
    for &c in s {
        if "[]{}()|.?+*".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The prefix length that minimises the number of distinct prefixes among the sorted,
/// non-empty `alternatives`.
fn reoptalt_prefix_length(alternatives: &[&[char]]) -> usize {
    let mut len = 1;
    let mut prevcard = i32::MAX;
    let mut cardinality = 0;
    let mut minlen = alternatives.first().map_or(0, |a| a.len());
    while cardinality <= prevcard && len <= minlen {
        cardinality = 1;
        let mut idx = 1;
        while idx < alternatives.len() && cardinality <= prevcard {
            let previous = alternatives.get(idx - 1).copied().unwrap_or_default();
            let current = alternatives.get(idx).copied().unwrap_or_default();
            if len == 1 {
                minlen = minlen.min(current.len());
            }
            if previous.get(..len) != current.get(..len) {
                cardinality += 1;
            }
            idx += 1;
        }
        if cardinality <= prevcard {
            prevcard = cardinality;
            len += 1;
        } else {
            len -= 1;
        }
    }
    len.min(minlen)
}

/// Append the optimised alternation of `suffixes`, factoring out their common end when they
/// all have the same length.
fn reoptalt_append_suffixes(regexp: &mut String, suffixes: &BTreeSet<&[char]>) {
    let mut subalts: Vec<Vec<char>> = suffixes.iter().map(|s| s.to_vec()).collect();
    let mut end: Vec<char> = Vec::new();
    let mut endlen = 0;
    if let [first, others @ ..] = subalts.as_slice()
        && !others.is_empty()
        && others.iter().all(|s| s.len() == first.len())
    {
        let length = first.len();
        endlen = length.saturating_sub(1);
        let mut done = false;
        while !done && endlen > 0 {
            end = first.get(length - endlen..).unwrap_or_default().to_vec();
            done = subalts.iter().all(|s| s.ends_with(&end));
            if !done {
                endlen -= 1;
            }
        }
        if endlen > 0 {
            for subalt in &mut subalts {
                subalt.truncate(subalt.len() - endlen);
            }
        }
    }
    regexp.push_str(&build_opt_alt(&subalts));
    if endlen > 0 {
        regexp.push_str(&reoptalt_escape(&end));
    }
}

/// `REOPTALT.buildOptAlt`.
fn build_opt_alt(alternatives: &[Vec<char>]) -> String {
    if let [single] = alternatives {
        return reoptalt_escape(single);
    }
    let mut uniques: BTreeSet<&[char]> = alternatives.iter().map(Vec::as_slice).collect();
    let has_empty = uniques.remove(&[][..]);
    let alternatives: Vec<&[char]> = uniques.into_iter().collect();
    let Some(&first) = alternatives.first() else {
        return String::new();
    };
    let len = reoptalt_prefix_length(&alternatives);
    let mut regexp = String::from("(");
    if has_empty {
        regexp.push('|');
    }
    let mut prefix = first.get(..len).unwrap_or_default();
    let mut nprefixes = 0;
    let mut suffixes: BTreeSet<&[char]> = BTreeSet::new();
    for alternative in &alternatives {
        if alternative.starts_with(prefix) {
            suffixes.insert(alternative.get(len..).unwrap_or_default());
        } else {
            if nprefixes > 0 {
                regexp.push('|');
            }
            regexp.push_str(&reoptalt_escape(prefix));
            reoptalt_append_suffixes(&mut regexp, &suffixes);
            suffixes.clear();
            suffixes.insert(alternative.get(len..).unwrap_or_default());
            prefix = alternative.get(..len).unwrap_or_default();
            nprefixes += 1;
        }
    }
    if nprefixes > 0 {
        regexp.push('|');
    }
    regexp.push_str(&reoptalt_escape(prefix));
    reoptalt_append_suffixes(&mut regexp, &suffixes);
    regexp.push(')');
    // `(x|y|z)` with single character alternatives becomes `[xyz]` (unless a `-` is among
    // them, which would form a range).
    let chars: Vec<char> = regexp.chars().collect();
    let inner = &chars[1..chars.len() - 1];
    let mut count = (chars.len() as i64 - 3) / 2;
    let mut valid = true;
    for (i, &c) in inner.iter().enumerate() {
        if c == '|' {
            count -= 1;
            if (i + 1) % 2 != 0 {
                valid = false;
                break;
            }
        }
    }
    if valid && count == 0 && !regexp.contains('-') {
        let class: String = inner.iter().filter(|&&c| c != '|').collect();
        regexp = format!("[{class}]");
    }
    regexp
}

// -----------------------------------------------------------------------------------------
// Bit sets
// -----------------------------------------------------------------------------------------

/// Pop a `BITSET`.
fn pop_bitset(stack: &mut Stack, name: &str, message: &str) -> Result<Rc<BitSet>> {
    match stack.pop()? {
        Value::BitSet(set) => Ok(set),
        _ => Err(fail(name, message)),
    }
}

/// `BITGET`: `BITSET index BITGET` → whether the bit is set.
fn bitget(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "BITGET";
    let index = pop_long(stack, NAME, "expects a bit index on top of the stack.")?;
    let set = pop_bitset(stack, NAME, "expects a bit set below the bit index.")?;
    stack.push(set.get(index as i32)?);
    Ok(())
}

/// `BITCOUNT`: `BITSET BITCOUNT` → the length (highest set bit plus one) then the number of
/// set bits (both as `LONG`).
fn bitcount(stack: &mut Stack) -> Result<()> {
    let set = pop_bitset(stack, "BITCOUNT", "expects a bit set below the bit index.")?;
    stack.push(set.length());
    stack.push(set.cardinality());
    Ok(())
}

/// `BITSTOBYTES`: `BitSet.toByteArray()`.
fn bitstobytes(stack: &mut Stack) -> Result<()> {
    let set = pop_bitset(
        stack,
        "BITSTOBYTES",
        "expects a bit set on top of the stack.",
    )?;
    stack.push(Value::bytes(set.to_bytes()));
    Ok(())
}

/// `BYTESTOBITS`: `BitSet.valueOf(byte[])`.
fn bytestobits(stack: &mut Stack) -> Result<()> {
    let Value::Bytes(bytes) = stack.pop()? else {
        return Err(fail(
            "BYTESTOBITS",
            "expects a byte array on top of the stack.",
        ));
    };
    stack.push(Value::BitSet(Rc::new(BitSet::from_bytes(&bytes))));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// ->LONGBYTES, varints, RLP
// -----------------------------------------------------------------------------------------

/// `->LONGBYTES`: `LONG|LIST n ->LONGBYTES` → the `n` low-order bytes (big endian) of each
/// `LONG`.
fn tolongbytes(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "->LONGBYTES";
    const MESSAGE: &str = "operates on a LONG or a list of LONG and expects a number of output bytes per LONG between 1 and 8 on top of the stack.";
    let Value::Long(nb) = stack.pop()? else {
        return Err(fail(NAME, MESSAGE));
    };
    let nb = nb as i32;
    let value = stack.pop()?;
    if !(1..=8).contains(&nb) {
        return Err(fail(NAME, MESSAGE));
    }
    let skip = 8 - nb as usize;
    match value {
        Value::Long(l) => {
            stack.push(Value::bytes(&l.to_be_bytes()[skip..]));
        }
        Value::List(list) => {
            let list = list.borrow();
            let mut out = Vec::with_capacity(list.len() * (8 - skip));
            for item in list.iter() {
                let Value::Long(l) = item else {
                    return Err(fail_with_cause(
                        NAME,
                        MESSAGE,
                        Error::new(format!(
                            "Invalid element type {}, only LONG elements are supported.",
                            item.type_name()
                        )),
                    ));
                };
                out.extend_from_slice(&l.to_be_bytes()[skip..]);
            }
            stack.push(Value::bytes(out));
        }
        _ => return Err(fail(NAME, MESSAGE)),
    }
    Ok(())
}

/// `Varint.encodeUnsignedLong`: 7 bits per byte, low bits first, continuation bit 0x80.
fn varint_encode(value: i64, out: &mut Vec<u8>) {
    let mut value = value as u64;
    while value & !0x7F != 0 {
        out.push((value as u8 & 0x7F) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// `Varint.decodeUnsignedLong` from `data` at `pos`, which is advanced past the value.
fn varint_decode(data: &[u8], pos: &mut usize) -> Result<i64> {
    let mut value: u64 = 0;
    let mut shift = 0;
    loop {
        let Some(&byte) = data.get(*pos) else {
            return Err(Error::new("Buffer underflow."));
        };
        *pos += 1;
        if byte & 0x80 == 0 {
            return Ok((value | (u64::from(byte) << shift)) as i64);
        }
        value |= u64::from(byte & 0x7F) << shift;
        shift += 7;
        if shift > 63 {
            return Err(Error::new("Variable length quantity is too long"));
        }
    }
}

/// `->VARINT`: `LONG|LIST ->VARINT` → the concatenated unsigned varint encodings.
fn tovarint(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "->VARINT";
    let mut out = Vec::new();
    match stack.pop()? {
        Value::Long(l) => varint_encode(l, &mut out),
        Value::List(list) => {
            for item in list.borrow().iter() {
                let Value::Long(l) = item else {
                    return Err(fail(NAME, "operates on a LIST of LONGs."));
                };
                varint_encode(*l, &mut out);
            }
        }
        _ => return Err(fail(NAME, "operates on a LONG or LIST of LONGs.")),
    }
    stack.push(Value::bytes(out));
    Ok(())
}

/// `VARINT->`: `BYTES [count] VARINT->` → the list of decoded `LONG`s (and, when a count was
/// given, the number of bytes consumed).
fn varintto(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "VARINT->";
    let mut top = stack.pop()?;
    let mut count = i64::MAX;
    let mut countbased = false;
    if let Value::Long(c) = top {
        count = c;
        countbased = true;
        top = stack.pop()?;
    }
    let Value::Bytes(data) = top else {
        return Err(fail(NAME, OPERATES_ON_BYTES));
    };
    let mut values = Vec::new();
    let mut pos = 0;
    while pos < data.len() && count > 0 {
        let value = varint_decode(&data, &mut pos)
            .map_err(|cause| fail_with_cause(NAME, "error while decoding values.", cause))?;
        values.push(Value::Long(value));
        count -= 1;
    }
    stack.push(Value::list(values));
    if countbased {
        stack.push(pos as i64);
    }
    Ok(())
}

/// The RLP header of a payload of `len` bytes: `base + len` up to 55 bytes, otherwise
/// `base + 55 + n` followed by the `n` bytes of the big endian length.
fn rlp_header(out: &mut Vec<u8>, base: u8, len: usize) {
    if len <= 55 {
        out.push(base + len as u8);
    } else {
        let be = (len as u64).to_be_bytes();
        let idx = be.iter().position(|&b| b != 0).unwrap_or(7);
        out.push(base + 0x37 + (8 - idx) as u8);
        out.extend_from_slice(&be[idx..]);
    }
}

/// RLP encoding of a byte string.
fn rlp_string(out: &mut Vec<u8>, data: &[u8]) {
    if let [single] = data
        && *single <= 0x7F
    {
        out.push(*single);
    } else {
        rlp_header(out, 0x80, data.len());
        out.extend_from_slice(data);
    }
}

/// `TORLP.encode`: lists, byte arrays, strings (UTF-8) and longs (big endian without leading
/// zero bytes, 8 bytes when negative).
fn rlp_encode(value: &Value, out: &mut Vec<u8>) -> Result<()> {
    match value {
        Value::List(list) => {
            let mut payload = Vec::new();
            for item in list.borrow().iter() {
                rlp_encode(item, &mut payload)?;
            }
            rlp_header(out, 0xC0, payload.len());
            out.extend_from_slice(&payload);
        }
        Value::Bytes(bytes) => rlp_string(out, bytes),
        Value::String(s) => rlp_string(out, s.as_bytes()),
        Value::Long(l) => {
            let be = l.to_be_bytes();
            let data = match l.signum() {
                1 => {
                    let idx = be.iter().position(|&b| b != 0).unwrap_or(8);
                    &be[idx..]
                }
                0 => &be[8..],
                _ => &be[..],
            };
            rlp_string(out, data);
        }
        _ => {
            return Err(Error::new(
                "Invalid item type, only LIST, LONG, BYTES and STRING are supported.",
            ));
        }
    }
    Ok(())
}

/// `->RLP`: Recursive Length Prefix encoding of a `LIST`, `BYTES`, `STRING` or `LONG`.
fn torlp(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "->RLP";
    let top = stack.pop()?;
    let mut out = Vec::new();
    rlp_encode(&top, &mut out).map_err(|cause| {
        fail_with_cause(NAME, "encountered an error while encoding RLP.", cause)
    })?;
    stack.push(Value::bytes(out));
    Ok(())
}

/// The byte at `index`, or the Java `ArrayIndexOutOfBoundsException` message.
fn rlp_byte(data: &[u8], index: usize) -> Result<u8> {
    data.get(index).copied().ok_or_else(|| {
        Error::new(format!(
            "Index {index} out of bounds for length {}",
            data.len()
        ))
    })
}

/// Read a big endian size of `sizebytes` bytes at `offset`.
fn rlp_size(data: &[u8], offset: &mut usize, sizebytes: usize) -> Result<usize> {
    if sizebytes > 4 {
        return Err(Error::new("Unable to support size over 2**32."));
    }
    *offset += 1;
    let mut size = 0usize;
    for _ in 0..sizebytes {
        size = (size << 8) | usize::from(rlp_byte(data, *offset)?);
        *offset += 1;
    }
    Ok(size)
}

/// The `size` bytes of payload at `offset`, which is advanced past them. A truncated payload
/// is an error (Warp 10 would zero-pad it).
fn rlp_payload<'a>(data: &'a [u8], offset: &mut usize, size: usize) -> Result<&'a [u8]> {
    let end = offset.saturating_add(size);
    let payload = data.get(*offset..end).ok_or_else(|| {
        Error::new(format!(
            "Index {} out of bounds for length {}",
            end.saturating_sub(1),
            data.len()
        ))
    })?;
    *offset = end;
    Ok(payload)
}

/// The items of a list payload ending at `end`.
fn rlp_list(data: &[u8], offset: &mut usize, end: usize) -> Result<Value> {
    let mut items = Vec::new();
    while *offset < end {
        items.push(rlp_decode(data, offset)?);
    }
    Ok(Value::list(items))
}

/// `RLPTO.decode`: one RLP item at `offset` (strings become `BYTES`, lists `LIST`).
fn rlp_decode(data: &[u8], offset: &mut usize) -> Result<Value> {
    let first = rlp_byte(data, *offset)?;
    if first > 0xF7 {
        let size = rlp_size(data, offset, usize::from(first - 0xF7))?;
        let end = offset.saturating_add(size);
        rlp_list(data, offset, end)
    } else if first >= 0xC0 {
        *offset += 1;
        let end = *offset + usize::from(first - 0xC0);
        rlp_list(data, offset, end)
    } else if first > 0xB7 {
        let size = rlp_size(data, offset, usize::from(first - 0xB7))?;
        Ok(Value::bytes(rlp_payload(data, offset, size)?))
    } else if first >= 0x80 {
        *offset += 1;
        Ok(Value::bytes(rlp_payload(
            data,
            offset,
            usize::from(first - 0x80),
        )?))
    } else {
        *offset += 1;
        Ok(Value::bytes([first]))
    }
}

/// `RLP->`: decode the first RLP item of a byte array (trailing bytes are ignored).
fn rlpto(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "RLP->";
    let Value::Bytes(data) = stack.pop()? else {
        return Err(fail(NAME, OPERATES_ON_BYTES));
    };
    let mut offset = 0;
    let value = rlp_decode(&data, &mut offset).map_err(|cause| {
        fail_with_cause(NAME, "encountered an error while decoding RLP.", cause)
    })?;
    stack.push(value);
    Ok(())
}
