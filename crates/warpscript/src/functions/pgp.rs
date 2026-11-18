// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The OpenPGP functions: `PGPRING`, `PGPINFO`, `PGPPUBLIC` and `PGPPRIVATE`. The format is
//! that of [`crate::openpgp`], read and written the way BouncyCastle does.

use std::rc::Rc;

use indexmap::IndexMap;

use super::Entry;
use super::args::{fail, fail_with_cause};
use crate::error::pgp::PgpError;
use crate::error::{Error, Result};
use crate::openpgp::armor;
use crate::openpgp::key::key_id_hex;
use crate::openpgp::ring::{Key, Ring, read_rings};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PGPRING", Entry::Stack(pgpring)),
    ("PGPINFO", Entry::Stack(pginfo)),
    ("PGPPUBLIC", Entry::Stack(pgppublic)),
    ("PGPPRIVATE", Entry::Stack(pgpprivate)),
];

/// The rings of an armored or binary input (`STRING` or `BYTES`).
fn read_input(input: &[u8]) -> Result<Vec<Ring>> {
    let raw = armor::decode(input).map_err(|error| Error::from(PgpError::from(error)))?;
    Ok(read_rings(&raw)?)
}

/// The rings of a `STRING` or `BYTES` on top of the stack, or `None` for another value.
fn input_rings(value: &Value, name: &str, message: &str) -> Option<Result<Vec<Ring>>> {
    let input: &[u8] = match value {
        Value::String(text) => text.as_bytes(),
        Value::Bytes(bytes) => bytes,
        _ => return None,
    };
    Some(read_input(input).map_err(|cause| fail_with_cause(name, message, cause)))
}

/// A ring as a `PGPSecretKeyRing` or `PGPPublicKeyRing` value.
fn ring_value(ring: Ring) -> Value {
    Value::PgpRing(Rc::new(ring))
}

/// A key as a `PGPPublicKey` value.
fn key_value(key: &Key) -> Value {
    Value::PgpKey(Rc::new(key.clone()))
}

