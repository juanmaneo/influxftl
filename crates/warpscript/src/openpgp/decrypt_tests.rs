// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of the OpenPGP decryption: the messages that Warp 10 makes open to their data under the
//! secret key of the recipient, and the messages that fail, with the failure that Warp 10 gives.

use std::fmt::Display;
use std::io::Write;

use num_bigint::{BigInt, BigUint};

use super::decrypt::decrypt;
use super::encrypt::{EncryptParams, encrypt};
use super::packet::write_new_packet;
use super::pgp_fixtures::{ALICE_PUBLIC, ALICE_SECRET, HELLO_BZIP2, HELLO_MESSAGES};
use super::ring::read_rings;
use super::secret::rsa_private_key;
use super::sign::{LITERAL, literal, public_rsa, write_mpi};
use super::symmetric::Cipher;
use crate::encoding::hex_decode;
use crate::error::crypto::RsaError;
use crate::error::pgp::PgpError;
use crate::rsa::{Exponentiation, Hash, RsaKey};
use crate::tests::scripted;

const KEY_ID: u64 = 0x8C80_1205_4C30_1346;
const DATE: u32 = 1_500_000_000;
const DATA: &[u8] = b"hello";
const PKESK: u8 = 1;
const SIGNATURE: u8 = 2;
const SED: u8 = 9;
const COMPRESSED: u8 = 8;
const SEIPD: u8 = 18;

const ALL: [Cipher; 12] = [
    Cipher::Idea,
    Cipher::TripleDes,
    Cipher::Cast5,
    Cipher::Blowfish,
    Cipher::Des,
    Cipher::Aes128,
    Cipher::Aes192,
    Cipher::Aes256,
    Cipher::Twofish,
    Cipher::Camellia128,
    Cipher::Camellia192,
    Cipher::Camellia256,
];

/// The value of a result that must be a success.
fn ok<T, E: Display>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{what} must succeed: {error}"),
    }
}

/// The error of a result that must be a failure.
fn failure(result: Result<Vec<u8>, PgpError>, what: &str) -> PgpError {
    match result {
        Ok(data) => panic!("{what} must fail, but opens to {data:?}"),
        Err(error) => error,
    }
}

/// The body of the first packet of the first key of a key ring fixture.
fn first_body(text: &str) -> Vec<u8> {
    let rings = ok(read_rings(&ok(hex_decode(text), "the fixture")), "the ring");
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
    ok(public_rsa(&first_body(ALICE_PUBLIC)), "the public key")
}

/// The RSA private key of the alice fixture.
fn alice_private() -> RsaKey {
    ok(
        rsa_private_key(&first_body(ALICE_SECRET), "secret"),
        "the private key",
    )
}

/// Bytes of random data that are never zero, from a fixed sequence.
fn random_bytes(length: usize) -> Vec<u8> {
    (0..length).map(|i| (i % 251 + 1) as u8).collect()
}

/// The session key of a cipher of `length` bytes, which is the one of the messages the tests make.
fn session_key(length: usize) -> Vec<u8> {
    (0..length).map(|i| (i * 7 + 3) as u8).collect()
}

/// The checksum of a session key: the sum of its bytes modulo 65536.
fn checksum(key: &[u8]) -> u16 {
    key.iter()
        .fold(0u16, |sum, byte| sum.wrapping_add(u16::from(*byte)))
}

/// The session info of a cipher, as its identifier, its session key, and the checksum of the key.
fn session_info(id: u8, key: &[u8]) -> Vec<u8> {
    let mut info = vec![id];
    info.extend(key);
    info.extend(checksum(key).to_be_bytes());
    info
}

/// The packet of a tag and a body, in the new format.
fn packet(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    write_new_packet(&mut out, tag, body);
    out
}

/// The body of a session key packet of version 3 for RSA, which holds `info` encrypted to `public`.
fn pkesk_body(info: &[u8], public: &RsaKey) -> Vec<u8> {
    let key = ok(Exponentiation::new(public), "the public key");
    let ciphertext = ok(
        key.encrypt_random(info, &mut scripted(random_bytes(2000))),
        "the session info",
    );
    let mut body = vec![3];
    body.extend(KEY_ID.to_be_bytes());
    body.push(1);
    write_mpi(&mut body, &BigUint::from_bytes_be(&ciphertext));
    body
}

/// The session key packet of `info`, encrypted to alice.
fn pkesk(info: &[u8]) -> Vec<u8> {
    packet(PKESK, &pkesk_body(info, &alice_public()))
}

