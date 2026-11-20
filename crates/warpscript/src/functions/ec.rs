// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The elliptic curve functions (`ECGEN`, `ECPRIVATE`, `ECPUBLIC`, `ECSIGN`, `ECVERIFY`,
//! `ECRECOVER`, `ECDH`) and the signature of macros (`MSIGN`, `MSIG`, `MSIGCOUNT`, `MSIGINFO`,
//! `MVERIFY`, `MCHECKSIG`, `MDETACH`). The curves are those of [`crate::ec`].

use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use num_bigint::{BigInt, BigUint};
use sha1::Sha1;
use sha2::{Digest, Sha224, Sha256, Sha384, Sha512};

use super::Entry;
use super::args::{fail, fail_with_cause};
use super::bd::to_big_integer;
use super::crypto::sha3;
use crate::bigdecimal::parse_bigint;
use crate::ec::{
    CURVE_NAMES, Candidate, Curve, Key, Point, PrivateKey, PublicKey, Random, der_decode,
    der_encode, system_random,
};
use crate::encoding::hex_encode;
use crate::error::crypto::CryptoError;
use crate::error::fault::NumberFormatError;
use crate::error::{Error, Result};
use crate::macros::{Callable, Macro, Statement};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("ECGEN", Entry::Stack(ecgen)),
    ("ECDH", Entry::Stack(ecdh)),
    ("ECPRIVATE", Entry::Stack(ecprivate)),
    ("ECPUBLIC", Entry::Stack(ecpublic)),
    ("ECRECOVER", Entry::Stack(ecrecover)),
    ("ECSIGN", Entry::Stack(ecsign)),
    ("ECVERIFY", Entry::Stack(ecverify)),
    ("MSIGN", Entry::Stack(msign)),
    ("MSIG", Entry::Stack(msig)),
    ("MSIGCOUNT", Entry::Stack(msigcount)),
    ("MSIGINFO", Entry::Stack(msiginfo)),
    ("MVERIFY", Entry::Stack(|s| mverify(s, "MVERIFY", true))),
    (
        "MCHECKSIG",
        Entry::Stack(|s| mverify(s, "MCHECKSIG", false)),
    ),
    ("MDETACH", Entry::Stack(mdetach)),
];

/// The algorithm of the signature of macros.
const SIGNATURE_ALGORITHM: &str = "SHA256WITHECDSA";

/// The names of the curves, as the messages list them.
fn curves() -> String {
    CURVE_NAMES.join(", ")
}

/// `String.valueOf` of the value of a parameter.
fn string_of(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => "null".to_owned(),
        Some(Value::String(text)) => text.to_string(),
        Some(other) => other.to_string(),
    }
}

/// The value of a parameter, when it is there and is not `null`.
fn parameter<'a>(params: &'a IndexMap<Value, Value>, name: &str) -> Option<&'a Value> {
    params
        .get(&Value::string(name))
        .filter(|value| !matches!(value, Value::Null))
}

fn key_value(key: Key) -> Value {
    Value::Key(Rc::new(key))
}

/// The map of a private key, which `ECPRIVATE` reads.
fn private_map(key: &PrivateKey) -> Value {
    Value::map(IndexMap::from([
        (Value::string("curve"), Value::string(&key.name)),
        (Value::string("d"), Value::string(key.d.to_string())),
    ]))
}

/// The map of a public key, which `ECPUBLIC` reads.
fn public_map(key: &PublicKey) -> Value {
    Value::map(IndexMap::from([
        (Value::string("curve"), Value::string(&key.name)),
        (Value::string("Q"), Value::string(hex_encode(&key.encoded))),
    ]))
}

/// A private key popped from the stack.
fn pop_private(stack: &mut Stack, function: &str, message: &str) -> Result<PrivateKey> {
    match stack.pop()? {
        Value::Key(key) => key
            .ec_private()
            .cloned()
            .ok_or_else(|| fail(function, message)),
        _ => Err(fail(function, message)),
    }
}

/// A public key popped from the stack.
fn pop_public(stack: &mut Stack, function: &str, message: &str) -> Result<PublicKey> {
    match stack.pop()? {
        Value::Key(key) => key
            .ec_public()
            .cloned()
            .ok_or_else(|| fail(function, message)),
        _ => Err(fail(function, message)),
    }
}

/// `ECGEN`: a curve name to the maps of a private key and, on top of it, of its public key.
fn ecgen(stack: &mut Stack) -> Result<()> {
    ecgen_with(stack, &mut system_random)
}

