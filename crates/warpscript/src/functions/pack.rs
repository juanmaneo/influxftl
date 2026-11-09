// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Bit packing functions of WarpLib: `PACK` and `UNPACK`.
//!
//! A format string describes a sequence of fields: `<` (little endian) or `>` (big endian)
//! followed by `L` or `U` and a decimal bit length (signed or unsigned integer, 64 bits when
//! the length is omitted) or by `D` (a 64-bit double); `B` (one bit, `false` / `true`); `S` or
//! `s` and a length (skipped bits, written as ones or zeros by `PACK`). Bits are packed most
//! significant first; `PACK` pads the last byte with zeros and `UNPACK` reads zeros past the
//! end of the data.
//!
//! The format is scanned by UTF-16 code unit like in Java, and lengths are accumulated with
//! Java `int` arithmetic (`L4294967297` is `L1`).

use super::Entry;
use super::args::{fail, pop_list, pop_string};
use crate::error::Result;
use crate::error::fault::{BoundsError, IllegalArgumentError};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PACK", Entry::Stack(pack)),
    ("UNPACK", Entry::Stack(unpack)),
];

/// The string made of a single UTF-16 code unit (`String.valueOf(char)`); a lone surrogate
/// becomes U+FFFD.
fn utf16_unit_string(unit: u16) -> String {
    String::from_utf16_lossy(&[unit])
}

/// The kind of a field of a `PACK` / `UNPACK` format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PackKind {
    /// `L` (signed) or `U` (unsigned) integer.
    Long { signed: bool },
    /// `D`: a 64-bit double.
    Double,
    /// `S` (ones) / `s` (zeros): skipped bits.
    Skip { ones: bool },
    /// `B`: a single bit.
    Bit,
}

/// A field of a `PACK` / `UNPACK` format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PackField {
    kind: PackKind,
    big_endian: bool,
    /// Bit length, a Java `int` (digits are accumulated with wrapping arithmetic).
    len: i32,
}

/// Parse the decimal digits at `idx` with Java `int` arithmetic.
fn parse_pack_digits(units: &[u16], idx: &mut usize) -> i32 {
    let mut len: i32 = 0;
    while let Some(&unit) = units.get(*idx)
        && (u16::from(b'0')..=u16::from(b'9')).contains(&unit)
    {
        len = len
            .wrapping_mul(10)
            .wrapping_add(i32::from(unit - u16::from(b'0')));
        *idx += 1;
    }
    len
}

/// `PACK.parseFormat`: the fields of a format string and the total bit length (a Java `int`).
fn parse_pack_format(name: &str, format: &str) -> Result<(Vec<PackField>, i32)> {
    let units: Vec<u16> = format.encode_utf16().collect();
    let mut fields = Vec::new();
    let mut total: i32 = 0;
    let mut idx = 0;
    while let Some(&first) = units.get(idx) {
        idx += 1;
        let mut big_endian = false;
        let (kind, len) = if first == u16::from(b'<') || first == u16::from(b'>') {
            let Some(&kind) = units.get(idx) else {
                return Err(fail(name, "encountered an invalid format specification."));
            };
            idx += 1;
            big_endian = first == u16::from(b'>');
            if kind == u16::from(b'L') || kind == u16::from(b'U') {
                let mut len = parse_pack_digits(&units, &mut idx);
                if len == 0 {
                    len = 64;
                } else if len > 64 {
                    return Err(fail(
                        name,
                        "encountered an invalid length for 'L', max length is 64.",
                    ));
                }
                (
                    PackKind::Long {
                        signed: kind == u16::from(b'L'),
                    },
                    len,
                )
            } else if kind == u16::from(b'D') {
                (PackKind::Double, 64)
            } else {
                return Err(fail(
                    name,
                    &format!(
                        "encountered an invalid format specification '{}'.",
                        utf16_unit_string(kind)
                    ),
                ));
            }
        } else if first == u16::from(b'S') || first == u16::from(b's') {
            let len = parse_pack_digits(&units, &mut idx);
            if len == 0 {
                return Err(fail(
                    name,
                    "encountered an invalid Skip specification, length must be a strictly positive number.",
                ));
            }
            (
                PackKind::Skip {
                    ones: first == u16::from(b'S'),
                },
                len,
            )
        } else if first == u16::from(b'B') {
            (PackKind::Bit, 1)
        } else {
            return Err(fail(
                name,
                &format!(
                    "encountered an invalid format specification '{}'.",
                    utf16_unit_string(first)
                ),
            ));
        };
        fields.push(PackField {
            kind,
            big_endian,
            len,
        });
        total = total.wrapping_add(len);
    }
    Ok((fields, total))
}

