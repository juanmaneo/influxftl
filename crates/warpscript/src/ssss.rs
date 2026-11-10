// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Shamir's secret sharing over GF(256), the scheme behind `->SSSS` and `SSSS->`.
//!
//! The wire format is the one of the OSS `CryptoHelper` Warp 10 uses, so shares made by one
//! implementation are recovered by the other. Each secret byte is the constant term of a random
//! polynomial of degree `needed - 1` over GF(256) (generator `2`, prime polynomial `0x11d`).
//! A share is a sequence of `(x, y)` byte pairs, one per secret byte, where `y = P(x)`; an
//! abscissa of `0` never carries data, a pair `(0, random)` is inserted instead so the shares
//! pass randomness tests, and the readers skip it.

use core::num::NonZeroU8;

use crate::error::crypto::{RecoverError, SplitError};

/// The reduction polynomial of GF(256), `x^8 + x^4 + x^3 + x^2 + 1`.
const PRIME_POLYNOMIAL: u16 = 0x11d;

/// A source of random bytes, one at a time.
pub type RandomByte<'a> = &'a mut dyn FnMut() -> Result<u8, getrandom::Error>;

/// The arithmetic of GF(256), through the exponentiation and logarithm tables of the generator.
#[derive(Debug)]
pub struct Field {
    /// `exp[i]` is the generator raised to `i`.
    exp: [u8; 255],
    /// `log[v]` is the power of the generator that gives `v` (`log[0]` is unused).
    log: [u8; 256],
}

impl Field {
    /// The tables of the field.
    pub fn new() -> Self {
        let mut exp = [0u8; 255];
        let mut log = [0u8; 256];
        let mut value: u16 = 1;
        for (power, slot) in (0u8..255).zip(exp.iter_mut()) {
            *slot = value as u8;
            log[usize::from(value as u8)] = power;
            value <<= 1;
            if value > 0xff {
                value ^= PRIME_POLYNOMIAL;
            }
        }
        Field { exp, log }
    }

    /// The product of two elements.
    pub fn mul(&self, a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            return 0;
        }
        let power = usize::from(self.log[usize::from(a)]) + usize::from(self.log[usize::from(b)]);
        self.exp[power % 255]
    }

    /// The multiplicative inverse of a non-zero element.
    pub fn inverse(&self, a: NonZeroU8) -> u8 {
        let power = 255 - usize::from(self.log[usize::from(a.get())]);
        self.exp[power % 255]
    }

    /// The value at `x` of the polynomial whose coefficients are `coefficients`, constant term
    /// first (Horner's rule; the addition of GF(256) is the exclusive or).
    pub fn evaluate(&self, coefficients: &[u8], x: u8) -> u8 {
        coefficients
            .iter()
            .rev()
            .fold(0, |acc, &coefficient| self.mul(acc, x) ^ coefficient)
    }
}

impl Default for Field {
    fn default() -> Self {
        Self::new()
    }
}

/// The most shares a secret can be split in: the abscissae are the non-zero bytes.
pub const MAX_SHARES: usize = 255;

/// Split `secret` in `shares` shares, any `needed` of which recover it.
///
/// `random` supplies the random bytes of the polynomials and of the abscissae.
pub fn split(
    secret: &[u8],
    shares: usize,
    needed: usize,
    random: RandomByte<'_>,
) -> Result<Vec<Vec<u8>>, SplitError> {
    if !(2..=MAX_SHARES).contains(&shares) || needed < 2 || needed > shares {
        return Err(SplitError::InvalidParameters { shares, needed });
    }
    let field = Field::new();
    let mut result = vec![Vec::with_capacity(secret.len() * 2); shares];
    for &byte in secret {
        let coefficients = random_polynomial(byte, needed, random)?;
        let mut picked = [false; 256];
        for share in &mut result {
            let x = loop {
                let x = random()?;
                let Some(x) = NonZeroU8::new(x) else {
                    share.extend([0, random()?]);
                    continue;
                };
                if !picked[usize::from(x.get())] {
                    break x;
                }
            };
            picked[usize::from(x.get())] = true;
            share.extend([x.get(), field.evaluate(&coefficients, x.get())]);
        }
    }
    Ok(result)
}

/// The coefficients, constant term first, of a random polynomial of degree `needed - 1`
/// whose value at `0` is `constant` and whose leading coefficient is not zero.
fn random_polynomial(
    constant: u8,
    needed: usize,
    random: RandomByte<'_>,
) -> Result<Vec<u8>, SplitError> {
    let mut coefficients = Vec::with_capacity(needed);
    coefficients.push(constant);
    for _ in 2..needed {
        coefficients.push(random()?);
    }
    let leading = loop {
        if let Some(leading) = NonZeroU8::new(random()?) {
            break leading;
        }
    };
    coefficients.push(leading.get());
    Ok(coefficients)
}

/// The next `(x, y)` pair of a share, skipping those whose abscissa is `0`, or `None` when
/// the share is exhausted (an unpaired trailing byte counts as exhausted).
fn next_point(share: &mut &[u8]) -> Option<(NonZeroU8, u8)> {
    loop {
        let [x, y, rest @ ..] = *share else {
            return None;
        };
        *share = rest;
        if let Some(x) = NonZeroU8::new(*x) {
            return Some((x, *y));
        }
    }
}

/// The value at `0` of the polynomial through `points`, by Lagrange interpolation: the sum over
/// `j` of `y_j * prod(x_i / (x_i + x_j))` for `i != j`.
fn interpolate_at_zero(field: &Field, points: &[(NonZeroU8, u8)]) -> Result<u8, RecoverError> {
    let mut secret = 0;
    for (j, &(xj, yj)) in points.iter().enumerate() {
        let mut term = yj;
        for (i, &(xi, _)) in points.iter().enumerate() {
            if i == j {
                continue;
            }
            let gap = NonZeroU8::new(xi.get() ^ xj.get()).ok_or(RecoverError::DuplicateAbscissa)?;
            term = field.mul(field.mul(term, xi.get()), field.inverse(gap));
        }
        secret ^= term;
    }
    Ok(secret)
}

/// Recover a secret from `shares`. It stops at the first share that runs out of points.
pub fn recover<S: AsRef<[u8]>>(shares: &[S]) -> Result<Vec<u8>, RecoverError> {
    if shares.is_empty() {
        return Err(RecoverError::NoShares);
    }
    let field = Field::new();
    let mut cursors: Vec<&[u8]> = shares.iter().map(AsRef::as_ref).collect();
    let mut secret = Vec::new();
    while let Some(points) = cursors
        .iter_mut()
        .map(next_point)
        .collect::<Option<Vec<_>>>()
    {
        secret.push(interpolate_at_zero(&field, &points)?);
    }
    Ok(secret)
}
