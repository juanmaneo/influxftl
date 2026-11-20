// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of the unlocking of secret keys, against the key of the fixtures and against keys that
//! the tests protect themselves with each cipher and string-to-key.

use aes::cipher::KeyInit;
use aes::{Aes128, Aes192, Aes256};
use digest::Digest;
use num_bigint::BigUint;

use super::key::{PublicKey, key_id_hex};
use super::pgp_fixtures::{ALICE_PUBLIC, ALICE_SECRET};
use super::ring::read_rings;
use super::secret::{cfb, rsa_private_key, s2k_key};
use crate::encoding::hex_decode;
use crate::error::pgp::PgpError;
use crate::rsa::RsaKey;

const PASSPHRASE: &str = "secret";

/// The bytes of a hexadecimal fixture.
fn bytes(text: &str) -> Vec<u8> {
    match hex_decode(text) {
        Ok(bytes) => bytes,
        Err(error) => panic!("the fixture must be hexadecimal: {error}"),
    }
}

/// The body of the first packet of the first ring of a fixture.
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

/// The public part of the packet of the alice key.
fn alice_public() -> Vec<u8> {
    let body = first_body(ALICE_PUBLIC);
    let Ok((_, _, rest)) = PublicKey::rsa_numbers(&body) else {
        panic!("the public key must be RSA");
    };
    body[..body.len() - rest.len()].to_vec()
}

/// The private exponent of the alice key, unlocked by the tests' first step.
fn alice_key() -> RsaKey {
    match rsa_private_key(&first_body(ALICE_SECRET), PASSPHRASE) {
        Ok(key) => key,
        Err(error) => panic!("the fixture must unlock: {error}"),
    }
}

/// An MPI of a number: its bit length, then its bytes.
fn mpi(number: &BigUint) -> Vec<u8> {
    let mut out = (number.bits() as u16).to_be_bytes().to_vec();
    out.extend(number.to_bytes_be());
    out
}

/// The private exponent of a key as an MPI, the material of a secret key.
fn exponent_material(key: &RsaKey) -> Vec<u8> {
    match key.exponent.to_biguint() {
        Some(exponent) => mpi(&exponent),
        None => panic!("the exponent must be positive"),
    }
}

/// A secret key body of usage 254 whose material (the MPIs, then their SHA-1 digest) is
/// encrypted with the cipher and the string-to-key of the arguments.
fn protected(public: &[u8], cipher: u8, hash_id: u8, count: u8, material: &[u8]) -> Vec<u8> {
    let salt = [1u8; 8];
    let iv = [9u8; 16];
    let hash = match super::sign::hash_of_id(hash_id) {
        Ok(hash) => hash,
        Err(error) => panic!("the hash must be known: {error}"),
    };
    let mut plain = material.to_vec();
    plain.extend(sha1::Sha1::digest(material));
    let encrypted = match cipher {
        7 => cfb(
            &Aes128::new(&s2k_key::<16>(hash, salt, count, PASSPHRASE).into()),
            iv,
            &plain,
            true,
        ),
        8 => cfb(
            &Aes192::new(&s2k_key::<24>(hash, salt, count, PASSPHRASE).into()),
            iv,
            &plain,
            true,
        ),
        9 => cfb(
            &Aes256::new(&s2k_key::<32>(hash, salt, count, PASSPHRASE).into()),
            iv,
            &plain,
            true,
        ),
        other => panic!("the test does not encrypt with cipher {other}"),
    };
    let mut body = public.to_vec();
    body.extend([254, cipher, 3, hash_id]);
    body.extend(salt);
    body.push(count);
    body.extend(iv);
    body.extend(encrypted);
    body
}

#[test]
fn the_fixture_key_unlocks_with_its_passphrase() {
    let key = alice_key();
    let Ok((modulus, _, _)) = PublicKey::rsa_numbers(&first_body(ALICE_SECRET)) else {
        panic!("the fixture must be RSA");
    };
    assert_eq!(key.modulus.to_biguint(), Some(modulus));
    assert_eq!(key_id_hex(0x8C80_1205_4C30_1346), "8C8012054C301346");
}

#[test]
fn a_wrong_passphrase_fails_the_checksum() {
    assert_eq!(
        rsa_private_key(&first_body(ALICE_SECRET), "wrong"),
        Err(PgpError::ChecksumMismatch)
    );
}

#[test]
fn an_unprotected_key_gives_its_exponent_as_it_is() {
    let key = alice_key();
    let mut body = alice_public();
    body.push(0);
    body.extend(exponent_material(&key));
    assert_eq!(rsa_private_key(&body, ""), Ok(key));
}

