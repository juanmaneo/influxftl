// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The OpenPGP signature functions: `PGPSIGN` and `PGPVERIFY`, for RSA keys. The formats are
//! those of [`crate::openpgp`], written and read the way BouncyCastle does.

use std::rc::Rc;

use indexmap::IndexMap;

use super::Entry;
use super::args::{fail, fail_with_cause, pop_map};
use super::ec::bc_hex_decode;
use super::runtime::current_time;
use crate::error::pgp::PgpError;
use crate::error::{Error, Result};
use crate::openpgp::armor;
use crate::openpgp::key::low_u64;
use crate::openpgp::ring::Key;
use crate::openpgp::secret::rsa_private_key;
use crate::openpgp::sign::{
    SigningParams, detached_body, detached_packet, digest_by_name, inline_message, public_rsa,
    read_inline, verify,
};
use crate::rsa::RsaKey;
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PGPSIGN", Entry::Stack(pgpsign)),
    ("PGPVERIFY", Entry::Stack(pgpverify)),
];

/// The digest of a signature when the parameters do not name one.
const DEFAULT_DIGEST: &str = "SHA512";

/// The failure of a verification whose input or key cannot be read.
const VERIFY_FAILED: &str = "encountered an error while verifying signature.";

/// The failure of a signature that cannot be made.
const SIGN_FAILED: &str = "exception creating signature.";

/// A parameter of the map of `PGPSIGN`, by its name.
pub(super) fn parameter<'a>(params: &'a IndexMap<Value, Value>, name: &str) -> Option<&'a Value> {
    params.get(&Value::string(name))
}

/// The date of a literal packet, in seconds, from the `date` of a map in micros: 0 when the date
/// is absent or is not a LONG.
pub(super) fn literal_date(params: &IndexMap<Value, Value>) -> u32 {
    match parameter(params, "date") {
        Some(Value::Long(micros)) => (micros / 1_000_000) as u32,
        _ => 0,
    }
}

/// A boolean parameter: true when it is absent, and false when it is not a boolean.
pub(super) fn flag(params: &IndexMap<Value, Value>, name: &str) -> bool {
    match parameter(params, name) {
        None => true,
        Some(Value::Boolean(flag)) => *flag,
        Some(_) => false,
    }
}

/// The body of the packet of a key that holds its public or secret part.
pub(super) fn key_body(key: &Key) -> &[u8] {
    key.packets.first().map_or(&[][..], |packet| &packet.body)
}

/// The bytes of a `STRING` (read as armor) or of `BYTES`, or `None` for another value.
pub(super) fn input_bytes(value: &Value) -> Option<std::result::Result<Vec<u8>, PgpError>> {
    match value {
        Value::String(text) => Some(armor::decode(text.as_bytes()).map_err(PgpError::from)),
        Value::Bytes(bytes) => Some(Ok(bytes.to_vec())),
        _ => None,
    }
}

/// The bytes of a `STRING` or of `BYTES`, as they are, or `None` for another value.
pub(super) fn raw_bytes(value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::String(text) => Some(text.as_bytes().to_vec()),
        Value::Bytes(bytes) => Some(bytes.to_vec()),
        _ => None,
    }
}