/// The key IDs of a ring, each once, in the order of the ring (`getPublicKeys`).
pub(crate) fn key_ids(ring: &Ring) -> Value {
    let mut ids: Vec<String> = Vec::new();
    for key in ring.public_keys() {
        let id = key_id_hex(key.public.key_id);
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Value::list(ids.into_iter().map(Value::string).collect())
}

/// The armored text of a key (`ArmoredOutputStream` over `getEncoded(false)`).
fn armored(key: &Key) -> Value {
    Value::string(armor::encode(&key.encode()))
}

/// The key ID field of a key map.
fn field_keyid(key: &Key) -> Value {
    Value::string(key_id_hex(key.public.key_id))
}

/// The fingerprint field of a key map: the lowercase hexadecimal of the fingerprint.
fn field_fingerprint(key: &Key) -> Value {
    Value::string(key.public.fingerprint_hex())
}

/// The strength field of a key map, in bits.
fn field_bits(key: &Key) -> Value {
    Value::Long(i64::from(key.public.bits))
}

/// The algorithm field of a key map, by its name.
fn field_algorithm(key: &Key) -> Value {
    Value::string(key.public.algorithm_name())
}

/// The user IDs of a key, as strings.
fn field_uid(key: &Key) -> Value {
    Value::list(
        key.user_ids
            .iter()
            .map(|id| Value::string(String::from_utf8_lossy(id)))
            .collect(),
    )
}

/// The user attributes of a key, as the bytes of their packets.
fn field_attributes(key: &Key) -> Value {
    Value::list(key.attributes.iter().map(Value::bytes).collect())
}

/// The expiry field of a key map (0 when the key never expires).
fn field_expiry(key: &Key) -> Value {
    Value::Long(key.expiry())
}

/// The fields `PGPINFO` gives a key, in its order, without the attributes of a ring's keys.
/// `signing` is whether the key can sign: a secret key whose algorithm signs.
fn info_map(key: &Key, signing: bool) -> IndexMap<Value, Value> {
    IndexMap::from([
        (Value::string("keyid"), field_keyid(key)),
        (Value::string("fingerprint"), field_fingerprint(key)),
        (Value::string("uid"), field_uid(key)),
        (Value::string("bits"), field_bits(key)),
        (Value::string("algorithm"), field_algorithm(key)),
        (Value::string("master"), Value::Boolean(key.is_master())),
        (Value::string("signing"), Value::Boolean(signing)),
        (
            Value::string("encryption"),
            Value::Boolean(key.public.is_encryption()),
        ),
        (Value::string("expiry"), field_expiry(key)),
        (Value::string("pubkey"), key_value(key)),
    ])
}

/// The fields `PGPPUBLIC` gives a key, in its order: its armored text first, and no secret.
fn public_map(key: &Key) -> IndexMap<Value, Value> {
    IndexMap::from([
        (Value::string("key"), key_value(key)),
        (Value::string("keyid"), field_keyid(key)),
        (Value::string("fingerprint"), field_fingerprint(key)),
        (Value::string("bits"), field_bits(key)),
        (Value::string("algorithm"), field_algorithm(key)),
        (Value::string("master"), Value::Boolean(key.is_master())),
        (Value::string("signing"), Value::Boolean(false)),
        (
            Value::string("encryption"),
            Value::Boolean(key.public.is_encryption()),
        ),
        (Value::string("uid"), field_uid(key)),
        (Value::string("expiry"), field_expiry(key)),
        (Value::string("pubkey"), key_value(key)),
        (Value::string("attributes"), field_attributes(key)),
    ])
}

/// The map of the keys of a ring, by key ID: the fields of each key, with its attributes.
fn info_of_ring(ring: &Ring) -> Value {
    Value::map(
        ring.public_keys()
            .map(|key| {
                let mut fields = info_map(key, is_signing(key));
                fields.insert(Value::string("attributes"), field_attributes(key));
                (
                    Value::string(key_id_hex(key.public.key_id)),
                    Value::map(fields),
                )
            })
            .collect(),
    )
}

/// Whether a key can sign: a secret key whose algorithm signs.
fn is_signing(key: &Key) -> bool {
    key.secret && key.public.is_signing()
}

/// `PGPRING`: `ring PGPRING` to its encoded bytes, or `input PGPRING` to the list of the rings
/// of an armored or binary input.
fn pgpring(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PGPRING";
    let top = stack.pop()?;
    if let Value::PgpRing(ring) = &top {
        stack.push(Value::bytes(ring.encode()));
        return Ok(());
    }
    let Some(rings) = input_rings(&top, NAME, "error decoding PGP key ring.") else {
        return Err(fail(NAME, "invalid PGP ring, expected STRING or BYTES."));
    };
    let rings = rings?;
    stack.push(Value::list(rings.into_iter().map(ring_value).collect()));
    Ok(())
}

/// `PGPINFO`: `ring PGPINFO` to the map of the keys of the ring, by key ID, or `key PGPINFO` to
/// the map of the key.
fn pginfo(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PGPINFO";
    let info = match stack.pop()? {
        Value::PgpRing(ring) => info_of_ring(&ring),
        Value::PgpKey(key) => Value::map(info_map(&key, is_signing(&key))),
        _ => {
            return Err(fail(NAME, "expected a PGP public key or secret key ring."));
        }
    };
    stack.push(info);
    Ok(())
}

/// `PGPPUBLIC`: `key PGPPUBLIC` to the armored key, `ring PGPPUBLIC` to the key IDs of the ring,
/// `input PGPPUBLIC` to the map of the public keys of the input, and `ring keyid PGPPUBLIC` to
/// nothing when the key is in the ring.
fn pgppublic(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PGPPUBLIC";
    const DECODING: &str = "error decoding public key.";
    match stack.pop()? {
        Value::Long(key_id) => {
            let Value::PgpRing(ring) = stack.pop()? else {
                return Err(fail(NAME, "expected PGP public or secret key ring."));
            };
            let id = key_id as u64;
            if !ring.public_keys().any(|key| key.public.key_id == id) {
                return Err(fail(NAME, &format!("key with id 0x{id:x} not found.")));
            }
        }
        Value::PgpRing(ring) => stack.push(key_ids(&ring)),
        Value::PgpKey(key) => stack.push(armored(&key)),
        top => {
            let Some(rings) = input_rings(&top, NAME, DECODING) else {
                return Err(fail(NAME, "invalid public key, expected STRING or BYTES."));
            };
            let rings = rings?;
            let mut keys = IndexMap::new();
            for ring in &rings {
                if ring.secret {
                    let found = PgpError::NotPublicRing {
                        found: "org.bouncycastle.openpgp.PGPSecretKeyRing",
                    };
                    return Err(fail_with_cause(NAME, DECODING, found.into()));
                }
                for key in ring.public_keys() {
                    keys.insert(
                        Value::string(key_id_hex(key.public.key_id)),
                        Value::map(public_map(key)),
                    );
                }
            }
            stack.push(Value::map(keys));
        }
    }
    Ok(())
}

/// `PGPPRIVATE`: `ring PGPPRIVATE` to the key IDs of a secret key ring.
fn pgpprivate(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PGPPRIVATE";
    let Value::PgpRing(ring) = stack.pop()? else {
        return Err(fail(NAME, "expected a PGP secret key ring."));
    };
    if !ring.secret {
        return Err(fail(NAME, "expected a PGP secret key ring."));
    }
    stack.push(key_ids(&ring));
    Ok(())
}
