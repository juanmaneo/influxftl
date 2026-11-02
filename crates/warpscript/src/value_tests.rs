// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};

use super::bitset::BitSet;
use super::macros::Macro;
use super::value::{Context, FunctionObject, Value, double_to_long};

fn bitset(bytes: &[u8]) -> Value {
    Value::BitSet(Rc::new(BitSet::from_bytes(bytes)))
}

struct Dummy;

impl FunctionObject for Dummy {
    fn name(&self) -> &'static str {
        "dummy.fn"
    }

    fn type_name(&self) -> &'static str {
        "OPERATOR"
    }
}

fn hash_of(value: &Value) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn context() -> Value {
    Value::Context(Rc::new(Context {
        symbols: IndexMap::new(),
        registers: Vec::new(),
        defined: IndexMap::new(),
    }))
}

#[test]
fn constructors_and_type_names() {
    assert_eq!(Value::Null.type_name(), "NULL");
    assert_eq!(Value::from(true).type_name(), "BOOLEAN");
    assert_eq!(Value::from(1i64).type_name(), "LONG");
    assert_eq!(Value::from(1.0).type_name(), "DOUBLE");
    assert_eq!(Value::from("s").type_name(), "STRING");
    assert_eq!(Value::from(String::from("s")).type_name(), "STRING");
    assert_eq!(Value::string("s").type_name(), "STRING");
    assert_eq!(Value::bytes([1u8]).type_name(), "BYTES");
    assert_eq!(bitset(&[1]).type_name(), "BITSET");
    assert_eq!(Value::from(vec![Value::Null]).type_name(), "LIST");
    assert_eq!(Value::map(IndexMap::new()).type_name(), "MAP");
    assert_eq!(Value::set(IndexSet::new()).type_name(), "SET");
    assert_eq!(Value::from(Rc::new(Macro::new())).type_name(), "MACRO");
    assert_eq!(Value::Function(Rc::new(Dummy)).type_name(), "OPERATOR");
    assert_eq!(Value::Mark.type_name(), "MARK");
    assert_eq!(context().type_name(), "CONTEXT");
}

#[test]
fn accessors() {
    assert!(Value::Long(1).is_number());
    assert!(Value::Double(1.0).is_number());
    assert!(!Value::Null.is_number());
    assert!(Value::Null.is_null());
    assert!(!Value::Long(0).is_null());
    assert_eq!(Value::Long(3).as_f64(), Some(3.0));
    assert_eq!(Value::Double(2.5).as_f64(), Some(2.5));
    assert_eq!(Value::from("x").as_f64(), None);
    assert_eq!(Value::Long(3).as_i64(), Some(3));
    assert_eq!(Value::Double(2.9).as_i64(), Some(2));
    assert_eq!(Value::Double(f64::NAN).as_i64(), Some(0));
    assert_eq!(Value::Double(1e30).as_i64(), Some(i64::MAX));
    assert_eq!(Value::from("x").as_i64(), None);
    assert_eq!(Value::from("x").as_str(), Some("x"));
    assert_eq!(Value::Long(1).as_str(), None);
    assert_eq!(Value::from(true).as_bool(), Some(true));
    assert_eq!(Value::Long(1).as_bool(), None);
    assert_eq!(double_to_long(-1e30), i64::MIN);
}

#[test]
fn equality_follows_java() {
    assert_eq!(Value::Null, Value::Null);
    assert_eq!(Value::Mark, Value::Mark);
    assert_eq!(Value::from(true), Value::from(true));
    assert_ne!(Value::from(true), Value::from(false));
    assert_eq!(Value::Long(1), Value::Long(1));
    assert_ne!(Value::Long(1), Value::Double(1.0));
    assert_eq!(Value::Double(f64::NAN), Value::Double(f64::NAN));
    assert_ne!(Value::Double(0.0), Value::Double(-0.0));
    assert_eq!(Value::from("a"), Value::from("a"));
    assert_ne!(Value::from("a"), Value::from("b"));
    assert_eq!(Value::bytes([1u8]), Value::bytes([1u8]));
    assert_ne!(Value::bytes([1u8]), Value::bytes([2u8]));
    assert_eq!(
        Value::list(vec![Value::Long(1)]),
        Value::list(vec![Value::Long(1)])
    );
    assert_ne!(
        Value::list(vec![Value::Long(1)]),
        Value::list(vec![Value::Long(2)])
    );
    let list = Value::list(vec![Value::Long(1)]);
    assert_eq!(list, list.clone());
    let mut m1 = IndexMap::new();
    m1.insert(Value::from("k"), Value::Long(1));
    let map = Value::map(m1.clone());
    assert_eq!(map, Value::map(m1.clone()));
    assert_eq!(map, map.clone());
    assert_ne!(map, Value::map(IndexMap::new()));
    let set = Value::set(IndexSet::from([Value::Long(1)]));
    assert_eq!(set, Value::set(IndexSet::from([Value::Long(1)])));
    assert_eq!(set, set.clone());
    assert_ne!(set, Value::set(IndexSet::new()));
    let m = Value::from(Rc::new(Macro::new()));
    assert_eq!(m, m.clone());
    assert_ne!(m, Value::from(Rc::new(Macro::new())));
    let f = Value::Function(Rc::new(Dummy));
    assert_eq!(f, f.clone());
    assert_ne!(f, Value::Function(Rc::new(Dummy)));
    assert!(Dummy.as_any().is_none());
    assert_eq!(bitset(&[5, 0]), bitset(&[5]));
    assert_ne!(bitset(&[5]), bitset(&[4]));
    assert_ne!(bitset(&[5]), Value::bytes([5u8]));
    let c = context();
    assert_eq!(c, c.clone());
    assert_ne!(c, context());
    assert_ne!(Value::Null, Value::Long(0));
    assert_ne!(Value::from("1"), Value::Long(1));
}

