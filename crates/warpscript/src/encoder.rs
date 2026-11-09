// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Warp 10 binary encoding of datapoints: `GTSEncoder` / `GTSDecoder` (the format of
//! `WRAP`ped series and of `GTSENCODER` values), the `Varint` helpers they rely on, the
//! `BigDecimal` values of optimized doubles, and the `GTSWrapperHelper` conversions between
//! series, encoders and `GTSWrapper` structures.
//!
//! The encoder and the decoder are faithful ports, including the quirks of the format (the
//! state of the decoder that `getEncoder` captures, the deltas `merge` keeps) and the errors a
//! malformed payload raises ([`EncoderError`]).

use std::fmt;
use std::rc::Rc;

use crate::error::{Error, IoFailure, Result, compat};
use crate::format::{decimal_value, java_double_to_string};
use crate::functions::compression::{gzip_vec, ungzip_bytes};
use crate::functions::strings::decode_utf8_java;
use crate::gts::{GeoTimeSerie, Metadata, NO_ELEVATION, NO_LOCATION, Type, Values};
use crate::thrift::GtsWrapper;
use crate::value::{Value, ValueKind};

const FLAGS_CONTINUATION: u8 = 0x80;
const FLAGS_MASK_TIMESTAMP: u8 = 0x60;
const FLAGS_MASK_TYPE: u8 = 0x18;
const FLAGS_MASK_TYPE_FLAGS: u8 = 0x07;
const FLAGS_ENCRYPTED: u8 = 0x00;
const FLAGS_TIMESTAMP_ZIGZAG_DELTA_PREVIOUS: u8 = 0x00;
const FLAGS_TIMESTAMP_EQUALS_BASE: u8 = 0x20;
const FLAGS_TIMESTAMP_ZIGZAG_DELTA_BASE: u8 = 0x40;
const FLAGS_TIMESTAMP_RAW_ABSOLUTE: u8 = 0x60;
const FLAGS_TYPE_BOOLEAN: u8 = 0x00;
const FLAGS_TYPE_LONG: u8 = 0x08;
const FLAGS_TYPE_DOUBLE: u8 = 0x10;
const FLAGS_TYPE_STRING: u8 = 0x18;
const FLAGS_STRING_BINARY: u8 = 0x02;
const FLAGS_BOOLEAN_VALUE_TRUE: u8 = 0x04;
const FLAGS_BOOLEAN_VALUE_FALSE: u8 = 0x02;
const FLAGS_DELETE_MARKER: u8 = 0x07;
const FLAGS_LONG_ZIGZAG: u8 = 0x04;
const FLAGS_LONG_DELTA_PREVIOUS: u8 = 0x02;
const FLAGS_DOUBLE_IEEE754: u8 = 0x04;
const FLAGS_VALUE_IDENTICAL: u8 = 0x01;
const FLAGS_LOCATION: u8 = 0x40;
const FLAGS_LOCATION_GEOXPPOINT_ZIGZAG_DELTA: u8 = 0x20;
const FLAGS_LOCATION_IDENTICAL: u8 = 0x10;
const FLAGS_ELEVATION: u8 = 0x08;
const FLAGS_ELEVATION_ZIGZAG: u8 = 0x04;
const FLAGS_ELEVATION_DELTA_PREVIOUS: u8 = 0x02;
const FLAGS_ELEVATION_IDENTICAL: u8 = 0x01;

/// Deltas and values at least this large (in absolute value) are not zig-zag encoded.
const ZIGZAG_LIMIT: i64 = 1 << 48;

/// `GTSWrapperHelper.DEFAULT_COMP_RATIO_THRESHOLD`.
pub const DEFAULT_COMP_RATIO_THRESHOLD: f64 = 100.0;

// -----------------------------------------------------------------------------------------
// BigDecimal
// -----------------------------------------------------------------------------------------

/// The `java.math.BigDecimal` values the encoding stores: `unscaled × 10^-scale`, with an
/// unscaled value that always fits a `long` here (those of the format do).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decimal {
    /// The unscaled value.
    pub unscaled: i64,
    /// The scale (number of fractional digits, negative for multiples of powers of ten).
    pub scale: i32,
}

impl Decimal {
    /// `BigDecimal.stripTrailingZeros`: zero is `0` with scale 0.
    #[must_use]
    pub fn strip(self) -> Decimal {
        if self.unscaled == 0 {
            return Decimal {
                unscaled: 0,
                scale: 0,
            };
        }
        let mut out = self;
        while out.unscaled % 10 == 0 {
            out.unscaled /= 10;
            out.scale = out.scale.wrapping_sub(1);
        }
        out
    }

    /// `BigDecimal.valueOf(d)` (`new BigDecimal(Double.toString(d))`) of a finite double,
    /// stripped of its trailing zeros as the encoder does.
    pub fn from_double(d: f64) -> Decimal {
        let text = java_double_to_string(d);
        let (mantissa, exponent) = match text.split_once('E') {
            Some((mantissa, exponent)) => (mantissa, decimal_value(exponent) as i32),
            None => (text.as_str(), 0),
        };
        let (negative, mantissa) = match mantissa.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, mantissa),
        };
        let mut unscaled = 0i64;
        let mut fraction_digits = 0i32;
        let mut in_fraction = false;
        for c in mantissa.chars() {
            if c == '.' {
                in_fraction = true;
                continue;
            }
            unscaled = unscaled
                .wrapping_mul(10)
                .wrapping_add(i64::from(c.to_digit(10).unwrap_or_default()));
            if in_fraction {
                fraction_digits += 1;
            }
        }
        if negative {
            unscaled = -unscaled;
        }
        Decimal {
            unscaled,
            scale: fraction_digits - exponent,
        }
        .strip()
    }

    /// `BigDecimal.doubleValue`, correctly rounded.
    pub fn double_value(&self) -> f64 {
        format!("{}E{}", self.unscaled, -i64::from(self.scale))
            .parse()
            .unwrap_or_default()
    }

    /// `BigDecimal.longValue`: the integer part, truncated to its low 64 bits.
    pub fn long_value(&self) -> i64 {
        if self.scale <= 0 {
            let mut value = self.unscaled;
            for _ in 0..(-i64::from(self.scale)).min(64) {
                value = value.wrapping_mul(10);
            }
            value
        } else if self.scale >= 19 {
            0
        } else {
            self.unscaled / 10i64.pow(self.scale.unsigned_abs())
        }
    }

    /// `unscaledValue().bitLength()`.
    pub fn bit_length(&self) -> u32 {
        let magnitude = if self.unscaled < 0 {
            !self.unscaled
        } else {
            self.unscaled
        };
        64 - magnitude.leading_zeros()
    }

    /// `BigDecimal.compareTo`, numerically.
    fn same_number(self, other: Decimal) -> bool {
        self.strip() == other.strip()
    }
}