/// `ECGEN` with a source of random bytes.
pub(crate) fn ecgen_with(stack: &mut Stack, random: Random<'_>) -> Result<()> {
    const NAME: &str = "ECGEN";
    let Value::String(name) = stack.pop()? else {
        return Err(fail(NAME, "expects a curve name."));
    };
    let Some(curve) = Curve::lookup(&name) else {
        return Err(fail(
            NAME,
            &format!("only supports the following curves: {}.", curves()),
        ));
    };
    let d = curve.generate(random).map_err(CryptoError::from)?;
    let private = PrivateKey {
        curve,
        name: name.to_string(),
        d: BigInt::from(d),
    };
    stack.push(private_map(&private));
    stack.push(public_map(&private.public()));
    Ok(())
}

/// The hexadecimal text `Hex.decode` of BouncyCastle reads: pairs of digits, with the white
/// space between them ignored.
pub(crate) fn bc_hex_decode(text: &str) -> core::result::Result<Vec<u8>, CryptoError> {
    let mut digits = Vec::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\n' | '\r' | '\t' | ' ') {
            continue;
        }
        match c.to_digit(16) {
            Some(digit) => digits.push(digit as u8),
            None => {
                return Err(CryptoError::HexDecoding(
                    "invalid characters encountered in Hex string".to_owned(),
                ));
            }
        }
    }
    let (pairs, rest) = digits.as_chunks::<2>();
    if rest.is_empty() {
        Ok(pairs.iter().map(|[high, low]| high << 4 | low).collect())
    } else {
        Err(CryptoError::HexDecoding(format!(
            "String index out of range: {}",
            text.chars().count()
        )))
    }
}

/// The curve of a parameter map, by the name under `curve`.
fn curve_of(params: &IndexMap<Value, Value>) -> (String, Option<Curve>) {
    let name = string_of(params.get(&Value::string("curve")));
    let curve = Curve::lookup(&name);
    (name, curve)
}

/// `ECPRIVATE`: the map of a private key to the key, and the key to its map.
fn ecprivate(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ECPRIVATE";
    const MESSAGE: &str = "expects a parameter map or an ECC private key instance.";
    match stack.pop()? {
        Value::Key(key) => {
            let Some(private) = key.ec_private() else {
                return Err(fail(NAME, MESSAGE));
            };
            stack.push(private_map(private));
            Ok(())
        }
        Value::Map(params) => {
            let params = params.borrow();
            let (name, curve) = curve_of(&params);
            let Some(curve) = curve else {
                return Err(fail(NAME, &format!("curve name not in {}.", curves())));
            };
            let Some(d) = parameter(&params, "d") else {
                return Err(fail(NAME, "missing private key under 'd'."));
            };
            let d = to_big_integer(NAME, d)?;
            stack.push(key_value(Key::Private(PrivateKey { curve, name, d })));
            Ok(())
        }
        _ => Err(fail(NAME, MESSAGE)),
    }
}

/// `ECPUBLIC`: the map of a public key, or a private key, to the public key, and a public key
/// to its map.
fn ecpublic(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ECPUBLIC";
    match stack.pop()? {
        Value::Key(key) => {
            match (key.ec_public(), key.ec_private()) {
                (Some(public), _) => stack.push(public_map(public)),
                (None, Some(private)) => {
                    stack.push(key_value(Key::Public(Box::new(private.public()))));
                }
                (None, None) => {
                    return Err(fail(
                        NAME,
                        "expects a parameter map, or a public or private key.",
                    ));
                }
            }
            Ok(())
        }
        Value::Map(params) => {
            let params = params.borrow();
            let (name, curve) = curve_of(&params);
            let Some(curve) = curve else {
                return Err(fail(
                    NAME,
                    &format!("curve name '{name}' not in {}.", curves()),
                ));
            };
            let Some(Value::String(q)) = parameter(&params, "Q") else {
                return Err(fail(NAME, "missing or non-String parameter 'Q'."));
            };
            let encoded = bc_hex_decode(q).map_err(Error::from)?;
            let public = PublicKey::new(curve, &name, encoded).map_err(CryptoError::from)?;
            stack.push(key_value(Key::Public(Box::new(public))));
            Ok(())
        }
        _ => Err(fail(
            NAME,
            "expects a parameter map, or a public or private key.",
        )),
    }
}