/// The session key packet of the AES-256 session key of the tests, to alice.
fn good_key() -> Vec<u8> {
    pkesk(&session_info(9, &session_key(32)))
}

/// The body of a data packet under a session key: the version, then the cipher in CFB mode over
/// a random prefix, the inner packets, and the modification detection code, which `tamper` makes
/// wrong.
fn protected(cipher: Cipher, key: &[u8], inner: &[u8], tamper: bool) -> Vec<u8> {
    let block = cipher.block_len();
    let mut plain = random_bytes(block);
    let repeated = [plain[block - 2], plain[block - 1]];
    plain.extend(repeated);
    plain.extend(inner);
    let mut trailer = vec![0xD3, 0x14];
    let mut digested = plain.clone();
    digested.extend(&trailer);
    trailer.extend(Hash::Sha1.digest(&digested));
    if tamper {
        let last = trailer.len() - 1;
        trailer[last] ^= 1;
    }
    plain.extend(trailer);
    let mut body = vec![1];
    body.extend(ok(cipher.crypt(key, &plain, true), "the data"));
    body
}

/// The data packet of some inner packets under the AES-256 session key of the tests.
fn seipd(inner: &[u8]) -> Vec<u8> {
    packet(
        SEIPD,
        &protected(Cipher::Aes256, &session_key(32), inner, false),
    )
}

/// A message of some inner packets, to alice, under the session key of the tests.
fn wrap(inner: &[u8]) -> Vec<u8> {
    [good_key(), seipd(inner)].concat()
}

/// The data of a message, opened with the private key of alice.
fn open(message: &[u8]) -> Result<Vec<u8>, PgpError> {
    decrypt(message, &alice_private())
}

/// The literal packet of some data, dated like the messages of the tests.
fn literal_packet(data: &[u8]) -> Vec<u8> {
    packet(LITERAL, &literal(DATE, data))
}

/// The compressed packet of an algorithm and some data that it compresses.
fn compressed(algorithm: u8, data: &[u8]) -> Vec<u8> {
    packet(COMPRESSED, &[&[algorithm], data].concat())
}

/// The raw deflate of some data, as ZIP compresses it.
fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    ok(encoder.write_all(data), "the deflate");
    ok(encoder.finish(), "the deflate")
}

/// The zlib stream of some data, as ZLIB compresses it.
fn zlib(data: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    ok(encoder.write_all(data), "the zlib");
    ok(encoder.finish(), "the zlib")
}

#[test]
fn the_messages_of_warp_10_open_to_hello() {
    let private = alice_private();
    for (name, hex) in HELLO_MESSAGES {
        let message = ok(hex_decode(hex), name);
        assert_eq!(ok(decrypt(&message, &private), name), DATA, "{name}");
    }
}

#[test]
fn the_messages_of_the_port_open_under_each_cipher() {
    let public = alice_public();
    let private = alice_private();
    for cipher in ALL {
        let params = EncryptParams {
            key: &public,
            key_id: KEY_ID,
            cipher,
            date: DATE,
        };
        let message = ok(
            encrypt(&params, DATA, &mut scripted(random_bytes(2000))),
            "the message",
        );
        assert_eq!(
            ok(decrypt(&message, &private), "the data"),
            DATA,
            "{cipher:?}"
        );
    }
}

#[test]
fn data_of_many_blocks_opens() {
    let public = alice_public();
    let data = vec![0x5A; 1000];
    let params = EncryptParams {
        key: &public,
        key_id: KEY_ID,
        cipher: Cipher::Aes256,
        date: DATE,
    };
    let message = ok(
        encrypt(&params, &data, &mut scripted(random_bytes(2000))),
        "the message",
    );
    assert_eq!(ok(decrypt(&message, &alice_private()), "the data"), data);
}

#[test]
fn compressed_content_opens_under_each_algorithm() {
    let bzip2 = ok(hex_decode(HELLO_BZIP2), "the bzip2 fixture");
    let cases = [
        (0, literal_packet(DATA)),
        (1, deflate(&literal_packet(DATA))),
        (2, zlib(&literal_packet(DATA))),
        (3, bzip2),
    ];
    for (algorithm, data) in cases {
        let message = wrap(&compressed(algorithm, &data));
        assert_eq!(
            ok(open(&message), "the compressed data"),
            DATA,
            "algorithm {algorithm}"
        );
    }
}

#[test]
fn corrupt_compressed_data_is_a_decompression_failure() {
    let cases = [(1, vec![0xFF; 8]), (2, DATA.to_vec()), (3, DATA.to_vec())];
    for (algorithm, data) in cases {
        let error = failure(open(&wrap(&compressed(algorithm, &data))), "the data");
        assert!(
            matches!(error, PgpError::Decompression(_)),
            "algorithm {algorithm}: {error}"
        );
    }
}

