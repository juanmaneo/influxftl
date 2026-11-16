// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of `GZIP` / `UNGZIP` / `DEFLATE` / `INFLATE`, including streams produced by the JDK.

use super::compression::{
    compress_fn, deflate_bytes, deflate_into, gzip_bytes, gzip_into, inflate_bytes, ungzip_bytes,
};
use crate::encoding::hex_decode;
use crate::stack::Stack;
use crate::tests::{assert_fails, assert_top, fail_msg, top};
use crate::value::Value;

const TEXT: &str =
    "The quick brown fox jumps over the lazy dog. The quick brown fox jumps over the lazy dog.";

/// `GZIPOutputStream` output of the JDK for [`TEXT`].
const GZIP_SAMPLE: &str = "1f8b08000000000000ff0bc94855282ccd4cce56482aca2fcf5348cbaf50c82acd2d2856c82f4b2d5228014ae72456552aa4e4a7eb298490a01800a70ae99a59000000";

/// `DeflaterOutputStream` output of the JDK for [`TEXT`].
const DEFLATE_SAMPLE: &str = "789c0bc94855282ccd4cce56482aca2fcf5348cbaf50c82acd2d2856c82f4b2d5228014ae72456552aa4e4a7eb298490a01800aed1202f";

#[test]
fn gzip_round_trips() {
    assert_top(&format!("'{TEXT}' GZIP UNGZIP 'UTF-8' BYTES->"), TEXT);
    assert_top("'' GZIP UNGZIP ->HEX", "");
    assert_top("'00ff' HEX-> GZIP UNGZIP ->HEX", "00ff");
    // gzip magic, deflate method, no flags, no mtime, and the "unknown" OS byte like JDK 16+.
    let header = top(&format!("'{TEXT}' GZIP ->HEX")).to_string();
    assert!(header.starts_with("1f8b08000000000000ff"), "{header}");
    assert!(
        top(&format!("'{TEXT}' GZIP SIZE"))
            .as_i64()
            .expect("as_i64 should succeed")
            < TEXT.len() as i64
    );
}

#[test]
fn deflate_round_trips() {
    assert_top(&format!("'{TEXT}' DEFLATE INFLATE 'UTF-8' BYTES->"), TEXT);
    assert_top("'' DEFLATE INFLATE ->HEX", "");
    assert_top("'00ff' HEX-> DEFLATE INFLATE ->HEX", "00ff");
    assert!(top("'' DEFLATE ->HEX").to_string().starts_with("789c"));
}

#[test]
fn java_streams_decompress() {
    assert_top(
        &format!("'{GZIP_SAMPLE}' HEX-> UNGZIP 'UTF-8' BYTES->"),
        TEXT,
    );
    assert_top(
        &format!("'{DEFLATE_SAMPLE}' HEX-> INFLATE 'UTF-8' BYTES->"),
        TEXT,
    );
    // An empty gzip member written by the JDK.
    assert_top(
        "'1f8b08000000000000ff03000000000000000000' HEX-> UNGZIP ->HEX",
        "",
    );
}

#[test]
fn ungzip_handles_members_and_trailing_bytes() {
    let hello = gzip_bytes(b"hello").expect("gzip_bytes should succeed");
    let world = gzip_bytes(b" world").expect("gzip_bytes should succeed");
    let empty = gzip_bytes(b"").expect("gzip_bytes should succeed");
    let mut members = hello.clone();
    members.extend_from_slice(&empty);
    members.extend_from_slice(&world);
    assert_eq!(
        ungzip_bytes(&members).expect("ungzip_bytes should succeed"),
        b"hello world"
    );
    // Like `GZIPInputStream`, bytes that are not another member are ignored…
    let mut trailing = hello.clone();
    trailing.extend_from_slice(&[1, 2, 3]);
    assert_eq!(
        ungzip_bytes(&trailing).expect("ungzip_bytes should succeed"),
        b"hello"
    );
    // …but a second member that is corrupt or truncated is an error.
    let mut corrupt = hello.clone();
    let mut bad = world.clone();
    let crc = bad.len() - 5;
    bad[crc] ^= 1;
    corrupt.extend_from_slice(&bad);
    assert_eq!(
        ungzip_bytes(&corrupt)
            .expect_err("ungzip_bytes should fail")
            .to_string(),
        "corrupt gzip stream does not have a matching checksum"
    );
    let mut truncated = hello.clone();
    truncated.extend_from_slice(&world[..world.len() - 3]);
    assert!(ungzip_bytes(&truncated).is_err());
    // A corrupt first member is an error too.
    let crc = hello.len() - 5;
    let mut bad = hello.clone();
    bad[crc] ^= 1;
    assert!(ungzip_bytes(&bad).is_err());
    assert!(ungzip_bytes(&hello[..hello.len() - 2]).is_err());
    assert!(ungzip_bytes(b"").is_err());
}