/// The digest an algorithm of signature (`SHA256WITHECDSA`) applies to the data.
fn signed_hash(algorithm: &str, data: &[u8]) -> core::result::Result<Vec<u8>, CryptoError> {
    let upper = algorithm.to_ascii_uppercase();
    let digest = match upper.as_str() {
        "ECDSA" => "SHA1",
        other => other.strip_suffix("WITHECDSA").unwrap_or(""),
    };
    Ok(match digest {
        "NONE" => data.to_vec(),
        "SHA1" => Sha1::digest(data).to_vec(),
        "SHA224" => Sha224::digest(data).to_vec(),
        "SHA256" => Sha256::digest(data).to_vec(),
        "SHA384" => Sha384::digest(data).to_vec(),
        "SHA512" => Sha512::digest(data).to_vec(),
        "SHA3-224" => sha3(224, data),
        "SHA3-256" => sha3(256, data),
        "SHA3-384" => sha3(384, data),
        "SHA3-512" => sha3(512, data),
        "RIPEMD160" => ripemd::Ripemd160::digest(data).to_vec(),
        _ => return Err(CryptoError::NoSuchAlgorithm(algorithm.to_owned())),
    })
}

/// The DER signature of the data by a private key, in the order Warp 10 checks what it is given:
/// the algorithm, then the private number, then the signature itself.
fn sign_data(
    private: &PrivateKey,
    algorithm: &str,
    data: &[u8],
    random: Random<'_>,
) -> Result<Vec<u8>> {
    let wrapped = |cause: CryptoError| {
        fail_with_cause("ECSIGN", "error signing content.", Error::from(cause))
    };
    let hash = signed_hash(algorithm, data).map_err(wrapped)?;
    let d = private
        .curve
        .private_scalar(&private.d)
        .map_err(CryptoError::from)?;
    let signature = private
        .curve
        .sign(&d, &hash, random)
        .map_err(|cause| wrapped(cause.into()))?;
    Ok(der_encode(&signature))
}

/// `ECSIGN`: `data algorithm key ECSIGN` to the DER signature.
fn ecsign(stack: &mut Stack) -> Result<()> {
    ecsign_with(stack, &mut system_random)
}

/// `ECSIGN` with a source of random bytes.
pub(crate) fn ecsign_with(stack: &mut Stack, random: Random<'_>) -> Result<()> {
    const NAME: &str = "ECSIGN";
    let private = pop_private(stack, NAME, "expects an ECC private key.")?;
    let Value::String(algorithm) = stack.pop()? else {
        return Err(fail(NAME, "expects an algorithm name."));
    };
    let Value::Bytes(data) = stack.pop()? else {
        return Err(fail(NAME, "operates on a byte array."));
    };
    let signature = sign_data(&private, &algorithm, &data, random)?;
    stack.push(Value::bytes(signature));
    Ok(())
}

/// Whether a DER signature of the data is the one of the owner of a public key.
fn verify_data(
    public: &PublicKey,
    algorithm: &str,
    data: &[u8],
    signature: &[u8],
) -> core::result::Result<bool, CryptoError> {
    let hash = signed_hash(algorithm, data)?;
    let signature = der_decode(signature)?;
    Ok(public.curve.verify(&public.point, &hash, &signature))
}

/// `ECVERIFY`: `data signature algorithm key ECVERIFY` to a boolean.
fn ecverify(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ECVERIFY";
    let public = pop_public(stack, NAME, "expects an ECC public key.")?;
    let Value::String(algorithm) = stack.pop()? else {
        return Err(fail(NAME, "expects an algorithm name."));
    };
    let Value::Bytes(signature) = stack.pop()? else {
        return Err(fail(NAME, "expects a signature."));
    };
    let Value::Bytes(data) = stack.pop()? else {
        return Err(fail(NAME, "operates on a byte array."));
    };
    let verified = verify_data(&public, &algorithm, &data, &signature).map_err(|cause| {
        fail_with_cause(NAME, "error while verifying signature.", Error::from(cause))
    })?;
    stack.push(verified);
    Ok(())
}

/// `ECDH`: `private-key public-key ECDH` to the secret of their agreement, in hexadecimal.
fn ecdh(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ECDH";
    let public = pop_public(stack, NAME, "expects an ECC public key.")?;
    let private = pop_private(stack, NAME, "expects an ECC private key.")?;
    if private.curve != public.curve {
        return Err(CryptoError::CurveMismatch.into());
    }
    let d = private
        .curve
        .private_scalar(&private.d)
        .map_err(CryptoError::from)?;
    let secret = private.curve.agree(&d, &public.point).to_str_radix(16);
    stack.push(if secret.len() % 2 == 0 {
        secret
    } else {
        format!("0{secret}")
    });
    Ok(())
}

