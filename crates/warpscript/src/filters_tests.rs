// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of `FILTER` / `PFILTER` and the filters. The expected values were checked against
//! Warp 10 3.5.0 (`warp10io/warp10:3.5.0-alpine`, with the absent label support).

use crate::tests::{assert_fails, assert_top};

const A: &str = "NEWGTS 'cpu' RENAME { 'h' 'h1' } RELABEL { 'at' 'x' } SETATTRIBUTES \
    1 NaN NaN NaN 1 ADDVALUE 3 NaN NaN NaN 5 ADDVALUE";
const B: &str = "NEWGTS 'mem' RENAME { 'h' 'h2' } RELABEL 2 NaN NaN NaN 2.5 ADDVALUE";
const C: &str = "NEWGTS 'cpu2' RENAME { 'z' 'h1' } RELABEL 1 NaN NaN NaN 'abc' ADDVALUE \
    2 NaN NaN NaN 'b' ADDVALUE";
const NUMBERS: &str = "NEWGTS 'n' RENAME 1 NaN NaN NaN 4 ADDVALUE 2 NaN NaN NaN 7 ADDVALUE";
const DOUBLES: &str = "NEWGTS 'd' RENAME 1 NaN NaN NaN 2.5 ADDVALUE 2 NaN NaN NaN NaN ADDVALUE";

/// The names of the series of `series` retained by `filter`, as JSON.
fn filtered(series: &str, filter: &str) -> String {
    format!("[ [ {series} ] [] {filter} ] FILTER <% NAME %> F LMAP ->JSON")
}

