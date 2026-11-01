#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of `CAPGET`, `CAPCHECK`, `CAPDEL`, `CAPADD` and `CAPEXPORT`.

use std::rc::Rc;

use indexmap::IndexMap;

use crate::stack::{Limits, Stack, TokenResolver};
use crate::tests::{assert_fails, assert_top};
use crate::value::Value;

/// A stack granted the capabilities `caps`.
fn granted(caps: &[(&str, &str)]) -> Stack {
    let mut stack = Stack::with_limits(Limits::unlimited());
    for (name, value) in caps {
        stack
            .capabilities_mut()
            .insert((*name).to_owned(), (*value).to_owned());
    }
    stack
}

/// Run `script` on `stack` and return the top of the stack.
fn run(stack: &mut Stack, script: &str) -> Value {
    stack.exec_checked(script).unwrap();
    stack.pop().unwrap()
}

fn map(entries: &[(&str, Value)]) -> Value {
    Value::map(
        entries
            .iter()
            .map(|(k, v)| (Value::from(*k), v.clone()))
            .collect(),
    )
}

#[test]
fn capget_and_capcheck() {
    let mut stack = granted(&[("a", "1"), ("b", "two")]);
    assert_eq!(run(&mut stack, "'a' CAPGET"), Value::from("1"));
    assert_eq!(run(&mut stack, "'c' CAPGET"), Value::Null);
    assert_eq!(run(&mut stack, "'a' CAPCHECK"), Value::Boolean(true));
    assert_eq!(run(&mut stack, "'c' CAPCHECK"), Value::Boolean(false));
    assert_eq!(
        run(&mut stack, "NULL CAPGET"),
        map(&[("a", Value::from("1")), ("b", Value::from("two"))])
    );
    assert_eq!(
        run(&mut stack, "[ ] CAPGET"),
        map(&[("a", Value::from("1")), ("b", Value::from("two"))])
    );
    // Requested names: missing ones map to NULL, non-string names are ignored.
    assert_eq!(
        run(&mut stack, "[ 'b' 'c' 3 ] CAPGET"),
        map(&[("b", Value::from("two")), ("c", Value::Null)])
    );
    // Without any capability there is no `Capabilities` object: nothing is reported.
    let mut none = granted(&[]);
    assert_eq!(run(&mut none, "[ 'a' ] CAPGET"), map(&[]));
    assert_eq!(run(&mut none, "NULL CAPGET"), map(&[]));
    assert_eq!(run(&mut none, "'a' CAPGET"), Value::Null);
    assert_eq!(run(&mut none, "'a' CAPCHECK"), Value::Boolean(false));
    // The test stacks are granted `limits` only.
    assert_top("[ 'a' ] CAPGET", map(&[("a", Value::Null)]));
    assert_top("NULL CAPGET", map(&[("limits", Value::from(""))]));
    assert_fails(
        "1 CAPGET",
        "CAPGET expects a capability name (STRING) or a LIST thereof.",
    );
    assert_fails("1 CAPCHECK", "CAPCHECK expects a STRING capability name.");
    assert_fails(
        "NULL CAPCHECK",
        "CAPCHECK expects a STRING capability name.",
    );
    assert_fails("CAPGET", "Empty stack.");
    assert_fails("CAPCHECK", "Empty stack.");
}

#[test]
fn capdel() {
    let mut stack = granted(&[("a", "1"), ("b", "2"), ("c", "3"), ("d", "4")]);
    stack.exec_checked("'a' CAPDEL").unwrap();
    assert_eq!(run(&mut stack, "'a' CAPCHECK"), Value::Boolean(false));
    stack.exec_checked("[ 'b' 7 'zz' ] CAPDEL").unwrap();
    assert_eq!(
        run(&mut stack, "NULL CAPGET"),
        map(&[("c", Value::from("3")), ("d", Value::from("4"))])
    );
    stack.exec_checked("NULL CAPDEL").unwrap();
    assert_eq!(run(&mut stack, "NULL CAPGET"), map(&[]));
    assert_eq!(stack.depth(), 0);
    // Deleting from a stack without capabilities is a no-op.
    let mut none = granted(&[]);
    assert_eq!(
        run(&mut none, "'x' CAPDEL [ 'x' ] CAPDEL NULL CAPDEL 1"),
        Value::Long(1)
    );
    assert_fails(
        "1 CAPDEL",
        "CAPDEL expects a capability name (STRING), a LIST thereof or NULL.",
    );
    assert_fails("CAPDEL", "Empty stack.");
}

/// A resolver knowing a single token with two capabilities and one plain attribute.
struct OneToken;

impl TokenResolver for OneToken {
    fn token_attributes(&self, token: &str) -> Option<IndexMap<String, String>> {
        (token == "good").then(|| {
            IndexMap::from([
                (".cap:a".to_owned(), "from-token".to_owned()),
                (".cap:x".to_owned(), "10".to_owned()),
                ("owner".to_owned(), "me".to_owned()),
            ])
        })
    }
}

#[test]
fn capadd() {
    let mut stack = granted(&[("a", "1")]);
    stack.set_token_resolver(Some(Rc::new(OneToken)));
    assert!(stack.token_resolver().is_some());
    stack.exec_checked("'good' CAPADD").unwrap();
    // Existing capabilities are kept, new ones added, other attributes ignored.
    assert_eq!(
        run(&mut stack, "NULL CAPGET"),
        map(&[("a", Value::from("1")), ("x", Value::from("10"))])
    );
    let err = stack.exec_checked("'bad' CAPADD").unwrap_err().to_string();
    assert!(err.contains("(invalid token.)"), "{err}");
    let err = stack.exec_checked("1 CAPADD").unwrap_err().to_string();
    assert!(err.contains("CAPADD expects a TOKEN."), "{err}");
    // Without a resolver every token is invalid.
    assert_fails("'good' CAPADD", "invalid token.");
    assert_fails("CAPADD", "Empty stack.");
}

#[test]
fn capexport() {
    assert_fails(
        "'a' CAPEXPORT",
        "CAPEXPORT can only be called from a GUARDed macro.",
    );
    assert_fails("CAPEXPORT", "Empty stack.");
    let mut stack = granted(&[]);
    assert!(stack.set_exported_capabilities(Some(Vec::new())).is_none());
    stack
        .exec_checked("'a' CAPEXPORT NULL CAPEXPORT [ 'b' 'a' NULL ] CAPEXPORT")
        .unwrap();
    assert_eq!(
        stack.exported_capabilities_mut().unwrap().clone(),
        vec![Some("a".to_owned()), None, Some("b".to_owned())]
    );
    for bad in ["1 CAPEXPORT", "[ 'c' 1 ] CAPEXPORT"] {
        let err = stack.exec_checked(bad).unwrap_err().to_string();
        assert!(
            err.contains("CAPEXPORT expects NULL, a capability name (STRING) or a LIST thereof."),
            "{err}"
        );
    }
    // The elements before the invalid one were exported.
    assert_eq!(stack.exported_capabilities_mut().unwrap().len(), 4);
    assert_eq!(stack.set_exported_capabilities(None).unwrap().len(), 4);
    assert!(stack.exported_capabilities_mut().is_none());
}
