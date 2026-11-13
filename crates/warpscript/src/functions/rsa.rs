// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The RSA functions: `RSAGEN`, `RSAPRIVATE`, `RSAPUBLIC`, `RSAENCRYPT`, `RSADECRYPT`,
//! `RSASIGN` and `RSAVERIFY`. The keys are those of [`crate::rsa`].

use std::rc::Rc;

use indexmap::IndexMap;
use num_bigint::BigInt;

use super::Entry;
use super::args::fail;
use crate::bigdecimal::parse_bigint;
use crate::ec::{Key, Random, system_random};
use crate::error::crypto::{CryptoError, RsaError};
use crate::error::fault::NumberFormatError;
use crate::error::{Error, Result};
use crate::rsa::{Exponentiation, NONCE, RsaKey, Scheme, generate};
use crate::stack::Stack;
use crate::value::{Value, ValueKind};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("RSAGEN", Entry::Stack(rsagen)),
    (
        "RSAPRIVATE",
        Entry::Stack(|s| rsakey(s, "RSAPRIVATE", Key::RsaPrivate)),
    ),
    (
        "RSAPUBLIC",
        Entry::Stack(|s| rsakey(s, "RSAPUBLIC", Key::RsaPublic)),
    ),
    ("RSAENCRYPT", Entry::Stack(rsaencrypt)),
    ("RSADECRYPT", Entry::Stack(rsadecrypt)),
    ("RSASIGN", Entry::Stack(rsasign)),
    ("RSAVERIFY", Entry::Stack(rsaverify)),
];

/// An RSA failure as the error of a function.
fn crypto(error: RsaError) -> Error {
    Error::from(CryptoError::from(error))
}

/// `new BigInteger(text)`: the number of a text in decimal.
fn big_integer(text: &str) -> core::result::Result<BigInt, NumberFormatError> {
    parse_bigint(text, 10)
}

/// The map of a key, as `RSAPRIVATE` and `RSAPUBLIC` read it.
fn key_map(key: &RsaKey, exponent: &BigInt) -> Value {
    Value::map(IndexMap::from([
        (
            Value::string("modulus"),
            Value::string(key.modulus.to_string()),
        ),
        (Value::string("algorithm"), Value::string("RSA")),
        (
            Value::string("exponent"),
            Value::string(exponent.to_string()),
        ),
    ]))
}

/// `RSAGEN`: `exponent strength RSAGEN` to the map of a private key and, on top of it, the map
/// of its public key.
fn rsagen(stack: &mut Stack) -> Result<()> {
    rsagen_with(stack, &mut system_random)
}

/// `RSAGEN` with a source of random bytes.
pub(crate) fn rsagen_with(stack: &mut Stack, random: Random<'_>) -> Result<()> {
    const NAME: &str = "RSAGEN";
    let Value::Long(strength) = stack.pop()? else {
        return Err(fail(NAME, "expects a key length."));
    };
    // `top.toString()` of the exponent.
    let exponent = match stack.pop()? {
        Value::Null => return Err(Error::null_pointer()),
        Value::String(text) => text.to_string(),
        other => other.to_string(),
    };
    let exponent = big_integer(&exponent)?;
    // `((Number) top).intValue()`.
    let pair = generate(&exponent, strength as i32, random).map_err(crypto)?;
    stack.push(key_map(&pair.private, &pair.private.exponent));
    stack.push(key_map(&pair.public, &pair.public.exponent));
    Ok(())
}

/// A number the map of a key holds, as `new BigInteger(params.get(name))` reads it.
fn parameter(params: &IndexMap<Value, Value>, name: &str) -> Result<BigInt> {
    match params.get(&Value::string(name)) {
        None | Some(Value::Null) => Err(Error::null_pointer()),
        Some(Value::String(text)) => Ok(big_integer(text)?),
        Some(other) => Err(Error::class_cast(other.kind(), ValueKind::String)),
    }
}

/// `RSAPRIVATE` and `RSAPUBLIC`: the map of a key to the key.
fn rsakey(stack: &mut Stack, name: &str, make: fn(RsaKey) -> Key) -> Result<()> {
    let Value::Map(params) = stack.pop()? else {
        return Err(fail(name, "expects a map on top of the stack."));
    };
    let params = params.borrow();
    let is_rsa = matches!(
        params.get(&Value::string("algorithm")),
        Some(Value::String(algorithm)) if &**algorithm == "RSA"
    );
    if !is_rsa {
        return Err(fail(
            name,
            "invalid value for key 'algorithm', expected value 'RSA'.",
        ));
    }
    let modulus = parameter(&params, "modulus")?;
    let exponent = parameter(&params, "exponent")?;
    stack.push(Value::Key(Rc::new(make(RsaKey { modulus, exponent }))));
    Ok(())
}

/// `RSAENCRYPT`: `data key RSAENCRYPT` to the ciphertext of the data preceded by a random nonce
/// (`null` for a key that is not an RSA key).
fn rsaencrypt(stack: &mut Stack) -> Result<()> {
    rsaencrypt_with(stack, &mut system_random)
}

