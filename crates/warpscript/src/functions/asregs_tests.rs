// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of `ASREGS`.

use std::rc::Rc;

use crate::macros::{Macro, Statement};
use crate::tests::{assert_fails, assert_stack, assert_top, test_stack, top};
use crate::value::Value;

#[test]
fn variables_become_registers() {
    // Without a list, the variables the macro writes are converted (most used first).
    assert_top(
        "<% 1 'a' STORE $a $b %> ASREGS SNAPSHOT",
        "<% 1 POPR0 PUSHR0 'b' LOAD %> ",
    );
    assert_top(
        "<% 1 'a' STORE 2 'b' STORE $b $b $a %> ASREGS SNAPSHOT",
        "<% 1 POPR1 2 POPR0 PUSHR0 PUSHR0 PUSHR1 %> ",
    );
    // With a list, only the listed variables are converted; `LONG`s are skipped.
    assert_top(
        "<% $a $b %> [ 'b' 7 ] ASREGS SNAPSHOT",
        "<% 'a' LOAD PUSHR0 %> ",
    );
    assert_top(
        "<% 1 'a' CSTORE $a %> [ 'a' 'a' ] ASREGS SNAPSHOT",
        "<% 1 CPOPR0 PUSHR0 %> ",
    );
    // The rewritten macro behaves like the original without touching the symbol table.
    assert_stack(
        "<% 1 'a' STORE 2 'b' STORE $a $b + %> ASREGS EVAL SYMBOLS",
        &[Value::Long(3), Value::list(Vec::new())],
    );
    assert_fails("<% 1 'a' STORE %> ASREGS EVAL $a", "Unknown symbol 'a'");
}

#[test]
fn registers_in_use_are_skipped() {
    assert_top(
        "<% PUSHR0 POPR1 CPOPR2 RUNR3 1 'a' STORE $a %> ASREGS SNAPSHOT",
        "<% PUSHR0 POPR1 CPOPR2 RUNR3 1 POPR4 PUSHR4 %> ",
    );
    // Register numbers used with `LOAD`, `STORE`, `CSTORE` and `RUN` are in use too and get
    // the faster register functions.
    assert_top(
        "<% 0 LOAD 1 STORE 2 CSTORE 3 RUN 'a' STORE %> ASREGS SNAPSHOT",
        "<% PUSHR0 POPR1 CPOPR2 RUNR3 POPR4 %> ",
    );
    assert_top(
        "<% [ 1 'x' 3 ] STORE 'a' STORE %> [ 'a' ] ASREGS SNAPSHOT",
        "<% [ 1 'x' 3 ] STORE POPR0 %> ",
    );
    // Without a list `x`, written by the `STORE`, is converted too (to the free register 2).
    assert_top(
        "<% [ 1 'x' 3 ] STORE 'a' STORE %> ASREGS SNAPSHOT",
        "<% POPR3 POPR2 POPR1 POPR0 %> ",
    );
    // Registers are 32-bit numbers in Warp 10 (`Long.intValue()`).
    assert_top("<% 4294967296 LOAD %> [ ] ASREGS SNAPSHOT", "<% PUSHR0 %> ");
    // A register number out of range has no register function: the access is kept.
    assert_top(
        "<% 300 LOAD 'a' STORE %> ASREGS SNAPSHOT",
        "<% 300 LOAD POPR0 %> ",
    );
    assert_fails("<% -1 LOAD %> [ ] ASREGS", "bitIndex < 0: -1");
    assert_fails("<% [ -5 ] STORE %> [ ] ASREGS", "bitIndex < 0: -5");
}

