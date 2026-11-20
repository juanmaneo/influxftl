// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of the OpenPGP signatures: the signatures that Warp 10 3.5.0 gives, the ones the port
//! makes, and the failures of their parsing.

use super::key::PublicKey;
use super::packet::{PacketReader, write_new_packet, write_packet};
use super::pgp_fixtures::{ALICE_PUBLIC, ALICE_SECRET, HELLO_DETACHED, HELLO_INLINE};
use super::ring::read_rings;
use super::secret::rsa_private_key;
use super::sign::{
    SigningParams, detached_body, detached_packet, digest_by_name, hash_of_id, inline_message,
    public_rsa, read_inline, verify,
};
use crate::encoding::hex_decode;
use crate::error::pgp::PgpError;
use crate::rsa::{Hash, RsaKey};

const KEY_ID: u64 = 0x8C80_1205_4C30_1346;

/// The bytes of a hexadecimal fixture.
fn bytes(text: &str) -> Vec<u8> {
    match hex_decode(text) {
        Ok(bytes) => bytes,
        Err(error) => panic!("the fixture must be hexadecimal: {error}"),
    }
}

/// The body of the first key packet of a key ring fixture.
fn first_body(text: &str) -> Vec<u8> {
    let Ok(rings) = read_rings(&bytes(text)) else {
        panic!("the fixture must be a key ring");
    };
    let Some(ring) = rings.first() else {
        panic!("the fixture must have a ring");
    };
    let Some(key) = ring.keys.first() else {
        panic!("the ring must have a key");
    };
    let Some(packet) = key.packets.first() else {
        panic!("the key must have a packet");
    };
    packet.body.clone()
}

/// The RSA public key of the alice fixture.
fn alice_public() -> RsaKey {
    match public_rsa(&first_body(ALICE_PUBLIC)) {
        Ok(key) => key,
        Err(error) => panic!("the fixture must be RSA: {error}"),
    }
}

/// The private key of the alice fixture.
fn alice_private() -> RsaKey {
    match rsa_private_key(&first_body(ALICE_SECRET), "secret") {
        Ok(key) => key,
        Err(error) => panic!("the fixture must unlock: {error}"),
    }
}

/// The body of the signature packet of the detached signature of `hello` by Warp 10.
fn oracle_detached() -> Vec<u8> {
    match detached_body(&bytes(HELLO_DETACHED)) {
        Ok(body) => body,
        Err(error) => panic!("the signature must parse: {error}"),
    }
}

/// The packets of a one-pass message, as the reader gives them.
fn packets(input: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut reader = PacketReader::new(input);
    let mut out = Vec::new();
    while let Ok(Some(_)) = reader.next_tag() {
        match reader.read() {
            Ok(packet) => out.push((packet.tag, packet.body)),
            Err(error) => panic!("the packets must parse: {error}"),
        }
    }
    out
}

#[test]
fn the_digests_are_known_by_name_in_any_case() {
    assert_eq!(digest_by_name("sha512"), Ok((10, Hash::Sha512)));
    assert_eq!(digest_by_name("MD5"), Ok((1, Hash::Md5)));
    assert_eq!(digest_by_name("RIPEMD160"), Ok((3, Hash::Ripemd160)));
    assert_eq!(
        digest_by_name("SHA3-256"),
        Err(PgpError::UnportedDigest(12))
    );
    assert_eq!(
        digest_by_name("Nope"),
        Err(PgpError::UnknownDigest("nope".to_owned()))
    );
}

#[test]
fn the_digests_are_known_by_their_identifiers() {
    assert_eq!(hash_of_id(8), Ok(Hash::Sha256));
    assert_eq!(hash_of_id(11), Ok(Hash::Sha224));
    assert_eq!(hash_of_id(12), Err(PgpError::UnportedDigest(12)));
}

#[test]
fn a_signature_of_warp_10_verifies() {
    let key = alice_public();
    assert_eq!(verify(&key, &oracle_detached(), b"hello"), Ok(true));
}

#[test]
fn a_signature_of_other_data_does_not_verify() {
    let key = alice_public();
    assert_eq!(verify(&key, &oracle_detached(), b"hellx"), Ok(false));
}

#[test]
fn a_tampered_signature_does_not_verify() {
    let key = alice_public();
    let mut body = oracle_detached();
    let Some(last) = body.last_mut() else {
        panic!("the signature has a body");
    };
    *last ^= 1;
    assert_eq!(verify(&key, &body, b"hello"), Ok(false));
}

#[test]
fn a_signature_of_another_version_is_unsupported() {
    let key = alice_public();
    let mut body = oracle_detached();
    if let Some(version) = body.first_mut() {
        *version = 3;
    }
    assert_eq!(
        verify(&key, &body, b"hello"),
        Err(PgpError::UnsupportedVersion(3))
    );
}

#[test]
fn every_cut_of_a_signature_fails_to_parse() {
    let key = alice_public();
    let body = oracle_detached();
    for cut in 0..body.len() {
        let Some(prefix) = body.get(..cut) else {
            panic!("the cut is within the body");
        };
        assert!(
            verify(&key, prefix, b"hello").is_err(),
            "a signature cut at {cut} must not parse"
        );
    }
}

#[test]
fn a_signature_with_an_unknown_digest_fails() {
    let key = alice_public();
    let mut body = oracle_detached();
    if let Some(digest) = body.get_mut(3) {
        *digest = 12;
    }
    assert_eq!(
        verify(&key, &body, b"hello"),
        Err(PgpError::UnportedDigest(12))
    );
}

#[test]
fn a_key_that_is_not_a_key_fails_the_verification() {
    let key = RsaKey {
        modulus: num_bigint::BigInt::from(0),
        exponent: num_bigint::BigInt::from(3),
    };
    assert!(verify(&key, &oracle_detached(), b"hello").is_err());
}

