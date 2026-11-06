// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `MUSIGMA`, `NSUMSUMSQ` and `LR` (computed with `BigDecimal` like Warp 10), the Morton codes
//! `->Z` / `Z->`, and the normal density `NPDF`.

use std::rc::Rc;

use super::Entry;
use super::args::fail;
use crate::bigdecimal::{BigDecimal, RoundingMode};
use crate::error::Result;
use crate::format::java_double_to_string;
use crate::gts::{GeoTimeSerie, Values};
use crate::macros::{Callable, StackFunction, StackObject};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("MUSIGMA", Entry::Stack(musigma)),
    ("NSUMSUMSQ", Entry::Stack(nsumsumsq)),
    ("LR", Entry::Stack(lr)),
    ("->Z", Entry::Stack(toz)),
    ("Z->", Entry::Stack(zto)),
    ("NPDF", Entry::Stack(npdf)),
];

/// The values of a numeric series as `BigDecimal`s: `valueOf(double)` for a `DOUBLE` series
/// (`NumberFormatException` for `NaN` and infinities), `valueOf(long)` for a `LONG` one.
/// `None` for a series of another type.
fn decimals(gts: &GeoTimeSerie) -> Option<Result<Vec<BigDecimal>>> {
    match &gts.values {
        Values::Double(values) => Some(values.iter().map(|&v| BigDecimal::from_f64(v)).collect()),
        Values::Long(values) => Some(Ok(values
            .iter()
            .map(|&v| BigDecimal::from_i64(v))
            .collect())),
        _ => None,
    }
}

/// `BigDecimal.valueOf(0.0D)`: `0.0`.
fn zero_point_zero() -> BigDecimal {
    BigDecimal::new(0.into(), 1)
}

/// `MUSIGMA`: the mean and (Bessel corrected with `true`) standard deviation of a numeric
/// series.
fn musigma(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MUSIGMA";
    let Value::Boolean(bessel) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a boolean on top of the stack to determine if Bessel's correction should be applied or not.",
        ));
    };
    let Value::Gts(gts) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a Geo Time Series instance below the top of the stack.",
        ));
    };
    let gts = gts.borrow();
    let n = gts.size();
    if n == 0 {
        return Err(fail(
            NAME,
            "can only compute mu and sigma for non empty series.",
        ));
    }
    let Some(values) = decimals(&gts) else {
        return Err(fail(
            NAME,
            "can only compute mu and sigma for numerical series.",
        ));
    };
    let values = values?;
    let (mut sum, mut sumsq) = (zero_point_zero(), zero_point_zero());
    for (i, bd) in values.iter().enumerate() {
        // The sum adds `valueOf(doubleValue())`, even for a `LONG` series.
        let as_double = gts.value_at(i).as_f64().unwrap_or_default();
        sum = sum.add(&BigDecimal::from_finite_f64(as_double));
        sumsq = sumsq.add(&bd.times(bd));
    }
    let bdn = BigDecimal::from_i64(n as i64);
    stack.push(Value::Double(
        sum.divide_nonzero(&bdn, RoundingMode::HalfUp)
            .double_value(),
    ));
    let mut variance = sumsq
        .divide_nonzero(&bdn, RoundingMode::HalfUp)
        .subtract(
            &sum.times(&sum)
                .divide_nonzero(&bdn.times(&bdn), RoundingMode::HalfUp),
        )
        .double_value();
    if bessel && n > 1 {
        variance *= n as f64 / (n as f64 - 1.0);
    }
    stack.push(Value::Double(variance.sqrt()));
    Ok(())
}

/// `NSUMSUMSQ`: the number of values, their sum and the sum of their squares.
fn nsumsumsq(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "NSUMSUMSQ";
    let Value::Gts(gts) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a Geo Time Series instance on top of the stack.",
        ));
    };
    let gts = gts.borrow();
    let Some(values) = decimals(&gts) else {
        return Err(fail(
            NAME,
            "can only compute mu and sigma for numerical series.",
        ));
    };
    let (mut sum, mut sumsq) = (zero_point_zero(), zero_point_zero());
    for bd in &values? {
        sum = sum.add(bd);
        sumsq = sumsq.add(&bd.times(bd));
    }
    stack.push(Value::Long(gts.size() as i64));
    stack.push(Value::Double(sum.double_value()));
    stack.push(Value::Double(sumsq.double_value()));
    Ok(())
}

/// `LR`: the intercept and slope of the least squares line of the values by tick.
fn lr(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "LR";
    let Value::Gts(gts) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a Geo Time Series instance on top of the stack.",
        ));
    };
    let gts = gts.borrow();
    let Some(values) = decimals(&gts) else {
        return Err(fail(
            NAME,
            "can only compute simple linear regression parameters for numerical series.",
        ));
    };
    // `new BigDecimal(0.0D, MathContext.UNLIMITED).setScale(16)`.
    let zero = BigDecimal::new(0.into(), 16);
    let (mut sumx, mut sumy, mut sumxx, mut sumxy) =
        (zero.clone(), zero.clone(), zero.clone(), zero);
    for (i, bd) in values?.iter().enumerate() {
        let tick = BigDecimal::from_i64(gts.tick_at(i));
        sumx = sumx.add(&tick);
        sumy = sumy.add(bd);
        sumxx = sumxx.add(&tick.times(&tick));
        sumxy = sumxy.add(&bd.times(&tick));
    }
    // `BigDecimal.valueOf((double) n)`.
    let n = BigDecimal::from_finite_f64(gts.size() as f64);
    let half_up = RoundingMode::HalfUp;
    // A numeric series has values: `n` is not zero.
    let xybar = sumxy.divide_nonzero(&n, half_up);
    let xbar = sumx.divide_nonzero(&n, half_up);
    let ybar = sumy.divide_nonzero(&n, half_up);
    let xxbar = sumxx.divide_nonzero(&n, half_up);
    let beta = xybar
        .subtract(&xbar.times(&ybar))
        .divide_rounded(&xxbar.subtract(&xbar.times(&xbar)), half_up)?;
    let alpha = ybar.subtract(&beta.times(&xbar));
    stack.push(Value::Double(alpha.double_value()));
    stack.push(Value::Double(beta.double_value()));
    Ok(())
}