#[test]
fn store_on_list_literals() {
    // A list of registers (and `NULL`s) becomes `POPR`s in reverse order.
    assert_top(
        "<% 1 2 3 4 [ 3 7 NULL 9 ] STORE %> [ ] ASREGS SNAPSHOT",
        "<% 1 2 3 4 POPR9 DROP POPR7 POPR3 %> ",
    );
    assert_top(
        "<% 1 2 [ 'a' 'b' ] STORE $a $b %> ASREGS SNAPSHOT",
        "<% 1 2 POPR1 POPR0 PUSHR0 PUSHR1 %> ",
    );
    assert_stack(
        "<% 1 2 [ 'a' 'b' ] STORE $a $b %> ASREGS EVAL",
        &[Value::Long(1), Value::Long(2)],
    );
    // A register stored several times keeps the top-most value, like `STORE`.
    assert_top(
        "<% 1 2 3 [ 'a' 'b' 'a' ] STORE $a $b %> ASREGS SNAPSHOT",
        "<% 1 2 3 POPR0 POPR1 DROP PUSHR0 PUSHR1 %> ",
    );
    assert_stack(
        "<% 1 2 3 [ 'a' 'b' 'a' ] STORE $a $b %> ASREGS EVAL",
        &[Value::Long(3), Value::Long(2)],
    );
    assert_top("<% [ ] STORE %> [ ] ASREGS SNAPSHOT", "<% %> ");
    // A variable without a register keeps the list, with the other names replaced.
    assert_top(
        "<% 1 2 [ 'a' 'z' ] STORE %> [ 'a' ] ASREGS SNAPSHOT",
        "<% 1 2 [ 0 'z' ] STORE %> ",
    );
    assert_top(
        "<% 1 2 3 [ 'a' NULL 'z' ] STORE %> [ 'a' ] ASREGS SNAPSHOT",
        "<% 1 2 3 [ 0 NULL 'z' ] STORE %> ",
    );
    // Any mark opens the list; a register out of range falls back to `n STORE`.
    assert_top(
        "<% 1 2 MARK 300 1 ] STORE %> [ ] ASREGS SNAPSHOT",
        "<% 1 2 POPR1 300 STORE %> ",
    );
    // A `null` pushed as a value (`->MACRO`) counts as `NULL`.
    assert_top(
        "[ '[' FUNCREF NULL ']' FUNCREF 'STORE' FUNCREF ] ->MACRO [ ] ASREGS SNAPSHOT",
        "<% DROP %> ",
    );
}

#[test]
fn store_on_list_values() {
    // A list value (`->MACRO`) has its names replaced; its registers are in use.
    assert_top(
        "[ 1 2 [ 'a' 0 'z' ] 'STORE' FUNCREF ] ->MACRO [ 'a' ] ASREGS SNAPSHOT",
        "<% 1 2 [] 1 +! 0 +! 'z' +! STORE %> ",
    );
    // The list of the original macro is left untouched.
    assert_top(
        "[ [ 'a' ] 'STORE' FUNCREF ] ->MACRO DUP [ 'a' ] ASREGS DROP SNAPSHOT",
        "<% [] 'a' +! STORE %> ",
    );
    assert_fails(
        "[ [ -1 ] 'STORE' FUNCREF ] ->MACRO [ ] ASREGS",
        "bitIndex < 0: -1",
    );
}

#[test]
fn nested_macros_are_rewritten() {
    assert_top(
        "<% <% 1 'a' STORE %> EVAL $a %> ASREGS SNAPSHOT",
        "<% <% 1 POPR0 %> EVAL PUSHR0 %> ",
    );
    assert_top("<% <% 1 'a' STORE %> EVAL $a %> ASREGS EVAL", 1);
    // The original macro is not modified.
    assert_top(
        "<% 1 'a' STORE %> DUP ASREGS DROP SNAPSHOT",
        "<% 1 'a' STORE %> ",
    );
    // Redefined functions are kept as is.
    assert_top(
        "<% 1 %> 'one' DEF <% one 'a' STORE %> ASREGS SNAPSHOT",
        "<% one POPR0 %> ",
    );
    // The names of the macros are preserved.
    let inner = top("<% 1 'a' STORE %>");
    let Value::Macro(inner_macro) = &inner else {
        panic!("macro expected");
    };
    *inner_macro.name.borrow_mut() = Some("inner".to_owned());
    let outer = Macro::from_statements(vec![Statement::Push(inner)]);
    *outer.name.borrow_mut() = Some("outer".to_owned());
    let mut stack = test_stack();
    stack.push(Value::Macro(Rc::new(outer)));
    stack.exec("ASREGS").unwrap();
    let Value::Macro(result) = stack.pop().unwrap() else {
        panic!("macro expected");
    };
    assert_eq!(result.name().as_deref(), Some("outer"));
    let Some(Statement::Push(Value::Macro(nested))) = result.statements.first() else {
        panic!("nested macro expected");
    };
    assert_eq!(nested.name().as_deref(), Some("inner"));
    assert_eq!(nested.snapshot(false), "<% 1 POPR0 %>");
}

