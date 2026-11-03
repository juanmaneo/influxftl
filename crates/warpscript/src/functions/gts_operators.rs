// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Geo Time Series operands of the operators (`+`, `-`, `*`, `/`, the comparisons, `&&`,
//! `||`, `!`, the bitwise operators, `->DOUBLEBITS`…): the `GTSOpsHelper` machinery and the
//! GTS branches of the Warp 10 `binary` / `unary` operator classes.
//!
//! A series combined with a scalar keeps the metadata and bucketization of the series and
//! computes a value per datapoint. Two series are combined on their common ticks only (both are
//! sorted in place first, like in Warp 10), into an unnamed series that is bucketized when both
//! inputs are bucketized on the same aligned grid.

use std::cmp::Ordering;

use super::args::fail;
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Gts, NO_ELEVATION, NO_LOCATION, Type, Values};
use crate::value::Value;

/// `GeoTimeSerie.setType` on a series that has no type yet (every caller passes a fresh or
/// `cloneEmpty` series): later values are converted to `ty`.
pub(crate) fn set_type(gts: &mut GeoTimeSerie, ty: Type) {
    gts.values = match ty {
        Type::Long => Values::Long(Vec::new()),
        Type::Double => Values::Double(Vec::new()),
        Type::Boolean => Values::Boolean(Vec::new()),
        Type::String => Values::String(Vec::new()),
        Type::Undefined => Values::Undefined,
    };
}

/// `GTSOpsHelper.handleBucketization`: the result of an operation between two series bucketized
/// with the same span on aligned grids covers the buckets of both; it is not bucketized
/// otherwise.
pub(crate) fn handle_bucketization(result: &mut GeoTimeSerie, a: &GeoTimeSerie, b: &GeoTimeSerie) {
    let span = a.bucketspan;
    if a.is_bucketized()
        && b.is_bucketized()
        && span == b.bucketspan
        && a.lastbucket.wrapping_rem(span) == b.lastbucket.wrapping_rem(span)
    {
        let first = |g: &GeoTimeSerie| {
            g.lastbucket
                .wrapping_sub(g.bucketcount.wrapping_sub(1).wrapping_mul(span))
        };
        result.bucketspan = span;
        result.lastbucket = a.lastbucket.max(b.lastbucket);
        let firstbucket = first(a).min(first(b));
        // `(int) ((lastbucket - firstbucket) / bucketspan) + 1`
        let count = result
            .lastbucket
            .wrapping_sub(firstbucket)
            .wrapping_div(span) as i32;
        result.bucketcount = i64::from(count.wrapping_add(1));
    }
}

/// The index pairs of the datapoints of `a` and `b` sharing a tick, in tick order, after both
/// series are sorted in place (the sweeping line of `GTSOpsHelper.applyBinaryOp`: duplicate
/// ticks are paired in order).
fn common_ticks(a: &Gts, b: &Gts) -> Vec<(usize, usize)> {
    a.borrow_mut().sort(false);
    b.borrow_mut().sort(false);
    let (ga, gb) = (a.borrow(), b.borrow());
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < ga.ticks.len() && j < gb.ticks.len() {
        match ga.ticks[i].cmp(&gb.ticks[j]) {
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
            Ordering::Equal => {
                pairs.push((i, j));
                i += 1;
                j += 1;
            }
        }
    }
    pairs
}

/// `GTSOpsHelper.applyBinaryOp`: a new series of type `ty` with `op(a[t], b[t])` at every tick
/// `t` common to `a` and `b` (`null` results are skipped), located like `a` when
/// `copy_location`.
///
/// The operation is a trait object so that the helper is compiled (and measured) once.
fn try_binary_op(
    a: &Gts,
    b: &Gts,
    ty: Type,
    copy_location: bool,
    op: &mut dyn FnMut(&Value, &Value) -> Result<Value>,
) -> Result<GeoTimeSerie> {
    let mut result = GeoTimeSerie::new();
    handle_bucketization(&mut result, &a.borrow(), &b.borrow());
    set_type(&mut result, ty);
    let pairs = common_ticks(a, b);
    let (ga, gb) = (a.borrow(), b.borrow());
    for (i, j) in pairs {
        let value = op(&ga.value_at(i), &gb.value_at(j))?;
        let (location, elevation) = if copy_location {
            (ga.location_at(i), ga.elevation_at(i))
        } else {
            (NO_LOCATION, NO_ELEVATION)
        };
        result.set_value(ga.ticks[i], location, elevation, &value, false);
    }
    Ok(result)
}

