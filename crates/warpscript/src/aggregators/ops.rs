// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The operations of `APPLY` (`io.warp10.script.op`, `WarpScriptNAryFunction`): `op.add`,
//! `op.sub`, `op.mul`, `op.div`, `op.mask` / `op.negmask`, the comparisons `op.eq` …
//! `op.ge` and the boolean `op.and` / `op.or`, applied tick by tick to the values of the
//! series of an equivalence class (`null` for the series without a datapoint at that tick).

use std::cmp::Ordering;

use super::{Aggregated, Aggregator, AggregatorArgs, Named, Num, Roles};
use crate::error::{Error, Result};
use crate::functions::Entry;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::value::Value;

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("op.add", Entry::Object(|n| op(n, Kind::Add, true))),
    (
        "op.add.ignore-nulls",
        Entry::Object(|n| op(n, Kind::Add, false)),
    ),
    ("op.sub", Entry::Object(|n| op(n, Kind::Sub, true))),
    ("op.mul", Entry::Object(|n| op(n, Kind::Mul, true))),
    (
        "op.mul.ignore-nulls",
        Entry::Object(|n| op(n, Kind::Mul, false)),
    ),
    ("op.div", Entry::Object(|n| op(n, Kind::Div, true))),
    ("op.mask", Entry::Object(|n| op(n, Kind::Mask, false))),
    ("op.negmask", Entry::Object(|n| op(n, Kind::Mask, true))),
    ("op.ne", Entry::Object(|n| op(n, Kind::Ne, true))),
    ("op.eq", Entry::Object(|n| op(n, Kind::Eq, true))),
    ("op.lt", Entry::Object(|n| op(n, Kind::Lt, true))),
    ("op.gt", Entry::Object(|n| op(n, Kind::Gt, true))),
    ("op.le", Entry::Object(|n| op(n, Kind::Le, true))),
    ("op.ge", Entry::Object(|n| op(n, Kind::Ge, true))),
    (
        "op.and.ignore-nulls",
        Entry::Object(|n| op(n, Kind::And, false)),
    ),
    ("op.and", Entry::Object(|n| op(n, Kind::And, true))),
    (
        "op.or.ignore-nulls",
        Entry::Object(|n| op(n, Kind::Or, false)),
    ),
    ("op.or", Entry::Object(|n| op(n, Kind::Or, true))),
];

/// An operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `OpAdd`: the sum of the values (string concatenation from a first string).
    Add,
    /// `OpSub`: the difference of exactly two values.
    Sub,
    /// `OpMul`: the product of the values (string concatenation from a first string).
    Mul,
    /// `OpDiv`: the quotient of exactly two values, as a `DOUBLE`.
    Div,
    /// `OpMask`: the value of the second series where the first is `true` (`false` when
    /// negated).
    Mask,
    /// `OpNE`: whether the values are all different.
    Ne,
    /// `OpEQ`: whether the values are all equal.
    Eq,
    /// `OpLT`: whether the values are strictly increasing.
    Lt,
    /// `OpGT`: whether the values are strictly decreasing.
    Gt,
    /// `OpLE`: whether the values are increasing.
    Le,
    /// `OpGE`: whether the values are decreasing.
    Ge,
    /// `OpBoolean` `and`.
    And,
    /// `OpBoolean` `or`.
    Or,
}

/// An operation of `APPLY`.
pub struct Op {
    /// The registered name, for the error messages.
    pub name: &'static str,
    /// The operation.
    pub kind: Kind,
    /// `Add`, `Mul`, `And`, `Or`: whether a `null` value makes the result `null` (otherwise
    /// nulls are ignored); `Mask`: whether the mask is negated.
    pub flag: bool,
}

/// The operation `kind` registered as `name`.
fn op(name: &'static str, kind: Kind, flag: bool) -> std::rc::Rc<dyn crate::value::FunctionObject> {
    Named::object(name, Op { name, kind, flag })
}

/// `Boolean.TRUE.equals(value) ? 1L : 0L`.
fn bit(b: bool) -> i64 {
    i64::from(b)
}

/// One step of `OpAdd` / `OpMul`: the accumulated value combined with the next one.
fn combine(acc: Value, value: &Value, multiply: bool) -> Result<Value> {
    let verb = if multiply { "multiply" } else { "add" };
    let long = |a: i64, b: i64| {
        if multiply {
            a.wrapping_mul(b)
        } else {
            a.wrapping_add(b)
        }
    };
    let double = |a: f64, b: f64| if multiply { a * b } else { a + b };
    Ok(match (acc, value) {
        (Value::String(s), value) => Value::string(format!("{s}{value}")),
        (acc, Value::String(_)) => {
            let kind = match acc {
                Value::Long(_) => "longs",
                Value::Double(_) => "doubles",
                _ => "booleans",
            };
            return Err(Error::new(format!("Cannot {verb} strings and {kind}.")));
        }
        (Value::Long(a), Value::Long(b)) => Value::Long(long(a, *b)),
        (Value::Long(a), Value::Double(b)) => Value::Double(double(a as f64, *b)),
        (Value::Long(a), Value::Boolean(b)) => Value::Long(long(a, bit(*b))),
        (Value::Double(a), Value::Long(b)) => Value::Double(double(a, *b as f64)),
        (Value::Double(a), Value::Double(b)) => Value::Double(double(a, *b)),
        (Value::Double(a), Value::Boolean(b)) => Value::Double(double(a, f64::from(u8::from(*b)))),
        (Value::Boolean(a), Value::Long(b)) => Value::Long(long(bit(a), *b)),
        (Value::Boolean(a), Value::Double(b)) => Value::Double(double(f64::from(u8::from(a)), *b)),
        (Value::Boolean(a), Value::Boolean(b)) => Value::Long(long(bit(a), bit(*b))),
        // Series values are numbers, booleans or strings.
        (acc, _) => acc,
    })
}

