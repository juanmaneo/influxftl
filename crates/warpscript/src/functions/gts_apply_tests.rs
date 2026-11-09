// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of `APPLY` / `PAPPLY`. The expected values were checked against Warp 10 3.5.0
//! (`warp10io/warp10:3.5.0-alpine`).

use crate::tests::{assert_fails, assert_top, run};
use crate::value::Value;

const A: &str = "NEWGTS 'a' RENAME { 'h' 'h1' 'dc' 'x' } RELABEL { 'k' 'v' } SETATTRIBUTES \
    1 NaN NaN NaN 1 ADDVALUE 3 NaN NaN NaN 5 ADDVALUE";
const B: &str = "NEWGTS 'b' RENAME { 'h' 'h2' 'dc' 'x' } RELABEL 2 NaN NaN NaN 2 ADDVALUE \
    3 NaN NaN NaN 7 ADDVALUE";
const C: &str = "NEWGTS 'c' RENAME { 'h' 'h1' 'dc' 'y' } RELABEL 1 NaN NaN NaN 3 ADDVALUE \
    3 NaN NaN NaN 4 ADDVALUE";
const D: &str = "NEWGTS 'd' RENAME { 'h' 'h2' 'dc' 'y' } RELABEL 1 NaN NaN NaN 10.5 ADDVALUE \
    2 NaN NaN NaN 20.5 ADDVALUE";
const BOOLEANS: &str = "NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE";
const TRUES: &str = "NEWGTS 1 NaN NaN NaN true ADDVALUE 3 NaN NaN NaN true ADDVALUE";

