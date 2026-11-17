// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The aggregators of a whole window positioned at its current tick: `mapper.product` /
//! `reducer.product` (the exact `BigDecimal` product of the values) and the
//! `mapper.dotproduct` family (the dot product of the window with a vector, optionally fed to
//! `tanh`, the logistic function or a rectifier).

use super::{Aggregated, Aggregator, AggregatorArgs, Named, Roles, double_value};
use crate::error::fault::NumberFormatError;
use crate::error::{Error, Result};
use crate::format::double_to_text;
use crate::functions::Entry;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::jmath;
use crate::position::position;
use crate::stack::Stack;
use crate::value::{Value, ValueKind};

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "mapper.product",
        Entry::Object(|n| Named::object(n, Product { name: n })),
    ),
    (
        "reducer.product",
        Entry::Object(|n| Named::object(n, Product { name: n })),
    ),
    (
        "mapper.dotproduct",
        Entry::Builder(|s, n| dot_product_builder(s, n, Activation::Identity)),
    ),
    (
        "mapper.dotproduct.tanh",
        Entry::Builder(|s, n| dot_product_builder(s, n, Activation::Tanh)),
    ),
    (
        "mapper.dotproduct.sigmoid",
        Entry::Builder(|s, n| dot_product_builder(s, n, Activation::Sigmoid)),
    ),
    (
        "mapper.dotproduct.positive",
        Entry::Builder(|s, n| dot_product_builder(s, n, Activation::Positive)),
    ),
];

/// `MAP` and `REDUCE` only.
const MAPPER_REDUCER: Roles = Roles {
    bucketizer: false,
    mapper: true,
    reducer: true,
};

/// The index of the current tick in the window (`(int) window[4]`), `default` when the
/// framework provides no window parameters; out of range (`-1` for an output tick that holds
/// no datapoint) it is the `ArrayIndexOutOfBoundsException` of the location lookup.
fn tick_index(args: &AggregatorArgs, default: Option<i64>) -> Result<usize> {
    let index = match (args.window, default) {
        (Some(window), _) => window
            .tick_index
            .ok_or_else(|| Error::index_out_of_bounds(4, 4))?,
        (None, Some(index)) => index,
        (None, None) => return Err(Error::index_out_of_bounds(7, 7)),
    };
    position(index, args.locations.len())
        .ok_or_else(|| Error::index_out_of_bounds(index, args.locations.len()))
}

/// The result of an empty window: `{ tick, NO_LOCATION, NO_ELEVATION, null }`.
fn nothing(args: &AggregatorArgs) -> Aggregated {
    Aggregated {
        tick: args.tick,
        location: NO_LOCATION,
        elevation: NO_ELEVATION,
        value: Value::Null,
    }
}

/// `MapperProduct`: the product of the non-null values, computed exactly like the Java
/// `BigDecimal` product: a `LONG` when all the values are `LONG`s (the low 64 bits of the
/// product, i.e. the wrapping product), otherwise the correctly rounded `DOUBLE` (`-0.0`
/// factors giving `0.0`). The result is at the current tick, with its location and
/// elevation.
pub struct Product {
    /// The registered name, for the error messages.
    pub name: &'static str,
}

/// An exact non-negative binary number `mantissa × 2^exponent`, the mantissa as
/// little-endian 64-bit words.
pub(super) struct Exact {
    /// The words of the mantissa.
    pub(super) words: Vec<u64>,
    /// The binary exponent.
    pub(super) exponent: i64,
}

impl Exact {
    /// Multiply by `m × 2^e`.
    pub(super) fn multiply(&mut self, m: u64, e: i64) {
        let mut carry = 0u128;
        for word in &mut self.words {
            let wide = u128::from(*word) * u128::from(m) + carry;
            *word = wide as u64;
            carry = wide >> 64;
        }
        if carry != 0 {
            self.words.push(carry as u64);
        }
        self.exponent += e;
    }

    /// The nearest `f64`: the 64 leading bits of the mantissa, the others making the sticky
    /// bit of the rounding.
    pub(super) fn to_f64(&self) -> f64 {
        let Some(top) = self.words.iter().rposition(|w| *w != 0) else {
            return 0.0;
        };
        let leading = self.words[top].leading_zeros();
        let mut m = self.words[top] << leading;
        if leading != 0 && top > 0 {
            m |= self.words[top - 1] >> (64 - leading);
        }
        // The bits below the 64 kept: the rest of word `top - 1` (`leading` is below 64) and
        // the words beneath.
        let sticky = top > 0
            && (self.words[top - 1] << leading != 0
                || self.words[..top - 1].iter().any(|w| *w != 0));
        let exponent = self.exponent + 64 * top as i64 - i64::from(leading);
        jmath::binary_to_f64(m, exponent, sticky)
    }
}

