// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the regular expression mappers. The expected values were checked against
//! Warp 10 3.5.0 (`warp10io/warp10:3.5.0-alpine`).

use super::pattern::{RegExpMatch, RegExpReplace};
use super::tests::Window;
use super::{Aggregated, Aggregator, Roles};
use crate::functions::strings::CompiledPattern;
use crate::tests::{assert_fails, assert_top, fail};
use crate::value::Value;

const STRINGS: &str = "NEWGTS 1 10 20 NaN 'abc' ADDVALUE 2 NaN NaN NaN 'xbz' ADDVALUE \
    3 30 40 7 'ab' ADDVALUE";

/// The ticks, values, latitudes and longitudes of the strings mapped with `parameters`.
fn mapped(parameters: &str) -> String {
    format!(
        "[ {STRINGS} {parameters} ] MAP 0 GET 'r' STORE \
         $r TICKLIST $r VALUES $r LOCATIONS 4 ->LIST ->JSON"
    )
}

#[test]
fn matches() {
    assert_top(
        &mapped("'a.c|x.*' mapper.regexp.match 1 0 0"),
        r#"[[1,2,3],["abc","abc","xbz"],[9.999999990686774,9.999999990686774,NaN],[19.99999998137355,19.99999998137355,NaN]]"#,
    );
    // `Matcher.matches()` backtracks into the alternation.
    assert_top(
        &mapped("'a|ab' mapper.regexp.match 0 1 0"),
        r#"[[2,3],["ab","ab"],[29.999999972060323,29.999999972060323],[39.9999999627471,39.9999999627471]]"#,
    );
    let message =
        fail("[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE '1' mapper.regexp.match 0 0 0 ] MAP 0 GET VALUES")
            .to_string();
    assert!(
        message.contains("(class java.lang.Long cannot be cast to class java.lang.String)"),
        "{message}"
    );
    let matcher = RegExpMatch {
        pattern: CompiledPattern::compile("a").expect("compile should succeed"),
    };
    assert_eq!(matcher.roles(), Roles::MAPPER);
    assert_eq!(
        Window::of(&[])
            .apply(&matcher)
            .expect("apply should succeed"),
        Aggregated {
            tick: 100,
            ..Aggregated::none()
        }
    );
    assert_eq!(
        Window::of(&[Value::Null]).error(&matcher),
        "NullPointerException"
    );
    // The backtracking limit of the engine is reported as an error.
    let catastrophic = RegExpMatch {
        pattern: CompiledPattern::compile("(a|a)*\\1b").expect("compile should succeed"),
    };
    assert!(
        Window::of(&[Value::string("a".repeat(46))])
            .error(&catastrophic)
            .contains("Max limit for backtracking count exceeded")
    );
}

#[test]
fn replacements() {
    assert_top(
        &mapped("'b(.)' '<$1>' mapper.regexp.replace 0 0 0"),
        r#"[[1,2,3],["a<c>","x<z>","ab"],[9.999999990686774,NaN,29.999999972060323],[19.99999998137355,NaN,39.9999999627471]]"#,
    );
    // The replacement errors are runtime exceptions, which `MAP` lets through.
    let err = fail(&mapped("'b' '$2' mapper.regexp.replace 0 0 0"));
    let message = err.to_string();
    assert!(message.contains("(No group 2)"), "{message}");
    assert!(!message.contains("invalid parameters"), "{message}");
    assert_eq!(
        crate::error::compat::exception_class(err.root_cause()),
        "java.lang.IndexOutOfBoundsException"
    );
    let err = fail(&mapped("'b' '$x' mapper.regexp.replace 0 0 0"));
    assert!(
        err.to_string().contains("(Illegal group reference)"),
        "{err}"
    );
    assert_eq!(
        crate::error::compat::exception_class(err.root_cause()),
        "java.lang.IllegalArgumentException"
    );
    assert_fails(
        &mapped("'b' 'B' mapper.regexp.replace 1 0 0"),
        "MAP was given invalid parameters. (mapper.regexp.replace can only be applied to a single value.)",
    );
    let message =
        fail("[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE '1' '2' mapper.regexp.replace 0 0 0 ] MAP")
            .to_string();
    assert!(
        message.contains("(class java.lang.Long cannot be cast to class java.lang.String)"),
        "{message}"
    );
    let replace = RegExpReplace {
        name: "mapper.x",
        source: "a".into(),
        replacement: "b".into(),
    };
    assert_eq!(replace.roles(), Roles::MAPPER);
    assert_eq!(
        Window::of(&[])
            .apply(&replace)
            .expect("apply should succeed"),
        Aggregated {
            tick: 0,
            ..Aggregated::none()
        }
    );
}

#[test]
fn builders() {
    for (script, message) in [
        (
            "'(' mapper.regexp.match",
            "mapper.regexp.match expects a valid regular expression. (",
        ),
        (
            "1 mapper.regexp.match",
            "mapper.regexp.match expects a regular expression STRING.",
        ),
        (
            "'a' 1 mapper.regexp.replace",
            "mapper.regexp.replace expects a replacement STRING.",
        ),
        (
            "1 'a' mapper.regexp.replace",
            "mapper.regexp.replace expects a regular expression STRING.",
        ),
        (
            "'(' 'a' mapper.regexp.replace",
            "mapper.regexp.replace expects a valid regular expression. (",
        ),
        ("mapper.regexp.match", "Empty stack"),
        ("mapper.regexp.replace", "Empty stack"),
        ("'a' mapper.regexp.replace", "Empty stack"),
    ] {
        assert_fails(script, message);
    }
    for (script, snapshot) in [
        ("'a' mapper.regexp.match", "'mapper.regexp.match' FUNCREF "),
        (
            "'a' 'b' mapper.regexp.replace",
            "'mapper.regexp.replace' FUNCREF ",
        ),
    ] {
        assert_top(&format!("{script} SNAPSHOT"), snapshot);
        assert_top(&format!("{script} TYPEOF"), "AGGREGATOR");
    }
}