#[test]
fn decompression_errors() {
    assert_fails(
        "'0102' HEX-> UNGZIP",
        "UNGZIP encountered an error while decompressing.",
    );
    assert_fails(
        "'' HEX-> UNGZIP",
        "UNGZIP encountered an error while decompressing.",
    );
    assert!(fail_msg("'0102' HEX-> INFLATE").ends_with(
        "(INFLATE encountered an error while decompressing. (corrupt deflate stream)))"
    ));
    assert_fails(
        "'' HEX-> INFLATE",
        "INFLATE encountered an error while decompressing. (incomplete deflate stream)",
    );
    assert_fails("'abc' UNGZIP", "UNGZIP operates on a byte array.");
    assert_fails("1 INFLATE", "INFLATE operates on a byte array.");
    assert_fails("1 GZIP", "GZIP operates on a byte array or a String.");
    assert_fails(
        "[ ] DEFLATE",
        "DEFLATE operates on a byte array or a String.",
    );
    for f in ["GZIP", "UNGZIP", "DEFLATE", "INFLATE"] {
        assert_fails(f, "Empty stack.");
    }
}

#[test]
fn compression_failures_are_reported() {
    fn failing(_: &[u8]) -> std::io::Result<Vec<u8>> {
        Err(std::io::Error::other("disk full"))
    }
    let mut stack = Stack::new();
    stack.push(Value::from("data"));
    let err = compress_fn(&mut stack, "GZIP", failing).expect_err("compress_fn should fail");
    assert_eq!(
        err.to_string(),
        "GZIP encountered an error while compressing. (disk full)"
    );
    assert_eq!(stack.depth(), 0);
}

/// A sink that refuses every write.
#[derive(Debug)]
struct BrokenSink;

impl std::io::Write for BrokenSink {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("broken pipe"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn write_errors_surface_while_compressing() {
    // Incompressible data larger than the encoders' internal buffer forces a write to the
    // sink before `finish`.
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let noise: Vec<u8> = (0..200_000)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 56) as u8
        })
        .collect();
    assert_eq!(
        gzip_into(&mut BrokenSink, &noise)
            .expect_err("gzip_into should fail")
            .to_string(),
        "broken pipe"
    );
    assert_eq!(
        deflate_into(&mut BrokenSink, &noise)
            .expect_err("the value should fail")
            .to_string(),
        "broken pipe"
    );
    // A sink that fails only once the stream is finished.
    assert!(gzip_into(&mut BrokenSink, b"").is_err());
    assert!(deflate_into(&mut BrokenSink, b"").is_err());
    assert_eq!(
        ungzip_bytes(&gzip_bytes(&noise).expect("gzip_bytes should succeed"))
            .expect("ungzip_bytes should succeed"),
        noise
    );
}

#[test]
fn byte_helpers_round_trip() {
    let data = hex_decode("00010203fffefd").expect("hex_decode should succeed");
    assert_eq!(
        ungzip_bytes(&gzip_bytes(&data).expect("gzip_bytes should succeed"))
            .expect("ungzip_bytes should succeed"),
        data
    );
    assert_eq!(
        inflate_bytes(&deflate_bytes(&data).expect("deflate_bytes should succeed"))
            .expect("inflate_bytes should succeed"),
        data
    );
}