/// A number written as `0x…` in hexadecimal or in decimal, like `ECRECOVER` reads it.
fn recover_number(text: &str) -> core::result::Result<BigInt, NumberFormatError> {
    let lower = text.to_lowercase();
    let (digits, radix) = match lower.strip_prefix("0x") {
        Some(hex) => (format!("00{hex}"), 16),
        None => (lower, 10),
    };
    parse_bigint(&digits, radix).map_err(|err| match err {
        // The message of the oracle is the one of JDK 8, which does not tell the radix.
        NumberFormatError::InvalidRadixInput { input, .. } => {
            NumberFormatError::InvalidInput(input)
        }
        other => other,
    })
}

/// The `r` and the `s` of a DER signature the way `ECRECOVER` reads them: at fixed places, the
/// bytes after those of `r` being `s` (the quirks of the reader are those of Warp 10).
fn recover_signature(signature: &[u8]) -> core::result::Result<(BigInt, BigInt), CryptoError> {
    let at = |index: usize| {
        signature
            .get(index)
            .copied()
            .ok_or(CryptoError::SignatureIndex(index))
    };
    let mut offset = 2;
    let length = at(1)?;
    if length & 0x80 != 0 {
        // The length is on `sig[1] - 0x80` bytes.
        for _ in 0..usize::from(length & 0x7f) {
            at(offset)?;
            offset += 1;
        }
    }
    // The tag of the integer.
    offset += 1;
    // The length is a signed byte.
    let r_length = i64::from(at(offset)? as i8);
    offset += 1;
    // `Arrays.copyOfRange(sig, offset, offset + length)`: a range that ends before it starts is
    // refused, what is past the end is padded with zeroes.
    let to = offset as i64 + r_length;
    if to < offset as i64 {
        return Err(CryptoError::RangeInverted {
            from: offset as i64,
            to,
        });
    }
    let r_end = to as usize;
    // `offset` is at most the length of the signature: its byte before was read.
    let mut r_bytes = signature[offset..r_end.min(signature.len())].to_vec();
    r_bytes.resize(r_end - offset, 0);
    offset = r_end + 2;
    if offset > signature.len() {
        return Err(CryptoError::RangeInverted {
            from: offset as i64,
            to: signature.len() as i64,
        });
    }
    let r = BigInt::from(BigUint::from_bytes_be(&r_bytes));
    let s = BigInt::from(BigUint::from_bytes_be(&signature[offset..]));
    Ok((r, s))
}

