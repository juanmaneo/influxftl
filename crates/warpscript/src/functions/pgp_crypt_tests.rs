// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of `PGPENCRYPT` and `PGPDECRYPT`: the round trips, the messages of Warp 10 3.5.0 under
//! each cipher, and the messages of the failures that Warp 10 gives under the same inputs.

use super::pgp_crypt::FUNCTIONS;
use crate::encoding::hex_encode;
use crate::openpgp::key::{PublicKey, key_id_hex};
use crate::openpgp::pgp_fixtures::{ALICE_PUBLIC, ALICE_SECRET, HELLO_MESSAGES};
use crate::tests::{assert_fails, fail_msg, test_stack, top};
use crate::value::Value;

/// The key ID of the alice fixture, as a hexadecimal string.
const KEY_ID: &str = "'8C8012054C301346'";

/// The bytes of the key ID of the alice fixture, as they are in a session key packet.
const KEY_ID_BYTES: [u8; 8] = [0x8C, 0x80, 0x12, 0x05, 0x4C, 0x30, 0x13, 0x46];

/// The names of the ciphers that `PGPENCRYPT` takes.
const CIPHERS: [&str; 13] = [
    "IDEA",
    "TRIPLE_DES",
    "3DES",
    "CAST5",
    "BLOWFISH",
    "DES",
    "AES_128",
    "AES_192",
    "AES_256",
    "TWOFISH",
    "CAMELLIA_128",
    "CAMELLIA_192",
    "CAMELLIA_256",
];

/// Armor that does not decode: a line of the body that starts with a dash is not armor.
const BAD_ARMOR: &str = "-----BEGIN PGP MESSAGE-----\n-\n";

/// The secret ring of the alice fixture, on the stack.
fn secret_ring() -> String {
    format!("'{ALICE_SECRET}' HEX-> PGPRING 0 GET")
}

/// The public ring of the alice fixture, on the stack.
fn public_ring() -> String {
    format!("'{ALICE_PUBLIC}' HEX-> PGPRING 0 GET")
}

/// A `PGPENCRYPT` parameter map of the alice key, with `extra` parameters after its key.
fn encrypt_map(extra: &str) -> String {
    format!(
        "{{ 'recipient' {KEY_ID} 'keyring' {} {extra} }}",
        public_ring()
    )
}

/// Encrypt `'hello'` to the alice key, then decrypt it with the alice secret key.
fn round_trip(extra: &str) -> String {
    format!(
        "'hello' {} PGPENCRYPT {} {KEY_ID} 'secret' PGPDECRYPT",
        encrypt_map(extra),
        secret_ring()
    )
}

/// The bytes of a `BYTES` value, or a panic.
fn expect_bytes(value: Value) -> Vec<u8> {
    match value {
        Value::Bytes(bytes) => bytes.to_vec(),
        other => panic!("expected BYTES, got {other:?}"),
    }
}

#[test]
fn the_functions_are_registered() {
    let names: Vec<&str> = FUNCTIONS.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, ["PGPENCRYPT", "PGPDECRYPT"]);
}

#[test]
fn an_armored_message_decrypts_to_its_content() {
    assert_eq!(top(&round_trip("")), Value::bytes(b"hello"));
}

#[test]
fn the_default_message_is_an_armored_string() {
    let script = format!("'hello' {} PGPENCRYPT", encrypt_map(""));
    let value = top(&script);
    assert!(
        value
            .as_str()
            .is_some_and(|text| text.starts_with("-----BEGIN PGP MESSAGE-----"))
    );
}

#[test]
fn a_raw_message_is_bytes_of_a_session_key_packet() {
    let script = format!("'hello' {} PGPENCRYPT", encrypt_map("'armor' false"));
    let bytes = expect_bytes(top(&script));
    assert_eq!(bytes.first(), Some(&0xC1));
    assert_eq!(bytes.get(3), Some(&3));
}

#[test]
fn an_armor_that_is_not_a_boolean_gives_bytes() {
    let script = format!("'hello' {} PGPENCRYPT", encrypt_map("'armor' 'yes'"));
    assert_eq!(expect_bytes(top(&script)).first(), Some(&0xC1));
}

#[test]
fn the_key_id_is_hidden_by_default() {
    let script = format!("'hello' {} PGPENCRYPT", encrypt_map("'armor' false"));
    let bytes = expect_bytes(top(&script));
    assert_eq!(bytes.get(4..12), Some(&[0u8; 8][..]));
}

