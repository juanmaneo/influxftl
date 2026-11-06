// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Arithmetic, comparison, bitwise and logical operators, and the scalar type conversions.
//!
//! The Geo Time Series operands are handled in [`super::gts_operators`]. The matrix, vector and
//! `BIGDECIMAL` operand types of Warp 10 are not modelled yet; the operators raise the Warp 10
//! error message for them.

use std::cmp::Ordering;
use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::bd::{bd_value, to_big_decimal, to_big_integer};
use super::gts_operators::{self, Arithmetic, Comparison};
use super::matrix::{
    add_matrices, add_vectors, map_matrix, matrix_value, multiply, operate, pre_multiply,
    vector_value,
};
use super::numerical::{self, Binary};
use crate::bigdecimal::BigDecimal;
use crate::error::{Error, Result};
use crate::jmath;
use crate::macros::Macro;
use crate::stack::Stack;
use crate::value::{Value, double_to_long, ensure_mutable};

/// Platform time units per millisecond (microseconds).
pub const TIME_UNITS_PER_MS: f64 = 1000.0;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("+", Entry::Stack(add)),
    ("-", Entry::Stack(sub)),
    ("*", Entry::Stack(mul)),
    ("/", Entry::Stack(div)),
    ("%", Entry::Stack(modulo)),
    ("**", Entry::Stack(pow)),
    ("+!", Entry::Stack(inplace_add)),
    ("==", Entry::Stack(eq)),
    ("!=", Entry::Stack(ne)),
    ("<", Entry::Stack(lt)),
    (">", Entry::Stack(gt)),
    ("<=", Entry::Stack(le)),
    (">=", Entry::Stack(ge)),
    ("&&", Entry::Builder(and)),
    ("AND", Entry::Builder(and)),
    ("||", Entry::Builder(or)),
    ("OR", Entry::Builder(or)),
    ("!", Entry::Builder(not)),
    ("NOT", Entry::Builder(not)),
    ("~", Entry::Stack(complement)),
    ("&", Entry::Stack(bitwise_and)),
    ("|", Entry::Stack(bitwise_or)),
    ("^", Entry::Stack(bitwise_xor)),
    ("<<", Entry::Stack(shiftleft)),
    (">>", Entry::Stack(shiftright_signed)),
    (">>>", Entry::Stack(shiftright_unsigned)),
    ("REVBITS", Entry::Stack(reversebits)),
    ("TODOUBLE", Entry::Stack(todouble)),
    ("TOLONG", Entry::Stack(tolong)),
    ("TOBOOLEAN", Entry::Stack(toboolean)),
    ("TOSTRING", Entry::Stack(tostring)),
    ("TOHEX", Entry::Stack(tohex)),
    ("->HEX", Entry::Stack(tohex_arrow)),
    ("FROMHEX", Entry::Stack(fromhex)),
    ("TOBIN", Entry::Stack(tobin)),
    ("->BIN", Entry::Stack(tobin_arrow)),
    ("FROMBIN", Entry::Stack(frombin)),
    ("->DOUBLEBITS", Entry::Stack(to_double_bits)),
    ("DOUBLEBITS->", Entry::Stack(from_double_bits)),
    ("->FLOATBITS", Entry::Stack(to_float_bits)),
    ("FLOATBITS->", Entry::Stack(from_float_bits)),
    ("w", Entry::Stack(unit_week)),
    ("d", Entry::Stack(unit_day)),
    ("h", Entry::Stack(unit_hour)),
    ("m", Entry::Stack(unit_minute)),
    ("s", Entry::Stack(unit_second)),
    ("ms", Entry::Stack(unit_milli)),
    ("us", Entry::Stack(unit_micro)),
    ("ns", Entry::Stack(unit_nano)),
    ("ps", Entry::Stack(unit_pico)),
];

// -----------------------------------------------------------------------------------------
// Arithmetic
// -----------------------------------------------------------------------------------------

/// Apply a numeric binary operator with Java promotion: `DOUBLE` if either operand is a
/// `DOUBLE`, otherwise wrapping `LONG` arithmetic.
fn numeric(
    op1: &Value,
    op2: &Value,
    on_long: fn(i64, i64) -> i64,
    on_double: fn(f64, f64) -> f64,
) -> Value {
    match (op1, op2) {
        (Value::Long(a), Value::Long(b)) => Value::Long(on_long(*a, *b)),
        _ => {
            let a = op1.as_f64().unwrap_or(f64::NAN);
            let b = op2.as_f64().unwrap_or(f64::NAN);
            Value::Double(on_double(a, b))
        }
    }
}

