// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The mappers configured with constants (`3 mapper.add`, `10 mapper.log`,
//! `0 1 mapper.npdf`, `'z' mapper.replace`, `'fr' mapper.parsedouble`…), applied by `MAP` to
//! a window of one datapoint.

use super::single::{empty, single_value};
use super::{Aggregated, Aggregator, AggregatorArgs, Named, Num, Roles};
use crate::error::aggregator::AggregatorError;
use crate::error::{Error, Result};
use crate::format::java_double_to_string;
use crate::functions::Entry;
use crate::functions::stringformat::Locale;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::jmath;
use crate::snapshot::value_snapshot;
use crate::stack::Stack;
use crate::value::{Value, double_to_long};

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "mapper.add",
        Entry::Builder(|s, n| operand_builder(s, n, Op::Add)),
    ),
    (
        "mapper.mul",
        Entry::Builder(|s, n| operand_builder(s, n, Op::Mul)),
    ),
    (
        "mapper.mod",
        Entry::Builder(|s, n| operand_builder(s, n, Op::Mod)),
    ),
    (
        "mapper.min.x",
        Entry::Builder(|s, n| operand_builder(s, n, Op::MinX)),
    ),
    (
        "mapper.max.x",
        Entry::Builder(|s, n| operand_builder(s, n, Op::MaxX)),
    ),
    (
        "mapper.pow",
        Entry::Builder(|s, n| operand_builder(s, n, Op::Pow)),
    ),
    (
        "mapper.sqrt",
        Entry::Object(|n| {
            Named::object(
                n,
                Arithmetic {
                    name: n,
                    op: Op::Pow,
                    operand: Num::Double(0.5),
                },
            )
        }),
    ),
    (
        "mapper.exp",
        Entry::Builder(|s, n| operand_builder(s, n, Op::Exp)),
    ),
    (
        "mapper.log",
        Entry::Builder(|s, n| operand_builder(s, n, Op::Log)),
    ),
    ("mapper.replace", Entry::Builder(replace_builder)),
    ("mapper.npdf", Entry::Builder(npdf_builder)),
    ("mapper.parsedouble", Entry::Builder(parsedouble_builder)),
];

/// The operation of an [`Arithmetic`] mapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// `MapperAdd`: `operand + value`, a `LONG` operand truncating a `DOUBLE` value.
    Add,
    /// `MapperMul`: `operand * value`, a `LONG` operand truncating a `DOUBLE` value.
    Mul,
    /// `MapperMod`: `value % operand`.
    Mod,
    /// `MapperMinX`: `Math.min(operand, value)`.
    MinX,
    /// `MapperMaxX`: `Math.max(operand, value)`.
    MaxX,
    /// `MapperPow`: `Math.pow(value, operand)` (`mapper.sqrt` with `0.5`).
    Pow,
    /// `MapperExp`: `Math.pow(operand, value)`.
    Exp,
    /// `MapperLog`: `Math.log(value) / operand`, the operand being the logarithm of the base.
    Log,
}

/// A mapper combining the value of the datapoint with a constant operand.
pub struct Arithmetic {
    /// The registered name, for the error messages.
    pub name: &'static str,
    /// The operation.
    pub op: Op,
    /// The operand as the Java object stores it: a `double` for `Pow`, `Exp` and `Log` (the
    /// natural logarithm of the base given to the builder).
    pub(crate) operand: Num,
}

/// `((Number) value).longValue()`.
fn long_of(value: Num) -> i64 {
    match value {
        Num::Long(l) => l,
        Num::Double(d) => double_to_long(d),
    }
}

impl Arithmetic {
    /// The new value of a numeric datapoint value.
    fn compute(&self, value: Num) -> Result<Value> {
        Ok(match (self.op, self.operand) {
            (Op::Add, Num::Long(l)) => Value::Long(l.wrapping_add(long_of(value))),
            (Op::Mul, Num::Long(l)) => Value::Long(l.wrapping_mul(long_of(value))),
            (Op::Add, Num::Double(d)) => Value::Double(d + value.as_f64()),
            (Op::Mul, Num::Double(d)) => Value::Double(d * value.as_f64()),
            (Op::Mod, Num::Long(l)) => match value {
                Num::Long(_) if l == 0 => {
                    return Err(Error::division_by_zero());
                }
                Num::Long(v) => Value::Long(v.wrapping_rem(l)),
                Num::Double(v) => Value::Double(v % l as f64),
            },
            (Op::Mod, Num::Double(d)) => Value::Double(value.as_f64() % d),
            (Op::MinX, Num::Long(l)) if matches!(value, Num::Long(_)) => {
                Value::Long(l.min(long_of(value)))
            }
            (Op::MaxX, Num::Long(l)) if matches!(value, Num::Long(_)) => {
                Value::Long(l.max(long_of(value)))
            }
            (Op::MinX, operand) => Value::Double(jmath::min(operand.as_f64(), value.as_f64())),
            (Op::MaxX, operand) => Value::Double(jmath::max(operand.as_f64(), value.as_f64())),
            (Op::Pow, operand) => Value::Double(jmath::pow(value.as_f64(), operand.as_f64())),
            (Op::Exp, operand) => Value::Double(jmath::pow(operand.as_f64(), value.as_f64())),
            (Op::Log, operand) => Value::Double(value.as_f64().ln() / operand.as_f64()),
        })
    }
}

