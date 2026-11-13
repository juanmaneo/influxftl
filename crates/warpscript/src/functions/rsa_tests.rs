// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the RSA functions. The tables (`rsa_tables_tests.rs`) are what Warp 10 3.5.0
//! (BouncyCastle) answers to the same scripts, on keys and blocks that a script of this repository
//! builds apart from both implementations. What is random in Warp 10 (the keys, the nonce of an
//! encryption, the salt of a PSS signature) is not compared: what it made is read here, and what
//! is made here is read here, with the random bytes of the tests.

use super::rsa::{rsaencrypt_with, rsagen_with, rsasign_with};
use super::rsa_tables_tests::{
    ARGS_ERRORS, CIPHERTEXTS, CRAFTED_VALUES, CROSS_ERRORS, CROSS_VALUES, DECRYPT_ERRORS,
    DECRYPT_VALUES, DIVERGENT, GEN_ERRORS, GEN_SHAPES, KEYS, KEYS_ERRORS, KEYS_VALUES, PSS_ERRORS,
    PSS_SIGNATURES, PSS_VALUES, SIGN_ERRORS, SIGN_VALUES, SIGNATURES,
};
use crate::encoding::hex_encode;
use crate::rsa::NONCE;
use crate::tests::{assert_fails, assert_top, exec, scripted, test_stack};
use crate::value::Value;
use indexmap::IndexMap;
use num_bigint::BigUint;

/// The script of a template, its `@PUBn` and `@PRIVn` replaced by the keys of `n` bits.
fn expand(template: &str) -> String {
    let mut script = template.to_owned();
    for (bits, modulus, private) in KEYS {
        for (what, exponent, make) in [
            ("PUB", "65537", "RSAPUBLIC"),
            ("PRIV", private, "RSAPRIVATE"),
        ] {
            script = script.replace(
                &format!("@{what}{bits}"),
                &format!(
                    "{{ 'modulus' '{modulus}' 'algorithm' 'RSA' 'exponent' '{exponent}' }} {make}"
                ),
            );
        }
    }
    script
}

/// The scripts of a table give what Warp 10 gave.
fn assert_values(table: &[(&str, &str)]) {
    for (script, expected) in table {
        assert_top(&expand(script), *expected);
    }
}

/// The scripts of a table fail with what Warp 10 said.
fn assert_all_fail(table: &[(&str, &str)]) {
    for (script, message) in table {
        assert_fails(&expand(script), message);
    }
}

#[test]
fn keys_are_those_of_warp_10() {
    assert_values(KEYS_VALUES);
    assert_all_fail(KEYS_ERRORS);
}

#[test]
fn arguments_are_those_of_warp_10() {
    assert_all_fail(GEN_ERRORS);
    assert_all_fail(ARGS_ERRORS);
}

#[test]
fn keys_of_curves_and_keys_of_rsa_are_not_exchanged() {
    assert_values(CROSS_VALUES);
    assert_all_fail(CROSS_ERRORS);
}

#[test]
fn what_warp_10_cannot_sign_is_refused() {
    assert_values(SIGN_VALUES);
    assert_all_fail(SIGN_ERRORS);
    assert_values(PSS_VALUES);
    assert_all_fail(PSS_ERRORS);
}

#[test]
fn signatures_are_verified_like_warp_10() {
    assert_values(CRAFTED_VALUES);
}

#[test]
fn ciphertexts_are_read_like_warp_10() {
    assert_values(DECRYPT_VALUES);
    assert_all_fail(DECRYPT_ERRORS);
}

/// The bytes of a text in hexadecimal.
fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|index| u8::from_str_radix(&text[2 * index..2 * index + 2], 16).expect("a byte"))
        .collect()
}

/// The data of the tables of ciphertexts: the `size` bytes that Warp 10 encrypted.
fn data(size: usize) -> Vec<u8> {
    (0..size)
        .map(|index| ((index * 31 + 7) % 251) as u8)
        .collect()
}

/// The text of some bytes in hexadecimal.
fn hex(bytes: &[u8]) -> String {
    hex_encode(bytes)
}

