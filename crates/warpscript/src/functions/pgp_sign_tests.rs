// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of `PGPSIGN` and `PGPVERIFY`: the round trips, the signatures of Warp 10 3.5.0, and the
//! messages of the failures that Warp 10 gives under the same inputs.

use super::pgp_sign::FUNCTIONS;
use crate::encoding::{hex_decode, hex_encode};
use crate::openpgp::armor;
use crate::openpgp::key::{PublicKey, key_id_hex};
use crate::openpgp::pgp_fixtures::{ALICE_PUBLIC, ALICE_SECRET, HELLO_DETACHED, HELLO_INLINE};
use crate::tests::{assert_fails, exec, fail_msg, run, test_stack, top};
use crate::value::Value;

/// The key ID of the alice fixture, as a hexadecimal string.
const KEY_ID: &str = "'8C8012054C301346'";

/// The secret ring of the alice fixture, on the stack.
fn secret_ring() -> String {
    format!("'{ALICE_SECRET}' HEX-> PGPRING 0 GET")
}

/// The public ring of the alice fixture, on the stack.
fn public_ring() -> String {
    format!("'{ALICE_PUBLIC}' HEX-> PGPRING 0 GET")
}

/// A `PGPSIGN` parameter map of the alice key, with `extra` parameters after its key.
fn sign_map(extra: &str) -> String {
    format!(
        "{{ 'passphrase' 'secret' 'keyring' {} 'keyid' {KEY_ID} {extra} }}",
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
    assert_eq!(names, ["PGPSIGN", "PGPVERIFY"]);
}

#[test]
fn a_detached_signature_verifies_with_its_public_key() {
    let script = format!(
        "'hello' DUP {} PGPSIGN {} {KEY_ID} true PGPVERIFY",
        sign_map("'armor' false"),
        public_ring()
    );
    assert_eq!(top(&script), Value::Boolean(true));
}

#[test]
fn bytes_content_signs_and_verifies() {
    let script = format!(
        "'68656c6c6f' HEX-> DUP {} PGPSIGN {} {KEY_ID} true PGPVERIFY",
        sign_map("'armor' false"),
        public_ring()
    );
    assert_eq!(top(&script), Value::Boolean(true));
}

#[test]
fn a_signature_verifies_with_the_secret_ring_too() {
    let script = format!(
        "'hello' DUP {} PGPSIGN {} {KEY_ID} true PGPVERIFY",
        sign_map("'armor' false"),
        secret_ring()
    );
    assert_eq!(top(&script), Value::Boolean(true));
}

#[test]
fn a_signature_of_other_content_does_not_verify() {
    let script = format!(
        "'hellx' 'hello' {} PGPSIGN {} {KEY_ID} true PGPVERIFY",
        sign_map("'armor' false"),
        public_ring()
    );
    let values = run(&script);
    assert_eq!(values.last(), Some(&Value::Boolean(false)));
}

#[test]
fn the_default_signature_is_detached_and_armored() {
    let script = format!("'hello' {} PGPSIGN", sign_map(""));
    let bytes = expect_bytes(top(&script));
    assert!(bytes.starts_with(b"-----BEGIN PGP SIGNATURE-----"));
}

#[test]
fn a_signature_that_is_not_a_boolean_is_inline_and_raw() {
    let script = format!("'hello' {} PGPSIGN", sign_map("'detached' 1 'armor' 1"));
    let bytes = expect_bytes(top(&script));
    assert_eq!(bytes.first(), Some(&0x90));
}

#[test]
fn an_inline_message_verifies_and_gives_its_content() {
    let script = format!(
        "'hello' {} PGPSIGN {} {KEY_ID} PGPVERIFY",
        sign_map("'detached' false 'armor' false"),
        public_ring()
    );
    assert_eq!(
        run(&script),
        vec![Value::bytes(b"hello"), Value::Boolean(true)]
    );
}

#[test]
fn an_inline_message_carries_its_date_in_seconds() {
    let script = format!(
        "'hello' {} PGPSIGN",
        sign_map("'detached' false 'armor' false 'date' 1500000000000000")
    );
    let bytes = expect_bytes(top(&script));
    let name = b"_CONSOLE";
    let Some(at) = bytes.windows(name.len()).position(|window| window == name) else {
        panic!("the literal is named _CONSOLE");
    };
    let date = bytes.get(at + name.len()..at + name.len() + 4);
    assert_eq!(date, Some(&1_500_000_000u32.to_be_bytes()[..]));
}

#[test]
fn a_date_that_is_not_a_number_is_zero() {
    let script = format!(
        "'hello' {} PGPSIGN",
        sign_map("'detached' false 'armor' false 'date' '1000'")
    );
    let bytes = expect_bytes(top(&script));
    let name = b"_CONSOLE";
    let Some(at) = bytes.windows(name.len()).position(|window| window == name) else {
        panic!("the literal is named _CONSOLE");
    };
    let date = bytes.get(at + name.len()..at + name.len() + 4);
    assert_eq!(date, Some(&[0u8; 4][..]));
}

#[test]
fn a_digest_that_is_not_a_name_is_reported_unknown() {
    let script = format!("'hello' {} PGPSIGN", sign_map("'digest' TRUE"));
    assert_fails(&script, "unable to map  to a hash id");
}

#[test]
fn a_keyid_that_is_a_number_selects_the_key() {
    let key_id = 0x8C80_1205_4C30_1346u64 as i64;
    let script = format!(
        "'hello' DUP {{ 'passphrase' 'secret' 'keyring' {} 'keyid' {key_id} 'armor' false }} PGPSIGN {} {key_id} true PGPVERIFY",
        secret_ring(),
        public_ring()
    );
    assert_eq!(top(&script), Value::Boolean(true));
}

#[test]
fn a_signature_of_warp_10_verifies() {
    let signature = format!("'{HELLO_DETACHED}' HEX->");
    let script = format!(
        "'hello' {signature} {} {KEY_ID} true PGPVERIFY",
        public_ring()
    );
    assert_eq!(top(&script), Value::Boolean(true));
}

#[test]
fn a_one_pass_message_of_warp_10_verifies() {
    let script = format!(
        "'{HELLO_INLINE}' HEX-> {} {KEY_ID} PGPVERIFY",
        public_ring()
    );
    assert_eq!(
        run(&script),
        vec![Value::bytes(b"hello"), Value::Boolean(true)]
    );
}

#[test]
fn a_tampered_signature_of_warp_10_does_not_verify() {
    let mut bytes = match hex_decode(HELLO_DETACHED) {
        Ok(bytes) => bytes,
        Err(error) => panic!("the fixture is hexadecimal: {error}"),
    };
    if let Some(last) = bytes.last_mut() {
        *last ^= 1;
    }
    let hex = crate::encoding::hex_encode(&bytes);
    let script = format!(
        "'hello' '{hex}' HEX-> {} {KEY_ID} true PGPVERIFY",
        public_ring()
    );
    assert_eq!(top(&script), Value::Boolean(false));
}

#[test]
fn an_armored_detached_signature_verifies_from_a_string() {
    let Ok(sig) = hex_decode(HELLO_DETACHED) else {
        panic!("the fixture is hexadecimal");
    };
    let armored = armor::encode(&sig);
    let mut stack = test_stack();
    stack.push(Value::string("hello"));
    stack.push(Value::string(armored));
    if let Err(error) = stack.exec_checked(&format!("{} {KEY_ID} true PGPVERIFY", public_ring())) {
        panic!("the verification must run: {error}");
    }
    assert_eq!(stack.elements().last(), Some(&Value::Boolean(true)));
}

#[test]
fn an_armored_message_verifies_from_a_string() {
    let Ok(message) = hex_decode(HELLO_INLINE) else {
        panic!("the fixture is hexadecimal");
    };
    let armored = armor::encode(&message);
    let mut stack = test_stack();
    stack.push(Value::string(armored));
    if let Err(error) = stack.exec_checked(&format!("{} {KEY_ID} PGPVERIFY", public_ring())) {
        panic!("the verification must run: {error}");
    }
    assert_eq!(
        stack.elements().to_vec(),
        vec![Value::bytes(b"hello"), Value::Boolean(true)]
    );
}

#[test]
fn a_key_of_the_ring_can_be_given_directly() {
    let script = format!(
        "'hello' '{HELLO_DETACHED}' HEX-> {} PGPINFO {KEY_ID} GET 'pubkey' GET true PGPVERIFY",
        public_ring()
    );
    assert_eq!(top(&script), Value::Boolean(true));
}

#[test]
fn a_hexadecimal_key_id_that_is_not_hex_is_a_decoding_error() {
    assert_fails(
        &format!(
            "'hello' '{HELLO_DETACHED}' HEX-> {} 'zz' true PGPVERIFY",
            public_ring()
        ),
        "exception decoding Hex string",
    );
    assert_fails(
        &format!("'hello' {} 'zz' PGPVERIFY", public_ring()),
        "exception decoding Hex string",
    );
    assert_fails(
        &format!(
            "'hello' {} PGPSIGN",
            sign_map("'armor' false").replace(KEY_ID, "'zz'")
        ),
        "exception decoding Hex string",
    );
}

#[test]
fn the_failures_of_pgpsign_are_those_of_warp_10() {
    let secret = secret_ring();
    assert_fails("'hello' 'x' PGPSIGN", "expected a parameter MAP.");
    assert_fails(
        &format!("'hello' {{ 'keyring' {secret} 'keyid' {KEY_ID} }} PGPSIGN"),
        "missing PGP secret key passphrase.",
    );
    assert_fails(
        &format!("'hello' {{ 'passphrase' 'secret' 'keyid' {KEY_ID} }} PGPSIGN"),
        "expected a PGP secret key ring.",
    );
    assert_fails(
        &format!(
            "'hello' {{ 'passphrase' 'secret' 'keyring' {} 'keyid' {KEY_ID} }} PGPSIGN",
            public_ring()
        ),
        "expected a PGP secret key ring.",
    );
    assert_fails(
        &format!("'hello' {{ 'passphrase' 'secret' 'keyring' {secret} }} PGPSIGN"),
        "missing PGP secret key id.",
    );
    assert_fails(
        &format!("'hello' {{ 'passphrase' 'secret' 'keyring' {secret} 'keyid' 1 }} PGPSIGN"),
        "key with id 0x1 not found.",
    );
    assert_fails(
        &format!(
            "'hello' {} PGPSIGN",
            sign_map("").replace("'secret'", "'wrong'")
        ),
        "unable to extract private key.",
    );
    assert_fails(
        &format!("'hello' {} PGPSIGN", sign_map("'digest' 'SHA3-256'")),
        "unsupported digest algorithm: 12",
    );
    assert_fails(
        &format!("'hello' {} PGPSIGN", sign_map("'digest' 5")),
        "unable to map 5 to a hash id",
    );
    assert_fails(
        &format!("12 {} PGPSIGN", sign_map("")),
        "invalid content to sign, expected STRING or BYTES.",
    );
}

#[test]
fn the_failures_of_pgpverify_are_those_of_warp_10() {
    let public = public_ring();
    assert_fails(
        &format!("'hello' 'x' {public} {KEY_ID} true PGPVERIFY"),
        "encountered an error while verifying signature.",
    );
    assert_fails("'hello' 'x' 5 true PGPVERIFY", "missing PGP key ring.");
    assert_fails(
        &format!("{public} 1 PGPVERIFY"),
        "key with id 0x1 not found.",
    );
    assert_fails(
        "1.5 PGPVERIFY",
        "expected a PGP public key or key ring and key id.",
    );
    assert_fails(
        &format!("'hello' 12 {public} {KEY_ID} true PGPVERIFY"),
        "expected signature data (STRING or BYTES).",
    );
    assert_fails(
        &format!("12 '{HELLO_DETACHED}' HEX-> {public} {KEY_ID} true PGPVERIFY"),
        "expected content data (STRING or BYTES).",
    );
    assert_fails(
        &format!("12 {public} {KEY_ID} PGPVERIFY"),
        "expected signed message (STRING or BYTES).",
    );
    assert_fails(
        &format!("'{HELLO_DETACHED}' HEX-> {public} {KEY_ID} PGPVERIFY"),
        "unexpected packet: 2",
    );
}

#[test]
fn a_detached_string_that_is_not_armor_is_an_error() {
    let mut stack = test_stack();
    stack.push(Value::string("hello"));
    stack.push(Value::string("not armor"));
    if stack
        .exec_checked(&format!("{} {KEY_ID} true PGPVERIFY", public_ring()))
        .is_ok()
    {
        panic!("the armor must not decode");
    }
}

#[test]
fn exec_runs_the_signing_scripts() {
    let stack = exec(&format!("'hello' {} PGPSIGN", sign_map("'armor' false")));
    assert!(matches!(stack.elements().last(), Some(Value::Bytes(_))));
}

#[test]
fn each_missing_operand_of_pgpsign_and_pgpverify_is_an_error() {
    let public = public_ring();
    let scripts = [
        "PGPVERIFY".to_string(),
        "true PGPVERIFY".to_string(),
        "1 PGPVERIFY".to_string(),
        format!("{public} {KEY_ID} true PGPVERIFY"),
        format!("'{HELLO_DETACHED}' HEX-> {public} {KEY_ID} true PGPVERIFY"),
        format!("{public} {KEY_ID} PGPVERIFY"),
        format!("{} PGPSIGN", sign_map("'armor' false")),
    ];
    for script in scripts {
        assert_ne!(fail_msg(&script), String::new(), "{script}");
    }
}

#[test]
fn an_armored_string_that_is_not_armor_is_an_error_for_both_verify_forms() {
    let public = public_ring();
    let detached = format!("'hello' 'not armor' {public} {KEY_ID} true PGPVERIFY");
    let inline = format!("'not armor' {public} {KEY_ID} PGPVERIFY");
    assert_ne!(fail_msg(&detached), String::new());
    assert_ne!(fail_msg(&inline), String::new());
}

/// The body of a version 4 secret RSA key whose modulus has one bit: its private exponent reads,
/// but the modulus cannot hold the block of a signature.
const TINY_SECRET: &[u8] = &[4, 0, 0, 0, 0, 1, 0, 1, 1, 0, 8, 3, 0, 0, 8, 3];

/// The body of a version 3 RSA public key.
const V3_PUBLIC: &[u8] = &[3, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 0, 8, 3];

/// Armor that does not decode: a line of the body that starts with a dash is not armor.
const BAD_ARMOR: &str = "-----BEGIN PGP SIGNATURE-----\n-\n";

/// A packet with a one-octet length, in the new format, for the tests that craft keys.
fn packet(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![0xC0 | tag, body.len() as u8];
    out.extend_from_slice(body);
    out
}

/// The hexadecimal key ID of a key body.
fn key_id_of(body: &[u8]) -> String {
    match PublicKey::parse(body) {
        Ok((key, _)) => key_id_hex(key.key_id),
        Err(error) => panic!("the key parses: {error}"),
    }
}

/// The ring of some packets, on the stack.
fn ring_of(packets: &[u8]) -> String {
    format!("'{}' HEX-> PGPRING 0 GET", hex_encode(packets))
}

/// A `PGPSIGN` parameter map of the tiny secret key, with `extra` parameters after its key.
fn tiny_map(extra: &str) -> String {
    format!(
        "{{ 'passphrase' 'secret' 'keyring' {} 'keyid' '{}' {extra} }}",
        ring_of(&packet(5, TINY_SECRET)),
        key_id_of(TINY_SECRET)
    )
}

/// The bytes of a hexadecimal fixture, or a panic.
fn fixture_bytes(hex: &str) -> Vec<u8> {
    match hex_decode(hex) {
        Ok(bytes) => bytes,
        Err(error) => panic!("the fixture is hexadecimal: {error}"),
    }
}

/// The hexadecimal of a fixture whose signature packet has the version 3.
fn with_version_3_signature(hex: &str) -> String {
    let header = [0x89, 0x01, 0x1c, 0x04];
    let mut bytes = fixture_bytes(hex);
    let Some(at) = bytes
        .windows(header.len())
        .position(|window| window == header)
    else {
        panic!("the fixture has a signature packet of version 4");
    };
    bytes[at + 3] = 3;
    hex_encode(&bytes)
}

/// The message of the failure of a script that starts with `values` on its stack.
fn fail_with_values(values: Vec<Value>, script: &str) -> String {
    let mut stack = test_stack();
    for value in values {
        stack.push(value);
    }
    match stack.exec_checked(script) {
        Ok(()) => panic!("script did not fail:\n{script}"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn a_key_too_small_to_sign_is_a_failed_signature() {
    assert_fails(
        &format!("'hello' {} PGPSIGN", tiny_map("'armor' false")),
        "exception creating signature.",
    );
    assert_fails(
        &format!(
            "'hello' {} PGPSIGN",
            tiny_map("'detached' false 'armor' false")
        ),
        "exception creating signature.",
    );
}

#[test]
fn a_version_3_key_cannot_verify() {
    assert_fails(
        &format!(
            "'hello' 'x' {} '{}' true PGPVERIFY",
            ring_of(&packet(6, V3_PUBLIC)),
            key_id_of(V3_PUBLIC)
        ),
        "encountered an error while verifying signature.",
    );
}

#[test]
fn armor_that_does_not_decode_fails_the_verification() {
    let public = public_ring();
    let detached = fail_with_values(
        vec![Value::string("hello"), Value::string(BAD_ARMOR)],
        &format!("{public} {KEY_ID} true PGPVERIFY"),
    );
    assert!(detached.contains("encountered an error while verifying signature."));
    let inline = fail_with_values(
        vec![Value::string(BAD_ARMOR)],
        &format!("{public} {KEY_ID} PGPVERIFY"),
    );
    assert!(inline.contains("encountered an error while verifying signature."));
}

#[test]
fn a_signature_of_version_3_fails_the_verification() {
    let public = public_ring();
    assert_fails(
        &format!(
            "'hello' '{}' HEX-> {public} {KEY_ID} true PGPVERIFY",
            with_version_3_signature(HELLO_DETACHED)
        ),
        "encountered an error while verifying signature.",
    );
    assert_fails(
        &format!(
            "'{}' HEX-> {public} {KEY_ID} PGPVERIFY",
            with_version_3_signature(HELLO_INLINE)
        ),
        "encountered an error while verifying signature.",
    );
}
