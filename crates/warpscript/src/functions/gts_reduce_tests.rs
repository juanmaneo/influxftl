// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of `PARTITION`, `REDUCE` and the macro wrappers. The expected values were checked
//! against Warp 10 3.5.0 (`warp10io/warp10:3.5.0-alpine`); labels and attributes are listed
//! in insertion order (DIVERGENCES §18).

use crate::tests::{assert_fails, assert_top, fail};

const A: &str = "NEWGTS 'a' RENAME { 'host' 'h1' 'dc' 'x' } RELABEL { 'k' 'v' 'z' 'w' } SETATTRIBUTES \
    1 NaN NaN NaN 1 ADDVALUE 3 NaN NaN NaN 5 ADDVALUE";
const B: &str = "NEWGTS 'a' RENAME { 'host' 'h2' 'dc' 'x' } RELABEL { 'k' 'v' } SETATTRIBUTES \
    2 NaN NaN NaN 2 ADDVALUE 3 NaN NaN NaN 7 ADDVALUE";
const C: &str = "NEWGTS 'b' RENAME { 'host' 'h1' 'dc' 'y' } RELABEL 1 NaN NaN NaN 3 ADDVALUE";

const A_JSON: &str =
    r#"{"c":"a","l":{"host":"h1","dc":"x"},"a":{"k":"v","z":"w"},"la":0,"v":[[1,1],[3,5]]}"#;
const B_JSON: &str =
    r#"{"c":"a","l":{"host":"h2","dc":"x"},"a":{"k":"v"},"la":0,"v":[[2,2],[3,7]]}"#;
const C_JSON: &str = r#"{"c":"b","l":{"host":"h1","dc":"y"},"a":{},"la":0,"v":[[1,3]]}"#;

/// `script` followed by `->JSON`.
fn json(script: &str) -> String {
    format!("{script} ->JSON")
}

#[test]
fn reductions() {
    for (script, expected) in [
        (
            format!("[ [ {A} {B} {C} ] NULL reducer.sum ] REDUCE"),
            format!("[{B_JSON},{A_JSON},{C_JSON}]"),
        ),
        (
            format!("[ [ {A} {B} {C} ] [] reducer.sum ] REDUCE"),
            r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,4],[2,2],[3,12]]}]"#.to_owned(),
        ),
        (
            format!("[ [ {A} {B} ] [ {C} ] [ 'dc' ] reducer.sum ] REDUCE"),
            format!(
                r#"[{C_JSON},{{"c":"a","l":{{"dc":"x"}},"a":{{"k":"v"}},"la":0,"v":[[1,1],[2,2],[3,12]]}}]"#
            ),
        ),
        (
            format!("[ [ {A} {B} {C} ] [ 'dc' ] reducer.max ] PREDUCE"),
            format!(
                r#"{{"{{host=h1, dc=y}}":[{C_JSON}],"{{dc=x}}":[{{"c":"a","l":{{"dc":"x"}},"a":{{"k":"v"}},"la":0,"v":[[1,1],[2,2],[3,7]]}}]}}"#
            ),
        ),
        (
            "[ [ NEWGTS NEWGTS ] [] reducer.sum ] REDUCE".to_owned(),
            "[]".to_owned(),
        ),
        (
            format!("[ [ {A} {B} {C} ] [] reducer.min true ] REDUCE"),
            r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1],[2,2],[3,5]]}]"#.to_owned(),
        ),
        (
            format!("[ [ {A} {B} {C} ] [] reducer.sum false ] REDUCE"),
            r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,4],[2,2],[3,12]]}]"#.to_owned(),
        ),
    ] {
        assert_top(&json(&script), expected);
    }
    // A repeated tick is reduced once per occurrence.
    assert_top(
        "[ [ NEWGTS 1 NaN NaN NaN 1 ADDVALUE 1 NaN NaN NaN 2 ADDVALUE NEWGTS 1 NaN NaN NaN 10 ADDVALUE ] \
         [] reducer.sum ] REDUCE 0 GET DUP TICKLIST SWAP VALUES 2 ->LIST ->JSON",
        "[[1,1],[11,2]]",
    );
    // The input series are sorted in place.
    assert_top(
        "NEWGTS 3 NaN NaN NaN 1 ADDVALUE 1 NaN NaN NaN 2 ADDVALUE 'g' STORE \
         [ [ $g ] [] reducer.sum ] REDUCE DROP $g TICKLIST ->JSON",
        "[1,3]",
    );
    // A partitioning label the series do not have; attributes in common only when all the
    // series share them.
    assert_top(
        &json(&format!(
            "[ [ {A} {B} {C} ] [ 'nope' ] reducer.count ] REDUCE <% DUP LABELS SWAP ATTRIBUTES 2 ->LIST %> F LMAP"
        )),
        "[[{},{}]]",
    );
    assert_top(
        &json(&format!(
            "[ [ {A} NEWGTS {{ 'q' 'r' }} SETATTRIBUTES 1 NaN NaN NaN 1 ADDVALUE {B} ] [] reducer.count ] REDUCE 0 GET ATTRIBUTES"
        )),
        "{}",
    );
    assert_top(
        &json(&format!(
            "[ [ {A} {B} ] [] reducer.count ] REDUCE 0 GET ATTRIBUTES"
        )),
        r#"{"k":"v"}"#,
    );
    // A label set as the partitioning labels.
    assert_top(
        &json(&format!(
            "[ [ {A} {B} {C} ] ( 'dc' ) reducer.sum ] REDUCE <% LABELS %> F LMAP"
        )),
        r#"[{"host":"h1","dc":"y"},{"dc":"x"}]"#,
    );
}

