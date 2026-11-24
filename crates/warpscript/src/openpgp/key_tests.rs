// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of the public part of the key packets: the fingerprints and key IDs of versions 3 and 4,
//! the material of each algorithm family, the curves, and the failures of the algorithm names.

use digest::Digest;

use crate::error::pgp::PgpError;
use crate::openpgp::key::{PublicKey, key_id_hex, to_hex};

/// A multi-precision integer: its bit length, then its bytes.
fn mpi(bits: u16, bytes: &[u8]) -> Vec<u8> {
    let mut out = bits.to_be_bytes().to_vec();
    out.extend_from_slice(bytes);
    out
}

/// The RSA material of a key: a modulus of 16 bits and an exponent of 2 bits.
fn rsa_material() -> Vec<u8> {
    [mpi(16, &[0xAB, 0xCD]), mpi(2, &[0x03])].concat()
}

/// A version 4 key packet body with the given algorithm and material.
fn v4(algorithm: u8, material: &[u8]) -> Vec<u8> {
    let mut body = vec![4];
    body.extend_from_slice(&1_000u32.to_be_bytes());
    body.push(algorithm);
    body.extend_from_slice(material);
    body
}

/// A version 2 or 3 key packet body with the given algorithm, validity and material.
fn v3(version: u8, valid_days: u16, algorithm: u8, material: &[u8]) -> Vec<u8> {
    let mut body = vec![version];
    body.extend_from_slice(&1_000u32.to_be_bytes());
    body.extend_from_slice(&valid_days.to_be_bytes());
    body.push(algorithm);
    body.extend_from_slice(material);
    body
}

fn parse(body: &[u8]) -> Result<PublicKey, PgpError> {
    PublicKey::parse(body).map(|(key, _)| key)
}

#[test]
fn version_4_rsa_fingerprint_and_key_id() {
    let body = v4(1, &rsa_material());
    let (key, consumed) = PublicKey::parse(&body).expect("parses");
    assert_eq!(consumed, body.len());
    assert_eq!(key.version, 4);
    assert_eq!(key.created, 1_000);
    assert_eq!(key.bits, 16);
    assert_eq!(key.creation_ms(), 1_000_000);
    assert_eq!(key.valid_seconds_v3(), 0);
    assert_eq!(key.algorithm_name(), "RSA_GENERAL");
    assert!(key.is_signing());
    assert!(key.is_encryption());

    let mut input = vec![0x99, 0, body.len() as u8];
    input.extend_from_slice(&body);
    let fingerprint = sha1::Sha1::digest(&input).to_vec();
    assert_eq!(key.fingerprint, fingerprint);
    assert_eq!(key.fingerprint_hex(), to_hex(&fingerprint));
    let low: [u8; 8] = fingerprint[12..].try_into().expect("8 bytes");
    assert_eq!(key.key_id, u64::from_be_bytes(low));
}

#[test]
fn consumed_stops_before_trailing_bytes() {
    let body = v4(1, &rsa_material());
    let mut with_tail = body.clone();
    with_tail.extend_from_slice(&[0xFF, 0x00]);
    let (_, consumed) = PublicKey::parse(&with_tail).expect("parses");
    assert_eq!(consumed, body.len());
}

#[test]
fn version_2_and_3_rsa_use_the_modulus() {
    for version in [2, 3] {
        let body = v3(version, 2, 1, &rsa_material());
        let key = parse(&body).expect("parses");
        assert_eq!(key.version, version);
        assert_eq!(key.valid_days, 2);
        assert_eq!(key.valid_seconds_v3(), 2 * 86_400);
        assert_eq!(key.fingerprint.len(), 16);
        assert_eq!(key.key_id, 0xABCD);
        assert_eq!(key_id_hex(key.key_id), "000000000000ABCD");
    }
}

#[test]
fn version_2_and_3_of_another_algorithm_are_unsupported() {
    let curve = [
        vec![8],
        vec![0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07],
    ]
    .concat();
    let ecdsa = v3(3, 0, 19, &[curve, mpi(256, &[1; 32])].concat());
    assert_eq!(parse(&ecdsa), Err(PgpError::UnsupportedAlgorithm(19)));
    let elgamal = v3(
        3,
        0,
        16,
        &[mpi(16, &[0xAB, 0xCD]), mpi(2, &[0x03]), mpi(3, &[0x05])].concat(),
    );
    assert_eq!(parse(&elgamal), Err(PgpError::UnsupportedAlgorithm(16)));
}

#[test]
fn unsupported_version_after_the_material() {
    let body = [5, 0, 0, 0, 0, 1, 0, 16, 0xAB, 0xCD, 0, 2, 3];
    assert_eq!(parse(&body), Err(PgpError::UnsupportedVersion(5)));
}

#[test]
fn discrete_log_algorithms() {
    let elgamal = [mpi(16, &[1, 2]), mpi(8, &[3]), mpi(8, &[4])].concat();
    for algorithm in [16, 20, 21] {
        let key = parse(&v4(algorithm, &elgamal)).expect("parses");
        assert_eq!(key.bits, 16);
    }
    let dsa = [elgamal.clone(), mpi(8, &[5])].concat();
    let key = parse(&v4(17, &dsa)).expect("parses");
    assert_eq!(key.bits, 16);
    assert_eq!(key.algorithm_name(), "DSA");
}

