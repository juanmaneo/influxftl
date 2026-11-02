// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of `MAXDEPTH`, `MAXOPS`, `MAXLOOP`, `MAXBUCKETS`, `MAXGEOCELLS`, `MAXPIXELS`,
//! `MAXRECURSION`, `MAXSYMBOLS`, `LIMIT`, `MAXGTS`, `MAXJSON` and `OPS`.

use crate::stack::{Limits, Stack};
use crate::tests::{assert_fails, assert_top, exec};
use crate::value::Value;

/// A stack with the default limits, hard limits `hard` and the capabilities `caps`.
fn stack_with(hard: Limits, caps: &[(&str, &str)]) -> Stack {
    let mut stack = Stack::with_limits(hard);
    stack.limits = Limits::default();
    for (name, value) in caps {
        stack
            .capabilities_mut()
            .insert((*name).to_owned(), (*value).to_owned());
    }
    stack
}

/// A reader of one soft limit.
type Read = fn(&Limits) -> i64;

/// The limit functions, their capability, and a reader of the soft limit they set.
const LIMIT_FUNCTIONS: &[(&str, &str, Read)] = &[
    ("MAXDEPTH", "maxdepth", |l| l.max_depth as i64),
    ("MAXOPS", "maxops", |l| l.max_ops as i64),
    ("MAXLOOP", "maxloop", |l| l.max_loop_duration),
    ("MAXBUCKETS", "maxbuckets", |l| l.max_buckets),
    ("MAXGEOCELLS", "maxgeocells", |l| l.max_geocells),
    ("MAXPIXELS", "maxpixels", |l| l.max_pixels),
    ("MAXRECURSION", "maxrecursion", |l| l.max_recursion as i64),
    ("MAXSYMBOLS", "maxsymbols", |l| l.max_symbols as i64),
    ("LIMIT", "limit", |l| l.fetch_limit),
    ("MAXGTS", "maxgts", |l| l.gts_limit),
    ("MAXJSON", "maxjson", |l| l.max_json),
];

#[test]
fn limits_are_set_with_the_limits_capability() {
    for (name, _, read) in LIMIT_FUNCTIONS {
        let stack = exec(&format!("500 {name}"));
        assert_eq!(read(&stack.limits), 500, "{name}");
        // The hard limits (the defaults here) cannot be exceeded without a capability value.
        let mut stack = stack_with(Limits::default(), &[("limits", "")]);
        let err = stack
            .exec_checked(&format!("{} {name}", 1i64 << 40))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(&format!("{name} cannot extend limit past ")),
            "{err}"
        );
    }
}

#[test]
fn limits_require_a_capability() {
    for (name, cap, read) in LIMIT_FUNCTIONS {
        let mut stack = stack_with(Limits::default(), &[]);
        let err = stack
            .exec_checked(&format!("5 {name}"))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(&format!("{name} missing capability '{cap}' or 'limits'.")),
            "{err}"
        );
        // The limit's own capability caps the value.
        let mut stack = stack_with(Limits::default(), &[(cap, "2000")]);
        stack.exec_checked(&format!("2000 {name}")).unwrap();
        assert_eq!(read(&stack.limits), 2000, "{name}");
        let err = stack
            .exec_checked(&format!("2001 {name}"))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(&format!("{name} cannot extend limit past 2000")),
            "{err}"
        );
        let mut stack = stack_with(Limits::default(), &[(cap, "many")]);
        let err = stack
            .exec_checked(&format!("5 {name}"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("Error parsing capability."), "{err}");
        let expects = if *name == "LIMIT" {
            "LIMIT expects a numeric (LONG) limit."
        } else {
            "expects a numeric (long) limit."
        };
        assert_fails(&format!("'5' {name}"), expects);
        assert_fails(name, "Empty stack.");
    }
}

#[test]
fn int_limits_are_truncated_like_java() {
    // `(int) limit`: the low 32 bits; a negative value lets nothing through.
    let stack = exec(&format!("{} MAXSYMBOLS", (1i64 << 32) + 7));
    assert_eq!(stack.limits.max_symbols, 7);
    let stack = exec("-1 MAXRECURSION");
    assert_eq!(stack.limits.max_recursion, 0);
    let stack = exec("-5 MAXOPS");
    assert_eq!(stack.limits.max_ops, 0);
    assert_fails("-1 MAXOPS 1", "Operation count (3) exceeded maximum of 0.");
}

#[test]
fn maxdepth_checks_the_current_depth() {
    assert_top("1 2 3 4 MAXDEPTH DEPTH", 3);
    assert_fails(
        "1 2 3 3 MAXDEPTH DEPTH",
        "Stack depth would exceed set limit of 3.",
    );
    assert_fails(
        "1 2 3 2 MAXDEPTH",
        "MAXDEPTH failed. (The stack depth is over the requested maximum depth.)",
    );
    // Hidden elements count.
    assert_fails(
        "1 2 3 NULL HIDE DROP 2 MAXDEPTH",
        "MAXDEPTH failed. (The stack depth is over the requested maximum depth.)",
    );
    assert_fails(
        "2 MAXDEPTH 1 2 3",
        "Stack depth would exceed set limit of 2.",
    );
}

#[test]
fn ops_counts_operations() {
    assert_top("OPS", 1);
    assert_top("1 2 OPS", 3);
    let mut stack = exec("1 2 3");
    stack.set_ops(42);
    stack.exec_checked("OPS").unwrap();
    assert_eq!(stack.pop().unwrap(), Value::Long(43));
}
