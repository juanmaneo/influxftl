// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `BIGDECIMAL` values: `->BD`, `BD->` and the `BD*` functions (Java's `BigDecimal` and the
//! `BigInteger` operations on integral values).

use std::cmp::Ordering;
use std::rc::Rc;

use num_bigint::BigInt;
use num_integer::Integer;

use super::Entry;
use super::args::fail;
use super::math::{ATTRIBUTE_SEEDED_PRNG, entropy_error};
use crate::bigdecimal::{
    BigDecimal, RoundingMode, arithmetic, bit_count, bit_length, change_bit, from_byte_array,
    is_probable_prime, lowest_set_bit, mod_inverse, mod_pow, modulo, next_probable_prime,
    parse_bigint, shift_left, test_bit, to_byte_array,
};
use crate::error::Result;
use crate::jrandom::JavaRandom;
use crate::stack::Stack;
use crate::value::Value;

/// A `BIGDECIMAL` value.
pub fn bd_value(bd: BigDecimal) -> Value {
    Value::BigDecimal(Rc::new(bd))
}

/// `TOBD.toBigDecimal`: the `BigDecimal` of a `BIGDECIMAL`, a string (decimal, or `0x` /
/// `0b` integers), bytes (a two's complement integer), a `LONG` or a `DOUBLE`.
pub fn to_big_decimal(name: &str, value: &Value) -> Result<BigDecimal> {
    Ok(match value {
        Value::BigDecimal(bd) => (**bd).clone(),
        Value::String(s) => {
            let integer = |digits: &str, radix: u32, negative: bool| -> Result<BigDecimal> {
                let value = parse_bigint(digits, radix)?;
                Ok(BigDecimal::from_bigint(if negative {
                    -value
                } else {
                    value
                }))
            };
            if let Some(hex) = s.strip_prefix("0x") {
                integer(hex, 16, false)?
            } else if let Some(hex) = s.strip_prefix("-0x") {
                integer(hex, 16, true)?
            } else if let Some(bin) = s.strip_prefix("0b") {
                integer(bin, 2, false)?
            } else if let Some(bin) = s.strip_prefix("-0b") {
                integer(bin, 2, true)?
            } else {
                BigDecimal::parse(s)?
            }
        }
        Value::Long(l) => BigDecimal::from_i64(*l),
        Value::Double(d) => BigDecimal::from_f64(*d)?,
        Value::Bytes(b) => BigDecimal::from_bigint(from_byte_array(b)?),
        _ => {
            return Err(fail(
                name,
                "can only be applied to a BIGDECIMAL, STRING, BYTES, LONG or DOUBLE argument.",
            ));
        }
    })
}

