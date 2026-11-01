#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};

use super::macros::Macro;
use super::snapshot::value_snapshot;
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
    assert_eq!(value_snapshot(&Value::Mark), "MARK ");
    assert_eq!(
        value_snapshot(&Value::Context(Rc::new(Context {
            symbols: IndexMap::new(),
            registers: Vec::new(),
            defined: IndexMap::new(),
        }))),
        "/* Error while snapshoting element of type 'CONTEXT' */ "
    );
    assert_eq!(
        value_snapshot(&Value::Function(Rc::new(Dummy))),
        "dummy.fn "
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
    let snapshot = value_snapshot(&inner);
    assert!(snapshot.contains("/* Recursive data structures exceeded 1000 levels. */ "));
}

#[test]
fn macro_snapshot_is_source() {
    let m = top("<% 1 'a' STORE <% $a %> EVAL DROP %>");
    assert_eq!(
        value_snapshot(&m),
        "<% 1 'a' STORE <% 'a' LOAD %> EVAL DROP %> "
    );
}