#[test]
fn bucketized_reductions() {
    let bucketized = |series: &str, lastbucket: i64, span: i64| {
        format!("[ [ {series} ] bucketizer.last {lastbucket} {span} 0 ] BUCKETIZE 0 GET")
    };
    let buckets = "DUP BUCKETSPAN SWAP DUP LASTBUCKET SWAP BUCKETCOUNT 3 ->LIST ->JSON";
    for (first, second, expected) in [
        (bucketized(A, 4, 1), bucketized(B, 4, 1), "[1,4,4]"),
        // Different spans: not bucketized.
        (bucketized(A, 4, 1), bucketized(B, 5, 2), "[0,0,0]"),
        // Congruent last buckets: the result covers both.
        (bucketized(A, 4, 2), bucketized(B, 8, 2), "[2,8,4]"),
        // Incongruent last buckets.
        (bucketized(A, 4, 2), bucketized(B, 7, 2), "[0,0,0]"),
        // A series that is not bucketized.
        (bucketized(A, 4, 2), B.to_owned(), "[0,0,0]"),
    ] {
        assert_top(
            &format!("[ [ {first} {second} ] [] reducer.sum ] REDUCE 0 GET {buckets}"),
            expected,
        );
    }
}

#[test]
fn macro_reducers() {
    // The arguments of a macro reducer: the labels of the class come last, missing datapoints
    // have the tick `Long.MIN_VALUE` and a `null` value.
    assert_top(
        &json(&format!(
            "[ [ {A} {B} {C} ] [] <% ->JSON 'j' STORE [ 0 NaN NaN NaN $j ] %> MACROREDUCER ] REDUCE 0 GET VALUES"
        )),
        concat!(
            r#"["[1,[\"a\",\"a\",\"b\"],[{\"host\":\"h1\",\"dc\":\"x\"},{\"host\":\"h2\",\"dc\":\"x\"},{\"host\":\"h1\",\"dc\":\"y\"},{}],[1,-9223372036854775808,1],[NaN,NaN,NaN],[NaN,NaN,NaN],[NaN,NaN,NaN],[1,null,3]]","#,
            r#""[2,[\"a\",\"a\",\"b\"],[{\"host\":\"h1\",\"dc\":\"x\"},{\"host\":\"h2\",\"dc\":\"x\"},{\"host\":\"h1\",\"dc\":\"y\"},{}],[-9223372036854775808,2,-9223372036854775808],[NaN,NaN,NaN],[NaN,NaN,NaN],[NaN,NaN,NaN],[null,2,null]]","#,
            r#""[3,[\"a\",\"a\",\"b\"],[{\"host\":\"h1\",\"dc\":\"x\"},{\"host\":\"h2\",\"dc\":\"x\"},{\"host\":\"h1\",\"dc\":\"y\"},{}],[3,3,-9223372036854775808],[NaN,NaN,NaN],[NaN,NaN,NaN],[NaN,NaN,NaN],[5,7,null]]"]"#
        ),
    );
    assert_top(
        &json(&format!(
            "[ [ {A} {B} {C} ] [] <% DROP {{ 'y' [ 1 2 ] 'x' [ 3 ] }} %> MACROREDUCER ] REDUCE"
        )),
        r#"[{"c":"x","l":{},"a":{},"la":0,"v":[[1,3],[2,3],[3,3]]},{"c":"y","l":{},"a":{},"la":0,"v":[[1,2],[2,2],[3,2]]}]"#,
    );
    assert_top(
        &json(&format!(
            "[ [ {A} ] [] <% 0 GET [ SWAP 100 + 5 ] %> MACROREDUCER true ] REDUCE"
        )),
        r#"[{"c":"a","l":{"host":"h1","dc":"x"},"a":{"k":"v","z":"w"},"la":0,"v":[[101,5],[103,5]]}]"#,
    );
    // Named outputs also take the overridden tick; a `null` value adds no datapoint.
    assert_top(
        &json(&format!(
            "[ [ {A} ] [] <% 0 GET 't' STORE {{ 'o' [ $t 10 * $t 1 == <% NULL %> <% 1 %> IFTE ] }} %> MACROREDUCER true ] REDUCE"
        )),
        r#"[{"c":"o","l":{"host":"h1","dc":"x"},"a":{},"la":0,"v":[[30,1]]}]"#,
    );
    // A `null` value adds no datapoint; a failing macro fails the reduction.
    assert_top(
        &format!("[ [ {A} ] [] <% DROP [ NULL ] %> MACROREDUCER ] REDUCE 0 GET SIZE"),
        0,
    );
    assert_fails(
        &format!("[ [ {A} ] [] <% 'boom' MSGFAIL %> MACROREDUCER ] REDUCE"),
        "boom",
    );
    assert_fails(
        &format!("[ {A} <% 'boom' MSGFAIL %> MACROMAPPER 0 0 0 ] MAP"),
        "boom",
    );
    // The single-element list result has an `Integer` zero tick.
    assert_fails(
        &format!("[ [ {A} ] [] <% DROP [ 5 ] %> MACROREDUCER true ] REDUCE"),
        "class java.lang.Integer cannot be cast to class java.lang.Long",
    );
    assert_fails(
        &format!("[ [ {A} ] [] <% DROP {{ 'o' [ 5 ] }} %> MACROREDUCER true ] REDUCE"),
        "class java.lang.Integer cannot be cast to class java.lang.Long",
    );
    // The macro may reach the reduced series.
    assert_top(
        &format!(
            "{A} 'g' STORE [ [ $g ] [] <% DROP $g SORT [ 1 ] %> MACROREDUCER ] REDUCE 0 GET VALUES ->JSON"
        ),
        "[1,1]",
    );
}

