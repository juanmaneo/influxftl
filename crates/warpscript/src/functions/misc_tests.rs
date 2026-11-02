// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::rc::Rc;

use indexmap::IndexMap;

use crate::error::{Error, Result};
use crate::functions::misc::{
    ATTRIBUTE_INFOMODE, ATTRIBUTE_JSON_STRICT, ATTRIBUTE_MACRO_TTL, ATTRIBUTE_SECURE_KEY,
};
use crate::functions::symbols::ATTRIBUTE_IN_XEVAL;
use crate::stack::{Limits, SecureCodec, SecureScript, Stack};
use crate::tests::{assert_fails, assert_stack, assert_top, exec};
use crate::value::Value;

#[test]
fn constants() {
    assert_top("NULL", Value::Null);
    // `NaN` is parsed as a double literal before any function lookup, like in Warp 10; the
    // function itself is still registered.
    assert_top("NaN", f64::NAN);
    let mut stack = Stack::new();
    crate::functions::call(&mut stack, "NaN").unwrap();
    assert_eq!(stack.pop().unwrap(), Value::Double(f64::NAN));
    assert_top("PI", std::f64::consts::PI);
    assert_top("pi", std::f64::consts::PI);
    assert_top("E", std::f64::consts::E);
    assert_top("e", std::f64::consts::E);
    assert_top("MINLONG", i64::MIN);
    assert_top("MAXLONG", i64::MAX);
}

#[test]
fn typeof_reports_java_types() {
    for (script, expected) in [
        ("NULL", "NULL"),
        ("true", "BOOLEAN"),
        ("1", "LONG"),
        ("1.0", "DOUBLE"),
        ("'s'", "STRING"),
        ("[ ]", "LIST"),
        ("{ }", "MAP"),
        ("( )", "SET"),
        ("<% %>", "MACRO"),
        ("MARK", "MARK"),
        ("SAVE", "CONTEXT"),
        ("test.object", "OPERATOR"),
    ] {
        assert_top(&format!("{script} TYPEOF"), expected);
    }
    assert_fails("TYPEOF", "Empty stack.");
}

#[test]
fn info_and_attributes() {
    assert_stack("{ 'name' 'doc' } INFO", &[]);
    assert_fails("1 INFO", "INFO expects an info map on top of the stack.");
    let mut stack = Stack::with_limits(Limits::unlimited());
    stack.set_attribute(ATTRIBUTE_INFOMODE, Some(Value::from(true)));
    stack.exec("{ } INFO 'not reached'").unwrap();
    assert_eq!(stack.depth(), 1);
    assert_eq!(stack.pop().unwrap().to_string(), "{}");
    // A map nested too deep cannot be snapshot: the error is reported.
    let Value::Map(doc) = crate::tests::top("{ 'name' 'doc' }") else {
        panic!("not a map");
    };
    let mut nested = Value::Long(1);
    for _ in 0..20 {
        nested = Value::list(vec![nested]);
    }
    doc.borrow_mut().insert(Value::from("deep"), nested);
    stack.push(Value::Map(doc));
    let err = stack.exec("INFO").unwrap_err();
    assert!(
        err.to_string()
            .contains("Recursive data structures exceeded 16 levels."),
        "{err}"
    );
    assert!(stack.attribute(ATTRIBUTE_IN_XEVAL).is_none());
    // The xeval attribute is left alone when it was already set.
    stack.set_attribute(ATTRIBUTE_IN_XEVAL, Some(Value::from(true)));
    stack.exec("{ 'a' 1 } INFO").unwrap();
    assert_eq!(stack.pop().unwrap().to_string(), "{a=1}");
    assert_eq!(
        stack.attribute(ATTRIBUTE_IN_XEVAL),
        Some(&Value::from(true))
    );
    let stack = exec("JSONSTRICT");
    assert_eq!(
        stack.attribute(ATTRIBUTE_JSON_STRICT),
        Some(&Value::from(true))
    );
    let stack = exec("5000 MACROTTL");
    assert_eq!(stack.attribute(ATTRIBUTE_MACRO_TTL), Some(&Value::Long(5)));
    assert_fails(
        "'x' MACROTTL",
        "MACROTTL expects a ttl on top of the stack.",
    );
}