impl fmt::Display for Decimal {
    /// `BigDecimal.toString`: plain notation when the scale is non negative and the adjusted
    /// exponent at least -6, scientific notation (`d.dddE±n`) otherwise.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.scale == 0 {
            return write!(f, "{}", self.unscaled);
        }
        let digits = self.unscaled.unsigned_abs().to_string();
        let sign = if self.unscaled < 0 { "-" } else { "" };
        let len = digits.len() as i64;
        let scale = i64::from(self.scale);
        let adjusted = len - 1 - scale;
        if scale > 0 && adjusted >= -6 {
            if scale >= len {
                let zeros = "0".repeat((scale - len) as usize);
                write!(f, "{sign}0.{zeros}{digits}")
            } else {
                let (int, frac) = digits.split_at((len - scale) as usize);
                write!(f, "{sign}{int}.{frac}")
            }
        } else {
            let (first, rest) = digits.split_at(1);
            let point = if rest.is_empty() { "" } else { "." };
            // The adjusted exponent is never 0 here (Java omits the exponent then).
            let plus = if adjusted > 0 { "+" } else { "" };
            write!(f, "{sign}{first}{point}{rest}E{plus}{adjusted}")
        }
    }
}

// -----------------------------------------------------------------------------------------
// Values
// -----------------------------------------------------------------------------------------

/// A failure of the binary encoding or decoding of datapoints.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EncoderError {
    /// A value of a kind the encoder cannot hold.
    #[error("Unsuported value type 'class {}'", compat::value_class(*.0))]
    UnsupportedValue(ValueKind),
    /// A read past the end of the payload.
    #[error("BufferUnderflowException")]
    BufferUnderflow,
    /// A variable length quantity of more than 9 continuation bytes.
    #[error("Variable length quantity is too long")]
    VarintTooLong,
    /// A skip to before the start of the payload.
    #[error("newPosition < 0: ({target} < 0)")]
    PositionBeforeStart {
        /// The position skipped to.
        target: i32,
    },
    /// A skip to after the end of the payload.
    #[error("newPosition > limit: ({target} > {limit})")]
    PositionAfterLimit {
        /// The position skipped to.
        target: i32,
        /// The length of the payload.
        limit: usize,
    },
    /// A string longer than what is left of the payload.
    #[error("Invalid string length.")]
    InvalidStringLength,
    /// A string whose length overflows an `int`.
    #[error("{0}")]
    NegativeStringLength(i32),
    /// A boolean that is neither 0 nor 1.
    #[error("Invalid boolean value.")]
    InvalidBoolean,
    /// A compressed payload that does not inflate.
    #[error("Invalid compressed content.")]
    InvalidCompressedContent(#[source] IoFailure),
    /// An encoder asked of a decoder that has not read a datapoint yet.
    #[error("Can only get an encoder for a decoder on which 'next' was called at least once.")]
    DecoderNotStarted,
}

impl EncoderError {
    /// `true` for the failures Warp 10 raises as runtime exceptions, `false` for the
    /// `IOException` of a decoder that was not started.
    pub fn is_runtime(&self) -> bool {
        !matches!(self, EncoderError::DecoderNotStarted)
    }
}

/// A value an encoder stores (`addValue`) or a decoder returns (`getBinaryValue`).
#[derive(Clone, Debug, PartialEq)]
pub enum EncodedValue {
    /// `null`: a delete marker.
    Null,
    /// A boolean.
    Boolean(bool),
    /// A long.
    Long(i64),
    /// A double.
    Double(f64),
    /// An optimized double (`BigDecimal`).
    Decimal(Decimal),
    /// A string.
    String(Rc<str>),
    /// A byte array.
    Bytes(Rc<[u8]>),
}

impl EncodedValue {
    /// The value at `idx` of a series, which holds only the types an encoder holds.
    pub fn of_series(gts: &GeoTimeSerie, idx: usize) -> EncodedValue {
        match &gts.values {
            Values::Long(values) => values
                .get(idx)
                .map_or(EncodedValue::Null, |value| EncodedValue::Long(*value)),
            Values::Double(values) => values
                .get(idx)
                .map_or(EncodedValue::Null, |value| EncodedValue::Double(*value)),
            Values::Boolean(values) => values
                .get(idx)
                .map_or(EncodedValue::Null, |value| EncodedValue::Boolean(*value)),
            Values::String(values) => values.get(idx).map_or(EncodedValue::Null, |value| {
                EncodedValue::String(Rc::clone(value))
            }),
            Values::Undefined => EncodedValue::Null,
        }
    }

    /// The value of a stack object, failing like `addValue` for the unsupported types.
    pub fn from_value(value: &Value) -> Result<EncodedValue> {
        Ok(match value {
            Value::Null => EncodedValue::Null,
            Value::Boolean(b) => EncodedValue::Boolean(*b),
            Value::Long(l) => EncodedValue::Long(*l),
            Value::Double(d) => EncodedValue::Double(*d),
            Value::BigDecimal(bd) => EncodedValue::from_big_decimal(bd),
            Value::String(s) => EncodedValue::String(Rc::clone(s)),
            Value::Bytes(b) => EncodedValue::Bytes(Rc::clone(b)),
            other => {
                return Err(EncoderError::UnsupportedValue(other.kind()).into());
            }
        })
    }

    /// `addValue(BigDecimal)`: the value stripped of its trailing zeros, kept as a decimal when
    /// its scale fits a byte and its unscaled value 46 bits, an IEEE 754 double otherwise.
    pub fn from_big_decimal(bd: &crate::bigdecimal::BigDecimal) -> EncodedValue {
        use num_traits::ToPrimitive;
        let stripped = bd.strip_trailing_zeros();
        let fits = (-128..=127).contains(&stripped.scale)
            && crate::bigdecimal::bit_length(&stripped.unscaled) <= 46;
        match stripped.unscaled.to_i64() {
            Some(unscaled) if fits => EncodedValue::Decimal(Decimal {
                unscaled,
                scale: stripped.scale,
            }),
            _ => EncodedValue::Double(bd.double_value()),
        }
    }

