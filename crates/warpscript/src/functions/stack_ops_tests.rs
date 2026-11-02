// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use indexmap::{IndexMap, IndexSet};

use crate::tests::{assert_fails, assert_stack, assert_top, exec, top};
use crate::value::Value;

fn long(l: i64) -> Value {
    Value::Long(l)
}

#[test]
fn dup_swap_drop() {
    assert_stack("1 DUP", &[long(1), long(1)]);
    assert_fails("DUP", "Empty stack.");
    assert_stack(
        "1 2 3 2 DUPN",
        &[long(1), long(2), long(3), long(2), long(3)],
    );
    assert_stack("1 0 DUPN", &[long(1)]);
    assert_fails("1 2 DUPN", "Index out of bound.");
    assert_fails("1 -1 DUPN", "Index out of bound.");
    assert_fails(
        "1 'x' DUPN",
        "Unexpected type, expecting a numerical value.",
    );
    assert_fails("DUPN", "Empty stack.");
    assert_stack("1 2 SWAP", &[long(2), long(1)]);
    assert_fails("SWAP", "Empty stack.");
    assert_fails("1 SWAP", "Index out of bound.");
    assert_stack("1 2 DROP", &[long(1)]);
    assert_fails("DROP", "Empty stack.");
    assert_stack("1 2 3 2 DROPN", &[long(1)]);
    assert_stack("1 2 3 3.0 DROPN", &[]);
    assert_fails("1 2 DROPN", "Index out of bound.");
    assert_stack("1 2 CLEAR", &[]);
}

#[test]
fn rotations() {
    assert_stack("1 2 3 ROT", &[long(2), long(3), long(1)]);
    assert_fails("ROT", "Empty stack.");
    assert_fails("1 2 ROT", "Index out of bound.");
    assert_stack("1 2 3 4 3 ROLL", &[long(1), long(3), long(4), long(2)]);
    assert_stack("1 2 3 4 0 ROLL", &[long(1), long(2), long(3), long(4)]);
    assert_stack("1 2 3 4 1 ROLL", &[long(1), long(2), long(3), long(4)]);
    assert_fails("1 2 3 ROLL", "Index out of bound.");
    assert_stack("1 2 3 4 3 ROLLD", &[long(1), long(4), long(2), long(3)]);
    assert_fails("1 2 3 ROLLD", "Index out of bound.");
    assert_stack("1 2 3 2 PICK", &[long(1), long(2), long(3), long(2)]);
    assert_stack("1 2 3 3 PICK", &[long(1), long(2), long(3), long(1)]);
    assert_fails("1 2 3 0 PICK", "Index out of bound.");
    assert_fails("1 2 3 4 PICK", "Index out of bound.");
}

#[test]
fn depth_reset_marks() {
    assert_top("1 2 DEPTH", 2);
    assert_top("DEPTH", 0);
    assert_stack("1 2 3 1 RESET", &[long(1)]);
    assert_stack("1 2 3 5 RESET", &[long(1), long(2), long(3)]);
    assert_fails("1 -1 RESET", "Index out of bound.");
    assert_fails("1 'x' RESET", "RESET expects a numeric (long) limit.");
    assert_stack("MARK", &[Value::Mark]);
    assert_stack("1 MARK 2 3 CLEARTOMARK", &[long(1)]);
    assert_fails("1 2 CLEARTOMARK", "Empty stack.");
    assert_top("1 MARK 2 3 COUNTTOMARK", 2);
    assert_top("MARK COUNTTOMARK", 0);
    assert_fails("1 2 COUNTTOMARK", "Invalid level.");
}

#[test]
fn hide_and_show() {
    assert_stack("1 2 3 NULL HIDE", &[long(3)]);
    assert_stack("1 2 3 2 HIDE", &[long(3), long(2)]);
    assert_stack("1 2 3 2 HIDE DROP NULL SHOW", &[long(1), long(2), long(3)]);
    assert_stack("1 2 3 2 HIDE DROP 1 SHOW", &[long(2), long(3)]);
    assert_fails("1 'x' HIDE", "HIDE expects a number of levels to hide.");
    assert_fails("1 'x' SHOW", "SHOW expects a number of levels to show.");
    assert_fails("1 2 3 2 HIDE DROP DROP DROP", "Empty stack.");
}