#[test]
fn macro_config() {
    assert_fails(
        "'key' MACROCONFIG",
        "MACROCONFIG can only be used from named macro.",
    );
    assert_fails(
        "'d' 'key' MACROCONFIGDEFAULT",
        "MACROCONFIGDEFAULT can only be used from named macro.",
    );
    let mut stack = Stack::with_limits(Limits::unlimited());
    stack.set_config(IndexMap::from([
        ("k1@a/b/c".to_owned(), "v1".to_owned()),
        ("k2@a".to_owned(), "v2".to_owned()),
    ]));
    // Only macros loaded by name (from a repository) are named; emulate that through the API.
    let define = |stack: &mut Stack, name: &str, source: &str| {
        stack.exec(source).unwrap();
        let Value::Macro(m) = stack.pop().unwrap() else {
            panic!("not a macro");
        };
        m.set_name_recursive(name);
        stack.store(name, Value::Macro(m)).unwrap();
    };
    define(
        &mut stack,
        "a/b/c",
        "<% 'k1' MACROCONFIG 'k2' MACROCONFIG %>",
    );
    define(&mut stack, "a/b", "<% 'k3' MACROCONFIG %>");
    define(
        &mut stack,
        "a",
        "<% 'k3' 'dflt' MACROCONFIGDEFAULT 'k3' NULL MACROCONFIGDEFAULT ' k2 ' 7 MACROCONFIGDEFAULT %>",
    );
    define(&mut stack, "bad", "<% 1 MACROCONFIG %>");
    define(&mut stack, "empty", "<% MACROCONFIGDEFAULT %>");
    let err = stack.exec("@empty").unwrap_err();
    assert!(err.to_string().contains("Empty stack."), "{err}");
    stack.exec("@a/b/c").unwrap();
    assert_eq!(stack.pop().unwrap(), Value::from("v2"));
    assert_eq!(stack.pop().unwrap(), Value::from("v1"));
    let err = stack.exec("@a/b").unwrap_err();
    assert!(
        err.to_string().contains(
            "MACROCONFIG macro configuration 'k3' not found, need to set one of [ 'k3@a/b' 'k3@a' ]."
        ),
        "{err}"
    );
    stack.exec("@a").unwrap();
    assert_eq!(stack.pop().unwrap(), Value::from("v2"));
    assert_eq!(stack.pop().unwrap(), Value::Null);
    assert_eq!(stack.pop().unwrap(), Value::from("dflt"));
    let err = stack.exec("@bad").unwrap_err();
    assert!(
        err.to_string()
            .contains("MACROCONFIG expects a macro configuration key name."),
        "{err}"
    );
}

struct FakeCodec;

impl SecureCodec for FakeCodec {
    fn secure(&self, key: &str, script: &str) -> Result<String> {
        if script == "fail" {
            return Err(Error::new("Unable to secure script."));
        }
        Ok(format!("{key}|{script}"))
    }

    fn unsecure(&self, encoded: &str) -> Result<SecureScript> {
        let (key, script) = encoded
            .split_once('|')
            .ok_or_else(|| Error::new("Unable to unsecure script."))?;
        Ok(SecureScript {
            key: key.to_owned(),
            script: script.to_owned(),
        })
    }
}

fn secure_stack() -> Stack {
    let mut stack = Stack::with_limits(Limits::unlimited());
    stack.set_secure_codec(Some(Rc::new(FakeCodec)));
    stack
}

fn run(stack: &mut Stack, script: &str) -> Result<()> {
    stack.exec_checked(script)
}

#[test]
fn secure_key() {
    let stack = exec("'k' SECUREKEY");
    assert_eq!(
        stack.attribute(ATTRIBUTE_SECURE_KEY),
        Some(&Value::from("k"))
    );
    assert_fails("1 SECUREKEY", "SECUREKEY only accepts a string secure key.");
}

#[test]
fn secure_and_unsecure() {
    assert_fails("'1 2' SECURE", "You need to set the secure key first.");
    assert_fails("'k' SECUREKEY 1 SECURE", "SECURE operates on a string.");
    assert_fails(
        "'k' SECUREKEY '1 2' SECURE",
        "Missing secure script encryption key.",
    );
    assert_fails("SECURE", "Empty stack.");
    assert_fails("UNSECURE", "Empty stack.");
    assert_fails("EVALSECURE", "Empty stack.");
    let mut stack = secure_stack();
    run(&mut stack, "'k' SECUREKEY '1 2' SECURE").unwrap();
    assert_eq!(stack.pop().unwrap(), Value::from("k|1 2"));
    let err = run(&mut stack, "'fail' SECURE").unwrap_err();
    assert!(
        err.to_string().contains("Unable to secure script."),
        "{err}"
    );
    run(&mut stack, "'k|1 2' UNSECURE").unwrap();
    assert_eq!(stack.pop().unwrap(), Value::from("1 2"));
    let err = run(&mut stack, "'other|1 2' UNSECURE").unwrap_err();
    assert!(err.to_string().contains("Invalid secure key."), "{err}");
    let err = run(&mut stack, "1 UNSECURE").unwrap_err();
    assert!(
        err.to_string().contains("UNSECURE operates on a string."),
        "{err}"
    );
    let err = run(&mut stack, "'garbage' UNSECURE").unwrap_err();
    assert!(
        err.to_string().contains("Unable to unsecure script."),
        "{err}"
    );
    assert_fails("'x' UNSECURE", "You need to set the secure key first.");
    assert_fails(
        "'k' SECUREKEY 'x' UNSECURE",
        "Missing secure script encryption key.",
    );
}