    /// The stack object of the value: a `BigDecimal` is pushed as a double, as WarpScript
    /// does for every value it gets out of an encoder.
    pub fn to_value(&self) -> Value {
        match self {
            EncodedValue::Null => Value::Null,
            EncodedValue::Boolean(b) => Value::Boolean(*b),
            EncodedValue::Long(l) => Value::Long(*l),
            EncodedValue::Double(d) => Value::Double(*d),
            EncodedValue::Decimal(d) => Value::Double(d.double_value()),
            EncodedValue::String(s) => Value::String(Rc::clone(s)),
            EncodedValue::Bytes(b) => Value::Bytes(Rc::clone(b)),
        }
    }
}

/// `new String(bytes, ISO_8859_1)`.
fn latin1_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| char::from(*b)).collect()
}

/// `String.getBytes(ISO_8859_1)`: unmappable characters become `?`.
pub fn latin1_bytes(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect()
}

// -----------------------------------------------------------------------------------------
// Varint
// -----------------------------------------------------------------------------------------

/// `Varint.encodeUnsignedLongInBuf`.
fn put_unsigned(out: &mut Vec<u8>, value: i64) {
    let mut value = value as u64;
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// `Varint.encodeSignedLongInBuf`: zig-zag then unsigned.
fn put_signed(out: &mut Vec<u8>, value: i64) {
    put_unsigned(out, (value << 1) ^ (value >> 63));
}

/// An 8-byte big-endian long.
fn put_raw(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_be_bytes());
}

/// A `ByteBuffer` over the payload of a decoder.
#[derive(Clone, Debug)]
struct Buffer {
    bytes: Rc<[u8]>,
    pos: usize,
}

impl Buffer {
    fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }

    /// The next byte, which the caller checked is there.
    fn take(&mut self) -> u8 {
        let byte = self.bytes.get(self.pos).copied().unwrap_or_default();
        self.pos += 1;
        byte
    }

    fn get(&mut self) -> Result<u8> {
        let byte = *self
            .bytes
            .get(self.pos)
            .ok_or(EncoderError::BufferUnderflow)?;
        self.pos += 1;
        Ok(byte)
    }

    fn get_long(&mut self) -> Result<i64> {
        if self.remaining() < 8 {
            return Err(EncoderError::BufferUnderflow.into());
        }
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&self.bytes[self.pos..self.pos + 8]);
        self.pos += 8;
        Ok(i64::from_be_bytes(raw))
    }

    /// `Varint.decodeUnsignedLong`: at most 9 continuation bytes, shifts taken modulo 64.
    fn unsigned(&mut self) -> Result<i64> {
        let mut value = 0i64;
        let mut shift = 0u32;
        loop {
            let byte = self.get()?;
            if byte & 0x80 == 0 {
                return Ok(value | (i64::from(byte) << (shift % 64)));
            }
            value |= i64::from(byte & 0x7f) << (shift % 64);
            shift += 7;
            if shift > 63 {
                return Err(EncoderError::VarintTooLong.into());
            }
        }
    }

    /// `Varint.decodeSignedLong`.
    fn signed(&mut self) -> Result<i64> {
        let unsigned = self.unsigned()? as u64;
        Ok(((unsigned >> 1) as i64) ^ -((unsigned & 1) as i64))
    }

    /// `Buffer.position(position + delta)` with Java's `int` arithmetic.
    fn skip(&mut self, delta: i32) -> Result<()> {
        let target = (self.pos as i32).wrapping_add(delta);
        let limit = self.bytes.len();
        if target < 0 {
            return Err(EncoderError::PositionBeforeStart { target }.into());
        }
        if target as usize > limit {
            return Err(EncoderError::PositionAfterLimit { target, limit }.into());
        }
        self.pos = target as usize;
        Ok(())
    }
}

// -----------------------------------------------------------------------------------------
// Encoder
// -----------------------------------------------------------------------------------------

/// The state a decoder starts from (`GTSEncoder.initialize` / `GTSDecoder.initialize`).
#[derive(Clone, Debug)]
struct Initial {
    timestamp: i64,
    location: i64,
    elevation: i64,
    long: i64,
    double: f64,
    decimal: Option<Decimal>,
    string: Option<Rc<str>>,
}

impl Default for Initial {
    fn default() -> Self {
        Initial {
            timestamp: 0,
            location: NO_LOCATION,
            elevation: NO_ELEVATION,
            long: i64::MAX,
            double: f64::NAN,
            decimal: None,
            string: None,
        }
    }
}

/// `GTSEncoder`: datapoints appended to a byte stream, each encoded relatively to the base
/// timestamp or to the previous datapoint.
#[derive(Clone, Debug)]
pub struct GtsEncoder {
    base: i64,
    last: Initial,
    initial: Initial,
    valid_location: bool,
    valid_elevation: bool,
    valid_long: bool,
    valid_double: bool,
    valid_decimal: bool,
    valid_string: bool,
    no_delta_meta_timestamp: bool,
    stream: Vec<u8>,
    /// The metadata of the encoded series.
    pub metadata: Metadata,
    count: i64,
}

impl GtsEncoder {
    /// `new GTSEncoder(baseTimestamp)`.
    pub fn new(base: i64) -> Self {
        GtsEncoder {
            base,
            last: Initial::default(),
            initial: Initial::default(),
            valid_location: false,
            valid_elevation: false,
            valid_long: false,
            valid_double: false,
            valid_decimal: false,
            valid_string: false,
            no_delta_meta_timestamp: false,
            stream: Vec::new(),
            metadata: Metadata::default(),
            count: 0,
        }
    }

    /// The base timestamp.
    pub fn base(&self) -> i64 {
        self.base
    }

    /// The number of datapoints (`getCount`).
    pub fn count(&self) -> i64 {
        self.count
    }

    /// The encoded bytes (`getBytes`).
    pub fn bytes(&self) -> &[u8] {
        &self.stream
    }