#[test]
fn macro_wrappers_in_other_frameworks() {
    // `MAP`: one name and label map per datapoint of the window, plus the window parameters.
    assert_top(
        &json(&format!(
            "[ {A} <% 'p' STORE [ $p 0 GET NaN NaN NaN $p ->JSON ] %> MACROMAPPER 1 0 0 ] MAP 0 GET VALUES"
        )),
        concat!(
            r#"["[1,[\"a\"],[{\"host\":\"h1\",\"dc\":\"x\"}],[1],[NaN],[NaN],[NaN],[1]]","#,
            r#""[3,[\"a\",\"a\"],[{\"host\":\"h1\",\"dc\":\"x\"},{\"host\":\"h1\",\"dc\":\"x\"}],[1,3],[NaN,NaN],[NaN,NaN],[NaN,NaN],[1,5]]"]"#
        ),
    );
    // `BUCKETIZE`: one name and label map per bucket.
    assert_top(
        &json(
            "[ NEWGTS 'a' RENAME 1 10.0 20.0 7 1 ADDVALUE 3 NaN NaN NaN 5 ADDVALUE \
             <% 'p' STORE [ $p 0 GET NaN NaN NaN $p ->JSON ] %> MACROBUCKETIZER 4 4 0 ] BUCKETIZE 0 GET VALUES",
        ),
        r#"["[4,[\"a\"],[{}],[1,3],[9.999999990686774,NaN],[19.99999998137355,NaN],[7,NaN],[1,5]]"]"#,
    );
    // Five values left on the stack, or a map of outputs.
    assert_top(
        &format!("[ {A} <% DROP 9 NaN NaN NaN 'v' %> MACROMAPPER 0 0 0 ] MAP 0 GET VALUES ->JSON"),
        r#"["v","v"]"#,
    );
    assert_top(
        &format!("[ {A} <% DROP {{ 'm' [ 1 2 ] }} %> MACROMAPPER 0 0 0 ] MAP 0 GET NAME"),
        "m",
    );
    let message = fail(&format!(
        "[ {A} <% DROP {{ 'm' [ 1 2 ] }} %> MACROBUCKETIZER 4 4 0 ] BUCKETIZE"
    ))
    .to_string();
    assert!(
        message
            .contains("class java.util.LinkedHashMap cannot be cast to class [Ljava.lang.Object;"),
        "{message}"
    );
    assert_top(
        &format!("[ {A} <% DROP [ NULL ] %> MACROBUCKETIZER 4 4 0 ] BUCKETIZE 0 GET SIZE"),
        0,
    );
    for name in ["MACROMAPPER", "MACROREDUCER", "MACROBUCKETIZER"] {
        assert_top(
            &format!("<% 1 %> {name} SNAPSHOT"),
            format!("<% <% 1 %> {name} %> EVAL "),
        );
        assert_top(&format!("<% 1 %> {name} TYPEOF"), "AGGREGATOR");
        let crate::value::Value::Function(wrapper) = crate::tests::top(&format!("<% 1 %> {name}"))
        else {
            panic!("{name}: not a function");
        };
        assert_eq!(wrapper.name(), name);
        assert_fails(
            &format!("1 {name}"),
            &format!("{name} expects a macro on top of the stack."),
        );
        assert_fails(name, "Empty stack");
    }
}

