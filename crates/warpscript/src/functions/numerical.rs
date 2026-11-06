// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Ports of `NumericalUnaryFunction` and `NumericalBinaryFunction`: the machinery that applies
//! a `java.lang.Math` style operator to numbers and to lists of numbers, with Java's numeric
//! promotion rules, to lists of numbers and to the values of numeric Geo Time Series.

use super::args::fail;
use super::gts_operators::try_unary_op;
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Type};
use crate::stack::Stack;
use crate::value::Value;

/// Whether a series can be an operand: `LONG`, `DOUBLE` or untyped (empty).
fn numeric_series(gts: &GeoTimeSerie) -> bool {
    matches!(gts.type_of(), Type::Long | Type::Double | Type::Undefined)
}

/// `cloneEmpty` of `gts` filled with `op` applied to each value.
fn map_series(gts: &GeoTimeSerie, op: &mut dyn FnMut(&Value) -> Result<Value>) -> Result<Value> {
    let mut result = gts.clone_empty();
    try_unary_op(&mut result, gts, op)?;
    Ok(Value::Gts(result.shared()))
}

const UNARY_MSG: &str =
    "can only operate on numerical values, list of numerical values or numerical GTSs.";

/// The operator(s) of a unary numerical function.
#[derive(Clone, Copy)]
pub enum Unary {
    /// Only a `double` operator: every number is converted to a `DOUBLE` first (`COS`, `SQRT`…).
    Double(fn(f64) -> f64),
    /// Only a `long` operator: every number is converted to a `LONG` first (`INCREMENTEXACT`…).
    Long(fn(i64) -> Result<i64>),
    /// Type preserving: `LONG` → `LONG`, `DOUBLE` → `DOUBLE` (`ABS`).
    Both(fn(i64) -> i64, fn(f64) -> f64),
    /// A `LONG` is kept as is, a `DOUBLE` is mapped to a `LONG` (`ROUND`).
    DoubleToLong(fn(f64) -> i64),
}

impl Unary {
    /// Apply the operator to a number.
    fn apply(self, value: &Value) -> Result<Value> {
        Ok(match (self, value) {
            (Unary::Long(f), n) => Value::Long(f(n.as_i64().unwrap_or(0))?),
            (Unary::Both(f, _), Value::Long(l)) => Value::Long(f(*l)),
            (Unary::Double(f) | Unary::Both(_, f), n) => {
                Value::Double(f(n.as_f64().unwrap_or(f64::NAN)))
            }
            (Unary::DoubleToLong(f), Value::Double(d)) => Value::Long(f(*d)),
            (Unary::DoubleToLong(f), Value::BigDecimal(bd)) => Value::Long(f(bd.double_value())),
            (Unary::DoubleToLong(_), n) => n.clone(),
        })
    }
}

/// `NumericalUnaryFunction`: pop one element and apply `op` to it, or recursively to every
/// number of a (nested) list. Lists are rebuilt, not modified in place.
pub fn unary(stack: &mut Stack, name: &str, op: Unary) -> Result<()> {
    let top = stack.pop()?;
    let result = apply_recursive(&top, name, op)?;
    stack.push(result);
    Ok(())
}

fn apply_recursive(value: &Value, name: &str, op: Unary) -> Result<Value> {
    match value {
        n if n.is_number() => op.apply(n),
        Value::Gts(gts) => {
            let gts = gts.borrow();
            if !numeric_series(&gts) {
                return Err(fail(
                    name,
                    "can only operate on LONG, DOUBLE or empty GTSs.",
                ));
            }
            map_series(&gts, &mut |v| op.apply(v))
        }
        Value::List(list) => {
            let items = list.borrow();
            let mut out = Vec::with_capacity(items.len());
            for item in items.iter() {
                out.push(apply_recursive(item, name, op)?);
            }
            Ok(Value::list(out))
        }
        _ => Err(fail(name, UNARY_MSG)),
    }
}

/// The operator(s) of a binary numerical function.
#[derive(Clone, Copy)]
pub enum Binary {
    /// Only a `double` operator: the operands are converted to `DOUBLE` (`ATAN2`, `HYPOT`…).
    Double(fn(f64, f64) -> f64),
    /// Only a `long` operator: the operands are converted to `LONG` (`FLOORDIV`, `ADDEXACT`…).
    Long(fn(i64, i64) -> Result<i64>),
    /// Java promotion: `LONG` when both operands are `LONG`, `DOUBLE` otherwise (`MAX`, `**`).
    Both(fn(i64, i64) -> i64, fn(f64, f64) -> f64),
}

