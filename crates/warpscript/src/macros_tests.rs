// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

use std::rc::Rc;

use super::macros::{Callable, Macro, Statement};
use super::stack::Stack;
use super::tests::{exec, top};
use super::value::Value;

fn noop(_stack: &mut Stack) -> super::error::Result<()> {
    Ok(())
}

#[test]
fn callables() {
    let native = Callable::Native {
        name: "NOOP",
        func: noop,
    };
    assert_eq!(native.name(), "NOOP");
    let mut stack = Stack::new();
    native.apply(&mut stack).expect("apply should succeed");
    let defined = Callable::Defined {
        name: Rc::from("MYFN"),
        body: Rc::new(Macro::from_statements(vec![Statement::Push(Value::Long(
            1,
        ))])),
    };
    assert_eq!(defined.name(), "MYFN");
    defined.apply(&mut stack).expect("apply should succeed");
    assert_eq!(stack.pop().expect("pop should succeed"), Value::Long(1));
    let undefined = Callable::Undefined {
        name: Rc::from("GONE"),
    };
    assert_eq!(undefined.name(), "GONE");
    assert_eq!(
        undefined
            .apply(&mut stack)
            .expect_err("apply should fail")
            .to_string(),
        "GONE is undefined."
    );
    let cloned = undefined.clone();
    assert_eq!(cloned.name(), "GONE");
}

#[test]
fn statements_describe_themselves() {
    assert_eq!(Statement::Push(Value::from("a b")).describe(), "'a%20b'");
    assert_eq!(
        Statement::Call(Callable::Native {
            name: "DUP",
            func: noop
        })
        .describe(),
        "DUP"
    );
    let cloned = Statement::Push(Value::Long(1)).clone();
    assert_eq!(cloned.describe(), "1");
}

#[test]
fn macro_basics() {
    let m = Macro::default();
    assert!(m.is_empty());
    assert_eq!(m.len(), 0);
    assert!(!m.is_secure());
    assert_eq!(m.name(), None);
    assert_eq!(m.to_string(), "<% %>");
    let Value::Macro(m) = top("<% 1 <% 2 %> %>") else {
        panic!("not a macro");
    };
    assert_eq!(m.len(), 2);
    assert_eq!(m.snapshot(true), "<% 1 <% 2 %> %>");
    m.set_name_recursive("outer");
    assert_eq!(m.name().as_deref(), Some("outer"));
    let Statement::Push(Value::Macro(inner)) = &m.statements[1] else {
        panic!("not a nested macro");
    };
    assert_eq!(inner.name().as_deref(), Some("outer"));
    // Names are only set once.
    m.set_name_recursive("other");
    assert_eq!(m.name().as_deref(), Some("outer"));
    m.set_secure_recursive(true);
    assert!(m.is_secure());
    assert!(inner.is_secure());
    assert_eq!(m.snapshot(true), "<% /* Secure Macro */ %>");
    assert_eq!(m.snapshot(false), "<% 1 <% 2 %> %>");
    assert_eq!(m.to_string(), "<% /* Secure Macro */ %>");
    assert_eq!(format!("{m:?}"), "Macro(<% 1 <% 2 %> %>)");
    assert!(format!("{:?}", m.statements[0]).starts_with("Push(LONG(1))"));
}

#[test]
fn macro_with_calls_snapshots_names() {
    let m = top("<% DUP 'x' STORE %>");
    assert_eq!(m.to_string(), "<% DUP 'x' STORE %>");
    let stack = exec("<% DUP %> 'm' STORE");
    let Value::Macro(m) = stack.load("m").expect("load should succeed") else {
        panic!("not a macro");
    };
    assert_eq!(m.len(), 1);
}