/// The error of the mappers of this module given a non numeric value.
fn not_numeric(name: &str) -> Error {
    Error::usage(name, "can only be applied to LONG or DOUBLE values.")
}

impl Aggregator for Arithmetic {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let Some(value) = single_value(args, self.name)? else {
            return Ok(empty());
        };
        let value = Num::of(value).ok_or_else(|| not_numeric(self.name))?;
        Ok(Aggregated {
            tick: args.tick,
            location: args.locations[0],
            elevation: args.elevations[0],
            value: self.compute(value)?,
        })
    }

    fn parameters(&self) -> Option<String> {
        Some(match self.operand {
            Num::Long(l) => l.to_string(),
            Num::Double(d) => java_double_to_string(d),
        })
    }
}

/// The builders of the [`Arithmetic`] mappers: `operand name` pushes the mapper, the operand
/// being a `LONG` or a `DOUBLE`.
fn operand_builder(stack: &mut Stack, name: &'static str, op: Op) -> Result<()> {
    let Some(operand) = Num::of(&stack.pop()?) else {
        return Err(AggregatorError::InvalidParameter(name.to_string()).into());
    };
    let operand = match op {
        Op::Pow | Op::Exp => Num::Double(operand.as_f64()),
        Op::Log => Num::Double(operand.as_f64().ln()),
        _ => operand,
    };
    stack.push(Value::Function(Named::object(
        name,
        Arithmetic { name, op, operand },
    )));
    Ok(())
}

/// `MapperReplace`: the value of every window that holds datapoints is replaced by a
/// constant (the location and elevation being those of the first datapoint).
pub struct Replace {
    /// The constant: a `LONG`, `DOUBLE`, `BOOLEAN` or `STRING`.
    pub value: Value,
}

impl Aggregator for Replace {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        // No single value check: a larger window takes the constant too.
        Ok(if args.values.is_empty() {
            Aggregated {
                tick: args.tick,
                location: NO_LOCATION,
                elevation: NO_ELEVATION,
                value: Value::Null,
            }
        } else {
            Aggregated {
                tick: args.tick,
                location: args.locations[0],
                elevation: args.elevations[0],
                value: self.value.clone(),
            }
        })
    }

    fn parameters(&self) -> Option<String> {
        Some(value_snapshot(&self.value).trim_end().to_owned())
    }
}

/// `MapperReplace.Builder`: `constant mapper.replace`.
fn replace_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let value = stack.pop()?;
    if !matches!(
        value,
        Value::Long(_) | Value::Double(_) | Value::Boolean(_) | Value::String(_)
    ) {
        return Err(AggregatorError::InvalidValueType.into());
    }
    stack.push(Value::Function(Named::object(name, Replace { value })));
    Ok(())
}

/// `MapperNPDF`: the density of the normal distribution of mean `mu` and standard deviation
/// `sigma` at the value of the datapoint.
pub struct Npdf {
    /// The registered name, for the error messages.
    pub name: &'static str,
    /// The mean.
    pub mu: f64,
    /// The standard deviation.
    pub sigma: f64,
}

impl Aggregator for Npdf {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let Some(value) = single_value(args, self.name)? else {
            return Ok(empty());
        };
        let value = Num::of(value)
            .ok_or_else(|| not_numeric(self.name))?
            .as_f64();
        let (mu, sigma) = (self.mu, self.sigma);
        let p = (1.0 / (sigma * (2.0 * std::f64::consts::PI).sqrt()))
            * (-(value - mu) * (value - mu) / (2.0 * sigma * sigma)).exp();
        Ok(Aggregated {
            tick: args.tick,
            location: args.locations[0],
            elevation: args.elevations[0],
            value: Value::Double(p),
        })
    }

    fn parameters(&self) -> Option<String> {
        Some(format!(
            "{} {}",
            java_double_to_string(self.mu),
            java_double_to_string(self.sigma)
        ))
    }
}

