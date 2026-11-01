//! Arithmetic, comparison, bitwise and logical operators, and the scalar type conversions.
//!
//! The Geo Time Series, matrix, vector and `BIGDECIMAL` operand types of Warp 10 are not
//! modelled yet; the operators implement the scalar (and, where relevant, list / set / macro /
//! bytes) behaviour faithfully and raise the Warp 10 error message for the other operands.

use std::cmp::Ordering;
use std::rc::Rc;

use super::Entry;
use super::args::fail;
use crate::error::{Error, Result};
use crate::macros::Macro;
use crate::stack::Stack;
use crate::value::Value;

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
    ("&&", Entry::Stack(and)),
    ("AND", Entry::Stack(and)),
    ("||", Entry::Stack(or)),
    ("OR", Entry::Stack(or)),
    ("!", Entry::Stack(not)),
    ("NOT", Entry::Stack(not)),
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
    ("->HEX", Entry::Stack(tohex)),
    ("FROMHEX", Entry::Stack(fromhex)),
    ("TOBIN", Entry::Stack(tobin)),
    ("->BIN", Entry::Stack(tobin)),
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
        _ => {
            return Err(fail(
                "+",
                "can only operate on numeric, string, lists, matrices, vectors, Geo Time Series, byte array and macro values.",
            ));
        }
    };
    stack.push(result);
    Ok(())
}

const SUB_MSG: &str =
    "can only operate on numeric values, vectors, matrices and numeric Geo Time Series.";
const MUL_MSG: &str =
    "can only operate on numeric values, vectors, matrices and numeric Geo Time Series.";
const DIV_MSG: &str = "can only operate on numeric values, vectors and numeric Geo Time Series.";

fn sub(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    if op1.is_number() && op2.is_number() {
        stack.push(numeric(&op1, &op2, i64::wrapping_sub, |x, y| x - y));
        Ok(())
    } else {
        Err(fail("-", SUB_MSG))
    }
}

fn mul(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    if op1.is_number() && op2.is_number() {
        stack.push(numeric(&op1, &op2, i64::wrapping_mul, |x, y| x * y));
        Ok(())
    } else {
        Err(fail("*", MUL_MSG))
    }
}

fn div(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    match (&op1, &op2) {
        (Value::Long(_), Value::Long(0)) => Err(Error::new("/ by zero")),
        (a, b) if a.is_number() && b.is_number() => {
            stack.push(numeric(a, b, i64::wrapping_div, |x, y| x / y));
            Ok(())
        }
        _ => Err(fail("/", DIV_MSG)),
    }
}

fn modulo(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    match (&op1, &op2) {
        _ if !op1.is_number() || !op2.is_number() => {
            Err(fail("%", "operates on numeric arguments."))
        }
        (Value::Long(_), Value::Long(0)) => Err(Error::new("/ by zero")),
        (a, b) => {
            stack.push(numeric(a, b, i64::wrapping_rem, |x, y| x % y));
            Ok(())
        }
    }
}

fn pow(stack: &mut Stack) -> Result<()> {
    numerical_binary(
        stack,
        "**",
        |a, b| crate::value::double_to_long((a as f64).powf(b as f64)),
        f64::powf,
    )
}

/// Port of `NumericalBinaryFunction` (the `applyOnSingleList = false` form used by `**`):
/// two numbers, a number on top of a list, or a list on top of a single number operand.
pub fn numerical_binary(
    stack: &mut Stack,
    name: &str,
    on_long: fn(i64, i64) -> i64,
    on_double: fn(f64, f64) -> f64,
) -> Result<()> {
    let message = format!(
        "{name} can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values."
    );
    let op0 = stack.pop()?;
    match op0 {
        ref top if top.is_number() => {
            let op1 = stack.pop()?;
            match op1 {
                ref n if n.is_number() => {
                    stack.push(combine(n, top, on_long, on_double));
                    Ok(())
                }
                Value::List(list) => {
                    let out = map_list(&list.borrow(), top, true, on_long, on_double, &message)?;
                    stack.push(Value::list(out));
                    Ok(())
                }
                _ => Err(Error::new(message)),
            }
        }
        Value::List(list) => {
            let op1 = stack.pop()?;
            if !op1.is_number() {
                return Err(Error::new(message));
            }
            let out = map_list(&list.borrow(), &op1, false, on_long, on_double, &message)?;
            stack.push(Value::list(out));
            Ok(())
        }
        _ => Err(Error::new(message)),
    }
}