/// [`try_binary_op`] with an infallible operation.
fn binary_op(
    a: &Gts,
    b: &Gts,
    ty: Type,
    copy_location: bool,
    mut op: impl FnMut(&Value, &Value) -> Value,
) -> GeoTimeSerie {
    try_binary_op(a, b, ty, copy_location, &mut |x, y| Ok(op(x, y))).unwrap_or_default()
}

/// A result series, reset to an untyped empty series (`cloneEmpty`) when it has no datapoint.
fn finish(result: GeoTimeSerie) -> Value {
    let result = if result.size() == 0 {
        result.clone_empty()
    } else {
        result
    };
    Value::Gts(result.shared())
}

/// An empty unnamed series bucketized like the result of an operation between `a` and `b`.
fn empty_result(a: &GeoTimeSerie, b: &GeoTimeSerie) -> Value {
    let mut result = GeoTimeSerie::new();
    handle_bucketization(&mut result, a, b);
    Value::Gts(result.shared())
}

/// `GTSOpsHelper.applyUnaryOp`: the datapoints of `gts` (with their location and elevation)
/// with the values mapped by `op` (`null` results are skipped), into `result`.
pub(crate) fn try_unary_op(
    result: &mut GeoTimeSerie,
    gts: &GeoTimeSerie,
    op: &mut dyn FnMut(&Value) -> Result<Value>,
) -> Result<()> {
    for i in 0..gts.size() {
        let value = op(&gts.value_at(i))?;
        result.set_value(
            gts.tick_at(i),
            gts.location_at(i),
            gts.elevation_at(i),
            &value,
            false,
        );
    }
    Ok(())
}

/// [`try_unary_op`] with an infallible operation.
pub(crate) fn unary_op(
    result: &mut GeoTimeSerie,
    gts: &GeoTimeSerie,
    mut op: impl FnMut(&Value) -> Value,
) {
    try_unary_op(result, gts, &mut |value| Ok(op(value))).unwrap_or_default();
}

fn long_of(value: &Value) -> i64 {
    value.as_i64().unwrap_or_default()
}

fn double_of(value: &Value) -> f64 {
    value.as_f64().unwrap_or_default()
}

fn shared(gts: GeoTimeSerie) -> Value {
    Value::Gts(gts.shared())
}

// -----------------------------------------------------------------------------------------
// Arithmetic
// -----------------------------------------------------------------------------------------

const BOOLEAN_ADD: &str = "cannot operate on BOOLEAN Geo Time Series.";

/// `+` with a series operand: string concatenation or numeric addition. `None` when neither
/// operand is a series.
pub(super) fn add(op1: &Value, op2: &Value) -> Option<Result<Value>> {
    Some(match (op1, op2) {
        (Value::Gts(a), Value::Gts(b)) => add_series(a, b),
        (Value::Gts(gts), other) => add_scalar(&gts.borrow(), other, true),
        (other, Value::Gts(gts)) => add_scalar(&gts.borrow(), other, false),
        _ => return None,
    })
}

fn add_series(a: &Gts, b: &Gts) -> Result<Value> {
    let (t1, t2) = (a.borrow().type_of(), b.borrow().type_of());
    let either = |t: Type| t1 == t || t2 == t;
    if either(Type::Boolean) {
        return Err(fail("+", BOOLEAN_ADD));
    }
    let ty = [Type::String, Type::Double, Type::Long]
        .into_iter()
        .find(|t| either(*t))
        .unwrap_or(Type::Undefined);
    let result = match ty {
        Type::String => binary_op(a, b, ty, false, |x, y| Value::from(format!("{x}{y}"))),
        Type::Double => binary_op(a, b, ty, false, |x, y| {
            Value::Double(double_of(x) + double_of(y))
        }),
        Type::Long => binary_op(a, b, ty, false, |x, y| {
            Value::Long(long_of(x).wrapping_add(long_of(y)))
        }),
        // Both series are untyped (thus empty): no operation, not even the bucketization.
        _ => GeoTimeSerie::new(),
    };
    Ok(finish(result))
}