/// The bit reversal of a byte of Warp 10's Morton codes.
fn reverse_byte(value: i64) -> i64 {
    (value.wrapping_mul(0x02_0202_0202) & 0x0108_8442_2010) % 1023
}

/// Pop the bit width of `->Z` / `Z->` (0 to 63).
fn pop_bit_width(stack: &mut Stack, name: &str) -> Result<i32> {
    let Value::Long(width) = stack.pop()? else {
        return Err(fail(name, "expects a bit width on top of the stack."));
    };
    // `intValue()`.
    let width = width as i32;
    if !(0..=63).contains(&width) {
        return Err(fail(name, "expects a bit width <= 63."));
    }
    Ok(width)
}

/// `->Z`: the Morton code (bit interleaving) of non negative `LONG`s on `width` bits each.
fn toz(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "->Z";
    const POSITIVE: &str = "operates on a list of positive LONGs.";
    let width = pop_bit_width(stack, NAME)?;
    let Value::List(list) = stack.pop()? else {
        return Err(fail(NAME, POSITIVE));
    };
    let mut longs = Vec::new();
    for value in list.borrow().iter() {
        match value {
            Value::Long(l) if *l >= 0 => longs.push(*l),
            _ => return Err(fail(NAME, POSITIVE)),
        }
    }
    let nbits = width as usize * longs.len();
    let nbytes = nbits.div_ceil(8);
    let mut bitcount = nbytes * 8 - nbits;
    let mut encoded = vec![0u8; nbytes];
    let mut value = 0i64;
    let mut byteidx = nbytes;
    for _ in 0..width {
        for long in longs.iter_mut().rev() {
            value = (value << 1) | (*long & 1);
            *long = ((*long as u64) >> 1) as i64;
            bitcount += 1;
            if bitcount == 8 {
                byteidx -= 1;
                encoded[byteidx] = reverse_byte(value) as u8;
                bitcount = 0;
                value = 0;
            }
        }
    }
    stack.push(Value::bytes(encoded));
    Ok(())
}

/// `Z->`: the `LONG`s of a Morton code, given their number and bit width.
fn zto(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "Z->";
    let width = pop_bit_width(stack, NAME)?;
    let Value::Long(count) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a number of components below the bit width.",
        ));
    };
    let Value::Bytes(encoded) = stack.pop()? else {
        return Err(fail(
            NAME,
            "operates on a Morton code encoded as a byte array.",
        ));
    };
    // `new long[(int) count]`.
    let count = count as i32;
    let Ok(count) = usize::try_from(count) else {
        return Err(crate::error::Error::runtime(
            "java.lang.NegativeArraySizeException",
            count.to_string(),
        ));
    };
    let mut longs = vec![0i64; count];
    let mut byteidx = 0;
    let mut bitcount = 0;
    let mut value = 0i64;
    for _ in 0..width {
        for long in &mut longs {
            if bitcount == 0 {
                let Some(&byte) = encoded.get(byteidx) else {
                    return Err(crate::error::Error::index_out_of_bounds(
                        byteidx as i64,
                        encoded.len(),
                    ));
                };
                byteidx += 1;
                bitcount = 8;
                value = reverse_byte(i64::from(byte));
            }
            *long = (*long << 1) | (value & 1);
            value = ((value as u64) >> 1) as i64;
            bitcount -= 1;
        }
    }
    stack.push(Value::list(longs.into_iter().map(Value::Long).collect()));
    Ok(())
}

/// The function `NPDF` builds: the density of the normal distribution `N(mu, sigma²)`.
#[derive(Debug)]
struct Npdf {
    mu: f64,
    sigma: f64,
}

impl StackObject for Npdf {
    fn name(&self) -> &'static str {
        "NPDF"
    }

    fn to_snapshot(&self) -> String {
        format!(
            "{} {} NPDF",
            java_double_to_string(self.mu),
            java_double_to_string(self.sigma)
        )
    }

    fn apply(&self, stack: &mut Stack) -> Result<()> {
        let x = stack.pop()?;
        let Some(x) = x.as_f64() else {
            return Err(fail("NPDF", "expects a numeric value on top of the stack."));
        };
        let two_pi_sqrt = 1.0 / (std::f64::consts::PI * 2.0).sqrt();
        let sigma = self.sigma;
        let delta = x - self.mu;
        let p = (two_pi_sqrt / sigma) * ((-1.0 / (2.0 * sigma * sigma)) * (delta * delta)).exp();
        stack.push(Value::Double(p));
        Ok(())
    }
}

/// `NPDF`: the normal density function of a mean and a (positive) standard deviation.
fn npdf(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "NPDF";
    // `0 >= sigma` lets `NaN` through.
    let sigma = stack.pop()?.as_f64().filter(|s| {
        !matches!(
            s.partial_cmp(&0.0),
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        )
    });
    let Some(sigma) = sigma else {
        return Err(fail(
            NAME,
            "expects a standard deviation (sigma) on top of the stack.",
        ));
    };
    let Some(mu) = stack.pop()?.as_f64() else {
        return Err(fail(
            NAME,
            "expects a mean (mu) below the standard deviation.",
        ));
    };
    stack.push(StackFunction::value(Callable::Object(Rc::new(Npdf {
        mu,
        sigma,
    }))));
    Ok(())
}