/// `TOBD.toBigInteger`: the integer of a value without fractional part.
pub fn to_big_integer(name: &str, value: &Value) -> Result<BigInt> {
    to_big_decimal(name, value)?
        .to_bigint_exact()
        .ok_or_else(|| {
            fail(
                name,
                "can only be applied to a BIGDECIMAL with no fractional part.",
            )
        })
}

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("->BD", Entry::Stack(tobd)),
    ("BD->", Entry::Stack(bdto)),
    (
        "BDMUL",
        Entry::Stack(|s| binary(s, "BDMUL", |a, b| a.multiply(&b))),
    ),
    (
        "BDADD",
        Entry::Stack(|s| binary(s, "BDADD", |a, b| Ok(a.add(&b)))),
    ),
    (
        "BDSUB",
        Entry::Stack(|s| binary(s, "BDSUB", |a, b| Ok(b.subtract(&a)))),
    ),
    (
        "BDDIV",
        Entry::Stack(|s| binary(s, "BDDIV", |a, b| b.divide(&a))),
    ),
    (
        "BDDIVINT",
        Entry::Stack(|s| binary(s, "BDDIVINT", |a, b| b.divide_to_integral_value(&a))),
    ),
    (
        "BDREM",
        Entry::Stack(|s| binary(s, "BDREM", |a, b| b.remainder(&a))),
    ),
    (
        "BDMAX",
        Entry::Stack(|s| {
            binary(s, "BDMAX", |a, b| {
                Ok(if a.compare(&b) == Ordering::Less {
                    b
                } else {
                    a
                })
            })
        }),
    ),
    (
        "BDMIN",
        Entry::Stack(|s| {
            binary(s, "BDMIN", |a, b| {
                Ok(if a.compare(&b) == Ordering::Greater {
                    b
                } else {
                    a
                })
            })
        }),
    ),
    ("BDCOMP", Entry::Stack(bdcomp)),
    ("BDDIVIDEANDREMAINDER", Entry::Stack(bddivideandremainder)),
    (
        "BDNEG",
        Entry::Stack(|s| unary(s, "BDNEG", BigDecimal::negate)),
    ),
    (
        "BDABS",
        Entry::Stack(|s| unary(s, "BDABS", BigDecimal::abs)),
    ),
    (
        "BDULP",
        Entry::Stack(|s| unary(s, "BDULP", BigDecimal::ulp)),
    ),
    (
        "BDSTRIP",
        Entry::Stack(|s| unary(s, "BDSTRIP", BigDecimal::strip_trailing_zeros)),
    ),
    (
        "BDFLOOR",
        Entry::Stack(|s| unary(s, "BDFLOOR", |a| a.round(0, RoundingMode::Floor))),
    ),
    (
        "BDCEIL",
        Entry::Stack(|s| unary(s, "BDCEIL", |a| a.round(0, RoundingMode::Ceiling))),
    ),
    ("BDROUND", Entry::Stack(|s| unary(s, "BDROUND", bdround))),
    (
        "BDCLEAR",
        Entry::Stack(|s| unary(s, "BDCLEAR", |a| BigDecimal::from_bigint(a.to_bigint()))),
    ),
    ("BDSIGNUM", Entry::Stack(bdsignum)),
    ("BDPOW", Entry::Stack(bdpow)),
    ("BDSCALE", Entry::Stack(bdscale)),
    (
        "BDAND",
        Entry::Stack(|s| integers(s, "BDAND", |a, b| Ok(a & b))),
    ),
    (
        "BDOR",
        Entry::Stack(|s| integers(s, "BDOR", |a, b| Ok(a | b))),
    ),
    (
        "BDXOR",
        Entry::Stack(|s| integers(s, "BDXOR", |a, b| Ok(a ^ b))),
    ),
    (
        "BDANDNOT",
        Entry::Stack(|s| integers(s, "BDANDNOT", |a, b| Ok(a & !b))),
    ),
    (
        "BDGCD",
        Entry::Stack(|s| integers(s, "BDGCD", |a, b| Ok(a.gcd(&b)))),
    ),
    (
        "BDMOD",
        Entry::Stack(|s| integers(s, "BDMOD", |a, b| modulo(&a, &b))),
    ),
    (
        "BDMODINVERSE",
        Entry::Stack(|s| integers(s, "BDMODINVERSE", |a, b| mod_inverse(&a, &b))),
    ),
    ("BDMODPOW", Entry::Stack(bdmodpow)),
    ("BDNOT", Entry::Stack(|s| integer(s, "BDNOT", |a| Ok(!a)))),
    (
        "BDNEXTPROBABLEPRIME",
        Entry::Stack(|s| integer(s, "BDNEXTPROBABLEPRIME", |a| next_probable_prime(&a))),
    ),
    (
        "BDBITLENGTH",
        Entry::Stack(|s| integer_long(s, "BDBITLENGTH", |a| bit_length(&a) as i64)),
    ),
    (
        "BDBITCOUNT",
        Entry::Stack(|s| integer_long(s, "BDBITCOUNT", |a| bit_count(&a) as i64)),
    ),
    (
        "BDLOWESTSETBIT",
        Entry::Stack(|s| integer_long(s, "BDLOWESTSETBIT", |a| lowest_set_bit(&a))),
    ),
    (
        "BDSETBIT",
        Entry::Stack(|s| bit(s, "BDSETBIT", |a, n| change_bit(&a, n, |x, m| x | m))),
    ),
    (
        "BDCLEARBIT",
        Entry::Stack(|s| bit(s, "BDCLEARBIT", |a, n| change_bit(&a, n, |x, m| x & !m))),
    ),
    (
        "BDFLIPBIT",
        Entry::Stack(|s| bit(s, "BDFLIPBIT", |a, n| change_bit(&a, n, |x, m| x ^ m))),
    ),
    (
        "BDSHIFTLEFT",
        Entry::Stack(|s| bit(s, "BDSHIFTLEFT", |a, n| Ok(shift_left(&a, n)))),
    ),
    (
        "BDSHIFTRIGHT",
        Entry::Stack(|s| {
            bit(s, "BDSHIFTRIGHT", |a, n| {
                Ok(shift_left(&a, n.wrapping_neg()))
            })
        }),
    ),
    ("BDTESTBIT", Entry::Stack(bdtestbit)),
    ("BDISPROBABLEPRIME", Entry::Stack(bdisprobableprime)),
    ("BDRAND", Entry::Stack(|s| bdrand(s, "BDRAND", false))),
    ("SBDRAND", Entry::Stack(|s| bdrand(s, "SBDRAND", true))),
    (
        "BDPROBABLEPRIME",
        Entry::Stack(|s| bdprobableprime(s, "BDPROBABLEPRIME", false)),
    ),
    (
        "SBDPROBABLEPRIME",
        Entry::Stack(|s| bdprobableprime(s, "SBDPROBABLEPRIME", true)),
    ),
];