/// `MapperNPDF.Builder`: `mu sigma mapper.npdf`, `sigma` positive (a `NaN` passes).
fn npdf_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let Some(sigma) = Num::of(&stack.pop()?) else {
        return Err(Error::usage(
            name,
            "expects a standard deviation (sigma) on top of the stack.",
        ));
    };
    let sigma = sigma.as_f64();
    if sigma <= 0.0 {
        return Err(Error::usage(
            name,
            "expects a positive standard deviation (sigma) on top of the stack.",
        ));
    }
    let Some(mu) = Num::of(&stack.pop()?) else {
        return Err(Error::usage(
            name,
            "expects a mean (mu) below the standard deviation.",
        ));
    };
    let mu = mu.as_f64();
    stack.push(Value::Function(Named::object(
        name,
        Npdf { name, mu, sigma },
    )));
    Ok(())
}

/// `MapperParseDouble`: a `STRING` value parsed by the `NumberFormat` of a locale (no result
/// when it does not start with a number, nor for the other types).
pub struct ParseDouble {
    /// The registered name, for the error messages.
    pub name: &'static str,
    /// The separators of the locale.
    pub locale: Locale,
}

impl Aggregator for ParseDouble {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let Some(value) = single_value(args, self.name)? else {
            return Ok(empty());
        };
        let value = match value {
            Value::String(s) => parse_number(s, self.locale).map_or(Value::Null, Value::Double),
            _ => Value::Null,
        };
        Ok(Aggregated {
            tick: args.tick,
            location: args.locations[0],
            elevation: args.elevations[0],
            value,
        })
    }
}

/// `MapperParseDouble.Builder`: `'languagetag' mapper.parsedouble`. The tag is read like
/// `Locale.forLanguageTag`: `fr-FR` is French, `fr_FR` is not a well-formed tag (the root
/// locale), and the languages without data use the root symbols.
fn parsedouble_builder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let Value::String(tag) = stack.pop()? else {
        return Err(AggregatorError::LanguageTagExpected(name.to_string()).into());
    };
    let locale = if tag.contains('_') {
        Locale::ROOT
    } else {
        Locale::for_tag(&tag).unwrap_or(Locale::ROOT)
    };
    stack.push(Value::Function(Named::object(
        name,
        ParseDouble { name, locale },
    )));
    Ok(())
}

/// `DecimalFormat.parse` with the default pattern of a locale: the longest prefix of `text`
/// that is a number. `NaN`, an optional `-`, then `∞` or digits with grouping separators
/// (ignored, up to the decimal separator), a decimal separator and an exponent (`E`, an
/// optional `-` and digits; one that does not fit a `long` is ignored, one that does is
/// truncated to an `int`). `None` when no digit starts the text.
pub(crate) fn parse_number(text: &str, locale: Locale) -> Option<f64> {
    if text.starts_with("NaN") {
        return Some(f64::NAN);
    }
    let (sign, rest) = match text.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, text),
    };
    if rest.starts_with('\u{221E}') {
        return Some(sign * f64::INFINITY);
    }
    let mut digits = String::new();
    let mut decimal_at = None;
    let mut exponent = 0i64;
    for (i, c) in rest.char_indices() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else if c == locale.decimal && decimal_at.is_none() {
            decimal_at = Some(digits.len());
        } else if c == locale.grouping && decimal_at.is_none() {
            // Grouping separators are ignored in the integer part.
        } else {
            if c == 'E' {
                exponent = parse_exponent(&rest[i + 1..]);
            }
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }
    let fraction = (digits.len() - decimal_at.unwrap_or(digits.len())) as i64;
    // Rust parses the digits and any exponent with correct rounding, like
    // `Double.parseDouble` on the digit list.
    let magnitude: f64 = format!("{digits}e{}", exponent - fraction)
        .parse()
        .unwrap_or_default();
    Some(sign * magnitude)
}

/// The exponent after the `E` of a [`parse_number`] text: `0` when there is none or it does
/// not fit a `long`, otherwise its value truncated to an `int` (`(int) exponentDigits.getLong()`).
fn parse_exponent(text: &str) -> i64 {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, text),
    };
    let mut magnitude = 0u64;
    for c in digits.chars().take_while(char::is_ascii_digit) {
        let Some(next) = magnitude
            .checked_mul(10)
            .and_then(|m| m.checked_add(u64::from(c) - u64::from('0')))
        else {
            return 0;
        };
        magnitude = next;
    }
    let value = if negative {
        0i64.checked_sub_unsigned(magnitude)
    } else {
        i64::try_from(magnitude).ok()
    };
    value.map_or(0, |v| i64::from(v as i32))
}