#[test]
fn a_key_protected_with_aes_192_unlocks() {
    let key = alice_key();
    let body = protected(&alice_public(), 8, 2, 0x60, &exponent_material(&key));
    let Ok(unlocked) = rsa_private_key(&body, PASSPHRASE) else {
        panic!("the AES-192 key must unlock");
    };
    assert_eq!(unlocked, key);
}

#[test]
fn a_key_protected_with_aes_256_and_sha256_unlocks() {
    let key = alice_key();
    let body = protected(&alice_public(), 9, 8, 0x60, &exponent_material(&key));
    let Ok(unlocked) = rsa_private_key(&body, PASSPHRASE) else {
        panic!("the AES-256 key must unlock");
    };
    assert_eq!(unlocked, key);
}

#[test]
fn a_material_too_short_for_its_digest_is_premature() {
    let body = protected(&alice_public(), 7, 2, 0x60, &[1, 2, 3]);
    assert_eq!(
        rsa_private_key(&body, PASSPHRASE),
        Err(PgpError::PrematureEnd)
    );
}

#[test]
fn an_unprotected_key_with_a_cut_exponent_is_premature() {
    let mut body = alice_public();
    body.extend([0, 0]);
    assert_eq!(rsa_private_key(&body, ""), Err(PgpError::PrematureEnd));
}

/// Every cut of the header of the protection fails before any key is derived, and three cuts of
/// the encrypted material fail after it (each one derives the key, which is slow in a debug build).
#[test]
fn every_cut_of_the_fixture_fails() {
    let body = first_body(ALICE_SECRET);
    let public = alice_public().len();
    let header_end = public + 29;
    let cuts = (0..=header_end).chain([body.len() - 1, body.len() - 21, body.len() - 22]);
    for cut in cuts {
        let Some(prefix) = body.get(..cut) else {
            panic!("the cut is within the body");
        };
        assert!(
            rsa_private_key(prefix, PASSPHRASE).is_err(),
            "a body cut at {cut} must fail"
        );
    }
}

#[test]
fn a_protection_the_port_does_not_read_fails() {
    let mut body = alice_public();
    body.push(1);
    assert_eq!(
        rsa_private_key(&body, PASSPHRASE),
        Err(PgpError::UnportedProtection(1))
    );
}

#[test]
fn a_string_to_key_the_port_does_not_derive_fails() {
    let mut body = alice_public();
    body.extend([254, 7, 0, 2]);
    body.extend([0u8; 8 + 1 + 16 + 32]);
    assert_eq!(
        rsa_private_key(&body, PASSPHRASE),
        Err(PgpError::UnportedS2k(0))
    );
}

#[test]
fn a_hash_of_the_string_to_key_the_port_does_not_know_fails() {
    let mut body = alice_public();
    body.extend([254, 7, 3, 12]);
    body.extend([0u8; 8 + 1 + 16 + 32]);
    assert_eq!(
        rsa_private_key(&body, PASSPHRASE),
        Err(PgpError::UnportedDigest(12))
    );
}

#[test]
fn a_cipher_the_port_does_not_decrypt_with_fails() {
    let mut body = alice_public();
    body.extend([254, 3, 3, 2]);
    body.extend([0u8; 8 + 1 + 16 + 32]);
    assert_eq!(
        rsa_private_key(&body, PASSPHRASE),
        Err(PgpError::UnportedCipher(3))
    );
}

#[test]
fn a_cfb_round_trip_keeps_partial_blocks() {
    let cipher = Aes128::new(&[7u8; 16].into());
    let iv = [3u8; 16];
    for length in [0usize, 5, 16, 37] {
        let plain: Vec<u8> = (0..length).map(|i| i as u8).collect();
        let encrypted = cfb(&cipher, iv, &plain, true);
        assert_eq!(cfb(&cipher, iv, &encrypted, false), plain);
    }
}

#[test]
fn a_key_longer_than_its_digest_repeats_the_hash_with_zeros() {
    let short = s2k_key::<20>(
        super::sign::hash_of_id(2).unwrap_or_else(|_| panic!("SHA-1")),
        [0; 8],
        0,
        "p",
    );
    let long = s2k_key::<32>(
        super::sign::hash_of_id(2).unwrap_or_else(|_| panic!("SHA-1")),
        [0; 8],
        0,
        "p",
    );
    assert_eq!(long[..20], short);
    assert_ne!(long[20..], long[..12]);
}