#[test]
fn elliptic_curves_and_their_strength() {
    let curves: &[(&[u8], u16)] = &[
        (&[0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07], 256),
        (&[0x2B, 0x81, 0x04, 0x00, 0x22], 384),
        (&[0x2B, 0x81, 0x04, 0x00, 0x23], 521),
        (&[0x2B, 0x81, 0x04, 0x00, 0x0A], 256),
        (&[0x2B, 0x24, 0x03, 0x03, 0x02, 0x08, 0x01, 0x01, 0x07], 256),
        (&[0x2B, 0x24, 0x03, 0x03, 0x02, 0x08, 0x01, 0x01, 0x0B], 384),
        (&[0x2B, 0x24, 0x03, 0x03, 0x02, 0x08, 0x01, 0x01, 0x0D], 512),
        (&[0x2B, 0x06, 0x01, 0x04, 0x01, 0xDA, 0x47, 0x0F, 0x01], 256),
        (
            &[0x2B, 0x06, 0x01, 0x04, 0x01, 0x97, 0x55, 0x01, 0x05, 0x01],
            256,
        ),
    ];
    for &(oid, bits) in curves {
        let curve = [vec![oid.len() as u8], oid.to_vec()].concat();
        let point = mpi(bits, &vec![9; usize::from(bits).div_ceil(8)]);

        let material = [curve.clone(), point.clone()].concat();
        let key = parse(&v4(19, &material)).expect("ECDSA parses");
        assert_eq!(key.bits, u32::from(bits));
        assert_eq!(key.algorithm_name(), "ECDSA");

        let key = parse(&v4(22, &material)).expect("EdDSA parses");
        assert_eq!(key.algorithm_name(), "EDDSA");

        let kdf = [3, 1, 2, 3];
        let material = [curve, point, kdf.to_vec()].concat();
        let key = parse(&v4(18, &material)).expect("ECDH parses");
        assert_eq!(key.bits, u32::from(bits));
        assert_eq!(key.algorithm_name(), "ECDH");
    }
}

#[test]
fn elliptic_curve_failures() {
    let point = mpi(256, &[9; 32]);
    let oid = [0x2B, 0x81, 0x04, 0x00, 0x22];
    let unknown = [vec![3], vec![1, 2, 3]].concat();
    assert_eq!(
        parse(&v4(19, &[unknown, point.clone()].concat())),
        Err(PgpError::UnknownCurve)
    );
    assert_eq!(parse(&v4(19, &[8, 1, 2])), Err(PgpError::PrematureEnd));
    let short_kdf = [vec![5], oid.to_vec(), point, vec![5, 1]].concat();
    assert_eq!(parse(&v4(18, &short_kdf)), Err(PgpError::PrematureEnd));
}

#[test]
fn unreadable_material() {
    assert_eq!(parse(&v4(25, &[])), Err(PgpError::UnsupportedAlgorithm(25)));
    assert_eq!(parse(&v4(28, &[])), Err(PgpError::UnsupportedAlgorithm(28)));
    assert_eq!(parse(&v4(99, &[])), Err(PgpError::UnknownAlgorithm(99)));
    assert_eq!(parse(&[4, 0, 0, 0, 0]), Err(PgpError::UnknownAlgorithm(-1)));
    assert_eq!(
        parse(&[4, 0, 0, 0, 0, 1, 0, 16, 0xAB]),
        Err(PgpError::PrematureEnd)
    );
    assert_eq!(parse(&[4, 0, 0, 0, 0, 1, 0]), Err(PgpError::PrematureEnd));
}

#[test]
fn algorithm_names_and_flags() {
    let named = |algorithm: u8| PublicKey {
        version: 4,
        created: 0,
        valid_days: 0,
        algorithm,
        bits: 0,
        fingerprint: Vec::new(),
        key_id: 0,
    };
    assert_eq!(named(1).algorithm_name(), "RSA_GENERAL");
    assert_eq!(named(2).algorithm_name(), "RSA_ENCRYPT");
    assert_eq!(named(3).algorithm_name(), "RSA_SIGN");
    assert_eq!(named(16).algorithm_name(), "ELGAMAL_ENCRYPT");
    assert_eq!(named(17).algorithm_name(), "DSA");
    assert_eq!(named(19).algorithm_name(), "ECDSA");
    assert_eq!(named(20).algorithm_name(), "ELGAMAL_GENERAL");
    assert_eq!(named(18).algorithm_name(), "ECDH");
    assert_eq!(named(21).algorithm_name(), "DIFFIE_HELLMAN");
    assert_eq!(named(22).algorithm_name(), "EDDSA");
    assert_eq!(named(0).algorithm_name(), "UNKNOWN_0");
    assert_eq!(named(255).algorithm_name(), "UNKNOWN_-1");
    assert!(named(2).is_encryption());
    assert!(named(18).is_encryption());
    assert!(!named(3).is_encryption());
    assert!(named(3).is_signing());
    assert!(named(22).is_signing());
    assert!(!named(2).is_signing());
}

#[test]
fn key_ids_are_sixteen_upper_case_digits() {
    assert_eq!(key_id_hex(0x1234), "0000000000001234");
    assert_eq!(key_id_hex(u64::MAX), "FFFFFFFFFFFFFFFF");
}

#[test]
fn material_cut_short_after_an_integer_fails() {
    assert_eq!(
        parse(&v4(1, &mpi(16, &[0xAB, 0xCD]))),
        Err(PgpError::PrematureEnd)
    );
    let elgamal = [mpi(16, &[1, 2]), mpi(8, &[3])].concat();
    assert_eq!(parse(&v4(16, &elgamal)), Err(PgpError::PrematureEnd));
    let curve = [8, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07];
    assert_eq!(parse(&v4(19, &curve)), Err(PgpError::PrematureEnd));
}