/// Whether `a` then `b` break the order of the comparison `kind`, tested like Java (`a >= b`
/// for `op.lt`…), so that a `NaN` never breaks it.
fn breaks<T: PartialOrd>(kind: Kind, a: &T, b: &T) -> bool {
    match kind {
        Kind::Lt => a >= b,
        Kind::Gt => a <= b,
        Kind::Le => a > b,
        _ => a < b,
    }
}

/// Whether two consecutive values of a comparison break its order (a `null` second value
/// does).
fn breaks_order(name: &str, kind: Kind, a: &Value, b: &Value) -> Result<bool> {
    Ok(match (a, b) {
        (_, Value::Null) => true,
        (Value::Long(a), Value::Long(b)) => breaks(kind, a, b),
        (Value::Double(a), Value::Double(b)) => breaks(kind, a, b),
        (Value::String(a), Value::String(b)) => breaks(
            kind,
            &a.encode_utf16().cmp(b.encode_utf16()),
            &Ordering::Equal,
        ),
        _ => {
            return Err(Error::new(format!(
                "{name} can only be applied to Geo Time Series of the same type."
            )));
        }
    })
}

impl Op {
    /// The value of the operation on the values of a tick.
    fn compute(&self, args: &AggregatorArgs) -> Result<Value> {
        let values = args.values;
        match self.kind {
            Kind::Add | Kind::Mul => {
                let mut acc: Option<Value> = None;
                for value in values {
                    if value.is_null() {
                        if self.flag {
                            return Ok(Value::Null);
                        }
                        continue;
                    }
                    acc = Some(match acc {
                        None => value.clone(),
                        Some(acc) => combine(acc, value, self.kind == Kind::Mul)?,
                    });
                }
                Ok(acc.unwrap_or(Value::Null))
            }
            Kind::Sub | Kind::Div => {
                let [a, b] = values else {
                    return Ok(Value::Null);
                };
                if a.is_null() || b.is_null() {
                    return Ok(Value::Null);
                }
                let (Some(x), Some(y)) = (Num::of(a), Num::of(b)) else {
                    return Err(Error::new(format!(
                        "{} can only be applied to LONG or DOUBLE values.",
                        self.name
                    )));
                };
                Ok(match (self.kind, x, y) {
                    (Kind::Sub, Num::Long(x), Num::Long(y)) => Value::Long(x.wrapping_sub(y)),
                    (Kind::Sub, x, y) => Value::Double(x.as_f64() - y.as_f64()),
                    (_, x, y) => Value::Double(x.as_f64() / y.as_f64()),
                })
            }
            Kind::Mask => {
                // The ticks hold the indices of the parameter collections.
                if args.ticks.len() != 2 || args.ticks[0] == args.ticks[1] {
                    return Err(Error::new(
                        "op.mask can only be applied to pairs of Geo Time Series, the first one being boolean.",
                    ));
                }
                let is_true = values[0] == Value::Boolean(true);
                // The mask is negated when `flag` is set.
                Ok(if is_true == self.flag {
                    Value::Null
                } else {
                    values[1].clone()
                })
            }
            Kind::Eq => Ok(Value::Boolean(
                values.iter().all(|v| !v.is_null()) && values.windows(2).all(|w| w[0] == w[1]),
            )),
            Kind::Ne => {
                let mut seen: Vec<&Value> = Vec::with_capacity(values.len());
                for value in values {
                    if value.is_null() || seen.contains(&value) {
                        return Ok(Value::Boolean(false));
                    }
                    seen.push(value);
                }
                Ok(Value::Boolean(true))
            }
            Kind::Lt | Kind::Gt | Kind::Le | Kind::Ge => {
                if values[0].is_null() {
                    return Ok(Value::Boolean(false));
                }
                for pair in values.windows(2) {
                    if breaks_order(self.name, self.kind, &pair[0], &pair[1])? {
                        return Ok(Value::Boolean(false));
                    }
                }
                Ok(Value::Boolean(true))
            }
            Kind::And | Kind::Or => {
                let or = self.kind == Kind::Or;
                for value in values {
                    if value.is_null() {
                        if self.flag {
                            return Ok(Value::Null);
                        }
                        continue;
                    }
                    if *value == Value::Boolean(or) {
                        return Ok(Value::Boolean(or));
                    }
                }
                Ok(Value::Boolean(!or))
            }
        }
    }
}

impl Aggregator for Op {
    fn roles(&self) -> Roles {
        Roles::NONE
    }

    fn nary(&self) -> bool {
        true
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let value = self.compute(args)?;
        let (location, elevation) = if self.kind == Kind::Mask && !value.is_null() {
            (args.locations[1], args.elevations[1])
        } else {
            (NO_LOCATION, NO_ELEVATION)
        };
        Ok(Aggregated {
            tick: args.tick,
            location,
            elevation,
            value,
        })
    }
}