fn add(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    let result = match (&op1, &op2) {
        (a, b) if a.is_number() && b.is_number() && has_bd(a, b) => {
            bd_value(to_big_decimal("+", a)?.add(&to_big_decimal("+", b)?))
        }
        (a, b) if a.is_number() && b.is_number() => numeric(a, b, i64::wrapping_add, |x, y| x + y),
        (Value::String(a), Value::String(b)) => Value::from(format!("{a}{b}")),
        (Value::List(a), _) => {
            let mut list = a.borrow().clone();
            list.push(op2.clone());
            Value::list(list)
        }
        (Value::Set(a), _) => {
            let mut set = a.borrow().clone();
            set.insert(op2.clone());
            Value::set(set)
        }
        (Value::Macro(a), Value::Macro(b)) => {
            let mut statements = a.statements.clone();
            statements.extend(b.statements.iter().cloned());
            let m = Macro::from_statements(statements);
            m.secure.set(a.is_secure() || b.is_secure());
            Value::Macro(Rc::new(m))
        }
        (Value::Bytes(a), Value::Bytes(b)) => {
            let mut bytes = a.to_vec();
            bytes.extend_from_slice(b);
            Value::bytes(bytes)
        }
        (Value::Matrix(a), Value::Matrix(b)) => matrix_value(add_matrices(a, b, 1.0)?),
        (Value::Matrix(m), n) | (n, Value::Matrix(m)) if n.is_number() => {
            let x = n.as_f64().unwrap_or_default();
            matrix_value(map_matrix(m, |v| v + x))
        }
        (Value::Vector(a), Value::Vector(b)) => vector_value(add_vectors(a, b, 1.0)?),
        (Value::Vector(v), n) | (n, Value::Vector(v)) if n.is_number() => {
            let x = n.as_f64().unwrap_or_default();
            vector_value(v.iter().map(|e| e + x).collect())
        }
        _ => match gts_operators::add(&op1, &op2) {
            Some(result) => result?,
            None => {
                return Err(fail(
                    "+",
                    "can only operate on numeric, string, lists, matrices, vectors, Geo Time Series, byte array and macro values.",
                ));
            }
        },
    };
    stack.push(result);
    Ok(())
}

const SUB_MSG: &str =
    "can only operate on numeric values, vectors, matrices and numeric Geo Time Series.";
const MUL_MSG: &str =
    "can only operate on numeric values, vectors, matrices and numeric Geo Time Series.";
const DIV_MSG: &str = "can only operate on numeric values, vectors and numeric Geo Time Series.";

/// `-`, `*` and `/`: two numbers, or series operands.
fn arithmetic(stack: &mut Stack, op: Arithmetic) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    let result = if op1.is_number() && op2.is_number() && has_bd(&op1, &op2) {
        let (a, b) = (
            to_big_decimal(op.name, &op1)?,
            to_big_decimal(op.name, &op2)?,
        );
        bd_value(match op.name {
            "-" => a.subtract(&b),
            "*" => a.multiply(&b)?,
            _ => a.divide(&b)?,
        })
    } else if let Some(result) = matrix_arithmetic(op.name, &op1, &op2) {
        result?
    } else if op1.is_number() && op2.is_number() {
        match (&op1, &op2) {
            (Value::Long(a), Value::Long(b)) => Value::Long((op.long)(*a, *b)?),
            _ => Value::Double((op.double)(
                op1.as_f64().unwrap_or(f64::NAN),
                op2.as_f64().unwrap_or(f64::NAN),
            )),
        }
    } else {
        gts_operators::arithmetic(op, &op1, &op2)?
    };
    stack.push(result);
    Ok(())
}

/// `-`, `*` and `/` on matrices and vectors (`None` for other operands).
fn matrix_arithmetic(name: &str, op1: &Value, op2: &Value) -> Option<Result<Value>> {
    let number = |v: &Value| v.is_number().then(|| v.as_f64().unwrap_or_default());
    Some(match (name, op1, op2) {
        ("-", Value::Matrix(a), Value::Matrix(b)) => add_matrices(a, b, -1.0).map(matrix_value),
        ("-", Value::Vector(a), Value::Vector(b)) => add_vectors(a, b, -1.0).map(vector_value),
        ("*", Value::Matrix(a), Value::Matrix(b)) => multiply(a, b).map(matrix_value),
        ("*", Value::Vector(v), Value::Matrix(m)) => pre_multiply(m, v).map(vector_value),
        ("*", Value::Matrix(m), Value::Vector(v)) => operate(m, v).map(vector_value),
        ("*", Value::Matrix(m), n) | ("*", n, Value::Matrix(m)) => {
            let x = number(n)?;
            Ok(matrix_value(map_matrix(m, |v| v * x)))
        }
        ("*", Value::Vector(v), n) | ("*", n, Value::Vector(v)) => {
            let x = number(n)?;
            Ok(vector_value(v.iter().map(|e| e * x).collect()))
        }
        ("/", Value::Vector(v), n) => {
            let x = number(n)?;
            Ok(vector_value(v.iter().map(|e| e / x).collect()))
        }
        _ => return None,
    })
}