/// `ECRECOVER`: the public keys that can have made a signature of a hash.
fn ecrecover(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ECRECOVER";
    let Value::Map(params) = stack.pop()? else {
        return Err(fail(NAME, "expects a MAP."));
    };
    let params = params.borrow();
    let Some(Value::String(curve_name)) = params.get(&Value::string("curve")) else {
        return Err(fail(NAME, "missing ECC curve name under 'curve'."));
    };
    let Some(curve) = Curve::lookup(curve_name) else {
        return Err(Error::null_pointer());
    };
    let Some(Value::Bytes(hash)) = parameter(&params, "hash") else {
        return Err(fail(NAME, "invalid 'hash', expected BYTES."));
    };
    let order = curve.order();
    let (r, s) = if let Some(signature) = parameter(&params, "sig") {
        let Value::Bytes(signature) = signature else {
            return Err(fail(NAME, "invalid 'sig', expected BYTES."));
        };
        let (r, s) = recover_signature(signature).map_err(Error::from)?;
        for (name, number) in [("r", &r), ("s", &s)] {
            if *number > BigInt::from(order.clone()) || *number < BigInt::from(1) {
                return Err(fail(
                    NAME,
                    &format!(
                        "invalid value for {name}, should be in [1, 0x00{}] but was 0x{}.",
                        order.to_str_radix(16),
                        number.to_str_radix(16)
                    ),
                ));
            }
        }
        (r, s)
    } else if let (Some(r), Some(s)) = (parameter(&params, "r"), parameter(&params, "s")) {
        let Value::String(r) = r else {
            return Err(fail(NAME, "invalid 'r', expected STRING."));
        };
        let Value::String(s) = s else {
            return Err(fail(NAME, "invalid 's', expected STRING."));
        };
        (
            recover_number(r).map_err(Error::from)?,
            recover_number(s).map_err(Error::from)?,
        )
    } else {
        return Err(fail(NAME, "expects 'sig' or 'r' and 's' to be provided."));
    };
    // The multiples of the order added to `r`: the cofactor of the curve and below, or the
    // one given.
    let cofactors = match parameter(&params, "i") {
        Some(Value::Long(i)) => *i..=*i,
        _ => 0..=1,
    };
    let parities: &[u8] = match parameter(&params, "even") {
        Some(Value::Boolean(true)) => &[2],
        Some(Value::Boolean(false)) => &[3],
        _ => &[2, 3],
    };
    let coords = matches!(parameter(&params, "coords"), Some(Value::Boolean(true)));
    let candidates = curve
        .recover(&r, &s, hash, cofactors, parities)
        .map_err(CryptoError::from)?;
    let key = |point: &Point| {
        key_value(Key::Public(Box::new(PublicKey {
            curve,
            name: curve_name.to_string(),
            encoded: point.encode(),
            point: *point,
        })))
    };
    let keys = if coords {
        candidates
            .iter()
            .map(|Candidate { key: point, x, y }| {
                Value::map(IndexMap::from([
                    (
                        Value::string("R.x"),
                        Value::string(format!("0x{}", x.to_str_radix(16))),
                    ),
                    (
                        Value::string("R.y"),
                        Value::string(format!("0x{}", y.to_str_radix(16))),
                    ),
                    (Value::string("pubkey"), key(point)),
                ]))
            })
            .collect()
    } else {
        // One key for each point, whatever the point `R` it comes from.
        let mut seen = IndexSet::new();
        candidates
            .iter()
            .filter(|candidate| seen.insert(candidate.key.encode()))
            .map(|candidate| key(&candidate.key))
            .collect()
    };
    stack.push(Value::list(keys));
    Ok(())
}

/// Whether a statement of a macro is the call of `MSIG`.
fn is_msig(statement: &Statement) -> bool {
    matches!(statement, Statement::Call(callable) if callable.name() == "MSIG")
}

/// What a macro is signed with: a curve name, a public key and a signature, in text.
struct Signing<'a> {
    curve: &'a str,
    key: &'a str,
    signature: &'a str,
}

/// The signature a list of statements ends with: three texts and `MSIG`.
fn signing(statements: &[Statement]) -> Option<Signing<'_>> {
    match statements {
        [
            ..,
            Statement::Push(Value::String(curve)),
            Statement::Push(Value::String(key)),
            Statement::Push(Value::String(signature)),
            last,
        ] if is_msig(last) => Some(Signing {
            curve,
            key,
            signature,
        }),
        _ => None,
    }
}

/// The four statements that sign a macro, if it ends with them.
fn signature_statements(macro_: &Macro) -> &[Statement] {
    let statements = macro_.statements.as_slice();
    match signing(statements) {
        Some(_) => &statements[statements.len() - 4..],
        None => &[],
    }
}

fn macro_value(statements: Vec<Statement>) -> Value {
    Value::Macro(Rc::new(Macro::from_statements(statements)))
}

/// `MSIGN`: `macro key MSIGN` to the macro and the macro that signs it.
fn msign(stack: &mut Stack) -> Result<()> {
    msign_with(stack, &mut system_random)
}

/// `MSIGN` with a source of random bytes.
pub(crate) fn msign_with(stack: &mut Stack, random: Random<'_>) -> Result<()> {
    const NAME: &str = "MSIGN";
    let private = pop_private(stack, NAME, "expects an ECC private key.")?;
    let Value::Macro(macro_) = stack.pop()? else {
        return Err(fail(NAME, "operates on a macro."));
    };
    let data = macro_.snapshot(false);
    let signature = sign_data(&private, SIGNATURE_ALGORITHM, data.as_bytes(), random)?;
    let public = private.public();
    let signing = vec![
        Statement::Push(Value::string(&private.name)),
        Statement::Push(Value::string(hex_encode(&public.encoded))),
        Statement::Push(Value::string(hex_encode(&signature))),
        Statement::Call(Callable::Native {
            name: "MSIG",
            func: msig,
        }),
    ];
    stack.push(Value::Macro(macro_));
    stack.push(macro_value(signing));
    Ok(())
}