/// `->BD`.
fn tobd(stack: &mut Stack) -> Result<()> {
    let value = stack.pop()?;
    stack.push(bd_value(to_big_decimal("->BD", &value)?));
    Ok(())
}

/// `BD->`: the plain string of a `BIGDECIMAL`, or (with `true`) the bytes of an integral one.
fn bdto(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "BD->";
    let mut top = stack.pop()?;
    let mut to_bytes = false;
    if let Value::Boolean(b) = top {
        to_bytes = b;
        top = stack.pop()?;
    }
    let Value::BigDecimal(bd) = top else {
        return Err(fail(NAME, "operates on a BIGDECIMAL."));
    };
    if to_bytes {
        let Some(integer) = bd.to_bigint_exact() else {
            return Err(fail(
                NAME,
                "cannot convert a BIGDECIMAL to BYTES if its scale is not 0.",
            ));
        };
        stack.push(Value::bytes(to_byte_array(&integer)));
    } else {
        stack.push(Value::from(bd.to_plain_string()));
    }
    Ok(())
}

/// A function of the top value `a` and the one below `b`.
fn binary(
    stack: &mut Stack,
    name: &str,
    op: fn(BigDecimal, BigDecimal) -> Result<BigDecimal>,
) -> Result<()> {
    let a = to_big_decimal(name, &stack.pop()?)?;
    let b = to_big_decimal(name, &stack.pop()?)?;
    stack.push(bd_value(op(a, b)?));
    Ok(())
}

/// A function of the top value.
fn unary(stack: &mut Stack, name: &str, op: fn(&BigDecimal) -> BigDecimal) -> Result<()> {
    let a = to_big_decimal(name, &stack.pop()?)?;
    stack.push(bd_value(op(&a)));
    Ok(())
}

/// `BDCOMP`: `below.compareTo(top)`.
fn bdcomp(stack: &mut Stack) -> Result<()> {
    let a = to_big_decimal("BDCOMP", &stack.pop()?)?;
    let b = to_big_decimal("BDCOMP", &stack.pop()?)?;
    stack.push(Value::Long(b.compare(&a) as i64));
    Ok(())
}

/// `BDDIVIDEANDREMAINDER`: the integral quotient and the remainder.
fn bddivideandremainder(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "BDDIVIDEANDREMAINDER";
    let divisor = to_big_decimal(NAME, &stack.pop()?)?;
    let dividend = to_big_decimal(NAME, &stack.pop()?)?;
    let (quotient, remainder) = dividend.divide_and_remainder(&divisor)?;
    stack.push(bd_value(quotient));
    stack.push(bd_value(remainder));
    Ok(())
}

/// `BDROUND`: half up, but half down (towards zero) for negative values whose fraction is
/// `-0.5` with scale 1 (Java compares with `equals`).
fn bdround(a: &BigDecimal) -> BigDecimal {
    let integer = BigDecimal::from_bigint(a.to_bigint());
    let half = BigDecimal::new(BigInt::from(-5), 1);
    if a.signum() < 0 && a.subtract(&integer) == half {
        integer
    } else {
        a.round(0, RoundingMode::HalfUp)
    }
}

/// `BDSIGNUM`.
fn bdsignum(stack: &mut Stack) -> Result<()> {
    let a = to_big_decimal("BDSIGNUM", &stack.pop()?)?;
    stack.push(Value::Long(i64::from(a.signum())));
    Ok(())
}