/// Java `long` division: `ArithmeticException` on a zero divisor, wrapping otherwise.
fn long_div(a: i64, b: i64) -> Result<i64> {
    if b == 0 {
        Err(Error::new("/ by zero"))
    } else {
        Ok(a.wrapping_div(b))
    }
}

fn sub(stack: &mut Stack) -> Result<()> {
    arithmetic(
        stack,
        Arithmetic {
            name: "-",
            message: SUB_MSG,
            long: |a, b| Ok(a.wrapping_sub(b)),
            double: |x, y| x - y,
        },
    )
}

fn mul(stack: &mut Stack) -> Result<()> {
    arithmetic(
        stack,
        Arithmetic {
            name: "*",
            message: MUL_MSG,
            long: |a, b| Ok(a.wrapping_mul(b)),
            double: |x, y| x * y,
        },
    )
}

fn div(stack: &mut Stack) -> Result<()> {
    arithmetic(
        stack,
        Arithmetic {
            name: "/",
            message: DIV_MSG,
            long: long_div,
            double: |x, y| x / y,
        },
    )
}

fn modulo(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    match (&op1, &op2) {
        _ if !op1.is_number() || !op2.is_number() => {
            Err(fail("%", "operates on numeric arguments."))
        }
        (a, b) if has_bd(a, b) => {
            let remainder = to_big_decimal("%", a)?.remainder(&to_big_decimal("%", b)?)?;
            stack.push(bd_value(remainder));
            Ok(())
        }
        (Value::Long(_), Value::Long(0)) => Err(Error::new("/ by zero")),
        (a, b) => {
            stack.push(numeric(a, b, i64::wrapping_rem, |x, y| x % y));
            Ok(())
        }
    }
}

/// `**` is a `NumericalBinaryFunction` built on `Math.pow`; the `LONG` form truncates the
/// double result like Java's `(long)` cast.
fn pow(stack: &mut Stack) -> Result<()> {
    numerical::binary(
        stack,
        "**",
        Binary::Both(
            |a, b| double_to_long(jmath::pow(a as f64, b as f64)),
            jmath::pow,
        ),
        false,
    )
}

fn inplace_add(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    match &op1 {
        Value::List(list) => {
            ensure_mutable(list)?;
            list.borrow_mut().push(op2);
            stack.push(op1);
            Ok(())
        }
        Value::Set(set) => {
            ensure_mutable(set)?;
            set.borrow_mut().insert(op2);
            stack.push(op1);
            Ok(())
        }
        _ => Err(fail("+!", "can only operate on lists and sets.")),
    }
}

// -----------------------------------------------------------------------------------------
// Comparison
// -----------------------------------------------------------------------------------------

