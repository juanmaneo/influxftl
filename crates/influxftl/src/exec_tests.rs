// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the execution of scripts.

use super::{ScriptError, execute};

#[test]
fn the_stack_comes_top_first() {
    let execution = execute("1 'two' [ 3 ]", None).expect("the script should run");
    assert_eq!(execution.json, r#"[[3],"two",1]"#);
    assert_eq!(execution.ops, 5);
}

#[test]
fn an_empty_stack_is_an_empty_array() {
    assert_eq!(execute("", None).expect("no script").json, "[]");
}

#[test]
fn a_failing_script_reports_its_line() {
    let err = execute("1\nDROP DROP", None).expect_err("the stack is empty");
    assert_eq!(err.line, Some(2));
    assert_eq!(
        err.message,
        "Exception at 'DROP =>DROP<=' in section [TOP] (Empty stack.)"
    );
    assert_eq!(err.to_string(), format!("Line #2: {}", err.message));
}

#[test]
fn a_script_left_open_has_no_line() {
    assert_eq!(
        execute("<% 1", None).expect_err("unbalanced"),
        ScriptError {
            line: None,
            message: "Unbalanced <% construct.".to_owned()
        }
    );
    assert_eq!(
        execute("<% 1", None).expect_err("unbalanced").to_string(),
        "Unbalanced <% construct."
    );
}

#[test]
fn a_context_that_is_not_a_line_is_part_of_the_message() {
    // The failure of a macro statement is wrapped in a context that is not a line marker.
    let err = execute("<% 1 DROP DROP %> EVAL", None).expect_err("the stack is empty");
    assert!(err.message.contains("Empty stack."), "{err}");
}

#[test]
fn a_stack_too_big_for_json_is_refused() {
    let err = execute(
        "'0123456789' 'x' STORE 0 20 <% DROP $x $x + 'x' STORE %> FOR $x",
        None,
    )
    .expect_err("20 MB of JSON");
    assert_eq!(err.message, "Resulting JSON is too big.");
}

#[test]
fn a_stack_that_contains_itself_is_refused() {
    let err = execute("[ ] 'a' STORE $a $a +! DROP $a", None).expect_err("a loop");
    assert_eq!(
        err.message,
        "The stack is too deep to be written as JSON, or it references itself."
    );
}