#[test]
fn an_unknown_compression_algorithm_is_named() {
    let message = wrap(&compressed(4, DATA));
    assert_eq!(
        failure(open(&message), "the data"),
        PgpError::UnknownCompression(4)
    );
}

#[test]
fn content_that_is_not_one_literal_packet_has_no_data() {
    let nested = compressed(0, &compressed(0, &literal_packet(DATA)));
    let signature = packet(SIGNATURE, b"sig");
    for inner in [Vec::new(), signature, nested] {
        assert_eq!(
            failure(open(&wrap(&inner)), "the content"),
            PgpError::NoEncryptedContent
        );
    }
    for algorithm in [1, 2] {
        assert_eq!(
            failure(
                open(&wrap(&compressed(algorithm, &[]))),
                "the empty content"
            ),
            PgpError::NoEncryptedContent,
            "algorithm {algorithm}"
        );
    }
}

#[test]
fn garbage_content_is_an_invalid_header() {
    assert_eq!(
        failure(open(&wrap(&[0x00])), "the content"),
        PgpError::InvalidHeader
    );
    assert_eq!(
        failure(
            open(&wrap(&compressed(0, &[0x00]))),
            "the compressed content"
        ),
        PgpError::InvalidHeader
    );
}

#[test]
fn packets_cut_short_in_the_content_are_premature_ends() {
    assert_eq!(
        failure(open(&wrap(&[0xCB, 0x40])), "the literal packet"),
        PgpError::PrematureEnd
    );
    assert_eq!(
        failure(open(&wrap(&[0xC8, 0x40])), "the compressed packet"),
        PgpError::PrematureEnd
    );
    assert_eq!(
        failure(
            open(&wrap(&packet(COMPRESSED, &[]))),
            "the compressed packet"
        ),
        PgpError::PrematureEnd
    );
    for body in [vec![b'b', 8, b'_'], vec![b'b', 1, b'x', 0, 0, 0]] {
        assert_eq!(
            failure(open(&wrap(&packet(LITERAL, &body))), "the literal packet"),
            PgpError::PrematureEnd
        );
    }
}

#[test]
fn the_first_literal_packet_is_the_data_and_what_follows_it_is_ignored() {
    let first = [literal_packet(b"first"), literal_packet(DATA)].concat();
    assert_eq!(ok(open(&wrap(&first)), "the data"), b"first".to_vec());
    let trailing = [literal_packet(DATA), vec![0xFF, 0x00]].concat();
    assert_eq!(ok(open(&wrap(&trailing)), "the data"), DATA);
}

#[test]
fn a_tampered_modification_detection_code_still_opens() {
    let tampered = packet(
        SEIPD,
        &protected(
            Cipher::Aes256,
            &session_key(32),
            &literal_packet(DATA),
            true,
        ),
    );
    let message = [good_key(), tampered].concat();
    assert_eq!(ok(open(&message), "the data"), DATA);
}

#[test]
fn the_first_session_key_of_version_3_is_the_one_used() {
    let wrong = pkesk(&session_info(9, &[1; 32]));
    let right = good_key();
    let data = literal_packet(DATA);
    let wrong_first = [wrong.clone(), right.clone(), seipd(&data)].concat();
    assert!(
        open(&wrong_first).is_err(),
        "a wrong first session key fails"
    );
    let right_first = [right, wrong, seipd(&data)].concat();
    assert_eq!(ok(open(&right_first), "the data"), DATA);
}

#[test]
fn only_session_keys_of_version_3_are_read() {
    for version in [2, 4] {
        let mut body = pkesk_body(&session_info(9, &session_key(32)), &alice_public());
        body[0] = version;
        let message = [packet(PKESK, &body), seipd(&literal_packet(DATA))].concat();
        assert_eq!(
            failure(open(&message), "the message"),
            PgpError::NoSessionKey,
            "version {version}"
        );
    }
}

#[test]
fn data_packets_of_another_version_or_too_short_fail() {
    let cases = [
        (vec![2, 0, 0], PgpError::UnsupportedVersion(2)),
        (vec![], PgpError::PrematureEnd),
        (vec![1, 1, 2, 3], PgpError::PrematureEnd),
    ];
    for (body, expected) in cases {
        let message = [good_key(), packet(SEIPD, &body)].concat();
        assert_eq!(
            failure(open(&message), "the data packet"),
            expected,
            "{body:?}"
        );
    }
}