    /// The size of the encoded bytes (`size`).
    pub fn size(&self) -> usize {
        self.stream.len()
    }

    /// `safeDelta`: the next datapoint is encoded without reference to the previous ones.
    fn safe_delta(&mut self) {
        self.no_delta_meta_timestamp = true;
        self.valid_elevation = false;
        self.valid_location = false;
        self.valid_long = false;
        self.valid_double = false;
        self.valid_decimal = false;
        self.valid_string = false;
    }

    /// `addValue`.
    #[allow(clippy::too_many_lines)]
    pub fn add_value(
        &mut self,
        timestamp: i64,
        location: i64,
        elevation: i64,
        value: &EncodedValue,
    ) {
        let mut flags = if self.no_delta_meta_timestamp {
            self.no_delta_meta_timestamp = false;
            FLAGS_TIMESTAMP_RAW_ABSOLUTE
        } else if self.base == timestamp {
            FLAGS_TIMESTAMP_EQUALS_BASE
        } else {
            let delta_base = timestamp.wrapping_sub(self.base).wrapping_abs();
            let delta_last = timestamp.wrapping_sub(self.last.timestamp).wrapping_abs();
            if self.last.timestamp != 0 && delta_base >= delta_last {
                if delta_last < ZIGZAG_LIMIT {
                    FLAGS_TIMESTAMP_ZIGZAG_DELTA_PREVIOUS
                } else {
                    FLAGS_TIMESTAMP_RAW_ABSOLUTE
                }
            } else if delta_base < ZIGZAG_LIMIT {
                FLAGS_TIMESTAMP_ZIGZAG_DELTA_BASE
            } else {
                FLAGS_TIMESTAMP_RAW_ABSOLUTE
            }
        };
        match value {
            EncodedValue::Long(l) => {
                flags |= FLAGS_TYPE_LONG;
                if self.valid_long && *l == self.last.long {
                    flags |= FLAGS_VALUE_IDENTICAL;
                } else {
                    let offset = l.wrapping_sub(self.last.long).wrapping_abs();
                    if self.valid_long && offset < l.wrapping_abs() && offset < ZIGZAG_LIMIT {
                        flags |= FLAGS_LONG_DELTA_PREVIOUS | FLAGS_LONG_ZIGZAG;
                    } else if l.wrapping_abs() < ZIGZAG_LIMIT {
                        flags |= FLAGS_LONG_ZIGZAG;
                    }
                }
            }
            EncodedValue::Boolean(b) => {
                flags |= if *b {
                    FLAGS_BOOLEAN_VALUE_TRUE
                } else {
                    FLAGS_BOOLEAN_VALUE_FALSE
                };
            }
            EncodedValue::String(s) => {
                flags |= FLAGS_TYPE_STRING;
                if self.valid_string && self.last.string.as_deref() == Some(&**s) {
                    flags |= FLAGS_VALUE_IDENTICAL;
                }
            }
            EncodedValue::Bytes(b) => {
                flags |= FLAGS_TYPE_STRING | FLAGS_STRING_BINARY;
                if self.valid_string
                    && self.last.string.as_deref() == Some(latin1_string(b).as_str())
                {
                    flags |= FLAGS_VALUE_IDENTICAL;
                }
            }
            EncodedValue::Double(d) => {
                flags |= FLAGS_TYPE_DOUBLE;
                #[allow(clippy::float_cmp)]
                let identical = *d == self.last.double || (d.is_nan() && self.last.double.is_nan());
                if self.valid_double && identical {
                    flags |= FLAGS_VALUE_IDENTICAL;
                } else {
                    flags |= FLAGS_DOUBLE_IEEE754;
                }
            }
            EncodedValue::Decimal(d) => {
                flags |= FLAGS_TYPE_DOUBLE;
                let d = d.strip();
                if self.valid_decimal && self.last.decimal.is_some_and(|last| last.same_number(d)) {
                    flags |= FLAGS_VALUE_IDENTICAL;
                } else if !(-128..=127).contains(&d.scale) || d.bit_length() > 46 {
                    flags |= FLAGS_DOUBLE_IEEE754;
                }
            }
            EncodedValue::Null => flags |= FLAGS_TYPE_BOOLEAN | FLAGS_DELETE_MARKER,
        }
        let present = *value != EncodedValue::Null;
        let mut loc_flags = 0u8;
        if location != NO_LOCATION && present {
            flags |= FLAGS_CONTINUATION;
            loc_flags |= FLAGS_LOCATION;
            if self.valid_location {
                if self.last.location == location {
                    loc_flags |= FLAGS_LOCATION_IDENTICAL;
                } else if location.wrapping_sub(self.last.location).wrapping_abs() < ZIGZAG_LIMIT {
                    loc_flags |= FLAGS_LOCATION_GEOXPPOINT_ZIGZAG_DELTA;
                }
            }
        } else {
            self.valid_location = false;
        }
        if elevation != NO_ELEVATION && present {
            flags |= FLAGS_CONTINUATION;
            loc_flags |= FLAGS_ELEVATION;
            if self.valid_elevation && self.last.elevation == elevation {
                loc_flags |= FLAGS_ELEVATION_IDENTICAL;
            } else if self.valid_elevation
                && elevation.wrapping_sub(self.last.elevation).wrapping_abs() < ZIGZAG_LIMIT
            {
                loc_flags |= FLAGS_ELEVATION_DELTA_PREVIOUS | FLAGS_ELEVATION_ZIGZAG;
            } else if elevation.wrapping_abs() < ZIGZAG_LIMIT {
                loc_flags |= FLAGS_ELEVATION_ZIGZAG;
            }
        } else {
            self.valid_elevation = false;
        }
        self.stream.push(flags);
        if flags & FLAGS_CONTINUATION != 0 {
            self.stream.push(loc_flags);
        }
        match flags & FLAGS_MASK_TIMESTAMP {
            FLAGS_TIMESTAMP_RAW_ABSOLUTE => put_raw(&mut self.stream, timestamp),
            FLAGS_TIMESTAMP_ZIGZAG_DELTA_BASE => {
                put_signed(&mut self.stream, timestamp.wrapping_sub(self.base));
            }
            FLAGS_TIMESTAMP_ZIGZAG_DELTA_PREVIOUS => {
                put_signed(
                    &mut self.stream,
                    timestamp.wrapping_sub(self.last.timestamp),
                );
            }
            _ => {}
        }
        self.last.timestamp = timestamp;
        if loc_flags & FLAGS_LOCATION != 0 {
            if loc_flags & FLAGS_LOCATION_IDENTICAL == 0 {
                if loc_flags & FLAGS_LOCATION_GEOXPPOINT_ZIGZAG_DELTA == 0 {
                    put_raw(&mut self.stream, location);
                } else {
                    put_signed(&mut self.stream, location.wrapping_sub(self.last.location));
                }
            }
            self.last.location = location;
            self.valid_location = true;
        }
        if loc_flags & FLAGS_ELEVATION != 0 {
            if loc_flags & FLAGS_ELEVATION_IDENTICAL == 0 {
                let encoded = if loc_flags & FLAGS_ELEVATION_DELTA_PREVIOUS == 0 {
                    elevation
                } else {
                    elevation.wrapping_sub(self.last.elevation)
                };
                if loc_flags & FLAGS_ELEVATION_ZIGZAG == 0 {
                    put_raw(&mut self.stream, encoded);
                } else {
                    put_signed(&mut self.stream, encoded);
                }
            }
            self.last.elevation = elevation;
            self.valid_elevation = true;
        }
        if flags & FLAGS_VALUE_IDENTICAL == 0 {
            self.write_value(flags, value);
        }
        self.count += 1;
    }

