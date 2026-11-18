// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of the signature parser: the version 4 areas and their subpackets in each length form,
//! the version 3 layout, the versions that are skipped, and the failures on truncated input.

use crate::error::pgp::PgpError;
use crate::openpgp::signature::{Signature, Subpacket};

/// A subpacket of one octet of length, with its tag.
fn subpacket(tag: u8, data: &[u8]) -> Vec<u8> {
    let mut out = vec![(data.len() + 1) as u8, tag];
    out.extend_from_slice(data);
    out
}

/// A version 4 signature with the given hashed and unhashed subpackets, and no MPI.
fn v4(sig_type: u8, hashed: &[Vec<u8>], unhashed: &[Vec<u8>]) -> Vec<u8> {
    let hashed = hashed.concat();
    let unhashed = unhashed.concat();
    let mut body = vec![4, sig_type, 1, 8];
    body.extend_from_slice(&(hashed.len() as u16).to_be_bytes());
    body.extend_from_slice(&hashed);
    body.extend_from_slice(&(unhashed.len() as u16).to_be_bytes());
    body.extend_from_slice(&unhashed);
    body.extend_from_slice(&[0xAB, 0xCD]);
    body
}

#[test]
fn version_4_reads_creation_issuer_and_expiry() {
    let hashed = [
        subpacket(2, &2_000u32.to_be_bytes()),
        subpacket(0x80 | 16, &0x1122_3344_5566_7788u64.to_be_bytes()),
        subpacket(9, &3_600u32.to_be_bytes()),
        subpacket(5, &[1]),
    ];
    let unhashed = [subpacket(16, &0x99AA_BBCC_DDEE_FF00u64.to_be_bytes())];
    let sig = Signature::parse(&v4(0x13, &hashed, &unhashed))
        .expect("version 4 parses")
        .expect("version 4 is read");
    assert_eq!(sig.sig_type, 0x13);
    assert_eq!(sig.creation_ms, 2_000_000);
    assert_eq!(sig.key_id, 0x99AA_BBCC_DDEE_FF00);
    assert_eq!(sig.key_expiration(), Some(3_600));
}

#[test]
fn version_4_issuer_from_hashed_area() {
    let hashed = [subpacket(16, &7u64.to_be_bytes())];
    let unhashed = [subpacket(3, &[0])];
    let sig = Signature::parse(&v4(0x10, &hashed, &unhashed))
        .expect("parses")
        .expect("read");
    assert_eq!(sig.key_id, 7);
    assert_eq!(sig.key_expiration(), None);
}

#[test]
fn version_4_subpacket_lengths() {
    // Two octets of length: 201 is written as 192 + ((201 - 192) >> 8), then (201 - 192) & 0xFF.
    let mut long = vec![192, 9, 0x70];
    long.extend(std::iter::repeat_n(0u8, 200));
    // Five octets of length: 255, then the length on four octets.
    let five = [255, 0, 0, 0, 5, 9, 0, 0, 0x0E, 0x10];
    let sig = Signature::parse(&v4(0x13, &[long, five.to_vec()], &[]))
        .expect("parses")
        .expect("read");
    assert_eq!(sig.key_expiration(), Some(3_600));
    assert_eq!(sig.hashed.as_ref().map(Vec::len), Some(2));
}

#[test]
fn version_4_empty_subpacket_is_skipped() {
    let sig = Signature::parse(&v4(0x13, &[vec![0]], &[]))
        .expect("parses")
        .expect("read");
    assert_eq!(sig.hashed, Some(Vec::new()));
    assert_eq!(sig.creation_ms, 0);
    assert_eq!(sig.key_id, 0);
}

#[test]
fn version_4_truncated_areas() {
    let parse = |body: &[u8]| Signature::parse(body);
    assert_eq!(parse(&[4]), Err(PgpError::PrematureEnd));
    assert_eq!(parse(&[4, 0x13]), Err(PgpError::PrematureEnd));
    assert_eq!(parse(&[4, 0x13, 1, 8]), Err(PgpError::PrematureEnd));
    assert_eq!(parse(&[4, 0x13, 1, 8, 0, 0]), Err(PgpError::PrematureEnd));
    assert_eq!(
        parse(&[4, 0x13, 1, 8, 0, 5, 1]),
        Err(PgpError::PrematureEnd)
    );
}

#[test]
fn version_4_truncated_subpackets() {
    let parse = |hashed: &[u8]| {
        let mut body = vec![4, 0x13, 1, 8];
        body.extend_from_slice(&(hashed.len() as u16).to_be_bytes());
        body.extend_from_slice(hashed);
        body.extend_from_slice(&[0, 0]);
        Signature::parse(&body)
    };
    assert_eq!(parse(&[192]), Err(PgpError::PrematureEnd));
    assert_eq!(parse(&[255, 0, 0]), Err(PgpError::PrematureEnd));
    assert_eq!(parse(&[5, 9]), Err(PgpError::PrematureEnd));
    assert_eq!(parse(&subpacket(2, &[1, 2])), Err(PgpError::PrematureEnd));
    assert_eq!(parse(&subpacket(16, &[1])), Err(PgpError::PrematureEnd));
}

#[test]
fn version_4_truncated_unhashed_issuer() {
    let body = v4(0x13, &[], &[subpacket(16, &[1])]);
    assert_eq!(Signature::parse(&body), Err(PgpError::PrematureEnd));
    let body = v4(0x13, &[], &[vec![192]]);
    assert_eq!(Signature::parse(&body), Err(PgpError::PrematureEnd));
}

#[test]
fn version_3_fixed_layout() {
    let mut body = vec![3, 5, 0x18];
    body.extend_from_slice(&2u32.to_be_bytes());
    body.extend_from_slice(&0x0102_0304_0506_0708u64.to_be_bytes());
    body.extend_from_slice(&[1, 8, 0xAB, 0xCD]);
    let sig = Signature::parse(&body).expect("parses").expect("read");
    assert_eq!(sig.sig_type, 0x18);
    assert_eq!(sig.creation_ms, 2_000);
    assert_eq!(sig.key_id, 0x0102_0304_0506_0708);
    assert_eq!(sig.hashed, None);
    assert_eq!(sig.key_expiration(), None);
}

#[test]
fn version_3_truncated() {
    assert_eq!(Signature::parse(&[3, 5]), Err(PgpError::PrematureEnd));
    assert_eq!(
        Signature::parse(&[3, 5, 0x18, 0, 0, 0]),
        Err(PgpError::PrematureEnd)
    );
    assert_eq!(
        Signature::parse(&[3, 5, 0x18, 0, 0, 0, 1, 1, 2, 3]),
        Err(PgpError::PrematureEnd)
    );
}

#[test]
fn other_versions_are_not_read() {
    assert_eq!(Signature::parse(&[5, 0]), Ok(None));
    assert_eq!(Signature::parse(&[]), Ok(None));
}

#[test]
fn key_expiration_needs_a_full_value() {
    let sig = Signature {
        sig_type: 0x13,
        key_id: 0,
        creation_ms: 0,
        hashed: Some(vec![Subpacket {
            tag: 9,
            data: vec![1],
        }]),
    };
    assert_eq!(sig.key_expiration(), None);
}
