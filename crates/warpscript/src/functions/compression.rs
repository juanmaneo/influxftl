// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Compression functions of WarpLib: `GZIP` / `UNGZIP` (RFC 1952) and `DEFLATE` / `INFLATE`
//! (zlib streams, RFC 1950), the formats of `java.util.zip.GZIPOutputStream` and
//! `DeflaterOutputStream`.
//!
//! The streams are produced by the pure Rust `miniz_oxide` backend of `flate2` at the default
//! level: they decompress to the same bytes as Java's but are not byte-identical to the zlib
//! output of the JDK (see `docs/DIVERGENCES.md`).

use std::io::{Read, Write};

use flate2::Compression;
use flate2::bufread::GzDecoder;
use flate2::read::ZlibDecoder;
use flate2::write::{GzEncoder, ZlibEncoder};

use super::Entry;
use super::args::{fail, fail_with_cause};
use crate::error::{Error, IoFailure, Result};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("GZIP", Entry::Stack(gzip)),
    ("UNGZIP", Entry::Stack(ungzip)),
    ("DEFLATE", Entry::Stack(deflate)),
    ("INFLATE", Entry::Stack(inflate)),
];

/// Compress a byte array or string (UTF-8) with `compress`.
pub(crate) fn compress_fn(
    stack: &mut Stack,
    name: &str,
    compress: fn(&[u8]) -> std::io::Result<Vec<u8>>,
) -> Result<()> {
    let data = match stack.pop()? {
        Value::Bytes(bytes) => bytes.to_vec(),
        Value::String(s) => s.as_bytes().to_vec(),
        _ => return Err(fail(name, "operates on a byte array or a String.")),
    };
    let compressed = compress(&data).map_err(|err| {
        fail_with_cause(
            name,
            "encountered an error while compressing.",
            Error::from(IoFailure::from(err)),
        )
    })?;
    stack.push(Value::bytes(compressed));
    Ok(())
}

/// Decompress a byte array with `decompress`.
fn decompress_fn(
    stack: &mut Stack,
    name: &str,
    decompress: fn(&[u8]) -> std::io::Result<Vec<u8>>,
) -> Result<()> {
    let Value::Bytes(data) = stack.pop()? else {
        return Err(fail(name, "operates on a byte array."));
    };
    let decompressed = decompress(&data).map_err(|err| {
        fail_with_cause(
            name,
            "encountered an error while decompressing.",
            Error::from(IoFailure::from(err)),
        )
    })?;
    stack.push(Value::bytes(decompressed));
    Ok(())
}

/// `GZIPOutputStream`: a single gzip member at the default level written to `sink`.
pub(crate) fn gzip_into(sink: &mut dyn Write, data: &[u8]) -> std::io::Result<()> {
    let mut encoder = GzEncoder::new(sink, Compression::default());
    encoder.write_all(data)?;
    encoder.finish().map(|_| ())
}

/// `GZIPOutputStream`: a single gzip member at the default level.
pub(crate) fn gzip_bytes(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut compressed = Vec::new();
    gzip_into(&mut compressed, data).map(|()| compressed)
}

/// `GZIPInputStream`: every member of a gzip stream. Like Java, trailing bytes that do not
/// start another member are ignored, while a member that starts and then turns out corrupt
/// (or truncated) is an error.
pub(crate) fn ungzip_bytes(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut decompressed = Vec::with_capacity(data.len());
    let mut rest = data;
    let mut first = true;
    loop {
        let mut decoder = GzDecoder::new(rest);
        if let Err(err) = decoder.read_to_end(&mut decompressed) {
            if first || decoder.header().is_some() {
                return Err(err);
            }
            break;
        }
        rest = decoder.into_inner();
        first = false;
        if rest.is_empty() {
            break;
        }
    }
    Ok(decompressed)
}

/// `DeflaterOutputStream`: a zlib stream at the default level written to `sink`.
pub(crate) fn deflate_into(sink: &mut dyn Write, data: &[u8]) -> std::io::Result<()> {
    let mut encoder = ZlibEncoder::new(sink, Compression::default());
    encoder.write_all(data)?;
    encoder.finish().map(|_| ())
}

/// `DeflaterOutputStream`: a zlib stream at the default level.
pub(crate) fn deflate_bytes(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut compressed = Vec::new();
    deflate_into(&mut compressed, data).map(|()| compressed)
}

/// `InflaterInputStream`: the content of a zlib stream.
pub(crate) fn inflate_bytes(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut decompressed = Vec::with_capacity(data.len());
    ZlibDecoder::new(data).read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

/// `GZIP`: gzip a byte array or string.
fn gzip(stack: &mut Stack) -> Result<()> {
    compress_fn(stack, "GZIP", gzip_bytes)
}

/// `UNGZIP`: decompress a gzip byte array.
fn ungzip(stack: &mut Stack) -> Result<()> {
    decompress_fn(stack, "UNGZIP", ungzip_bytes)
}

/// `DEFLATE`: compress a byte array or string into a zlib stream.
fn deflate(stack: &mut Stack) -> Result<()> {
    compress_fn(stack, "DEFLATE", deflate_bytes)
}

/// `INFLATE`: decompress a zlib byte array.
fn inflate(stack: &mut Stack) -> Result<()> {
    decompress_fn(stack, "INFLATE", inflate_bytes)
}