fn add_scalar(gts: &GeoTimeSerie, other: &Value, gts_first: bool) -> Result<Value> {
    let ty = gts.type_of();
    let numeric = matches!(ty, Type::Long | Type::Double);
    let target = match other {
        Value::Double(_) | Value::Long(_) if ty == Type::Boolean => {
            return Err(fail("+", BOOLEAN_ADD));
        }
        Value::Double(_) if numeric => Type::Double,
        Value::Long(_) if numeric => ty,
        Value::String(_) | Value::Double(_) | Value::Long(_) => Type::String,
        _ => {
            return Err(fail(
                "+",
                "can only be used with String or numeric types with a GTS.",
            ));
        }
    };
    let mut result = gts.clone_empty();
    unary_op(&mut result, gts, |value| match target {
        Type::String if gts_first => Value::from(format!("{value}{other}")),
        Type::String => Value::from(format!("{other}{value}")),
        Type::Double => Value::Double(double_of(value) + double_of(other)),
        _ => Value::Long(long_of(value).wrapping_add(long_of(other))),
    });
    Ok(shared(result))
}

/// The operators of `-`, `*` and `/` on series.
#[derive(Clone, Copy)]
pub(super) struct Arithmetic {
    /// The operator name.
    pub name: &'static str,
    /// The type check error message (after the name).
    pub message: &'static str,
    /// The `LONG` operation (`/` fails on a zero divisor).
    pub long: fn(i64, i64) -> Result<i64>,
    /// The `DOUBLE` operation.
    pub double: fn(f64, f64) -> f64,
}

impl Arithmetic {
    fn apply(self, as_double: bool, x: &Value, y: &Value) -> Result<Value> {
        Ok(if as_double {
            Value::Double((self.double)(double_of(x), double_of(y)))
        } else {
            Value::Long((self.long)(long_of(x), long_of(y))?)
        })
    }
}

/// `-`, `*` or `/` when the operands are not two numbers (`SUB`, `MUL`, `DIV`): numeric series
/// and numbers only.
pub(super) fn arithmetic(op: Arithmetic, op1: &Value, op2: &Value) -> Result<Value> {
    let numeric = |t: Type| matches!(t, Type::Long | Type::Double);
    match (op1, op2) {
        (Value::Gts(a), Value::Gts(b)) => {
            let ok = |g: &Gts| {
                let g = g.borrow();
                numeric(g.type_of()) || g.size() == 0
            };
            if !ok(a) || !ok(b) {
                return Err(fail(op.name, op.message));
            }
            let longs = a.borrow().type_of() == Type::Long && b.borrow().type_of() == Type::Long;
            let ty = if longs { Type::Long } else { Type::Double };
            Ok(finish(try_binary_op(a, b, ty, false, &mut |x, y| {
                op.apply(!longs, x, y)
            })?))
        }
        (Value::Gts(gts), scalar @ (Value::Long(_) | Value::Double(_))) => {
            arithmetic_scalar(op, &gts.borrow(), scalar, true)
        }
        (scalar @ (Value::Long(_) | Value::Double(_)), Value::Gts(gts)) => {
            arithmetic_scalar(op, &gts.borrow(), scalar, false)
        }
        _ => Err(fail(op.name, op.message)),
    }
}

fn arithmetic_scalar(
    op: Arithmetic,
    gts: &GeoTimeSerie,
    scalar: &Value,
    gts_first: bool,
) -> Result<Value> {
    let mut result = gts.clone_empty();
    if gts.size() > 0 {
        let ty = gts.type_of();
        if !matches!(ty, Type::Long | Type::Double) {
            return Err(fail(op.name, op.message));
        }
        let as_double = matches!(scalar, Value::Double(_)) || ty == Type::Double;
        try_unary_op(&mut result, gts, &mut |value| {
            if gts_first {
                op.apply(as_double, value, scalar)
            } else {
                op.apply(as_double, scalar, value)
            }
        })?;
    }
    Ok(shared(result))
}

// -----------------------------------------------------------------------------------------
// Comparisons
// -----------------------------------------------------------------------------------------

/// A comparison operator (`ComparisonOperation`): its name, its outcome for an ordering, and
/// its outcomes when one or both operands are `NaN`.
#[derive(Clone, Copy)]
pub(super) struct Comparison {
    /// The operator name.
    pub name: &'static str,
    /// The outcome for the ordering of the left operand relative to the right one.
    pub operator: fn(Ordering) -> bool,
    /// The outcome when exactly one operand is `NaN`.
    pub one_nan: bool,
    /// The outcome when both operands are `NaN`.
    pub two_nan: bool,
}

impl Comparison {
    /// The outcome for two numbers, with the `NaN` rules.
    fn numbers(self, x: &Value, y: &Value) -> bool {
        let nan = |v: &Value| matches!(v, Value::Double(d) if d.is_nan());
        match (nan(x), nan(y)) {
            (true, true) => self.two_nan,
            (false, false) => (self.operator)(super::operators::compare_numbers(x, y)),
            _ => self.one_nan,
        }
    }
}

