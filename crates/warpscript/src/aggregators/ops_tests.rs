// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the `op.*` operations applied directly (the window's ticks stand for the indices
//! of the collections of `APPLY`).

use super::ops::{Kind, Op};
use super::tests::{Window, doubles, longs};
use super::{Aggregator, Roles};
use crate::value::Value;

fn op(kind: Kind, flag: bool) -> Op {
    Op {
        name: "op.x",
        kind,
        flag,
    }
}

fn value(kind: Kind, flag: bool, values: &[Value]) -> Value {
    Window::of(values).value(&op(kind, flag))
}

fn b(b: bool) -> Value {
    Value::Boolean(b)
}

#[test]
fn sums_and_products() {
    for (kind, values, expected) in [
        (Kind::Add, vec![b(true), b(true)], Value::Long(2)),
        (
            Kind::Add,
            vec![b(true), Value::Double(1.5)],
            Value::Double(2.5),
        ),
        (Kind::Add, vec![b(false), Value::Long(4)], Value::Long(4)),
        (Kind::Add, vec![Value::Long(1), b(true)], Value::Long(2)),
        (
            Kind::Add,
            vec![Value::Long(1), Value::Double(0.5)],
            Value::Double(1.5),
        ),
        (
            Kind::Add,
            vec![Value::Double(1.5), b(true)],
            Value::Double(2.5),
        ),
        (
            Kind::Add,
            vec![Value::Double(1.5), Value::Long(2)],
            Value::Double(3.5),
        ),
        (Kind::Add, longs(&[i64::MAX, 1]), Value::Long(i64::MIN)),
        (Kind::Add, vec![b(true)], b(true)),
        (
            Kind::Add,
            vec![Value::string("x"), b(true)],
            Value::string("xtrue"),
        ),
        (
            Kind::Mul,
            vec![Value::string("x"), Value::Long(2)],
            Value::string("x2"),
        ),
        (Kind::Mul, vec![b(true), Value::Long(4)], Value::Long(4)),
        (
            Kind::Mul,
            vec![b(true), Value::Double(2.5)],
            Value::Double(2.5),
        ),
        (Kind::Mul, vec![b(true), b(false)], Value::Long(0)),
        (Kind::Mul, vec![Value::Long(3), b(true)], Value::Long(3)),
        (
            Kind::Mul,
            vec![Value::Long(3), Value::Double(0.5)],
            Value::Double(1.5),
        ),
        (
            Kind::Mul,
            vec![Value::Double(0.5), b(false)],
            Value::Double(0.0),
        ),
        (Kind::Mul, doubles(&[0.5, 4.0]), Value::Double(2.0)),
        (
            Kind::Mul,
            vec![Value::Double(0.5), Value::Long(4)],
            Value::Double(2.0),
        ),
        (Kind::Mul, longs(&[i64::MAX, 2]), Value::Long(-2)),
        // Values a series cannot hold are skipped.
        (
            Kind::Add,
            vec![Value::Long(1), Value::list(vec![])],
            Value::Long(1),
        ),
    ] {
        assert_eq!(value(kind, true, &values), expected, "{kind:?} {values:?}");
    }
    // Nulls: forbidden (no result) or ignored.
    assert_eq!(
        value(Kind::Add, true, &[Value::Long(1), Value::Null]),
        Value::Null
    );
    assert_eq!(
        value(Kind::Mul, false, &[Value::Null, Value::Long(3)]),
        Value::Long(3)
    );
    assert_eq!(value(Kind::Add, false, &[Value::Null]), Value::Null);
    for (kind, acc, message) in [
        (Kind::Add, Value::Long(1), "Cannot add strings and longs."),
        (
            Kind::Add,
            Value::Double(1.0),
            "Cannot add strings and doubles.",
        ),
        (Kind::Mul, b(true), "Cannot multiply strings and booleans."),
    ] {
        assert_eq!(
            Window::of(&[acc, Value::string("s")]).error(&op(kind, true)),
            message
        );
    }
}

#[test]
fn differences_and_quotients() {
    assert_eq!(
        value(Kind::Sub, true, &longs(&[i64::MIN, 1])),
        Value::Long(i64::MAX)
    );
    assert_eq!(
        value(Kind::Sub, true, &[Value::Long(1), Value::Double(0.5)]),
        Value::Double(0.5)
    );
    assert_eq!(value(Kind::Div, true, &longs(&[1, 4])), Value::Double(0.25));
    assert_eq!(value(Kind::Sub, true, &longs(&[1, 2, 3])), Value::Null);
    assert_eq!(
        value(Kind::Div, true, &[Value::Null, Value::Long(1)]),
        Value::Null
    );
    assert_eq!(
        value(Kind::Div, true, &[Value::Long(1), Value::Null]),
        Value::Null
    );
    assert_eq!(
        Window::of(&[Value::Long(1), b(true)]).error(&op(Kind::Div, true)),
        "op.x can only be applied to LONG or DOUBLE values."
    );
}