/// Exact comparison of a `LONG` and a `DOUBLE` (Java `EQ.compare` via `BigDecimal`), avoiding
/// the precision loss of casting the long to a double.
pub(crate) fn compare_long_double(l: i64, d: f64) -> Ordering {
    // `d` is finite and non-NaN here (callers screen NaN first).
    if d >= 9_223_372_036_854_775_808.0 {
        return Ordering::Less;
    }
    if d < -9_223_372_036_854_775_808.0 {
        return Ordering::Greater;
    }
    let truncated = d.trunc() as i64;
    match l.cmp(&truncated) {
        Ordering::Equal => {
            let frac = d.fract();
            if frac > 0.0 {
                Ordering::Less
            } else if frac < 0.0 {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        }
        other => other,
    }
}

/// The ordering used by the comparison operators: `EQ.compare(Number, Number)` for the
/// `LONG` / `DOUBLE` types (no NaN), `String.compareTo` for strings and lexicographic unsigned
/// byte order for byte arrays. `None` when the operands are not of a comparable pair of types.
fn compare_values(a: &Value, b: &Value) -> Option<Ordering> {
    Some(match (a, b) {
        (Value::Long(x), Value::Long(y)) => x.cmp(y),
        (Value::Double(x), Value::Double(y)) => x.total_cmp(y),
        (Value::Long(l), Value::Double(d)) => compare_long_double(*l, *d),
        (Value::Double(d), Value::Long(l)) => compare_long_double(*l, *d).reverse(),
        (Value::String(x), Value::String(y)) => x.encode_utf16().cmp(y.encode_utf16()),
        (Value::Bytes(x), Value::Bytes(y)) => x.as_ref().cmp(y.as_ref()),
        _ => return None,
    })
}

/// Whether either operand is a `BIGDECIMAL` (the operators then compute with `BigDecimal`).
fn has_bd(a: &Value, b: &Value) -> bool {
    matches!(a, Value::BigDecimal(_)) || matches!(b, Value::BigDecimal(_))
}

/// `EQ.compare` when a `BIGDECIMAL` is involved: `equals`, then the comparison of the exact
/// `BigDecimal`s (`new BigDecimal(double)` for doubles, which fails on infinities).
fn eq_compare_bd(a: &Value, b: &Value) -> Result<Ordering> {
    if a == b {
        return Ok(Ordering::Equal);
    }
    let exact = |v: &Value| match v {
        Value::Double(d) => BigDecimal::from_f64_exact(*d),
        Value::BigDecimal(bd) => Ok((**bd).clone()),
        other => Ok(BigDecimal::from_i64(other.as_i64().unwrap_or_default())),
    };
    Ok(exact(a)?.compare(&exact(b)?))
}

/// `EQ.compare` of two numbers (`Equal` for operands that are not two numbers).
pub(super) fn compare_numbers(a: &Value, b: &Value) -> Ordering {
    compare_values(a, b).unwrap_or(Ordering::Equal)
}

/// The ordering comparison shared by `<`, `>`, `<=`, `>=` (and the numeric part of `==`/`!=`).
fn comparison(
    stack: &mut Stack,
    op1: &Value,
    op2: &Value,
    one_nan: bool,
    two_nan: bool,
    operator: fn(Ordering) -> bool,
    name: &'static str,
) -> Result<()> {
    let cmp = Comparison {
        name,
        operator,
        one_nan,
        two_nan,
    };
    if let Some(result) = gts_operators::compare(cmp, op1, op2) {
        stack.push(result?);
        return Ok(());
    }
    let nan1 = matches!(op1, Value::Double(d) if d.is_nan());
    let nan2 = matches!(op2, Value::Double(d) if d.is_nan());
    let result = if nan1 && nan2 {
        two_nan
    } else if nan1 || nan2 {
        one_nan
    } else if op1.is_number() && op2.is_number() && has_bd(op1, op2) {
        operator(eq_compare_bd(op1, op2)?)
    } else if let Some(ordering) = compare_values(op1, op2) {
        operator(ordering)
    } else {
        return Err(gts_operators::same_type(name));
    };
    stack.push(result);
    Ok(())
}

/// `EQ` / `NE` hand numbers, series, and a series with a scalar to the comparison.
fn compared_by_eq(op1: &Value, op2: &Value) -> bool {
    let scalar = |v: &Value| {
        matches!(
            v,
            Value::Long(_) | Value::Double(_) | Value::String(_) | Value::Boolean(_)
        )
    };
    match (op1, op2) {
        (Value::Gts(_), other) | (other, Value::Gts(_)) => {
            scalar(other) || matches!(other, Value::Gts(_))
        }
        _ => op1.is_number() && op2.is_number(),
    }
}

fn lt(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    comparison(
        stack,
        &op1,
        &op2,
        false,
        false,
        |o| o == Ordering::Less,
        "<",
    )
}

fn gt(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    comparison(
        stack,
        &op1,
        &op2,
        false,
        false,
        |o| o == Ordering::Greater,
        ">",
    )
}

fn le(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    comparison(
        stack,
        &op1,
        &op2,
        false,
        true,
        |o| o != Ordering::Greater,
        "<=",
    )
}

fn ge(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    comparison(
        stack,
        &op1,
        &op2,
        false,
        true,
        |o| o != Ordering::Less,
        ">=",
    )
}

fn eq(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    if compared_by_eq(&op1, &op2) {
        comparison(
            stack,
            &op1,
            &op2,
            false,
            true,
            |o| o == Ordering::Equal,
            "==",
        )
    } else {
        stack.push(op1 == op2);
        Ok(())
    }
}

fn ne(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    if compared_by_eq(&op1, &op2) {
        comparison(
            stack,
            &op1,
            &op2,
            true,
            false,
            |o| o != Ordering::Equal,
            "!=",
        )
    } else {
        stack.push(op1 != op2);
        Ok(())
    }
}

// -----------------------------------------------------------------------------------------
// Logical
// -----------------------------------------------------------------------------------------

/// `CondShortCircuit`: `&&` triggers (short-circuits) on `false`, `||` on `true`. Operates on
/// two booleans, or on a list of booleans / macros (each macro leaving a boolean).
fn short_circuit(stack: &mut Stack, name: &str, trigger: bool) -> Result<()> {
    const MSG: &str = "can only operate on two boolean values, or two boolean GTS, or a list of booleans or macros, each macro putting a single boolean on top of the stack.";
    match stack.pop()? {
        Value::List(list) => {
            let operands: Vec<Value> = list.borrow().clone();
            for operand in operands {
                let value = match operand {
                    Value::Macro(m) => {
                        stack.exec_macro(&m)?;
                        stack.pop()?
                    }
                    other => other,
                };
                match value {
                    Value::Boolean(b) => {
                        if b == trigger {
                            stack.push(trigger);
                            return Ok(());
                        }
                    }
                    _ => return Err(fail(name, MSG)),
                }
            }
            stack.push(!trigger);
            Ok(())
        }
        op2 => {
            let op1 = stack.pop()?;
            let op: fn(bool, bool) -> bool = if trigger {
                |a, b| a || b
            } else {
                |a, b| a && b
            };
            match (op1, op2) {
                (Value::Boolean(a), Value::Boolean(b)) => {
                    stack.push(op(a, b));
                    Ok(())
                }
                (Value::Gts(a), Value::Gts(b)) => {
                    stack.push(gts_operators::logical(name, &a, &b, op)?);
                    Ok(())
                }
                _ => Err(fail(name, MSG)),
            }
        }
    }
}

fn and(stack: &mut Stack, name: &'static str) -> Result<()> {
    short_circuit(stack, name, false)
}

fn or(stack: &mut Stack, name: &'static str) -> Result<()> {
    short_circuit(stack, name, true)
}

fn not(stack: &mut Stack, name: &'static str) -> Result<()> {
    match stack.pop()? {
        Value::Boolean(b) => {
            stack.push(!b);
            Ok(())
        }
        Value::Gts(gts) => {
            let result = gts_operators::not(name, &gts.borrow())?;
            stack.push(result);
            Ok(())
        }
        _ => Err(fail(
            name,
            "can only operate on a boolean value or a boolean GTS.",
        )),
    }
}

// -----------------------------------------------------------------------------------------
// Bitwise
// -----------------------------------------------------------------------------------------

fn bitwise(stack: &mut Stack, name: &str, op: fn(i64, i64) -> i64) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    let result = match (&op1, &op2) {
        (Value::Long(a), Value::Long(b)) => Some(Value::Long(op(*a, *b))),
        (a, b) if a.is_number() && b.is_number() && has_bd(a, b) => {
            let (x, y) = (to_big_integer(name, a)?, to_big_integer(name, b)?);
            let shift = || {
                use num_traits::ToPrimitive;
                y.to_i32()
                    .ok_or_else(|| crate::bigdecimal::arithmetic("BigInteger out of int range"))
            };
            let value = match name {
                "&" => &x & &y,
                "|" => &x | &y,
                "^" => &x ^ &y,
                "<<" => crate::bigdecimal::shift_left(&x, shift()?),
                _ => crate::bigdecimal::shift_left(&x, shift()?.wrapping_neg()),
            };
            Some(bd_value(BigDecimal::from_bigint(value)))
        }
        _ => gts_operators::bitwise(&op1, &op2, op),
    };
    match result {
        Some(result) => {
            stack.push(result);
            Ok(())
        }
        None => Err(fail(
            name,
            "can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
        )),
    }
}