#[test]
fn same_object_is_reference_identity() {
    let list = Value::list(Vec::new());
    assert!(list.same_object(&list.clone()));
    assert!(!list.same_object(&Value::list(Vec::new())));
    let map = Value::map(IndexMap::new());
    assert!(map.same_object(&map.clone()));
    let set = Value::set(IndexSet::new());
    assert!(set.same_object(&set.clone()));
    let s = Value::from("s");
    assert!(s.same_object(&s.clone()));
    assert!(!s.same_object(&Value::from("s")));
    let b = Value::bytes([1u8]);
    assert!(b.same_object(&b.clone()));
    assert!(!Value::Long(1).same_object(&Value::Long(1)));
    assert!(!list.same_object(&map));
}

#[test]
fn hashing_is_consistent_with_equality() {
    let values = [
        Value::Null,
        Value::Mark,
        Value::from(true),
        Value::Long(7),
        Value::Double(f64::NAN),
        Value::from("s"),
        Value::bytes([1u8, 2]),
        bitset(&[1, 2]),
        Value::list(vec![Value::Long(1)]),
        Value::map(IndexMap::from([(Value::from("k"), Value::Long(1))])),
        Value::set(IndexSet::from([Value::Long(1), Value::Long(2)])),
        Value::from(Rc::new(Macro::new())),
        Value::Function(Rc::new(Dummy)),
        context(),
    ];
    for value in &values {
        assert_eq!(hash_of(value), hash_of(&value.clone()));
    }
    assert_eq!(
        hash_of(&Value::Double(f64::NAN)),
        hash_of(&Value::Double(-f64::NAN))
    );
    assert_eq!(
        hash_of(&Value::set(IndexSet::from([
            Value::Long(1),
            Value::Long(2)
        ]))),
        hash_of(&Value::set(IndexSet::from([
            Value::Long(2),
            Value::Long(1)
        ]))),
        "set hashes are order independent"
    );
    let mut set = IndexSet::new();
    set.insert(Value::Double(f64::NAN));
    assert!(set.contains(&Value::Double(f64::NAN)));
}

#[test]
fn display_matches_java_to_string() {
    assert_eq!(Value::Null.to_string(), "null");
    assert_eq!(Value::from(true).to_string(), "true");
    assert_eq!(Value::Long(-3).to_string(), "-3");
    assert_eq!(Value::Double(1.0).to_string(), "1.0");
    assert_eq!(Value::from("s").to_string(), "s");
    assert_eq!(Value::bytes([1u8, 2, 3]).to_string(), "[B@3");
    assert_eq!(bitset(&[5]).to_string(), "{0, 2}");
    assert_eq!(
        Value::list(vec![Value::Long(1), Value::from("a")]).to_string(),
        "[1, a]"
    );
    assert_eq!(
        Value::set(IndexSet::from([Value::Long(1), Value::Long(2)])).to_string(),
        "[1, 2]"
    );
    assert_eq!(
        Value::map(IndexMap::from([
            (Value::from("a"), Value::Long(1)),
            (Value::from("b"), Value::Null)
        ]))
        .to_string(),
        "{a=1, b=null}"
    );
    assert_eq!(Value::from(Rc::new(Macro::new())).to_string(), "<% %>");
    assert_eq!(
        Value::Function(Rc::new(Dummy)).to_string(),
        "'dummy.fn' FUNCREF"
    );
    assert_eq!(Value::Mark.to_string(), "MARK");
    assert_eq!(context().to_string(), "CONTEXT");
}

#[test]
fn display_propagates_writer_failures() {
    use std::fmt::Write;

    use super::error_tests::FailAfter;

    let list = Value::list(vec![Value::Long(1), Value::Long(2)]);
    let set = Value::set(IndexSet::from([Value::Long(1), Value::Long(2)]));
    let map = Value::map(IndexMap::from([
        (Value::from("a"), Value::Long(1)),
        (Value::from("b"), Value::Long(2)),
    ]));
    for value in [list, set, map] {
        let outcomes: Vec<bool> = (0..16)
            .map(|n| write!(FailAfter(n), "{value}").is_ok())
            .collect();
        assert!(!outcomes[0]);
        assert!(outcomes[15]);
    }
}

#[test]
fn debug_is_type_tagged() {
    assert_eq!(format!("{:?}", Value::from("a")), "STRING(\"a\")");
    assert_eq!(format!("{:?}", Value::Double(2.0)), "DOUBLE(2.0)");
    assert_eq!(format!("{:?}", Value::Long(2)), "LONG(2)");
    assert_eq!(format!("{:?}", Value::Null), "NULL(null)");
}

#[test]
fn context_is_cloneable() {
    let Value::Context(ctx) = context() else {
        panic!("not a context");
    };
    let copy = (*ctx).clone();
    assert_eq!(copy.registers.len(), 0);
}