#[test]
fn masks() {
    let window = Window {
        ticks: vec![0, 1],
        locations: vec![11, 12],
        elevations: vec![21, 22],
        values: vec![b(true), Value::Long(5)],
    };
    let aggregated = window.apply(&op(Kind::Mask, false)).unwrap();
    assert_eq!(
        (aggregated.location, aggregated.elevation, aggregated.value),
        (12, 22, Value::Long(5))
    );
    let aggregated = window.apply(&op(Kind::Mask, true)).unwrap();
    assert_eq!(aggregated.value, Value::Null);
    assert_eq!(aggregated.location, crate::gts::NO_LOCATION);
    // A non boolean mask is not `true`.
    let window = Window {
        values: vec![Value::Long(1), Value::Long(5)],
        ..window
    };
    assert_eq!(window.value(&op(Kind::Mask, true)), Value::Long(5));
    let window = Window {
        ticks: vec![1, 1],
        ..window
    };
    assert_eq!(
        window.error(&op(Kind::Mask, false)),
        "op.mask can only be applied to pairs of Geo Time Series, the first one being boolean."
    );
}

#[test]
fn comparisons() {
    for (kind, values, expected) in [
        (Kind::Eq, longs(&[1, 1, 1]), true),
        (Kind::Eq, vec![Value::Long(1), Value::Double(1.0)], false),
        (Kind::Eq, doubles(&[f64::NAN, f64::NAN]), true),
        (Kind::Eq, doubles(&[0.0, -0.0]), false),
        (Kind::Eq, vec![Value::Long(1), Value::Null], false),
        (Kind::Eq, vec![], true),
        (Kind::Ne, longs(&[1, 2, 3]), true),
        (Kind::Ne, longs(&[1, 2, 1]), false),
        (Kind::Ne, vec![Value::Long(1), Value::Null], false),
        (Kind::Lt, longs(&[1, 2, 3]), true),
        (Kind::Lt, longs(&[1, 1]), false),
        (Kind::Le, longs(&[1, 1, 2]), true),
        (Kind::Le, longs(&[2, 1]), false),
        (Kind::Gt, longs(&[3, 2]), true),
        (Kind::Gt, longs(&[2, 2]), false),
        (Kind::Ge, longs(&[2, 2, 1]), true),
        (Kind::Ge, longs(&[1, 2]), false),
        // `NaN` never breaks the order, as Java tests `a >= b`…
        (Kind::Lt, doubles(&[f64::NAN, 1.0]), true),
        (Kind::Ge, doubles(&[1.0, f64::NAN]), true),
        (Kind::Lt, doubles(&[1.0, 0.5]), false),
        (Kind::Lt, vec![Value::string("a"), Value::string("b")], true),
        (
            Kind::Gt,
            vec![Value::string("a"), Value::string("b")],
            false,
        ),
        (Kind::Lt, vec![Value::Null, Value::Long(1)], false),
        (Kind::Lt, vec![Value::Long(1), Value::Null], false),
        (Kind::Lt, longs(&[1]), true),
        (Kind::And, vec![b(true), b(true)], true),
        (Kind::And, vec![b(true), b(false)], false),
        (Kind::Or, vec![b(false), b(true)], true),
        (Kind::Or, vec![b(false), Value::Long(1)], false),
    ] {
        assert_eq!(
            value(kind, true, &values),
            b(expected),
            "{kind:?} {values:?}"
        );
    }
    assert_eq!(value(Kind::And, true, &[b(true), Value::Null]), Value::Null);
    assert_eq!(value(Kind::Or, false, &[Value::Null, b(false)]), b(false));
    assert_eq!(
        Window::of(&[Value::Long(1), Value::Double(2.0)]).error(&op(Kind::Gt, true)),
        "op.x can only be applied to Geo Time Series of the same type."
    );
    let op = op(Kind::Eq, true);
    assert_eq!(op.roles(), Roles::NONE);
    assert!(op.nary());
}

#[test]
fn registration() {
    use crate::tests::assert_top;
    for (name, _) in super::ops::FUNCTIONS {
        assert_top(&format!("{name} TYPEOF"), "AGGREGATOR");
        assert_top(&format!("{name} SNAPSHOT"), format!("'{name}' FUNCREF "));
        let object = crate::tests::top(name);
        assert!(super::aggregator_of(&object).unwrap().nary(), "{name}");
    }
}
