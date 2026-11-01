#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Shared helpers for the unit tests of the crate (this file is excluded from coverage).

use crate::error::Error;
use crate::stack::{Limits, Stack};
use crate::value::Value;

/// A fresh stack with generous limits and the `limits` capability (so that tests may use
/// `MAXOPS`, `MAXJSON`… freely), like `MemoryWarpScriptStack.maxLimits()`.
pub(crate) fn test_stack() -> Stack {
    let mut stack = Stack::with_limits(Limits::unlimited());
    stack
        .capabilities_mut()
        .insert("limits".to_owned(), String::new());
    stack
}

/// Execute `script` on a fresh stack with generous limits and return the resulting stack.
pub(crate) fn exec(script: &str) -> Stack {
    let mut stack = test_stack();
    if let Err(err) = stack.exec_checked(script) {
        panic!("script failed: {err}\n{script}");
    }
    stack
}

/// Execute `script` and return the visible elements, bottom first.
pub(crate) fn run(script: &str) -> Vec<Value> {
    exec(script).elements().to_vec()
}

/// Execute `script` and return the top of the stack.
pub(crate) fn top(script: &str) -> Value {
    let mut stack = exec(script);
    stack.pop().expect("empty stack")
}

/// Execute `script`, which must fail, and return the error.
pub(crate) fn fail(script: &str) -> Error {
    let mut stack = test_stack();
    match stack.exec_checked(script) {
        Ok(()) => panic!("script did not fail:\n{script}"),
        Err(err) => err,
    }
}

/// Execute `script`, which must fail, and return the full error message.
pub(crate) fn fail_msg(script: &str) -> String {
    fail(script).to_string()
}

/// Assert that `script` fails with a message containing `needle`.
#[track_caller]
pub(crate) fn assert_fails(script: &str, needle: &str) {
    let message = fail_msg(script);
    assert!(
        message.contains(needle),
        "expected error containing {needle:?}, got {message:?}\n{script}"
    );
}

/// Assert that `script` leaves exactly `expected` on the stack (bottom first).
#[track_caller]
pub(crate) fn assert_stack(script: &str, expected: &[Value]) {
    let actual = run(script);
    assert_eq!(actual, expected, "{script}");
}

/// Assert that `script` leaves `expected` on top of the stack.
#[track_caller]
pub(crate) fn assert_top(script: &str, expected: impl Into<Value>) {
    let expected = expected.into();
    let actual = top(script);
    assert_eq!(actual, expected, "{script}");
}

#[test]
fn registry_has_names() {
    assert!(crate::functions::registered_names().contains(&"DUP"));
}

#[test]
fn version_is_the_crate_version() {
    assert_eq!(crate::version(), env!("CARGO_PKG_VERSION"));
}
