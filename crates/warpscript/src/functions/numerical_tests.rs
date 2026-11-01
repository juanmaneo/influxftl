#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::functions::numerical::{Binary, Unary, binary, unary};
use crate::stack::Stack;
use crate::tests::{assert_fails, assert_top};
use crate::value::Value;

fn long(l: i64) -> Value {
    Value::Long(l)
}

fn double(d: f64) -> Value {
    Value::Double(d)
}

fn list(items: Vec<Value>) -> Value {
    Value::list(items)
}

const UNARY_MSG: &str =
    "ABS can only operate on numerical values, list of numerical values or numerical GTSs.";

const BINARY_MSG: &str = "HYPOT can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values.";

const IN_LIST_MSG: &str = "MAX can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values, or a list of numerical values.";

#[test]
fn unary_operators_follow_java_promotion() {
    // `Both`: the type is preserved.
    assert_top("-5 ABS", 5);
    assert_top("-5.5 ABS", 5.5);
    // `Double`: longs are converted to doubles.
    assert_top("0 COS", 1.0);
    assert_top("0.0 COS", 1.0);
    // `Long`: doubles are truncated to longs.
    assert_top("5 INCREMENTEXACT", 6);
    assert_top("5.7 INCREMENTEXACT", 6);
    assert_top("-5.7 INCREMENTEXACT", -4);
    // `DoubleToLong`: longs are kept as they are, doubles are mapped to longs.
    assert_top("2.5 ROUND", 3);
    assert_top("2 ROUND", 2);
}

#[test]
fn unary_recurses_into_lists() {
    assert_top(
        "[ 1 2.5 [ 3.5 [ ] ] ] ROUND",
        list(vec![long(1), long(3), list(vec![long(4), list(vec![])])]),
    );
    assert_top("[ ] ABS", list(vec![]));
    assert_top(
        "[ -1 [ -2.5 ] ] ABS",
        list(vec![long(1), list(vec![double(2.5)])]),
    );
    // The original list is left untouched.
    assert_top("[ -1 ] 'l' STORE $l ABS DROP $l", list(vec![long(-1)]));
}

#[test]
fn unary_rejects_other_operands() {
    assert_fails("'a' ABS", UNARY_MSG);
    assert_fails("[ 1 'a' ] ABS", UNARY_MSG);
    assert_fails("[ [ true ] ] ABS", UNARY_MSG);
    assert_fails("NULL ABS", UNARY_MSG);
    assert_fails("ABS", "Empty stack.");
}

#[test]
fn unary_errors_of_the_operator_propagate() {
    assert_fails("MAXLONG INCREMENTEXACT", "long overflow");
    assert_fails("[ 1 [ MAXLONG ] ] INCREMENTEXACT", "long overflow");
}

#[test]
fn binary_on_two_numbers() {
    // `Long`: everything is converted to long.
    assert_top("7 2 FLOORDIV", 3);
    assert_top("7.9 2.1 FLOORDIV", 3);
    assert_fails("1 0 FLOORDIV", "/ by zero");
    // `Both`: long when both operands are longs, double otherwise.
    assert_top("1 2 MAX", 2);
    assert_top("1 2.5 MAX", 2.5);
    assert_top("2.5 1 MAX", 2.5);
    assert_top("-1 2.0 MIN", -1.0);
    // `Double`: everything is converted to double.
    assert_top("3 4 HYPOT", 5.0);
    assert_top("3 4.0 HYPOT", 5.0);
}

