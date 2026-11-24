// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The OpenPGP encryption functions: `PGPENCRYPT` and `PGPDECRYPT`, for RSA keys. The formats are
//! those of [`crate::openpgp`], written and read the way BouncyCastle does.

use std::rc::Rc;

use indexmap::IndexMap;

use super::Entry;
use super::args::{fail, fail_with_cause, pop_map};
use super::ec::{bc_hex_decode, string_of};
use super::pgp_sign::{flag, input_bytes, key_body, literal_date, parameter, raw_bytes};
use crate::ec::system_random;
use crate::error::pgp::PgpError;
use crate::error::{Error, Result};
use crate::openpgp::armor;
use crate::openpgp::decrypt::decrypt;
use crate::openpgp::encrypt::{EncryptParams, encrypt};
use crate::openpgp::key::low_u64;
use crate::openpgp::ring::Key;
use crate::openpgp::secret::rsa_private_key;
use crate::openpgp::sign::public_rsa;
use crate::openpgp::symmetric::{Cipher, algorithm_id};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PGPENCRYPT", Entry::Stack(pgpencrypt)),
    ("PGPDECRYPT", Entry::Stack(pgpdecrypt)),
];

/// The cipher of a message when its map does not name one.
const DEFAULT_ALGORITHM: &str = "AES_256";

/// The failure of an encryption, whatever its cause.
const ENCRYPT_FAILED: &str = "error while encrypting data.";

/// The failure of a decryption, whatever its cause.
const DECRYPT_FAILED: &str = "error during decryption.";

/// The public key that a message is encrypted to: a `PGPPublicKey` as it is, or the key of the
/// keyring whose ID is given (a `LONG` or a hexadecimal `STRING`).
fn recipient(params: &IndexMap<Value, Value>) -> Result<Rc<Key>> {
    const NAME: &str = "PGPENCRYPT";
    let key_id = match parameter(params, "recipient") {
        Some(Value::PgpKey(key)) => return Ok(Rc::clone(key)),
        Some(Value::Long(id)) => *id as u64,
        Some(Value::String(text)) => low_u64(&bc_hex_decode(text)?),
        _ => {
            return Err(fail(
                NAME,
                "missing recipient PGP public key or key ring and key id.",
            ));
        }
    };
    let Some(Value::PgpRing(ring)) = parameter(params, "keyring") else {
        return Err(fail(NAME, "missing PGP secret key ring."));
    };
    ring.public_keys()
        .find(|key| key.public.key_id == key_id)
        .map(|key| Rc::new(key.clone()))
        .ok_or_else(|| fail(NAME, &format!("key with id 0x{key_id:x} not found.")))
}

/// The encrypted message of `data` to `key`, under the cipher that `algorithm` names. The key ID
/// is the one the session key packet gives.
fn seal(
    key: &Key,
    key_id: u64,
    algorithm: &str,
    date: u32,
    data: &[u8],
) -> std::result::Result<Vec<u8>, PgpError> {
    let cipher = algorithm_id(algorithm).and_then(Cipher::from_id)?;
    let public = public_rsa(key_body(key))?;
    let params = EncryptParams {
        key: &public,
        key_id,
        cipher,
        date,
    };
    encrypt(&params, data, &mut system_random)
}

/// `content {map} PGPENCRYPT` to the message of the content, in `BYTES`, or in an armored
/// `STRING` unless `armor` is false. The map has the `recipient` (a `PGPPublicKey`, or a key ID
/// of the `keyring`), and optionally `algorithm`, `throw_keyid`, `armor` and `date`.
fn pgpencrypt(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PGPENCRYPT";
    let params = pop_map(stack, NAME, "expected a parameter MAP.")?;
    let params = params.borrow();
    let key = recipient(&params)?;
    let data = stack.pop()?;
    let data = raw_bytes(&data).ok_or_else(|| {
        fail(
            NAME,
            "expects data to encrypt to be either STRING or BYTES.",
        )
    })?;
    let algorithm = params.get(&Value::string("algorithm")).map_or_else(
        || DEFAULT_ALGORITHM.to_owned(),
        |value| string_of(Some(value)),
    );
    let key_id = if flag(&params, "throw_keyid") {
        0
    } else {
        key.public.key_id
    };
    let message = seal(&key, key_id, &algorithm, literal_date(&params), &data)
        .map_err(|cause| fail_with_cause(NAME, ENCRYPT_FAILED, Error::from(cause)))?;
    if flag(&params, "armor") {
        stack.push(Value::string(armor::encode(&message)));
    } else {
        stack.push(Value::bytes(message));
    }
    Ok(())
}

/// `content keyring keyid passphrase PGPDECRYPT` to the data of the literal packet of the message,
/// in `BYTES`. The content is a `STRING` of armor or `BYTES` of the message, and the keyring is a
/// secret key ring, whose key with the key ID is unlocked with the passphrase.
fn pgpdecrypt(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PGPDECRYPT";
    let passphrase = match stack.pop()? {
        Value::String(text) => text.to_string(),
        _ => return Err(fail(NAME, "missing passphrase.")),
    };
    let key_id = match stack.pop()? {
        Value::Long(id) => id as u64,
        Value::String(text) => low_u64(&bc_hex_decode(&text)?),
        _ => return Err(fail(NAME, "missing PGP secret key id.")),
    };
    let ring = match stack.pop()? {
        Value::PgpRing(ring) if ring.secret => ring,
        _ => return Err(fail(NAME, "missing PGP secret key ring.")),
    };
    let key = ring
        .keys
        .iter()
        .find(|key| key.public.key_id == key_id)
        .ok_or_else(|| fail(NAME, &format!("key with id 0x{key_id:x} not found.")))?;
    let secret = rsa_private_key(key_body(key), &passphrase).map_err(|cause| {
        fail_with_cause(NAME, "unable to extract private key.", Error::from(cause))
    })?;
    let content = stack.pop()?;
    let message = input_bytes(&content)
        .ok_or_else(|| fail(NAME, "expected encrypted content as STRING or BYTES."))?
        .map_err(Error::from)?;
    let data = decrypt(&message, &secret)
        .map_err(|cause| fail_with_cause(NAME, DECRYPT_FAILED, Error::from(cause)))?;
    stack.push(Value::bytes(data));
    Ok(())
}
