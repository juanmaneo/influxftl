#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};

use super::bitset::BitSet;
use super::macros::{Macro, Statement};
use super::snapshot::{add_element, value_snapshot};
use super::tests::top;
use super::value::{Context, FunctionObject, Value};

struct Dummy;

impl FunctionObject for Dummy {
    fn name(&self) -> &'static str {
        "dummy.fn"
    }

    fn type_name(&self) -> &'static str {
        "OPERATOR"
    }
}

#[test]
fn scalars() {
    assert_eq!(value_snapshot(&Value::Null), "NULL ");
    assert_eq!(value_snapshot(&Value::from(true)), "true ");
    assert_eq!(value_snapshot(&Value::from(false)), "false ");
    assert_eq!(value_snapshot(&Value::Long(-5)), "-5 ");
    assert_eq!(value_snapshot(&Value::Double(1.5)), "1.5 ");
    assert_eq!(value_snapshot(&Value::from("a b'c")), "'a%20b%27c' ");
    assert_eq!(
        value_snapshot(&Value::bytes(b"foobar")),
        "'OaxjNa4m' OPB64-> "
    );
    assert_eq!(
        value_snapshot(&Value::BitSet(Rc::new(BitSet::from_bytes(b"foobar")))),
        "'OaxjNa4m' OPB64-> BYTESTOBITS "
    );
    assert_eq!(value_snapshot(&Value::Mark), "MARK ");
    assert_eq!(
        value_snapshot(&Value::Context(Rc::new(Context {
            symbols: IndexMap::new(),
            registers: Vec::new(),
            defined: IndexMap::new(),
        }))),
        "'UNSUPPORTED:CONTEXT' "
    );
    assert_eq!(
        value_snapshot(&Value::Function(Rc::new(Dummy))),
        "'dummy.fn' FUNCREF "
    );
    assert_eq!(
        value_snapshot(&Value::from(Rc::new(Macro::new()))),
        "<% %> "
    );
}

#[test]
fn containers() {
    assert_eq!(
        value_snapshot(&Value::list(vec![Value::Long(1), Value::from("x")])),
        "[] 1 +! 'x' +! "
    );
    assert_eq!(
        value_snapshot(&Value::set(IndexSet::from([Value::Long(1)]))),
        "() 1 +! "
    );
    assert_eq!(
        value_snapshot(&Value::map(IndexMap::from([(
            Value::from("k"),
            Value::Long(2)
        )]))),
        "{} 2 'k' PUT "
    );
}

#[test]
fn recursive_structures_are_cut() {
    let inner = Value::list(Vec::new());
    let Value::List(list) = &inner else {
        panic!("not a list");
    };
    list.borrow_mut().push(inner.clone());
    // `value_snapshot` falls back to the comment `Macro.snapshot` emits…
    assert_eq!(
        value_snapshot(&inner),
        "/* Error while snapshoting element of type 'LIST' */ "
    );
    // …while `addElement` reports the error, past 16 levels.
    let mut out = String::new();
    assert_eq!(
        add_element(&mut out, &inner, false).unwrap_err().message(),
        "Recursive data structures exceeded 16 levels."
    );
    let mut nested = Value::Long(1);
    for _ in 0..15 {
        nested = Value::list(vec![nested]);
    }
    let mut out = String::new();
    add_element(&mut out, &nested, false).unwrap();
    assert_eq!(out.matches("[] ").count(), 15);
    let mut out = String::new();
    assert!(add_element(&mut out, &Value::list(vec![nested]), false).is_err());
    // A macro statement nested too deep becomes a comment inside the macro; the macro
    // itself does not count as a level.
    let deep = |levels: usize| {
        let mut nested = Value::Long(1);
        for _ in 0..levels {
            nested = Value::list(vec![nested]);
        }
        Value::from(Rc::new(Macro::from_statements(vec![Statement::Push(
            nested,
        )])))
    };
    // `Macro.toString()`: the statements are the first level (15 lists + the scalar = 16).
    assert_eq!(
        deep(16).to_string(),
        "<% /* Error while snapshoting element of type 'LIST' */ %>"
    );
    assert!(deep(15).to_string().starts_with("<% [] [] [] "));
    // `SNAPSHOT.addElement(macro)`: the macro itself is the first level.
    assert_eq!(
        value_snapshot(&deep(15)),
        "<% /* Error while snapshoting element of type 'LIST' */ %> "
    );
    assert!(value_snapshot(&deep(14)).starts_with("<% [] [] [] "));
    // Nested macros do not add a level.
    let inner = deep(14);
    let outer = Value::from(Rc::new(Macro::from_statements(vec![Statement::Push(
        inner,
    )])));
    assert!(!value_snapshot(&outer).contains("/* Error"));
    assert!(value_snapshot(&Value::list(vec![outer])).contains("/* Error"));
}

#[test]
fn readable_snapshots() {
    let mut out = String::new();
    let value = top("{ 'k' [ 1 'a%25b' ( true NULL ) ] 'it%27s' 2.5 }");
    add_element(&mut out, &value, true).unwrap();
    assert_eq!(
        out,
        "{\n'k' [ 1 'a%25b' ( true NULL ) ] \n'it%27s' 2.5 \n} "
    );
    let mut out = String::new();
    add_element(&mut out, &top("'tab\there'"), true).unwrap();
    assert_eq!(out, "'tab%09here' ");
    // Structures nested too deep fail in every container, readable or not.
    let mut deep = Value::Long(1);
    for _ in 0..16 {
        deep = Value::list(vec![deep]);
    }
    let containers = [
        Value::list(vec![deep.clone()]),
        Value::set([deep.clone()].into_iter().collect()),
        Value::map(IndexMap::from([(Value::from("k"), deep.clone())])),
        Value::map(IndexMap::from([(deep, Value::Long(1))])),
    ];
    for container in &containers {
        for readable in [false, true] {
            let mut out = String::new();
            assert!(add_element(&mut out, container, readable).is_err());
        }
    }
}

#[test]
fn macro_snapshot_is_source() {
    let m = top("<% 1 'a' STORE <% $a %> EVAL DROP %>");
    assert_eq!(
        value_snapshot(&m),
        "<% 1 'a' STORE <% 'a' LOAD %> EVAL DROP %> "
    );
}
