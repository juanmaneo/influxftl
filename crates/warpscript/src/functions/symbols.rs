// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Symbols, registers and (re)definitions: `STORE`, `LOAD`, `DEF`, `RUN`, `EVAL`, `POPRn`…

use std::rc::Rc;

use super::Entry;
use super::args::{fail, pop_string};
use crate::error::{Error, Result};
use crate::macros::StackFunction;
use crate::stack::Stack;
use crate::value::Value;

/// Stack attribute set while `XEVAL` executes a string (`ATTRIBUTE_IN_XEVAL`).
pub const ATTRIBUTE_IN_XEVAL: &str = "in.xeval";

macro_rules! register_functions {
    ($($n:literal),* $(,)?) => {
        /// The functions of this module.
        pub const FUNCTIONS: &[(&str, Entry)] = &[
            ("STORE", Entry::Stack(store)),
            ("CSTORE", Entry::Stack(cstore)),
            ("LOAD", Entry::Stack(load)),
            ("DEF", Entry::Stack(def)),
            ("RUN", Entry::Stack(run)),
            ("EVAL", Entry::Stack(eval)),
            ("XEVAL", Entry::Stack(xeval)),
            ("FORGET", Entry::Stack(forget)),
            ("DEFINED", Entry::Stack(defined)),
            ("DEFINEDMACRO", Entry::Stack(definedmacro)),
            ("CHECKMACRO", Entry::Stack(checkmacro)),
            ("CLEARDEFS", Entry::Stack(cleardefs)),
            ("CLEARSYMBOLS", Entry::Stack(clearsymbols)),
            ("CLEARREGS", Entry::Stack(clearregs)),
            $( (concat!("POPR", $n), Entry::Stack(popr::<$n>)), )*
            $( (concat!("CPOPR", $n), Entry::Stack(cpopr::<$n>)), )*
            $( (concat!("PUSHR", $n), Entry::Stack(pushr::<$n>)), )*
            $( (concat!("RUNR", $n), Entry::Stack(runr::<$n>)), )*
        ];
    };
}

register_functions!(
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
    50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73,
    74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97,
    98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116,
    117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135,
    136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154,
    155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173,
    174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192,
    193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211,
    212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230,
    231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249,
    250, 251, 252, 253, 254, 255,
);

const STORE_MESSAGE: &str =
    "expects a variable name or register number or a list thereof on top of the stack.";

/// Java `((Long) l).intValue()`.
fn register_index(l: i64) -> i64 {
    i64::from(l as i32)
}

fn store(stack: &mut Stack) -> Result<()> {
    let var = stack.pop()?;
    match var {
        Value::String(name) => {
            let value = stack.pop()?;
            stack.store(&name, value)
        }
        Value::Long(index) => {
            let value = stack.pop()?;
            stack.store_register(register_index(index), value)
        }
        Value::List(names) => {
            let names: Vec<Value> = names.borrow().clone();
            if names
                .iter()
                .any(|n| !matches!(n, Value::Null | Value::String(_) | Value::Long(_)))
            {
                return Err(fail("STORE", STORE_MESSAGE));
            }
            let count = names.len();
            if stack.depth() < count {
                return Err(Error::usage(
                    "STORE",
                    format!(
                        "expects {count} elements on the stack, only found {}",
                        stack.depth()
                    ),
                ));
            }
            // Iterate from first to last symbol so that `1 2 3 [ 'a' 'b' 'b' ] STORE $b`
            // pushes 3.
            let first = stack.depth() - count;
            for (i, symbol) in names.iter().enumerate() {
                let value = stack.elements()[first + i].clone();
                match symbol {
                    Value::Null => {}
                    Value::Long(index) => stack.store_register(register_index(*index), value)?,
                    other => stack.store(&other.to_string(), value)?,
                }
            }
            stack.dropn(count)
        }
        _ => Err(fail("STORE", STORE_MESSAGE)),
    }
}

fn cstore(stack: &mut Stack) -> Result<()> {
    let var = stack.pop()?;
    match var {
        Value::String(name) => {
            let value = stack.pop()?;
            if !stack.has_symbol(&name) {
                stack.store(&name, value)?;
            }
            Ok(())
        }
        Value::Long(index) => {
            let value = stack.pop()?;
            let index = register_index(index);
            if stack.load_register(index)?.is_null() {
                // The register index was just validated by `load_register`.
                let _ = stack.store_register(index, value);
            }
            Ok(())
        }
        _ => Err(fail(
            "CSTORE",
            "expects variable name to be a string or a register number.",
        )),
    }
}

/// `LOAD`: push the value of a symbol or register (also emitted for `$symbol` inside macros).
pub(crate) fn load(stack: &mut Stack) -> Result<()> {
    let var = stack.pop()?;
    let value = match &var {
        Value::String(name) => match stack.load(name) {
            Some(value) => value.clone(),
            None => {
                return Err(Error::usage(
                    "LOAD",
                    format!("symbol '{name}' does not exist."),
                ));
            }
        },
        Value::Long(index) => stack.load_register(register_index(*index))?.clone(),
        _ => {
            return Err(fail(
                "LOAD",
                "expects a variable name (STRING) or a register number (LONG) on top of the stack.",
            ));
        }
    };
    stack.push(value);
    Ok(())
}

