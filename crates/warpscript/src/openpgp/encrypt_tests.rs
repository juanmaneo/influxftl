// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of the OpenPGP encryption: the layout of the messages it makes, and the messages that
//! open to their data under the secret key of the recipient.

use std::fmt::Display;

use super::encrypt::{EncryptParams, encrypt, encrypt_with_session_key};
use super::key::read_big;
use super::packet::PacketReader;
use super::pgp_fixtures::{ALICE_PUBLIC, ALICE_SECRET};
use super::ring::read_rings;
use super::secret::rsa_private_key;
use super::sign::public_rsa;
use super::symmetric::Cipher;
use crate::encoding::hex_decode;
use crate::rsa::{Exponentiation, Hash, RsaKey};
use crate::tests::scripted;

const KEY_ID: u64 = 0x8C80_1205_4C30_1346;
const DATE: u32 = 1_500_000_000;
const DATA: &[u8] = b"hello";

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

/// The packets of a message, as their tags and bodies.
fn packets(message: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut reader = PacketReader::new(message);
    let mut out = Vec::new();
    while ok(reader.next_tag(), "the next packet").is_some() {
        let packet = ok(reader.read(), "a packet");
        out.push((packet.tag, packet.body));
    }
    out
}

/// The parameters of a message to alice, with the key ID of alice.
fn to_alice(key: &RsaKey, cipher: Cipher) -> EncryptParams<'_> {
    EncryptParams {
        key,
        key_id: KEY_ID,
        cipher,
        date: DATE,
    }
}

/// Bytes of random data that are never zero, from a fixed sequence, so that messages repeat.
fn random_bytes(length: usize) -> Vec<u8> {
    (0..length).map(|i| (i % 251 + 1) as u8).collect()
}

/// The data and the date of the literal packet of a message, opened with the private key by
/// hand: the session key of the session key packet, then the data packet under that session
/// key, with the checks of its prefix, its literal packet and its modification detection code.
fn open(message: &[u8], private: &RsaKey) -> (u32, Vec<u8>) {
    let packets = packets(message);
    let [(1, pkesk), (18, seipd)] = packets.as_slice() else {
        panic!("a message is a session key packet and a data packet");
    };
    assert_eq!(pkesk[0], 3, "the session key packet is of version 3");
    assert_eq!(pkesk[9], 1, "the session key is for RSA");
    let mut rest = &pkesk[10..];
    let number = ok(read_big(&mut rest), "the ciphertext");
    assert!(rest.is_empty(), "the session key packet ends with its MPI");
    let block = (private.modulus.bits() as usize).div_ceil(8);
    let mut ciphertext = vec![0; block];
    let bytes = number.to_bytes_be();
    ciphertext[block - bytes.len()..].copy_from_slice(&bytes);
    let exponentiation = ok(Exponentiation::new(private), "the private key");
    let info = ok(exponentiation.decrypt(&ciphertext), "the session info");
    let cipher = ok(Cipher::from_id(info[0]), "the cipher");
    let (session_key, checksum) = info[1..].split_at(info.len() - 3);
    assert_eq!(session_key.len(), cipher.key_len(), "the key of the cipher");
    let sum = session_key
        .iter()
        .fold(0u16, |sum, byte| sum.wrapping_add(u16::from(*byte)));
    assert_eq!(
        checksum,
        sum.to_be_bytes(),
        "the checksum of the session key"
    );

    assert_eq!(seipd[0], 1, "the data packet is of version 1");
    let plain = ok(cipher.crypt(session_key, &seipd[1..], false), "the data");
    let size = cipher.block_len();
    assert_eq!(
        plain[size - 2..size],
        plain[size..size + 2],
        "the prefix repeats"
    );
    let mdc = plain.len() - 22;
    assert_eq!(
        plain[mdc..mdc + 2],
        [0xD3, 0x14],
        "the modification detection code"
    );
    assert_eq!(
        plain[mdc + 2..],
        Hash::Sha1.digest(&plain[..mdc + 2]),
        "the digest of the modification detection code"
    );

    let mut reader = PacketReader::new(&plain[size + 2..mdc]);
    let literal = ok(reader.read(), "the literal packet");
    assert_eq!(literal.tag, 11, "the data is a literal packet");
    assert!(ok(reader.next_tag(), "the end of the literal").is_none());
    let body = literal.body;
    assert_eq!(body[0], b'b', "the literal is binary");
    let name = usize::from(body[1]);
    let date = u32::from_be_bytes(ok(body[2 + name..6 + name].try_into(), "the date"));
    (date, body[6 + name..].to_vec())
}