/// The size of the blocks of a key, in bytes.
fn block_size(bits: &str) -> usize {
    bits.parse::<usize>().expect("a size").div_ceil(8)
}

#[test]
fn pkcs1_signatures_are_those_of_warp_10() {
    for (algorithm, signature) in SIGNATURES {
        assert_top(
            &expand(&format!(
                "'hello' 'UTF-8' ->BYTES '{algorithm}' @PRIV1024 RSASIGN ->HEX"
            )),
            *signature,
        );
        let verify = |text: &str| {
            expand(&format!(
                "'{text}' 'UTF-8' ->BYTES '{signature}' HEX-> '{algorithm}' @PUB1024 RSAVERIFY"
            ))
        };
        assert_top(&verify("hello"), true);
        assert_top(&verify("hellp"), false);
    }
}

#[test]
fn pss_signatures_are_made_of_the_salt_they_are_given() {
    for (key, name, salt, signature) in PSS_SIGNATURES {
        let algorithm = if *name == "RSASSA-PSS" {
            (*name).to_owned()
        } else {
            format!("{name}WITHRSAANDMGF1")
        };
        let mut stack = test_stack();
        stack
            .exec(&expand(&format!(
                "'hello' 'UTF-8' ->BYTES '{algorithm}' @PRIV{key}"
            )))
            .expect("the arguments");
        rsasign_with(&mut stack, &mut scripted(unhex(salt))).expect("a signature");
        stack.exec("->HEX").expect("a text");
        assert_eq!(
            stack.pop().expect("a signature"),
            Value::string(signature),
            "{key} {algorithm}"
        );
        // And it is verified, by the name of the algorithm, and not for other data.
        let verify = |text: &str| {
            expand(&format!(
                "'{text}' 'UTF-8' ->BYTES '{signature}' HEX-> '{algorithm}' @PUB{key} RSAVERIFY"
            ))
        };
        assert_top(&verify("hello"), true);
        assert_top(&verify("hellp"), false);
    }
}

#[test]
fn ciphertexts_of_warp_10_are_decrypted() {
    for (key, other, size, ciphertext) in CIPHERTEXTS {
        assert_top(
            &expand(&format!(
                "'{ciphertext}' HEX-> @{other}{key} RSADECRYPT ->HEX"
            )),
            hex(&data(*size)),
        );
    }
}

#[test]
fn what_is_encrypted_by_a_key_is_decrypted_by_the_other() {
    for (bits, _, _) in KEYS {
        let capacity = block_size(bits) - 11 - NONCE;
        let sizes = [
            0,
            1,
            capacity - 1,
            capacity,
            capacity + 1,
            2 * capacity,
            2 * capacity + 1,
        ];
        for size in sizes {
            let clear = hex(&data(size));
            for (from, to) in [("PRIV", "PUB"), ("PUB", "PRIV")] {
                assert_top(
                    &expand(&format!(
                        "'{clear}' HEX-> @{from}{bits} RSAENCRYPT @{to}{bits} RSADECRYPT ->HEX"
                    )),
                    clear.as_str(),
                );
            }
            // The nonce makes a block of what is nothing.
            let blocks = (size + NONCE).div_ceil(block_size(bits) - 11);
            assert_top(
                &expand(&format!("'{clear}' HEX-> @PUB{bits} RSAENCRYPT SIZE")),
                i64::try_from(blocks * block_size(bits)).expect("a size"),
            );
        }
    }
}

/// The blocks of the PKCS#1 of `size` bytes, of type 1, for some data.
fn block_of_type_1(size: usize, data: &[u8]) -> Vec<u8> {
    let mut block = vec![0, 1];
    block.resize(size - data.len() - 1, 0xff);
    block.push(0);
    block.extend_from_slice(data);
    block
}