fn def(stack: &mut Stack) -> Result<()> {
    let name = pop_string(stack, "DEF", "expects statement name to be a string.")?;
    let body = match stack.pop()? {
        Value::Null => None,
        Value::Macro(m) => Some(m),
        _ => return Err(fail("DEF", "needs a macro below the statement name.")),
    };
    stack.define(&name, body);
    Ok(())
}

/// `RUN`: execute the macro bound to a symbol or register (also emitted for `@symbol` inside
/// macros).
pub(crate) fn run(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::String(name) => stack.run(&name),
        Value::Long(index) => run_register(stack, register_index(index), false),
        _ => Err(fail(
            "RUN",
            "expects macro name to be a STRING or a register number (LONG).",
        )),
    }
}

/// Execute the macro held in register `index` (`RUN` with a `LONG`, or `RUNRn` when
/// `dedicated` is set, which only changes the function name in the error message).
fn run_register(stack: &mut Stack, index: i64, dedicated: bool) -> Result<()> {
    match stack.load_register(index)? {
        Value::Macro(m) => {
            let m = Rc::clone(m);
            stack.exec_macro(&m)
        }
        _ => {
            let name = if dedicated {
                format!("RUNR{index}")
            } else {
                "RUN".to_owned()
            };
            Err(Error::usage(
                name,
                format!("expects register number {index} to contain a macro."),
            ))
        }
    }
}

pub(crate) fn eval(stack: &mut Stack) -> Result<()> {
    let top = stack.pop()?;
    if let Some(callable) = StackFunction::of(&top) {
        return callable.clone().apply(stack);
    }
    match top {
        Value::String(script) => stack.exec_multi(&script),
        Value::Macro(m) => stack.exec_macro(&m),
        _ => Err(fail("EVAL", "expects a Macro, a function or a String.")),
    }
}

/// `XEVAL`: like `EVAL` but restricted to strings; the `in.xeval` attribute is set while the
/// string executes.
fn xeval(stack: &mut Stack) -> Result<()> {
    let already_set = matches!(
        stack.attribute(ATTRIBUTE_IN_XEVAL),
        Some(Value::Boolean(true))
    );
    let result = match stack.pop()? {
        Value::String(script) => {
            stack.set_attribute(ATTRIBUTE_IN_XEVAL, Some(Value::Boolean(true)));
            stack.exec_multi(&script)
        }
        Value::Macro(_) => Err(fail("XEVAL", "can only be applied to a STRING.")),
        _ => Err(fail("XEVAL", "expects a Macro, a function or a String.")),
    };
    if !already_set {
        stack.set_attribute(ATTRIBUTE_IN_XEVAL, None);
    }
    result
}

fn forget(stack: &mut Stack) -> Result<()> {
    let name = pop_string(stack, "FORGET", "expects variable name to be a string.")?;
    stack.forget(&name);
    Ok(())
}

fn defined(stack: &mut Stack) -> Result<()> {
    let name = pop_string(stack, "DEFINED", "expects a string on top of the stack.")?;
    let defined = stack.has_symbol(&name);
    stack.push(defined);
    Ok(())
}

fn definedmacro(stack: &mut Stack) -> Result<()> {
    let name = pop_string(
        stack,
        "DEFINEDMACRO",
        "expects a string on top of the stack.",
    )?;
    let found = stack.find_macro(&name).is_ok();
    stack.push(found);
    Ok(())
}

fn checkmacro(stack: &mut Stack) -> Result<()> {
    let name = pop_string(stack, "CHECKMACRO", "expects a string on top of the stack.")?;
    if stack.find_macro(&name).is_err() {
        return Err(Error::usage(
            "CHECKMACRO",
            format!("macro '{name}' could not be loaded."),
        ));
    }
    Ok(())
}

fn cleardefs(stack: &mut Stack) -> Result<()> {
    let names: Vec<String> = stack.defined().keys().cloned().collect();
    for name in names {
        stack.define(&name, None);
    }
    Ok(())
}

fn clearsymbols(stack: &mut Stack) -> Result<()> {
    stack.forget_all();
    Ok(())
}

fn clearregs(stack: &mut Stack) -> Result<()> {
    stack.clear_registers();
    Ok(())
}

fn popr<const N: i64>(stack: &mut Stack) -> Result<()> {
    let top = stack.pop()?;
    stack.store_register(N, top)
}

fn cpopr<const N: i64>(stack: &mut Stack) -> Result<()> {
    let top = stack.pop()?;
    // `N` is always a valid register number.
    if stack.registers()[N as usize].is_null() {
        return stack.store_register(N, top);
    }
    Ok(())
}

fn pushr<const N: i64>(stack: &mut Stack) -> Result<()> {
    let value = stack.registers()[N as usize].clone();
    stack.push(value);
    Ok(())
}

fn runr<const N: i64>(stack: &mut Stack) -> Result<()> {
    run_register(stack, N, true)
}