#[test]
fn reduce_errors() {
    for (script, message) in [
        ("1 REDUCE", "REDUCE expects a list as input."),
        ("[ 1 2 ] REDUCE", "REDUCE expects at least 3 parameters."),
        (
            "[ 1 [] reducer.sum ] REDUCE",
            "REDUCE expects lists of Geo Time Series as first parameter.",
        ),
        (
            "[ [ 1 ] [] reducer.sum ] REDUCE",
            "REDUCE expects lists of Geo Time Series as first parameter.",
        ),
        (
            "[ [] 'a' reducer.sum ] REDUCE",
            "REDUCE expects a list of label names or null as parameter number 2.",
        ),
        (
            "[ [] [ 1 ] reducer.sum ] REDUCE",
            "REDUCE expects a list of label names as parameter number 2.",
        ),
        (
            "[ [] [] mapper.abs ] REDUCE",
            "REDUCE expects a function as parameter number 3.",
        ),
        (
            "[ [] [] <% %> ] REDUCE",
            "REDUCE expects a function as parameter number 3.",
        ),
        // With the override, the labels are among the parameters that must be lists.
        (
            "[ [] NULL reducer.sum true ] REDUCE",
            "REDUCE expects lists of Geo Time Series as first parameter.",
        ),
        (
            "[ [] [] 1 true ] PREDUCE",
            "PREDUCE expects a function as parameter number 3.",
        ),
        ("REDUCE", "Empty stack"),
    ] {
        assert_fails(script, message);
    }
    assert_top("[ [] [] reducer.sum ] REDUCE ->JSON", "[]");
    assert_top("[ [] [] reducer.sum ] PREDUCE ->JSON", "{}");
    // A wrapper applied outside of the frameworks has no stack to run on.
    let wrapper = crate::tests::top("<% 1 %> MACROREDUCER");
    let aggregator =
        crate::aggregators::aggregator_of(&wrapper).expect("aggregator_of should succeed");
    assert_eq!(aggregator.roles(), crate::aggregators::Roles::ALL);
    let names = ["g".to_owned()];
    let labels = [indexmap::IndexMap::new()];
    let err = aggregator
        .apply(&crate::aggregators::AggregatorArgs {
            tick: 0,
            names: &names,
            labels: &labels,
            ticks: &[0],
            locations: &[0],
            elevations: &[0],
            values: &[crate::value::Value::Long(1)],
            window: None,
        })
        .expect_err("the value should fail");
    assert_eq!(
        err.to_string(),
        "MACROREDUCER can only be applied by BUCKETIZE, MAP or REDUCE."
    );
}