fn bitwise_and(stack: &mut Stack) -> Result<()> {
    bitwise(stack, "&", |a, b| a & b)
}

fn bitwise_or(stack: &mut Stack) -> Result<()> {
    bitwise(stack, "|", |a, b| a | b)
}

fn bitwise_xor(stack: &mut Stack) -> Result<()> {
    bitwise(stack, "^", |a, b| a ^ b)
}

fn shiftleft(stack: &mut Stack) -> Result<()> {
    bitwise(stack, "<<", |a, b| a.wrapping_shl(b as u32))
}

fn shiftright_signed(stack: &mut Stack) -> Result<()> {
    bitwise(stack, ">>", |a, b| a.wrapping_shr(b as u32))
}

fn shiftright_unsigned(stack: &mut Stack) -> Result<()> {
    bitwise(stack, ">>>", |a, b| {
        ((a as u64).wrapping_shr(b as u32)) as i64
    })
}

fn complement(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::Long(v) => {
            stack.push(!v);
            Ok(())
        }
        _ => Err(fail("~", "can only operate on a LONG.")),
    }
}

fn reversebits(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::Long(v) => {
            stack.push(v.reverse_bits());
            Ok(())
        }
        _ => Err(fail("REVBITS", "can only operate on a LONG.")),
    }
}

// -----------------------------------------------------------------------------------------
// Conversions
// -----------------------------------------------------------------------------------------

fn todouble(stack: &mut Stack) -> Result<()> {
    let value = match stack.pop()? {
        ref n if n.is_number() => Value::Double(n.as_f64().unwrap_or(f64::NAN)),
        Value::Boolean(b) => Value::Double(if b { 1.0 } else { 0.0 }),
        Value::String(s) => Value::Double(parse_java_double(&s)?),
        _ => {
            return Err(fail(
                "TODOUBLE",
                "can only operate on numeric, boolean or string values.",
            ));
        }
    };
    stack.push(value);
    Ok(())
}