#[test]
fn all_registers_in_use() {
    let pops: Vec<String> = (0..256).map(|r| format!("POPR{r}")).collect();
    let script = format!("<% {} 1 'a' STORE %> ", pops.join(" "));
    // No register is left: the variables keep their names, whatever the list contains.
    assert_top(
        &format!("{script} [ 'a' 1.5 ] ASREGS SNAPSHOT"),
        script.as_str(),
    );
    assert_fails(
        "<% 1 %> [ 'a' 1.5 ] ASREGS",
        "ASREGS expects a list of variable names on top of the stack.",
    );
}

#[test]
fn unresolvable_accesses_abort() {
    const UNABLE: &str = "ASREGS was unable to convert variables to registers.";
    // Accesses whose symbol is not a literal.
    assert_fails("<% LOAD %> [ ] ASREGS", UNABLE);
    assert_fails("<% STORE %> [ ] ASREGS", UNABLE);
    assert_fails("<% 1.5 LOAD %> [ ] ASREGS", UNABLE);
    assert_fails("<% DUP STORE %> [ ] ASREGS", UNABLE);
    assert_fails("<% 'x' 'y' SWAP CSTORE %> [ ] ASREGS", UNABLE);
    // `RUN` on a name calls a macro which may use any symbol.
    assert_fails("<% @foo %> [ ] ASREGS", UNABLE);
    // List literals must only hold registers, names and `NULL`s, and must be opened.
    assert_fails("<% [ 1.5 ] STORE %> [ ] ASREGS", UNABLE);
    assert_fails("<% [ $x ] STORE %> [ ] ASREGS", UNABLE);
    assert_fails("<% 1 ] STORE %> [ ] ASREGS", UNABLE);
    // `LSTORE` and `MSTORE` write symbols that are not explicit.
    assert_fails("<% 1 'a' 0 LSTORE %> [ ] ASREGS", UNABLE);
    assert_fails("<% { 'a' 1 } MSTORE %> [ ] ASREGS", UNABLE);
    // Nested macros are inspected too.
    assert_fails("<% <% LOAD %> %> [ ] ASREGS", UNABLE);
    // Secure macros cannot be inspected.
    assert_fails(
        "<% 1 %> MSEC [ ] ASREGS",
        "ASREGS cannot operate on a secure Macro.",
    );
    let inner = top("<% 1 %> MSEC");
    let outer = Macro::from_statements(vec![Statement::Push(inner)]);
    let mut stack = test_stack();
    stack.push(Value::Macro(Rc::new(outer)));
    let err = stack.exec("ASREGS").unwrap_err().to_string();
    assert!(
        err.contains("ASREGS cannot operate on a secure Macro."),
        "{err}"
    );
    // Without a list, `VARS` must be able to find the variables.
    assert_fails("<% LOAD %> ASREGS", "ASREGS failed.");
    assert_fails(
        "<% LOAD %> ASREGS",
        "Encountered a LOAD/STORE or CSTORE operation with a non explicit symbol name.",
    );
}

#[test]
fn argument_errors() {
    const EXPECTS: &str = "ASREGS expects an optional list of variable names and a macro.";
    assert_fails("1 ASREGS", EXPECTS);
    assert_fails("[ ] 1 ASREGS", EXPECTS);
    assert_fails("[ ] ASREGS", "Empty stack.");
    assert_fails("ASREGS", "Empty stack.");
}