/// `BDPOW`: a `LONG` or `BIGDECIMAL` exponent (an `int`).
fn bdpow(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "BDPOW";
    let exponent = match stack.pop()? {
        Value::BigDecimal(bd) => bd.int_value_exact()?,
        Value::Long(l) => i32::try_from(l).map_err(|_| arithmetic("integer overflow"))?,
        _ => return Err(fail(NAME, "only supports LONG or BIGDECIMAL exponent.")),
    };
    let base = to_big_decimal(NAME, &stack.pop()?)?;
    stack.push(bd_value(base.pow(exponent)?));
    Ok(())
}

/// `BDSCALE`: the scale of a `BIGDECIMAL`, or a value set to the `LONG` scale on top.
fn bdscale(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "BDSCALE";
    let scale = match stack.pop()? {
        Value::BigDecimal(bd) => {
            stack.push(Value::Long(i64::from(bd.scale)));
            return Ok(());
        }
        Value::Long(l) => i32::try_from(l).map_err(|_| arithmetic("integer overflow"))?,
        _ => return Err(fail(NAME, "only supports LONG scale.")),
    };
    let value = to_big_decimal(NAME, &stack.pop()?)?;
    stack.push(bd_value(value.set_scale(scale)?));
    Ok(())
}

/// A function of the integers below (`a`) and on top (`b`).
fn integers(stack: &mut Stack, name: &str, op: fn(BigInt, BigInt) -> Result<BigInt>) -> Result<()> {
    let b = to_big_integer(name, &stack.pop()?)?;
    let a = to_big_integer(name, &stack.pop()?)?;
    stack.push(bd_value(BigDecimal::from_bigint(op(a, b)?)));
    Ok(())
}

/// A function of the integer on top.
fn integer(stack: &mut Stack, name: &str, op: fn(BigInt) -> Result<BigInt>) -> Result<()> {
    let a = to_big_integer(name, &stack.pop()?)?;
    stack.push(bd_value(BigDecimal::from_bigint(op(a)?)));
    Ok(())
}

/// A `LONG` property of the integer on top.
fn integer_long(stack: &mut Stack, name: &str, op: fn(BigInt) -> i64) -> Result<()> {
    let a = to_big_integer(name, &stack.pop()?)?;
    stack.push(Value::Long(op(a)));
    Ok(())
}

/// Pop the `LONG` bit number of `BDSETBIT` and friends (`intValue()`).
fn pop_bit(stack: &mut Stack, name: &str) -> Result<i32> {
    match stack.pop()? {
        Value::Long(n) => Ok(n as i32),
        _ => Err(fail(name, "expects a LONG.")),
    }
}

/// A bit operation of the integer below with the bit number on top.
fn bit(stack: &mut Stack, name: &str, op: fn(BigInt, i32) -> Result<BigInt>) -> Result<()> {
    let n = pop_bit(stack, name)?;
    let a = to_big_integer(name, &stack.pop()?)?;
    stack.push(bd_value(BigDecimal::from_bigint(op(a, n)?)));
    Ok(())
}

/// `BDTESTBIT`.
fn bdtestbit(stack: &mut Stack) -> Result<()> {
    let n = pop_bit(stack, "BDTESTBIT")?;
    let a = to_big_integer("BDTESTBIT", &stack.pop()?)?;
    stack.push(Value::Boolean(test_bit(&a, n)?));
    Ok(())
}

/// `BDMODPOW`: `base^exponent mod m`.
fn bdmodpow(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "BDMODPOW";
    let m = to_big_integer(NAME, &stack.pop()?)?;
    let exponent = to_big_integer(NAME, &stack.pop()?)?;
    let base = to_big_integer(NAME, &stack.pop()?)?;
    stack.push(bd_value(BigDecimal::from_bigint(mod_pow(
        &base, &exponent, &m,
    )?)));
    Ok(())
}

/// `BDISPROBABLEPRIME`: the `LONG` certainty on top.
fn bdisprobableprime(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "BDISPROBABLEPRIME";
    let Value::Long(certainty) = stack.pop()? else {
        return Err(fail(NAME, "expects the certainty to be a LONG."));
    };
    let a = to_big_integer(NAME, &stack.pop()?)?;
    stack.push(Value::Boolean(is_probable_prime(&a, certainty as i32)));
    Ok(())
}

/// The random source of `BDRAND` / `BDPROBABLEPRIME`: the seeded PRNG of the stack, or a
/// SHA-256 counter mode generator keyed with 32 bytes from the operating system (Java uses
/// a `SecureRandom`).
pub(super) enum Source {
    Seeded(JavaRandom),
    Secure { key: [u8; 32], counter: u64 },
}