#[test]
fn the_key_id_is_written_when_throw_keyid_is_false() {
    for flag in ["false", "'yes'"] {
        let script = format!(
            "'hello' {} PGPENCRYPT",
            encrypt_map(&format!("'armor' false 'throw_keyid' {flag}"))
        );
        let bytes = expect_bytes(top(&script));
        assert_eq!(
            bytes.get(4..12),
            Some(&KEY_ID_BYTES[..]),
            "throw_keyid {flag}"
        );
    }
}

#[test]
fn every_cipher_round_trips() {
    for cipher in CIPHERS {
        let script = round_trip(&format!("'algorithm' '{cipher}'"));
        assert_eq!(top(&script), Value::bytes(b"hello"), "cipher {cipher}");
    }
}

#[test]
fn warp_10_messages_decrypt_under_each_cipher() {
    for (cipher, hex) in HELLO_MESSAGES {
        let script = format!(
            "'{hex}' HEX-> {} {KEY_ID} 'secret' PGPDECRYPT",
            secret_ring()
        );
        assert_eq!(top(&script), Value::bytes(b"hello"), "cipher {cipher}");
    }
}

#[test]
fn bytes_content_round_trips() {
    let script = format!(
        "'68656c6c6f' HEX-> {} PGPENCRYPT {} {KEY_ID} 'secret' PGPDECRYPT",
        encrypt_map(""),
        secret_ring()
    );
    assert_eq!(top(&script), Value::bytes(b"hello"));
}

#[test]
fn a_message_decrypts_with_the_key_id_as_a_number() {
    let key_id = 0x8C80_1205_4C30_1346u64 as i64;
    let script = format!(
        "'hello' {} PGPENCRYPT {} {key_id} 'secret' PGPDECRYPT",
        encrypt_map("'armor' false"),
        secret_ring()
    );
    assert_eq!(top(&script), Value::bytes(b"hello"));
}

#[test]
fn a_message_to_a_subkey_decrypts_with_the_subkey() {
    let script = format!(
        "'hello' {{ 'recipient' '7BDFE3E20D4709CD' 'keyring' {} }} PGPENCRYPT {} '7BDFE3E20D4709CD' 'secret' PGPDECRYPT",
        public_ring(),
        secret_ring()
    );
    assert_eq!(top(&script), Value::bytes(b"hello"));
}

#[test]
fn a_recipient_that_is_a_public_key_needs_no_keyring() {
    let script = format!(
        "'hello' {{ 'recipient' {} PGPINFO {KEY_ID} GET 'pubkey' GET 'armor' false }} PGPENCRYPT {} {KEY_ID} 'secret' PGPDECRYPT",
        public_ring(),
        secret_ring()
    );
    assert_eq!(top(&script), Value::bytes(b"hello"));
}

#[test]
fn an_unknown_algorithm_is_refused_with_its_name() {
    assert_fails(
        &format!("'hello' {} PGPENCRYPT", encrypt_map("'algorithm' 'AES128'")),
        "PGPENCRYPT error while encrypting data. (Invalid encryption algorithm 'AES128'.)",
    );
}

#[test]
fn an_algorithm_that_is_null_or_a_number_is_named_as_java_writes_it() {
    assert_fails(
        &format!("'hello' {} PGPENCRYPT", encrypt_map("'algorithm' NULL")),
        "Invalid encryption algorithm 'null'.",
    );
    assert_fails(
        &format!("'hello' {} PGPENCRYPT", encrypt_map("'algorithm' 5")),
        "Invalid encryption algorithm '5'.",
    );
}

#[test]
fn safer_is_a_name_with_no_cipher() {
    assert_fails(
        &format!("'hello' {} PGPENCRYPT", encrypt_map("'algorithm' 'SAFER'")),
        "cannot recognise cipher",
    );
}

#[test]
fn a_recipient_that_is_hexadecimal_with_a_prefix_is_refused() {
    assert_fails(
        &format!(
            "'hello' {{ 'recipient' '0x8C8012054C301346' 'keyring' {} }} PGPENCRYPT",
            public_ring()
        ),
        "invalid characters encountered in Hex string",
    );
}