    /// The value part of a datapoint that is not identical to the previous value.
    fn write_value(&mut self, flags: u8, value: &EncodedValue) {
        match value {
            EncodedValue::String(s) => {
                put_unsigned(&mut self.stream, s.len() as i64);
                self.stream.extend_from_slice(s.as_bytes());
                self.last.string = Some(Rc::clone(s));
                self.valid_string = true;
            }
            EncodedValue::Bytes(b) => {
                put_unsigned(&mut self.stream, b.len() as i64);
                self.stream.extend_from_slice(b);
                self.last.string = Some(Rc::from(latin1_string(b)));
                self.valid_string = true;
            }
            EncodedValue::Long(l) => {
                let encoded = if flags & FLAGS_LONG_DELTA_PREVIOUS == 0 {
                    *l
                } else {
                    l.wrapping_sub(self.last.long)
                };
                if flags & FLAGS_LONG_ZIGZAG == 0 {
                    put_raw(&mut self.stream, encoded);
                } else {
                    put_signed(&mut self.stream, encoded);
                }
                self.last.long = *l;
                self.valid_long = true;
            }
            EncodedValue::Double(d) => self.write_ieee754(*d),
            EncodedValue::Decimal(d) => {
                let d = d.strip();
                if flags & FLAGS_DOUBLE_IEEE754 == 0 {
                    self.stream.push(d.scale as u8);
                    put_signed(&mut self.stream, d.unscaled);
                    self.last.decimal = Some(d);
                    self.valid_decimal = true;
                    self.valid_double = false;
                } else {
                    self.write_ieee754(d.double_value());
                }
            }
            EncodedValue::Boolean(_) | EncodedValue::Null => {}
        }
    }

    fn write_ieee754(&mut self, d: f64) {
        self.last.double = d;
        self.stream.extend_from_slice(&d.to_bits().to_be_bytes());
        self.last.decimal = None;
        self.valid_double = true;
        self.valid_decimal = false;
    }

    /// `encode`: every datapoint of `gts`.
    pub fn encode(&mut self, gts: &GeoTimeSerie) {
        self.encode_with(gts, false);
    }

    /// `encodeOptimized`: every datapoint of `gts`, finite doubles as `BigDecimal`s.
    pub fn encode_optimized(&mut self, gts: &GeoTimeSerie) {
        self.encode_with(gts, true);
    }

    fn encode_with(&mut self, gts: &GeoTimeSerie, optimized: bool) {
        for idx in 0..gts.size() {
            let value = match gts.value_at(idx) {
                Value::Double(d) if optimized && d.is_finite() => {
                    EncodedValue::Decimal(Decimal::from_double(d))
                }
                _ => EncodedValue::of_series(gts, idx),
            };
            self.add_value(
                gts.tick_at(idx),
                gts.location_at(idx),
                gts.elevation_at(idx),
                &value,
            );
        }
    }

    /// `getDecoder`.
    pub fn decoder(&self) -> GtsDecoder {
        let mut decoder = GtsDecoder::new(self.base, Rc::from(self.stream.as_slice()));
        decoder.metadata = self.metadata.clone();
        decoder.initialize(&self.initial);
        decoder.count = self.count;
        decoder
    }

    /// `cloneEmpty`: an empty encoder with the same base and metadata.
    #[must_use]
    pub fn clone_empty(&self) -> GtsEncoder {
        let mut encoder = GtsEncoder::new(self.base);
        encoder.metadata = self.metadata.clone();
        encoder
    }

    /// `merge`: append the datapoints of `other`.
    pub fn merge(&mut self, other: &GtsEncoder) -> Result<()> {
        if self.stream.is_empty() && self.base == other.base {
            // `reset(encoder)`
            *self = other.clone();
        } else if self.base != other.base {
            let mut decoder = other.decoder();
            while decoder.next()? {
                self.add_value(
                    decoder.timestamp(),
                    decoder.location(),
                    decoder.elevation(),
                    &decoder.binary_value(),
                );
            }
        } else {
            self.stream.extend_from_slice(&other.stream);
            self.last = other.last.clone();
            self.safe_delta();
            self.valid_elevation = other.valid_elevation;
            self.valid_location = other.valid_location;
            self.valid_decimal = other.valid_decimal;
            self.valid_double = other.valid_double;
            self.valid_long = other.valid_long;
            self.valid_string = other.valid_string;
            self.count = self.count.wrapping_add(other.count);
        }
        Ok(())
    }

    /// `new GTSEncoder(base, key, content)`-like: an encoder over raw `content`, whose next
    /// datapoint is encoded without reference to the content.
    fn with_content(base: i64, content: &[u8], initial: Initial) -> GtsEncoder {
        let mut encoder = GtsEncoder::new(base);
        encoder.initial = initial;
        encoder.stream = content.to_vec();
        encoder.safe_delta();
        encoder
    }

