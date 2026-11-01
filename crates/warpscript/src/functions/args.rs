//! Helpers shared by the function implementations: typed argument popping with Warp 10 style
//! error messages (`<FUNCTION> <message>`).

use std::rc::Rc;

use crate::error::{Error, Result};
use crate::macros::Macro;
use crate::stack::Stack;
use crate::value::{List, Map, Value};

/// An error whose message is `"<name> <message>"`, the usual WarpLib wording.
pub fn fail(name: &str, message: &str) -> Error {
    Error::new(format!("{name} {message}"))
}

/// [`fail`] with a cause (`new WarpScriptException(getName() + " message", cause)`).
pub fn fail_with_cause(name: &str, message: &str, cause: Error) -> Error {
    Error::with_cause(format!("{name} {message}"), cause)
}

/// Pop a `MACRO`.
pub fn pop_macro(stack: &mut Stack, name: &str, message: &str) -> Result<Rc<Macro>> {
    match stack.pop()? {
        Value::Macro(m) => Ok(m),
        _ => Err(fail(name, message)),
    }
}

/// Pop a `STRING`.
pub fn pop_string(stack: &mut Stack, name: &str, message: &str) -> Result<Rc<str>> {
    match stack.pop()? {
        Value::String(s) => Ok(s),
        _ => Err(fail(name, message)),
    }
}

/// Pop a `LONG`.
pub fn pop_long(stack: &mut Stack, name: &str, message: &str) -> Result<i64> {
    match stack.pop()? {
        Value::Long(l) => Ok(l),
        _ => Err(fail(name, message)),
    }
}

/// Pop a `BOOLEAN`.
pub fn pop_bool(stack: &mut Stack, name: &str, message: &str) -> Result<bool> {
    match stack.pop()? {
        Value::Boolean(b) => Ok(b),
        _ => Err(fail(name, message)),
    }
}

/// Pop a `LIST`.
pub fn pop_list(stack: &mut Stack, name: &str, message: &str) -> Result<List> {
    match stack.pop()? {
        Value::List(l) => Ok(l),
        _ => Err(fail(name, message)),
    }
}

/// Pop a `MAP`.
pub fn pop_map(stack: &mut Stack, name: &str, message: &str) -> Result<Map> {
    match stack.pop()? {
        Value::Map(m) => Ok(m),
        _ => Err(fail(name, message)),
    }
}

/// Pop a number (`LONG` or `DOUBLE`), returned as is.
pub fn pop_number(stack: &mut Stack, name: &str, message: &str) -> Result<Value> {
    let value = stack.pop()?;
    if value.is_number() {
        Ok(value)
    } else {
        Err(fail(name, message))
    }
}

/// Java `Number.intValue()`: a `long` keeps its low 32 bits, a `double` saturates.
pub fn int_value(value: &Value) -> Option<i32> {
    match value {
        Value::Long(l) => Some(*l as i32),
        Value::Double(d) => Some(*d as i32),
        _ => None,
    }
}

/// `GET.computeAndCheckIndex`: resolve a possibly negative index (counted from the end) into
/// `0..size`, with the Java error messages when it is out of bounds.
pub fn compute_and_check_index(index: i32, size: i32) -> Result<i32> {
    let mut index = index;
    if index < 0 {
        index = index.wrapping_add(size);
    } else if index >= size {
        return Err(Error::new(format!("Index out of bound, {index} >= {size}")));
    }
    if index < 0 {
        return Err(Error::new(format!(
            "Index out of bound, {} < -{size}",
            index.wrapping_sub(size)
        )));
    }
    Ok(index)
}

/// `MemoryWarpScriptStack.getn`: pop the top of the stack as a count of elements, checked
/// against the stack depth like every caller does (`DROPN`, `DUPN`, `ROLL`, `->LIST`…).
pub fn getn(stack: &mut Stack) -> Result<usize> {
    let top = stack.pop()?;
    let Some(n) = int_value(&top) else {
        return Err(Error::new("Unexpected type, expecting a numerical value."));
    };
    if n < 0 || stack.depth() < n as usize {
        return Err(Error::new("Index out of bound."));
    }
    Ok(n as usize)
}
