// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

use crate::functions::args::{
    compute_and_check_index, getn, int_value, pop_bool, pop_list, pop_long, pop_macro, pop_map,
    pop_number, pop_string,
};
use crate::functions::{Entry, call, lookup, registered_names, registry};
use crate::stack::Stack;
use crate::tests::exec;
use crate::value::Value;

#[test]
fn typed_pops() {
    let mut stack = exec("<% %> 's' 1 true [ ] { } 2.5 'x'");
    assert_eq!(
        pop_string(&mut stack, "F", "wants a string.")
            .expect("the value should succeed")
            .as_ref(),
        "x"
    );
    assert_eq!(
        pop_string(&mut stack, "F", "wants a string.")
            .expect_err("the value should fail")
            .to_string(),
        "F wants a string."
    );
    assert_eq!(
        pop_number(&mut stack, "F", "m")
            .expect_err("pop_number should fail")
            .to_string(),
        "F m"
    );
    assert_eq!(
        pop_number(&mut stack, "F", "m")
            .expect_err("pop_number should fail")
            .to_string(),
        "F m"
    );
    assert_eq!(
        pop_list(&mut stack, "F", "m")
            .expect_err("pop_list should fail")
            .to_string(),
        "F m"
    );
    assert_eq!(
        pop_bool(&mut stack, "F", "m")
            .expect_err("pop_bool should fail")
            .to_string(),
        "F m"
    );
    assert_eq!(
        pop_macro(&mut stack, "F", "m")
            .expect_err("pop_macro should fail")
            .to_string(),
        "F m"
    );
    assert_eq!(
        pop_map(&mut stack, "F", "m")
            .expect_err("pop_map should fail")
            .to_string(),
        "F m"
    );
    assert_eq!(
        pop_long(&mut stack, "F", "m")
            .expect_err("pop_long should fail")
            .to_string(),
        "Empty stack."
    );
    let mut stack = exec("<% %> [ ] { } 2.5 1 true");
    assert!(pop_bool(&mut stack, "F", "m").expect("pop_bool should succeed"));
    assert_eq!(
        pop_long(&mut stack, "F", "m").expect("pop_long should succeed"),
        1
    );
    assert_eq!(
        pop_number(&mut stack, "F", "m").expect("pop_number should succeed"),
        Value::Double(2.5)
    );
    assert!(pop_map(&mut stack, "F", "m").is_ok());
    assert!(pop_list(&mut stack, "F", "m").is_ok());
    assert!(pop_macro(&mut stack, "F", "m").is_ok());
    let mut stack = exec("'s'");
    assert_eq!(
        pop_long(&mut stack, "F", "m")
            .expect_err("pop_long should fail")
            .to_string(),
        "F m"
    );
    let mut empty = Stack::new();
    assert!(pop_macro(&mut empty, "F", "m").is_err());
    assert!(pop_string(&mut empty, "F", "m").is_err());
    assert!(pop_bool(&mut empty, "F", "m").is_err());
    assert!(pop_list(&mut empty, "F", "m").is_err());
    assert!(pop_map(&mut empty, "F", "m").is_err());
    assert_eq!(
        pop_number(&mut empty, "F", "m")
            .expect_err("pop_number should fail")
            .to_string(),
        "Empty stack."
    );
}

#[test]
fn int_values_and_counts() {
    assert_eq!(int_value(&Value::Long(5)), Some(5));
    assert_eq!(int_value(&Value::Long(1 << 33)), Some(0));
    assert_eq!(int_value(&Value::Double(1e12)), Some(i32::MAX));
    assert_eq!(int_value(&Value::Double(-2.7)), Some(-2));
    assert_eq!(int_value(&Value::from("x")), None);
    let mut stack = exec("1 2 2.0");
    assert_eq!(getn(&mut stack).expect("getn should succeed"), 2);
    let mut stack = Stack::new();
    assert_eq!(
        getn(&mut stack).expect_err("getn should fail").to_string(),
        "Empty stack."
    );
}

#[test]
fn indices_are_checked_like_get() {
    assert_eq!(
        compute_and_check_index(0, 3).expect("compute_and_check_index should succeed"),
        0
    );
    assert_eq!(
        compute_and_check_index(2, 3).expect("compute_and_check_index should succeed"),
        2
    );
    assert_eq!(
        compute_and_check_index(-1, 3).expect("compute_and_check_index should succeed"),
        2
    );
    assert_eq!(
        compute_and_check_index(-3, 3).expect("compute_and_check_index should succeed"),
        0
    );
    assert_eq!(
        compute_and_check_index(3, 3)
            .expect_err("compute_and_check_index should fail")
            .to_string(),
        "Index out of bound, 3 >= 3"
    );
    assert_eq!(
        compute_and_check_index(-4, 3)
            .expect_err("compute_and_check_index should fail")
            .to_string(),
        "Index out of bound, -4 < -3"
    );
    assert_eq!(
        compute_and_check_index(i32::MIN, 5)
            .expect_err("the value should fail")
            .to_string(),
        "Index out of bound, -2147483648 < -5"
    );
}

#[test]
fn registry_lookup_and_call() {
    assert!(matches!(lookup("DUP"), Some(("DUP", Entry::Stack(_)))));
    assert!(matches!(lookup("test.object"), Some((_, Entry::Object(_)))));
    assert!(lookup("nope").is_none());
    assert!(registry().len() > 1000);
    let names = registered_names();
    assert!(names.windows(2).all(|w| w[0] < w[1]));
    let mut stack = exec("1");
    call(&mut stack, "DUP").expect("call should succeed");
    assert_eq!(stack.depth(), 2);
    assert_eq!(
        call(&mut stack, "nope")
            .expect_err("call should fail")
            .to_string(),
        "Unknown function 'nope'"
    );
    assert_eq!(
        call(&mut stack, "test.object")
            .expect_err("call should fail")
            .to_string(),
        "Unknown function 'test.object'"
    );
    // A builder is called with its registered name.
    assert!(matches!(
        lookup("bucketizer.var"),
        Some(("bucketizer.var", Entry::Builder(_)))
    ));
    let mut stack = exec("true");
    call(&mut stack, "bucketizer.var").expect("call should succeed");
    assert_eq!(
        stack.pop().expect("pop should succeed").type_name(),
        "AGGREGATOR"
    );
}