    /// An encoder whose stream is `stream`, whatever it holds: for the tests of corrupt content.
    #[cfg(test)]
    pub(crate) fn with_stream(stream: &[u8]) -> GtsEncoder {
        GtsEncoder::with_content(0, stream, Initial::default())
    }

    /// The encoded datapoints, for the tests of corrupt content.
    #[cfg(test)]
    pub(crate) fn stream_bytes(&self) -> &[u8] {
        &self.stream
    }

    /// `setCount`.
    pub fn set_count(&mut self, count: i64) {
        self.count = count;
    }
}

// -----------------------------------------------------------------------------------------
// Decoder
// -----------------------------------------------------------------------------------------

/// The type of the last decoded value (`lastType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LastType {
    Undefined,
    Boolean,
    Long,
    Double,
    String,
}

/// `GTSDecoder`: the datapoints of an encoded stream, one `next` call at a time.
#[derive(Clone, Debug)]
pub struct GtsDecoder {
    base: i64,
    buffer: Buffer,
    last_type: LastType,
    last: Initial,
    last_boolean: bool,
    last_string_binary: bool,
    previous: Initial,
    next_called: bool,
    consuming_next_calls: i64,
    position: usize,
    /// The metadata of the decoded series.
    pub metadata: Metadata,
    count: i64,
}

impl GtsDecoder {
    /// `new GTSDecoder(baseTimestamp, buffer)`.
    pub fn new(base: i64, bytes: Rc<[u8]>) -> Self {
        GtsDecoder {
            base,
            buffer: Buffer { bytes, pos: 0 },
            last_type: LastType::Undefined,
            last: Initial::default(),
            last_boolean: false,
            last_string_binary: false,
            previous: Initial::default(),
            next_called: false,
            consuming_next_calls: 0,
            position: 0,
            metadata: Metadata::default(),
            count: 0,
        }
    }

    fn initialize(&mut self, initial: &Initial) {
        self.last = initial.clone();
    }

    /// The timestamp of the current datapoint.
    pub fn timestamp(&self) -> i64 {
        self.last.timestamp
    }

    /// The location of the current datapoint.
    pub fn location(&self) -> i64 {
        self.last.location
    }

    /// The elevation of the current datapoint.
    pub fn elevation(&self) -> i64 {
        self.last.elevation
    }

    /// `getValue`: strings of binary values are ISO-8859-1 strings.
    pub fn value(&self) -> EncodedValue {
        match self.last_type {
            LastType::Boolean => EncodedValue::Boolean(self.last_boolean),
            LastType::Long => EncodedValue::Long(self.last.long),
            LastType::Double => self.last.decimal.map_or(
                EncodedValue::Double(self.last.double),
                EncodedValue::Decimal,
            ),
            LastType::String => self
                .last
                .string
                .as_ref()
                .map_or(EncodedValue::Null, |s| EncodedValue::String(Rc::clone(s))),
            LastType::Undefined => EncodedValue::Null,
        }
    }

    /// `getBinaryValue`: binary values as byte arrays.
    pub fn binary_value(&self) -> EncodedValue {
        match self.value() {
            EncodedValue::String(s) if self.last_string_binary => {
                EncodedValue::Bytes(Rc::from(latin1_bytes(&s)))
            }
            other => other,
        }
    }