/// `content {map} PGPSIGN` to the signature of the content, in `BYTES`, or to the signed message
/// when the map has `detached` set to false. The map has the `passphrase`, the `keyring` (a
/// secret key ring) and the `keyid` of the key, and optionally `detached`, `armor`, `digest` and
/// `date`.
fn pgpsign(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PGPSIGN";
    let params = pop_map(stack, NAME, "expected a parameter MAP.")?;
    let params = params.borrow();
    let passphrase = match parameter(&params, "passphrase") {
        Some(Value::String(text)) => text.to_string(),
        _ => return Err(fail(NAME, "missing PGP secret key passphrase.")),
    };
    let ring = match parameter(&params, "keyring") {
        Some(Value::PgpRing(ring)) if ring.secret => Rc::clone(ring),
        _ => return Err(fail(NAME, "expected a PGP secret key ring.")),
    };
    let key_id = match parameter(&params, "keyid") {
        Some(Value::Long(id)) => *id as u64,
        Some(Value::String(text)) => low_u64(&bc_hex_decode(text)?),
        _ => return Err(fail(NAME, "missing PGP secret key id.")),
    };
    let detached = flag(&params, "detached");
    let armored = flag(&params, "armor");
    let digest = match parameter(&params, "digest") {
        None => DEFAULT_DIGEST.to_string(),
        Some(Value::String(name)) => name.to_string(),
        Some(Value::Long(id)) => id.to_string(),
        Some(_) => String::new(),
    };
    let date = literal_date(&params);
    let key = ring
        .keys
        .iter()
        .find(|key| key.public.key_id == key_id)
        .ok_or_else(|| fail(NAME, &format!("key with id 0x{key_id:x} not found.")))?;
    let secret = rsa_private_key(key_body(key), &passphrase).map_err(|cause| {
        fail_with_cause(NAME, "unable to extract private key.", Error::from(cause))
    })?;
    let (hash_id, hash) = digest_by_name(&digest)?;
    let content = match stack.pop()? {
        Value::String(text) => text.as_bytes().to_vec(),
        Value::Bytes(bytes) => bytes.to_vec(),
        _ => {
            return Err(fail(
                NAME,
                "invalid content to sign, expected STRING or BYTES.",
            ));
        }
    };
    let signing = SigningParams {
        key: &secret,
        algorithm: key.public.algorithm,
        key_id: key.public.key_id,
        hash_id,
        hash,
        created: (current_time() / 1_000_000) as u32,
    };
    let packet = if detached {
        detached_packet(&signing, &content)
    } else {
        inline_message(&signing, date, &content)
    }
    .map_err(|cause| fail_with_cause(NAME, SIGN_FAILED, Error::from(cause)))?;
    let output = if armored {
        armor::encode(&packet).into_bytes()
    } else {
        packet
    };
    stack.push(Value::bytes(output));
    Ok(())
}

/// The public key of a key-thing of `PGPVERIFY`: a `PGPPublicKey` as it is, or the key of a key
/// ring whose ID is given (a `LONG` or a hexadecimal `STRING`, with the ring popped under it).
fn verifying_key(stack: &mut Stack, key_thing: Value) -> Result<Rc<Key>> {
    const NAME: &str = "PGPVERIFY";
    let key_id = match key_thing {
        Value::PgpKey(key) => return Ok(key),
        Value::Long(id) => id as u64,
        Value::String(text) => low_u64(&bc_hex_decode(&text)?),
        _ => {
            return Err(fail(
                NAME,
                "expected a PGP public key or key ring and key id.",
            ));
        }
    };
    let Value::PgpRing(ring) = stack.pop()? else {
        return Err(fail(NAME, "missing PGP key ring."));
    };
    ring.public_keys()
        .find(|key| key.public.key_id == key_id)
        .map(|key| Rc::new(key.clone()))
        .ok_or_else(|| fail(NAME, &format!("key with id 0x{key_id:x} not found.")))
}

/// The RSA key of a key that verifies with its public part.
fn public_of(key: &Key) -> std::result::Result<RsaKey, PgpError> {
    public_rsa(key_body(key))
}

/// `content signature keyring keyid true PGPVERIFY` (a detached signature), or
/// `message keyring keyid PGPVERIFY` (a signed message, whose content is pushed under the
/// boolean), to the boolean of the verification. The key is a `PGPPublicKey` or the key of a
/// ring given by its ID.
fn pgpverify(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PGPVERIFY";
    let wrap = |cause: PgpError| fail_with_cause(NAME, VERIFY_FAILED, Error::from(cause));
    let top = stack.pop()?;
    let (detached, key_thing) = match top {
        Value::Boolean(detached) => (detached, stack.pop()?),
        other => (false, other),
    };
    let key = verifying_key(stack, key_thing)?;
    let public = public_of(&key).map_err(wrap)?;
    if detached {
        let signature = stack.pop()?;
        let signature = input_bytes(&signature)
            .ok_or_else(|| fail(NAME, "expected signature data (STRING or BYTES)."))?
            .map_err(wrap)?;
        let body = detached_body(&signature).map_err(wrap)?;
        let content = stack.pop()?;
        let content = raw_bytes(&content)
            .ok_or_else(|| fail(NAME, "expected content data (STRING or BYTES)."))?;
        let verdict = verify(&public, &body, &content).map_err(wrap)?;
        stack.push(Value::Boolean(verdict));
    } else {
        let message = stack.pop()?;
        let message = input_bytes(&message)
            .ok_or_else(|| fail(NAME, "expected signed message (STRING or BYTES)."))?
            .map_err(wrap)?;
        let (content, body) = read_inline(&message).map_err(wrap)?;
        let verdict = verify(&public, &body, &content).map_err(wrap)?;
        stack.push(Value::bytes(content));
        stack.push(Value::Boolean(verdict));
    }
    Ok(())
}