#[test]
fn applications() {
    for (script, expected) in [
        (
            format!("[ [ {A} ] [ {B} ] [] op.add ] APPLY"),
            r#"[{"c":"b","l":{"h":"h1","dc":"x"},"a":{},"la":0,"v":[[3,12]]}]"#,
        ),
        (
            format!("[ [ {A} {B} ] [ {C} {D} ] [ 'h' ] op.add ] APPLY"),
            r#"[{"c":"c","l":{"h":"h1"},"a":{},"la":0,"v":[[1,4],[3,9]]},{"c":"d","l":{"h":"h2"},"a":{},"la":0,"v":[[2,22.5]]}]"#,
        ),
        (
            format!("[ [ {A} {C} ] [ {B} {D} ] [ 'h' ] op.sub ] APPLY"),
            r#"[{"c":"c","l":{"h":"h1"},"a":{},"la":0,"v":[[1,-2],[3,1]]},{"c":"d","l":{"h":"h2"},"a":{},"la":0,"v":[[2,-18.5]]}]"#,
        ),
        (
            format!("[ [ {A} ] [ {C} {D} ] [ 'h' ] op.mul ] APPLY"),
            r#"[{"c":"d","l":{"h":"h2","dc":"y"},"a":{},"la":0,"v":[[1,10.5]]},{"c":"c","l":{"h":"h1","dc":"y"},"a":{},"la":0,"v":[[1,3],[3,20]]}]"#,
        ),
        (
            format!("[ [ {A} {B} ] [ {C} {D} ] NULL op.add ] APPLY"),
            r#"[{"c":"a","l":{"h":"h1","dc":"x"},"a":{},"la":0,"v":[[1,1],[3,5]]},{"c":"d","l":{"h":"h2","dc":"y"},"a":{},"la":0,"v":[[1,10.5],[2,20.5]]},{"c":"b","l":{"h":"h2","dc":"x"},"a":{},"la":0,"v":[[2,2],[3,7]]},{"c":"c","l":{"h":"h1","dc":"y"},"a":{},"la":0,"v":[[1,3],[3,4]]}]"#,
        ),
        (
            format!("[ [ {A} {B} ] [ {C} {D} ] [ 'h' ] op.add.ignore-nulls ] APPLY"),
            r#"[{"c":"c","l":{"h":"h1"},"a":{},"la":0,"v":[[1,4],[3,9]]},{"c":"d","l":{"h":"h2"},"a":{},"la":0,"v":[[1,10.5],[2,22.5],[3,7.0]]}]"#,
        ),
        (
            format!("[ [ {A} {B} ] [ {C} {D} ] [ 'h' ] op.add ] PAPPLY"),
            r#"{"{h=h1}":[{"c":"c","l":{"h":"h1"},"a":{},"la":0,"v":[[1,4],[3,9]]}],"{h=h2}":[{"c":"d","l":{"h":"h2"},"a":{},"la":0,"v":[[2,22.5]]}]}"#,
        ),
        (
            format!("[ [ {A} ] [ {D} ] [] op.div ] APPLY"),
            r#"[{"c":"d","l":{"h":"h1","dc":"x"},"a":{},"la":0,"v":[[1,0.09523809523809523]]}]"#,
        ),
        (
            format!("[ [ {BOOLEANS} ] [ {C} ] [] op.mask ] APPLY"),
            r#"[{"c":"c","l":{},"a":{},"la":0,"v":[[1,3]]}]"#,
        ),
        (
            format!("[ [ {BOOLEANS} ] [ {C} ] [] op.negmask ] APPLY"),
            r#"[{"c":"c","l":{},"a":{},"la":0,"v":[[3,4]]}]"#,
        ),
        (
            format!("[ [ {A} ] [ {C} ] [ {A} ] [] op.eq ] APPLY"),
            r#"[{"c":"a","l":{"h":"h1","dc":"x"},"a":{},"la":0,"v":[[1,false],[3,false]]}]"#,
        ),
        (
            format!("[ [ {A} ] [ {C} ] [] op.ne ] APPLY"),
            r#"[{"c":"c","l":{"h":"h1","dc":"x"},"a":{},"la":0,"v":[[1,true],[3,true]]}]"#,
        ),
        (
            format!("[ [ {A} ] [ {C} ] [] op.lt ] APPLY"),
            r#"[{"c":"c","l":{"h":"h1","dc":"x"},"a":{},"la":0,"v":[[1,true],[3,false]]}]"#,
        ),
        (
            format!("[ [ {BOOLEANS} ] [ {TRUES} ] [] op.and ] APPLY"),
            r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,true],[2,false]]}]"#,
        ),
        (
            format!("[ [ {BOOLEANS} ] [ {TRUES} ] [] op.or.ignore-nulls ] APPLY"),
            r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,true],[2,false],[3,true]]}]"#,
        ),
        (
            format!("[ [ NEWGTS 1 NaN NaN NaN 'x' ADDVALUE ] [ {A} ] [] op.add ] APPLY"),
            r#"[{"c":"a","l":{},"a":{},"la":0,"v":[[1,"x1"]]}]"#,
        ),
        (format!("[ [ ] [ {A} ] [] op.add ] APPLY"), "[]"),
        ("[ [ ] [ ] [] op.add ] APPLY".to_owned(), "[]"),
        ("[ [] [] op.add 3 ] APPLY".to_owned(), "[]"),
        ("[ [] NULL op.add ] APPLY".to_owned(), "[]"),
        (
            format!("[ [ {A} ] [ {B} ] [ {C} ] [] op.sub ] APPLY"),
            r#"[{"c":"c","l":{"h":"h1","dc":"x"},"a":{},"la":0,"v":[]}]"#,
        ),
        (
            format!("[ [ {A} ] [ {B} ] [] op.add ] APPLY 0 GET ATTRIBUTES"),
            "{}",
        ),
        // A series in several collections is partitioned once.
        (
            format!("{A} 'g' STORE [ [ $g $g ] [ $g ] [] op.add ] APPLY"),
            r#"[{"c":"a","l":{"h":"h1","dc":"x"},"a":{},"la":0,"v":[[1,2],[3,10]]}]"#,
        ),
        // Partitioning labels given as a set.
        (
            format!("[ [ {A} {B} ] [ {C} {D} ] ( 'h' ) op.add ] APPLY <% LABELS %> F LMAP"),
            r#"[{"h":"h1"},{"h":"h2"}]"#,
        ),
    ] {
        assert_top(&format!("{script} ->JSON"), expected);
    }
}

#[test]
fn validators() {
    assert_top(
        &format!(
            "[ [ {A} {B} ] [ {C} {D} ] [ 'h' ] op.add <% 'l' STORE 's' STORE $l 'h' GET 'h1' == %> ] APPLY ->JSON"
        ),
        r#"[{"c":"c","l":{"h":"h1"},"a":{},"la":0,"v":[[1,4],[3,9]]}]"#,
    );
    assert_top(
        &format!(
            "[ [ {A} {B} ] [ {C} ] [ 'h' ] op.add <% ->JSON SWAP <% <% NAME %> F LMAP %> F LMAP ->JSON \
             2 ->LIST 'x' STORE true %> ] APPLY DROP $x ->JSON"
        ),
        r#"["{\"h\":\"h2\",\"dc\":\"x\"}","[[\"b\"],[\"c\"]]"]"#,
    );
    // A validator leaving something else than `true` skips the class (and what it left
    // below stays on the stack).
    let stack = run(&format!("[ [ {A} ] [ {B} ] [] op.add <% 1 %> ] APPLY"));
    assert_eq!(stack.len(), 3);
    assert_eq!(stack[2], Value::list(vec![]));
    assert_fails(
        &format!("[ [ {A} ] [ {B} ] [] op.add <% 'boom' MSGFAIL %> ] APPLY"),
        "boom",
    );
    assert_fails(
        &format!("[ [ {A} ] [ {B} ] [] op.add <% DROP DROP %> ] APPLY"),
        "Empty stack",
    );
}