    /// `next`: decode the next datapoint, `false` at the end of the stream.
    #[allow(clippy::too_many_lines, clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<bool> {
        let flags = loop {
            self.position = self.buffer.pos;
            if self.buffer.remaining() == 0 {
                return Ok(false);
            }
            self.next_called = true;
            let flags = self.buffer.take();
            if flags != FLAGS_ENCRYPTED {
                break flags;
            }
            // An encrypted block: without the key, it is skipped.
            let length = self.buffer.unsigned()? as i32;
            self.buffer.skip(length)?;
        };
        let mut loc_flags = 0u8;
        if flags & FLAGS_CONTINUATION != 0 {
            if self.buffer.remaining() == 0 {
                return Ok(false);
            }
            loc_flags = self.buffer.take();
        }
        let timestamp = match flags & FLAGS_MASK_TIMESTAMP {
            FLAGS_TIMESTAMP_RAW_ABSOLUTE => self.buffer.get_long()?,
            FLAGS_TIMESTAMP_EQUALS_BASE => self.base,
            FLAGS_TIMESTAMP_ZIGZAG_DELTA_BASE => self.base.wrapping_add(self.buffer.signed()?),
            _ => self.last.timestamp.wrapping_add(self.buffer.signed()?),
        };
        self.previous.timestamp = self.last.timestamp;
        self.last.timestamp = timestamp;
        if loc_flags & FLAGS_LOCATION == 0 {
            self.previous.location = self.last.location;
            self.last.location = NO_LOCATION;
        } else if loc_flags & FLAGS_LOCATION_IDENTICAL == 0 {
            let location = if loc_flags & FLAGS_LOCATION_GEOXPPOINT_ZIGZAG_DELTA == 0 {
                self.buffer.get_long()?
            } else {
                self.last.location.wrapping_add(self.buffer.signed()?)
            };
            self.previous.location = self.last.location;
            self.last.location = location;
        }
        if loc_flags & FLAGS_ELEVATION == 0 {
            self.previous.elevation = self.last.elevation;
            self.last.elevation = NO_ELEVATION;
        } else if loc_flags & FLAGS_ELEVATION_IDENTICAL == 0 {
            let encoded = if loc_flags & FLAGS_ELEVATION_ZIGZAG == 0 {
                self.buffer.get_long()?
            } else {
                self.buffer.signed()?
            };
            let elevation = if loc_flags & FLAGS_ELEVATION_DELTA_PREVIOUS == 0 {
                encoded
            } else {
                self.last.elevation.wrapping_add(encoded)
            };
            self.previous.elevation = self.last.elevation;
            self.last.elevation = elevation;
        }
        let identical = flags & FLAGS_VALUE_IDENTICAL != 0;
        match flags & FLAGS_MASK_TYPE {
            FLAGS_TYPE_LONG => {
                self.last_type = LastType::Long;
                let mut value = self.last.long;
                if !identical {
                    let encoded = if flags & FLAGS_LONG_ZIGZAG == 0 {
                        self.buffer.get_long()?
                    } else {
                        self.buffer.signed()?
                    };
                    value = if flags & FLAGS_LONG_DELTA_PREVIOUS == 0 {
                        encoded
                    } else {
                        value.wrapping_add(encoded)
                    };
                }
                self.previous.long = self.last.long;
                self.last.long = value;
            }
            FLAGS_TYPE_DOUBLE => {
                self.last_type = LastType::Double;
                if identical {
                    self.previous.double = self.last.double;
                    self.previous.decimal = self.last.decimal;
                } else if flags & FLAGS_DOUBLE_IEEE754 == 0 {
                    let scale = i32::from(self.buffer.get()? as i8);
                    let unscaled = self.buffer.signed()?;
                    self.previous.decimal = self.last.decimal;
                    self.last.decimal = Some(Decimal { unscaled, scale });
                } else {
                    let bits = self.buffer.get_long()?;
                    self.previous.double = self.last.double;
                    self.last.double = f64::from_bits(bits as u64);
                    self.previous.decimal = self.last.decimal;
                    self.last.decimal = None;
                }
            }
            FLAGS_TYPE_STRING => {
                self.last_type = LastType::String;
                let binary = flags & FLAGS_STRING_BINARY != 0;
                if identical {
                    self.previous.string.clone_from(&self.last.string);
                } else {
                    let len = self.buffer.unsigned()?;
                    if len > self.buffer.remaining() as i64 {
                        return Err(EncoderError::InvalidStringLength.into());
                    }
                    let len = len as i32;
                    if len < 0 {
                        return Err(EncoderError::NegativeStringLength(len).into());
                    }
                    let len = len as usize;
                    if len > self.buffer.remaining() {
                        return Err(EncoderError::BufferUnderflow.into());
                    }
                    let start = self.buffer.pos;
                    self.buffer.pos += len;
                    let bytes = &self.buffer.bytes[start..start + len];
                    let text = if binary {
                        latin1_string(bytes)
                    } else {
                        decode_utf8_java(bytes)
                    };
                    self.previous.string = self.last.string.take();
                    self.last.string = Some(Rc::from(text));
                }
                self.last_string_binary = binary;
            }
            _ => match flags & FLAGS_MASK_TYPE_FLAGS {
                FLAGS_DELETE_MARKER => self.last_type = LastType::Undefined,
                FLAGS_BOOLEAN_VALUE_TRUE => {
                    self.last_type = LastType::Boolean;
                    self.last_boolean = true;
                }
                FLAGS_BOOLEAN_VALUE_FALSE => {
                    self.last_type = LastType::Boolean;
                    self.last_boolean = false;
                }
                _ => return Err(EncoderError::InvalidBoolean.into()),
            },
        }
        self.consuming_next_calls += 1;
        Ok(true)
    }

    /// `getEncoder`: an encoder holding the datapoints from the current one on.
    pub fn encoder(&self) -> Result<GtsEncoder> {
        if !self.next_called {
            return Err(EncoderError::DecoderNotStarted.into());
        }
        let mut encoder = GtsEncoder::with_content(
            self.base,
            &self.buffer.bytes[self.position..],
            self.previous.clone(),
        );
        encoder.metadata = self.metadata.clone();
        encoder.count = self
            .count
            .wrapping_sub(self.consuming_next_calls)
            .wrapping_add(1);
        Ok(encoder)
    }

    /// `decode`: the datapoints as a series, with the metadata of the decoder.
    pub fn decode(&mut self) -> Result<GeoTimeSerie> {
        let mut gts = GeoTimeSerie::new();
        gts.metadata = self.metadata.clone();
        while self.next()? {
            self.set_value_on(&mut gts);
        }
        Ok(gts)
    }

    /// `GTSHelper.setValue(gts, timestamp, location, elevation, getBinaryValue(), false)`:
    /// append the current datapoint to `gts`, converted to its type.
    pub fn set_value_on(&self, gts: &mut GeoTimeSerie) {
        let (tick, location, elevation) = (self.timestamp(), self.location(), self.elevation());
        match self.binary_value() {
            EncodedValue::Decimal(d) => gts.set_decimal(tick, location, elevation, d),
            other => gts.set_value(tick, location, elevation, &other.to_value(), false),
        }
    }
}

/// `GTSHelper.fullsort(GTSEncoder, reversed, baseTimestamp)`: the datapoints split by type
/// (`LONG`, `DOUBLE`, `BOOLEAN`, `STRING`, binary) into series that are fully sorted, then
/// merged by ascending tick (whatever `reversed`) with that type precedence, the doubles
/// optimized.
pub fn fullsort_encoder(encoder: &GtsEncoder, reversed: bool, base: i64) -> Result<GtsEncoder> {
    let mut parts: [GeoTimeSerie; 5] = std::array::from_fn(|_| GeoTimeSerie::new());
    let mut decoder = encoder.decoder();
    while decoder.next()? {
        let (tick, location, elevation) =
            (decoder.timestamp(), decoder.location(), decoder.elevation());
        match decoder.binary_value() {
            EncodedValue::Long(l) => {
                parts[0].set_value(tick, location, elevation, &Value::Long(l), false);
            }
            EncodedValue::Double(d) => {
                parts[1].set_value(tick, location, elevation, &Value::Double(d), false);
            }
            EncodedValue::Decimal(d) => parts[1].set_decimal(tick, location, elevation, d),
            EncodedValue::Boolean(b) => {
                parts[2].set_value(tick, location, elevation, &Value::Boolean(b), false);
            }
            EncodedValue::String(s) => {
                parts[3].set_value(tick, location, elevation, &Value::String(s), false);
            }
            // A decoded datapoint is never `null`.
            other => parts[4].set_value(tick, location, elevation, &other.to_value(), false),
        }
    }
    for part in &mut parts {
        part.fullsort(reversed);
    }
    let mut sorted = GtsEncoder::new(base);
    sorted.metadata = encoder.metadata.clone();
    let mut idx = [0usize; 5];
    loop {
        let mut next: Option<(usize, i64)> = None;
        for (i, part) in parts.iter().enumerate() {
            if idx[i] < part.size() {
                let tick = part.tick_at(idx[i]);
                if next.is_none_or(|(_, ts)| tick < ts) {
                    next = Some((i, tick));
                }
            }
        }
        let Some((i, ts)) = next else {
            break;
        };
        let part = &parts[i];
        loop {
            let k = idx[i];
            let value = match part.value_at(k) {
                Value::String(s) if i == 4 => EncodedValue::Bytes(Rc::from(latin1_bytes(&s))),
                Value::Double(d) if d.is_finite() => EncodedValue::Decimal(Decimal::from_double(d)),
                _ => EncodedValue::of_series(part, k),
            };
            sorted.add_value(ts, part.location_at(k), part.elevation_at(k), &value);
            idx[i] += 1;
            if idx[i] >= part.size() || part.tick_at(idx[i]) != ts {
                break;
            }
        }
    }
    Ok(sorted)
}