/// Java `Double.valueOf(String)` (`FloatingDecimal.readJavaFormatString`): the string is
/// trimmed of the characters up to `' '`, then holds an optional sign followed by `NaN`,
/// `Infinity`, a decimal number or a hexadecimal one (`0x1.8p3`), the numbers optionally
/// followed by one of the type suffixes `f`, `F`, `d`, `D`. The Rust-only forms `inf`, `nan`…
/// are rejected.
pub(crate) fn parse_java_double(s: &str) -> Result<f64> {
    let trimmed = s.trim_matches(|c: char| c <= ' ');
    if trimmed.is_empty() {
        return Err(Error::new("empty String"));
    }
    let error = || Error::new(format!("For input string: \"{trimmed}\""));
    let (sign, rest) = match trimmed.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let magnitude = match rest {
        "NaN" => f64::NAN,
        "Infinity" => f64::INFINITY,
        other => {
            let body = other.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(other);
            if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
                parse_hex_double(hex).ok_or_else(error)?
            } else if body.starts_with(|c: char| c.is_ascii_digit() || c == '.')
                && body
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
            {
                // Within these characters Rust's grammar is Java's decimal one.
                body.parse::<f64>().map_err(|_| error())?
            } else {
                return Err(error());
            }
        }
    };
    Ok(sign * magnitude)
}

/// The magnitude of a Java hexadecimal floating-point literal after its `0x` prefix:
/// `HexDigits? (. HexDigits?)? [pP] [+-]? Digits` with at least one hexadecimal digit,
/// correctly rounded (to nearest, ties to even) to a `f64`.
fn parse_hex_double(body: &str) -> Option<f64> {
    // Exponents beyond this magnitude saturate: the result is 0 or infinite either way.
    const EXPONENT_LIMIT: i64 = 1 << 20;
    let (mantissa, exponent) = body.split_once(['p', 'P'])?;
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let (negative, digits) = match exponent.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, exponent.strip_prefix('+').unwrap_or(exponent)),
    };
    if int.is_empty() && frac.is_empty() || digits.is_empty() {
        return None;
    }
    let mut e = 0i64;
    for b in digits.bytes() {
        let digit = char::from(b).to_digit(10)?;
        e = (e * 10 + i64::from(digit)).min(EXPONENT_LIMIT);
    }
    if negative {
        e = -e;
    }
    // The first 16 significant hexadecimal digits, the others only telling whether the
    // value lies above the truncated one.
    let mut m = 0u64;
    let mut sticky = false;
    let digits = int.bytes().map(|b| (b, false));
    for (b, fractional) in digits.chain(frac.bytes().map(|b| (b, true))) {
        let digit = u64::from(char::from(b).to_digit(16)?);
        if m >> 60 == 0 {
            m = m << 4 | digit;
            e -= i64::from(fractional) * 4;
        } else {
            sticky |= digit != 0;
            e += i64::from(!fractional) * 4;
        }
    }
    Some(crate::jmath::binary_to_f64(m, e, sticky))
}

fn tolong(stack: &mut Stack) -> Result<()> {
    let value = match stack.pop()? {
        ref n if n.is_number() => Value::Long(n.as_i64().unwrap_or(0)),
        Value::Boolean(b) => Value::Long(i64::from(b)),
        Value::String(s) => Value::Long(long_decode(s.trim())?),
        Value::Bytes(bytes) => Value::Long(bytes_to_long("TOLONG", &bytes)?),
        _ => {
            return Err(fail(
                "TOLONG",
                "can only operate on numeric, boolean, byte array or string values.",
            ));
        }
    };
    stack.push(value);
    Ok(())
}

/// Java `Long.decode`: optional sign, then `0x`/`0X`/`#` for hex, leading `0` for octal, else
/// decimal.
fn long_decode(s: &str) -> Result<i64> {
    let error = || Error::new(format!("For input string: \"{s}\""));
    let (negative, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let (radix, digits) =
        if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
            (16, hex)
        } else if let Some(hex) = body.strip_prefix('#') {
            (16, hex)
        } else if body.len() > 1 && body.starts_with('0') {
            (8, &body[1..])
        } else {
            (10, body)
        };
    if digits.is_empty() {
        return Err(error());
    }
    let magnitude = i64::from_str_radix(digits, radix).map_err(|_| error())?;
    Ok(if negative { -magnitude } else { magnitude })
}

/// Java `TOLONG` on a 1..8 byte big-endian array, sign-extended.
fn bytes_to_long(name: &str, bytes: &[u8]) -> Result<i64> {
    if bytes.is_empty() || bytes.len() > 8 {
        return Err(fail(
            name,
            "can only operate on byte arrays from 1 to 8 bytes.",
        ));
    }
    let mut v: i64 = 0;
    for &b in bytes {
        v = (v << 8) | i64::from(b);
    }
    let shift = 64 - 8 * bytes.len() as u32;
    Ok((v << shift) >> shift)
}

fn toboolean(stack: &mut Stack) -> Result<()> {
    let value = match stack.pop()? {
        ref n if n.is_number() => Value::Boolean(n.as_i64().unwrap_or(0) != 0),
        b @ Value::Boolean(_) => b,
        Value::String(s) => Value::Boolean(!s.is_empty()),
        _ => {
            return Err(fail(
                "TOBOOLEAN",
                "can only operate on numeric, boolean or string values.",
            ));
        }
    };
    stack.push(value);
    Ok(())
}

