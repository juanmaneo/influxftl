#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::functions::args::{
    getn, int_value, pop_bool, pop_list, pop_long, pop_macro, pop_map, pop_number, pop_string,
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
            .unwrap()
            .as_ref(),
        "x"
    );
    assert_eq!(
        pop_string(&mut stack, "F", "wants a string.")
            .unwrap_err()
            .to_string(),
        "F wants a string."
    );
    assert_eq!(
        pop_number(&mut stack, "F", "m").unwrap_err().to_string(),
        "F m"
    );
    assert_eq!(
        pop_number(&mut stack, "F", "m").unwrap_err().to_string(),
        "F m"
    );
    assert_eq!(
        pop_list(&mut stack, "F", "m").unwrap_err().to_string(),
        "F m"
    );
    assert_eq!(
        pop_bool(&mut stack, "F", "m").unwrap_err().to_string(),
        "F m"
    );
    assert_eq!(
        pop_macro(&mut stack, "F", "m").unwrap_err().to_string(),
        "F m"
    );
    assert_eq!(
        pop_map(&mut stack, "F", "m").unwrap_err().to_string(),
        "F m"
    );
    assert_eq!(
        pop_long(&mut stack, "F", "m").unwrap_err().to_string(),
        "Empty stack."
    );
    let mut stack = exec("<% %> [ ] { } 2.5 1 true");
    assert!(pop_bool(&mut stack, "F", "m").unwrap());
    assert_eq!(pop_long(&mut stack, "F", "m").unwrap(), 1);
    assert_eq!(
        pop_number(&mut stack, "F", "m").unwrap(),
        Value::Double(2.5)
    );
    assert!(pop_map(&mut stack, "F", "m").is_ok());
    assert!(pop_list(&mut stack, "F", "m").is_ok());
    assert!(pop_macro(&mut stack, "F", "m").is_ok());
    let mut stack = exec("'s'");
    assert_eq!(
        pop_long(&mut stack, "F", "m").unwrap_err().to_string(),
        "F m"
    );
    let mut empty = Stack::new();
    assert!(pop_macro(&mut empty, "F", "m").is_err());
    assert!(pop_string(&mut empty, "F", "m").is_err());
    assert!(pop_bool(&mut empty, "F", "m").is_err());
    assert!(pop_list(&mut empty, "F", "m").is_err());
    assert!(pop_map(&mut empty, "F", "m").is_err());
    assert_eq!(
        pop_number(&mut empty, "F", "m").unwrap_err().to_string(),
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
    assert_eq!(getn(&mut stack).unwrap(), 2);
    let mut stack = Stack::new();
    assert_eq!(getn(&mut stack).unwrap_err().to_string(), "Empty stack.");
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
    call(&mut stack, "DUP").unwrap();
    assert_eq!(stack.depth(), 2);
    assert_eq!(
        call(&mut stack, "nope").unwrap_err().to_string(),
        "Unknown function 'nope'"
    );
    assert_eq!(
        call(&mut stack, "test.object").unwrap_err().to_string(),
        "Unknown function 'test.object'"
    );
}
