// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! `GEO.GML`, `GEO.KML` (and their `.UNIFORM` forms), the GML and KML texts of `->WKT`,
//! `->WKB` and `->GEOJSON`, and the writers `->GML` and `->KML`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template.to_string()
}

/// The stack left by `script`, top first, as the JSON array of the Warp 10 exec endpoint,
/// or the error message.
fn stack_json(script: &str) -> Result<String, String> {
    let mut stack = test_stack();
    // The default limit of Warp 10, which the oracle runs with.
    stack.limits.max_geocells = 10_000;
    stack
        .exec_checked(&format!("{script} DEPTH ->LIST REVERSE ->JSON"))
        .map_err(|err| err.to_string())?;
    match stack.pop().expect("pop should succeed") {
        Value::String(json) => Ok(json.to_string()),
        other => panic!("not a string: {other:?}"),
    }
}

/// Check every case, reporting all the mismatches at once.
fn check(cases: &[(&str, Result<&str, &str>)]) {
    let mut failures = Vec::new();
    for (template, expected) in cases {
        let script = expand(template);
        match (expected, stack_json(&script)) {
            (Ok(json), Ok(actual)) if actual == *json => {}
            (Err(message), Err(actual)) if actual.contains(message) => {}
            (expected, actual) => failures.push(format!(
                "{template}\n  expected {expected:?}\n  actual   {actual:?}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn gml_read_0() {
    check(&[
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3</gml:coordinates></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3</gml:coordinates></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0,3.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3</gml:coordinates></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0,3.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3</gml:coordinates></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0,3.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3,4</gml:coordinates></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (dimension must be <= 3)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3,4</gml:coordinates></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (dimension must be <= 3)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3,4</gml:coordinates></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (dimension must be <= 3)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3,4</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (dimension must be <= 3)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3,4</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (dimension must be <= 3)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 2</gml:coordinates></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 2</gml:coordinates></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 2</gml:coordinates></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 2</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 3,4</gml:coordinates></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 3,4</gml:coordinates></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 3,4</gml:coordinates></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 3,4</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 3,4</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates>3,4</gml:coordinates></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates>3,4</gml:coordinates></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates>3,4</gml:coordinates></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates>3,4</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates>3,4</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates></gml:coordinates></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates></gml:coordinates></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates></gml:coordinates></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates></gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates></gml:coordinates></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>   </gml:coordinates></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a coordinate sequence without a non-null touple to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>   </gml:coordinates></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a coordinate sequence without a non-null touple to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>   </gml:coordinates></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a coordinate sequence without a non-null touple to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>   </gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a coordinate sequence without a non-null touple to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>   </gml:coordinates></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a coordinate sequence without a non-null touple to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>x,2</gml:coordinates></gml:Point>' ->WKT",
            Err(r#"->WKT was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>x,2</gml:coordinates></gml:Point>' ->GML",
            Err(r#"->GML was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>x,2</gml:coordinates></gml:Point>' ->KML",
            Err(r#"->KML was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>x,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(r#"->WKB was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>x,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(r#"->GEOJSON was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 </gml:coordinates></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 </gml:coordinates></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 </gml:coordinates></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 </gml:coordinates></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 </gml:coordinates></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>,</gml:coordinates></gml:Point>' ->WKT",
            Ok(r#"["POINT (0 0)"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>,</gml:coordinates></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>0.0,0.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>,</gml:coordinates></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>0.0,0.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>,</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Ok(r#"["000000000100000000000000000000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>,</gml:coordinates></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[0.0,0.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,,2</gml:coordinates></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,,2</gml:coordinates></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,,2</gml:coordinates></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
    ]);
}

#[test]
fn gml_read_1() {
    check(&[
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Z>3</gml:Z><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 3)"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Z>3</gml:Z><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,3.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Z>3</gml:Z><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,3.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Z>3</gml:Z><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004008000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Z>3</gml:Z><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,3.0,2.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:Z>3</gml:Z><gml:Y>2</gml:Y><gml:X>1</gml:X></gml:coord></gml:Point>' ->WKT",
            Ok(r#"["POINT (3 2)"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:Z>3</gml:Z><gml:Y>2</gml:Y><gml:X>1</gml:X></gml:coord></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>3.0,2.0,1.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:Z>3</gml:Z><gml:Y>2</gml:Y><gml:X>1</gml:X></gml:coord></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>3.0,2.0,1.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:Z>3</gml:Z><gml:Y>2</gml:Y><gml:X>1</gml:X></gml:coord></gml:Point>' ->WKB ->HEX",
            Ok(r#"["000000000140080000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:Z>3</gml:Z><gml:Y>2</gml:Y><gml:X>1</gml:X></gml:coord></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[3.0,2.0,1.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 0)"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,0.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,0.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000000000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,0.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:Z>3</gml:Z><gml:Z>4</gml:Z></gml:coord></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a coordinate with more than 3 axis)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:Z>3</gml:Z><gml:Z>4</gml:Z></gml:coord></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a coordinate with more than 3 axis)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:Z>3</gml:Z><gml:Z>4</gml:Z></gml:coord></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a coordinate with more than 3 axis)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:Z>3</gml:Z><gml:Z>4</gml:Z></gml:coord></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a coordinate with more than 3 axis)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:Z>3</gml:Z><gml:Z>4</gml:Z></gml:coord></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a coordinate with more than 3 axis)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:X> 2 </gml:X></gml:coord></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:X> 2 </gml:X></gml:coord></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:X> 2 </gml:X></gml:coord></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:X> 2 </gml:X></gml:coord></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:X> 2 </gml:X></gml:coord></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:foo>3</gml:foo></gml:coord></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:foo>3</gml:foo></gml:coord></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:foo>3</gml:foo></gml:coord></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:foo>3</gml:foo></gml:coord></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:foo>3</gml:foo></gml:coord></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X/></gml:coord></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X/></gml:coord></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X/></gml:coord></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X/></gml:coord></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X/></gml:coord></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>  </gml:X></gml:coord></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (empty String)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>  </gml:X></gml:coord></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (empty String)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>  </gml:X></gml:coord></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (empty String)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>  </gml:X></gml:coord></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (empty String)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>  </gml:X></gml:coord></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (empty String)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 2</gml:X></gml:coord></gml:Point>' ->WKT",
            Err(r#"->WKT was given invalid input. (For input string: "1 2")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 2</gml:X></gml:coord></gml:Point>' ->GML",
            Err(r#"->GML was given invalid input. (For input string: "1 2")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 2</gml:X></gml:coord></gml:Point>' ->KML",
            Err(r#"->KML was given invalid input. (For input string: "1 2")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 2</gml:X></gml:coord></gml:Point>' ->WKB ->HEX",
            Err(r#"->WKB was given invalid input. (For input string: "1 2")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 2</gml:X></gml:coord></gml:Point>' ->GEOJSON",
            Err(r#"->GEOJSON was given invalid input. (For input string: "1 2")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>0x10</gml:X><gml:Y>1.5f</gml:Y></gml:coord></gml:Point>' ->WKT",
            Err(r#"->WKT was given invalid input. (For input string: "0x10")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>0x10</gml:X><gml:Y>1.5f</gml:Y></gml:coord></gml:Point>' ->GML",
            Err(r#"->GML was given invalid input. (For input string: "0x10")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>0x10</gml:X><gml:Y>1.5f</gml:Y></gml:coord></gml:Point>' ->KML",
            Err(r#"->KML was given invalid input. (For input string: "0x10")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>0x10</gml:X><gml:Y>1.5f</gml:Y></gml:coord></gml:Point>' ->WKB ->HEX",
            Err(r#"->WKB was given invalid input. (For input string: "0x10")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>0x10</gml:X><gml:Y>1.5f</gml:Y></gml:coord></gml:Point>' ->GEOJSON",
            Err(r#"->GEOJSON was given invalid input. (For input string: "0x10")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1e2</gml:X><gml:Y>-2.5d</gml:Y></gml:coord></gml:Point>' ->WKT",
            Ok(r#"["POINT (100 -2.5)"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1e2</gml:X><gml:Y>-2.5d</gml:Y></gml:coord></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>100.0,-2.5 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1e2</gml:X><gml:Y>-2.5d</gml:Y></gml:coord></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>100.0,-2.5 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1e2</gml:X><gml:Y>-2.5d</gml:Y></gml:coord></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000014059000000000000c004000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1e2</gml:X><gml:Y>-2.5d</gml:Y></gml:coord></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[100.0,-2.5]}"]"#),
        ),
    ]);
}

#[test]
fn gml_read_2() {
    check(&[
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point/>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point/>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point/>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point/>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point/>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:foo/></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:foo/></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:foo/></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:foo/></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:foo/></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates/></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates/></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates/></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates/></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates/></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString>' ->WKT",
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString>' ->KML",
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString>' ->WKB ->HEX",
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString>' ->GEOJSON",
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,5</gml:coordinates></gml:LineString>' ->WKT",
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,5</gml:coordinates></gml:LineString>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,5</gml:coordinates></gml:LineString>' ->KML",
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,5</gml:coordinates></gml:LineString>' ->WKB ->HEX",
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,5</gml:coordinates></gml:LineString>' ->GEOJSON",
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2,3 4,5,6</gml:coordinates></gml:LineString>' ->WKT",
            Ok(r#"["LINESTRING (1 2, 4 5)"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2,3 4,5,6</gml:coordinates></gml:LineString>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0,3.0 4.0,5.0,6.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2,3 4,5,6</gml:coordinates></gml:LineString>' ->KML",
            Ok(
                r#"["<LineString><coordinates>1.0,2.0,3.0 4.0,5.0,6.0 </coordinates></LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2,3 4,5,6</gml:coordinates></gml:LineString>' ->WKB ->HEX",
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040100000000000004014000000000000"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2,3 4,5,6</gml:coordinates></gml:LineString>' ->GEOJSON",
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0,3.0],[4.0,5.0,6.0]]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1 2 3</gml:coordinates></gml:LineString>' ->WKT",
            Ok(r#"["LINESTRING (1 0, 2 0, 3 0)"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1 2 3</gml:coordinates></gml:LineString>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,0.0 2.0,0.0 3.0,0.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1 2 3</gml:coordinates></gml:LineString>' ->KML",
            Ok(
                r#"["<LineString><coordinates>1.0,0.0 2.0,0.0 3.0,0.0 </coordinates></LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1 2 3</gml:coordinates></gml:LineString>' ->WKB ->HEX",
            Ok(
                r#"["0000000002000000033ff000000000000000000000000000004000000000000000000000000000000040080000000000000000000000000000"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1 2 3</gml:coordinates></gml:LineString>' ->GEOJSON",
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,0.0],[2.0,0.0],[3.0,0.0]]}"]"#),
        ),
    ]);
}

#[test]
fn gml_read_3() {
    check(&[
        (
            r"'<gml:LineString><gml:coordinates>1,2</gml:coordinates></gml:LineString>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2</gml:coordinates></gml:LineString>' ->GML",
            Err(
                r"->GML was given invalid input. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2</gml:coordinates></gml:LineString>' ->KML",
            Err(
                r"->KML was given invalid input. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2</gml:coordinates></gml:LineString>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2</gml:coordinates></gml:LineString>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coordinates>5,6</gml:coordinates></gml:LineString>' ->WKT",
            Err(
                r"->WKT was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coordinates>5,6</gml:coordinates></gml:LineString>' ->GML",
            Err(
                r"->GML was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coordinates>5,6</gml:coordinates></gml:LineString>' ->KML",
            Err(
                r"->KML was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coordinates>5,6</gml:coordinates></gml:LineString>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coordinates>5,6</gml:coordinates></gml:LineString>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:LineString>' ->WKT",
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:LineString>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:LineString>' ->KML",
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:LineString>' ->WKB ->HEX",
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:LineString>' ->GEOJSON",
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:LineString>' ->WKT",
            Err(
                r"->WKT was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:LineString>' ->GML",
            Err(
                r"->GML was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:LineString>' ->KML",
            Err(
                r"->KML was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:LineString>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:LineString>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString></gml:LineString>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString></gml:LineString>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString></gml:LineString>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString></gml:LineString>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString></gml:LineString>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LineString>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LineString>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LineString>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LineString>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LineString>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' ->WKT",
            Err(
                r"->WKT was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' ->GML",
            Err(
                r"->GML was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' ->KML",
            Err(
                r"->KML was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\";\">1,2;3,4</gml:coordinates></gml:LineString>"' JSON-> ->WKT"#,
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\";\">1,2;3,4</gml:coordinates></gml:LineString>"' JSON-> ->GML"#,
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\";\">1,2;3,4</gml:coordinates></gml:LineString>"' JSON-> ->KML"#,
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\";\">1,2;3,4</gml:coordinates></gml:LineString>"' JSON-> ->WKB ->HEX"#,
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\";\">1,2;3,4</gml:coordinates></gml:LineString>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\" \" cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> ->WKT"#,
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\" \" cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> ->GML"#,
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\" \" cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> ->KML"#,
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\" \" cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> ->WKB ->HEX"#,
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\" \" cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> ->WKT"#,
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> ->GML"#,
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> ->KML"#,
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> ->WKB ->HEX"#,
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2x3,4</gml:coordinates></gml:LineString>"' JSON-> ->WKT"#,
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2x3,4</gml:coordinates></gml:LineString>"' JSON-> ->GML"#,
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2x3,4</gml:coordinates></gml:LineString>"' JSON-> ->KML"#,
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2x3,4</gml:coordinates></gml:LineString>"' JSON-> ->WKB ->HEX"#,
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2x3,4</gml:coordinates></gml:LineString>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
    ]);
}

#[test]
fn gml_read_4() {
    check(&[
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> ->WKT"#,
            Err(r#"->WKT was given invalid input. (For input string: "2 3")"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> ->GML"#,
            Err(r#"->GML was given invalid input. (For input string: "2 3")"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> ->KML"#,
            Err(r#"->KML was given invalid input. (For input string: "2 3")"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> ->WKB ->HEX"#,
            Err(r#"->WKB was given invalid input. (For input string: "2 3")"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> ->GEOJSON"#,
            Err(r#"->GEOJSON was given invalid input. (For input string: "2 3")"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\"\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> ->WKT"#,
            Err(r#"->WKT was given invalid input. (For input string: ",")"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\"\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> ->GML"#,
            Err(r#"->GML was given invalid input. (For input string: ",")"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\"\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> ->KML"#,
            Err(r#"->KML was given invalid input. (For input string: ",")"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\"\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> ->WKB ->HEX"#,
            Err(r#"->WKB was given invalid input. (For input string: ",")"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\"\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> ->GEOJSON"#,
            Err(r#"->GEOJSON was given invalid input. (For input string: ",")"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,</gml:coordinates></gml:LineString>' ->WKT",
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,</gml:coordinates></gml:LineString>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,</gml:coordinates></gml:LineString>' ->KML",
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,</gml:coordinates></gml:LineString>' ->WKB ->HEX",
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,</gml:coordinates></gml:LineString>' ->GEOJSON",
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 </gml:coordinates></gml:LineString>' ->WKT",
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 </gml:coordinates></gml:LineString>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 </gml:coordinates></gml:LineString>' ->KML",
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 </gml:coordinates></gml:LineString>' ->WKB ->HEX",
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 </gml:coordinates></gml:LineString>' ->GEOJSON",
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2 3,4\n5,6</gml:coordinates></gml:LineString>"' JSON-> ->WKT"#,
            Ok(r#"["LINESTRING (1 2, 3 4, 5 6)"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2 3,4\n5,6</gml:coordinates></gml:LineString>"' JSON-> ->GML"#,
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 5.0,6.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2 3,4\n5,6</gml:coordinates></gml:LineString>"' JSON-> ->KML"#,
            Ok(
                r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 5.0,6.0 </coordinates></LineString>"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2 3,4\n5,6</gml:coordinates></gml:LineString>"' JSON-> ->WKB ->HEX"#,
            Ok(
                r#"["0000000002000000033ff000000000000040000000000000004008000000000000401000000000000040140000000000004018000000000000"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2 3,4\n5,6</gml:coordinates></gml:LineString>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0],[5.0,6.0]]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\n3,4</gml:coordinates></gml:LineString>"' JSON-> ->WKT"#,
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\n3,4</gml:coordinates></gml:LineString>"' JSON-> ->GML"#,
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\n3,4</gml:coordinates></gml:LineString>"' JSON-> ->KML"#,
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\n3,4</gml:coordinates></gml:LineString>"' JSON-> ->WKB ->HEX"#,
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\n3,4</gml:coordinates></gml:LineString>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 5,6 7,8 9,10</gml:coordinates></gml:LineString>' ->WKT",
            Ok(r#"["LINESTRING (1 2, 3 4, 5 6, 7 8, 9 10)"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 5,6 7,8 9,10</gml:coordinates></gml:LineString>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 5.0,6.0 7.0,8.0 9.0,10.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 5,6 7,8 9,10</gml:coordinates></gml:LineString>' ->KML",
            Ok(
                r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 5.0,6.0 7.0,8.0 9.0,10.0 </coordinates></LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 5,6 7,8 9,10</gml:coordinates></gml:LineString>' ->WKB ->HEX",
            Ok(
                r#"["0000000002000000053ff000000000000040000000000000004008000000000000401000000000000040140000000000004018000000000000401c000000000000402000000000000040220000000000004024000000000000"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 5,6 7,8 9,10</gml:coordinates></gml:LineString>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0],[5.0,6.0],[7.0,8.0],[9.0,10.0]]}"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 x</gml:coordinates></gml:LineString>' ->WKT",
            Err(r#"->WKT was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 x</gml:coordinates></gml:LineString>' ->GML",
            Err(r#"->GML was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 x</gml:coordinates></gml:LineString>' ->KML",
            Err(r#"->KML was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 x</gml:coordinates></gml:LineString>' ->WKB ->HEX",
            Err(r#"->WKB was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 x</gml:coordinates></gml:LineString>' ->GEOJSON",
            Err(r#"->GEOJSON was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 1,2 3,4 1,2 3,4</gml:coordinates></gml:LineString>' ->WKT",
            Ok(r#"["LINESTRING (1 2, 3 4, 1 2, 3 4, 1 2, 3 4)"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 1,2 3,4 1,2 3,4</gml:coordinates></gml:LineString>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 1.0,2.0 3.0,4.0 1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 1,2 3,4 1,2 3,4</gml:coordinates></gml:LineString>' ->KML",
            Ok(
                r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 1.0,2.0 3.0,4.0 1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 1,2 3,4 1,2 3,4</gml:coordinates></gml:LineString>' ->WKB ->HEX",
            Ok(
                r#"["0000000002000000063ff00000000000004000000000000000400800000000000040100000000000003ff00000000000004000000000000000400800000000000040100000000000003ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 1,2 3,4 1,2 3,4</gml:coordinates></gml:LineString>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0],[1.0,2.0],[3.0,4.0],[1.0,2.0],[3.0,4.0]]}"]"#,
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' ->WKT",
            Ok(r#"["LINEARRING (0 0, 1 0, 1 1, 0 0)"]"#),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>0.0,0.0 1.0,0.0 1.0,1.0 0.0,0.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' ->KML",
            Ok(
                r#"["<LineString><coordinates>0.0,0.0 1.0,0.0 1.0,1.0 0.0,0.0 </coordinates></LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' ->WKB ->HEX",
            Ok(
                r#"["000000000200000004000000000000000000000000000000003ff000000000000000000000000000003ff00000000000003ff000000000000000000000000000000000000000000000"]"#,
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (UnsupportedOperationException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing>' ->GML",
            Err(
                r"->GML was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing>' ->KML",
            Err(
                r"->KML was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' ->GML",
            Err(
                r"->GML was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' ->KML",
            Err(
                r"->KML was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
    ]);
}

#[test]
fn gml_read_5() {
    check(&[
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord></gml:LinearRing>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord></gml:LinearRing>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord></gml:LinearRing>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord></gml:LinearRing>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord></gml:LinearRing>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' ->WKT",
            Ok(r#"["LINEARRING (0 0, 1 0, 1 1, 0 0)"]"#),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>0.0,0.0 1.0,0.0 1.0,1.0 0.0,0.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' ->KML",
            Ok(
                r#"["<LineString><coordinates>0.0,0.0 1.0,0.0 1.0,1.0 0.0,0.0 </coordinates></LineString>"]"#,
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' ->WKB ->HEX",
            Ok(
                r#"["000000000200000004000000000000000000000000000000003ff000000000000000000000000000003ff00000000000003ff000000000000000000000000000000000000000000000"]"#,
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (UnsupportedOperationException)"),
        ),
        (
            r"'<gml:LinearRing><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LinearRing>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LinearRing>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LinearRing>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LinearRing>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LinearRing>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LinearRing></gml:LinearRing>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing></gml:LinearRing>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing></gml:LinearRing>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing></gml:LinearRing>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing></gml:LinearRing>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->WKT",
            Ok(r#"["POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0))"]"#),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->KML",
            Ok(
                r#"["<Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs></Polygon>"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->WKB ->HEX",
            Ok(
                r#"["000000000300000001000000050000000000000000000000000000000000000000000000004024000000000000402400000000000040240000000000004024000000000000000000000000000000000000000000000000000000000000"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"Polygon\",\"coordinates\":[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->WKT",
            Ok(r#"["POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0), (2 2, 2 4, 4 4, 4 2, 2 2))"]"#),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->KML",
            Ok(
                r#"["<Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </coordinates></LinearRing></innerBoundaryIs></Polygon>"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->WKB ->HEX",
            Ok(
                r#"["000000000300000002000000050000000000000000000000000000000000000000000000004024000000000000402400000000000040240000000000004024000000000000000000000000000000000000000000000000000000000000000000054000000000000000400000000000000040000000000000004010000000000000401000000000000040100000000000004010000000000000400000000000000040000000000000004000000000000000"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"Polygon\",\"coordinates\":[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]],[[2.0,2.0],[2.0,4.0],[4.0,4.0],[4.0,2.0],[2.0,2.0]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon></gml:Polygon>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon></gml:Polygon>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon></gml:Polygon>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon></gml:Polygon>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon></gml:Polygon>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:Polygon>' ->WKT",
            Ok(r#"["POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0))"]"#),
        ),
        (
            r"'<gml:Polygon><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:Polygon>' ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:Polygon>' ->KML",
            Ok(
                r#"["<Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs></Polygon>"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:Polygon>' ->WKB ->HEX",
            Ok(
                r#"["000000000300000001000000050000000000000000000000000000000000000000000000004024000000000000402400000000000040240000000000004024000000000000000000000000000000000000000000000000000000000000"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:Polygon>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"Polygon\",\"coordinates\":[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->WKT",
            Ok(r#"["POLYGON ((2 2, 2 4, 4 4, 4 2, 2 2))"]"#),
        ),
        (
            r"'<gml:Polygon><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->KML",
            Ok(
                r#"["<Polygon><outerBoundaryIs><LinearRing><coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </coordinates></LinearRing></outerBoundaryIs></Polygon>"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->WKB ->HEX",
            Ok(
                r#"["000000000300000001000000054000000000000000400000000000000040000000000000004010000000000000401000000000000040100000000000004010000000000000400000000000000040000000000000004000000000000000"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"Polygon\",\"coordinates\":[[[2.0,2.0],[2.0,4.0],[4.0,4.0],[4.0,2.0],[2.0,2.0]]]}"]"#,
            ),
        ),
    ]);
}

#[test]
fn gml_read_6() {
    check(&[
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->WKT",
            Ok(
                r#"["POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0), (2 2, 2 4, 4 4, 4 2, 2 2), (2 2, 2 4, 4 4, 4 2, 2 2))"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </gml:coordinates></gml:LinearRing></gml:innerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->KML",
            Ok(
                r#"["<Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </coordinates></LinearRing></innerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </coordinates></LinearRing></innerBoundaryIs></Polygon>"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->WKB ->HEX",
            Ok(
                r#"["000000000300000003000000050000000000000000000000000000000000000000000000004024000000000000402400000000000040240000000000004024000000000000000000000000000000000000000000000000000000000000000000054000000000000000400000000000000040000000000000004010000000000000401000000000000040100000000000004010000000000000400000000000000040000000000000004000000000000000000000054000000000000000400000000000000040000000000000004010000000000000401000000000000040100000000000004010000000000000400000000000000040000000000000004000000000000000"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"Polygon\",\"coordinates\":[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]],[[2.0,2.0],[2.0,4.0],[4.0,4.0],[4.0,2.0],[2.0,2.0]],[[2.0,2.0],[2.0,4.0],[4.0,4.0],[4.0,2.0],[2.0,2.0]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:outerBoundaryIs></gml:Polygon>' ->WKT",
            Err(
                r"->WKT was given invalid input. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:outerBoundaryIs></gml:Polygon>' ->GML",
            Err(
                r"->GML was given invalid input. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:outerBoundaryIs></gml:Polygon>' ->KML",
            Err(
                r"->KML was given invalid input. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:outerBoundaryIs></gml:Polygon>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:outerBoundaryIs></gml:Polygon>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs></gml:outerBoundaryIs></gml:Polygon>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs></gml:outerBoundaryIs></gml:Polygon>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs></gml:outerBoundaryIs></gml:Polygon>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs></gml:outerBoundaryIs></gml:Polygon>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs></gml:outerBoundaryIs></gml:Polygon>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->GML",
            Err(
                r"->GML was given invalid input. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->KML",
            Err(
                r"->KML was given invalid input. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->WKT",
            Ok(r#"["POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0), (2 2, 2 4, 4 4, 4 2, 2 2))"]"#),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->KML",
            Ok(
                r#"["<Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </coordinates></LinearRing></innerBoundaryIs></Polygon>"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->WKB ->HEX",
            Ok(
                r#"["000000000300000002000000050000000000000000000000000000000000000000000000004024000000000000402400000000000040240000000000004024000000000000000000000000000000000000000000000000000000000000000000054000000000000000400000000000000040000000000000004010000000000000401000000000000040100000000000004010000000000000400000000000000040000000000000004000000000000000"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"Polygon\",\"coordinates\":[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]],[[2.0,2.0],[2.0,4.0],[4.0,4.0],[4.0,2.0],[2.0,2.0]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:outerBoundaryIs></gml:Polygon>' ->WKT",
            Err(
                r"->WKT was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:outerBoundaryIs></gml:Polygon>' ->GML",
            Err(
                r"->GML was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:outerBoundaryIs></gml:Polygon>' ->KML",
            Err(
                r"->KML was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:outerBoundaryIs></gml:Polygon>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:outerBoundaryIs></gml:Polygon>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->GML",
            Err(
                r"->GML was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->KML",
            Err(
                r"->KML was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>3,4</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' ->WKT",
            Ok(r#"["MULTIPOINT ((1 2), (3 4))"]"#),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>3,4</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' ->GML",
            Ok(
                r#"["<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>3.0,4.0 </gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>"]"#,
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>3,4</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' ->KML",
            Ok(
                r#"["<MultiGeometry><Point><coordinates>1.0,2.0 </coordinates></Point><Point><coordinates>3.0,4.0 </coordinates></Point></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>3,4</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' ->WKB ->HEX",
            Ok(
                r#"["00000000040000000200000000013ff00000000000004000000000000000000000000140080000000000004010000000000000"]"#,
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>3,4</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPoint\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r"'<gml:MultiPoint></gml:MultiPoint>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint></gml:MultiPoint>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint></gml:MultiPoint>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint></gml:MultiPoint>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint></gml:MultiPoint>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:pointMember></gml:MultiPoint>' ->WKT",
            Err(r"->WKT was given invalid input. (com.vividsolutions.jts.geom.LineString)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:pointMember></gml:MultiPoint>' ->GML",
            Err(r"->GML was given invalid input. (com.vividsolutions.jts.geom.LineString)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:pointMember></gml:MultiPoint>' ->KML",
            Err(r"->KML was given invalid input. (com.vividsolutions.jts.geom.LineString)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:pointMember></gml:MultiPoint>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (com.vividsolutions.jts.geom.LineString)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:pointMember></gml:MultiPoint>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.LineString)"),
        ),
    ]);
}

#[test]
fn gml_read_7() {
    check(&[
        (
            r"'<gml:MultiPoint><gml:pointMember></gml:pointMember></gml:MultiPoint>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember></gml:pointMember></gml:MultiPoint>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember></gml:pointMember></gml:MultiPoint>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember></gml:pointMember></gml:MultiPoint>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember></gml:pointMember></gml:MultiPoint>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' ->GML",
            Err(
                r"->GML was given invalid input. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' ->KML",
            Err(
                r"->KML was given invalid input. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' ->WKT",
            Ok(r#"["MULTIPOINT ((1 2))"]"#),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' ->GML",
            Ok(
                r#"["<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>"]"#,
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' ->KML",
            Ok(
                r#"["<MultiGeometry><Point><coordinates>1.0,2.0 </coordinates></Point></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' ->WKB ->HEX",
            Ok(r#"["00000000040000000100000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPoint\",\"coordinates\":[[1.0,2.0]]}"]"#),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing></gml:lineStringMember><gml:lineStringMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString>' ->WKT",
            Ok(r#"["MULTILINESTRING ((0 0, 1 0, 1 1, 0 0), (1 2, 3 4))"]"#),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing></gml:lineStringMember><gml:lineStringMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString>' ->GML",
            Ok(
                r#"["<gml:MultiLineString><gml:lineStringMember><gml:LineString><gml:coordinates>0.0,0.0 1.0,0.0 1.0,1.0 0.0,0.0 </gml:coordinates></gml:LineString></gml:lineStringMember><gml:lineStringMember><gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString>"]"#,
            ),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing></gml:lineStringMember><gml:lineStringMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString>' ->KML",
            Ok(
                r#"["<MultiGeometry><LineString><coordinates>0.0,0.0 1.0,0.0 1.0,1.0 0.0,0.0 </coordinates></LineString><LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing></gml:lineStringMember><gml:lineStringMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString>' ->WKB ->HEX",
            Ok(
                r#"["000000000500000002000000000200000004000000000000000000000000000000003ff000000000000000000000000000003ff00000000000003ff0000000000000000000000000000000000000000000000000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing></gml:lineStringMember><gml:lineStringMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiLineString\",\"coordinates\":[[[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,0.0]],[[1.0,2.0],[3.0,4.0]]]}"]"#,
            ),
        ),
        (
            r"'<gml:MultiLineString></gml:MultiLineString>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString></gml:MultiLineString>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString></gml:MultiLineString>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString></gml:MultiLineString>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString></gml:MultiLineString>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>' ->WKT",
            Ok(
                r#"["MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0), (2 2, 2 4, 4 4, 4 2, 2 2)), ((0 0, 0 10, 10 10, 10 0, 0 0)))"]"#,
            ),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>' ->GML",
            Ok(
                r#"["<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>"]"#,
            ),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>' ->KML",
            Ok(
                r#"["<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </coordinates></LinearRing></innerBoundaryIs></Polygon><Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>' ->WKB ->HEX",
            Ok(
                r#"["000000000600000002000000000300000002000000050000000000000000000000000000000000000000000000004024000000000000402400000000000040240000000000004024000000000000000000000000000000000000000000000000000000000000000000054000000000000000400000000000000040000000000000004010000000000000401000000000000040100000000000004010000000000000400000000000000040000000000000004000000000000000000000000300000001000000050000000000000000000000000000000000000000000000004024000000000000402400000000000040240000000000004024000000000000000000000000000000000000000000000000000000000000"]"#,
            ),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]],[[2.0,2.0],[2.0,4.0],[4.0,4.0],[4.0,2.0],[2.0,2.0]]],[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:polygonMember></gml:MultiPolygon>' ->WKT",
            Err(r"->WKT was given invalid input. (com.vividsolutions.jts.geom.LinearRing)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:polygonMember></gml:MultiPolygon>' ->GML",
            Err(r"->GML was given invalid input. (com.vividsolutions.jts.geom.LinearRing)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:polygonMember></gml:MultiPolygon>' ->KML",
            Err(r"->KML was given invalid input. (com.vividsolutions.jts.geom.LinearRing)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:polygonMember></gml:MultiPolygon>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (com.vividsolutions.jts.geom.LinearRing)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:polygonMember></gml:MultiPolygon>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.LinearRing)"),
        ),
        (
            r"'<gml:MultiPolygon></gml:MultiPolygon>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon></gml:MultiPolygon>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon></gml:MultiPolygon>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon></gml:MultiPolygon>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon></gml:MultiPolygon>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:geometryMember><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:geometryMember><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:geometryMember><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:geometryMember><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:geometryMember><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry></gml:MultiGeometry>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry></gml:MultiGeometry>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry></gml:MultiGeometry>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry></gml:MultiGeometry>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry></gml:MultiGeometry>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry></gml:geometryMember></gml:MultiGeometry>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry></gml:geometryMember></gml:MultiGeometry>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry></gml:geometryMember></gml:MultiGeometry>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry></gml:geometryMember></gml:MultiGeometry>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry></gml:geometryMember></gml:MultiGeometry>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:geometryMember></gml:MultiGeometry>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:geometryMember></gml:MultiGeometry>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:geometryMember></gml:MultiGeometry>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:geometryMember></gml:MultiGeometry>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:geometryMember></gml:MultiGeometry>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_read_8() {
    check(&[
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' ->WKT",
            Ok(r#"["MULTIPOINT ((1 2))"]"#),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' ->GML",
            Ok(
                r#"["<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>"]"#,
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' ->KML",
            Ok(
                r#"["<MultiGeometry><Point><coordinates>1.0,2.0 </coordinates></Point></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' ->WKB ->HEX",
            Ok(r#"["00000000040000000100000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPoint\",\"coordinates\":[[1.0,2.0]]}"]"#),
        ),
        (
            r"'<gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs>' ->WKT",
            Ok(r#"["LINEARRING (0 0, 0 10, 10 10, 10 0, 0 0)"]"#),
        ),
        (
            r"'<gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs>' ->KML",
            Ok(
                r#"["<LineString><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LineString>"]"#,
            ),
        ),
        (
            r"'<gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs>' ->WKB ->HEX",
            Ok(
                r#"["0000000002000000050000000000000000000000000000000000000000000000004024000000000000402400000000000040240000000000004024000000000000000000000000000000000000000000000000000000000000"]"#,
            ),
        ),
        (
            r"'<gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (UnsupportedOperationException)"),
        ),
        (
            r"'<gml:point><gml:coordinates>1,2</gml:coordinates></gml:point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:point><gml:coordinates>1,2</gml:coordinates></gml:point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:point><gml:coordinates>1,2</gml:coordinates></gml:point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:point><gml:coordinates>1,2</gml:coordinates></gml:point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:point><gml:coordinates>1,2</gml:coordinates></gml:point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<gml:POINT><gml:coordinates>1,2</gml:coordinates></gml:POINT>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:POINT><gml:coordinates>1,2</gml:coordinates></gml:POINT>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:POINT><gml:coordinates>1,2</gml:coordinates></gml:POINT>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:POINT><gml:coordinates>1,2</gml:coordinates></gml:POINT>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:POINT><gml:coordinates>1,2</gml:coordinates></gml:POINT>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<gml:Foo><gml:coordinates>1,2</gml:coordinates></gml:Foo>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Foo><gml:coordinates>1,2</gml:coordinates></gml:Foo>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Foo><gml:coordinates>1,2</gml:coordinates></gml:Foo>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Foo><gml:coordinates>1,2</gml:coordinates></gml:Foo>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Foo><gml:coordinates>1,2</gml:coordinates></gml:Foo>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:Foo/></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:Foo/></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:Foo/></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:Foo/></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:Foo/></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_read_9() {
    check(&[
        (
            r"'<gml:Point><foo>1,2</foo></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><foo>1,2</foo></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><foo>1,2</foo></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><foo>1,2</foo></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><foo>1,2</foo></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><x' ->WKT",
            Err(
                r"->WKT was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><x' ->GML",
            Err(
                r"->GML was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><x' ->KML",
            Err(
                r"->KML was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><x' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><x' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates>' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates>' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates>' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>junk' ->WKT",
            Err(r"->WKT was given invalid input. (Content is not allowed in trailing section.)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>junk' ->GML",
            Err(r"->GML was given invalid input. (Content is not allowed in trailing section.)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>junk' ->KML",
            Err(r"->KML was given invalid input. (Content is not allowed in trailing section.)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>junk' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (Content is not allowed in trailing section.)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>junk' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Content is not allowed in trailing section.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r#"'"<?xml version=\"1.0\"?><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKT"#,
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r#"'"<?xml version=\"1.0\"?><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GML"#,
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r#"'"<?xml version=\"1.0\"?><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->KML"#,
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r#"'"<?xml version=\"1.0\"?><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKB ->HEX"#,
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r#"'"<?xml version=\"1.0\"?><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GEOJSON"#,
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!-- c --><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!-- c --><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!-- c --><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!-- c --><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!-- c --><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><!-- c --><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:Point><!-- c --><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><!-- c --><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><!-- c --><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><!-- c --><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates><![CDATA[1,2]]></gml:coordinates></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates><![CDATA[1,2]]></gml:coordinates></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates><![CDATA[1,2]]></gml:coordinates></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates><![CDATA[1,2]]></gml:coordinates></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates><![CDATA[1,2]]></gml:coordinates></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>&#49;,2</gml:coordinates></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>&#49;,2</gml:coordinates></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>&#49;,2</gml:coordinates></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>&#49;,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>&#49;,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&amp;</gml:coordinates></gml:Point>' ->WKT",
            Err(r#"->WKT was given invalid input. (For input string: "2&")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&amp;</gml:coordinates></gml:Point>' ->GML",
            Err(r#"->GML was given invalid input. (For input string: "2&")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&amp;</gml:coordinates></gml:Point>' ->KML",
            Err(r#"->KML was given invalid input. (For input string: "2&")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&amp;</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(r#"->WKB was given invalid input. (For input string: "2&")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&amp;</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(r#"->GEOJSON was given invalid input. (For input string: "2&")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&foo;</gml:coordinates></gml:Point>' ->WKT",
            Err(
                r#"->WKT was given invalid input. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&foo;</gml:coordinates></gml:Point>' ->GML",
            Err(
                r#"->GML was given invalid input. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&foo;</gml:coordinates></gml:Point>' ->KML",
            Err(
                r#"->KML was given invalid input. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&foo;</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(
                r#"->WKB was given invalid input. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&foo;</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(
                r#"->GEOJSON was given invalid input. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
    ]);
}

#[test]
fn gml_read_10() {
    check(&[
        (
            r#"'"<gml:Point id=\"a\" id=\"b\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKT"#,
            Err(
                r#"->WKT was given invalid input. (Attribute "id" was already specified for element "gml:Point".)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a\" id=\"b\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GML"#,
            Err(
                r#"->GML was given invalid input. (Attribute "id" was already specified for element "gml:Point".)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a\" id=\"b\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->KML"#,
            Err(
                r#"->KML was given invalid input. (Attribute "id" was already specified for element "gml:Point".)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a\" id=\"b\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKB ->HEX"#,
            Err(
                r#"->WKB was given invalid input. (Attribute "id" was already specified for element "gml:Point".)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a\" id=\"b\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GEOJSON"#,
            Err(
                r#"->GEOJSON was given invalid input. (Attribute "id" was already specified for element "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point id=a><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKT",
            Err(
                r#"->WKT was given invalid input. (Open quote is expected for attribute "id" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point id=a><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GML",
            Err(
                r#"->GML was given invalid input. (Open quote is expected for attribute "id" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point id=a><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->KML",
            Err(
                r#"->KML was given invalid input. (Open quote is expected for attribute "id" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point id=a><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(
                r#"->WKB was given invalid input. (Open quote is expected for attribute "id" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point id=a><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(
                r#"->GEOJSON was given invalid input. (Open quote is expected for attribute "id" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point a=\'1\'><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKT",
            Err(
                r#"->WKT was given invalid input. (Open quote is expected for attribute "a" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point a=\'1\'><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GML",
            Err(
                r#"->GML was given invalid input. (Open quote is expected for attribute "a" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point a=\'1\'><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->KML",
            Err(
                r#"->KML was given invalid input. (Open quote is expected for attribute "a" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point a=\'1\'><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(
                r#"->WKB was given invalid input. (Open quote is expected for attribute "a" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point a=\'1\'><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(
                r#"->GEOJSON was given invalid input. (Open quote is expected for attribute "a" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r#"'"<gml:Point xmlns:gml=\"http://www.opengis.net/gml\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKT"#,
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r#"'"<gml:Point xmlns:gml=\"http://www.opengis.net/gml\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GML"#,
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r#"'"<gml:Point xmlns:gml=\"http://www.opengis.net/gml\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->KML"#,
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r#"'"<gml:Point xmlns:gml=\"http://www.opengis.net/gml\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKB ->HEX"#,
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r#"'"<gml:Point xmlns:gml=\"http://www.opengis.net/gml\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r#"'"<gml:Point >\t<gml:coordinates>1,2</gml:coordinates>\n</gml:Point >"' JSON-> ->WKT"#,
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r#"'"<gml:Point >\t<gml:coordinates>1,2</gml:coordinates>\n</gml:Point >"' JSON-> ->GML"#,
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r#"'"<gml:Point >\t<gml:coordinates>1,2</gml:coordinates>\n</gml:Point >"' JSON-> ->KML"#,
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r#"'"<gml:Point >\t<gml:coordinates>1,2</gml:coordinates>\n</gml:Point >"' JSON-> ->WKB ->HEX"#,
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r#"'"<gml:Point >\t<gml:coordinates>1,2</gml:coordinates>\n</gml:Point >"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r#"'"<gml:Point>\n  <gml:coordinates>\n    1,2\n  </gml:coordinates>\n</gml:Point>"' JSON-> ->WKT"#,
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r#"'"<gml:Point>\n  <gml:coordinates>\n    1,2\n  </gml:coordinates>\n</gml:Point>"' JSON-> ->GML"#,
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r#"'"<gml:Point>\n  <gml:coordinates>\n    1,2\n  </gml:coordinates>\n</gml:Point>"' JSON-> ->KML"#,
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r#"'"<gml:Point>\n  <gml:coordinates>\n    1,2\n  </gml:coordinates>\n</gml:Point>"' JSON-> ->WKB ->HEX"#,
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r#"'"<gml:Point>\n  <gml:coordinates>\n    1,2\n  </gml:coordinates>\n</gml:Point>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>\n1,2\n3,4\n</gml:coordinates></gml:LineString>"' JSON-> ->WKT"#,
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>\n1,2\n3,4\n</gml:coordinates></gml:LineString>"' JSON-> ->GML"#,
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>\n1,2\n3,4\n</gml:coordinates></gml:LineString>"' JSON-> ->KML"#,
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>\n1,2\n3,4\n</gml:coordinates></gml:LineString>"' JSON-> ->WKB ->HEX"#,
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>\n1,2\n3,4\n</gml:coordinates></gml:LineString>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\t3,4</gml:coordinates></gml:LineString>"' JSON-> ->WKT"#,
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\t3,4</gml:coordinates></gml:LineString>"' JSON-> ->GML"#,
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\t3,4</gml:coordinates></gml:LineString>"' JSON-> ->KML"#,
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\t3,4</gml:coordinates></gml:LineString>"' JSON-> ->WKB ->HEX"#,
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\t3,4</gml:coordinates></gml:LineString>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r#"'"<gml:Point><gml:coord><gml:X>\n1\n</gml:X><gml:Y>\t2\t</gml:Y></gml:coord></gml:Point>"' JSON-> ->WKT"#,
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r#"'"<gml:Point><gml:coord><gml:X>\n1\n</gml:X><gml:Y>\t2\t</gml:Y></gml:coord></gml:Point>"' JSON-> ->GML"#,
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r#"'"<gml:Point><gml:coord><gml:X>\n1\n</gml:X><gml:Y>\t2\t</gml:Y></gml:coord></gml:Point>"' JSON-> ->KML"#,
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r#"'"<gml:Point><gml:coord><gml:X>\n1\n</gml:X><gml:Y>\t2\t</gml:Y></gml:coord></gml:Point>"' JSON-> ->WKB ->HEX"#,
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r#"'"<gml:Point><gml:coord><gml:X>\n1\n</gml:X><gml:Y>\t2\t</gml:Y></gml:coord></gml:Point>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 </gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 </gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 </gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 </gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 </gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
    ]);
}

#[test]
fn gml_read_11() {
    check(&[
        (
            r"'<gml:<' ->WKT",
            Err(
                r#"->WKT was given invalid input. (Element type "gml:" must be followed by either attribute specifications, ">" or "/>".)"#,
            ),
        ),
        (
            r"'<gml:<' ->GML",
            Err(
                r#"->GML was given invalid input. (Element type "gml:" must be followed by either attribute specifications, ">" or "/>".)"#,
            ),
        ),
        (
            r"'<gml:<' ->KML",
            Err(
                r#"->KML was given invalid input. (Element type "gml:" must be followed by either attribute specifications, ">" or "/>".)"#,
            ),
        ),
        (
            r"'<gml:<' ->WKB ->HEX",
            Err(
                r#"->WKB was given invalid input. (Element type "gml:" must be followed by either attribute specifications, ">" or "/>".)"#,
            ),
        ),
        (
            r"'<gml:<' ->GEOJSON",
            Err(
                r#"->GEOJSON was given invalid input. (Element type "gml:" must be followed by either attribute specifications, ">" or "/>".)"#,
            ),
        ),
        (
            r"'<gml' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<kml/>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml/>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml/>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml/>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml/>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml></kml>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml></kml>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml></kml>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml></kml>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml></kml>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point/>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point/>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point/>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point/>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point/>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<point><coordinates>1,2</coordinates></point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<point><coordinates>1,2</coordinates></point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<point><coordinates>1,2</coordinates></point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<point><coordinates>1,2</coordinates></point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<point><coordinates>1,2</coordinates></point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<POINT><coordinates>1,2</coordinates></POINT>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<POINT><coordinates>1,2</coordinates></POINT>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<POINT><coordinates>1,2</coordinates></POINT>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<POINT><coordinates>1,2</coordinates></POINT>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<POINT><coordinates>1,2</coordinates></POINT>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<kml:Point><coordinates>1,2</coordinates></kml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml:Point><coordinates>1,2</coordinates></kml:Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml:Point><coordinates>1,2</coordinates></kml:Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml:Point><coordinates>1,2</coordinates></kml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml:Point><coordinates>1,2</coordinates></kml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_read_12() {
    check(&[
        (
            r"'<Point><gml:coordinates>1,2</gml:coordinates></Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point><gml:coordinates>1,2</gml:coordinates></Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point><gml:coordinates>1,2</gml:coordinates></Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point><gml:coordinates>1,2</gml:coordinates></Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point><gml:coordinates>1,2</gml:coordinates></Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><coordinates>1,2</coordinates></gml:Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:Point><coordinates>1,2</coordinates></gml:Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><coordinates>1,2</coordinates></gml:Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><coordinates>1,2</coordinates></gml:Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><coordinates>1,2</coordinates></gml:Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<Foo/><' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo/><' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo/><' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo/><' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo/><' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo></Foo>x' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo></Foo>x' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo></Foo>x' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo></Foo>x' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo></Foo>x' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!-' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!-' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!-' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!-' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!-' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!x' ->WKT",
            Err(
                r"->WKT was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<!x' ->GML",
            Err(
                r"->GML was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<!x' ->KML",
            Err(
                r"->KML was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<!x' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<!x' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<![x' ->WKT",
            Err(
                r"->WKT was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<![x' ->GML",
            Err(
                r"->GML was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<![x' ->KML",
            Err(
                r"->KML was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<![x' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<![x' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<?xml ' ->WKT",
            Err(r"->WKT was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<?xml ' ->GML",
            Err(r"->GML was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<?xml ' ->KML",
            Err(r"->KML was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<?xml ' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<?xml ' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<?xml' ->WKT",
            Err(r"->WKT was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<?xml' ->GML",
            Err(r"->GML was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<?xml' ->KML",
            Err(r"->KML was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<?xml' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<?xml' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x [' ->WKT",
            Err(r"->WKT was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x [' ->GML",
            Err(r"->GML was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x [' ->KML",
            Err(r"->KML was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x [' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x [' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (Premature end of file.)"),
        ),
    ]);
}

#[test]
fn gml_read_13() {
    check(&[
        (
            r"'<!DOCTYPE x []' ->WKT",
            Err(r"->WKT was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x []' ->GML",
            Err(r"->GML was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x []' ->KML",
            Err(r"->KML was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x []' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x []' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (Premature end of file.)"),
        ),
        (
            r#"'"<!DOCTYPE x SYSTEM \"a"' JSON-> ->WKT"#,
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r#"'"<!DOCTYPE x SYSTEM \"a"' JSON-> ->GML"#,
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r#"'"<!DOCTYPE x SYSTEM \"a"' JSON-> ->KML"#,
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r#"'"<!DOCTYPE x SYSTEM \"a"' JSON-> ->WKB ->HEX"#,
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r#"'"<!DOCTYPE x SYSTEM \"a"' JSON-> ->GEOJSON"#,
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></' ->WKT",
            Err(
                r#"->WKT was given invalid input. (The element type "gml:Point" must be terminated by the matching end-tag "</gml:Point>".)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></' ->GML",
            Err(
                r#"->GML was given invalid input. (The element type "gml:Point" must be terminated by the matching end-tag "</gml:Point>".)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></' ->KML",
            Err(
                r#"->KML was given invalid input. (The element type "gml:Point" must be terminated by the matching end-tag "</gml:Point>".)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></' ->WKB ->HEX",
            Err(
                r#"->WKB was given invalid input. (The element type "gml:Point" must be terminated by the matching end-tag "</gml:Point>".)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></' ->GEOJSON",
            Err(
                r#"->GEOJSON was given invalid input. (The element type "gml:Point" must be terminated by the matching end-tag "</gml:Point>".)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!' ->WKT",
            Err(r#"->WKT was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!' ->GML",
            Err(r#"->GML was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!' ->KML",
            Err(r#"->KML was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!' ->WKB ->HEX",
            Err(r#"->WKB was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!' ->GEOJSON",
            Err(r#"->GEOJSON was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-x' ->WKT",
            Err(r#"->WKT was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-x' ->GML",
            Err(r#"->GML was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-x' ->KML",
            Err(r#"->KML was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-x' ->WKB ->HEX",
            Err(r#"->WKB was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-x' ->GEOJSON",
            Err(r#"->GEOJSON was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><![CDATA[x]]>' ->WKT",
            Err(r#"->WKT was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><![CDATA[x]]>' ->GML",
            Err(r#"->GML was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><![CDATA[x]]>' ->KML",
            Err(r#"->KML was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><![CDATA[x]]>' ->WKB ->HEX",
            Err(r#"->WKB was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><![CDATA[x]]>' ->GEOJSON",
            Err(r#"->GEOJSON was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<Point><!-' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><!-' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><!-' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><!-' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><!-' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><!x' ->WKT",
            Err(
                r"->WKT was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><!x' ->GML",
            Err(
                r"->GML was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><!x' ->KML",
            Err(
                r"->KML was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><!x' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><!x' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![x' ->WKT",
            Err(
                r"->WKT was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![x' ->GML",
            Err(
                r"->GML was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![x' ->KML",
            Err(
                r"->KML was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![x' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![x' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
    ]);
}

#[test]
fn gml_read_14() {
    check(&[
        (
            r"'<Point><![CDATA[x' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><![CDATA[x' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><![CDATA[x' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><![CDATA[x' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><![CDATA[x' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></' ->WKT",
            Err(
                r#"->WKT was given invalid input. (The element type "Point" must be terminated by the matching end-tag "</Point>".)"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></' ->GML",
            Err(
                r#"->GML was given invalid input. (The element type "Point" must be terminated by the matching end-tag "</Point>".)"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></' ->KML",
            Err(
                r#"->KML was given invalid input. (The element type "Point" must be terminated by the matching end-tag "</Point>".)"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></' ->WKB ->HEX",
            Err(
                r#"->WKB was given invalid input. (The element type "Point" must be terminated by the matching end-tag "</Point>".)"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></' ->GEOJSON",
            Err(
                r#"->GEOJSON was given invalid input. (The element type "Point" must be terminated by the matching end-tag "</Point>".)"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>x' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>x' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>x' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>x' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>x' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<MultiGeometry><MultiGeometry></MultiGeometry></MultiGeometry>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><MultiGeometry></MultiGeometry></MultiGeometry>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><MultiGeometry></MultiGeometry></MultiGeometry>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><MultiGeometry></MultiGeometry></MultiGeometry>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><MultiGeometry></MultiGeometry></MultiGeometry>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!-x' ->WKT",
            Err(r#"->WKT was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<!-x' ->GML",
            Err(r#"->GML was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<!-x' ->KML",
            Err(r#"->KML was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<!-x' ->WKB ->HEX",
            Err(r#"->WKB was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<!-x' ->GEOJSON",
            Err(r#"->GEOJSON was given invalid input. (Comment must start with "<!--".)"#),
        ),
        (
            r"'< x/>' ->WKT",
            Err(
                r"->WKT was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'< x/>' ->GML",
            Err(
                r"->GML was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'< x/>' ->KML",
            Err(
                r"->KML was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'< x/>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'< x/>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi?>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi?>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi?>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi?>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi?>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<gml:Point id><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKT",
            Err(
                r#"->WKT was given invalid input. (Attribute name "id" associated with an element type "gml:Point" must be followed by the ' = ' character.)"#,
            ),
        ),
        (
            r"'<gml:Point id><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GML",
            Err(
                r#"->GML was given invalid input. (Attribute name "id" associated with an element type "gml:Point" must be followed by the ' = ' character.)"#,
            ),
        ),
        (
            r"'<gml:Point id><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->KML",
            Err(
                r#"->KML was given invalid input. (Attribute name "id" associated with an element type "gml:Point" must be followed by the ' = ' character.)"#,
            ),
        ),
        (
            r"'<gml:Point id><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(
                r#"->WKB was given invalid input. (Attribute name "id" associated with an element type "gml:Point" must be followed by the ' = ' character.)"#,
            ),
        ),
        (
            r"'<gml:Point id><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(
                r#"->GEOJSON was given invalid input. (Attribute name "id" associated with an element type "gml:Point" must be followed by the ' = ' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKT"#,
            Err(
                r#"->WKT was given invalid input. (The value of attribute "id" associated with an element type "gml:Point" must not contain the '<' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GML"#,
            Err(
                r#"->GML was given invalid input. (The value of attribute "id" associated with an element type "gml:Point" must not contain the '<' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->KML"#,
            Err(
                r#"->KML was given invalid input. (The value of attribute "id" associated with an element type "gml:Point" must not contain the '<' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKB ->HEX"#,
            Err(
                r#"->WKB was given invalid input. (The value of attribute "id" associated with an element type "gml:Point" must not contain the '<' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GEOJSON"#,
            Err(
                r#"->GEOJSON was given invalid input. (The value of attribute "id" associated with an element type "gml:Point" must not contain the '<' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a"' JSON-> ->WKT"#,
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"a"' JSON-> ->GML"#,
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"a"' JSON-> ->KML"#,
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"a"' JSON-> ->WKB ->HEX"#,
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"a"' JSON-> ->GEOJSON"#,
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point x>' ->WKT",
            Err(
                r#"->WKT was given invalid input. (The end-tag for element type "gml:Point" must end with a '>' delimiter.)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point x>' ->GML",
            Err(
                r#"->GML was given invalid input. (The end-tag for element type "gml:Point" must end with a '>' delimiter.)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point x>' ->KML",
            Err(
                r#"->KML was given invalid input. (The end-tag for element type "gml:Point" must end with a '>' delimiter.)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point x>' ->WKB ->HEX",
            Err(
                r#"->WKB was given invalid input. (The end-tag for element type "gml:Point" must end with a '>' delimiter.)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point x>' ->GEOJSON",
            Err(
                r#"->GEOJSON was given invalid input. (The end-tag for element type "gml:Point" must end with a '>' delimiter.)"#,
            ),
        ),
    ]);
}

#[test]
fn gml_read_15() {
    check(&[
        (
            r"'<gml:Point><gml:coordinates>1,2 < 3</gml:coordinates></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 < 3</gml:coordinates></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 < 3</gml:coordinates></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 < 3</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 < 3</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<!-- c' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!-- c' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!-- c' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!-- c' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!-- c' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<?pi' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<?pi' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<?pi' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<?pi' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<?pi' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-- c' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-- c' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-- c' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-- c' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-- c' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!DOCTYPE gml><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE gml><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE gml><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE gml><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE gml><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE x [ ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE x [ ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE x [ ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE x [ ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE x [ ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r#"'"<!DOCTYPE x [ <!ENTITY e \"v\"> ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKT"#,
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r#"'"<!DOCTYPE x [ <!ENTITY e \"v\"> ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GML"#,
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r#"'"<!DOCTYPE x [ <!ENTITY e \"v\"> ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->KML"#,
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r#"'"<!DOCTYPE x [ <!ENTITY e \"v\"> ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKB ->HEX"#,
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r#"'"<!DOCTYPE x [ <!ENTITY e \"v\"> ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GEOJSON"#,
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#12,2</gml:coordinates></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#12,2</gml:coordinates></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#12,2</gml:coordinates></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#12,2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#12,2</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 & 2</gml:coordinates></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 & 2</gml:coordinates></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 & 2</gml:coordinates></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 & 2</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 & 2</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r"'<gml:LineString><gml:box><gml:coordinates>1,2 3,4</gml:coordinates></gml:box></gml:LineString>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Envelope cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:box><gml:coordinates>1,2 3,4</gml:coordinates></gml:box></gml:LineString>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Envelope cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:box><gml:coordinates>1,2 3,4</gml:coordinates></gml:box></gml:LineString>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Envelope cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:box><gml:coordinates>1,2 3,4</gml:coordinates></gml:box></gml:LineString>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Envelope cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:box><gml:coordinates>1,2 3,4</gml:coordinates></gml:box></gml:LineString>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Envelope cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:Point><gml:box><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:box></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence)",
            ),
        ),
        (
            r"'<gml:Point><gml:box><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:box></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence)",
            ),
        ),
        (
            r"'<gml:Point><gml:box><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:box></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence)",
            ),
        ),
        (
            r"'<gml:Point><gml:box><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:box></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence)",
            ),
        ),
        (
            r"'<gml:Point><gml:box><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:box></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:coordinates>1,2</gml:coordinates></gml:coord></gml:Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:coordinates>1,2</gml:coordinates></gml:coord></gml:Point>' ->GML",
            Err(
                r"->GML was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:coordinates>1,2</gml:coordinates></gml:coord></gml:Point>' ->KML",
            Err(
                r"->KML was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:coordinates>1,2</gml:coordinates></gml:coord></gml:Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:coordinates>1,2</gml:coordinates></gml:coord></gml:Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
    ]);
}

#[test]
fn gml_read_16() {
    check(&[
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:pointMember></gml:MultiPoint>' ->WKT",
            Err(r"->WKT was given invalid input. (com.vividsolutions.jts.geom.LinearRing)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:pointMember></gml:MultiPoint>' ->GML",
            Err(r"->GML was given invalid input. (com.vividsolutions.jts.geom.LinearRing)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:pointMember></gml:MultiPoint>' ->KML",
            Err(r"->KML was given invalid input. (com.vividsolutions.jts.geom.LinearRing)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:pointMember></gml:MultiPoint>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (com.vividsolutions.jts.geom.LinearRing)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:pointMember></gml:MultiPoint>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.LinearRing)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:pointMember></gml:MultiPoint>' ->WKT",
            Err(r"->WKT was given invalid input. (com.vividsolutions.jts.geom.Polygon)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:pointMember></gml:MultiPoint>' ->GML",
            Err(r"->GML was given invalid input. (com.vividsolutions.jts.geom.Polygon)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:pointMember></gml:MultiPoint>' ->KML",
            Err(r"->KML was given invalid input. (com.vividsolutions.jts.geom.Polygon)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:pointMember></gml:MultiPoint>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (com.vividsolutions.jts.geom.Polygon)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:pointMember></gml:MultiPoint>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.Polygon)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:lineStringMember></gml:MultiLineString>' ->WKT",
            Err(r"->WKT was given invalid input. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:lineStringMember></gml:MultiLineString>' ->GML",
            Err(r"->GML was given invalid input. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:lineStringMember></gml:MultiLineString>' ->KML",
            Err(r"->KML was given invalid input. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:lineStringMember></gml:MultiLineString>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:lineStringMember></gml:MultiLineString>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:lineStringMember></gml:MultiLineString>' ->WKT",
            Err(r"->WKT was given invalid input. (com.vividsolutions.jts.geom.Polygon)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:lineStringMember></gml:MultiLineString>' ->GML",
            Err(r"->GML was given invalid input. (com.vividsolutions.jts.geom.Polygon)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:lineStringMember></gml:MultiLineString>' ->KML",
            Err(r"->KML was given invalid input. (com.vividsolutions.jts.geom.Polygon)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:lineStringMember></gml:MultiLineString>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (com.vividsolutions.jts.geom.Polygon)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:lineStringMember></gml:MultiLineString>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.Polygon)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:polygonMember></gml:MultiPolygon>' ->WKT",
            Err(r"->WKT was given invalid input. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:polygonMember></gml:MultiPolygon>' ->GML",
            Err(r"->GML was given invalid input. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:polygonMember></gml:MultiPolygon>' ->KML",
            Err(r"->KML was given invalid input. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:polygonMember></gml:MultiPolygon>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:polygonMember></gml:MultiPolygon>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:polygonMember></gml:MultiPolygon>' ->WKT",
            Err(r"->WKT was given invalid input. (com.vividsolutions.jts.geom.LineString)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:polygonMember></gml:MultiPolygon>' ->GML",
            Err(r"->GML was given invalid input. (com.vividsolutions.jts.geom.LineString)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:polygonMember></gml:MultiPolygon>' ->KML",
            Err(r"->KML was given invalid input. (com.vividsolutions.jts.geom.LineString)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:polygonMember></gml:MultiPolygon>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (com.vividsolutions.jts.geom.LineString)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:polygonMember></gml:MultiPolygon>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.LineString)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:outerBoundaryIs></gml:Polygon>' ->WKT",
            Err(
                r"->WKT was given invalid input. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:outerBoundaryIs></gml:Polygon>' ->GML",
            Err(
                r"->GML was given invalid input. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:outerBoundaryIs></gml:Polygon>' ->KML",
            Err(
                r"->KML was given invalid input. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:outerBoundaryIs></gml:Polygon>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:outerBoundaryIs></gml:Polygon>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord/></gml:Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord/></gml:Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord/></gml:Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord/></gml:Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord/></gml:Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:box/>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:box/>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:box/>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:box/>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:box/>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint></gml:pointMember></gml:MultiPoint>' ->WKT",
            Err(r"->WKT was given invalid input. (com.vividsolutions.jts.geom.MultiPoint)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint></gml:pointMember></gml:MultiPoint>' ->GML",
            Err(r"->GML was given invalid input. (com.vividsolutions.jts.geom.MultiPoint)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint></gml:pointMember></gml:MultiPoint>' ->KML",
            Err(r"->KML was given invalid input. (com.vividsolutions.jts.geom.MultiPoint)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint></gml:pointMember></gml:MultiPoint>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (com.vividsolutions.jts.geom.MultiPoint)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint></gml:pointMember></gml:MultiPoint>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.MultiPoint)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:MultiLineString><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:MultiLineString></gml:lineStringMember></gml:MultiLineString>' ->WKT",
            Err(r"->WKT was given invalid input. (com.vividsolutions.jts.geom.MultiLineString)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:MultiLineString><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:MultiLineString></gml:lineStringMember></gml:MultiLineString>' ->GML",
            Err(r"->GML was given invalid input. (com.vividsolutions.jts.geom.MultiLineString)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:MultiLineString><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:MultiLineString></gml:lineStringMember></gml:MultiLineString>' ->KML",
            Err(r"->KML was given invalid input. (com.vividsolutions.jts.geom.MultiLineString)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:MultiLineString><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:MultiLineString></gml:lineStringMember></gml:MultiLineString>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (com.vividsolutions.jts.geom.MultiLineString)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:MultiLineString><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:MultiLineString></gml:lineStringMember></gml:MultiLineString>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.MultiLineString)",
            ),
        ),
    ]);
}

#[test]
fn gml_read_17() {
    check(&[
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:MultiPolygon><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:MultiPolygon></gml:polygonMember></gml:MultiPolygon>' ->WKT",
            Err(r"->WKT was given invalid input. (com.vividsolutions.jts.geom.MultiPolygon)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:MultiPolygon><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:MultiPolygon></gml:polygonMember></gml:MultiPolygon>' ->GML",
            Err(r"->GML was given invalid input. (com.vividsolutions.jts.geom.MultiPolygon)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:MultiPolygon><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:MultiPolygon></gml:polygonMember></gml:MultiPolygon>' ->KML",
            Err(r"->KML was given invalid input. (com.vividsolutions.jts.geom.MultiPolygon)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:MultiPolygon><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:MultiPolygon></gml:polygonMember></gml:MultiPolygon>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (com.vividsolutions.jts.geom.MultiPolygon)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:MultiPolygon><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:MultiPolygon></gml:polygonMember></gml:MultiPolygon>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.MultiPolygon)"),
        ),
        (
            r"'<gml:Point><!-- c' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><!-- c' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><!-- c' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><!-- c' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><!-- c' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><?pi' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><?pi' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><?pi' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><?pi' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><?pi' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"&foo;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKT"#,
            Err(
                r#"->WKT was given invalid input. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"&foo;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GML"#,
            Err(
                r#"->GML was given invalid input. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"&foo;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->KML"#,
            Err(
                r#"->KML was given invalid input. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"&foo;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKB ->HEX"#,
            Err(
                r#"->WKB was given invalid input. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"&foo;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GEOJSON"#,
            Err(
                r#"->GEOJSON was given invalid input. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"1 & 2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKT"#,
            Err(
                r"->WKT was given invalid input. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"1 & 2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GML"#,
            Err(
                r"->GML was given invalid input. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"1 & 2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->KML"#,
            Err(
                r"->KML was given invalid input. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"1 & 2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKB ->HEX"#,
            Err(
                r"->WKB was given invalid input. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"1 & 2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GEOJSON"#,
            Err(
                r"->GEOJSON was given invalid input. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"&#12;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKT"#,
            Err(r#"->WKT was given invalid input. (Character reference "&#)"#),
        ),
        (
            r#"'"<gml:Point id=\"&#12;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GML"#,
            Err(r#"->GML was given invalid input. (Character reference "&#)"#),
        ),
        (
            r#"'"<gml:Point id=\"&#12;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->KML"#,
            Err(r#"->KML was given invalid input. (Character reference "&#)"#),
        ),
        (
            r#"'"<gml:Point id=\"&#12;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKB ->HEX"#,
            Err(r#"->WKB was given invalid input. (Character reference "&#)"#),
        ),
        (
            r#"'"<gml:Point id=\"&#12;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GEOJSON"#,
            Err(r#"->GEOJSON was given invalid input. (Character reference "&#)"#),
        ),
        (
            r#"'"<gml:Point id=\"&#12,2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKT"#,
            Err(
                r"->WKT was given invalid input. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"&#12,2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GML"#,
            Err(
                r"->GML was given invalid input. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"&#12,2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->KML"#,
            Err(
                r"->KML was given invalid input. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"&#12,2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->WKB ->HEX"#,
            Err(
                r"->WKB was given invalid input. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"&#12,2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> ->GEOJSON"#,
            Err(
                r"->GEOJSON was given invalid input. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,x</gml:coordinates></gml:Point>' ->WKT",
            Err(r#"->WKT was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,x</gml:coordinates></gml:Point>' ->GML",
            Err(r#"->GML was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,x</gml:coordinates></gml:Point>' ->KML",
            Err(r#"->KML was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,x</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(r#"->WKB was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,x</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(r#"->GEOJSON was given invalid input. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3 4,5,y</gml:coordinates></gml:Point>' ->WKT",
            Err(r#"->WKT was given invalid input. (For input string: "y")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3 4,5,y</gml:coordinates></gml:Point>' ->GML",
            Err(r#"->GML was given invalid input. (For input string: "y")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3 4,5,y</gml:coordinates></gml:Point>' ->KML",
            Err(r#"->KML was given invalid input. (For input string: "y")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3 4,5,y</gml:coordinates></gml:Point>' ->WKB ->HEX",
            Err(r#"->WKB was given invalid input. (For input string: "y")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3 4,5,y</gml:coordinates></gml:Point>' ->GEOJSON",
            Err(r#"->GEOJSON was given invalid input. (For input string: "y")"#),
        ),
        (
            r"'<gml:Polygon><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:Polygon>' ->WKT",
            Err(
                r"->WKT was given invalid input. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:Polygon>' ->GML",
            Err(
                r"->GML was given invalid input. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:Polygon>' ->KML",
            Err(
                r"->KML was given invalid input. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:Polygon>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:Polygon>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:Point><gml:coordinates>0,0</gml:coordinates></gml:Point></gml:Polygon>' ->WKT",
            Err(
                r"->WKT was given invalid input. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:Point><gml:coordinates>0,0</gml:coordinates></gml:Point></gml:Polygon>' ->GML",
            Err(
                r"->GML was given invalid input. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:Point><gml:coordinates>0,0</gml:coordinates></gml:Point></gml:Polygon>' ->KML",
            Err(
                r"->KML was given invalid input. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:Point><gml:coordinates>0,0</gml:coordinates></gml:Point></gml:Polygon>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:Point><gml:coordinates>0,0</gml:coordinates></gml:Point></gml:Polygon>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
    ]);
}

#[test]
fn gml_read_18() {
    check(&[
        (
            r"'<Point><coordinates>1,2,3</coordinates></Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<Point><coordinates>1,2,3</coordinates></Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0,3.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<Point><coordinates>1,2,3</coordinates></Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0,3.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<Point><coordinates>1,2,3</coordinates></Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<Point><coordinates>1,2,3</coordinates></Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0,3.0]}"]"#),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<Point><coordinates>1,2,3,4</coordinates></Point>' ->WKT",
            Err(r"->WKT was given invalid input. (dimension must be <= 3)"),
        ),
        (
            r"'<Point><coordinates>1,2,3,4</coordinates></Point>' ->GML",
            Err(r"->GML was given invalid input. (dimension must be <= 3)"),
        ),
        (
            r"'<Point><coordinates>1,2,3,4</coordinates></Point>' ->KML",
            Err(r"->KML was given invalid input. (dimension must be <= 3)"),
        ),
        (
            r"'<Point><coordinates>1,2,3,4</coordinates></Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (dimension must be <= 3)"),
        ),
        (
            r"'<Point><coordinates>1,2,3,4</coordinates></Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (dimension must be <= 3)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><coord><X>1</X></coord></Point>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><coord><X>1</X></coord></Point>' ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><coord><X>1</X></coord></Point>' ->KML",
            Err(
                r"->KML was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><coord><X>1</X></coord></Point>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><coord><X>1</X></coord></Point>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<LineString><coordinates>1,2,3 4,5,6</coordinates></LineString>' ->WKT",
            Ok(r#"["LINESTRING (1 2, 4 5)"]"#),
        ),
        (
            r"'<LineString><coordinates>1,2,3 4,5,6</coordinates></LineString>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0,3.0 4.0,5.0,6.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<LineString><coordinates>1,2,3 4,5,6</coordinates></LineString>' ->KML",
            Ok(
                r#"["<LineString><coordinates>1.0,2.0,3.0 4.0,5.0,6.0 </coordinates></LineString>"]"#,
            ),
        ),
        (
            r"'<LineString><coordinates>1,2,3 4,5,6</coordinates></LineString>' ->WKB ->HEX",
            Ok(
                r#"["0000000002000000023ff0000000000000400000000000000040100000000000004014000000000000"]"#,
            ),
        ),
        (
            r"'<LineString><coordinates>1,2,3 4,5,6</coordinates></LineString>' ->GEOJSON",
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0,3.0],[4.0,5.0,6.0]]}"]"#),
        ),
        (
            r"'<LineString><coordinates>1,2</coordinates></LineString>' ->WKT",
            Err(
                r"->WKT was given invalid input. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<LineString><coordinates>1,2</coordinates></LineString>' ->GML",
            Err(
                r"->GML was given invalid input. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<LineString><coordinates>1,2</coordinates></LineString>' ->KML",
            Err(
                r"->KML was given invalid input. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<LineString><coordinates>1,2</coordinates></LineString>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<LineString><coordinates>1,2</coordinates></LineString>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<LinearRing><coordinates>0,0 1,0 1,1 0,0</coordinates></LinearRing>' ->WKT",
            Ok(r#"["LINEARRING (0 0, 1 0, 1 1, 0 0)"]"#),
        ),
        (
            r"'<LinearRing><coordinates>0,0 1,0 1,1 0,0</coordinates></LinearRing>' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>0.0,0.0 1.0,0.0 1.0,1.0 0.0,0.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'<LinearRing><coordinates>0,0 1,0 1,1 0,0</coordinates></LinearRing>' ->KML",
            Ok(
                r#"["<LineString><coordinates>0.0,0.0 1.0,0.0 1.0,1.0 0.0,0.0 </coordinates></LineString>"]"#,
            ),
        ),
        (
            r"'<LinearRing><coordinates>0,0 1,0 1,1 0,0</coordinates></LinearRing>' ->WKB ->HEX",
            Ok(
                r#"["000000000200000004000000000000000000000000000000003ff000000000000000000000000000003ff00000000000003ff000000000000000000000000000000000000000000000"]"#,
            ),
        ),
        (
            r"'<LinearRing><coordinates>0,0 1,0 1,1 0,0</coordinates></LinearRing>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (UnsupportedOperationException)"),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon>' ->WKT",
            Ok(r#"["POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0))"]"#),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon>' ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon>' ->KML",
            Ok(
                r#"["<Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs></Polygon>"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon>' ->WKB ->HEX",
            Ok(
                r#"["000000000300000001000000050000000000000000000000000000000000000000000000004024000000000000402400000000000040240000000000004024000000000000000000000000000000000000000000000000000000000000"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"Polygon\",\"coordinates\":[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]]]}"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2,2 2,4 4,4 4,2 2,2</coordinates></LinearRing></innerBoundaryIs></Polygon>' ->WKT",
            Ok(r#"["POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0), (2 2, 2 4, 4 4, 4 2, 2 2))"]"#),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2,2 2,4 4,4 4,2 2,2</coordinates></LinearRing></innerBoundaryIs></Polygon>' ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2,2 2,4 4,4 4,2 2,2</coordinates></LinearRing></innerBoundaryIs></Polygon>' ->KML",
            Ok(
                r#"["<Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </coordinates></LinearRing></innerBoundaryIs></Polygon>"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2,2 2,4 4,4 4,2 2,2</coordinates></LinearRing></innerBoundaryIs></Polygon>' ->WKB ->HEX",
            Ok(
                r#"["000000000300000002000000050000000000000000000000000000000000000000000000004024000000000000402400000000000040240000000000004024000000000000000000000000000000000000000000000000000000000000000000054000000000000000400000000000000040000000000000004010000000000000401000000000000040100000000000004010000000000000400000000000000040000000000000004000000000000000"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2,2 2,4 4,4 4,2 2,2</coordinates></LinearRing></innerBoundaryIs></Polygon>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"Polygon\",\"coordinates\":[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]],[[2.0,2.0],[2.0,4.0],[4.0,4.0],[4.0,2.0],[2.0,2.0]]]}"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><Point><coordinates>1,2</coordinates></Point><LineString><coordinates>1,2 3,4</coordinates></LineString></MultiGeometry>' ->WKT",
            Ok(r#"["GEOMETRYCOLLECTION (POINT (1 2), LINESTRING (1 2, 3 4))"]"#),
        ),
        (
            r"'<MultiGeometry><Point><coordinates>1,2</coordinates></Point><LineString><coordinates>1,2 3,4</coordinates></LineString></MultiGeometry>' ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point></gml:geometryMember><gml:geometryMember><gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><Point><coordinates>1,2</coordinates></Point><LineString><coordinates>1,2 3,4</coordinates></LineString></MultiGeometry>' ->KML",
            Ok(
                r#"["<MultiGeometry><geometryMember><Point><coordinates>1.0,2.0 </coordinates></Point></geometryMember><geometryMember><LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString></geometryMember></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><Point><coordinates>1,2</coordinates></Point><LineString><coordinates>1,2 3,4</coordinates></LineString></MultiGeometry>' ->WKB ->HEX",
            Ok(
                r#"["00000000070000000200000000013ff000000000000040000000000000000000000002000000023ff0000000000000400000000000000040080000000000004010000000000000"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><Point><coordinates>1,2</coordinates></Point><LineString><coordinates>1,2 3,4</coordinates></LineString></MultiGeometry>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"GeometryCollection\",\"geometries\":[{\"type\":\"Point\",\"coordinates\":[1.0,2.0]},{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}]}"]"#,
            ),
        ),
        (
            r"'<MultiGeometry></MultiGeometry>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry></MultiGeometry>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry></MultiGeometry>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry></MultiGeometry>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry></MultiGeometry>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>' ->WKT",
            Ok(r#"["GEOMETRYCOLLECTION (POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0)))"]"#),
        ),
        (
            r"'<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>' ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>' ->KML",
            Ok(
                r#"["<MultiGeometry><geometryMember><Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs></Polygon></geometryMember></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>' ->WKB ->HEX",
            Ok(
                r#"["000000000700000001000000000300000001000000050000000000000000000000000000000000000000000000004024000000000000402400000000000040240000000000004024000000000000000000000000000000000000000000000000000000000000"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"GeometryCollection\",\"geometries\":[{\"type\":\"Polygon\",\"coordinates\":[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]]]}]}"]"#,
            ),
        ),
    ]);
}

#[test]
fn gml_read_19() {
    check(&[
        (
            r"'<MultiGeometry><geometryMember><Point><coordinates>1,2</coordinates></Point></geometryMember></MultiGeometry>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><geometryMember><Point><coordinates>1,2</coordinates></Point></geometryMember></MultiGeometry>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><geometryMember><Point><coordinates>1,2</coordinates></Point></geometryMember></MultiGeometry>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><geometryMember><Point><coordinates>1,2</coordinates></Point></geometryMember></MultiGeometry>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><geometryMember><Point><coordinates>1,2</coordinates></Point></geometryMember></MultiGeometry>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></MultiGeometry>' ->WKT",
            Ok(r#"["GEOMETRYCOLLECTION (GEOMETRYCOLLECTION (POINT (1 2)))"]"#),
        ),
        (
            r"'<MultiGeometry><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></MultiGeometry>' ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></MultiGeometry>' ->KML",
            Ok(
                r#"["<MultiGeometry><geometryMember><MultiGeometry><geometryMember><Point><coordinates>1.0,2.0 </coordinates></Point></geometryMember></MultiGeometry></geometryMember></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></MultiGeometry>' ->WKB ->HEX",
            Ok(
                r#"["00000000070000000100000000070000000100000000013ff00000000000004000000000000000"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></MultiGeometry>' ->GEOJSON",
            Ok(
                r#"["{\"type\":\"GeometryCollection\",\"geometries\":[{\"type\":\"GeometryCollection\",\"geometries\":[{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}]}]}"]"#,
            ),
        ),
        (
            r"'<kml><Point><coordinates>1,2</coordinates></Point></kml>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml><Point><coordinates>1,2</coordinates></Point></kml>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml><Point><coordinates>1,2</coordinates></Point></kml>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml><Point><coordinates>1,2</coordinates></Point></kml>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml><Point><coordinates>1,2</coordinates></Point></kml>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml/>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml/>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml/>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml/>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml/>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r#"'"<Point xmlns=\"http://www.opengis.net/kml/2.2\"><coordinates>1,2</coordinates></Point>"' JSON-> ->WKT"#,
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r#"'"<Point xmlns=\"http://www.opengis.net/kml/2.2\"><coordinates>1,2</coordinates></Point>"' JSON-> ->GML"#,
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r#"'"<Point xmlns=\"http://www.opengis.net/kml/2.2\"><coordinates>1,2</coordinates></Point>"' JSON-> ->KML"#,
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r#"'"<Point xmlns=\"http://www.opengis.net/kml/2.2\"><coordinates>1,2</coordinates></Point>"' JSON-> ->WKB ->HEX"#,
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r#"'"<Point xmlns=\"http://www.opengis.net/kml/2.2\"><coordinates>1,2</coordinates></Point>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><Point/></Point>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><Point/></Point>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><Point/></Point>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><Point/></Point>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><Point/></Point>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point><Point/>' ->WKT",
            Err(
                r"->WKT was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point><Point/>' ->GML",
            Err(
                r"->GML was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point><Point/>' ->KML",
            Err(
                r"->KML was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point><Point/>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point><Point/>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<Foo><coordinates>1,2</coordinates></Foo>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo><coordinates>1,2</coordinates></Foo>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo><coordinates>1,2</coordinates></Foo>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo><coordinates>1,2</coordinates></Foo>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Foo><coordinates>1,2</coordinates></Foo>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>' ->KML",
            Err(
                r"->KML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>junk' ->WKT",
            Err(r"->WKT was given invalid input. (Content is not allowed in trailing section.)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>junk' ->GML",
            Err(r"->GML was given invalid input. (Content is not allowed in trailing section.)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>junk' ->KML",
            Err(r"->KML was given invalid input. (Content is not allowed in trailing section.)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>junk' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (Content is not allowed in trailing section.)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>junk' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Content is not allowed in trailing section.)",
            ),
        ),
        (
            r#"'"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Point><coordinates>1,2</coordinates></Point>"' JSON-> ->WKT"#,
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r#"'"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Point><coordinates>1,2</coordinates></Point>"' JSON-> ->GML"#,
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r#"'"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Point><coordinates>1,2</coordinates></Point>"' JSON-> ->KML"#,
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (
            r#"'"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Point><coordinates>1,2</coordinates></Point>"' JSON-> ->WKB ->HEX"#,
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r#"'"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Point><coordinates>1,2</coordinates></Point>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
    ]);
}

#[test]
fn gml_read_20() {
    check(&[
        (
            r#"'"<Point><coordinates>1,2,3</coordinates></Point>\n"' JSON-> ->WKT"#,
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3</coordinates></Point>\n"' JSON-> ->GML"#,
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0,3.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3</coordinates></Point>\n"' JSON-> ->KML"#,
            Ok(r#"["<Point><coordinates>1.0,2.0,3.0 </coordinates></Point>"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3</coordinates></Point>\n"' JSON-> ->WKB ->HEX"#,
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3</coordinates></Point>\n"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0,3.0]}"]"#),
        ),
        (
            r"'<Point><coordinates> 1,2,3 </coordinates></Point>' ->WKT",
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r"'<Point><coordinates> 1,2,3 </coordinates></Point>' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0,3.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'<Point><coordinates> 1,2,3 </coordinates></Point>' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0,3.0 </coordinates></Point>"]"#),
        ),
        (
            r"'<Point><coordinates> 1,2,3 </coordinates></Point>' ->WKB ->HEX",
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r"'<Point><coordinates> 1,2,3 </coordinates></Point>' ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0,3.0]}"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3\n</coordinates></Point>"' JSON-> ->WKT"#,
            Ok(r#"["POINT (1 2)"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3\n</coordinates></Point>"' JSON-> ->GML"#,
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0,3.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3\n</coordinates></Point>"' JSON-> ->KML"#,
            Ok(r#"["<Point><coordinates>1.0,2.0,3.0 </coordinates></Point>"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3\n</coordinates></Point>"' JSON-> ->WKB ->HEX"#,
            Ok(r#"["00000000013ff00000000000004000000000000000"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3\n</coordinates></Point>"' JSON-> ->GEOJSON"#,
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0,3.0]}"]"#),
        ),
        (
            r"'<MultiGeometry><coordinates>1,2</coordinates></MultiGeometry>' ->WKT",
            Err(
                r"->WKT was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<MultiGeometry><coordinates>1,2</coordinates></MultiGeometry>' ->GML",
            Err(
                r"->GML was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<MultiGeometry><coordinates>1,2</coordinates></MultiGeometry>' ->KML",
            Err(
                r"->KML was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<MultiGeometry><coordinates>1,2</coordinates></MultiGeometry>' ->WKB ->HEX",
            Err(
                r"->WKB was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<MultiGeometry><coordinates>1,2</coordinates></MultiGeometry>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<MultiPoint><pointMember><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></pointMember></MultiPoint>' ->WKT",
            Err(r"->WKT was given invalid input. (com.vividsolutions.jts.geom.GeometryCollection)"),
        ),
        (
            r"'<MultiPoint><pointMember><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></pointMember></MultiPoint>' ->GML",
            Err(r"->GML was given invalid input. (com.vividsolutions.jts.geom.GeometryCollection)"),
        ),
        (
            r"'<MultiPoint><pointMember><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></pointMember></MultiPoint>' ->KML",
            Err(r"->KML was given invalid input. (com.vividsolutions.jts.geom.GeometryCollection)"),
        ),
        (
            r"'<MultiPoint><pointMember><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></pointMember></MultiPoint>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (com.vividsolutions.jts.geom.GeometryCollection)"),
        ),
        (
            r"'<MultiPoint><pointMember><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></pointMember></MultiPoint>' ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (com.vividsolutions.jts.geom.GeometryCollection)",
            ),
        ),
        (
            r"'<Polygon/>' ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Polygon/>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Polygon/>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Polygon/>' ->WKB ->HEX",
            Err(r"->WKB was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<Polygon/>' ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3,4</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (dimension must be <= 3)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 3,4</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates>3,4</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates></gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>   </gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a coordinate sequence without a non-null touple to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>x,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r#"GEO.GML expects valid a GML STRING. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 </gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>,</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Z>3</gml:Z><gml:Y>2</gml:Y></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:Z>3</gml:Z><gml:Y>2</gml:Y><gml:X>1</gml:X></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,-11.25],[22.5,-11.25],[22.5,11.25],[0.0,11.25],[0.0,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:Z>3</gml:Z><gml:Z>4</gml:Z></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a coordinate with more than 3 axis)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:X> 2 </gml:X></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:foo>3</gml:foo></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X/></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>  </gml:X></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (empty String)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 2</gml:X></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r#"GEO.GML expects valid a GML STRING. (For input string: "1 2")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>0x10</gml:X><gml:Y>1.5f</gml:Y></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r#"GEO.GML expects valid a GML STRING. (For input string: "0x10")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1e2</gml:X><gml:Y>-2.5d</gml:Y></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[90.0,-11.25],[112.5,-11.25],[112.5,0.0],[90.0,0.0],[90.0,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point/>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a point without exactly one coordinate)",
            ),
        ),
    ]);
}

#[test]
fn gml_read_21() {
    check(&[
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:foo/></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates/></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,5</gml:coordinates></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2,3 4,5,6</gml:coordinates></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1 2 3</gml:coordinates></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,-11.25],[22.5,-11.25],[22.5,11.25],[0.0,11.25],[0.0,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2</gml:coordinates></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coordinates>5,6</gml:coordinates></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\";\">1,2;3,4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\" \" cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2x3,4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Err(r#"GEO.GML expects valid a GML STRING. (For input string: "2 3")"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\"\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Err(r#"GEO.GML expects valid a GML STRING. (For input string: ",")"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,</gml:coordinates></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 </gml:coordinates></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2 3,4\n5,6</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\n3,4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 5,6 7,8 9,10</gml:coordinates></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 x</gml:coordinates></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Err(r#"GEO.GML expects valid a GML STRING. (For input string: "x")"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 1,2 3,4 1,2 3,4</gml:coordinates></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord></gml:LinearRing>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:LinearRing><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LinearRing>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LinearRing></gml:LinearRing>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>3,4</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:MultiPoint></gml:MultiPoint>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:pointMember></gml:MultiPoint>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.LineString)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember></gml:pointMember></gml:MultiPoint>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing></gml:lineStringMember><gml:lineStringMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:MultiLineString></gml:MultiLineString>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
    ]);
}

#[test]
fn gml_read_22() {
    check(&[
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:polygonMember></gml:MultiPolygon>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.LinearRing)"),
        ),
        (
            r"'<gml:MultiPolygon></gml:MultiPolygon>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:geometryMember><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry></gml:MultiGeometry>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry></gml:geometryMember></gml:MultiGeometry>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:geometryMember></gml:MultiGeometry>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:point><gml:coordinates>1,2</gml:coordinates></gml:point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:POINT><gml:coordinates>1,2</gml:coordinates></gml:POINT>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Foo><gml:coordinates>1,2</gml:coordinates></gml:Foo>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:Foo/></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><foo>1,2</foo></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><x' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>junk' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Content is not allowed in trailing section.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r#"'"<?xml version=\"1.0\"?><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<!-- c --><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><!-- c --><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates><![CDATA[1,2]]></gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>&#49;,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&amp;</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r#"GEO.GML expects valid a GML STRING. (For input string: "2&")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&foo;</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r#"GEO.GML expects valid a GML STRING. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a\" id=\"b\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Err(
                r#"GEO.GML expects valid a GML STRING. (Attribute "id" was already specified for element "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point id=a><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r#"GEO.GML expects valid a GML STRING. (Open quote is expected for attribute "id" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point a=\'1\'><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r#"GEO.GML expects valid a GML STRING. (Open quote is expected for attribute "a" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r#"'"<gml:Point xmlns:gml=\"http://www.opengis.net/gml\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<gml:Point >\t<gml:coordinates>1,2</gml:coordinates>\n</gml:Point >"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<gml:Point>\n  <gml:coordinates>\n    1,2\n  </gml:coordinates>\n</gml:Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>\n1,2\n3,4\n</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\t3,4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<gml:Point><gml:coord><gml:X>\n1\n</gml:X><gml:Y>\t2\t</gml:Y></gml:coord></gml:Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 </gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:<' 4 F GEO.GML ->GEOJSON",
            Err(
                r#"GEO.GML expects valid a GML STRING. (Element type "gml:" must be followed by either attribute specifications, ">" or "/>".)"#,
            ),
        ),
        (
            r"'<gml' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<kml/>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<kml></kml>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point/>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<point><coordinates>1,2</coordinates></point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<POINT><coordinates>1,2</coordinates></POINT>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<kml:Point><coordinates>1,2</coordinates></kml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<Point><gml:coordinates>1,2</gml:coordinates></Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><coordinates>1,2</coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<Foo/><' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<Foo></Foo>x' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
    ]);
}

#[test]
fn gml_read_23() {
    check(&[
        (
            r"'<!-' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!x' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<![x' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<?xml ' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (Premature end of file.)"),
        ),
        (
            r"'<?xml' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x [' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x []' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (Premature end of file.)"),
        ),
        (
            r#"'"<!DOCTYPE x SYSTEM \"a"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></' 4 F GEO.GML ->GEOJSON",
            Err(
                r#"GEO.GML expects valid a GML STRING. (The element type "gml:Point" must be terminated by the matching end-tag "</gml:Point>".)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!' 4 F GEO.GML ->GEOJSON",
            Err(r#"GEO.GML expects valid a GML STRING. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-x' 4 F GEO.GML ->GEOJSON",
            Err(r#"GEO.GML expects valid a GML STRING. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><![CDATA[x]]>' 4 F GEO.GML ->GEOJSON",
            Err(r#"GEO.GML expects valid a GML STRING. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<Point><!-' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><!x' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![x' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![CDATA[x' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></' 4 F GEO.GML ->GEOJSON",
            Err(
                r#"GEO.GML expects valid a GML STRING. (The element type "Point" must be terminated by the matching end-tag "</Point>".)"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>x' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<MultiGeometry><MultiGeometry></MultiGeometry></MultiGeometry>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<!-x' 4 F GEO.GML ->GEOJSON",
            Err(r#"GEO.GML expects valid a GML STRING. (Comment must start with "<!--".)"#),
        ),
        (
            r"'< x/>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi?>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point id><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r#"GEO.GML expects valid a GML STRING. (Attribute name "id" associated with an element type "gml:Point" must be followed by the ' = ' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Err(
                r#"GEO.GML expects valid a GML STRING. (The value of attribute "id" associated with an element type "gml:Point" must not contain the '<' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point x>' 4 F GEO.GML ->GEOJSON",
            Err(
                r#"GEO.GML expects valid a GML STRING. (The end-tag for element type "gml:Point" must end with a '>' delimiter.)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 < 3</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<!-- c' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<?pi' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-- c' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!DOCTYPE gml><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<!DOCTYPE x [ ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<!DOCTYPE x [ <!ENTITY e \"v\"> ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#12,2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 & 2</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r"'<gml:LineString><gml:box><gml:coordinates>1,2 3,4</gml:coordinates></gml:box></gml:LineString>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Envelope cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:Point><gml:box><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:box></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:coordinates>1,2</gml:coordinates></gml:coord></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:pointMember></gml:MultiPoint>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.LinearRing)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:pointMember></gml:MultiPoint>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.Polygon)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:lineStringMember></gml:MultiLineString>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:lineStringMember></gml:MultiLineString>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.Polygon)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:polygonMember></gml:MultiPolygon>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:polygonMember></gml:MultiPolygon>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.LineString)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord/></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:box/>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint></gml:pointMember></gml:MultiPoint>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.MultiPoint)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:MultiLineString><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:MultiLineString></gml:lineStringMember></gml:MultiLineString>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.MultiLineString)",
            ),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:MultiPolygon><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:MultiPolygon></gml:polygonMember></gml:MultiPolygon>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.MultiPolygon)"),
        ),
        (
            r"'<gml:Point><!-- c' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><?pi' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"&foo;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Err(
                r#"GEO.GML expects valid a GML STRING. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"1 & 2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Err(
                r"GEO.GML expects valid a GML STRING. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
    ]);
}

#[test]
fn gml_read_24() {
    check(&[
        (
            r#"'"<gml:Point id=\"&#12;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Err(r#"GEO.GML expects valid a GML STRING. (Character reference "&#)"#),
        ),
        (
            r#"'"<gml:Point id=\"&#12,2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Err(
                r"GEO.GML expects valid a GML STRING. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,x</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r#"GEO.GML expects valid a GML STRING. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3 4,5,y</gml:coordinates></gml:Point>' 4 F GEO.GML ->GEOJSON",
            Err(r#"GEO.GML expects valid a GML STRING. (For input string: "y")"#),
        ),
        (
            r"'<gml:Polygon><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:Point><gml:coordinates>0,0</gml:coordinates></gml:Point></gml:Polygon>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2,3</coordinates></Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2,3,4</coordinates></Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (dimension must be <= 3)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><coord><X>1</X></coord></Point>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<LineString><coordinates>1,2,3 4,5,6</coordinates></LineString>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<LineString><coordinates>1,2</coordinates></LineString>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<LinearRing><coordinates>0,0 1,0 1,1 0,0</coordinates></LinearRing>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2,2 2,4 4,4 4,2 2,2</coordinates></LinearRing></innerBoundaryIs></Polygon>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><Point><coordinates>1,2</coordinates></Point><LineString><coordinates>1,2 3,4</coordinates></LineString></MultiGeometry>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<MultiGeometry></MultiGeometry>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><geometryMember><Point><coordinates>1,2</coordinates></Point></geometryMember></MultiGeometry>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></MultiGeometry>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<kml><Point><coordinates>1,2</coordinates></Point></kml>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<kml/>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<Point xmlns=\"http://www.opengis.net/kml/2.2\"><coordinates>1,2</coordinates></Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><Point/></Point>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point><Point/>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<Foo><coordinates>1,2</coordinates></Foo>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>junk' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (Content is not allowed in trailing section.)",
            ),
        ),
        (
            r#"'"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Point><coordinates>1,2</coordinates></Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<Point><coordinates>1,2,3</coordinates></Point>\n"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<Point><coordinates> 1,2,3 </coordinates></Point>' 4 F GEO.GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<Point><coordinates>1,2,3\n</coordinates></Point>"' JSON-> 4 F GEO.GML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><coordinates>1,2</coordinates></MultiGeometry>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<MultiPoint><pointMember><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></pointMember></MultiPoint>' 4 F GEO.GML ->GEOJSON",
            Err(
                r"GEO.GML expects valid a GML STRING. (com.vividsolutions.jts.geom.GeometryCollection)",
            ),
        ),
        (
            r"'<Polygon/>' 4 F GEO.GML ->GEOJSON",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3,4</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 3,4</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates>3,4</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates></gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>   </gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>x,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 </gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>,</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Z>3</gml:Z><gml:Y>2</gml:Y></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:Z>3</gml:Z><gml:Y>2</gml:Y><gml:X>1</gml:X></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:Z>3</gml:Z><gml:Z>4</gml:Z></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:X> 2 </gml:X></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:foo>3</gml:foo></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X/></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>  </gml:X></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 2</gml:X></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>0x10</gml:X><gml:Y>1.5f</gml:Y></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1e2</gml:X><gml:Y>-2.5d</gml:Y></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_read_25() {
    check(&[
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point/>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:foo/></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates/></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,5</gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2,3 4,5,6</gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1 2 3</gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2</gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coordinates>5,6</gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\";\">1,2;3,4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\" \" cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2x3,4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\"\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,</gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 </gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2 3,4\n5,6</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\n3,4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 5,6 7,8 9,10</gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 x</gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 1,2 3,4 1,2 3,4</gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord></gml:LinearRing>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LinearRing>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing></gml:LinearRing>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:LineString><gml:coordinates>0,0 1,1</gml:coordinates></gml:LineString></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:innerBoundaryIs></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>3,4</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint></gml:MultiPoint>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:pointMember></gml:MultiPoint>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_read_26() {
    check(&[
        (
            r"'<gml:MultiPoint><gml:pointMember></gml:pointMember></gml:MultiPoint>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing></gml:lineStringMember><gml:lineStringMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString></gml:MultiLineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:polygonMember></gml:MultiPolygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon></gml:MultiPolygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:geometryMember><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry></gml:MultiGeometry>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry></gml:geometryMember></gml:MultiGeometry>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:geometryMember></gml:MultiGeometry>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:coordinates>1,2</gml:coordinates>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:box><gml:coordinates>1,2 3,4</gml:coordinates></gml:box>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:coord><gml:X>1</gml:X></gml:coord>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:X>1</gml:X>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:point><gml:coordinates>1,2</gml:coordinates></gml:point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:POINT><gml:coordinates>1,2</gml:coordinates></gml:POINT>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Foo><gml:coordinates>1,2</gml:coordinates></gml:Foo>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:Foo/></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><foo>1,2</foo></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><x' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>junk' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<?xml version=\"1.0\"?><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<!-- c --><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><!-- c --><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates><![CDATA[1,2]]></gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>&#49;,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&amp;</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&foo;</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(
                r#"GEO.KML expects valid a KML STRING. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a\" id=\"b\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(
                r#"GEO.KML expects valid a KML STRING. (Attribute "id" was already specified for element "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point id=a><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(
                r#"GEO.KML expects valid a KML STRING. (Open quote is expected for attribute "id" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point a=\'1\'><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(
                r#"GEO.KML expects valid a KML STRING. (Open quote is expected for attribute "a" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r#"'"<gml:Point xmlns:gml=\"http://www.opengis.net/gml\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:Point >\t<gml:coordinates>1,2</gml:coordinates>\n</gml:Point >"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:Point>\n  <gml:coordinates>\n    1,2\n  </gml:coordinates>\n</gml:Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>\n1,2\n3,4\n</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\t3,4</gml:coordinates></gml:LineString>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:Point><gml:coord><gml:X>\n1\n</gml:X><gml:Y>\t2\t</gml:Y></gml:coord></gml:Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 </gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>NaN</gml:X><gml:Y>1</gml:Y></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>Infinity</gml:Y></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 NaN,3</gml:coordinates></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:<' 4 F GEO.KML ->GEOJSON",
            Err(
                r#"GEO.KML expects valid a KML STRING. (Element type "gml:" must be followed by either attribute specifications, ">" or "/>".)"#,
            ),
        ),
        (
            r"'<gml' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<kml/>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<kml></kml>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point/>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<point><coordinates>1,2</coordinates></point>' 4 F GEO.KML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<POINT><coordinates>1,2</coordinates></POINT>' 4 F GEO.KML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<kml:Point><coordinates>1,2</coordinates></kml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_read_27() {
    check(&[
        (
            r"'<Point><gml:coordinates>1,2</gml:coordinates></Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><coordinates>1,2</coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Foo/><' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Foo></Foo>x' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!-' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!x' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<![x' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<?xml ' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (Premature end of file.)"),
        ),
        (
            r"'<?xml' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x [' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x []' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (Premature end of file.)"),
        ),
        (
            r#"'"<!DOCTYPE x SYSTEM \"a"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-x' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><![CDATA[x]]>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><!-' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><!x' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![x' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![CDATA[x' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></' 4 F GEO.KML ->GEOJSON",
            Err(
                r#"GEO.KML expects valid a KML STRING. (The element type "Point" must be terminated by the matching end-tag "</Point>".)"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>x' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<MultiGeometry><MultiGeometry></MultiGeometry></MultiGeometry>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<!-x' 4 F GEO.KML ->GEOJSON",
            Err(r#"GEO.KML expects valid a KML STRING. (Comment must start with "<!--".)"#),
        ),
        (
            r"'< x/>' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi?>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point id><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(
                r#"GEO.KML expects valid a KML STRING. (Attribute name "id" associated with an element type "gml:Point" must be followed by the ' = ' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(
                r#"GEO.KML expects valid a KML STRING. (The value of attribute "id" associated with an element type "gml:Point" must not contain the '<' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point x>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 < 3</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<!-- c' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<?pi' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-- c' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE gml><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE x [ ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<!DOCTYPE x [ <!ENTITY e \"v\"> ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#12,2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 & 2</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r"'<gml:LineString><gml:box><gml:coordinates>1,2 3,4</gml:coordinates></gml:box></gml:LineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:box><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:box></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:coordinates>1,2</gml:coordinates></gml:coord></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:pointMember></gml:MultiPoint>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:pointMember></gml:MultiPoint>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:lineStringMember></gml:MultiLineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:lineStringMember></gml:MultiLineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:polygonMember></gml:MultiPolygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:polygonMember></gml:MultiPolygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:outerBoundaryIs></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord/></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:box/>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:X>1</gml:X></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint></gml:pointMember></gml:MultiPoint>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:MultiLineString><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:MultiLineString></gml:lineStringMember></gml:MultiLineString>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_read_28() {
    check(&[
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:MultiPolygon><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:MultiPolygon></gml:polygonMember></gml:MultiPolygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><!-- c' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><?pi' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:Point id=\"&foo;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(
                r#"GEO.KML expects valid a KML STRING. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"1 & 2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(
                r"GEO.KML expects valid a KML STRING. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"&#12;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(r#"GEO.KML expects valid a KML STRING. (Character reference "&#)"#),
        ),
        (
            r#"'"<gml:Point id=\"&#12,2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Err(
                r"GEO.KML expects valid a KML STRING. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,x</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3 4,5,y</gml:coordinates></gml:Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:Point><gml:coordinates>0,0</gml:coordinates></gml:Point></gml:Polygon>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2,3</coordinates></Point>' 4 F GEO.KML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>' 4 F GEO.KML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2,3,4</coordinates></Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (dimension must be <= 3)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><coord><X>1</X></coord></Point>' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<LineString><coordinates>1,2,3 4,5,6</coordinates></LineString>' 4 F GEO.KML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<LineString><coordinates>1,2</coordinates></LineString>' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<LinearRing><coordinates>0,0 1,0 1,1 0,0</coordinates></LinearRing>' 4 F GEO.KML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon>' 4 F GEO.KML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2,2 2,4 4,4 4,2 2,2</coordinates></LinearRing></innerBoundaryIs></Polygon>' 4 F GEO.KML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><Point><coordinates>1,2</coordinates></Point><LineString><coordinates>1,2 3,4</coordinates></LineString></MultiGeometry>' 4 F GEO.KML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<MultiGeometry></MultiGeometry>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>' 4 F GEO.KML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[-22.5,-11.25],[22.5,-11.25],[22.5,11.25],[-22.5,11.25],[-22.5,-11.25]]]]}"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><geometryMember><Point><coordinates>1,2</coordinates></Point></geometryMember></MultiGeometry>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></MultiGeometry>' 4 F GEO.KML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<kml><Point><coordinates>1,2</coordinates></Point></kml>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<kml/>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<Point xmlns=\"http://www.opengis.net/kml/2.2\"><coordinates>1,2</coordinates></Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><Point/></Point>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point><Point/>' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<Foo><coordinates>1,2</coordinates></Foo>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>junk' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (Content is not allowed in trailing section.)",
            ),
        ),
        (
            r#"'"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Point><coordinates>1,2</coordinates></Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<Point><coordinates>1,2,3</coordinates></Point>\n"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<Point><coordinates> 1,2,3 </coordinates></Point>' 4 F GEO.KML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r#"'"<Point><coordinates>1,2,3\n</coordinates></Point>"' JSON-> 4 F GEO.KML ->GEOJSON"#,
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[22.5,0.0],[22.5,11.25],[0.0,11.25],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><coordinates>1,2</coordinates></MultiGeometry>' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<MultiPoint><pointMember><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></pointMember></MultiPoint>' 4 F GEO.KML ->GEOJSON",
            Err(
                r"GEO.KML expects valid a KML STRING. (com.vividsolutions.jts.geom.GeometryCollection)",
            ),
        ),
        (
            r"'<Polygon/>' 4 F GEO.KML ->GEOJSON",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3,4</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (dimension must be <= 3)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 3,4</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (AssertionFailedException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates>3,4</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates></gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>   </gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a coordinate sequence without a non-null touple to parse)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>x,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 </gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>,</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Z>3</gml:Z><gml:Y>2</gml:Y></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:Z>3</gml:Z><gml:Y>2</gml:Y><gml:X>1</gml:X></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:Z>3</gml:Z><gml:Z>4</gml:Z></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a coordinate with more than 3 axis)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:X> 2 </gml:X></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
    ]);
}

#[test]
fn gml_read_29() {
    check(&[
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:foo>3</gml:foo></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X/></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>  </gml:X></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (empty String)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 2</gml:X></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (For input string: "1 2")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>0x10</gml:X><gml:Y>1.5f</gml:Y></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (For input string: "0x10")"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1e2</gml:X><gml:Y>-2.5d</gml:Y></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point/>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:foo/></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates/></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,5</gml:coordinates></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2,3 4,5,6</gml:coordinates></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1 2 3</gml:coordinates></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2</gml:coordinates></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coordinates>5,6</gml:coordinates></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:LineString></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\";\">1,2;3,4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\" \" cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2x3,4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (For input string: "2 3")"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\"\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (For input string: ",")"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,</gml:coordinates></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 </gml:coordinates></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2 3,4\n5,6</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\n3,4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 5,6 7,8 9,10</gml:coordinates></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 x</gml:coordinates></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (For input string: "x")"#),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 1,2 3,4 1,2 3,4</gml:coordinates></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord></gml:LinearRing>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:LinearRing><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LinearRing>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:LinearRing></gml:LinearRing>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a linear ring without atleast four coordinates or one coordinate sequence)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[5.625,0.0],[5.625,8.4375],[0.0,8.4375],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,5.625],[5.625,5.625],[5.625,8.4375],[0.0,8.4375],[0.0,5.625]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[5.625,0.0],[5.625,8.4375],[0.0,8.4375],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,5.625],[5.625,5.625],[5.625,8.4375],[0.0,8.4375],[0.0,5.625]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,5.625],[5.625,5.625],[5.625,8.4375],[0.0,8.4375],[0.0,5.625]]]]}"]"#,
            ),
        ),
    ]);
}

#[test]
fn gml_read_30() {
    check(&[
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Points of LinearRing do not form a closed linestring)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>3,4</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:MultiPoint></gml:MultiPoint>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:pointMember></gml:MultiPoint>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.LineString)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember></gml:pointMember></gml:MultiPoint>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Geometry Members may only contain one geometry.)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing></gml:lineStringMember><gml:lineStringMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:MultiLineString></gml:MultiLineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,5.625],[5.625,5.625],[5.625,8.4375],[0.0,8.4375],[0.0,5.625]]]]}"]"#,
            ),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:polygonMember></gml:MultiPolygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:MultiPolygon></gml:MultiPolygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:geometryMember><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry></gml:MultiGeometry>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry></gml:geometryMember></gml:MultiGeometry>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:geometryMember></gml:MultiGeometry>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:point><gml:coordinates>1,2</gml:coordinates></gml:point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:POINT><gml:coordinates>1,2</gml:coordinates></gml:POINT>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Foo><gml:coordinates>1,2</gml:coordinates></gml:Foo>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:Foo/></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><foo>1,2</foo></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><x' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>junk' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Content is not allowed in trailing section.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r#"'"<?xml version=\"1.0\"?><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<!-- c --><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><!-- c --><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates><![CDATA[1,2]]></gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>&#49;,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&amp;</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (For input string: "2&")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&foo;</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.GML.UNIFORM expects valid a GML STRING. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a\" id=\"b\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Err(
                r#"GEO.GML.UNIFORM expects valid a GML STRING. (Attribute "id" was already specified for element "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point id=a><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.GML.UNIFORM expects valid a GML STRING. (Open quote is expected for attribute "id" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point a=\'1\'><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.GML.UNIFORM expects valid a GML STRING. (Open quote is expected for attribute "a" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r#"'"<gml:Point xmlns:gml=\"http://www.opengis.net/gml\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<gml:Point >\t<gml:coordinates>1,2</gml:coordinates>\n</gml:Point >"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<gml:Point>\n  <gml:coordinates>\n    1,2\n  </gml:coordinates>\n</gml:Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>\n1,2\n3,4\n</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\t3,4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<gml:Point><gml:coord><gml:X>\n1\n</gml:X><gml:Y>\t2\t</gml:Y></gml:coord></gml:Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 </gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:<' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.GML.UNIFORM expects valid a GML STRING. (Element type "gml:" must be followed by either attribute specifications, ">" or "/>".)"#,
            ),
        ),
        (
            r"'<gml' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
    ]);
}

#[test]
fn gml_read_31() {
    check(&[
        (
            r"'<kml/>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<kml></kml>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point/>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<point><coordinates>1,2</coordinates></point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<POINT><coordinates>1,2</coordinates></POINT>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<kml:Point><coordinates>1,2</coordinates></kml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<Point><gml:coordinates>1,2</gml:coordinates></Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><coordinates>1,2</coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<Foo/><' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<Foo></Foo>x' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!-' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!x' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<![x' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<?xml ' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (Premature end of file.)"),
        ),
        (
            r"'<?xml' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x [' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x []' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (Premature end of file.)"),
        ),
        (
            r#"'"<!DOCTYPE x SYSTEM \"a"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.GML.UNIFORM expects valid a GML STRING. (The element type "gml:Point" must be terminated by the matching end-tag "</gml:Point>".)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-x' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><![CDATA[x]]>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (Comment must start with "<!--".)"#),
        ),
        (
            r"'<Point><!-' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><!x' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![x' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![CDATA[x' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.GML.UNIFORM expects valid a GML STRING. (The element type "Point" must be terminated by the matching end-tag "</Point>".)"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>x' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<MultiGeometry><MultiGeometry></MultiGeometry></MultiGeometry>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<!-x' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (Comment must start with "<!--".)"#),
        ),
        (
            r"'< x/>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi?>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point id><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.GML.UNIFORM expects valid a GML STRING. (Attribute name "id" associated with an element type "gml:Point" must be followed by the ' = ' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Err(
                r#"GEO.GML.UNIFORM expects valid a GML STRING. (The value of attribute "id" associated with an element type "gml:Point" must not contain the '<' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point x>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.GML.UNIFORM expects valid a GML STRING. (The end-tag for element type "gml:Point" must end with a '>' delimiter.)"#,
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 < 3</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<!-- c' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<?pi' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-- c' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!DOCTYPE gml><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<!DOCTYPE x [ ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<!DOCTYPE x [ <!ENTITY e \"v\"> ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#12,2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 & 2</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r"'<gml:LineString><gml:box><gml:coordinates>1,2 3,4</gml:coordinates></gml:box></gml:LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a linestring without atleast two coordinates or one coordinate sequence (com.vividsolutions.jts.geom.Envelope cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence))",
            ),
        ),
        (
            r"'<gml:Point><gml:box><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:box></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.Coordinate cannot be cast to com.vividsolutions.jts.geom.CoordinateSequence)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:coordinates>1,2</gml:coordinates></gml:coord></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:pointMember></gml:MultiPoint>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:pointMember></gml:MultiPoint>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.Polygon)",
            ),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:lineStringMember></gml:MultiLineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:lineStringMember></gml:MultiLineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.Polygon)",
            ),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:polygonMember></gml:MultiPolygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.Point)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:polygonMember></gml:MultiPolygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.LineString)",
            ),
        ),
    ]);
}

#[test]
fn gml_read_32() {
    check(&[
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Point><gml:coord/></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:box/>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint></gml:pointMember></gml:MultiPoint>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.MultiPoint)",
            ),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:MultiLineString><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:MultiLineString></gml:lineStringMember></gml:MultiLineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.MultiLineString)",
            ),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:MultiPolygon><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:MultiPolygon></gml:polygonMember></gml:MultiPolygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.MultiPolygon)",
            ),
        ),
        (
            r"'<gml:Point><!-- c' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><?pi' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"&foo;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Err(
                r#"GEO.GML.UNIFORM expects valid a GML STRING. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"1 & 2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"&#12;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (Character reference "&#)"#),
        ),
        (
            r#"'"<gml:Point id=\"&#12,2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,x</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (For input string: "x")"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3 4,5,y</gml:coordinates></gml:Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r#"GEO.GML.UNIFORM expects valid a GML STRING. (For input string: "y")"#),
        ),
        (
            r"'<gml:Polygon><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.LineString cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<gml:Polygon><gml:Point><gml:coordinates>0,0</gml:coordinates></gml:Point></gml:Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.Point cannot be cast to com.vividsolutions.jts.geom.LinearRing)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2,3</coordinates></Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<Point><coordinates>1,2,3,4</coordinates></Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (dimension must be <= 3)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><coord><X>1</X></coord></Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<LineString><coordinates>1,2,3 4,5,6</coordinates></LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<LineString><coordinates>1,2</coordinates></LineString>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<LinearRing><coordinates>0,0 1,0 1,1 0,0</coordinates></LinearRing>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[5.625,0.0],[5.625,8.4375],[0.0,8.4375],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2,2 2,4 4,4 4,2 2,2</coordinates></LinearRing></innerBoundaryIs></Polygon>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,5.625],[5.625,5.625],[5.625,8.4375],[0.0,8.4375],[0.0,5.625]]]]}"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><Point><coordinates>1,2</coordinates></Point><LineString><coordinates>1,2 3,4</coordinates></LineString></MultiGeometry>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<MultiGeometry></MultiGeometry>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"This method does not support GeometryCollection arguments"),
        ),
        (
            r"'<MultiGeometry><geometryMember><Point><coordinates>1,2</coordinates></Point></geometryMember></MultiGeometry>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></MultiGeometry>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<kml><Point><coordinates>1,2</coordinates></Point></kml>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<kml/>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<Point xmlns=\"http://www.opengis.net/kml/2.2\"><coordinates>1,2</coordinates></Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><Point/></Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point><Point/>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<Foo><coordinates>1,2</coordinates></Foo>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>junk' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (Content is not allowed in trailing section.)",
            ),
        ),
        (
            r#"'"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Point><coordinates>1,2</coordinates></Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3</coordinates></Point>\n"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<Point><coordinates> 1,2,3 </coordinates></Point>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3\n</coordinates></Point>"' JSON-> 6 T GEO.GML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<MultiGeometry><coordinates>1,2</coordinates></MultiGeometry>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<MultiPoint><pointMember><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></pointMember></MultiPoint>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.GML.UNIFORM expects valid a GML STRING. (com.vividsolutions.jts.geom.GeometryCollection)",
            ),
        ),
        (
            r"'<Polygon/>' 6 T GEO.GML.UNIFORM ->GEOJSON",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3,4</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 3,4</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates>3,4</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates></gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>   </gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>x,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 </gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>,</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_read_33() {
    check(&[
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Z>3</gml:Z><gml:Y>2</gml:Y></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:Z>3</gml:Z><gml:Y>2</gml:Y><gml:X>1</gml:X></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:Z>3</gml:Z><gml:Z>4</gml:Z></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:X> 2 </gml:X></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y><gml:foo>3</gml:foo></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X/></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>  </gml:X></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 2</gml:X></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>0x10</gml:X><gml:Y>1.5f</gml:Y></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1e2</gml:X><gml:Y>-2.5d</gml:Y></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point/>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coord><gml:X>1</gml:X></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:foo/></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates><gml:coordinates/></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,5</gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2,3 4,5,6</gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1 2 3</gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2</gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coordinates>5,6</gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coord><gml:X>3</gml:X><gml:Y>4</gml:Y></gml:coord></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord><gml:coordinates>1,2</gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\";\">1,2;3,4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\" \" cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\";\">1;2 3;4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2x3,4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates ts=\"x\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates cs=\"\">1,2 3,4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4,</gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 </gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2 3,4\n5,6</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\n3,4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 5,6 7,8 9,10</gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 x</gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LineString><gml:coordinates>1,2 3,4 1,2 3,4 1,2 3,4</gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord></gml:LinearRing>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coord><gml:X>1</gml:X><gml:Y>1</gml:Y></gml:coord><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:LinearRing>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing></gml:LinearRing>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord></gml:LinearRing>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:LinearRing><gml:coord><gml:X>0</gml:X><gml:Y>0</gml:Y></gml:coord><gml:coordinates>0,0 1,0</gml:coordinates></gml:LinearRing>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_read_34() {
    check(&[
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:LineString><gml:coordinates>0,0 1,1</gml:coordinates></gml:LineString></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 1,0 1,1</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:innerBoundaryIs></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>3,4</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint></gml:MultiPoint>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:pointMember></gml:MultiPoint>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember></gml:pointMember></gml:MultiPoint>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:LinearRing><gml:coordinates>0,0 1,0 1,1 0,0</gml:coordinates></gml:LinearRing></gml:lineStringMember><gml:lineStringMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString></gml:MultiLineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2,2 2,4 4,4 4,2 2,2</gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:polygonMember></gml:MultiPolygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon></gml:MultiPolygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:geometryMember><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry></gml:MultiGeometry>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry></gml:geometryMember></gml:MultiGeometry>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:geometryMember></gml:MultiGeometry>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:coordinates>1,2</gml:coordinates>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:box><gml:coordinates>1,2 3,4</gml:coordinates></gml:box>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:coord><gml:X>1</gml:X></gml:coord>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:X>1</gml:X>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:pointMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:pointMember>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:point><gml:coordinates>1,2</gml:coordinates></gml:point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:POINT><gml:coordinates>1,2</gml:coordinates></gml:POINT>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Foo><gml:coordinates>1,2</gml:coordinates></gml:Foo>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:Foo/></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><foo>1,2</foo></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><x' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>junk' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<?xml version=\"1.0\"?><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<!-- c --><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><!-- c --><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates><![CDATA[1,2]]></gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>&#49;,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&amp;</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2&foo;</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.KML.UNIFORM expects valid a KML STRING. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a\" id=\"b\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(
                r#"GEO.KML.UNIFORM expects valid a KML STRING. (Attribute "id" was already specified for element "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point id=a><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.KML.UNIFORM expects valid a KML STRING. (Open quote is expected for attribute "id" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r"'<gml:Point a=\'1\'><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.KML.UNIFORM expects valid a KML STRING. (Open quote is expected for attribute "a" associated with an  element type  "gml:Point".)"#,
            ),
        ),
        (
            r#"'"<gml:Point xmlns:gml=\"http://www.opengis.net/gml\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:Point >\t<gml:coordinates>1,2</gml:coordinates>\n</gml:Point >"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:Point>\n  <gml:coordinates>\n    1,2\n  </gml:coordinates>\n</gml:Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>\n1,2\n3,4\n</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:LineString><gml:coordinates>1,2\t3,4</gml:coordinates></gml:LineString>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:Point><gml:coord><gml:X>\n1\n</gml:X><gml:Y>\t2\t</gml:Y></gml:coord></gml:Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1 </gml:X><gml:Y>2</gml:Y></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>NaN</gml:X><gml:Y>1</gml:Y></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:Y>Infinity</gml:Y></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_read_35() {
    check(&[
        (
            r"'<gml:LineString><gml:coordinates>1,2 NaN,3</gml:coordinates></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:<' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.KML.UNIFORM expects valid a KML STRING. (Element type "gml:" must be followed by either attribute specifications, ">" or "/>".)"#,
            ),
        ),
        (
            r"'<gml' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<kml/>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<kml></kml>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point/>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<point><coordinates>1,2</coordinates></point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<POINT><coordinates>1,2</coordinates></POINT>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<kml:Point><coordinates>1,2</coordinates></kml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><gml:coordinates>1,2</gml:coordinates></Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><coordinates>1,2</coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Foo/><' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Foo></Foo>x' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!-' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<!x' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<![x' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<?xml ' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (Premature end of file.)"),
        ),
        (
            r"'<?xml' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x [' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (Premature end of file.)"),
        ),
        (
            r"'<!DOCTYPE x []' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (Premature end of file.)"),
        ),
        (
            r#"'"<!DOCTYPE x SYSTEM \"a"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-x' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><![CDATA[x]]>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><!-' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><!x' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![x' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<Point><![CDATA[x' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.KML.UNIFORM expects valid a KML STRING. (The element type "Point" must be terminated by the matching end-tag "</Point>".)"#,
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>x' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<MultiGeometry><MultiGeometry></MultiGeometry></MultiGeometry>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<!-x' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r#"GEO.KML.UNIFORM expects valid a KML STRING. (Comment must start with "<!--".)"#),
        ),
        (
            r"'< x/>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (The markup in the document preceding the root element must be well-formed.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi?>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point id><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r#"GEO.KML.UNIFORM expects valid a KML STRING. (Attribute name "id" associated with an element type "gml:Point" must be followed by the ' = ' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(
                r#"GEO.KML.UNIFORM expects valid a KML STRING. (The value of attribute "id" associated with an element type "gml:Point" must not contain the '<' character.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"a"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point x>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2 < 3</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (The content of elements must consist of well-formed character data or markup.)",
            ),
        ),
        (
            r"'<!-- c' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<?pi' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><!-- c' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE gml><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<!DOCTYPE x [ ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<!DOCTYPE x [ <!ENTITY e \"v\"> ]><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#12,2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1 & 2</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r"'<gml:LineString><gml:box><gml:coordinates>1,2 3,4</gml:coordinates></gml:box></gml:LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:box><gml:coord><gml:X>1</gml:X><gml:Y>2</gml:Y></gml:coord></gml:box></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord><gml:X>1</gml:X><gml:coordinates>1,2</gml:coordinates></gml:coord></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_read_36() {
    check(&[
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:pointMember></gml:MultiPoint>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:pointMember></gml:MultiPoint>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:lineStringMember></gml:MultiLineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:lineStringMember></gml:MultiLineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:polygonMember></gml:MultiPolygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:polygonMember></gml:MultiPolygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:outerBoundaryIs><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:outerBoundaryIs></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coord/></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:box/>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:X>1</gml:X></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPoint><gml:pointMember><gml:MultiPoint><gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point></gml:MultiPoint></gml:pointMember></gml:MultiPoint>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiLineString><gml:lineStringMember><gml:MultiLineString><gml:LineString><gml:coordinates>1,2 3,4</gml:coordinates></gml:LineString></gml:MultiLineString></gml:lineStringMember></gml:MultiLineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:MultiPolygon><gml:polygonMember><gml:MultiPolygon><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0,0 0,10 10,10 10,0 0,0</gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:MultiPolygon></gml:polygonMember></gml:MultiPolygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><!-- c' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><?pi' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><?pi' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<gml:Point id=\"&foo;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(
                r#"GEO.KML.UNIFORM expects valid a KML STRING. (The entity "foo" was referenced, but not declared.)"#,
            ),
        ),
        (
            r#"'"<gml:Point id=\"1 & 2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (The entity name must immediately follow the '&' in the entity reference.)",
            ),
        ),
        (
            r#"'"<gml:Point id=\"&#12;\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(r#"GEO.KML.UNIFORM expects valid a KML STRING. (Character reference "&#)"#),
        ),
        (
            r#"'"<gml:Point id=\"&#12,2\"><gml:coordinates>1,2</gml:coordinates></gml:Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (The character reference must end with the ';' delimiter.)",
            ),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,x</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2,3 4,5,y</gml:coordinates></gml:Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:LineString><gml:coordinates>0,0 1,1 2,2 0,0</gml:coordinates></gml:LineString></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Polygon><gml:Point><gml:coordinates>0,0</gml:coordinates></gml:Point></gml:Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2,3</coordinates></Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<Point><coordinates>1,2,3,4</coordinates></Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (dimension must be <= 3)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><coord><X>1</X></coord></Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (Cannot create a point without exactly one coordinate)",
            ),
        ),
        (
            r"'<LineString><coordinates>1,2,3 4,5,6</coordinates></LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<LineString><coordinates>1,2</coordinates></LineString>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (Invalid number of points in LineString (found 1 - must be 0 or >= 2))",
            ),
        ),
        (
            r"'<LinearRing><coordinates>0,0 1,0 1,1 0,0</coordinates></LinearRing>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[5.625,0.0],[5.625,8.4375],[0.0,8.4375],[0.0,0.0]]]]}"]"#,
            ),
        ),
        (
            r"'<Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2,2 2,4 4,4 4,2 2,2</coordinates></LinearRing></innerBoundaryIs></Polygon>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,5.625],[5.625,5.625],[5.625,8.4375],[0.0,8.4375],[0.0,5.625]]]]}"]"#,
            ),
        ),
        (
            r"'<MultiGeometry><Point><coordinates>1,2</coordinates></Point><LineString><coordinates>1,2 3,4</coordinates></LineString></MultiGeometry>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<MultiGeometry></MultiGeometry>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 0,10 10,10 10,0 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"This method does not support GeometryCollection arguments"),
        ),
        (
            r"'<MultiGeometry><geometryMember><Point><coordinates>1,2</coordinates></Point></geometryMember></MultiGeometry>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<MultiGeometry><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></MultiGeometry>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<kml><Point><coordinates>1,2</coordinates></Point></kml>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<kml/>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r#"'"<Point xmlns=\"http://www.opengis.net/kml/2.2\"><coordinates>1,2</coordinates></Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates><Point/></Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point><Point/>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (The markup in the document following the root element must be well-formed.)",
            ),
        ),
        (
            r"'<Foo><coordinates>1,2</coordinates></Foo>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<Point><coordinates>1,2</coordinates></Point>junk' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (Content is not allowed in trailing section.)",
            ),
        ),
        (
            r#"'"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Point><coordinates>1,2</coordinates></Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3</coordinates></Point>\n"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<Point><coordinates> 1,2,3 </coordinates></Point>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r#"'"<Point><coordinates>1,2,3\n</coordinates></Point>"' JSON-> 6 T GEO.KML.UNIFORM ->GEOJSON"#,
            Ok(r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[]}"]"#),
        ),
        (
            r"'<MultiGeometry><coordinates>1,2</coordinates></MultiGeometry>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (com.vividsolutions.jts.geom.impl.CoordinateArraySequence)",
            ),
        ),
        (
            r"'<MultiPoint><pointMember><MultiGeometry><Point><coordinates>1,2</coordinates></Point></MultiGeometry></pointMember></MultiPoint>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(
                r"GEO.KML.UNIFORM expects valid a KML STRING. (com.vividsolutions.jts.geom.GeometryCollection)",
            ),
        ),
        (
            r"'<Polygon/>' 6 T GEO.KML.UNIFORM ->GEOJSON",
            Err(r"GEO.KML.UNIFORM expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"'x' 4 F GEO.GML",
            Err(r"GEO.GML expects valid a GML STRING. (Content is not allowed in prolog.)"),
        ),
        (
            r"'<gml:Point/>' 'a' F GEO.GML",
            Err(
                r"GEO.GML expects a GML STRING, a DOUBLE error percentage or a LONG resolution (even number between 2 and 30) and a BOOLEAN.",
            ),
        ),
        (
            r"'<gml:Point/>' 4 1 GEO.GML",
            Err(
                r"GEO.GML expects a GML STRING, a DOUBLE error percentage or a LONG resolution (even number between 2 and 30) and a BOOLEAN.",
            ),
        ),
        (
            r"'<gml:Point/>' 1 F GEO.GML",
            Err(r"GEO.GML expects the resolution to be an even number between 2 and 30"),
        ),
        (
            r"'<gml:Point/>' 3 F GEO.GML",
            Err(r"GEO.GML expects the resolution to be an even number between 2 and 30"),
        ),
        (
            r"'<gml:Point/>' 4 F GEO.KML",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_read_37() {
    check(&[
        (
            r"'<Point/>' 4 F GEO.KML",
            Err(r"GEO.KML expects valid a KML STRING. (NullPointerException)"),
        ),
        (
            r"1 4 F GEO.GML",
            Err(
                r"GEO.GML expects a GML STRING, a DOUBLE error percentage or a LONG resolution (even number between 2 and 30) and a BOOLEAN.",
            ),
        ),
        (r"'<gml:Point/>' 4 F GEO.WKT", Err(r"NullPointerException")),
        (
            r"'<gml:Point/>' 4 F GEO.GML SIZE",
            Err(r"GEO.GML expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point/>' 4 F GEO.GML.UNIFORM",
            Err(r"GEO.GML.UNIFORM expects valid a GML STRING. (NullPointerException)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#0;,2</gml:coordinates></gml:Point>' ->WKT",
            Err(r#"->WKT was given invalid input. (Character reference "&#)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#1;,2</gml:coordinates></gml:Point>' ->WKT",
            Err(r#"->WKT was given invalid input. (Character reference "&#)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#xD800;,2</gml:coordinates></gml:Point>' ->WKT",
            Err(r#"->WKT was given invalid input. (Character reference "&#)"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1&#x2e;5,2</gml:coordinates></gml:Point>' ->WKT",
            Ok(r#"["POINT (1.5 2)"]"#),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>x' ->WKT",
            Err(r"->WKT was given invalid input. (Content is not allowed in trailing section.)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point>xy' ->WKT",
            Err(r"->WKT was given invalid input. (Content is not allowed in trailing section.)"),
        ),
        (
            r"'<gml:Point><gml:coordinates>1,2</gml:coordinates></gml:Point><' ->WKT",
            Err(
                r"->WKT was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
    ]);
}

#[test]
fn gml_write_0() {
    check(&[
        (
            r"'POINT (1 2)' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'POINT (1 2)' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0 </coordinates></Point>"]"#),
        ),
        (r"'POINT (1 2)' ->GML ->WKT", Ok(r#"["POINT (1 2)"]"#)),
        (
            r"'POINT (1 2)' ->KML ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'POINT (1 2)' ->GML ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0]}"]"#),
        ),
        (
            r"'POINT (1 2 3)' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0,3.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'POINT (1 2 3)' ->KML",
            Ok(r#"["<Point><coordinates>1.0,2.0,3.0 </coordinates></Point>"]"#),
        ),
        (r"'POINT (1 2 3)' ->GML ->WKT", Ok(r#"["POINT (1 2)"]"#)),
        (
            r"'POINT (1 2 3)' ->KML ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0,2.0,3.0 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'POINT (1 2 3)' ->GML ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0,2.0,3.0]}"]"#),
        ),
        (
            r"'POINT EMPTY' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'POINT EMPTY' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'POINT EMPTY' ->GML ->WKT",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'POINT EMPTY' ->KML ->GML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'POINT EMPTY' ->GML ->GEOJSON",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'POINT (1e20 1e-5)' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0E20,1.0E-5 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'POINT (1e20 1e-5)' ->KML",
            Ok(r#"["<Point><coordinates>1.0E20,1.0E-5 </coordinates></Point>"]"#),
        ),
        (
            r"'POINT (1e20 1e-5)' ->GML ->WKT",
            Ok(r#"["POINT (100000000000000000000 0.00001)"]"#),
        ),
        (
            r"'POINT (1e20 1e-5)' ->KML ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0E20,1.0E-5 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'POINT (1e20 1e-5)' ->GML ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0E20,1.0E-5]}"]"#),
        ),
        (
            r"'POINT (-0.0 0.30000000000000004)' ->GML",
            Ok(
                r#"["<gml:Point><gml:coordinates>-0.0,0.30000000000000004 </gml:coordinates></gml:Point>"]"#,
            ),
        ),
        (
            r"'POINT (-0.0 0.30000000000000004)' ->KML",
            Ok(r#"["<Point><coordinates>-0.0,0.30000000000000004 </coordinates></Point>"]"#),
        ),
        (
            r"'POINT (-0.0 0.30000000000000004)' ->GML ->WKT",
            Ok(r#"["POINT (-0 0.3)"]"#),
        ),
        (
            r"'POINT (-0.0 0.30000000000000004)' ->KML ->GML",
            Ok(
                r#"["<gml:Point><gml:coordinates>-0.0,0.30000000000000004 </gml:coordinates></gml:Point>"]"#,
            ),
        ),
        (
            r"'POINT (-0.0 0.30000000000000004)' ->GML ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[-0.0,0.30000000000000004]}"]"#),
        ),
        (
            r"'POINT (1e300 0.1)' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0E300,0.1 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'POINT (1e300 0.1)' ->KML",
            Ok(r#"["<Point><coordinates>1.0E300,0.1 </coordinates></Point>"]"#),
        ),
        (
            r"'POINT (1e300 0.1)' ->GML ->WKT",
            Ok(
                r#"["POINT (1000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 0.1)"]"#,
            ),
        ),
        (
            r"'POINT (1e300 0.1)' ->KML ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>1.0E300,0.1 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'POINT (1e300 0.1)' ->GML ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[1.0E300,0.1]}"]"#),
        ),
        (
            r"'POINT (5e-324 123456789.123)' ->GML",
            Ok(
                r#"["<gml:Point><gml:coordinates>4.9E-324,1.23456789123E8 </gml:coordinates></gml:Point>"]"#,
            ),
        ),
        (
            r"'POINT (5e-324 123456789.123)' ->KML",
            Ok(r#"["<Point><coordinates>4.9E-324,1.23456789123E8 </coordinates></Point>"]"#),
        ),
        (
            r"'POINT (5e-324 123456789.123)' ->GML ->WKT",
            Ok(r#"["POINT (0 123456789.123)"]"#),
        ),
        (
            r"'POINT (5e-324 123456789.123)' ->KML ->GML",
            Ok(
                r#"["<gml:Point><gml:coordinates>4.9E-324,1.23456789123E8 </gml:coordinates></gml:Point>"]"#,
            ),
        ),
        (
            r"'POINT (5e-324 123456789.123)' ->GML ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[4.9E-324,1.23456789123E8]}"]"#),
        ),
        (
            r"'POINT (0.1 0.2)' ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>0.1,0.2 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'POINT (0.1 0.2)' ->KML",
            Ok(r#"["<Point><coordinates>0.1,0.2 </coordinates></Point>"]"#),
        ),
        (
            r"'POINT (0.1 0.2)' ->GML ->WKT",
            Ok(r#"["POINT (0.1 0.2)"]"#),
        ),
        (
            r"'POINT (0.1 0.2)' ->KML ->GML",
            Ok(r#"["<gml:Point><gml:coordinates>0.1,0.2 </gml:coordinates></gml:Point>"]"#),
        ),
        (
            r"'POINT (0.1 0.2)' ->GML ->GEOJSON",
            Ok(r#"["{\"type\":\"Point\",\"coordinates\":[0.1,0.2]}"]"#),
        ),
        (
            r"'POINT (1 2 3 4)' ->GML",
            Err(r"->GML was given invalid input. (Expected ) but found '4' (line 1))"),
        ),
        (
            r"'POINT (1 2 3 4)' ->KML",
            Err(r"->KML was given invalid input. (Expected ) but found '4' (line 1))"),
        ),
        (
            r"'POINT (1 2 3 4)' ->GML ->WKT",
            Err(r"->GML was given invalid input. (Expected ) but found '4' (line 1))"),
        ),
        (
            r"'POINT (1 2 3 4)' ->KML ->GML",
            Err(r"->KML was given invalid input. (Expected ) but found '4' (line 1))"),
        ),
        (
            r"'POINT (1 2 3 4)' ->GML ->GEOJSON",
            Err(r"->GML was given invalid input. (Expected ) but found '4' (line 1))"),
        ),
        (
            r"'LINESTRING (1 2, 3 4)' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'LINESTRING (1 2, 3 4)' ->KML",
            Ok(r#"["<LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString>"]"#),
        ),
        (
            r"'LINESTRING (1 2, 3 4)' ->GML ->WKT",
            Ok(r#"["LINESTRING (1 2, 3 4)"]"#),
        ),
        (
            r"'LINESTRING (1 2, 3 4)' ->KML ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'LINESTRING (1 2, 3 4)' ->GML ->GEOJSON",
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r"'LINESTRING (1 2 3, 4 5 6)' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0,3.0 4.0,5.0,6.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'LINESTRING (1 2 3, 4 5 6)' ->KML",
            Ok(
                r#"["<LineString><coordinates>1.0,2.0,3.0 4.0,5.0,6.0 </coordinates></LineString>"]"#,
            ),
        ),
        (
            r"'LINESTRING (1 2 3, 4 5 6)' ->GML ->WKT",
            Ok(r#"["LINESTRING (1 2, 4 5)"]"#),
        ),
        (
            r"'LINESTRING (1 2 3, 4 5 6)' ->KML ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>1.0,2.0,3.0 4.0,5.0,6.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'LINESTRING (1 2 3, 4 5 6)' ->GML ->GEOJSON",
            Ok(r#"["{\"type\":\"LineString\",\"coordinates\":[[1.0,2.0,3.0],[4.0,5.0,6.0]]}"]"#),
        ),
        (
            r"'LINESTRING EMPTY' ->GML",
            Ok(r#"["<gml:LineString><gml:coordinates></gml:coordinates></gml:LineString>"]"#),
        ),
        (
            r"'LINESTRING EMPTY' ->KML",
            Ok(r#"["<LineString><coordinates></coordinates></LineString>"]"#),
        ),
        (
            r"'LINESTRING EMPTY' ->GML ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'LINESTRING EMPTY' ->KML ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'LINESTRING EMPTY' ->GML ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
    ]);
}

#[test]
fn gml_write_1() {
    check(&[
        (
            r"'LINEARRING (0 0, 1 1, 1 0, 0 0)' ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>0.0,0.0 1.0,1.0 1.0,0.0 0.0,0.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'LINEARRING (0 0, 1 1, 1 0, 0 0)' ->KML",
            Ok(
                r#"["<LineString><coordinates>0.0,0.0 1.0,1.0 1.0,0.0 0.0,0.0 </coordinates></LineString>"]"#,
            ),
        ),
        (
            r"'LINEARRING (0 0, 1 1, 1 0, 0 0)' ->GML ->WKT",
            Ok(r#"["LINESTRING (0 0, 1 1, 1 0, 0 0)"]"#),
        ),
        (
            r"'LINEARRING (0 0, 1 1, 1 0, 0 0)' ->KML ->GML",
            Ok(
                r#"["<gml:LineString><gml:coordinates>0.0,0.0 1.0,1.0 1.0,0.0 0.0,0.0 </gml:coordinates></gml:LineString>"]"#,
            ),
        ),
        (
            r"'LINEARRING (0 0, 1 1, 1 0, 0 0)' ->GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"LineString\",\"coordinates\":[[0.0,0.0],[1.0,1.0],[1.0,0.0],[0.0,0.0]]}"]"#,
            ),
        ),
        (
            r"'LINEARRING EMPTY' ->GML",
            Ok(r#"["<gml:LineString><gml:coordinates></gml:coordinates></gml:LineString>"]"#),
        ),
        (
            r"'LINEARRING EMPTY' ->KML",
            Ok(r#"["<LineString><coordinates></coordinates></LineString>"]"#),
        ),
        (
            r"'LINEARRING EMPTY' ->GML ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'LINEARRING EMPTY' ->KML ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'LINEARRING EMPTY' ->GML ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0))' ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0))' ->KML",
            Ok(
                r#"["<Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs></Polygon>"]"#,
            ),
        ),
        (
            r"'POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0))' ->GML ->WKT",
            Ok(r#"["POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0))"]"#),
        ),
        (
            r"'POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0))' ->KML ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0))' ->GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"Polygon\",\"coordinates\":[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]]]}"]"#,
            ),
        ),
        (
            r"'POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0), (2 2, 2 4, 4 4, 4 2, 2 2))' ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0), (2 2, 2 4, 4 4, 4 2, 2 2))' ->KML",
            Ok(
                r#"["<Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs><innerBoundaryIs><LinearRing><coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </coordinates></LinearRing></innerBoundaryIs></Polygon>"]"#,
            ),
        ),
        (
            r"'POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0), (2 2, 2 4, 4 4, 4 2, 2 2))' ->GML ->WKT",
            Ok(r#"["POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0), (2 2, 2 4, 4 4, 4 2, 2 2))"]"#),
        ),
        (
            r"'POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0), (2 2, 2 4, 4 4, 4 2, 2 2))' ->KML ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs><gml:innerBoundaryIs><gml:LinearRing><gml:coordinates>2.0,2.0 2.0,4.0 4.0,4.0 4.0,2.0 2.0,2.0 </gml:coordinates></gml:LinearRing></gml:innerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0), (2 2, 2 4, 4 4, 4 2, 2 2))' ->GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"Polygon\",\"coordinates\":[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]],[[2.0,2.0],[2.0,4.0],[4.0,4.0],[4.0,2.0],[2.0,2.0]]]}"]"#,
            ),
        ),
        (
            r"'POLYGON EMPTY' ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates></gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'POLYGON EMPTY' ->KML",
            Ok(
                r#"["<Polygon><outerBoundaryIs><LinearRing><coordinates></coordinates></LinearRing></outerBoundaryIs></Polygon>"]"#,
            ),
        ),
        (
            r"'POLYGON EMPTY' ->GML ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'POLYGON EMPTY' ->KML ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'POLYGON EMPTY' ->GML ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'POLYGON ((0 0 1, 0 10 2, 10 10 3, 10 0 4, 0 0 1))' ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0,1.0 0.0,10.0,2.0 10.0,10.0,3.0 10.0,0.0,4.0 0.0,0.0,1.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'POLYGON ((0 0 1, 0 10 2, 10 10 3, 10 0 4, 0 0 1))' ->KML",
            Ok(
                r#"["<Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0,1.0 0.0,10.0,2.0 10.0,10.0,3.0 10.0,0.0,4.0 0.0,0.0,1.0 </coordinates></LinearRing></outerBoundaryIs></Polygon>"]"#,
            ),
        ),
        (
            r"'POLYGON ((0 0 1, 0 10 2, 10 10 3, 10 0 4, 0 0 1))' ->GML ->WKT",
            Ok(r#"["POLYGON ((0 0, 0 10, 10 10, 10 0, 0 0))"]"#),
        ),
        (
            r"'POLYGON ((0 0 1, 0 10 2, 10 10 3, 10 0 4, 0 0 1))' ->KML ->GML",
            Ok(
                r#"["<gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0,1.0 0.0,10.0,2.0 10.0,10.0,3.0 10.0,0.0,4.0 0.0,0.0,1.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon>"]"#,
            ),
        ),
        (
            r"'POLYGON ((0 0 1, 0 10 2, 10 10 3, 10 0 4, 0 0 1))' ->GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"Polygon\",\"coordinates\":[[[0.0,0.0,1.0],[0.0,10.0,2.0],[10.0,10.0,3.0],[10.0,0.0,4.0],[0.0,0.0,1.0]]]}"]"#,
            ),
        ),
        (
            r"'MULTIPOINT ((1 2), (3 4))' ->GML",
            Ok(
                r#"["<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>3.0,4.0 </gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>"]"#,
            ),
        ),
        (
            r"'MULTIPOINT ((1 2), (3 4))' ->KML",
            Ok(
                r#"["<MultiGeometry><Point><coordinates>1.0,2.0 </coordinates></Point><Point><coordinates>3.0,4.0 </coordinates></Point></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'MULTIPOINT ((1 2), (3 4))' ->GML ->WKT",
            Ok(r#"["MULTIPOINT ((1 2), (3 4))"]"#),
        ),
        (
            r"'MULTIPOINT ((1 2), (3 4))' ->KML ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point></gml:geometryMember><gml:geometryMember><gml:Point><gml:coordinates>3.0,4.0 </gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'MULTIPOINT ((1 2), (3 4))' ->GML ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPoint\",\"coordinates\":[[1.0,2.0],[3.0,4.0]]}"]"#),
        ),
        (
            r"'MULTIPOINT ((1 2 3), (4 5 6))' ->GML",
            Ok(
                r#"["<gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1.0,2.0,3.0 </gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>4.0,5.0,6.0 </gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint>"]"#,
            ),
        ),
        (
            r"'MULTIPOINT ((1 2 3), (4 5 6))' ->KML",
            Ok(
                r#"["<MultiGeometry><Point><coordinates>1.0,2.0,3.0 </coordinates></Point><Point><coordinates>4.0,5.0,6.0 </coordinates></Point></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'MULTIPOINT ((1 2 3), (4 5 6))' ->GML ->WKT",
            Ok(r#"["MULTIPOINT ((1 2), (4 5))"]"#),
        ),
        (
            r"'MULTIPOINT ((1 2 3), (4 5 6))' ->KML ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1.0,2.0,3.0 </gml:coordinates></gml:Point></gml:geometryMember><gml:geometryMember><gml:Point><gml:coordinates>4.0,5.0,6.0 </gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'MULTIPOINT ((1 2 3), (4 5 6))' ->GML ->GEOJSON",
            Ok(r#"["{\"type\":\"MultiPoint\",\"coordinates\":[[1.0,2.0,3.0],[4.0,5.0,6.0]]}"]"#),
        ),
        (
            r"'MULTIPOINT EMPTY' ->GML",
            Ok(r#"["<gml:MultiPoint></gml:MultiPoint>"]"#),
        ),
        (
            r"'MULTIPOINT EMPTY' ->KML",
            Ok(r#"["<MultiGeometry></MultiGeometry>"]"#),
        ),
        (
            r"'MULTIPOINT EMPTY' ->GML ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'MULTIPOINT EMPTY' ->KML ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'MULTIPOINT EMPTY' ->GML ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'MULTIPOINT (EMPTY)' ->GML",
            Err(r"->GML was given invalid input. (Invalid number: EMPTY (line 1))"),
        ),
        (
            r"'MULTIPOINT (EMPTY)' ->KML",
            Err(r"->KML was given invalid input. (Invalid number: EMPTY (line 1))"),
        ),
        (
            r"'MULTIPOINT (EMPTY)' ->GML ->WKT",
            Err(r"->GML was given invalid input. (Invalid number: EMPTY (line 1))"),
        ),
        (
            r"'MULTIPOINT (EMPTY)' ->KML ->GML",
            Err(r"->KML was given invalid input. (Invalid number: EMPTY (line 1))"),
        ),
        (
            r"'MULTIPOINT (EMPTY)' ->GML ->GEOJSON",
            Err(r"->GML was given invalid input. (Invalid number: EMPTY (line 1))"),
        ),
        (
            r"'MULTILINESTRING ((1 2, 3 4), (5 6, 7 8))' ->GML",
            Ok(
                r#"["<gml:MultiLineString><gml:lineStringMember><gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString></gml:lineStringMember><gml:lineStringMember><gml:LineString><gml:coordinates>5.0,6.0 7.0,8.0 </gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString>"]"#,
            ),
        ),
        (
            r"'MULTILINESTRING ((1 2, 3 4), (5 6, 7 8))' ->KML",
            Ok(
                r#"["<MultiGeometry><LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString><LineString><coordinates>5.0,6.0 7.0,8.0 </coordinates></LineString></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'MULTILINESTRING ((1 2, 3 4), (5 6, 7 8))' ->GML ->WKT",
            Ok(r#"["MULTILINESTRING ((1 2, 3 4), (5 6, 7 8))"]"#),
        ),
        (
            r"'MULTILINESTRING ((1 2, 3 4), (5 6, 7 8))' ->KML ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString></gml:geometryMember><gml:geometryMember><gml:LineString><gml:coordinates>5.0,6.0 7.0,8.0 </gml:coordinates></gml:LineString></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'MULTILINESTRING ((1 2, 3 4), (5 6, 7 8))' ->GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiLineString\",\"coordinates\":[[[1.0,2.0],[3.0,4.0]],[[5.0,6.0],[7.0,8.0]]]}"]"#,
            ),
        ),
        (
            r"'MULTILINESTRING EMPTY' ->GML",
            Ok(r#"["<gml:MultiLineString></gml:MultiLineString>"]"#),
        ),
        (
            r"'MULTILINESTRING EMPTY' ->KML",
            Ok(r#"["<MultiGeometry></MultiGeometry>"]"#),
        ),
        (
            r"'MULTILINESTRING EMPTY' ->GML ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'MULTILINESTRING EMPTY' ->KML ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'MULTILINESTRING EMPTY' ->GML ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_write_2() {
    check(&[
        (
            r"'MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0)), ((20 20, 20 30, 30 30, 30 20, 20 20)))' ->GML",
            Ok(
                r#"["<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>20.0,20.0 20.0,30.0 30.0,30.0 30.0,20.0 20.0,20.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>"]"#,
            ),
        ),
        (
            r"'MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0)), ((20 20, 20 30, 30 30, 30 20, 20 20)))' ->KML",
            Ok(
                r#"["<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs></Polygon><Polygon><outerBoundaryIs><LinearRing><coordinates>20.0,20.0 20.0,30.0 30.0,30.0 30.0,20.0 20.0,20.0 </coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0)), ((20 20, 20 30, 30 30, 30 20, 20 20)))' ->GML ->WKT",
            Ok(
                r#"["MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0)), ((20 20, 20 30, 30 30, 30 20, 20 20)))"]"#,
            ),
        ),
        (
            r"'MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0)), ((20 20, 20 30, 30 30, 30 20, 20 20)))' ->KML ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:geometryMember><gml:geometryMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>20.0,20.0 20.0,30.0 30.0,30.0 30.0,20.0 20.0,20.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0)), ((20 20, 20 30, 30 30, 30 20, 20 20)))' ->GML ->GEOJSON",
            Ok(
                r#"["{\"type\":\"MultiPolygon\",\"coordinates\":[[[[0.0,0.0],[0.0,10.0],[10.0,10.0],[10.0,0.0],[0.0,0.0]]],[[[20.0,20.0],[20.0,30.0],[30.0,30.0],[30.0,20.0],[20.0,20.0]]]]}"]"#,
            ),
        ),
        (
            r"'MULTIPOLYGON EMPTY' ->GML",
            Ok(r#"["<gml:MultiPolygon></gml:MultiPolygon>"]"#),
        ),
        (
            r"'MULTIPOLYGON EMPTY' ->KML",
            Ok(r#"["<MultiGeometry></MultiGeometry>"]"#),
        ),
        (
            r"'MULTIPOLYGON EMPTY' ->GML ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'MULTIPOLYGON EMPTY' ->KML ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'MULTIPOLYGON EMPTY' ->GML ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'MULTIPOLYGON (EMPTY, ((0 0, 0 10, 10 10, 10 0, 0 0)))' ->GML",
            Ok(
                r#"["<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates></gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>"]"#,
            ),
        ),
        (
            r"'MULTIPOLYGON (EMPTY, ((0 0, 0 10, 10 10, 10 0, 0 0)))' ->KML",
            Ok(
                r#"["<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates></coordinates></LinearRing></outerBoundaryIs></Polygon><Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'MULTIPOLYGON (EMPTY, ((0 0, 0 10, 10 10, 10 0, 0 0)))' ->GML ->WKT",
            Err(
                r"->WKT was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'MULTIPOLYGON (EMPTY, ((0 0, 0 10, 10 10, 10 0, 0 0)))' ->KML ->GML",
            Err(
                r"->GML was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'MULTIPOLYGON (EMPTY, ((0 0, 0 10, 10 10, 10 0, 0 0)))' ->GML ->GEOJSON",
            Err(
                r"->GEOJSON was given invalid input. (Cannot create a coordinate sequence without text to parse)",
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT (1 2), LINESTRING (1 2, 3 4))' ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point></gml:geometryMember><gml:geometryMember><gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT (1 2), LINESTRING (1 2, 3 4))' ->KML",
            Ok(
                r#"["<MultiGeometry><geometryMember><Point><coordinates>1.0,2.0 </coordinates></Point></geometryMember><geometryMember><LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString></geometryMember></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT (1 2), LINESTRING (1 2, 3 4))' ->GML ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT (1 2), LINESTRING (1 2, 3 4))' ->KML ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT (1 2), LINESTRING (1 2, 3 4))' ->GML ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION EMPTY' ->GML",
            Ok(r#"["<gml:MultiGeometry></gml:MultiGeometry>"]"#),
        ),
        (
            r"'GEOMETRYCOLLECTION EMPTY' ->KML",
            Ok(r#"["<MultiGeometry></MultiGeometry>"]"#),
        ),
        (
            r"'GEOMETRYCOLLECTION EMPTY' ->GML ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION EMPTY' ->KML ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION EMPTY' ->GML ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (GEOMETRYCOLLECTION (POINT (1 2)))' ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:MultiGeometry><gml:geometryMember><gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (GEOMETRYCOLLECTION (POINT (1 2)))' ->KML",
            Ok(
                r#"["<MultiGeometry><geometryMember><MultiGeometry><geometryMember><Point><coordinates>1.0,2.0 </coordinates></Point></geometryMember></MultiGeometry></geometryMember></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (GEOMETRYCOLLECTION (POINT (1 2)))' ->GML ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (GEOMETRYCOLLECTION (POINT (1 2)))' ->KML ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (GEOMETRYCOLLECTION (POINT (1 2)))' ->GML ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT EMPTY)' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT EMPTY)' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT EMPTY)' ->GML ->WKT",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT EMPTY)' ->KML ->GML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT EMPTY)' ->GML ->GEOJSON",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT EMPTY, LINESTRING EMPTY, POLYGON EMPTY)' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT EMPTY, LINESTRING EMPTY, POLYGON EMPTY)' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT EMPTY, LINESTRING EMPTY, POLYGON EMPTY)' ->GML ->WKT",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT EMPTY, LINESTRING EMPTY, POLYGON EMPTY)' ->KML ->GML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (POINT EMPTY, LINESTRING EMPTY, POLYGON EMPTY)' ->GML ->GEOJSON",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOINT ((1 2), (3 4)), MULTILINESTRING ((1 2, 3 4)), MULTIPOLYGON (((0 0, 0 1, 1 1, 1 0, 0 0))))' ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:MultiPoint><gml:pointMember><gml:Point><gml:coordinates>1.0,2.0 </gml:coordinates></gml:Point></gml:pointMember><gml:pointMember><gml:Point><gml:coordinates>3.0,4.0 </gml:coordinates></gml:Point></gml:pointMember></gml:MultiPoint></gml:geometryMember><gml:geometryMember><gml:MultiLineString><gml:lineStringMember><gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString></gml:geometryMember><gml:geometryMember><gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,1.0 1.0,1.0 1.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOINT ((1 2), (3 4)), MULTILINESTRING ((1 2, 3 4)), MULTIPOLYGON (((0 0, 0 1, 1 1, 1 0, 0 0))))' ->KML",
            Ok(
                r#"["<MultiGeometry><geometryMember><MultiPoint><pointMember><Point><coordinates>1.0,2.0 </coordinates></Point></pointMember><pointMember><Point><coordinates>3.0,4.0 </coordinates></Point></pointMember></MultiPoint></geometryMember><geometryMember><MultiLineString><lineStringMember><LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString></lineStringMember></MultiLineString></geometryMember><geometryMember><MultiPolygon><polygonMember><Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,1.0 1.0,1.0 1.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs></Polygon></polygonMember></MultiPolygon></geometryMember></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOINT ((1 2), (3 4)), MULTILINESTRING ((1 2, 3 4)), MULTIPOLYGON (((0 0, 0 1, 1 1, 1 0, 0 0))))' ->GML ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOINT ((1 2), (3 4)), MULTILINESTRING ((1 2, 3 4)), MULTIPOLYGON (((0 0, 0 1, 1 1, 1 0, 0 0))))' ->KML ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOINT ((1 2), (3 4)), MULTILINESTRING ((1 2, 3 4)), MULTIPOLYGON (((0 0, 0 1, 1 1, 1 0, 0 0))))' ->GML ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (LINEARRING (0 0, 1 1, 1 0, 0 0))' ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:LineString><gml:coordinates>0.0,0.0 1.0,1.0 1.0,0.0 0.0,0.0 </gml:coordinates></gml:LineString></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (LINEARRING (0 0, 1 1, 1 0, 0 0))' ->KML",
            Ok(
                r#"["<MultiGeometry><geometryMember><LineString><coordinates>0.0,0.0 1.0,1.0 1.0,0.0 0.0,0.0 </coordinates></LineString></geometryMember></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (LINEARRING (0 0, 1 1, 1 0, 0 0))' ->GML ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (LINEARRING (0 0, 1 1, 1 0, 0 0))' ->KML ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (LINEARRING (0 0, 1 1, 1 0, 0 0))' ->GML ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOINT EMPTY)' ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:MultiPoint></gml:MultiPoint></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOINT EMPTY)' ->KML",
            Ok(
                r#"["<MultiGeometry><geometryMember><MultiPoint></MultiPoint></geometryMember></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOINT EMPTY)' ->GML ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOINT EMPTY)' ->KML ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOINT EMPTY)' ->GML ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOLYGON EMPTY)' ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:MultiPolygon></gml:MultiPolygon></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOLYGON EMPTY)' ->KML",
            Ok(
                r#"["<MultiGeometry><geometryMember><MultiPolygon></MultiPolygon></geometryMember></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOLYGON EMPTY)' ->GML ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOLYGON EMPTY)' ->KML ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOLYGON EMPTY)' ->GML ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
    ]);
}

#[test]
fn gml_write_3() {
    check(&[
        (
            r"'GEOMETRYCOLLECTION (MULTILINESTRING ((1 2, 3 4)))' ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:MultiLineString><gml:lineStringMember><gml:LineString><gml:coordinates>1.0,2.0 3.0,4.0 </gml:coordinates></gml:LineString></gml:lineStringMember></gml:MultiLineString></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTILINESTRING ((1 2, 3 4)))' ->KML",
            Ok(
                r#"["<MultiGeometry><geometryMember><MultiLineString><lineStringMember><LineString><coordinates>1.0,2.0 3.0,4.0 </coordinates></LineString></lineStringMember></MultiLineString></geometryMember></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTILINESTRING ((1 2, 3 4)))' ->GML ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTILINESTRING ((1 2, 3 4)))' ->KML ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTILINESTRING ((1 2, 3 4)))' ->GML ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0)), ((20 20, 20 30, 30 30, 30 20, 20 20))), POINT (5 6))' ->GML",
            Ok(
                r#"["<gml:MultiGeometry><gml:geometryMember><gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>20.0,20.0 20.0,30.0 30.0,30.0 30.0,20.0 20.0,20.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon></gml:geometryMember><gml:geometryMember><gml:Point><gml:coordinates>5.0,6.0 </gml:coordinates></gml:Point></gml:geometryMember></gml:MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0)), ((20 20, 20 30, 30 30, 30 20, 20 20))), POINT (5 6))' ->KML",
            Ok(
                r#"["<MultiGeometry><geometryMember><MultiPolygon><polygonMember><Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,0.0 0.0,10.0 10.0,10.0 10.0,0.0 0.0,0.0 </coordinates></LinearRing></outerBoundaryIs></Polygon></polygonMember><polygonMember><Polygon><outerBoundaryIs><LinearRing><coordinates>20.0,20.0 20.0,30.0 30.0,30.0 30.0,20.0 20.0,20.0 </coordinates></LinearRing></outerBoundaryIs></Polygon></polygonMember></MultiPolygon></geometryMember><geometryMember><Point><coordinates>5.0,6.0 </coordinates></Point></geometryMember></MultiGeometry>"]"#,
            ),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0)), ((20 20, 20 30, 30 30, 30 20, 20 20))), POINT (5 6))' ->GML ->WKT",
            Err(r"->WKT was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0)), ((20 20, 20 30, 30 30, 30 20, 20 20))), POINT (5 6))' ->KML ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'GEOMETRYCOLLECTION (MULTIPOLYGON (((0 0, 0 10, 10 10, 10 0, 0 0)), ((20 20, 20 30, 30 30, 30 20, 20 20))), POINT (5 6))' ->GML ->GEOJSON",
            Err(r"->GEOJSON was given invalid input. (NullPointerException)"),
        ),
        (
            r"'POINT EMPTY' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (r"'POINT (1 2 3)' ->KML ->WKT", Ok(r#"["POINT (1 2)"]"#)),
        (
            r"'LINESTRING EMPTY' ->KML",
            Ok(r#"["<LineString><coordinates></coordinates></LineString>"]"#),
        ),
        (
            r"'MULTIPOINT ((1 2), EMPTY)' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'MULTIPOINT ((1 2), EMPTY)' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"[ 'e0' ] ->GEOSHAPE ->GML",
            Ok(
                r#"["<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,45.0 22.5,45.0 22.5,56.25 0.0,56.25 0.0,45.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>"]"#,
            ),
        ),
        (
            r"[ 'e0' ] ->GEOSHAPE ->KML",
            Ok(
                r#"["<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,45.0 22.5,45.0 22.5,56.25 0.0,56.25 0.0,45.0 </coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>"]"#,
            ),
        ),
        (
            r"[ 'e0' 'e1' ] ->GEOSHAPE T ->GML",
            Ok(
                r#"["<gml:MultiPolygon><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>0.0,45.0 22.49999991618097,45.0 22.49999991618097,56.249999958090484 0.0,56.249999958090484 0.0,45.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember><gml:polygonMember><gml:Polygon><gml:outerBoundaryIs><gml:LinearRing><gml:coordinates>22.5,45.0 44.99999991618097,45.0 44.99999991618097,56.249999958090484 22.5,56.249999958090484 22.5,45.0 </gml:coordinates></gml:LinearRing></gml:outerBoundaryIs></gml:Polygon></gml:polygonMember></gml:MultiPolygon>"]"#,
            ),
        ),
        (
            r"[ 'e0' ] ->GEOSHAPE T ->KML",
            Ok(
                r#"["<MultiGeometry><Polygon><outerBoundaryIs><LinearRing><coordinates>0.0,45.0 22.49999991618097,45.0 22.49999991618097,56.249999958090484 0.0,56.249999958090484 0.0,45.0 </coordinates></LinearRing></outerBoundaryIs></Polygon></MultiGeometry>"]"#,
            ),
        ),
        (r"T ->GML", Err(r"Empty stack.")),
        (
            r"1 ->GML",
            Err(
                r"->GML expects a GEOSHAPE, a WKT STRING, a GeoJSON STRING, a KML STRING or a GeoJSON STRING. (Unknown geometry format.)",
            ),
        ),
        (r"[1] ->GML", Err(r"Unknown function '[1]'")),
        (
            r"'' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'x' ->GML",
            Err(r"->GML was given invalid input. (Unknown geometry type: x (line 1))"),
        ),
        (
            r"'{}' ->GML",
            Err(
                r"->GML was given invalid input. (java.lang.NullPointerException (NullPointerException))",
            ),
        ),
        (
            r"'' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<' ->GML",
            Err(
                r"->GML was given invalid input. (XML document structures must start and end within the same entity.)",
            ),
        ),
        (
            r"'<kml/>' ->GML",
            Err(r"->GML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<kml/>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"'<gml:Point/>' ->KML",
            Err(r"->KML was given invalid input. (NullPointerException)"),
        ),
        (
            r"NULL ->GML",
            Err(
                r"->GML expects a GEOSHAPE, a WKT STRING, a GeoJSON STRING, a KML STRING or a GeoJSON STRING. (Unknown geometry format.)",
            ),
        ),
        (
            r"NULL ->KML",
            Err(
                r"->KML expects a GEOSHAPE, a WKT STRING, a GeoJSON STRING, a KML STRING or a GeoJSON STRING. (Unknown geometry format.)",
            ),
        ),
    ]);
}
