// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `->SSSS` and `SSSS->`: Shamir's secret sharing of a byte array. The scheme itself lives in
//! [`crate::ssss`].

use super::Entry;
use super::args::fail;
use crate::error::crypto::{CryptoError, RecoverError};
use crate::error::{Error, Result};
use crate::jmath::to_int_exact;
use crate::ssss::{self, RandomByte};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("->SSSS", Entry::Stack(tossss)),
    ("SSSS->", Entry::Stack(ssssto)),
];

/// One random byte from the operating system.
fn system_random_byte() -> core::result::Result<u8, getrandom::Error> {
    let mut byte = [0u8];
    getrandom::fill(&mut byte).map(|()| byte[0])
}

/// `->SSSS`: split a byte array in shares, `bytes shares needed [ensure_randomness] ->SSSS`.
fn tossss(stack: &mut Stack) -> Result<()> {
    tossss_with(stack, &mut system_random_byte)
}

/// `->SSSS` with the given source of random bytes.
pub(crate) fn tossss_with(stack: &mut Stack, random: RandomByte<'_>) -> Result<()> {
    const NAME: &str = "->SSSS";
    let mut top = stack.pop()?;
    let ensure_randomness = match top {
        Value::Boolean(ensure) => {
            top = stack.pop()?;
            ensure
        }
        _ => true,
    };
    let Value::Long(needed) = top else {
        return Err(fail(
            NAME,
            "expects the number of splits needed to reconstruct the input.",
        ));
    };
    let needed = to_int_exact(needed)?;
    if !(2..=255).contains(&needed) {
        return Err(fail(
            NAME,
            "expects the number of needed splits to be between 2 and 255.",
        ));
    }
    let Value::Long(shares) = stack.pop()? else {
        return Err(fail(NAME, "expects the number of splits to generate."));
    };
    let shares = to_int_exact(shares)?;
    if shares > 255 {
        return Err(fail(
            NAME,
            "the number of splits must be strictly less than 256.",
        ));
    }
    if shares < needed {
        return Err(fail(
            NAME,
            "the number of splits to generate must be at least equal to the number of needed splits.",
        ));
    }
    let Value::Bytes(secret) = stack.pop()? else {
        return Err(fail(NAME, "operates on a byte array."));
    };
    let mut shares = ssss::split(&secret, shares as usize, needed as usize, random)
        .map_err(|err| Error::from(CryptoError::from(err)))?;
    if !ensure_randomness {
        for share in &mut shares {
            *share = share
                .as_chunks::<2>()
                .0
                .iter()
                .filter(|[x, _]| *x != 0)
                .flatten()
                .copied()
                .collect();
        }
    }
    stack.push(Value::list(shares.into_iter().map(Value::bytes).collect()));
    Ok(())
}

/// `SSSS->`: recover a byte array from a list of shares; `NULL` when two shares have the same
/// abscissa, which happens when they do not belong to the same split.
fn ssssto(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "SSSS->";
    const MESSAGE: &str = "operates on a LIST of BYTES (byte arrays).";
    let Value::List(list) = stack.pop()? else {
        return Err(fail(NAME, MESSAGE));
    };
    let shares = list
        .borrow()
        .iter()
        .map(|item| match item {
            Value::Bytes(share) => Some(share.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| fail(NAME, MESSAGE))?;
    match ssss::recover(&shares) {
        Ok(secret) => stack.push(Value::bytes(secret)),
        Err(RecoverError::DuplicateAbscissa) => stack.push(Value::Null),
        Err(RecoverError::NoShares) => return Err(Error::null_pointer()),
    }
    Ok(())
}