/// `MSIG`: the signature of a macro, or (when executed by a signed macro) nothing.
fn msig(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MSIG";
    match stack.pop()? {
        Value::Macro(macro_) => {
            let signature = signature_statements(&macro_).to_vec();
            stack.push(Value::Macro(macro_));
            stack.push(macro_value(signature));
        }
        Value::String(_) => {
            let Value::String(_) = stack.pop()? else {
                return Err(fail(NAME, "expects a hex encoded ECC public key."));
            };
            let Value::String(_) = stack.pop()? else {
                return Err(fail(NAME, "expects an ECC curve name."));
            };
        }
        _ => return Err(fail(NAME, "expects a hex encoded signature.")),
    }
    Ok(())
}

/// `MSIGCOUNT`: the number of signatures a macro ends with.
fn msigcount(stack: &mut Stack) -> Result<()> {
    let Value::Macro(macro_) = stack.pop()? else {
        return Err(fail("MSIGCOUNT", "operates on a Macro."));
    };
    let mut statements = macro_.statements.as_slice();
    let mut count = 0i64;
    while signing(statements).is_some() {
        count += 1;
        statements = &statements[..statements.len() - 4];
    }
    stack.push(count);
    Ok(())
}

/// The key and the signature of the signature of a macro.
fn signature_parts(signing: &Signing<'_>) -> Result<(PublicKey, Vec<u8>)> {
    let Some(curve) = Curve::lookup(signing.curve) else {
        return Err(Error::null_pointer());
    };
    let encoded = bc_hex_decode(signing.key).map_err(Error::from)?;
    let public = PublicKey::new(curve, signing.curve, encoded).map_err(CryptoError::from)?;
    let signature = bc_hex_decode(signing.signature).map_err(Error::from)?;
    Ok((public, signature))
}

/// `MSIGINFO`: the macro and a map of the signature and the key of its (last) signature, or
/// `false` when it has none.
fn msiginfo(stack: &mut Stack) -> Result<()> {
    let Value::Macro(macro_) = stack.pop()? else {
        return Err(fail("MSIGINFO", "expects a macro."));
    };
    let Some(signing) = signing(&macro_.statements) else {
        stack.push(false);
        return Ok(());
    };
    let (public, signature) = signature_parts(&signing)?;
    let info = IndexMap::from([
        (Value::string("sig"), Value::bytes(signature)),
        (
            Value::string("key"),
            key_value(Key::Public(Box::new(public))),
        ),
    ]);
    stack.push(Value::Macro(macro_));
    stack.push(Value::map(info));
    Ok(())
}

/// Whether the (last) signature of a macro is good: the signature of the macro without it.
fn verified(macro_: &Macro) -> Result<bool> {
    let Some(signing) = signing(&macro_.statements) else {
        return Ok(false);
    };
    let (public, signature) = signature_parts(&signing)?;
    let signed = Macro::from_statements(macro_.statements[..macro_.statements.len() - 4].to_vec());
    let data = signed.snapshot(false);
    verify_data(&public, SIGNATURE_ALGORITHM, data.as_bytes(), &signature).map_err(|cause| {
        fail_with_cause(
            "ECVERIFY",
            "error while verifying signature.",
            Error::from(cause),
        )
    })
}

/// `MVERIFY` (`require`, the macro must be signed) and `MCHECKSIG` (the verdict is pushed).
fn mverify(stack: &mut Stack, name: &str, require: bool) -> Result<()> {
    let Value::Macro(macro_) = stack.pop()? else {
        return Err(fail(name, "operates on a macro."));
    };
    let verdict = verified(&macro_)?;
    stack.push(Value::Macro(macro_));
    if !require {
        stack.push(verdict);
    } else if !verdict {
        return Err(fail(name, "unable to verify macro."));
    }
    Ok(())
}

/// `MDETACH`: the macro without its (last) signature, and the signature.
fn mdetach(stack: &mut Stack) -> Result<()> {
    let Value::Macro(macro_) = stack.pop()? else {
        return Err(fail("MDETACH", "operates on a macro."));
    };
    let signature = signature_statements(&macro_);
    let kept = macro_.statements[..macro_.statements.len() - signature.len()].to_vec();
    let detached = Macro::from_statements(kept);
    detached.secure.set(macro_.is_secure());
    let signature = signature.to_vec();
    stack.push(Value::Macro(Rc::new(detached)));
    stack.push(macro_value(signature));
    Ok(())
}