#[test]
fn the_encryption_failures_are_reported_in_order() {
    assert_fails(
        "'hello' 'x' PGPENCRYPT",
        "PGPENCRYPT expected a parameter MAP.",
    );
    assert_fails(
        "'hello' {} PGPENCRYPT",
        "missing recipient PGP public key or key ring and key id.",
    );
    assert_fails(
        "'hello' { 'recipient' 1 } PGPENCRYPT",
        "missing PGP secret key ring.",
    );
    assert_fails(
        "'hello' { 'recipient' 1 'keyring' 'x' } PGPENCRYPT",
        "missing PGP secret key ring.",
    );
    assert_fails(
        &format!(
            "'hello' {{ 'recipient' 1 'keyring' {} }} PGPENCRYPT",
            public_ring()
        ),
        "key with id 0x1 not found.",
    );
    assert_fails(
        &format!("5 {} PGPENCRYPT", encrypt_map("")),
        "PGPENCRYPT expects data to encrypt to be either STRING or BYTES.",
    );
}

#[test]
fn the_decryption_failures_are_reported_in_order() {
    let secret = secret_ring();
    assert_fails(
        &format!("'x' {secret} {KEY_ID} 5 PGPDECRYPT"),
        "missing passphrase.",
    );
    assert_fails(
        &format!("'x' {secret} TRUE 'secret' PGPDECRYPT"),
        "missing PGP secret key id.",
    );
    assert_fails(
        &format!("'x' {} {KEY_ID} 'secret' PGPDECRYPT", public_ring()),
        "missing PGP secret key ring.",
    );
    assert_fails(
        &format!("'x' 5 {KEY_ID} 'secret' PGPDECRYPT"),
        "missing PGP secret key ring.",
    );
    assert_fails(
        &format!("'x' {secret} 1 'secret' PGPDECRYPT"),
        "key with id 0x1 not found.",
    );
    assert_fails(
        &format!("'x' {secret} {KEY_ID} 'wrong' PGPDECRYPT"),
        "unable to extract private key.",
    );
    assert_fails(
        &format!("5 {secret} {KEY_ID} 'secret' PGPDECRYPT"),
        "expected encrypted content as STRING or BYTES.",
    );
}

#[test]
fn a_content_that_is_not_a_message_is_an_error_of_decryption() {
    assert_fails(
        &format!("'00' HEX-> {} {KEY_ID} 'secret' PGPDECRYPT", secret_ring()),
        "PGPDECRYPT error during decryption.",
    );
}

#[test]
fn an_empty_armor_holds_no_packet() {
    assert_fails(
        &format!("'!!!' {} {KEY_ID} 'secret' PGPDECRYPT", secret_ring()),
        "unexpected packet in stream: null",
    );
}

#[test]
fn armor_that_does_not_decode_is_refused() {
    let mut stack = test_stack();
    stack.push(Value::string(BAD_ARMOR));
    let script = format!("{} {KEY_ID} 'secret' PGPDECRYPT", secret_ring());
    let message = match stack.exec_checked(&script) {
        Ok(()) => panic!("script did not fail:\n{script}"),
        Err(error) => error.to_string(),
    };
    assert!(message.contains("invalid armor"), "{message}");
}

#[test]
fn a_recipient_of_another_algorithm_is_refused() {
    let body = [
        4, 0, 0, 0, 0, 17, 0, 4, 0x0B, 0, 4, 0x0D, 0, 4, 0x05, 0, 4, 0x07,
    ];
    let Ok((key, _)) = PublicKey::parse(&body) else {
        panic!("the DSA key parses");
    };
    let mut packet = vec![0xC6, body.len() as u8];
    packet.extend(body);
    let ring = format!("'{}' HEX-> PGPRING 0 GET", hex_encode(&packet));
    assert_fails(
        &format!(
            "'hello' {{ 'recipient' '{}' 'keyring' {ring} }} PGPENCRYPT",
            key_id_hex(key.key_id)
        ),
        "unsupported PGP public key algorithm: 17",
    );
}

#[test]
fn a_stack_without_its_operands_fails() {
    let scripts = [
        "PGPENCRYPT".to_owned(),
        format!(
            "{{ 'recipient' {KEY_ID} 'keyring' {} }} PGPENCRYPT",
            public_ring()
        ),
        "PGPDECRYPT".to_owned(),
        "'secret' PGPDECRYPT".to_owned(),
        format!("{KEY_ID} 'secret' PGPDECRYPT"),
        format!("{} {KEY_ID} 'secret' PGPDECRYPT", secret_ring()),
    ];
    for script in scripts {
        let message = fail_msg(&script);
        assert!(message.contains("Empty stack."), "{script}: {message}");
    }
}

#[test]
fn a_key_id_with_a_prefix_is_refused() {
    assert_fails(
        &format!("'x' {} '0x8C80' 'secret' PGPDECRYPT", secret_ring()),
        "invalid characters encountered in Hex string",
    );
}