/// `RSAENCRYPT` with a source of random bytes.
pub(crate) fn rsaencrypt_with(stack: &mut Stack, random: Random<'_>) -> Result<()> {
    const NAME: &str = "RSAENCRYPT";
    let Value::Key(key) = stack.pop()? else {
        return Err(fail(NAME, "expects a key on top of the stack."));
    };
    let Value::Bytes(data) = stack.pop()? else {
        return Err(fail(NAME, "operates on a byte array."));
    };
    let mut nonced = vec![0; NONCE];
    random(&mut nonced).map_err(|error| crypto(error.into()))?;
    nonced.extend_from_slice(&data);
    let encrypted = match key.rsa() {
        Some(rsa) => Value::bytes(Exponentiation::new(rsa).map_err(crypto)?.encrypt(&nonced)),
        None => Value::Null,
    };
    stack.push(encrypted);
    Ok(())
}

/// `RSADECRYPT`: `ciphertext key RSADECRYPT` to the data, without the nonce.
fn rsadecrypt(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "RSADECRYPT";
    let Value::Key(key) = stack.pop()? else {
        return Err(fail(NAME, "expects a key on top of the stack."));
    };
    let Value::Bytes(data) = stack.pop()? else {
        return Err(fail(NAME, "operates on a byte array."));
    };
    // `decryptRSA` is `null` for a key that is not an RSA key.
    let Some(rsa) = key.rsa() else {
        return Err(Error::null_pointer());
    };
    let nonced = match Exponentiation::new(rsa).map_err(crypto)?.decrypt(&data) {
        Ok(nonced) => nonced,
        // `decryptRSA` is `null` for what is not a block of data: `Arrays.copyOfRange(null…)`.
        Err(RsaError::Block(_not_a_block)) => return Err(Error::null_pointer()),
        Err(other) => return Err(crypto(other)),
    };
    // `Arrays.copyOfRange(nonced, 8, nonced.length)`.
    let Some(clear) = nonced.get(NONCE..) else {
        return Err(CryptoError::RangeInverted {
            from: NONCE as i64,
            to: nonced.len() as i64,
        }
        .into());
    };
    stack.push(Value::bytes(clear));
    Ok(())
}

/// `RSASIGN`: `data algorithm key RSASIGN` to the signature.
fn rsasign(stack: &mut Stack) -> Result<()> {
    rsasign_with(stack, &mut system_random)
}

/// `RSASIGN` with a source of random bytes.
pub(crate) fn rsasign_with(stack: &mut Stack, random: Random<'_>) -> Result<()> {
    const NAME: &str = "RSASIGN";
    let key = match stack.pop()? {
        Value::Key(key) if key.is_private() => key,
        _ => return Err(fail(NAME, "expects a private key on top of the stack.")),
    };
    let Value::String(algorithm) = stack.pop()? else {
        return Err(fail(NAME, "expects an algorithm name below the key."));
    };
    let Value::Bytes(data) = stack.pop()? else {
        return Err(fail(NAME, "operates on a byte array."));
    };
    // `CryptoHelper.sign` is `null` for an algorithm BouncyCastle does not have, and for a key
    // that is not an RSA key (whatever the algorithm).
    let (Some(rsa), Some(scheme)) = (key.rsa(), Scheme::lookup(&algorithm)) else {
        return Err(fail(NAME, "invalid algorithm."));
    };
    let exponentiation = Exponentiation::new(rsa).map_err(crypto)?;
    let signer = exponentiation.signer(scheme).map_err(crypto)?;
    match signer.sign(&data, random) {
        Ok(signature) => {
            stack.push(Value::bytes(signature));
            Ok(())
        }
        // Data that `signature.sign()` refuses: `null`.
        Err(RsaError::DataTooLarge) => Err(fail(NAME, "invalid algorithm.")),
        Err(other) => Err(crypto(other)),
    }
}

/// `RSAVERIFY`: `data signature algorithm key RSAVERIFY` to a boolean.
fn rsaverify(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "RSAVERIFY";
    let key = match stack.pop()? {
        Value::Key(key) if key.is_public() => key,
        _ => return Err(fail(NAME, "expects a public key on top of the stack.")),
    };
    let Value::String(algorithm) = stack.pop()? else {
        return Err(fail(NAME, "expects an algorithm name below the key."));
    };
    let Value::Bytes(signature) = stack.pop()? else {
        return Err(fail(NAME, "expects a signature below the algorithm."));
    };
    let Value::Bytes(data) = stack.pop()? else {
        return Err(fail(NAME, "operates on a byte array."));
    };
    // `CryptoHelper.verify` is `false` for an algorithm BouncyCastle does not have, and for a
    // key that is not an RSA key.
    let verified = match (key.rsa(), Scheme::lookup(&algorithm)) {
        (Some(rsa), Some(scheme)) => {
            let exponentiation = Exponentiation::new(rsa).map_err(crypto)?;
            let signer = exponentiation.signer(scheme).map_err(crypto)?;
            match signer.verify(&data, &signature) {
                Ok(()) => true,
                // `DigestSignatureSpi.engineVerify` answers `false` to a signature it cannot
                // read, and to one that is not the one of the data: each `Rejection` is one of
                // these, and this library has no log to tell which.
                Err(_rejection) => false,
            }
        }
        _ => false,
    };
    stack.push(verified);
    Ok(())
}