fn tostring(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        // A counter is a `Number` (`AtomicLong`) in Java.
        value @ (Value::Long(_)
        | Value::Double(_)
        | Value::BigDecimal(_)
        | Value::Boolean(_)
        | Value::String(_)
        | Value::Counter(_)) => {
            stack.push(Value::from(value.to_string()));
            Ok(())
        }
        // `GeoTimeSerie.dump`: the GTS input format.
        Value::Gts(gts) => {
            stack.push(Value::from(gts.borrow().dump()));
            Ok(())
        }
        _ => Err(fail(
            "TOSTRING",
            "can only operate on numeric, boolean or string values.",
        )),
    }
}

fn tohex(stack: &mut Stack) -> Result<()> {
    to_hex(stack, "TOHEX")
}

fn tohex_arrow(stack: &mut Stack) -> Result<()> {
    to_hex(stack, "->HEX")
}

fn tobin(stack: &mut Stack) -> Result<()> {
    to_bin(stack, "TOBIN")
}

fn tobin_arrow(stack: &mut Stack) -> Result<()> {
    to_bin(stack, "->BIN")
}

/// `-0x…` / `0x…` (`-0b…` / `0b…`): the digits of an integral `BIGDECIMAL` in `radix`.
fn integral_radix(name: &str, bd: &BigDecimal, radix: u32, prefix: &str) -> Result<String> {
    let Some(integer) = bd.to_bigint_exact() else {
        return Err(fail(
            name,
            "can only operate on a BIGDECIMAL with no fractional part.",
        ));
    };
    let sign = if integer.sign() == num_bigint::Sign::Minus {
        "-"
    } else {
        ""
    };
    Ok(format!(
        "{sign}{prefix}{}",
        num_traits::Signed::abs(&integer).to_str_radix(radix)
    ))
}

/// `TOHEX` / `->HEX`: the hexadecimal form of a `STRING` (its UTF-8 bytes), of `BYTES`, or of
/// a `LONG` (two's complement, padded to 16 digits).
/// An integral `BIGDECIMAL` is written with its sign and a `0x` prefix.
fn to_hex(stack: &mut Stack, name: &str) -> Result<()> {
    let text = match stack.pop()? {
        Value::String(s) => crate::encoding::hex_encode(s.as_bytes()),
        Value::Bytes(b) => crate::encoding::hex_encode(&b),
        Value::Long(v) => format!("{:016x}", v as u64),
        Value::BigDecimal(bd) => integral_radix(name, &bd, 16, "0x")?,
        _ => {
            return Err(fail(
                name,
                "operates on a STRING, BYTES, BIGDECIMAL or LONG.",
            ));
        }
    };
    stack.push(Value::from(text));
    Ok(())
}

/// `TOBIN` / `->BIN`: the binary form of a `STRING` (its UTF-8 bytes), of `BYTES`, of a `LONG`
/// (two's complement, padded to 64 digits) or of a `BITSET` (one digit per bit below
/// `length()`).
fn to_bin(stack: &mut Stack, name: &str) -> Result<()> {
    let text = match stack.pop()? {
        Value::String(s) => bytes_to_bin(s.as_bytes()),
        Value::Bytes(b) => bytes_to_bin(&b),
        Value::Long(v) => format!("{:064b}", v as u64),
        Value::BigDecimal(bd) => integral_radix(name, &bd, 2, "0b")?,
        Value::BitSet(bits) => {
            let length = usize::try_from(bits.length()).unwrap_or_default();
            let set: std::collections::BTreeSet<usize> = bits.set_bits().collect();
            (0..length)
                .map(|i| if set.contains(&i) { '1' } else { '0' })
                .collect()
        }
        _ => {
            return Err(fail(
                name,
                "operates on a STRING, BYTES, LONG, BIGDECIMAL or BITSET.",
            ));
        }
    };
    stack.push(Value::from(text));
    Ok(())
}

/// Eight binary digits per byte, most significant bit first.
fn bytes_to_bin(data: &[u8]) -> String {
    use std::fmt::Write;
    data.iter().fold(String::new(), |mut text, b| {
        let _ = write!(text, "{b:08b}");
        text
    })
}

fn fromhex(stack: &mut Stack) -> Result<()> {
    from_radix(stack, "FROMHEX", 16, 16)
}

fn frombin(stack: &mut Stack) -> Result<()> {
    from_radix(stack, "FROMBIN", 2, 64)
}