#[test]
fn a_message_is_encrypted_with_the_nonce_it_is_given() {
    let nonce: Vec<u8> = (1..=8).collect();
    for (bits, modulus, private) in KEYS {
        let modulus: BigUint = modulus.parse().expect("a modulus");
        let size = block_size(bits);
        for (what, exponent) in [("PUB", "65537"), ("PRIV", private)] {
            let exponent: BigUint = exponent.parse().expect("an exponent");
            let message = [nonce.clone(), b"hello".to_vec()].concat();
            let block = BigUint::from_bytes_be(&block_of_type_1(size, &message));
            let power = block.modpow(&exponent, &modulus).to_bytes_be();
            let mut expected = vec![0; size - power.len()];
            expected.extend(power);

            let mut stack = test_stack();
            stack
                .exec(&expand(&format!("'hello' 'UTF-8' ->BYTES @{what}{bits}")))
                .expect("the arguments");
            rsaencrypt_with(&mut stack, &mut scripted(nonce.clone())).expect("a ciphertext");
            assert_eq!(
                stack.pop().expect("a ciphertext"),
                Value::bytes(&expected),
                "{what}{bits}"
            );
        }
    }
}

#[test]
fn a_message_cannot_be_encrypted_without_a_nonce() {
    let mut stack = test_stack();
    stack
        .exec(&expand("'hello' 'UTF-8' ->BYTES @PUB1024"))
        .expect("the arguments");
    let error = rsaencrypt_with(&mut stack, &mut scripted(Vec::new())).expect_err("no entropy");
    assert!(
        error
            .to_string()
            .contains("Unable to obtain random data from the operating system"),
        "{error}"
    );
}

#[test]
fn a_pss_signature_cannot_be_made_without_a_salt() {
    let mut stack = test_stack();
    stack
        .exec(&expand(
            "'hello' 'UTF-8' ->BYTES 'SHA256WITHRSAANDMGF1' @PRIV1024",
        ))
        .expect("the arguments");
    let error = rsasign_with(&mut stack, &mut scripted(Vec::new())).expect_err("no entropy");
    assert!(
        error
            .to_string()
            .contains("Unable to obtain random data from the operating system"),
        "{error}"
    );
}

#[test]
fn what_warp_10_signs_and_this_port_does_not_is_refused() {
    for script in DIVERGENT {
        assert_fails(&expand(script), "RSASIGN invalid algorithm.");
    }
}

/// The map of a key, as `RSAGEN` makes it.
fn key_map(modulus: &str, exponent: &str) -> IndexMap<Value, Value> {
    IndexMap::from([
        (Value::string("modulus"), Value::string(modulus)),
        (Value::string("algorithm"), Value::string("RSA")),
        (Value::string("exponent"), Value::string(exponent)),
    ])
}

/// The entries of a map.
fn entries(value: &Value) -> IndexMap<Value, Value> {
    let Value::Map(map) = value else {
        panic!("a map, not {value:?}");
    };
    map.borrow().clone()
}

/// The 2-byte candidates (big-endian) of a source of random bytes.
fn candidates(numbers: &[u16]) -> Vec<u8> {
    numbers
        .iter()
        .flat_map(|number| number.to_be_bytes())
        .collect()
}

#[test]
fn a_key_is_generated_from_random_bytes() {
    // 2897 and 3167 are primes of 12 bits: 9174799 is their product, 1528123 the inverse of 3.
    for exponent in [Value::Long(3), Value::string("3")] {
        let mut stack = test_stack();
        stack.push(exponent);
        stack.push(Value::Long(24));
        rsagen_with(&mut stack, &mut scripted(candidates(&[2897, 3167]))).expect("a key pair");
        assert_eq!(stack.depth(), 2);
        let public = stack.pop().expect("the public key");
        let private = stack.pop().expect("the private key");
        assert_eq!(entries(&private), key_map("9174799", "1528123"));
        assert_eq!(entries(&public), key_map("9174799", "3"));
    }
}

#[test]
fn a_key_cannot_be_generated_without_random_bytes() {
    let mut stack = test_stack();
    stack.push(Value::Long(3));
    stack.push(Value::Long(24));
    let error = rsagen_with(&mut stack, &mut scripted(Vec::new())).expect_err("no entropy");
    assert!(
        error
            .to_string()
            .contains("Unable to obtain random data from the operating system"),
        "{error}"
    );
}