#[test]
fn partitions() {
    for (script, expected) in [
        (
            format!("[ {A} {B} {C} ] [ 'host' ] PARTITION"),
            format!(r#"{{"{{host=h2, dc=x}}":[{B_JSON}],"{{host=h1}}":[{A_JSON},{C_JSON}]}}"#),
        ),
        (
            format!("[ {A} {B} {C} ] [ 'host' ] STRICTPARTITION"),
            format!(r#"{{"{{host=h1}}":[{A_JSON},{C_JSON}],"{{host=h2}}":[{B_JSON}]}}"#),
        ),
        (
            format!("[ {A} [ {B} {C} ] 1 ] NULL PARTITION"),
            format!(
                r#"{{"{{host=h2, dc=x}}":[{B_JSON}],"{{host=h1, dc=x}}":[{A_JSON}],"{{host=h1, dc=y}}":[{C_JSON}]}}"#
            ),
        ),
        (
            format!("[ {A} {B} {C} ] NULL STRICTPARTITION"),
            format!(
                r#"{{"{{host=h2, dc=x}}":[{B_JSON}],"{{host=h1, dc=x}}":[{A_JSON}],"{{host=h1, dc=y}}":[{C_JSON}]}}"#
            ),
        ),
    ] {
        assert_top(&json(&script), expected);
    }
    for (script, message) in [
        (
            format!("[ {A} ] 'a' PARTITION"),
            "PARTITION expects a list of labels or null on the top of the stack. (class java.lang.String cannot be cast to class java.util.List)",
        ),
        (
            "1 NULL PARTITION".to_owned(),
            "PARTITION expects a list of Geo Time Series instances under the top of the stack. (class java.lang.Long cannot be cast to class java.util.List)",
        ),
        (
            "[ [ 1 ] ] NULL PARTITION".to_owned(),
            "PARTITION expects a list of Geo Time Series as first parameter.",
        ),
        ("NULL NULL PARTITION".to_owned(), "NullPointerException"),
        // The label names are only cast while partitioning a series.
        (
            format!("[ {A} ] [ 'host' 1 ] STRICTPARTITION"),
            "class java.lang.Long cannot be cast to class java.lang.String",
        ),
        (
            format!("[ {A} ] [ 1 ] PARTITION"),
            "class java.lang.Long cannot be cast to class java.lang.String",
        ),
        ("PARTITION".to_owned(), "Empty stack"),
        ("NULL PARTITION".to_owned(), "Empty stack"),
    ] {
        assert_fails(&script, message);
    }
    assert_top("[] [ 1 ] PARTITION ->JSON", "{}");
    assert_top("[] [ 1 ] STRICTPARTITION ->JSON", "{}");
    // Twenty classes: the order of Java's `HashMap`s of 32 buckets.
    let twenty = "[ 1 20 <% 'i' STORE NEWGTS { 'k' $i TOSTRING 'j' 'x' } RELABEL \
                  $i NaN NaN NaN $i ADDVALUE %> FOR ]";
    let by_k = r#"["19","14","13","12","11","18","17","16","15","9","8","10","20","3","2","1","7","6","5","4"]"#;
    assert_top(
        "[ [ 1 20 <% 'i' STORE NEWGTS { 'k' $i TOSTRING } RELABEL $i NaN NaN NaN $i ADDVALUE %> FOR ] \
         [ 'k' ] reducer.sum ] REDUCE <% LABELS 'k' GET %> F LMAP ->JSON",
        by_k,
    );
    let keys = "'m' STORE [] $m <% DROP 'k' GET +! %> FOREACH ->JSON";
    assert_top(&format!("{twenty} [ 'k' ] STRICTPARTITION {keys}"), by_k);
    assert_top(
        &format!("{twenty} [ 'k' ] PARTITION {keys}"),
        r#"["16","15","9","8","10","20","3","2","1","7","6","5","4","19","14","13","12","11","18","17"]"#,
    );
}