// -----------------------------------------------------------------------------------------
// GTSWrapperHelper
// -----------------------------------------------------------------------------------------

/// `GTSWrapperHelper.fromGTSEncoderToGTSWrapper`: gzip the content (repeatedly, while each
/// pass shrinks it more than `compratio`) when `compress` is set.
pub fn encoder_to_wrapper(
    encoder: &GtsEncoder,
    compress: bool,
    compratio: f64,
    set_count: bool,
) -> GtsWrapper {
    let compratio = compratio.max(1.0);
    let mut wrapper = GtsWrapper::default();
    if compress {
        let mut bytes = encoder.stream.clone();
        let mut pass = 0i32;
        let (encoded, ratio) = loop {
            let encoded = bytes;
            let gzipped = gzip_vec(&encoded);
            let ratio = encoded.len() as f64 / gzipped.len() as f64;
            bytes = gzipped;
            pass += 1;
            if ratio <= compratio {
                break (encoded, ratio);
            }
        };
        if ratio > 1.0 {
            wrapper.encoded = Some(bytes);
        } else {
            pass -= 1;
            wrapper.encoded = Some(encoded);
        }
        if pass > 0 {
            wrapper.compressed = Some(true);
            if pass > 1 {
                wrapper.compression_passes = Some(pass);
            }
        }
    } else {
        wrapper.encoded = Some(encoder.stream.clone());
    }
    if encoder.base != 0 {
        wrapper.base = Some(encoder.base);
    }
    if set_count {
        wrapper.count = Some(encoder.count);
    }
    let meta = &encoder.metadata;
    if meta.name.as_deref().is_some_and(|name| !name.is_empty())
        || !meta.labels.is_empty()
        || !meta.attributes.is_empty()
        || meta.class_id_set
        || meta.labels_id_set
        || meta.source.is_some()
        || meta.last_activity_set
    {
        wrapper.metadata = Some(meta.clone());
    }
    wrapper
}

/// `GTSWrapperHelper.fromGTSToGTSWrapper`: the series encoded (doubles optimized when
/// `optimized`), with its bucketization.
pub fn gts_to_wrapper(
    gts: &GeoTimeSerie,
    compress: bool,
    compratio: f64,
    optimized: bool,
    set_count: bool,
) -> GtsWrapper {
    let mut encoder = GtsEncoder::new(0);
    encoder.metadata = gts.metadata.clone();
    if optimized && gts.type_of() == Type::Double {
        encoder.encode_optimized(gts);
    } else {
        encoder.encode(gts);
    }
    let mut wrapper = encoder_to_wrapper(&encoder, compress, compratio, set_count);
    if gts.is_bucketized() {
        wrapper.bucketcount = Some(gts.bucketcount);
        wrapper.bucketspan = Some(gts.bucketspan);
        wrapper.lastbucket = Some(gts.lastbucket);
    }
    wrapper
}

/// `GTSWrapperHelper.unwrapEncoded`: the content, gunzipped as many times as it was gzipped.
fn unwrap_encoded(wrapper: &GtsWrapper, encoded: &[u8]) -> Result<Vec<u8>> {
    let mut bytes = encoded.to_vec();
    if !wrapper.is_compressed() {
        return Ok(bytes);
    }
    for _ in 0..wrapper.compression_passes() {
        bytes = ungzip_bytes(&bytes)
            .map_err(|err| EncoderError::InvalidCompressedContent(err.into()))?;
    }
    Ok(bytes)
}

/// `GTSWrapperHelper.fromGTSWrapperToGTS`: the series of a wrapper, without its datapoints
/// when `empty`.
pub fn wrapper_to_gts(wrapper: &GtsWrapper, empty: bool) -> Result<GeoTimeSerie> {
    let mut gts = match &wrapper.encoded {
        Some(encoded) if !empty => {
            let bytes = unwrap_encoded(wrapper, encoded)?;
            GtsDecoder::new(wrapper.base(), Rc::from(bytes)).decode()?
        }
        _ => GeoTimeSerie::new(),
    };
    if let Some(bucketcount) = wrapper.bucketcount {
        gts.bucketcount = i64::from(bucketcount as i32);
    }
    if let Some(bucketspan) = wrapper.bucketspan {
        gts.bucketspan = bucketspan;
    }
    if let Some(lastbucket) = wrapper.lastbucket {
        gts.lastbucket = lastbucket;
    }
    gts.metadata = wrapper.metadata.clone().unwrap_or_default();
    Ok(gts)
}

/// `GTSWrapperHelper.fromGTSWrapperToGTSEncoder`.
pub fn wrapper_to_encoder(wrapper: &GtsWrapper) -> Result<GtsEncoder> {
    let encoded = wrapper.encoded.as_deref().ok_or_else(Error::null_pointer)?;
    let metadata = wrapper.metadata.clone().unwrap_or_default();
    if encoded.is_empty() {
        let mut encoder = GtsEncoder::new(wrapper.base());
        encoder.metadata = metadata;
        return Ok(encoder);
    }
    let mut decoder = wrapper_to_decoder(wrapper)?;
    decoder.next()?;
    decoder.encoder()
}

/// `GTSWrapperHelper.fromGTSWrapperToGTSDecoder`.
pub fn wrapper_to_decoder(wrapper: &GtsWrapper) -> Result<GtsDecoder> {
    let encoded = wrapper.encoded.as_deref().ok_or_else(Error::null_pointer)?;
    let bytes = unwrap_encoded(wrapper, encoded)?;
    let mut decoder = GtsDecoder::new(wrapper.base(), Rc::from(bytes));
    decoder.metadata = wrapper.metadata.clone().unwrap_or_default();
    decoder.count = wrapper.count();
    Ok(decoder)
}