impl Source {
    fn new(stack: &Stack, name: &str, seeded: bool) -> Result<Source> {
        if !seeded {
            let mut key = [0u8; 32];
            return Source::secure(getrandom::fill(&mut key).map(|()| key));
        }
        let Some(Value::Long(state)) = stack.attribute(ATTRIBUTE_SEEDED_PRNG) else {
            return Err(fail(name, "seeded PRNG was not initialized."));
        };
        Ok(Source::Seeded(JavaRandom::from_state(*state)))
    }

    /// The secure source of the operating system entropy `key`.
    pub(super) fn secure(key: core::result::Result<[u8; 32], getrandom::Error>) -> Result<Source> {
        let key = key.map_err(entropy_error)?;
        Ok(Source::Secure { key, counter: 0 })
    }

    /// `Random.nextBytes`.
    fn next_bytes(&mut self, bytes: &mut [u8]) {
        match self {
            Source::Seeded(random) => {
                for chunk in bytes.chunks_mut(4) {
                    let mut rnd = random.next_int();
                    for byte in chunk {
                        *byte = rnd as u8;
                        rnd >>= 8;
                    }
                }
            }
            Source::Secure { key, counter } => {
                use sha2::Digest;
                for chunk in bytes.chunks_mut(32) {
                    let mut hasher = sha2::Sha256::new();
                    hasher.update(*key);
                    hasher.update(counter.to_be_bytes());
                    *counter += 1;
                    let block = hasher.finalize();
                    chunk.copy_from_slice(&block[..chunk.len()]);
                }
            }
        }
    }

    /// Store the state of a seeded source back.
    fn save(&self, stack: &mut Stack) {
        if let Source::Seeded(random) = self {
            stack.set_attribute(ATTRIBUTE_SEEDED_PRNG, Some(Value::Long(random.state())));
        }
    }
}

/// `new BigInteger(numBits, rnd)`: a uniform integer in `[0, 2^numBits)`.
fn random_bits(source: &mut Source, bits: u64) -> BigInt {
    let len = bits.div_ceil(8) as usize;
    let mut bytes = vec![0u8; len];
    if len > 0 {
        source.next_bytes(&mut bytes);
        let excess = 8 * len as u64 - bits;
        bytes[0] &= ((1u32 << (8 - excess)) - 1) as u8;
    }
    BigInt::from_bytes_be(num_bigint::Sign::Plus, &bytes)
}

/// Pop the `LONG` number of bits of `BDRAND` / `BDPROBABLEPRIME`.
fn pop_bits(stack: &mut Stack, name: &str) -> Result<u64> {
    let Value::Long(bits) = stack.pop()? else {
        return Err(fail(name, "expects a LONG number of bits to generate."));
    };
    if !(0..=i64::from(i32::MAX)).contains(&bits) {
        return Err(fail(
            name,
            "invalid number of bits, MUST be between 0 and 2147483647.",
        ));
    }
    Ok(bits as u64)
}

/// `BDRAND` / `SBDRAND`: a random integer of the given number of bits.
fn bdrand(stack: &mut Stack, name: &str, seeded: bool) -> Result<()> {
    let mut source = Source::new(stack, name, seeded)?;
    let bits = pop_bits(stack, name)?;
    let value = random_bits(&mut source, bits);
    source.save(stack);
    stack.push(bd_value(BigDecimal::from_bigint(value)));
    Ok(())
}

/// `BDPROBABLEPRIME` / `SBDPROBABLEPRIME`: a random probable prime of exactly the given
/// number of bits. Odd candidates with their top bit set are drawn until one passes the
/// primality test (Java's sieve, and so the primes drawn from a seed, differ).
fn bdprobableprime(stack: &mut Stack, name: &str, seeded: bool) -> Result<()> {
    let mut source = Source::new(stack, name, seeded)?;
    let bits = pop_bits(stack, name)?;
    if bits < 2 {
        return Err(arithmetic("bitLength < 2"));
    }
    let top = BigInt::from(1u8) << (bits - 1) as usize;
    let prime = loop {
        let mut candidate = random_bits(&mut source, bits - 1) | &top;
        if bits > 2 {
            candidate |= BigInt::from(1u8);
        }
        if is_probable_prime(&candidate, 100) {
            break candidate;
        }
    };
    source.save(stack);
    stack.push(bd_value(BigDecimal::from_bigint(prime)));
    Ok(())
}