/// A number of the map of a key.
fn number(map: &Value, name: &str) -> BigUint {
    let entries = entries(map);
    let Some(Value::String(text)) = entries.get(&Value::string(name)) else {
        panic!("a text for {name}");
    };
    text.parse().expect("a number")
}

#[test]
fn the_keys_that_are_generated_are_those_of_rsa() {
    for script in GEN_SHAPES {
        let strength: u64 = script
            .split_whitespace()
            .nth(1)
            .expect("a strength")
            .parse()
            .expect("a number");
        for _ in 0..3 {
            let mut stack = exec(script);
            let public = stack.pop().expect("the public key");
            let private = stack.pop().expect("the private key");
            let modulus = number(&private, "modulus");
            assert_eq!(modulus, number(&public, "modulus"), "{script}");
            assert_eq!(modulus.bits(), strength, "{script}");
            let (d, e) = (number(&private, "exponent"), number(&public, "exponent"));
            assert!(d > BigUint::from(1u8) << (strength / 2), "{script}");
            for message in [2u8, 3, 5, 77] {
                let message = BigUint::from(message);
                assert_eq!(
                    message.modpow(&e, &modulus).modpow(&d, &modulus),
                    message,
                    "{script}"
                );
            }
        }
    }
}

#[test]
fn a_generated_key_signs_and_encrypts() {
    assert_top(
        "65537 512 RSAGEN RSAPUBLIC 'pub' STORE RSAPRIVATE 'priv' STORE
         'hello' 'UTF-8' ->BYTES 'data' STORE
         $data 'SHA256WITHRSA' $priv RSASIGN 'sig' STORE
         [
           $data $sig 'SHA256WITHRSA' $pub RSAVERIFY
           'hellp' 'UTF-8' ->BYTES $sig 'SHA256WITHRSA' $pub RSAVERIFY
           $data $pub RSAENCRYPT $priv RSADECRYPT ->HEX $data ->HEX ==
         ]
         ->JSON",
        "[true,false,true]",
    );
}

#[test]
fn a_key_that_is_too_small_for_blocks_is_refused_by_each_function() {
    for (script, key) in [
        ("'hello' 'UTF-8' ->BYTES", "RSAPUBLIC RSAENCRYPT"),
        ("'hello' 'UTF-8' ->BYTES", "RSAPRIVATE RSAENCRYPT"),
        ("'hello' 'UTF-8' ->BYTES", "RSAPRIVATE RSADECRYPT"),
        (
            "'hello' 'UTF-8' ->BYTES 'SHA1WITHRSA'",
            "RSAPRIVATE RSASIGN",
        ),
        (
            "'hello' 'UTF-8' ->BYTES 'hello' 'UTF-8' ->BYTES 'SHA1WITHRSA'",
            "RSAPUBLIC RSAVERIFY",
        ),
    ] {
        assert_fails(
            &format!("{script} {{ 'modulus' '5' 'algorithm' 'RSA' 'exponent' '3' }} {key}"),
            "RSA modulus of 3 bits is too small for PKCS#1 blocks",
        );
    }
}

#[test]
fn a_small_key_signs_with_the_digests_that_fit_it() {
    // PSS needs 16 times the size of the digest and 9 bits less than the modulus: 457 bits are
    // enough for SHA-224 in 511 bits, and the salt is drawn from the system at each signature.
    for algorithm in ["SHA1WITHRSAANDMGF1", "SHA224WITHRSA/PSS"] {
        let script = format!(
            "'hello' 'UTF-8' ->BYTES 'data' STORE
             $data '{algorithm}' @PRIV512 RSASIGN 'one' STORE
             $data '{algorithm}' @PRIV512 RSASIGN 'two' STORE
             [
               $data $one '{algorithm}' @PUB512 RSAVERIFY
               $data $two '{algorithm}' @PUB512 RSAVERIFY
               $one $two !=
             ]
             ->JSON"
        );
        assert_top(&expand(&script), "[true,true,true]");
    }
}