#[test]
fn a_detached_packet_is_read_only_when_it_is_a_signature() {
    assert_eq!(
        detached_body(&bytes(HELLO_INLINE)),
        Err(PgpError::UnexpectedPacket(4))
    );
    assert_eq!(detached_body(&[]), Err(PgpError::PrematureEnd));
}

#[test]
fn a_one_pass_message_of_warp_10_verifies_with_its_data() {
    let key = alice_public();
    let Ok((data, body)) = read_inline(&bytes(HELLO_INLINE)) else {
        panic!("the one-pass message must parse");
    };
    assert_eq!(data, b"hello");
    assert_eq!(verify(&key, &body, &data), Ok(true));
}

#[test]
fn a_detached_signature_is_not_a_one_pass_message() {
    assert_eq!(
        read_inline(&bytes(HELLO_DETACHED)),
        Err(PgpError::UnexpectedPacket(2))
    );
}

#[test]
fn every_cut_of_the_literal_of_a_one_pass_message_fails() {
    let found = packets(&bytes(HELLO_INLINE));
    let [(one_pass, _), (_, literal), (signature, _)] = found.as_slice() else {
        panic!("a one-pass message has three packets");
    };
    assert_eq!((*one_pass, *signature), (4, 2));
    for cut in 0..14 {
        let Some(prefix) = literal.get(..cut) else {
            panic!("the cut is within the literal");
        };
        let mut message = Vec::new();
        write_packet(&mut message, 4, &found[0].1);
        write_new_packet(&mut message, 11, prefix);
        write_packet(&mut message, 2, &found[2].1);
        assert!(read_inline(&message).is_err(), "a literal cut at {cut}");
    }
}

#[test]
fn a_signed_packet_verifies_with_the_key_that_made_it() {
    let private = alice_private();
    let signing = SigningParams {
        key: &private,
        algorithm: 1,
        key_id: KEY_ID,
        hash_id: 8,
        hash: Hash::Sha256,
        created: 1_700_000_000,
    };
    let Ok(packet) = detached_packet(&signing, b"some data") else {
        panic!("the signature must be made");
    };
    let Ok(body) = detached_body(&packet) else {
        panic!("the signature packet must parse");
    };
    assert_eq!(verify(&alice_public(), &body, b"some data"), Ok(true));
    assert_eq!(verify(&alice_public(), &body, b"other data"), Ok(false));
}

#[test]
fn an_inline_message_verifies_and_carries_its_date() {
    let private = alice_private();
    let signing = SigningParams {
        key: &private,
        algorithm: 1,
        key_id: KEY_ID,
        hash_id: 10,
        hash: Hash::Sha512,
        created: 1_700_000_000,
    };
    let Ok(message) = inline_message(&signing, 42, b"hello") else {
        panic!("the message must be signed");
    };
    let Ok((data, body)) = read_inline(&message) else {
        panic!("the message must parse");
    };
    assert_eq!(data, b"hello");
    assert_eq!(verify(&alice_public(), &body, &data), Ok(true));
    assert!(
        message
            .windows(4)
            .any(|window| window == 42u32.to_be_bytes())
    );
}

#[test]
fn a_key_too_small_to_sign_fails() {
    let key = RsaKey {
        modulus: num_bigint::BigInt::from(0),
        exponent: num_bigint::BigInt::from(3),
    };
    let signing = SigningParams {
        key: &key,
        algorithm: 1,
        key_id: KEY_ID,
        hash_id: 10,
        hash: Hash::Sha512,
        created: 0,
    };
    assert!(detached_packet(&signing, b"hello").is_err());
    assert!(inline_message(&signing, 0, b"hello").is_err());
}

#[test]
fn a_key_too_small_for_its_block_fails_to_sign() {
    let key = RsaKey {
        modulus: num_bigint::BigInt::from(0xFFFF_FFFBu64),
        exponent: num_bigint::BigInt::from(0x1_0001),
    };
    let signing = SigningParams {
        key: &key,
        algorithm: 1,
        key_id: KEY_ID,
        hash_id: 10,
        hash: Hash::Sha512,
        created: 0,
    };
    assert!(detached_packet(&signing, b"hello").is_err());
}

#[test]
fn a_public_key_packet_gives_its_numbers() {
    let key = alice_public();
    let Ok((modulus, exponent, _)) = PublicKey::rsa_numbers(&first_body(ALICE_PUBLIC)) else {
        panic!("the fixture must be RSA");
    };
    assert_eq!(key.modulus.to_biguint(), Some(modulus));
    assert_eq!(key.exponent.to_biguint(), Some(exponent));
}

#[test]
fn a_public_key_packet_of_version_3_is_unsupported() {
    let mut body = first_body(ALICE_PUBLIC);
    if let Some(version) = body.first_mut() {
        *version = 3;
    }
    assert_eq!(public_rsa(&body), Err(PgpError::UnsupportedVersion(3)));
}

#[test]
fn a_public_key_that_is_not_rsa_is_unsupported() {
    let dsa = [4, 0, 0, 0, 0, 17, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1];
    assert_eq!(public_rsa(&dsa), Err(PgpError::UnsupportedAlgorithm(17)));
}

#[test]
fn a_one_pass_message_without_its_literal_or_signature_is_an_error() {
    let found = packets(&bytes(HELLO_INLINE));
    let mut only_one_pass = Vec::new();
    write_packet(&mut only_one_pass, 4, &found[0].1);
    assert!(read_inline(&only_one_pass).is_err());
    let mut without_signature = only_one_pass.clone();
    write_new_packet(&mut without_signature, 11, &found[1].1);
    assert!(read_inline(&without_signature).is_err());
}