/// `op(a, b)` with Java promotion (double when either is a double).
fn combine(
    a: &Value,
    b: &Value,
    on_long: fn(i64, i64) -> i64,
    on_double: fn(f64, f64) -> f64,
) -> Value {
    match (a, b) {
        (Value::Long(x), Value::Long(y)) => Value::Long(on_long(*x, *y)),
        _ => Value::Double(on_double(
            a.as_f64().unwrap_or(f64::NAN),
            b.as_f64().unwrap_or(f64::NAN),
        )),
    }
}

/// Apply the operator between each element of `list` and `operand`. When `element_first`, the
/// element is the left-hand side (numeric-on-top, list-under case); otherwise the operand is.
fn map_list(
    list: &[Value],
    operand: &Value,
    element_first: bool,
    on_long: fn(i64, i64) -> i64,
    on_double: fn(f64, f64) -> f64,
    message: &str,
) -> Result<Vec<Value>> {
    let mut out = Vec::with_capacity(list.len());
    for element in list {
        if !element.is_number() {
            return Err(Error::new(message.to_owned()));
        }
        let value = if element_first {
            combine(element, operand, on_long, on_double)
        } else {
            combine(operand, element, on_long, on_double)
        };
        out.push(value);
    }
    Ok(out)
}

fn inplace_add(stack: &mut Stack) -> Result<()> {
    let op2 = stack.pop()?;
    let op1 = stack.pop()?;
    match &op1 {
        Value::List(list) => {
            list.borrow_mut().push(op2);
            stack.push(op1);
            Ok(())
        }
        Value::Set(set) => {
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
fn compare_long_double(l: i64, d: f64) -> Ordering {
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
        (Value::String(x), Value::String(y)) => x.as_ref().cmp(y.as_ref()),
        (Value::Bytes(x), Value::Bytes(y)) => x.as_ref().cmp(y.as_ref()),
        _ => return None,
    })
}

/// The ordering comparison shared by `<`, `>`, `<=`, `>=` (and the numeric part of `==`/`!=`).
fn comparison(
    stack: &mut Stack,
    op1: &Value,
    op2: &Value,
    one_nan: bool,
    two_nan: bool,
    operator: fn(Ordering) -> bool,
    name: &str,
) -> Result<()> {
    let nan1 = matches!(op1, Value::Double(d) if d.is_nan());
    let nan2 = matches!(op2, Value::Double(d) if d.is_nan());
    let result = if nan1 && nan2 {
        two_nan
    } else if nan1 || nan2 {
        one_nan
    } else if let Some(ordering) = compare_values(op1, op2) {
        operator(ordering)
    } else {
        return Err(fail(
            name,
            "can only operate when GTS values and the top stack operand have the same type.",
        ));
    };
    stack.push(result);
    Ok(())
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
    if op1.is_number() && op2.is_number() {
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
    if op1.is_number() && op2.is_number() {
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
            match (op1, op2) {
                (Value::Boolean(a), Value::Boolean(b)) => {
                    // `&&` is `a && b`, `||` is `a || b`; both equal `trigger == (a ? ... )`.
                    let result = if trigger { a || b } else { a && b };
                    stack.push(result);
                    Ok(())
                }
                _ => Err(fail(name, MSG)),
            }
        }
    }
}

fn and(stack: &mut Stack) -> Result<()> {
    short_circuit(stack, "AND", false)
}

fn or(stack: &mut Stack) -> Result<()> {
    short_circuit(stack, "OR", true)
}

fn not(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::Boolean(b) => {
            stack.push(!b);
            Ok(())
        }
        _ => Err(fail(
            "!",
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
    match (op1, op2) {
        (Value::Long(a), Value::Long(b)) => {
            stack.push(op(a, b));
            Ok(())
        }
        _ => Err(fail(
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

/// Java `Double.valueOf(String)` for the common cases: trims, accepts `NaN` / `Infinity` and a
/// leading sign, delegates the magnitude to Rust's `f64` parser.
fn parse_java_double(s: &str) -> Result<f64> {
    let trimmed = s.trim();
    let (sign, rest) = match trimmed.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let magnitude = match rest {
        "NaN" => f64::NAN,
        "Infinity" => f64::INFINITY,
        other => other
            .parse::<f64>()
            .map_err(|_| Error::new(format!("For input string: \"{s}\"")))?,
    };
    Ok(sign * magnitude)
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
        value @ (Value::Long(_) | Value::Double(_) | Value::Boolean(_) | Value::String(_)) => {
            stack.push(Value::from(value.to_string()));
            Ok(())
        }
        _ => Err(fail(
            "TOSTRING",
            "can only operate on numeric, boolean or string values.",
        )),
    }
}

fn tohex(stack: &mut Stack) -> Result<()> {
    radix_string(stack, "TOHEX", 16)
}

fn tobin(stack: &mut Stack) -> Result<()> {
    radix_string(stack, "TOBIN", 2)
}

/// `TOHEX` / `TOBIN`: the two's complement representation of a `LONG` padded to 16 (hex) or 64
/// (binary) digits.
fn radix_string(stack: &mut Stack, name: &str, radix: u32) -> Result<()> {
    match stack.pop()? {
        Value::Long(v) => {
            let unsigned = v as u64;
            let text = match radix {
                16 => format!("{unsigned:016x}"),
                _ => format!("{unsigned:064b}"),
            };
            stack.push(Value::from(text));
            Ok(())
        }
        _ => Err(fail(name, "can only operate on long values.")),
    }
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

fn to_double_bits(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        ref n if n.is_number() => {
            stack.push(n.as_f64().unwrap_or(f64::NAN).to_bits() as i64);
            Ok(())
        }
        _ => Err(fail(
            "->DOUBLEBITS",
            "operates on a DOUBLE, LONG or Geo Time Series thereof.",
        )),
    }
}

fn from_double_bits(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::Long(bits) => {
            stack.push(f64::from_bits(bits as u64));
            Ok(())
        }
        _ => Err(fail(
            "DOUBLEBITS->",
            "operates on a LONG or a Geo Time Series thereof.",
        )),
    }
}

fn to_float_bits(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        ref n if n.is_number() => {
            let bits = (n.as_f64().unwrap_or(f64::NAN) as f32).to_bits();
            stack.push(i64::from(bits));
            Ok(())
        }
        _ => Err(fail(
            "->FLOATBITS",
            "operates on a DOUBLE, LONG or Geo Time Series thereof.",
        )),
    }
}

fn from_float_bits(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::Long(bits) => {
            stack.push(f64::from(f32::from_bits((bits & 0xFFFF_FFFF) as u32)));
            Ok(())
        }
        _ => Err(fail(
            "FLOATBITS->",
            "operates on a LONG or a Geo Time Series thereof.",
        )),
    }
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
            stack.push(java_round(ms));
            Ok(())
        }
        _ => Err(fail(name, "can only operate on numeric values.")),
    }
}

/// Java `Math.round(double)`: `floor(x + 0.5)`, returning a `long`.
fn java_round(x: f64) -> i64 {
    crate::value::double_to_long((x + 0.5).floor())
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