/// `FROMHEX` / `FROMBIN`: parse a string of up to `max_digits` digits into a `LONG`, wrapping
/// like `new BigInteger(s, radix).longValue()`.
fn from_radix(stack: &mut Stack, name: &str, radix: u32, max_digits: usize) -> Result<()> {
    match stack.pop()? {
        Value::String(s) => {
            if s.len() > max_digits {
                let unit = if radix == 16 { "hexadecimal" } else { "binary" };
                return Err(fail(
                    name,
                    &format!("can only operate on {unit} representations of 64 bits or less."),
                ));
            }
            let value = crate::format::parse_radix_wrapping(&s, radix)?;
            stack.push(value);
            Ok(())
        }
        _ => {
            let unit = if radix == 16 { "hexadecimal" } else { "binary" };
            Err(fail(
                name,
                &format!("can only operate on string of {unit} representations."),
            ))
        }
    }
}

/// `TOBITS`: the IEEE 754 bits of a number (`Double.doubleToRawLongBits` of `doubleValue()`,
/// or the unsigned `Float.floatToRawIntBits` of `floatValue()`), or of each value of a series.
fn to_bits(stack: &mut Stack, name: &str, convert: fn(&Value) -> i64) -> Result<()> {
    let result = match stack.pop()? {
        ref n if n.is_number() => Value::Long(convert(n)),
        Value::Gts(gts) => gts_operators::to_bits(name, &gts.borrow(), convert)?,
        _ => {
            return Err(fail(
                name,
                "operates on a DOUBLE, LONG or Geo Time Series thereof.",
            ));
        }
    };
    stack.push(result);
    Ok(())
}

/// `FROMBITS`: the number of IEEE 754 bits, or of each value of a `LONG` series.
fn from_bits(stack: &mut Stack, name: &str, convert: fn(i64) -> f64) -> Result<()> {
    let result = match stack.pop()? {
        Value::Long(bits) => Value::Double(convert(bits)),
        Value::Gts(gts) => gts_operators::from_bits(name, &gts.borrow(), convert)?,
        _ => {
            return Err(fail(
                name,
                "operates on a LONG or a Geo Time Series thereof.",
            ));
        }
    };
    stack.push(result);
    Ok(())
}

fn to_double_bits(stack: &mut Stack) -> Result<()> {
    to_bits(stack, "->DOUBLEBITS", |n| {
        n.as_f64().unwrap_or(f64::NAN).to_bits() as i64
    })
}

fn from_double_bits(stack: &mut Stack) -> Result<()> {
    from_bits(stack, "DOUBLEBITS->", |bits| f64::from_bits(bits as u64))
}

fn to_float_bits(stack: &mut Stack) -> Result<()> {
    to_bits(stack, "->FLOATBITS", |n| {
        // `Number.floatValue()`: a `LONG` is rounded to a float directly.
        let float = match n {
            Value::Long(l) => *l as f32,
            _ => n.as_f64().unwrap_or(f64::NAN) as f32,
        };
        i64::from(float.to_bits())
    })
}

fn from_float_bits(stack: &mut Stack) -> Result<()> {
    from_bits(stack, "FLOATBITS->", |bits| {
        f64::from(f32::from_bits((bits & 0xFFFF_FFFF) as u32))
    })
}

// -----------------------------------------------------------------------------------------
// Time units
// -----------------------------------------------------------------------------------------

/// `UNIT`: `value * factor_ms * TIME_UNITS_PER_MS`, rounded (Java `Math.round`, i.e. round half
/// up), where `factor_ms` is how many milliseconds the unit is worth.
fn unit(stack: &mut Stack, name: &str, factor_ms: f64) -> Result<()> {
    match stack.pop()? {
        ref n if n.is_number() => {
            let ms = n.as_f64().unwrap_or(f64::NAN) * factor_ms * TIME_UNITS_PER_MS;
            stack.push(jmath::round(ms));
            Ok(())
        }
        _ => Err(fail(name, "can only operate on numeric values.")),
    }
}

fn unit_week(stack: &mut Stack) -> Result<()> {
    unit(stack, "w", 7.0 * 24.0 * 60.0 * 60.0 * 1000.0)
}

fn unit_day(stack: &mut Stack) -> Result<()> {
    unit(stack, "d", 24.0 * 60.0 * 60.0 * 1000.0)
}

fn unit_hour(stack: &mut Stack) -> Result<()> {
    unit(stack, "h", 60.0 * 60.0 * 1000.0)
}

fn unit_minute(stack: &mut Stack) -> Result<()> {
    unit(stack, "m", 60.0 * 1000.0)
}

fn unit_second(stack: &mut Stack) -> Result<()> {
    unit(stack, "s", 1000.0)
}

fn unit_milli(stack: &mut Stack) -> Result<()> {
    unit(stack, "ms", 1.0)
}

fn unit_micro(stack: &mut Stack) -> Result<()> {
    unit(stack, "us", 0.001)
}

fn unit_nano(stack: &mut Stack) -> Result<()> {
    unit(stack, "ns", 0.000_001)
}

fn unit_pico(stack: &mut Stack) -> Result<()> {
    unit(stack, "ps", 0.000_000_001)
}