/// Java `String.compareTo` (UTF-16 code unit order) of the `toString` of two values.
fn compare_text(x: &Value, y: &Value) -> Ordering {
    x.to_string()
        .encode_utf16()
        .cmp(y.to_string().encode_utf16())
}

/// The value kept by a comparison: the series value when the comparison holds, else `null`.
fn keep(holds: bool, value: &Value) -> Value {
    if holds { value.clone() } else { Value::Null }
}

/// `ComparisonOperation.comparison` with a series operand: the series filtered on the
/// datapoints whose value compares as requested to the other operand (or, for two series, to
/// the value of the other series at the same tick). `None` when neither operand is a series.
pub(super) fn compare(cmp: Comparison, op1: &Value, op2: &Value) -> Option<Result<Value>> {
    Some(match (op1, op2) {
        (Value::Gts(a), Value::Gts(b)) => compare_series(cmp, a, b),
        (
            Value::Gts(gts),
            Value::String(_) | Value::Long(_) | Value::Double(_) | Value::Boolean(_),
        ) if gts.borrow().type_of() == Type::Undefined => Ok(shared(gts.borrow().clone_empty())),
        (Value::Gts(gts), other) => compare_scalar(cmp, &gts.borrow(), other, false),
        (other, Value::Gts(gts)) => compare_scalar(cmp, &gts.borrow(), other, true),
        _ => return None,
    })
}

/// The error of a comparison between operands of different types.
pub(super) fn same_type(name: &str) -> Error {
    fail(
        name,
        "can only operate when GTS values and the top stack operand have the same type.",
    )
}

fn compare_series(cmp: Comparison, a: &Gts, b: &Gts) -> Result<Value> {
    let (t1, t2) = (a.borrow().type_of(), b.borrow().type_of());
    let numeric = |t: Type| matches!(t, Type::Long | Type::Double);
    if t1 == Type::Undefined || t2 == Type::Undefined {
        Ok(empty_result(&a.borrow(), &b.borrow()))
    } else if t1 == Type::String && t2 == Type::String {
        Ok(finish(binary_op(a, b, Type::String, true, |x, y| {
            keep((cmp.operator)(compare_text(x, y)), x)
        })))
    } else if numeric(t1) && numeric(t2) {
        let ty = if t1 == Type::Double || t2 == Type::Double {
            Type::Double
        } else {
            Type::Long
        };
        Ok(finish(binary_op(a, b, ty, true, |x, y| {
            keep(cmp.numbers(x, y), x)
        })))
    } else {
        // Sic: no space after the name in Warp 10.
        Err(Error::new(format!(
            "{}can only operate on two GTS with NUMBER or STRING values.",
            cmp.name
        )))
    }
}

/// A series compared to a scalar; `reversed` when the scalar is the left operand.
fn compare_scalar(
    cmp: Comparison,
    gts: &GeoTimeSerie,
    other: &Value,
    reversed: bool,
) -> Result<Value> {
    let ordering = |o: Ordering| if reversed { o.reverse() } else { o };
    let ty = gts.type_of();
    let mut result = gts.clone_empty();
    set_type(&mut result, ty);
    match (ty, other) {
        (Type::String, Value::String(_)) => unary_op(&mut result, gts, |v| {
            keep((cmp.operator)(ordering(compare_text(v, other))), v)
        }),
        (Type::Double, Value::Double(d)) if d.is_nan() => unary_op(&mut result, gts, |v| {
            let nan = double_of(v).is_nan();
            keep(if nan { cmp.two_nan } else { cmp.one_nan }, v)
        }),
        (Type::Long, Value::Double(d)) if d.is_nan() => {
            // Every comparison has the one `NaN` outcome: all the datapoints or none (the
            // empty result keeps its type).
            return Ok(shared(if cmp.one_nan { gts.clone() } else { result }));
        }
        (Type::Double | Type::Long, Value::Long(_) | Value::Double(_)) => {
            unary_op(&mut result, gts, |v| {
                let holds = if double_of(v).is_nan() {
                    cmp.one_nan
                } else {
                    (cmp.operator)(ordering(super::operators::compare_numbers(v, other)))
                };
                keep(holds, v)
            });
        }
        (Type::Boolean, Value::Boolean(b)) => unary_op(&mut result, gts, |v| {
            keep(
                (cmp.operator)(ordering(v.as_bool().unwrap_or_default().cmp(b))),
                v,
            )
        }),
        _ => return Err(same_type(cmp.name)),
    }
    Ok(finish(result))
}