#[test]
fn a_message_is_a_session_key_packet_then_a_data_packet() {
    let key = alice_public();
    let message = ok(
        encrypt(
            &to_alice(&key, Cipher::Aes256),
            DATA,
            &mut scripted(random_bytes(2000)),
        ),
        "the message",
    );
    let packets = packets(&message);
    let tags: Vec<u8> = packets.iter().map(|(tag, _)| *tag).collect();
    assert_eq!(tags, [1, 18]);
    assert_eq!(packets[0].1[0], 3);
    assert_eq!(packets[0].1[1..9], KEY_ID.to_be_bytes());
    assert_eq!(packets[0].1[9], 1);
    assert_eq!(packets[1].1[0], 1);
}

#[test]
fn the_key_id_is_the_one_given_or_zero_when_it_is_hidden() {
    let key = alice_public();
    let mut hidden = to_alice(&key, Cipher::Aes256);
    hidden.key_id = 0;
    let message = ok(
        encrypt(&hidden, DATA, &mut scripted(random_bytes(2000))),
        "the message",
    );
    assert_eq!(packets(&message)[0].1[1..9], [0; 8]);
}

#[test]
fn a_message_opens_to_its_data_with_the_secret_key_of_the_recipient() {
    let key = alice_public();
    let message = ok(
        encrypt(
            &to_alice(&key, Cipher::Aes256),
            DATA,
            &mut scripted(random_bytes(2000)),
        ),
        "the message",
    );
    assert_eq!(open(&message, &alice_private()), (DATE, DATA.to_vec()));
}

#[test]
fn every_cipher_opens_to_its_data_at_every_block_length() {
    let key = alice_public();
    let private = alice_private();
    let long = b"influxftl symmetric cipher check, 2026! ".repeat(3);
    for cipher in ALL {
        for data in [DATA, long.as_slice(), &b""[..]] {
            let message = ok(
                encrypt(
                    &to_alice(&key, cipher),
                    data,
                    &mut scripted(random_bytes(2000)),
                ),
                "the message",
            );
            assert_eq!(
                open(&message, &private),
                (DATE, data.to_vec()),
                "{cipher:?}"
            );
        }
    }
}

#[test]
fn the_data_packet_is_one_block_of_prefix_and_its_literal_and_digest() {
    let key = alice_public();
    for cipher in ALL {
        let message = ok(
            encrypt(
                &to_alice(&key, cipher),
                DATA,
                &mut scripted(random_bytes(2000)),
            ),
            "the message",
        );
        let seipd = &packets(&message)[1].1;
        let literal = 2 + 1 + 1 + 8 + 4 + DATA.len();
        assert_eq!(
            seipd.len(),
            1 + cipher.block_len() + 2 + literal + 22,
            "{cipher:?}"
        );
    }
}

#[test]
fn a_random_source_that_is_empty_fails_to_draw_the_session_key() {
    let key = alice_public();
    let message = encrypt(&to_alice(&key, Cipher::Aes256), DATA, &mut scripted(vec![]));
    assert!(message.is_err());
}

#[test]
fn the_padding_of_the_session_key_needs_random_bytes() {
    let key = alice_public();
    let message = encrypt_with_session_key(
        &to_alice(&key, Cipher::Aes256),
        &[0; 32],
        DATA,
        &mut scripted(vec![]),
    );
    assert!(message.is_err());
}

#[test]
fn the_prefix_of_the_data_needs_random_bytes_after_the_padding() {
    let key = alice_public();
    // A 2048-bit key takes 218 bytes of padding for AES_256 and 'hello': the prefix is next.
    let message = encrypt_with_session_key(
        &to_alice(&key, Cipher::Aes256),
        &[0; 32],
        DATA,
        &mut scripted(vec![1; 218]),
    );
    assert!(message.is_err());
}

#[test]
fn a_session_key_of_another_length_than_its_cipher_fails() {
    let key = alice_public();
    let message = encrypt_with_session_key(
        &to_alice(&key, Cipher::Aes256),
        &[0; 5],
        DATA,
        &mut scripted(random_bytes(2000)),
    );
    assert!(message.is_err());
}

#[test]
fn a_key_too_small_for_the_session_info_fails() {
    let key = RsaKey {
        modulus: (num_bigint::BigInt::from(1) << 127) + 1,
        exponent: num_bigint::BigInt::from(0x1_0001),
    };
    let message = encrypt(
        &to_alice(&key, Cipher::Aes256),
        DATA,
        &mut scripted(random_bytes(2000)),
    );
    assert!(message.is_err());
}

#[test]
fn a_key_without_a_modulus_fails() {
    let key = RsaKey {
        modulus: num_bigint::BigInt::from(0),
        exponent: num_bigint::BigInt::from(0x1_0001),
    };
    let message = encrypt(
        &to_alice(&key, Cipher::Aes256),
        DATA,
        &mut scripted(random_bytes(2000)),
    );
    assert!(message.is_err());
}