#[test]
fn evalsecure() {
    assert_fails(
        "1 EVALSECURE",
        "EVALSECURE can only evaluate a secure script stored as a string.",
    );
    assert_fails("'x' EVALSECURE", "Missing secure script encryption key.");
    let mut stack = secure_stack();
    run(&mut stack, "'k|1 2' EVALSECURE").unwrap();
    assert_eq!(stack.elements(), &[Value::Long(1), Value::Long(2)]);
    assert!(!stack.in_secure_macro());
    assert!(stack.allow_redefined());
    // Errors clear the stack so that nothing leaks.
    let err = run(&mut stack, "'k|3 DROP DROP DROP DROP' EVALSECURE").unwrap_err();
    assert!(err.to_string().contains("Empty stack."), "{err}");
    assert_eq!(stack.depth(), 0);
    // Redefined functions are disabled while the secure script runs.
    run(&mut stack, "<% 42 %> 'DUP' DEF").unwrap();
    let err = run(&mut stack, "'k|1 DUP' EVALSECURE").unwrap_err();
    assert!(
        err.to_string()
            .contains("Disallowed redefined function 'DUP'."),
        "{err}"
    );
    run(
        &mut stack,
        "CLEARDEFS NULL 'DUP' DEF 1 2 'k|1 RETURN 2' EVALSECURE 3",
    )
    .unwrap();
    assert_eq!(
        stack.elements(),
        &[
            Value::Long(1),
            Value::Long(2),
            Value::Long(1),
            Value::Long(3)
        ]
    );
    stack.clear();
    run(&mut stack, "<% 'k|1 2 NRETURN 2' EVALSECURE 9 %> EVAL 3").unwrap();
    assert_eq!(stack.elements(), &[Value::Long(1), Value::Long(3)]);
    stack.clear();
    run(&mut stack, "1 'k|STOP' EVALSECURE 2").unwrap();
    assert_eq!(stack.elements(), &[Value::Long(1)]);
    stack.clear();
    // Like `execMulti`, only STOP / RETURN cross the secure script boundary as signals: a
    // BREAK becomes a plain error (and the stack is cleared).
    let err = run(&mut stack, "[ 1 2 ] <% 'k|BREAK' EVALSECURE %> FOREACH").unwrap_err();
    assert!(err.to_string().contains("Exception at BREAK"), "{err}");
    assert_eq!(stack.depth(), 0);
    // Secure scripts run in secure mode: errors are not wrapped with statement details.
    let err = run(&mut stack, "'k|<%25 DROP %25> EVAL' EVALSECURE").unwrap_err();
    assert_eq!(
        err.to_string(),
        "Line #1 (Exception at ''k|<%25 DROP %25> EVAL' =>EVALSECURE<=' in section [TOP] (Line #1 (Exception at '<% DROP %> =>EVAL<=' in section [TOP] (Empty stack.))))"
    );
}

#[test]
fn counters() {
    assert_top("COUNTER COUNTERVALUE", 0);
    assert_top("COUNTER TYPEOF", "COUNTER");
    assert_top("COUNTER 5 COUNTERDELTA -2 COUNTERDELTA COUNTERVALUE", 3);
    assert_top("COUNTER 7 COUNTERSET 1 COUNTERDELTA COUNTERVALUE", 8);
    // Counters are shared objects.
    assert_top(
        "COUNTER 'c' STORE $c 4 COUNTERDELTA DROP $c COUNTERVALUE",
        4,
    );
    assert_top("COUNTER DUP ==", true);
    assert_top("COUNTER COUNTER ==", false);
    assert_top(
        "COUNTER 9223372036854775807 COUNTERSET 1 COUNTERDELTA COUNTERVALUE",
        i64::MIN,
    );
    assert_top("COUNTER 3 COUNTERSET TOSTRING", "3");
    assert_top("COUNTER 3 COUNTERSET ->JSON", "3");
    assert_top("COUNTER SNAPSHOT", "COUNTER ");
    assert_top("COUNTER 3 COUNTERSET SNAPSHOT", "COUNTER 3 COUNTERSET ");
    assert_top("COUNTER 3 COUNTERSET SNAPSHOT EVAL COUNTERVALUE", 3);
    assert_top("{ COUNTER 1 } SIZE", 1);
    assert_fails(
        "1 COUNTERVALUE",
        "COUNTERVALUE can only operate on a counter.",
    );
    assert_fails(
        "COUNTER 'x' COUNTERDELTA",
        "COUNTERDELTA expects a delta (LONG) on top of the stack.",
    );
    assert_fails(
        "1 2 COUNTERDELTA",
        "COUNTERDELTA expects a counter below the delta.",
    );
    assert_fails(
        "COUNTER 'x' COUNTERSET",
        "COUNTERSET expects a value (LONG) on top of the stack.",
    );
    assert_fails(
        "1 2 COUNTERSET",
        "COUNTERSET expects a counter below the value.",
    );
    for f in ["COUNTERVALUE", "COUNTERDELTA", "COUNTERSET"] {
        assert_fails(f, "Empty stack.");
    }
    assert_fails("1 COUNTERSET", "Empty stack.");
}