#[test]
fn bucketized_applications() {
    let bucketized = |series: &str, lastbucket: i64| {
        format!("[ [ {series} ] bucketizer.last {lastbucket} 1 0 ] BUCKETIZE 0 GET")
    };
    let buckets = "0 GET DUP BUCKETSPAN SWAP DUP LASTBUCKET SWAP BUCKETCOUNT 3 ->LIST ->JSON";
    for (first, second, expected) in [
        (bucketized(A, 4), bucketized(B, 6), "[1,6,6]"),
        // A series that is not bucketized before the bucketized ones is skipped.
        (B.to_owned(), bucketized(A, 4), "[1,4,4]"),
        // One after them cancels the bucketization.
        (bucketized(A, 4), B.to_owned(), "[0,0,0]"),
        (
            bucketized(A, 4),
            format!("[ [ {B} ] bucketizer.last 6 2 0 ] BUCKETIZE 0 GET"),
            "[0,0,0]",
        ),
        (
            format!("[ [ {A} ] bucketizer.last 4 2 0 ] BUCKETIZE 0 GET"),
            format!("[ [ {B} ] bucketizer.last 7 2 0 ] BUCKETIZE 0 GET"),
            "[0,0,0]",
        ),
    ] {
        assert_top(
            &format!("[ [ {first} ] [ {second} ] [] op.add ] APPLY {buckets}"),
            expected,
        );
    }
}

#[test]
fn apply_errors() {
    for (script, message) in [
        ("1 APPLY".to_owned(), "APPLY expects a list as input."),
        (
            "[ 1 2 ] APPLY".to_owned(),
            "APPLY expects at least 3 parameters.",
        ),
        (
            "[ 1 2 3 ] APPLY".to_owned(),
            "APPLY expects an operation in the parameter list.",
        ),
        (
            "[ op.add 1 2 ] APPLY".to_owned(),
            "APPLY expects a list of label names under the operation.",
        ),
        (
            "[ [] op.add 2 ] APPLY".to_owned(),
            "APPLY expects a list of label names under the operation.",
        ),
        (
            "[ [] 'a' op.add ] APPLY".to_owned(),
            "APPLY expects a list of label names under the operation.",
        ),
        (
            "[ [] [ 1 ] op.add ] APPLY".to_owned(),
            "APPLY expects a list of label names as penultimate parameter.",
        ),
        (
            "[ 1 [] op.add ] APPLY".to_owned(),
            "APPLY expects lists of Geo Time Series as first parameters.",
        ),
        (
            "[ [ 1 ] [] op.add ] APPLY".to_owned(),
            "APPLY expects lists of Geo Time Series as first parameters.",
        ),
        (
            format!("[ [ {A} ] [ {B} ] [] reducer.sum ] APPLY"),
            "APPLY expects an operation in the parameter list.",
        ),
        (
            format!("[ [ {A} {B} ] [] op.mask ] APPLY"),
            "op.mask can only be applied to pairs of Geo Time Series, the first one being boolean.",
        ),
        (
            format!("[ [ {A} ] [ {D} ] [] op.lt ] APPLY"),
            "op.lt can only be applied to Geo Time Series of the same type.",
        ),
        (
            format!("[ [ {A} ] [ NEWGTS 1 NaN NaN NaN 'x' ADDVALUE ] [] op.add ] APPLY"),
            "Cannot add strings and longs.",
        ),
        (
            format!("[ [ NEWGTS 1 NaN NaN NaN 'x' ADDVALUE ] [ {A} ] [] op.sub ] APPLY"),
            "op.sub can only be applied to LONG or DOUBLE values.",
        ),
        ("APPLY".to_owned(), "Empty stack"),
        (
            "[ [] [] op.add ] 1 PAPPLY".to_owned(),
            "PAPPLY expects a list as input.",
        ),
    ] {
        assert_fails(&script, message);
    }
    // The class and labels ids of the series are unset.
    assert_top(
        &format!("{A} 'g' STORE [ [ $g ] [ $g ] [] op.add ] APPLY DROP $g ->JSON"),
        r#"{"c":"a","l":{"h":"h1","dc":"x"},"a":{"k":"v"},"la":0,"v":[[1,1],[3,5]]}"#,
    );
    assert_top("op.add TYPEOF", "AGGREGATOR");
    assert_top("op.add SNAPSHOT", "'op.add' FUNCREF ");
}