#[test]
fn binary_number_on_top_of_list() {
    // Each element is the left operand, the number on top the right one.
    assert_top("[ 1 2.5 ] 2 MAX", list(vec![long(2), double(2.5)]));
    assert_top("[ 7 -7 ] 2 FLOORDIV", list(vec![long(3), long(-4)]));
    assert_top(
        "[ 3 -3.0 ] 1 COPYSIGN",
        list(vec![double(3.0), double(3.0)]),
    );
    assert_top("[ ] 2 MAX", list(vec![]));
    assert_fails("[ 1 'a' ] 2 MAX", IN_LIST_MSG);
    assert_fails("[ 'a' ] 4 HYPOT", BINARY_MSG);
    assert_fails("'a' 4 HYPOT", BINARY_MSG);
    assert_fails("'a' 1 MAX", IN_LIST_MSG);
    assert_fails("1 MAX", "Empty stack.");
    assert_fails("[ 1 2 ] 0 FLOORDIV", "/ by zero");
}

#[test]
fn binary_list_on_top_of_number() {
    // The number under the list is the left operand of every application.
    assert_top(
        "1 [ 3 -0.0 ] COPYSIGN",
        list(vec![double(1.0), double(-1.0)]),
    );
    assert_top("7 [ 2 -2 ] FLOORDIV", list(vec![long(3), long(-4)]));
    assert_top("2 [ 1 3.0 ] **", list(vec![long(2), double(8.0)]));
    assert_top("1 [ ] HYPOT", list(vec![]));
    assert_fails("4 [ 3 'a' ] HYPOT", BINARY_MSG);
    assert_fails("'a' [ 3 ] HYPOT", BINARY_MSG);
    assert_fails("[ 3 ] HYPOT", "Empty stack.");
    assert_fails("2 [ 1 0 ] FLOORDIV", "/ by zero");
}

#[test]
fn binary_reduces_a_single_list_when_allowed() {
    assert_top("[ 1 3 2 ] MAX", 3);
    assert_top("[ 1 3.5 2 ] MAX", 3.5);
    assert_top("[ 1.5 3 2 ] MIN", 1.5);
    assert_top("[ 4 ] MIN", 4);
    assert_top("[ 4.5 ] MIN", 4.5);
    assert_top("[ ] MAX", Value::Null);
    assert_top("[ 1 2 3 ] ADDEXACT", 6);
    assert_top("[ 1.9 2.9 ] ADDEXACT", 3);
    assert_top("[ 2.9 ] MULTIPLYEXACT", 2);
    assert_fails("[ MAXLONG 1 ] ADDEXACT", "long overflow");
    assert_fails("[ 1 'a' ] MAX", IN_LIST_MSG);
    assert_fails("'a' MAX", IN_LIST_MSG);
    assert_fails("true MAX", IN_LIST_MSG);
    assert_fails("MAX", "Empty stack.");
}

/// Run `op` on a stack pre-loaded with `inputs` (bottom first) and return the top result.
fn reduce(op: Binary, inputs: Vec<Value>) -> Value {
    let mut stack = Stack::new();
    stack.push(Value::list(inputs));
    binary(&mut stack, "OP", op, true).unwrap();
    stack.pop().unwrap()
}

#[test]
fn reduction_seeds_use_the_operator_type() {
    // No ported function reduces a list with a double-only operator, exercise the API.
    assert_eq!(
        reduce(Binary::Double(f64::hypot), vec![long(3), long(4)]),
        double(5.0)
    );
    assert_eq!(
        reduce(Binary::Double(f64::hypot), vec![long(3)]),
        double(3.0)
    );
    assert_eq!(
        reduce(Binary::Long(crate::jmath::add_exact), vec![double(1.5)]),
        long(1)
    );
    assert_eq!(
        reduce(Binary::Both(i64::max, f64::max), vec![double(1.5)]),
        double(1.5)
    );
}

#[test]
fn unary_api_rejects_non_numbers() {
    let mut stack = Stack::new();
    stack.push("x");
    let err = unary(&mut stack, "NEG", Unary::Double(|d| -d)).unwrap_err();
    assert_eq!(
        err.to_string(),
        "NEG can only operate on numerical values, list of numerical values or numerical GTSs."
    );
    stack.push(2.0);
    unary(&mut stack, "NEG", Unary::Double(|d| -d)).unwrap();
    assert_eq!(stack.pop().unwrap(), double(-2.0));
}