/// The bits of the `vidx`-th value of `PACK` for `field`, before endianness handling.
fn pack_value(values: &[Value], vidx: usize, kind: PackKind) -> Result<u64> {
    let Some(value) = values.get(vidx) else {
        return Err(BoundsError::ArrayIndex {
            index: vidx as i64,
            length: values.len(),
        }
        .into());
    };
    let number = match value {
        Value::Boolean(b) => Value::Long(i64::from(*b)),
        other => other.clone(),
    };
    Ok(match kind {
        PackKind::Double => number.as_f64().unwrap_or_default().to_bits(),
        PackKind::Bit => u64::from(number.as_i64().unwrap_or_default() != 0),
        _ => number.as_i64().unwrap_or_default() as u64,
    })
}

/// `PACK`: pack a list of numbers and booleans into a byte array according to a format
/// (`<`/`>` endianness followed by `L`/`U` with a bit length, `D`, `B`, or `S`/`s` skips).
fn pack(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PACK";
    const VALUES: &str = "operates on a list of numeric or boolean values.";
    let format = pop_string(stack, NAME, "expects a format string on top of the stack.")?;
    let values = pop_list(stack, NAME, VALUES)?;
    let values = values.borrow().clone();
    if values
        .iter()
        .any(|v| !(v.is_number() || matches!(v, Value::Boolean(_))))
    {
        return Err(fail(NAME, VALUES));
    }
    let (fields, total) = parse_pack_format(NAME, &format)?;
    let capacity = total.wrapping_add(7) / 8;
    if capacity < 0 {
        return Err(IllegalArgumentError::NegativeInitialSize(i64::from(capacity)).into());
    }
    let mut out = Vec::with_capacity(capacity as usize);
    let mut nbits = 0usize;
    let mut curbyte: u64 = 0;
    let mut vidx = 0;
    for field in &fields {
        let mut value = match field.kind {
            PackKind::Skip { ones } => {
                if ones {
                    u64::MAX
                } else {
                    0
                }
            }
            kind => {
                let value = pack_value(&values, vidx, kind)?;
                vidx += 1;
                value
            }
        };
        if field.big_endian {
            value = value.reverse_bits();
            if field.len < 64 {
                value >>= (64i32.wrapping_sub(field.len) & 63) as u32;
            }
        }
        for _ in 0..field.len.max(0) {
            curbyte = (curbyte << 1) | (value & 1);
            value >>= 1;
            nbits += 1;
            if nbits.is_multiple_of(8) {
                out.push(curbyte as u8);
                curbyte = 0;
            }
        }
    }
    if !nbits.is_multiple_of(8) {
        curbyte <<= 8 - (nbits % 8);
        out.push(curbyte as u8);
    }
    stack.push(Value::bytes(out));
    Ok(())
}

/// Bit `index` of `data`, most significant bit first; bits past the end read as 0 and negative
/// indices fail like `BitSet.get`.
fn bit_at(data: &[u8], index: i32) -> Result<u64> {
    if index < 0 {
        return Err(BoundsError::NegativeBitIndex(i64::from(index)).into());
    }
    let byte = data.get((index / 8) as usize).copied().unwrap_or_default();
    Ok(u64::from((byte >> (7 - index % 8)) & 1))
}

/// `UNPACK`: unpack a byte array into a list of values according to a format (see `PACK`).
fn unpack(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "UNPACK";
    let format = pop_string(stack, NAME, "expects a format string on top of the stack.")?;
    let Value::Bytes(data) = stack.pop()? else {
        return Err(fail(NAME, "operates on an array of bytes."));
    };
    let (fields, _) = parse_pack_format(NAME, &format)?;
    let mut values = Vec::new();
    let mut bitno: i32 = 0;
    for field in &fields {
        match field.kind {
            PackKind::Skip { .. } => {}
            PackKind::Bit => values.push(Value::Boolean(bit_at(&data, bitno)? != 0)),
            PackKind::Long { .. } | PackKind::Double => {
                let mut value: u64 = 0;
                for k in 0..field.len.max(0) {
                    value <<= 1;
                    let index = if field.big_endian {
                        bitno.wrapping_add(k)
                    } else {
                        bitno
                            .wrapping_add(field.len)
                            .wrapping_sub(1)
                            .wrapping_sub(k)
                    };
                    value |= bit_at(&data, index)?;
                }
                let shift = (64i32.wrapping_sub(field.len) & 63) as u32;
                values.push(match field.kind {
                    PackKind::Long { signed: true } => {
                        Value::Long(((value << shift) as i64) >> shift)
                    }
                    PackKind::Long { signed: false } => Value::Long(value as i64),
                    _ => Value::Double(f64::from_bits(value)),
                });
            }
        }
        bitno = bitno.wrapping_add(field.len);
    }
    stack.push(Value::list(values));
    Ok(())
}
