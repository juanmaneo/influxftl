// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::functions::symbols::ATTRIBUTE_IN_XEVAL;
use crate::stack::Stack;
use crate::tests::{assert_fails, assert_stack, assert_top, exec};
use crate::value::Value;

fn long(l: i64) -> Value {
    Value::Long(l)
}

#[test]
fn store_and_load() {
    assert_top("1 'a' STORE $a", 1);
    assert_top("1 'a' STORE 'a' LOAD", 1);
    assert_top("NULL 'a' STORE 'a' LOAD", Value::Null);
    assert_top("1 5 STORE 5 LOAD", 1);
    assert_top("1 5 STORE PUSHR5", 1);
    assert_fails(
        "1 300 STORE",
        "Invalid register number, must be between 0 and 255",
    );
    assert_fails(
        "300 LOAD",
        "Invalid register number, must be between 0 and 255",
    );
    assert_fails("'zz' LOAD", "LOAD symbol 'zz' does not exist.");
    assert_fails(
        "1.0 LOAD",
        "LOAD expects a variable name (STRING) or a register number (LONG) on top of the stack.",
    );
    assert_fails(
        "1 1.0 STORE",
        "STORE expects a variable name or register number or a list thereof on top of the stack.",
    );
    assert_fails("'a' STORE", "Empty stack.");
    assert_stack("1 2 3 [ 'a' 'b' 'b' ] STORE $a $b", &[long(1), long(3)]);
    assert_stack("1 2 3 [ NULL 7 'c' ] STORE $c PUSHR7", &[long(3), long(2)]);
    assert_stack("1 2 [ ] STORE", &[long(1), long(2)]);
    assert_fails(
        "1 [ 'a' 'b' ] STORE",
        "STORE expects 2 elements on the stack, only found 1",
    );
    assert_fails(
        "1 [ 1.5 ] STORE",
        "STORE expects a variable name or register number or a list thereof on top of the stack.",
    );
    assert_fails("1 2 [ 'a' 300 ] STORE", "Invalid register number");
}

#[test]
fn missing_arguments() {
    for script in [
        "STORE",
        "5 STORE",
        "'a' STORE",
        "CSTORE",
        "'x' CSTORE",
        "5 CSTORE",
        "LOAD",
        "DEF",
        "'x' DEF",
        "RUN",
        "EVAL",
        "XEVAL",
        "FORGET",
        "DEFINED",
        "DEFINEDMACRO",
        "CHECKMACRO",
    ] {
        assert_fails(script, "Empty stack.");
    }
    assert_fails("300 RUN", "Invalid register number");
}

#[test]
fn symbol_table_limit() {
    let mut stack = Stack::new();
    stack.limits.max_symbols = 1;
    let err = stack.exec("1 2 [ 'a' 'b' ] STORE").unwrap_err();
    assert!(
        err.to_string()
            .contains("Symbol table has reached its maximum number of entries: 1"),
        "{err}"
    );
    let err = stack.exec("2 'b' CSTORE").unwrap_err();
    assert!(
        err.to_string()
            .contains("Symbol table has reached its maximum number of entries: 1"),
        "{err}"
    );
}

#[test]
fn conditional_store() {
    assert_top("1 'a' CSTORE 2 'a' CSTORE $a", 1);
    assert_top("NULL 'a' CSTORE 2 'a' CSTORE $a", Value::Null);
    assert_top("1 3 CSTORE 2 3 CSTORE PUSHR3", 1);
    assert_top("2 3 CSTORE PUSHR3", 2);
    assert_fails(
        "1 1.5 CSTORE",
        "CSTORE expects variable name to be a string or a register number.",
    );
    assert_fails("1 300 CSTORE", "Invalid register number");
}