// -----------------------------------------------------------------------------------------
// Logical and bitwise
// -----------------------------------------------------------------------------------------

/// `&&` / `||` between two series (`CondShortCircuit`): boolean series only.
pub(super) fn logical(name: &str, a: &Gts, b: &Gts, op: fn(bool, bool) -> bool) -> Result<Value> {
    let (t1, t2) = (a.borrow().type_of(), b.borrow().type_of());
    let boolean_or_empty = |t: Type| matches!(t, Type::Boolean | Type::Undefined);
    if t1 == Type::Boolean && t2 == Type::Boolean {
        Ok(finish(binary_op(a, b, Type::Boolean, false, |x, y| {
            Value::Boolean(op(
                x.as_bool().unwrap_or_default(),
                y.as_bool().unwrap_or_default(),
            ))
        })))
    } else if boolean_or_empty(t1) && boolean_or_empty(t2) {
        Ok(empty_result(&a.borrow(), &b.borrow()))
    } else {
        Err(fail(
            name,
            "can only operate on boolean values or boolean GTS.",
        ))
    }
}

/// `!` on a series: the boolean values negated.
pub(super) fn not(name: &str, gts: &GeoTimeSerie) -> Result<Value> {
    match &gts.values {
        Values::Boolean(values) => {
            let mut result = gts.clone();
            result.values = Values::Boolean(values.iter().map(|value| !value).collect());
            Ok(shared(result))
        }
        Values::Undefined => Ok(shared(gts.clone_empty())),
        _ => Err(fail(
            name,
            "can only operate on a boolean value or a boolean GTS.",
        )),
    }
}

/// A bitwise operator with a series operand (`BitwiseOperation`): two `LONG` series, or a
/// `LONG` series and a `LONG` on top. `None` when the operands are not of these kinds.
pub(super) fn bitwise(op1: &Value, op2: &Value, op: fn(i64, i64) -> i64) -> Option<Value> {
    let apply = |x: &Value, y: &Value| Value::Long(op(long_of(x), long_of(y)));
    match (op1, op2) {
        (Value::Gts(a), Value::Gts(b)) => {
            let types = (a.borrow().type_of(), b.borrow().type_of());
            match types {
                // Sic: an empty result keeps its type.
                (Type::Long, Type::Long) => Some(shared(binary_op(a, b, Type::Long, false, apply))),
                (Type::Undefined, Type::Long) | (Type::Long, Type::Undefined) => {
                    Some(shared(GeoTimeSerie::new()))
                }
                _ => None,
            }
        }
        (Value::Gts(gts), mask @ Value::Long(_)) => {
            let gts = gts.borrow();
            match gts.type_of() {
                Type::Long => {
                    let mut result = gts.clone_empty();
                    set_type(&mut result, Type::Long);
                    unary_op(&mut result, &gts, |v| apply(v, mask));
                    Some(shared(result))
                }
                Type::Undefined => Some(shared(gts.clone_empty())),
                _ => None,
            }
        }
        _ => None,
    }
}

// -----------------------------------------------------------------------------------------
// Bits
// -----------------------------------------------------------------------------------------

/// `->DOUBLEBITS` / `->FLOATBITS` on a series (`TOBITS`): a new series (same metadata, no
/// bucketization, as decoded from a `GTSEncoder`) with the bits of the values.
pub(super) fn to_bits(name: &str, gts: &GeoTimeSerie, convert: fn(&Value) -> i64) -> Result<Value> {
    if !matches!(gts.type_of(), Type::Long | Type::Double) {
        return Err(fail(name, "operates on a DOUBLE or LONG Geo Time Series."));
    }
    Ok(reencoded(gts, |v| Value::Long(convert(v))))
}

/// `DOUBLEBITS->` / `FLOATBITS->` on a series (`FROMBITS`): a new series (same metadata, no
/// bucketization) with the doubles of the bits.
pub(super) fn from_bits(name: &str, gts: &GeoTimeSerie, convert: fn(i64) -> f64) -> Result<Value> {
    if gts.type_of() != Type::Long {
        return Err(fail(name, "operates on a LONG Geo Time Series."));
    }
    Ok(reencoded(gts, |v| Value::Double(convert(long_of(v)))))
}

fn reencoded(gts: &GeoTimeSerie, op: impl FnMut(&Value) -> Value) -> Value {
    let mut result = GeoTimeSerie::new();
    result.metadata = gts.metadata.clone();
    unary_op(&mut result, gts, op);
    shared(result)
}