#[test]
fn class_label_and_size_filters() {
    let abc = format!("{A} {B} {C}");
    for (filter, expected) in [
        ("'cpu' filter.byclass", r#"["cpu"]"#),
        ("'=cpu' filter.byclass", r#"["cpu"]"#),
        ("'~cpu.*' filter.byclass", r#"["cpu","cpu2"]"#),
        ("{ 'h' 'h1' } filter.bylabels", r#"["cpu"]"#),
        ("{ 'h' '~h.*' } filter.bylabels", r#"["cpu","mem"]"#),
        ("{ 'at' 'x' } filter.bylabels", "[]"),
        ("{ 'at' 'x' } filter.byattr", r#"["cpu"]"#),
        ("{ 'at' 'x' 'h' 'h1' } filter.bylabelsattr", r#"["cpu"]"#),
        ("{ 'at' '~y' } filter.byattr", "[]"),
        ("{ 'h' '' } filter.bylabels", r#"["cpu2"]"#),
        ("{ 'at' '=' } filter.bylabelsattr", r#"["mem","cpu2"]"#),
        ("{} filter.bylabels", r#"["cpu","mem","cpu2"]"#),
        ("1 1 filter.bysize", r#"["mem"]"#),
        ("2 5 filter.bysize", r#"["cpu","cpu2"]"#),
    ] {
        assert_top(&filtered(&abc, filter), expected);
    }
    // A series that was never named has the empty class name.
    assert_top("[ [ NEWGTS ] [] 'x' filter.byclass ] FILTER SIZE", 0);
    assert_top("[ [ NEWGTS ] [] '' filter.byclass ] FILTER SIZE", 1);
}

#[test]
fn value_filters() {
    for (series, filter, expected) in [
        (C, "'b' filter.last.eq", r#"["cpu2"]"#),
        (C, "'c' filter.last.lt", r#"["cpu2"]"#),
        (C, "'abc' filter.any.eq", r#"["cpu2"]"#),
        (C, "'a' filter.all.gt", r#"["cpu2"]"#),
        (C, "'b' filter.all.ge", "[]"),
        (NUMBERS, "7 filter.last.eq", r#"["n"]"#),
        (NUMBERS, "7 filter.last.ne", "[]"),
        (NUMBERS, "7 filter.last.ge", r#"["n"]"#),
        (NUMBERS, "7 filter.last.gt", "[]"),
        (NUMBERS, "7 filter.last.le", r#"["n"]"#),
        (NUMBERS, "8 filter.last.lt", r#"["n"]"#),
        (NUMBERS, "6.5 filter.last.gt", r#"["n"]"#),
        (NUMBERS, "5 filter.any.lt", r#"["n"]"#),
        (NUMBERS, "4 filter.any.lt", "[]"),
        (NUMBERS, "7 filter.all.le", r#"["n"]"#),
        (NUMBERS, "5 filter.all.ne", r#"["n"]"#),
        (NUMBERS, "4 filter.all.eq", "[]"),
        (NUMBERS, "4 filter.any.ne", r#"["n"]"#),
        (NUMBERS, "4 filter.all.lt", "[]"),
        (NUMBERS, "4 filter.any.ge", r#"["n"]"#),
        (NUMBERS, "8 filter.any.gt", "[]"),
        (NUMBERS, "3 filter.all.gt", r#"["n"]"#),
        (NUMBERS, "4 filter.any.le", r#"["n"]"#),
        (NUMBERS, "3 filter.any.le", "[]"),
        // `Double.compareTo`: `NaN` is greater than everything.
        (DOUBLES, "1e300 filter.last.gt", r#"["d"]"#),
        (DOUBLES, "NaN filter.last.eq", r#"["d"]"#),
        (NUMBERS, "'7' filter.last.eq", r#"["n"]"#),
    ] {
        assert_top(&filtered(series, filter), expected);
    }
    let booleans = "NEWGTS 'b' RENAME 1 NaN NaN NaN true ADDVALUE";
    assert_top(&filtered(booleans, "true filter.last.eq"), r#"["b"]"#);
    assert_top(&filtered(booleans, "true filter.last.ne"), "[]");
    assert_top(&filtered(booleans, "true filter.all.ne"), "[]");
    assert_top(&filtered(booleans, "false filter.any.ne"), r#"["b"]"#);
    // The last value of a bucketized series is the one at its last bucket.
    for (lastbucket, filter, expected) in [
        (4, "5 filter.last.eq", "[]"),
        (4, "5 filter.last.ne", "[]"),
        (5, "5 filter.last.ne", "[]"),
        (3, "5 filter.last.eq", r#"["cpu"]"#),
    ] {
        assert_top(
            &format!(
                "[ [ {A} ] bucketizer.last {lastbucket} 1 0 ] BUCKETIZE 'b' STORE \
                 [ $b [] {filter} ] FILTER <% NAME %> F LMAP ->JSON"
            ),
            expected,
        );
    }
    // The most recent tick of an unsorted series.
    assert_top(
        &filtered(
            "NEWGTS 'u' RENAME 3 NaN NaN NaN 1 ADDVALUE 1 NaN NaN NaN 2 ADDVALUE",
            "1 filter.last.eq",
        ),
        r#"["u"]"#,
    );
    assert_top(&filtered("NEWGTS 'e' RENAME", "1 filter.last.ne"), "[]");
    for filter in ["5 filter.last.eq", "2 filter.any.eq"] {
        assert_fails(
            &filtered(C, filter),
            "class java.lang.String cannot be cast to class java.lang.Number",
        );
    }
    assert_fails(
        &filtered(booleans, "2.5 filter.any.eq"),
        "class java.lang.Boolean cannot be cast to class java.lang.Number",
    );
}

#[test]
fn macro_filters_and_partitions() {
    assert_top(
        &format!(
            "[ [ {A} {B} ] [ {C} ] [] <% 1 GET SWAP DROP %> MACROFILTER ] FILTER <% NAME %> F LMAP ->JSON"
        ),
        r#"["cpu2"]"#,
    );
    assert_top(
        &format!(
            "[ [ {A} {B} ] [] <% DROP 'labels' STORE [ $labels ] %> MACROFILTER ] FILTER ->JSON"
        ),
        "[{}]",
    );
    assert_top(
        &format!("[ [ {A} ] [] <% DROP DROP [ 1 ] %> MACROFILTER ] FILTER ->JSON"),
        "[1]",
    );
    assert_fails(
        &format!("[ [ {A} ] [] <% DROP 0 GET %> MACROFILTER ] FILTER"),
        "MACROFILTER macro should return a list of Geo Time Series onto the stack.",
    );
    assert_fails(
        &format!("[ [ {A} ] [] <% 'boom' MSGFAIL %> MACROFILTER ] FILTER"),
        "boom",
    );
    assert_fails(
        &format!("[ [ {A} ] [] <% DROP DROP %> MACROFILTER ] FILTER"),
        "Empty stack",
    );
    assert_top(
        &format!("[ [ {A} {B} ] [ {C} ] [ 'h' ] 'cpu' filter.byclass ] PFILTER ->JSON"),
        r#"{"{h=h1}":[{"c":"cpu","l":{"h":"h1"},"a":{"at":"x"},"la":0,"v":[[1,1],[3,5]]}],"{h=h2}":[]}"#,
    );
    assert_top(
        &format!("[ [ {A} {B} ] ( 'h' ) 'mem' filter.byclass ] FILTER <% NAME %> F LMAP ->JSON"),
        r#"["mem"]"#,
    );
}

#[test]
fn builders_and_errors() {
    for (filter, snapshot) in [
        ("'=a' filter.byclass", "'%3Da' filter.byclass "),
        (
            "{ 'a' '~b.*' 'c' 'd' } filter.bylabels",
            "{ 'a' '%7Eb.*''c' 'd'} filter.bylabels ",
        ),
        ("{} filter.byattr", "{ } filter.byattr "),
        ("1 2 filter.bysize", "1 2 filter.bysize "),
        ("4294967297 -1 filter.bysize", "1 -1 filter.bysize "),
        ("5 filter.last.eq", "5 filter.last.eq "),
        ("'x' filter.any.lt", "'x' filter.any.lt "),
        ("true filter.all.eq", "true filter.all.eq "),
        ("<% %> MACROFILTER", "<% <% %> MACROFILTER %> EVAL "),
    ] {
        assert_top(&format!("{filter} SNAPSHOT"), snapshot);
        assert_top(&format!("{filter} TYPEOF"), "FILTER");
    }
    for (script, message) in [
        ("true filter.last.gt", "Invalid threshold type."),
        ("[] filter.last.eq", "Invalid threshold type."),
        (
            "true filter.any.gt",
            "filter.any.gt threshold type is invalid.",
        ),
        (
            "[] filter.all.eq",
            "filter.all.eq threshold type is invalid.",
        ),
        ("'~(' filter.byclass", "Parsing error"),
        ("1 filter.byclass", "Invalid class selector"),
        ("1 filter.bylabels", "Invalid labels/attributes selector"),
        (
            "{ 'a' 1 } filter.bylabels",
            "class java.lang.Long cannot be cast to class java.lang.String",
        ),
        (
            "{ 1 'a' } filter.bylabels",
            "class java.lang.Long cannot be cast to class java.lang.String",
        ),
        ("{ 'a' '~(' } filter.bylabels", "Parsing error"),
        (
            "1 'a' filter.bysize",
            "filter.bysize expects two LONG objects as arguments.",
        ),
        (
            "1 MACROFILTER",
            "MACROFILTER expects a macro on top of the stack.",
        ),
        ("1 FILTER", "FILTER expects a list as input."),
        ("[ 1 2 ] FILTER", "FILTER expects at least 3 parameters."),
        (
            "[ 1 [] 'a' filter.byclass ] FILTER",
            "FILTER expects lists of Geo Time Series as first parameters.",
        ),
        (
            "[ [] 'a' 'a' filter.byclass ] FILTER",
            "FILTER expects a list of label names or null as penultimate parameter.",
        ),
        (
            "[ [] [ 1 ] 'a' filter.byclass ] FILTER",
            "FILTER expects a list of label names as penultimate parameter.",
        ),
        (
            "[ [] [] 1 ] FILTER",
            "FILTER expects a filter function as last parameter.",
        ),
        (
            "[ [ 1 ] [] 'a' filter.byclass ] FILTER",
            "FILTER expects lists of Geo Time Series as first parameters.",
        ),
        ("filter.bysize", "Empty stack"),
        ("1 filter.bysize", "Empty stack"),
        ("FILTER", "Empty stack"),
        ("filter.last.eq", "Empty stack"),
        ("filter.any.eq", "Empty stack"),
        ("filter.byclass", "Empty stack"),
        ("filter.bylabels", "Empty stack"),
        ("MACROFILTER", "Empty stack"),
    ] {
        assert_fails(script, message);
    }
    assert_fails(
        "[ [] [] reducer.sum ] FILTER",
        "FILTER expects a filter function as last parameter.",
    );
    // The backtracking limit of the regular expression engine.
    let catastrophic = "(a|a)*\\1b";
    let long_name = "a".repeat(46);
    assert_fails(
        &format!("[ [ NEWGTS '{long_name}' RENAME ] [] '~{catastrophic}' filter.byclass ] FILTER"),
        "Max limit for backtracking count exceeded",
    );
    assert_fails(
        &format!(
            "[ [ NEWGTS {{ 'a' '{long_name}' }} RELABEL ] [] {{ 'a' '~{catastrophic}' }} filter.bylabels ] FILTER"
        ),
        "Max limit for backtracking count exceeded",
    );
    let crate::value::Value::Function(function) = crate::tests::top("1 2 filter.bysize") else {
        panic!("not a function");
    };
    assert_eq!(function.name(), "filter.bysize");
    assert_top("[ [] NULL 'a' filter.byclass ] FILTER ->JSON", "[]");
    assert_top("[ [] [] 'a' filter.byclass ] PFILTER ->JSON", "{}");
    assert!(crate::filters::filter_of(&crate::value::Value::Long(1)).is_none());
}

#[test]
fn byselector_regex_failures() {
    // The backtracking limit of the regular expression engine, in the class, label and
    // attribute patterns of `filter.byselector` (Warp 10 would run for an unbounded time).
    let catastrophic = "(a|a)*\\1b";
    let long = "a".repeat(46);
    let series = format!(
        "NEWGTS '{long}' RENAME {{ 'l' '{long}' }} RELABEL {{ 'a' '{long}' }} SETATTRIBUTES"
    );
    for selector in [
        format!("~{catastrophic}{{}}"),
        format!("~.*{{l~{catastrophic}}}{{}}"),
        format!("~.*{{}}{{a~{catastrophic}}}"),
    ] {
        assert_fails(
            &format!("[ [ {series} ] [] '{selector}' filter.byselector ] FILTER"),
            "Max limit for backtracking count exceeded",
        );
    }
}