#[test]
fn registers() {
    assert_top("1 POPR0 PUSHR0", 1);
    assert_top("1 POPR255 PUSHR255", 1);
    assert_top("1 CPOPR1 2 CPOPR1 PUSHR1", 1);
    assert_top("PUSHR2", Value::Null);
    assert_top("<% 42 %> POPR3 RUNR3", 42);
    assert_fails(
        "RUNR4",
        "RUNR4 expects register number 4 to contain a macro.",
    );
    assert_top("<% 43 %> POPR3 3 RUN", 43);
    assert_fails("4 RUN", "RUN expects register number 4 to contain a macro.");
    assert_fails("4 300 STORE", "Invalid register number");
    assert_top("1 POPR0 CLEARREGS PUSHR0", Value::Null);
    // Keep the register index of every CPOPRn test identical: coverage scores a generic
    // function by its best single instantiation.
    assert_fails("POPR0", "Empty stack.");
    assert_fails("CPOPR1", "Empty stack.");
}

#[test]
fn run_and_eval() {
    assert_top("<% 1 %> 'm' STORE 'm' RUN", 1);
    assert_fails(
        "1.5 RUN",
        "RUN expects macro name to be a STRING or a register number (LONG).",
    );
    assert_top("'1 2' EVAL", 2);
    assert_top("'1%0A2' EVAL", 2);
    assert_top("<% 3 %> EVAL", 3);
    assert_fails("1 EVAL", "EVAL expects a Macro, a function or a String.");
    assert_stack("1 'STOP 2' EVAL 3", &[long(1)]);
    assert_fails(
        "'DROP' EVAL",
        "Line #1 (Exception at '=>DROP<=' in section [TOP] (Empty stack.))",
    );
    assert_top("'1 2' XEVAL", 2);
    assert_fails("<% 1 %> XEVAL", "XEVAL can only be applied to a STRING.");
    assert_fails("1 XEVAL", "XEVAL expects a Macro, a function or a String.");
    let stack = exec("'1' XEVAL");
    assert_eq!(stack.attribute(ATTRIBUTE_IN_XEVAL), None);
    let mut stack = exec("");
    stack.set_attribute(ATTRIBUTE_IN_XEVAL, Some(Value::from(true)));
    stack.exec("'1' XEVAL").unwrap();
    assert_eq!(
        stack.attribute(ATTRIBUTE_IN_XEVAL),
        Some(&Value::from(true))
    );
}

#[test]
fn forget_and_defined() {
    assert_top("1 'a' STORE 'a' FORGET 'a' DEFINED", false);
    assert_top("1 'a' STORE 'a' DEFINED", true);
    assert_top("NULL 'a' STORE 'a' DEFINED", true);
    assert_fails("1 FORGET", "FORGET expects variable name to be a string.");
    assert_fails("1 DEFINED", "DEFINED expects a string on top of the stack.");
    assert_top("<% %> 'm' STORE 'm' DEFINEDMACRO", true);
    assert_top("'m' DEFINEDMACRO", false);
    assert_top("1 'm' STORE 'm' DEFINEDMACRO", false);
    assert_fails(
        "1 DEFINEDMACRO",
        "DEFINEDMACRO expects a string on top of the stack.",
    );
    assert_stack("<% %> 'm' STORE 'm' CHECKMACRO", &[]);
    assert_fails(
        "'m' CHECKMACRO",
        "CHECKMACRO macro 'm' could not be loaded.",
    );
    assert_fails(
        "1 CHECKMACRO",
        "CHECKMACRO expects a string on top of the stack.",
    );
    assert_top("1 'a' STORE 2 'b' STORE CLEARSYMBOLS 'a' DEFINED", false);
}

#[test]
fn clearing_definitions_undefines_them() {
    assert_fails("<% 1 %> 'DUP' DEF CLEARDEFS 1 DUP", "DUP is undefined.");
    assert_top("CLEARDEFS 1 DUP", 1);
    let stack = exec("<% 1 %> 'X' DEF NULL 'Y' DEF CLEARDEFS");
    assert!(stack.defined().values().all(Option::is_none));
}