#[test]
fn an_elgamal_session_key_is_unsupported() {
    let mut body = vec![3];
    body.extend(KEY_ID.to_be_bytes());
    body.push(16);
    write_mpi(&mut body, &BigUint::from(1u8));
    let message = [packet(PKESK, &body), seipd(&literal_packet(DATA))].concat();
    assert_eq!(
        failure(open(&message), "the message"),
        PgpError::UnsupportedAlgorithm(16)
    );
}

#[test]
fn messages_without_packets_or_session_keys_or_data_fail() {
    assert_eq!(
        failure(open(&[]), "the empty message"),
        PgpError::UnexpectedNull
    );
    assert_eq!(
        failure(open(&good_key()), "a session key alone"),
        PgpError::UnexpectedNull
    );
    let signature = [good_key(), packet(SIGNATURE, b"sig")].concat();
    assert_eq!(
        failure(open(&signature), "the message"),
        PgpError::NoEncryptedContent
    );
    let symmetric = [good_key(), packet(SED, b"data")].concat();
    assert_eq!(
        failure(open(&symmetric), "the message"),
        PgpError::UnsupportedPacket(SED)
    );
    assert_eq!(failure(open(&[0x00]), "garbage"), PgpError::InvalidHeader);
    let garbage = [good_key(), vec![0x00]].concat();
    assert_eq!(
        failure(open(&garbage), "the message"),
        PgpError::InvalidHeader
    );
}

#[test]
fn truncated_packets_are_premature_ends() {
    let mut key_id = vec![3];
    key_id.extend(KEY_ID.to_be_bytes());
    let mut no_mpi = key_id.clone();
    no_mpi.push(1);
    for body in [vec![3], key_id, no_mpi] {
        let message = [packet(PKESK, &body), seipd(&literal_packet(DATA))].concat();
        assert_eq!(
            failure(open(&message), "the message"),
            PgpError::PrematureEnd
        );
    }
    assert_eq!(
        failure(open(&[0xC1, 0x10]), "the session key packet"),
        PgpError::PrematureEnd
    );
    let cut = [good_key(), vec![0xD2, 0x10]].concat();
    assert_eq!(
        failure(open(&cut), "the data packet"),
        PgpError::PrematureEnd
    );
}

#[test]
fn a_session_info_must_have_a_good_checksum_before_its_cipher_is_read() {
    let data = seipd(&literal_packet(DATA));
    let key = session_key(32);
    let mut bad_sum = session_info(9, &key);
    let last = bad_sum.len() - 1;
    bad_sum[last] ^= 1;
    let cases = [
        (vec![], PgpError::SessionChecksum),
        (vec![9], PgpError::SessionChecksum),
        (vec![9, 0], PgpError::SessionChecksum),
        (vec![9, 0, 1], PgpError::SessionChecksum),
        (bad_sum, PgpError::SessionChecksum),
        (vec![99, 1, 0, 2], PgpError::SessionChecksum),
        (vec![99, 1, 0, 1], PgpError::UnrecognisedCipher),
        (vec![5, 1, 0, 1], PgpError::UnrecognisedCipher),
    ];
    for (info, expected) in cases {
        let message = [pkesk(&info), data.clone()].concat();
        assert_eq!(failure(open(&message), "the message"), expected, "{info:?}");
    }
}

#[test]
fn a_session_key_of_the_wrong_length_fails_for_its_cipher() {
    let key = session_key(16);
    let info = session_info(9, &key);
    let protected_body = packet(
        SEIPD,
        &protected(Cipher::Aes128, &key, &literal_packet(DATA), false),
    );
    let message = [pkesk(&info), protected_body].concat();
    assert_eq!(
        failure(open(&message), "the message"),
        PgpError::SessionKeyLength {
            length: 16,
            expected: 32
        }
    );
}

#[test]
fn a_session_key_is_opened_only_by_the_private_key() {
    let error = failure(
        decrypt(&wrap(&literal_packet(DATA)), &alice_public()),
        "the public key as private",
    );
    assert!(matches!(error, PgpError::SessionInfo(_)), "{error}");
}

#[test]
fn a_private_key_without_modulus_is_an_rsa_error() {
    let key = RsaKey {
        modulus: BigInt::from(0),
        exponent: BigInt::from(65537),
    };
    assert_eq!(
        failure(decrypt(&wrap(&literal_packet(DATA)), &key), "the message"),
        PgpError::Rsa(RsaError::ModulusTooSmall { bits: 0 })
    );
}