/// The mantissa and exponent of a finite `f64` magnitude (`new BigDecimal(double)` is exact).
fn decompose(d: f64) -> (u64, i64) {
    let bits = d.abs().to_bits();
    let biased = (bits >> 52) as i64;
    let fraction = bits & ((1 << 52) - 1);
    if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | 1 << 52, biased - 1075)
    }
}

impl Aggregator for Product {
    fn roles(&self) -> Roles {
        MAPPER_REDUCER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.values.is_empty() {
            return Ok(nothing(args));
        }
        let mut long_product = 1i64;
        let mut exact = Exact {
            words: vec![1],
            exponent: 0,
        };
        let mut negative = false;
        let mut is_double = false;
        for value in args.values {
            let (m, e) = match value {
                Value::Long(l) => {
                    long_product = long_product.wrapping_mul(*l);
                    negative ^= *l < 0;
                    (l.unsigned_abs(), 0)
                }
                Value::Double(d) if !d.is_finite() => {
                    return Err(NumberFormatError::NotFinite.into());
                }
                Value::Double(d) => {
                    is_double = true;
                    negative ^= d.is_sign_negative();
                    decompose(*d)
                }
                Value::Null => continue,
                _ => {
                    return Err(Error::usage(
                        self.name,
                        "can only be applied to LONG or DOUBLE values.",
                    ));
                }
            };
            exact.multiply(m, e);
        }
        let index = tick_index(args, Some(0))?;
        let value = if is_double {
            let magnitude = exact.to_f64();
            // `BigDecimal` has no negative zero.
            Value::Double(if negative && magnitude != 0.0 {
                -magnitude
            } else {
                magnitude
            })
        } else {
            Value::Long(long_product)
        };
        Ok(Aggregated {
            tick: args.tick,
            location: args.locations[index],
            elevation: args.elevations[index],
            value,
        })
    }
}

/// The function applied to the dot product of a [`DotProduct`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    /// `MapperDotProduct`: the dot product itself.
    Identity,
    /// `MapperDotProductTanh`: its hyperbolic tangent.
    Tanh,
    /// `MapperDotProductSigmoid`: its logistic function.
    Sigmoid,
    /// `MapperDotProductPositive`: `Math.max(0.0, x)`.
    Positive,
}

/// `MapperDotProduct` and its subclasses: the dot product of the values of a window holding
/// exactly as many datapoints as the vector has elements (no result otherwise), at the
/// current tick, with its location and elevation.
pub struct DotProduct {
    /// The registered name, for the error messages.
    pub name: &'static str,
    /// The vector.
    pub omega: Vec<f64>,
    /// The function applied to the dot product.
    pub activation: Activation,
}

impl Aggregator for DotProduct {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let values = args.values;
        if values.is_empty() || self.omega.len() != values.len() {
            return Ok(nothing(args));
        }
        if !values[0].is_number() {
            return Err(Error::usage(
                self.name,
                "can only be applied to LONG or DOUBLE values.",
            ));
        }
        let index = tick_index(args, None)?;
        let mut dot = 0.0;
        for (omega, value) in self.omega.iter().zip(values) {
            dot += omega * double_value(value)?;
        }
        let value = match self.activation {
            Activation::Identity => dot,
            Activation::Tanh => dot.tanh(),
            Activation::Sigmoid => 1.0 / (1.0 + (-dot).exp()),
            Activation::Positive => jmath::max(0.0, dot),
        };
        Ok(Aggregated {
            tick: args.tick,
            location: args.locations[index],
            elevation: args.elevations[index],
            value: Value::Double(value),
        })
    }

    fn parameters(&self) -> Option<String> {
        let mut out = String::from("[ ");
        for omega in &self.omega {
            out.push_str(&double_to_text(*omega));
            out.push(' ');
        }
        out.push(']');
        Some(out)
    }
}

/// `MapperDotProduct.Builder`: `[ doubles ] name` (each element cast to a `Double`).
fn dot_product_builder(
    stack: &mut Stack,
    name: &'static str,
    activation: Activation,
) -> Result<()> {
    let Value::List(list) = stack.pop()? else {
        return Err(Error::usage(
            name,
            "expects an input vector (list of doubles) on top of the stack.",
        ));
    };
    let omega = list
        .borrow()
        .iter()
        .map(|element| match element {
            Value::Double(d) => Ok(*d),
            Value::Null => Err(Error::null_pointer()),
            other => Err(Error::class_cast(other.kind(), ValueKind::Double)),
        })
        .collect::<Result<Vec<f64>>>()?;
    stack.push(Value::Function(Named::object(
        name,
        DotProduct {
            name,
            omega,
            activation,
        },
    )));
    Ok(())
}