impl Binary {
    /// Apply the operator to two numbers.
    fn apply(self, left: &Value, right: &Value) -> Result<Value> {
        let doubles = |f: fn(f64, f64) -> f64| {
            Value::Double(f(
                left.as_f64().unwrap_or(f64::NAN),
                right.as_f64().unwrap_or(f64::NAN),
            ))
        };
        Ok(match (self, left, right) {
            (Binary::Long(f), ..) => {
                Value::Long(f(left.as_i64().unwrap_or(0), right.as_i64().unwrap_or(0))?)
            }
            (Binary::Both(f, _), Value::Long(l), Value::Long(r)) => Value::Long(f(*l, *r)),
            (Binary::Double(f) | Binary::Both(_, f), ..) => doubles(f),
        })
    }

    /// The seed of a reduction over a list: the first element converted to the operator's type.
    fn first(self, value: &Value) -> Value {
        match self {
            Binary::Double(_) => Value::Double(value.as_f64().unwrap_or(f64::NAN)),
            Binary::Long(_) => Value::Long(value.as_i64().unwrap_or(0)),
            Binary::Both(..) => value.clone(),
        }
    }
}

/// `NumericalBinaryFunction`: two numbers, a number on top of a list (the operator is applied
/// between each element and the number), a list on top of a number (the number is the left
/// operand) or, when `apply_in_list`, a single list reduced from left to right (`MAX`, `MIN`,
/// `ADDEXACT`…; an empty list reduces to `NULL`).
pub fn binary(stack: &mut Stack, name: &str, op: Binary, apply_in_list: bool) -> Result<()> {
    let message = if apply_in_list {
        format!(
            "{name} can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values, or a list of numerical values."
        )
    } else {
        format!(
            "{name} can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values."
        )
    };
    let op0 = stack.pop()?;
    match op0 {
        ref top if top.is_number() => {
            let op1 = stack.pop()?;
            match op1 {
                ref n if n.is_number() => {
                    stack.push(op.apply(n, top)?);
                    Ok(())
                }
                Value::List(list) => {
                    let out = map_list(&list.borrow(), top, true, op, &message)?;
                    stack.push(Value::list(out));
                    Ok(())
                }
                Value::Gts(gts) if numeric_series(&gts.borrow()) => {
                    let result = map_series(&gts.borrow(), &mut |v| op.apply(v, top))?;
                    stack.push(result);
                    Ok(())
                }
                _ => Err(Error::new(message)),
            }
        }
        Value::Gts(ref gts) if !numeric_series(&gts.borrow()) => Err(Error::new(message)),
        Value::Gts(gts) if apply_in_list => {
            let values: Vec<Value> = {
                let gts = gts.borrow();
                (0..gts.size()).map(|i| gts.value_at(i)).collect()
            };
            let mut result: Option<Value> = None;
            for element in &values {
                result = Some(match result {
                    None => op.first(element),
                    Some(acc) => op.apply(&acc, element)?,
                });
            }
            stack.push(result.unwrap_or(Value::Null));
            Ok(())
        }
        Value::Gts(gts) => {
            let op1 = stack.pop()?;
            if !op1.is_number() {
                return Err(Error::new(message));
            }
            let result = map_series(&gts.borrow(), &mut |v| op.apply(&op1, v))?;
            stack.push(result);
            Ok(())
        }
        Value::List(list) if apply_in_list => {
            let items = list.borrow();
            let mut result: Option<Value> = None;
            for element in items.iter() {
                if !element.is_number() {
                    return Err(Error::new(message));
                }
                result = Some(match result {
                    None => op.first(element),
                    Some(acc) => op.apply(&acc, element)?,
                });
            }
            stack.push(result.unwrap_or(Value::Null));
            Ok(())
        }
        Value::List(list) => {
            let op1 = stack.pop()?;
            if !op1.is_number() {
                return Err(Error::new(message));
            }
            let out = map_list(&list.borrow(), &op1, false, op, &message)?;
            stack.push(Value::list(out));
            Ok(())
        }
        _ => Err(Error::new(message)),
    }
}

/// Apply the operator between each element of `list` and `operand`. When `element_first`, the
/// element is the left-hand side (number on top, list under); otherwise the operand is.
fn map_list(
    list: &[Value],
    operand: &Value,
    element_first: bool,
    op: Binary,
    message: &str,
) -> Result<Vec<Value>> {
    let mut out = Vec::with_capacity(list.len());
    for element in list {
        if !element.is_number() {
            return Err(Error::new(message.to_owned()));
        }
        let value = if element_first {
            op.apply(element, operand)?
        } else {
            op.apply(operand, element)?
        };
        out.push(value);
    }
    Ok(out)
}