#[test]
fn save_and_restore() {
    assert_top("1 'a' STORE SAVE 2 'a' STORE RESTORE $a", 1);
    assert_top("SAVE TYPEOF", "CONTEXT");
    assert_fails("1 RESTORE", "Invalid stack context.");
    assert_stack("1 NOOP BOOTSTRAP SAFECOMPILE", &[long(1)]);
}

#[test]
fn collection_builders() {
    assert_top("[ 1 2 ]", Value::list(vec![long(1), long(2)]));
    assert_top("[ ]", Value::list(Vec::new()));
    assert_top(
        "[ [ 1 ] [ ] ]",
        Value::list(vec![Value::list(vec![long(1)]), Value::list(Vec::new())]),
    );
    assert_fails("1 ]", "Cannot find a matching start of list.");
    assert_top("[[ 1 2 ]]", Value::list(vec![long(1), long(2)]));
    assert_fails("1 ]]", "Cannot find a matching start of vector.");
    assert_top("( 1 2 1 )", Value::set(IndexSet::from([long(1), long(2)])));
    assert_fails("1 )", "Cannot find a matching start of set.");
    assert_top(
        "{ 'a' 1 'b' 2 }",
        Value::map(IndexMap::from([
            (Value::from("a"), long(1)),
            (Value::from("b"), long(2)),
        ])),
    );
    assert_top("{ }", Value::map(IndexMap::new()));
    assert_fails("1 }", "Cannot find a matching start of map.");
    assert_fails("{ 'a' }", "Odd number of elements, cannot create map.");
    assert_fails("{ 'a' 1 'a' 2 }", "Duplicate map key 'a'");
}

#[test]
fn conversions() {
    assert_top("1 2 2 ->LIST", Value::list(vec![long(1), long(2)]));
    assert_top("[ 1 ] ->LIST", Value::list(vec![long(1)]));
    assert_top("( 1 2 ) ->LIST", Value::list(vec![long(1), long(2)]));
    assert_fails(
        "'x' ->LIST",
        "->LIST expects a LIST, SET or a LONG number of elements to pack into a LIST.",
    );
    assert_fails("1 2 ->LIST", "Index out of bound.");
    assert_top(
        "'a' 1 2 ->MAP",
        Value::map(IndexMap::from([(Value::from("a"), long(1))])),
    );
    assert_fails("'a' 1 ->MAP", "Odd number of elements, cannot create map.");
    assert_fails("'a' ->MAP", "Unexpected type, expecting a numerical value.");
    assert_top(
        "[ 1 1 2 ] ->SET",
        Value::set(IndexSet::from([long(1), long(2)])),
    );
    assert_top("( 1 ) ->SET", Value::set(IndexSet::from([long(1)])));
    assert_fails("1 ->SET", "->SET operates on a LIST or SET.");
    assert_top("[ 1 2 ] ->V", Value::list(vec![long(1), long(2)]));
    assert_top("( 1 ) ->V", Value::list(vec![long(1)]));
    assert_fails("1 ->V", "->V expects a collection on top of the stack.");
    assert_stack("1 2 STACKTOLIST", &[Value::list(vec![long(1), long(2)])]);
    assert_stack("STACKTOLIST", &[Value::list(Vec::new())]);
}

#[test]
fn missing_arguments() {
    for script in [
        "RESET", "HIDE", "SHOW", "RESTORE", "->LIST", "->SET", "->V", "ROLL", "PICK",
    ] {
        assert_fails(script, "Empty stack.");
    }
    assert_stack("1 2 1 ROLLD", &[long(1), long(2)]);
    assert_stack("1 2 0 ROLLD", &[long(1), long(2)]);
}

#[test]
fn empty_collections() {
    assert_top("[]", Value::list(Vec::new()));
    assert_top("{}", Value::map(IndexMap::new()));
    assert_top("()", Value::set(IndexSet::new()));
}

#[test]
fn lists_have_reference_semantics() {
    let stack = exec("[ 1 ] 'l' STORE $l $l");
    let elements = stack.elements();
    assert!(elements[0].same_object(&elements[1]));
    assert_eq!(top("[ 1 ] ->V").type_name(), "LIST");
}
