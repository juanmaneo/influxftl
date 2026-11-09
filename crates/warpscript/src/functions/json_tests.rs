// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use indexmap::{IndexMap, IndexSet};

use super::json::{
    ATTRIBUTE_JSON_PRETTY, JsonError, JsonOptions, from_json, number_value, to_json,
};
use super::misc::ATTRIBUTE_JSON_STRICT;
use crate::stack::Stack;
use crate::tests::{assert_fails, assert_top, exec, top};
use crate::value::Value;

// Reference values in this file were produced by Jackson 2.14.2 (the version Warp 10 pins)
// configured like `io.warp10.json.JsonUtils`.

/// The list of valid values quoted by the parser errors.
const VALID: &str = "(JSON String, Number (or 'NaN'/'INF'/'+INF'), Array, Object or token 'null', 'true' or 'false')";

fn json(script: &str) -> String {
    match top(script) {
        Value::String(s) => s.to_string(),
        other => panic!("not a string: {other:?}"),
    }
}

fn render(value: &Value, strict: bool, pretty: bool) -> String {
    let options = JsonOptions {
        strict,
        pretty,
        ..JsonOptions::default()
    };
    to_json(value, options).unwrap()
}

fn bounded(value: &Value, pretty: bool, max_size: i64) -> Result<String, JsonError> {
    let options = JsonOptions {
        strict: false,
        pretty,
        max_size,
    };
    to_json(value, options)
}

fn parse(text: &str) -> Value {
    from_json(text).unwrap()
}

fn parse_err(text: &str) -> String {
    from_json(text).unwrap_err().to_string()
}

fn list(items: Vec<Value>) -> Value {
    Value::list(items)
}

fn map(entries: Vec<(Value, Value)>) -> Value {
    Value::map(entries.into_iter().collect::<IndexMap<_, _>>())
}

fn nested_lists(depth: usize) -> Value {
    let mut value = list(vec![]);
    for _ in 1..depth {
        value = list(vec![value]);
    }
    value
}

/// Run `test` on a thread with a large stack: the recursive descent over 1000 nested
/// containers (the Jackson limit) needs more than the 2 MB of a test thread in debug builds.
fn with_big_stack(test: fn()) {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

// -----------------------------------------------------------------------------------------
// ->JSON: scalars
// -----------------------------------------------------------------------------------------

#[test]
fn tojson_numbers() {
    assert_eq!(json("42 ->JSON"), "42");
    assert_eq!(json("MINLONG ->JSON"), "-9223372036854775808");
    assert_eq!(json("MAXLONG ->JSON"), "9223372036854775807");
    assert_eq!(json("1.0 ->JSON"), "1.0");
    assert_eq!(json("123.456 ->JSON"), "123.456");
    assert_eq!(json("-0.0 ->JSON"), "-0.0");
    assert_eq!(json("0.1 0.2 + ->JSON"), "0.30000000000000004");
    let doubles = [
        (1e7, "1.0E7"),
        (1e-3, "0.001"),
        (1e-4, "1.0E-4"),
        (f64::MAX, "1.7976931348623157E308"),
        (f64::MIN_POSITIVE, "2.2250738585072014E-308"),
    ];
    for (d, expected) in doubles {
        assert_eq!(render(&Value::Double(d), false, false), expected);
        assert_eq!(render(&Value::Double(d), true, false), expected);
    }
}

#[test]
fn tojson_non_finite_doubles() {
    assert_eq!(json("NaN ->JSON"), "NaN");
    assert_eq!(json("1.0 0.0 / ->JSON"), "Infinity");
    assert_eq!(json("-1.0 0.0 / ->JSON"), "-Infinity");
    assert_eq!(json("JSONSTRICT NaN ->JSON"), "\"NaN\"");
    assert_eq!(json("JSONSTRICT 1.0 0.0 / ->JSON"), "\"Infinity\"");
    assert_eq!(json("JSONSTRICT -1.0 0.0 / ->JSON"), "\"-Infinity\"");
    assert_eq!(json("JSONSTRICT JSONLOOSE NaN ->JSON"), "NaN");
    assert_eq!(json("[ NaN 1.0 ] ->JSON"), "[NaN,1.0]");
    assert_eq!(json("JSONSTRICT [ NaN 1.0 ] ->JSON"), "[\"NaN\",1.0]");
    assert_eq!(json("JSONPRETTY [ NaN ] ->JSON"), "[ NaN ]");
    assert_eq!(json("JSONPRETTY JSONSTRICT [ NaN ] ->JSON"), "[ \"NaN\" ]");
    assert_eq!(
        json("JSONPRETTY JSONSTRICT [ NaN 1.0 0.0 / -1.0 0.0 / 1.5 ] ->JSON"),
        "[ \"NaN\", \"Infinity\", \"-Infinity\", 1.5 ]"
    );
    assert_eq!(
        json("JSONPRETTY [ NaN 1.0 0.0 / -1.0 0.0 / 1.5 ] ->JSON"),
        "[ NaN, Infinity, -Infinity, 1.5 ]"
    );
    assert_eq!(
        json("JSONSTRICT { 'a' NaN 'b' -1.0 0.0 / } ->JSON"),
        "{\"a\":\"NaN\",\"b\":\"-Infinity\"}"
    );
}

#[test]
fn tojson_booleans_and_null() {
    assert_eq!(json("true ->JSON"), "true");
    assert_eq!(json("false ->JSON"), "false");
    assert_eq!(json("NULL ->JSON"), "null");
    assert_eq!(json("JSONPRETTY NULL ->JSON"), "null");
}

#[test]
fn tojson_strings() {
    assert_eq!(json("'' ->JSON"), "\"\"");
    assert_eq!(json("'hello world' ->JSON"), "\"hello world\"");
    assert_eq!(json("JSONPRETTY 'x' ->JSON"), "\"x\"");
    assert_eq!(
        json("'q%22 bs%5C sl/ tab%09 nl%0A cr%0D bs%08 ff%0C' ->JSON"),
        "\"q\\\" bs\\\\ sl/ tab\\t nl\\n cr\\r bs\\b ff\\f\""
    );
    assert_eq!(
        json("'%00%01%1F%7F' ->JSON"),
        "\"\\u0000\\u0001\\u001F\u{7f}\""
    );
    assert_eq!(json("'%7F' ->JSON"), "\"\u{7f}\"");
    assert_eq!(
        json("'\u{e9}\u{ff}\u{100}\u{20ac}\u{2028}' ->JSON"),
        "\"\\u00E9\\u00FF\\u0100\\u20AC\\u2028\""
    );
    assert_eq!(json("'a\u{1f600}b' ->JSON"), "\"a\\uD83D\\uDE00b\"");
    assert_eq!(json("'\u{ffff}\u{fffe}' ->JSON"), "\"\\uFFFF\\uFFFE\"");
}

#[test]
fn tojson_bytes_and_objects() {
    // `BytesSerializer`: the ISO-8859-1 string of the bytes.
    assert_eq!(
        json("'00417f80ff225c0a' HEX-> ->JSON"),
        "\"\\u0000A\u{7f}\\u0080\\u00FF\\\"\\\\\\n\""
    );
    assert_eq!(render(&Value::bytes([]), false, false), "\"\"");
    assert_eq!(json("<% 1 2 + %> ->JSON"), "\"<% 1 2 + %>\"");
    assert_eq!(json("test.object ->JSON"), "\"'test.object' FUNCREF\"");
    assert_eq!(json("'01' HEX-> BYTESTOBITS ->JSON"), "null");
    assert_eq!(json("MARK ->JSON"), "null");
    assert_eq!(json("SAVE ->JSON"), "null");
    assert_eq!(json("'a' MATCHER ->JSON"), "null");
    assert_eq!(
        json("[ <% 1 2 + %> test.object 'a' MATCHER 'A' ] ->JSON"),
        "[\"<% 1 2 + %>\",\"'test.object' FUNCREF\",null,\"A\"]"
    );
    let context = top("SAVE");
    assert_eq!(
        render(&list(vec![Value::Mark, context, Value::Null]), false, true),
        "[ null, null, null ]"
    );
}

// -----------------------------------------------------------------------------------------
// ->JSON: containers, compact and pretty
// -----------------------------------------------------------------------------------------

#[test]
fn tojson_lists_and_sets() {
    assert_eq!(json("[] ->JSON"), "[]");
    assert_eq!(json("JSONPRETTY [] ->JSON"), "[ ]");
    assert_eq!(
        json("[ 1 2.5 'a' true NULL ] ->JSON"),
        "[1,2.5,\"a\",true,null]"
    );
    assert_eq!(
        json("JSONPRETTY [ 1 2.5 'a' true NULL ] ->JSON"),
        "[ 1, 2.5, \"a\", true, null ]"
    );
    assert_eq!(json("[ [ 1 2 ] [] [ [ 3 ] ] ] ->JSON"), "[[1,2],[],[[3]]]");
    assert_eq!(
        json("JSONPRETTY [ [ 1 2 ] [] [ [ 3 ] ] ] ->JSON"),
        "[ [ 1, 2 ], [ ], [ [ 3 ] ] ]"
    );
    assert_eq!(json("[ 1 'a' 2 ] ->SET ->JSON"), "[1,\"a\",2]");
    assert_eq!(
        json("JSONPRETTY [ 1 'a' 2 ] ->SET ->JSON"),
        "[ 1, \"a\", 2 ]"
    );
    assert_eq!(json("[] ->SET ->JSON"), "[]");
    assert_eq!(json("JSONPRETTY [] ->SET ->JSON"), "[ ]");
    assert_eq!(json("JSONPRETTY JSONCOMPACT [ 1 2 ] ->JSON"), "[1,2]");
}

#[test]
fn tojson_maps() {
    assert_eq!(json("{} ->JSON"), "{}");
    assert_eq!(json("JSONPRETTY {} ->JSON"), "{ }");
    assert_eq!(
        json("{ 'a' 1 'b' 2.5 'c' 'x' 'd' true 'e' NULL } ->JSON"),
        "{\"a\":1,\"b\":2.5,\"c\":\"x\",\"d\":true,\"e\":null}"
    );
    assert_eq!(
        json("JSONPRETTY { 'a' 1 'b' 2.5 'c' 'x' 'd' true 'e' NULL } ->JSON"),
        "{\n  \"a\" : 1,\n  \"b\" : 2.5,\n  \"c\" : \"x\",\n  \"d\" : true,\n  \"e\" : null\n}"
    );
    assert_eq!(
        json("JSONPRETTY { 'n' NULL 'l' [ NULL ] } ->JSON"),
        "{\n  \"n\" : null,\n  \"l\" : [ null ]\n}"
    );
    assert_eq!(json("JSONPRETTY { '' 1 } ->JSON"), "{\n  \"\" : 1\n}");
}

#[test]
fn tojson_map_keys() {
    // Non-string keys go through `toString()`, byte arrays through base64, null is "null".
    assert_eq!(
        json(
            "{ 1 'long' 2.5 'double' true 'bool' NULL 'null' 's' 'string' NaN 'nan' -0.0 'negzero' } ->JSON"
        ),
        "{\"1\":\"long\",\"2.5\":\"double\",\"true\":\"bool\",\"null\":\"null\",\"s\":\"string\",\"NaN\":\"nan\",\"-0.0\":\"negzero\"}"
    );
    assert_eq!(
        json("JSONSTRICT { NaN 'nan' 1.0 0.0 / 'inf' } ->JSON"),
        "{\"NaN\":\"nan\",\"Infinity\":\"inf\"}"
    );
    assert_eq!(
        json("{ [ 1 2 ] 'list' { 'k' 'v' } 'map' } ->JSON"),
        "{\"[1, 2]\":\"list\",\"{k=v}\":\"map\"}"
    );
    assert_eq!(
        json("{ 'ABC' 'UTF-8' ->BYTES 'bytes' } ->JSON"),
        "{\"QUJD\":\"bytes\"}"
    );
    assert_eq!(
        json("JSONPRETTY { 'ABC' 'UTF-8' ->BYTES 'bytes' } ->JSON"),
        "{\n  \"QUJD\" : \"bytes\"\n}"
    );
    assert_eq!(
        json("{ 'a%22b\u{e9}%0A' 1 } ->JSON"),
        "{\"a\\\"b\\u00E9\\n\":1}"
    );
    assert_eq!(
        json("{ <% 1 2 + %> 1 test.object 2 '01' HEX-> BYTESTOBITS 3 } ->JSON"),
        "{\"<% 1 2 + %>\":1,\"'test.object' FUNCREF\":2,\"{0}\":3}"
    );
    assert_eq!(json("{ [ 1 2 ] ->SET 1 } ->JSON"), "{\"[1, 2]\":1}");
    let keys = map(vec![
        (Value::Long(-1), Value::Long(1)),
        (Value::Double(1e7), Value::Long(2)),
        (Value::string("a\u{1f600}"), Value::Long(3)),
        (Value::Long(i64::MIN), Value::Long(4)),
        (Value::Double(1e-5), Value::Long(5)),
    ]);
    assert_eq!(
        render(&keys, false, false),
        "{\"-1\":1,\"1.0E7\":2,\"a\\uD83D\\uDE00\":3,\"-9223372036854775808\":4,\"1.0E-5\":5}"
    );
    assert_eq!(
        render(&keys, false, true),
        "{\n  \"-1\" : 1,\n  \"1.0E7\" : 2,\n  \"a\\uD83D\\uDE00\" : 3,\n  \"-9223372036854775808\" : 4,\n  \"1.0E-5\" : 5\n}"
    );
    assert_eq!(
        render(&map(vec![(Value::Mark, Value::Null)]), false, false),
        "{\"MARK\":null}"
    );
}

#[test]
fn tojson_pretty_nesting() {
    assert_eq!(
        json("JSONPRETTY { 'a' [ 1 [ 2 3 ] {} ] 'b' { 'c' [] } } ->JSON"),
        "{\n  \"a\" : [ 1, [ 2, 3 ], { } ],\n  \"b\" : {\n    \"c\" : [ ]\n  }\n}"
    );
    assert_eq!(
        json("JSONPRETTY [ { 'a' 1 } {} { 'b' { 'c' 2 } } [ {} ] ] ->JSON"),
        "[ {\n  \"a\" : 1\n}, { }, {\n  \"b\" : {\n    \"c\" : 2\n  }\n}, [ { } ] ]"
    );
    assert_eq!(
        json("JSONPRETTY { 'a' { 'b' { 'c' [ [ 1 ] ] } } } ->JSON"),
        "{\n  \"a\" : {\n    \"b\" : {\n      \"c\" : [ [ 1 ] ]\n    }\n  }\n}"
    );
    assert_eq!(
        json("JSONPRETTY { 'x' [ { 'y' [ 1 2 ] } [ [] ] ] } ->JSON"),
        "{\n  \"x\" : [ {\n    \"y\" : [ 1, 2 ]\n  }, [ [ ] ] ]\n}"
    );
    assert_eq!(
        json("JSONPRETTY { 's' [ 1 2 ] ->SET 'b' 'e9' HEX-> 'm' <% 1 2 + %> } ->JSON"),
        "{\n  \"s\" : [ 1, 2 ],\n  \"b\" : \"\\u00E9\",\n  \"m\" : \"<% 1 2 + %>\"\n}"
    );
    assert_eq!(
        json("{ 's' [ 1 2 ] ->SET 'b' 'e9' HEX-> 'm' <% 1 2 + %> } ->JSON"),
        "{\"s\":[1,2],\"b\":\"\\u00E9\",\"m\":\"<% 1 2 + %>\"}"
    );
}

#[test]
fn json_attributes() {
    let stack = exec("JSONLOOSE");
    assert_eq!(
        stack.attribute(ATTRIBUTE_JSON_STRICT),
        Some(&Value::Boolean(false))
    );
    let stack = exec("JSONPRETTY");
    assert_eq!(
        stack.attribute(ATTRIBUTE_JSON_PRETTY),
        Some(&Value::Boolean(true))
    );
    let stack = exec("JSONPRETTY JSONCOMPACT");
    assert_eq!(
        stack.attribute(ATTRIBUTE_JSON_PRETTY),
        Some(&Value::Boolean(false))
    );
    // Only `true` enables the modes.
    let mut stack = exec("[ NaN ]");
    stack.set_attribute(ATTRIBUTE_JSON_STRICT, Some(Value::Long(1)));
    stack.set_attribute(ATTRIBUTE_JSON_PRETTY, Some(Value::string("yes")));
    stack.exec("->JSON").unwrap();
    assert_eq!(stack.pop().unwrap(), Value::string("[NaN]"));
}

// -----------------------------------------------------------------------------------------
// ->JSON: size bound, depth, MAXJSON
// -----------------------------------------------------------------------------------------

#[test]
fn tojson_size_bound() {
    const TOO_BIG: &str = "->JSON failed with to convert to JSON. (Resulting JSON is too big. (Cannot write, maximum number of characters written :";
    assert_eq!(json("7 MAXJSON [ 1 2 3 ] ->JSON"), "[1,2,3]");
    assert_fails("6 MAXJSON [ 1 2 3 ] ->JSON", &format!("{TOO_BIG}6))"));
    assert_fails("0 MAXJSON NULL ->JSON", &format!("{TOO_BIG}0))"));
    assert_fails("-1 MAXJSON NULL ->JSON", &format!("{TOO_BIG}-1))"));
    assert_eq!(json("4 MAXJSON NULL ->JSON"), "null");
    assert_fails("3 MAXJSON NULL ->JSON", &format!("{TOO_BIG}3))"));
    assert_eq!(json("2 MAXJSON '' ->JSON"), "\"\"");
    assert_fails("1 MAXJSON '' ->JSON", &format!("{TOO_BIG}1))"));
    assert_eq!(json("8 MAXJSON JSONPRETTY [ 1 2 ] ->JSON"), "[ 1, 2 ]");
    assert_fails(
        "7 MAXJSON JSONPRETTY [ 1 2 ] ->JSON",
        &format!("{TOO_BIG}7))"),
    );
    assert_fails(
        "11 MAXJSON JSONPRETTY { 'a' 1 } ->JSON",
        &format!("{TOO_BIG}11))"),
    );
    assert_eq!(
        json("13 MAXJSON JSONPRETTY { 'a' 1 } ->JSON"),
        "{\n  \"a\" : 1\n}"
    );
    assert_eq!(json("MAXLONG MAXJSON [ 1 ] ->JSON"), "[1]");
    assert_fails("10 MAXJSON NEWENCODER ->JSON", &format!("{TOO_BIG}10))"));
    assert_eq!(json("MAXLONG 1 - MAXJSON [ 1 ] ->JSON"), "[1]");
    let big = Value::string("x".repeat(10_000));
    assert_eq!(bounded(&big, false, 10_002).unwrap().len(), 10_002);
    assert_eq!(bounded(&big, false, 10_001), Err(JsonError::TooBig));
    // The bound applies to characters of the escaped output.
    let astral = Value::string("\u{1f600}");
    assert_eq!(bounded(&astral, false, 14).unwrap(), "\"\\uD83D\\uDE00\"");
    assert_eq!(bounded(&astral, false, 13), Err(JsonError::TooBig));
    assert_eq!(
        bounded(&nested_lists(1000), false, 2000).unwrap().len(),
        2000
    );
    assert_eq!(
        bounded(&nested_lists(999), false, 2000).unwrap().len(),
        1998
    );
    assert_eq!(
        bounded(&nested_lists(1000), true, 2000),
        Err(JsonError::TooBig)
    );
    with_big_stack(|| {
        assert_eq!(
            bounded(&nested_lists(1000), false, 2000).unwrap().len(),
            2000
        );
        assert_eq!(
            bounded(&nested_lists(999), false, 2000).unwrap().len(),
            1998
        );
        assert_eq!(
            bounded(&nested_lists(1000), true, 2000),
            Err(JsonError::TooBig)
        );
    });
}

#[test]
fn tojson_depth() {
    const TOO_DEEP: &str = "->JSON failed with to convert to JSON, the structure is too deep or it references itself. (StackOverflowError)";
    with_big_stack(|| {
        assert_eq!(
            to_json(&nested_lists(1001), JsonOptions::default()),
            Err(JsonError::TooDeep)
        );
        let mut deep_map = map(vec![]);
        for _ in 0..1000 {
            deep_map = map(vec![(Value::string("k"), deep_map)]);
        }
        assert_eq!(
            to_json(&deep_map, JsonOptions::default()),
            Err(JsonError::TooDeep)
        );
        let mut deep_map = list(vec![]);
        for _ in 0..999 {
            deep_map = map(vec![(Value::string("k"), deep_map)]);
        }
        assert_eq!(
            to_json(&deep_map, JsonOptions::default()).unwrap().len(),
            999 * 6 + 2
        );
        let deep_set = Value::set(IndexSet::from([nested_lists(1000)]));
        assert_eq!(
            to_json(&deep_set, JsonOptions::default()),
            Err(JsonError::TooDeep)
        );
    });
    let self_map = map(vec![]);
    if let Value::Map(inner) = &self_map {
        inner
            .borrow_mut()
            .insert(Value::string("me"), self_map.clone());
    }
    assert_eq!(
        to_json(&self_map, JsonOptions::default()),
        Err(JsonError::TooDeep)
    );
    assert_fails("[] DUP DUP +! ->JSON", TOO_DEEP);
    assert_fails("->JSON", "Empty stack.");
}

#[test]
fn maxjson() {
    let stack = exec("5 MAXJSON");
    assert_eq!(stack.limits.max_json, 5);
    assert_fails("'x' MAXJSON", "MAXJSON expects a numeric (long) limit.");
    assert_fails("MAXJSON", "Empty stack.");
    // The hard limit cannot be exceeded (default stack: 20 MB).
    let mut stack = Stack::new();
    let err = stack.exec("20971521 MAXJSON").unwrap_err();
    assert!(
        err.to_string()
            .contains("MAXJSON missing capability 'maxjson' or 'limits'."),
        "{err}"
    );
    stack
        .capabilities_mut()
        .insert("limits".to_owned(), String::new());
    let err = stack.exec("20971521 MAXJSON").unwrap_err();
    assert!(
        err.to_string()
            .contains("MAXJSON cannot extend limit past 20971520"),
        "{err}"
    );
    assert_eq!(stack.limits.max_json, 20_971_520);
    stack.exec("20971520 MAXJSON 5 MAXJSON").unwrap();
    assert_eq!(stack.limits.max_json, 5);
}

// -----------------------------------------------------------------------------------------
// JSON->: values
// -----------------------------------------------------------------------------------------

#[test]
fn parse_integers() {
    assert_eq!(parse("42"), Value::Long(42));
    assert_eq!(parse("-42"), Value::Long(-42));
    assert_eq!(parse("-0"), Value::Long(0));
    assert_eq!(parse("0"), Value::Long(0));
    assert_eq!(parse("2147483648"), Value::Long(2_147_483_648));
    assert_eq!(parse("9223372036854775807"), Value::Long(i64::MAX));
    assert_eq!(parse("-9223372036854775808"), Value::Long(i64::MIN));
    // Beyond the long range Jackson yields BigIntegers; the port yields doubles.
    assert_eq!(
        parse("9223372036854775808"),
        Value::Double(9.223_372_036_854_776e18)
    );
    assert_eq!(
        parse("-9223372036854775809"),
        Value::Double(-9.223_372_036_854_776e18)
    );
    assert_eq!(
        parse("123456789012345678901234567890"),
        Value::Double(1.234_567_890_123_456_8e29)
    );
    assert_eq!(parse("[-0]"), list(vec![Value::Long(0)]));
}

#[test]
fn parse_doubles() {
    let cases = [
        ("1.5", 1.5),
        ("1.0", 1.0),
        ("0.0", 0.0),
        ("-0.5", -0.5),
        ("1e5", 100_000.0),
        ("1E5", 100_000.0),
        ("1e+5", 100_000.0),
        ("1e-5", 1e-5),
        ("1e05", 100_000.0),
        ("1.5E+3", 1500.0),
        ("-1E2", -100.0),
        ("1.0e5", 100_000.0),
        ("0.1e1", 1.0),
        ("0e1", 0.0),
        ("0E+1", 0.0),
        ("-0.0", -0.0),
        ("-0.0e-0", -0.0),
        ("1e400", f64::INFINITY),
        ("-1e400", f64::NEG_INFINITY),
        ("1e-400", 0.0),
        ("1e22", 1e22),
        ("100000000000000000000.0", 1e20),
        ("0.1000000000000000055511151231257827", 0.1),
        ("12345678901234567890", 1.234_567_890_123_456_7e19),
    ];
    for (text, expected) in cases {
        assert_eq!(parse(text), Value::Double(expected), "{text}");
    }
}

#[test]
fn parse_non_numeric_numbers() {
    assert_eq!(parse("NaN"), Value::Double(f64::NAN));
    assert_eq!(parse("Infinity"), Value::Double(f64::INFINITY));
    assert_eq!(parse("-Infinity"), Value::Double(f64::NEG_INFINITY));
    assert_eq!(parse("+Infinity"), Value::Double(f64::INFINITY));
    assert_eq!(parse("-INF"), Value::Double(f64::NEG_INFINITY));
    assert_eq!(parse("+INF"), Value::Double(f64::INFINITY));
    assert_eq!(
        parse("[-INF, +INF]"),
        list(vec![
            Value::Double(f64::NEG_INFINITY),
            Value::Double(f64::INFINITY)
        ])
    );
    assert_eq!(
        parse("[NaN,Infinity,-Infinity]"),
        list(vec![
            Value::Double(f64::NAN),
            Value::Double(f64::INFINITY),
            Value::Double(f64::NEG_INFINITY)
        ])
    );
    assert_eq!(
        parse("{\"a\":NaN}"),
        map(vec![(Value::string("a"), Value::Double(f64::NAN))])
    );
}

#[test]
fn parse_strings() {
    assert_eq!(parse("\"abc\""), Value::string("abc"));
    assert_eq!(parse("\"\""), Value::string(""));
    assert_eq!(parse("\"\\u0000\""), Value::string("\0"));
    assert_eq!(
        parse(
            "\"q\\\" bs\\\\ sl\\/ b\\b f\\f n\\n r\\r t\\t u\\u00e9 U\\u00E9 ast\\ud83d\\ude00\""
        ),
        Value::string("q\" bs\\ sl/ b\u{8} f\u{c} n\n r\r t\t u\u{e9} U\u{e9} ast\u{1f600}")
    );
    assert_eq!(parse("\"\\ud800\\udc00\""), Value::string("\u{10000}"));
    // Lone surrogates (which Java keeps as such) become U+FFFD.
    assert_eq!(parse("\"\\ud800x\""), Value::string("\u{fffd}x"));
    assert_eq!(parse("\"\\udc00x\""), Value::string("\u{fffd}x"));
    assert_eq!(
        parse("\"\\ude00\\ud83d\""),
        Value::string("\u{fffd}\u{fffd}")
    );
    assert_eq!(parse("\"\\ud800\\u0041\""), Value::string("\u{fffd}A"));
    assert_eq!(parse("\"\\ud800\""), Value::string("\u{fffd}"));
    assert_eq!(parse("\"\u{1f600}\""), Value::string("\u{1f600}"));
    assert_eq!(parse("\"a\u{7f}b\""), Value::string("a\u{7f}b"));
    assert_eq!(parse("\"\u{e9}\""), Value::string("\u{e9}"));
}

#[test]
fn parse_literals_and_whitespace() {
    assert_eq!(parse("true"), Value::Boolean(true));
    assert_eq!(parse("false"), Value::Boolean(false));
    assert_eq!(parse("null"), Value::Null);
    assert_eq!(
        parse("  \r\n\t [ \r\n 1 \t ] \r\n "),
        list(vec![Value::Long(1)])
    );
    assert_eq!(parse("[1]\t\n "), list(vec![Value::Long(1)]));
    assert_eq!(parse("1 "), Value::Long(1));
    assert_eq!(parse("1\t"), Value::Long(1));
    assert_eq!(parse("1\n"), Value::Long(1));
    assert_eq!(parse("1\r"), Value::Long(1));
}

#[test]
fn parse_arrays() {
    let longs = |items: &[i64]| list(items.iter().map(|&l| Value::Long(l)).collect());
    assert_eq!(parse("[]"), list(vec![]));
    assert_eq!(parse("[ ]"), list(vec![]));
    assert_eq!(
        parse("[1,2.5,\"a\",true,null,[],{}]"),
        list(vec![
            Value::Long(1),
            Value::Double(2.5),
            Value::string("a"),
            Value::Boolean(true),
            Value::Null,
            list(vec![]),
            map(vec![])
        ])
    );
    assert_eq!(
        parse("[[[[1]]]]"),
        list(vec![list(vec![list(vec![longs(&[1])])])])
    );
    // `ALLOW_MISSING_VALUES`.
    let null = Value::Null;
    assert_eq!(
        parse("[1,,2]"),
        list(vec![Value::Long(1), null.clone(), Value::Long(2)])
    );
    assert_eq!(
        parse("[1, ,2]"),
        list(vec![Value::Long(1), null.clone(), Value::Long(2)])
    );
    assert_eq!(parse("[1,]"), list(vec![Value::Long(1), null.clone()]));
    assert_eq!(
        parse("[1,2,]"),
        list(vec![Value::Long(1), Value::Long(2), null.clone()])
    );
    assert_eq!(parse("[,1]"), list(vec![null.clone(), Value::Long(1)]));
    assert_eq!(parse("[,]"), list(vec![null.clone(), null.clone()]));
    assert_eq!(parse("[ , ]"), list(vec![null.clone(), null.clone()]));
    assert_eq!(
        parse("[,,]"),
        list(vec![null.clone(), null.clone(), null.clone()])
    );
    assert_eq!(
        parse("[1,,]"),
        list(vec![Value::Long(1), null.clone(), null.clone()])
    );
    assert_eq!(parse("[,,,]"), list(vec![null.clone(); 4]));
    assert_eq!(
        parse("[,1,,2,]"),
        list(vec![
            null.clone(),
            Value::Long(1),
            null.clone(),
            Value::Long(2),
            null.clone()
        ])
    );
    assert_eq!(
        parse("[[,],[]]"),
        list(vec![list(vec![null.clone(), null.clone()]), list(vec![])])
    );
    assert_eq!(parse("[,{}]"), list(vec![null, map(vec![])]));
    assert_eq!(
        parse("[9223372036854775808]"),
        list(vec![Value::Double(9.223_372_036_854_776e18)])
    );
}

#[test]
fn parse_objects() {
    assert_eq!(parse("{}"), map(vec![]));
    assert_eq!(parse("{ }"), map(vec![]));
    assert_eq!(
        parse("{\"a\":1,\"b\":[2],\"c\":{\"d\":null}}"),
        map(vec![
            (Value::string("a"), Value::Long(1)),
            (Value::string("b"), list(vec![Value::Long(2)])),
            (
                Value::string("c"),
                map(vec![(Value::string("d"), Value::Null)])
            ),
        ])
    );
    assert_eq!(
        parse("{\"a\":1 ,\"b\":2}"),
        parse("{ \"a\" : 1 , \"b\" : 2 }")
    );
    assert_eq!(parse("{\"a\"\n:\n1\n}"), parse("{\"a\":1}"));
    assert_eq!(
        parse("{\"\":1,\"null\":2}"),
        map(vec![
            (Value::string(""), Value::Long(1)),
            (Value::string("null"), Value::Long(2))
        ])
    );
    assert_eq!(
        parse("{\"a\\nb\\u00e9\":1}"),
        map(vec![(Value::string("a\nb\u{e9}"), Value::Long(1))])
    );
    assert_eq!(
        parse("{\"\u{e9}\":\"\u{20ac}\"}"),
        map(vec![(Value::string("\u{e9}"), Value::string("\u{20ac}"))])
    );
    // A missing object value is null too.
    assert_eq!(
        parse("{\"a\":,\"b\":1}"),
        map(vec![
            (Value::string("a"), Value::Null),
            (Value::string("b"), Value::Long(1))
        ])
    );
    // Duplicate keys: last value, first position.
    assert_eq!(
        json("'{%22a%22:1,%22b%22:2,%22a%22:3}' JSON-> ->JSON"),
        "{\"a\":3,\"b\":2}"
    );
    assert_eq!(
        json("'{%22a%22:{%22x%22:1},%22a%22:{%22y%22:2}}' JSON-> ->JSON"),
        "{\"a\":{\"y\":2}}"
    );
    // Insertion order is preserved.
    assert_eq!(
        json("'{%22z%22:1,%22y%22:[1,2.5,null],%22x%22:{}}' JSON-> ->JSON"),
        "{\"z\":1,\"y\":[1,2.5,null],\"x\":{}}"
    );
}

#[test]
fn parse_depth() {
    const TOO_DEEP: &str = "Document nesting depth (1001) exceeds the maximum allowed (1000, from `StreamReadConstraints.getMaxNestingDepth()`)";
    with_big_stack(|| {
        let deep_lists = format!("{}{}", "[".repeat(1000), "]".repeat(1000));
        assert_eq!(parse(&deep_lists), nested_lists(1000));
        assert_eq!(parse_err(&"[".repeat(1001)), TOO_DEEP);
        assert_eq!(parse_err(&"{\"a\":".repeat(1001)), TOO_DEEP);
        let deep_maps = format!("{}[]{}", "{\"a\":".repeat(999), "}".repeat(999));
        assert_eq!(parse(&deep_maps).type_name(), "MAP");
    });
}

#[test]
fn jsonto_stack_function() {
    assert_eq!(
        top("'[1,2.5,%22a%22,true,null]' JSON->"),
        list(vec![
            Value::Long(1),
            Value::Double(2.5),
            Value::string("a"),
            Value::Boolean(true),
            Value::Null
        ])
    );
    assert_eq!(
        json("'{%22a%22:[1,2.5,null]}' JSON-> ->JSON"),
        "{\"a\":[1,2.5,null]}"
    );
    assert_top("'%22x%22' JSON->", "x");
    assert_top("'null' JSON->", Value::Null);
    assert_fails("1 JSON->", "JSON-> expects a string on top of the stack.");
    assert_fails("JSON->", "Empty stack.");
    assert_fails(
        "'[1] x' JSON->",
        &format!(
            "JSON-> failed to parse JSON (Unrecognized token 'x': was expecting {VALID}\n at [line: 1, column: 6])"
        ),
    );
}

// -----------------------------------------------------------------------------------------
// JSON->: errors
// -----------------------------------------------------------------------------------------

#[test]
fn parse_errors_end_of_input() {
    assert_eq!(
        parse_err(""),
        "No content to map due to end-of-input\n at [line: 1, column: 0]"
    );
    assert_eq!(
        parse_err("  \n "),
        "No content to map due to end-of-input\n at [line: 1, column: 0]"
    );
    assert_eq!(
        parse_err("\"ab"),
        "Unexpected end-of-input: was expecting closing quote for a string value\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("{\"ab"),
        "Unexpected end-of-input in field name\n at [line: 1, column: 5]"
    );
    assert_eq!(
        parse_err("{\"a\\"),
        "Unexpected end-of-input in character escape sequence\n at [line: 1, column: 5]"
    );
    assert_eq!(
        parse_err("\"\\"),
        "Unexpected end-of-input in character escape sequence\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("\"\\u00"),
        "Unexpected end-of-input in character escape sequence\n at [line: 1, column: 6]"
    );
    assert_eq!(
        parse_err("["),
        "Unexpected end-of-input: expected close marker for Array (start marker at [line: 1, column: 1])\n at [line: 1, column: 2]"
    );
    assert_eq!(
        parse_err("[1,2"),
        "Unexpected end-of-input: expected close marker for Array (start marker at [line: 1, column: 1])\n at [line: 1, column: 5]"
    );
    assert_eq!(
        parse_err("[,"),
        "Unexpected end-of-input within/between Array entries\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("[1,"),
        "Unexpected end-of-input within/between Array entries\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("{"),
        "Unexpected end-of-input: expected close marker for Object (start marker at [line: 1, column: 1])\n at [line: 1, column: 2]"
    );
    assert_eq!(
        parse_err("{\"a\":1"),
        "Unexpected end-of-input: expected close marker for Object (start marker at [line: 1, column: 1])\n at [line: 1, column: 7]"
    );
    assert_eq!(
        parse_err("{\"a\""),
        "Unexpected end-of-input within/between Object entries\n at [line: 1, column: 5]"
    );
    assert_eq!(
        parse_err("{\"a\":"),
        "Unexpected end-of-input within/between Object entries\n at [line: 1, column: 6]"
    );
    assert_eq!(
        parse_err("{\"a\":1,"),
        "Unexpected end-of-input within/between Object entries\n at [line: 1, column: 8]"
    );
    assert_eq!(
        parse_err("-"),
        "Unexpected end-of-inputNo digit following minus sign\n at [line: 1, column: 2]"
    );
    assert_eq!(
        parse_err("[-"),
        "Unexpected end-of-inputNo digit following minus sign\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("1e"),
        "Unexpected end-of-inputexpected a digit for number exponent\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("1e+"),
        "Unexpected end-of-inputexpected a digit for number exponent\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("+"),
        "Unexpected end-of-input in a Number value\n at [line: 1, column: 2]"
    );
    assert_eq!(
        parse_err("+I"),
        "Unexpected end-of-input in a Number value\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("-I"),
        "Unexpected end-of-input in a Number value\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("tru"),
        format!("Unrecognized token 'tru': was expecting {VALID}\n at [line: 1, column: 4]")
    );
}

#[test]
fn parse_errors_tokens() {
    let unrecognized = |token: &str, column: usize| {
        format!(
            "Unrecognized token '{token}': was expecting {VALID}\n at [line: 1, column: {column}]"
        )
    };
    assert_eq!(parse_err("[1] x"), unrecognized("x", 6));
    assert_eq!(parse_err("INF"), unrecognized("INF", 4));
    assert_eq!(parse_err("nan"), unrecognized("nan", 4));
    assert_eq!(parse_err("True"), unrecognized("True", 5));
    assert_eq!(parse_err("[truex]"), unrecognized("truex", 7));
    assert_eq!(parse_err("[true1]"), unrecognized("true1", 7));
    assert_eq!(parse_err("[true_]"), unrecognized("true_", 7));
    assert_eq!(parse_err("[true\u{e9}]"), unrecognized("true\u{e9}", 7));
    assert_eq!(parse_err("[truea$]"), unrecognized("truea$", 8));
    assert_eq!(parse_err("[nul]"), unrecognized("nul", 5));
    assert_eq!(parse_err("[nulll]"), unrecognized("nulll", 7));
    assert_eq!(parse_err("[fals]"), unrecognized("fals", 6));
    assert_eq!(parse_err("[falsey]"), unrecognized("falsey", 8));
    assert_eq!(parse_err("NaNx"), unrecognized("NaNx", 5));
    assert_eq!(parse_err("[NaN_]"), unrecognized("NaN_", 6));
    assert_eq!(parse_err("Infinityx"), unrecognized("Infinityx", 10));
    assert_eq!(parse_err("-Infinityx"), unrecognized("-Infinityx", 11));
    assert_eq!(parse_err("[-Infinity1]"), unrecognized("-Infinity1", 12));
    assert_eq!(parse_err("-Inf"), unrecognized("-Inf", 5));
    assert_eq!(parse_err("-In"), unrecognized("-In", 4));
    assert_eq!(parse_err("+In"), unrecognized("+In", 4));
    assert_eq!(parse_err("[-INFx]"), unrecognized("-INFx", 7));
    assert_eq!(parse_err("[+INFx]"), unrecognized("+INFx", 7));
    assert_eq!(parse_err("nullx"), unrecognized("nullx", 6));
    assert_eq!(parse_err("null x"), unrecognized("x", 7));
    assert_eq!(parse_err("{}x"), unrecognized("x", 4));
    assert_eq!(parse_err("\"a\"x"), unrecognized("x", 5));
    assert_eq!(parse_err("x\u{e9}$_1-"), unrecognized("x\u{e9}$_1", 6));
    assert_eq!(parse_err("[\u{e9}x]"), unrecognized("\u{e9}x", 4));
    assert_eq!(parse_err("[1] \u{20ac}"), unrecognized("\u{20ac}", 6));
    assert_eq!(parse_err("[1] \u{a3}"), unrecognized("\u{a3}", 6));
    // Java's "identifier ignorable" control characters are part of the token.
    assert_eq!(parse_err("[nulx\u{1}]"), unrecognized("nulx\u{1}", 7));
    assert_eq!(
        parse_err("[xa\u{7f}\u{85}]"),
        unrecognized("xa\u{7f}\u{85}", 6)
    );
    assert_eq!(parse_err("[x1\u{1b}\u{9}]"), unrecognized("x1\u{1b}", 5));
    // The quoted token is truncated to 256 characters.
    let long = "a".repeat(300);
    assert_eq!(
        parse_err(&long),
        unrecognized(&format!("{}...", "a".repeat(256)), 257)
    );
}

#[test]
fn parse_errors_unexpected_characters() {
    let valid_value = |desc: &str, column: usize| {
        format!(
            "Unexpected character ({desc}): expected a valid value {VALID}\n at [line: 1, column: {column}]"
        )
    };
    assert_eq!(parse_err("{\"a\":}"), valid_value("'}' (code 125)", 7));
    assert_eq!(parse_err("{\"a\":]}"), valid_value("']' (code 93)", 7));
    assert_eq!(parse_err("[1,}"), valid_value("'}' (code 125)", 5));
    assert_eq!(parse_err("[1],"), valid_value("',' (code 44)", 5));
    assert_eq!(parse_err(",1"), valid_value("',' (code 44)", 2));
    assert_eq!(parse_err(":"), valid_value("':' (code 58)", 2));
    assert_eq!(parse_err(".5"), valid_value("'.' (code 46)", 2));
    assert_eq!(parse_err("'a'"), valid_value("''' (code 39)", 2));
    assert_eq!(parse_err("#"), valid_value("'#' (code 35)", 2));
    assert_eq!(
        parse_err("[1] \u{ac}"),
        valid_value("'\u{ac}' (code 172)", 6)
    );
    assert_eq!(
        parse_err("\u{a0}[1]"),
        valid_value("'\u{a0}' (code 160)", 2)
    );
    assert_eq!(
        parse_err("\u{feff}[1]"),
        valid_value("'\u{feff}' (code 65279 / 0xfeff)", 2)
    );
    assert_eq!(
        parse_err("[1] \u{1f600}"),
        valid_value("'\u{1f600}' (code 128512 / 0x1f600)", 6)
    );
    assert_eq!(
        parse_err("\u{85}[1]"),
        valid_value("(CTRL-CHAR, code 133)", 2)
    );
    assert_eq!(parse_err("\u{7f}"), valid_value("(CTRL-CHAR, code 127)", 2));

    assert_eq!(
        parse_err("[1 2]"),
        "Unexpected character ('2' (code 50)): was expecting comma to separate Array entries\n at [line: 1, column: 5]"
    );
    assert_eq!(
        parse_err("[1,\n2,\n 3 x]"),
        "Unexpected character ('x' (code 120)): was expecting comma to separate Array entries\n at [line: 3, column: 5]"
    );
    assert_eq!(
        parse_err("[1,\r\n2 x]"),
        "Unexpected character ('x' (code 120)): was expecting comma to separate Array entries\n at [line: 2, column: 4]"
    );
    assert_eq!(
        parse_err("[1,\r2 x]"),
        "Unexpected character ('x' (code 120)): was expecting comma to separate Array entries\n at [line: 2, column: 4]"
    );
    assert_eq!(
        parse_err("[1a]"),
        "Unexpected character ('a' (code 97)): was expecting comma to separate Array entries\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("[1.5.5]"),
        "Unexpected character ('.' (code 46)): was expecting comma to separate Array entries\n at [line: 1, column: 6]"
    );
    assert_eq!(
        parse_err("[0x1]"),
        "Unexpected character ('x' (code 120)): was expecting comma to separate Array entries\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("[true$a]"),
        "Unexpected character ('$' (code 36)): was expecting comma to separate Array entries\n at [line: 1, column: 7]"
    );
    assert_eq!(
        parse_err("[true-]"),
        "Unexpected character ('-' (code 45)): was expecting comma to separate Array entries\n at [line: 1, column: 7]"
    );
    assert_eq!(
        parse_err("[true false]"),
        "Unexpected character ('f' (code 102)): was expecting comma to separate Array entries\n at [line: 1, column: 8]"
    );
    assert_eq!(
        parse_err("[1\u{7f}]"),
        "Unexpected character ((CTRL-CHAR, code 127)): was expecting comma to separate Array entries\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("{\"a\":1 \"b\":2}"),
        "Unexpected character ('\"' (code 34)): was expecting comma to separate Object entries\n at [line: 1, column: 9]"
    );
    assert_eq!(
        parse_err("{\"a\":1:2}"),
        "Unexpected character (':' (code 58)): was expecting comma to separate Object entries\n at [line: 1, column: 8]"
    );
    assert_eq!(
        parse_err("{\"a\":1,}"),
        "Unexpected character ('}' (code 125)): was expecting double-quote to start field name\n at [line: 1, column: 9]"
    );
    assert_eq!(
        parse_err("{\"a\":,}"),
        "Unexpected character ('}' (code 125)): was expecting double-quote to start field name\n at [line: 1, column: 8]"
    );
    assert_eq!(
        parse_err("{a:1}"),
        "Unexpected character ('a' (code 97)): was expecting double-quote to start field name\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("{1:2}"),
        "Unexpected character ('1' (code 49)): was expecting double-quote to start field name\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("{'a':1}"),
        "Unexpected character (''' (code 39)): was expecting double-quote to start field name\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("{,\"a\":1}"),
        "Unexpected character (',' (code 44)): was expecting double-quote to start field name\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("{\"a\" 1}"),
        "Unexpected character ('1' (code 49)): was expecting a colon to separate field name and value\n at [line: 1, column: 7]"
    );
    assert_eq!(
        parse_err("{\"a\"}"),
        "Unexpected character ('}' (code 125)): was expecting a colon to separate field name and value\n at [line: 1, column: 6]"
    );
}

#[test]
fn parse_errors_numbers() {
    assert_eq!(
        parse_err("01"),
        "Invalid numeric value: Leading zeroes not allowed\n at [line: 1, column: 2]"
    );
    assert_eq!(
        parse_err("-01"),
        "Invalid numeric value: Leading zeroes not allowed\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("[00.5]"),
        "Invalid numeric value: Leading zeroes not allowed\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("1."),
        "Unexpected character ('.' (code 46)) in numeric value: Decimal point not followed by a digit\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("[1.]"),
        "Unexpected character (']' (code 93)) in numeric value: Decimal point not followed by a digit\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("[1e]"),
        "Unexpected character (']' (code 93)) in numeric value: Exponent indicator not followed by a digit\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("[1e+]"),
        "Unexpected character (']' (code 93)) in numeric value: Exponent indicator not followed by a digit\n at [line: 1, column: 5]"
    );
    assert_eq!(
        parse_err("[-]"),
        "Unexpected character (']' (code 93)) in numeric value: expected digit (0-9) to follow minus sign, for valid numeric value\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("- 1"),
        "Unexpected character (' ' (code 32)) in numeric value: expected digit (0-9) to follow minus sign, for valid numeric value\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("-NaN"),
        "Unexpected character ('N' (code 78)) in numeric value: expected digit (0-9) to follow minus sign, for valid numeric value\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("-Ix"),
        "Unexpected character ('x' (code 120)) in numeric value: expected digit (0-9) to follow minus sign, for valid numeric value\n at [line: 1, column: 4]"
    );
    let plus = "Unexpected character ('+' (code 43)) in numeric value: JSON spec does not allow numbers to have plus signs: enable `JsonReadFeature.ALLOW_LEADING_PLUS_SIGN_FOR_NUMBERS` to allow";
    assert_eq!(parse_err("+1"), format!("{plus}\n at [line: 1, column: 3]"));
    assert_eq!(
        parse_err("[+x]"),
        format!("{plus}\n at [line: 1, column: 4]")
    );
    assert_eq!(
        parse_err("+Ix"),
        format!("{plus}\n at [line: 1, column: 4]")
    );
    let root_space = |desc: &str, column: usize| {
        format!(
            "Unexpected character ({desc}): Expected space separating root-level values\n at [line: 1, column: {column}]"
        )
    };
    assert_eq!(parse_err("1.5x"), root_space("'x' (code 120)", 5));
    assert_eq!(parse_err("1]"), root_space("']' (code 93)", 3));
    assert_eq!(parse_err("1,"), root_space("',' (code 44)", 3));
    assert_eq!(parse_err("0x10"), root_space("'x' (code 120)", 3));
    assert_eq!(parse_err("1e5x"), root_space("'x' (code 120)", 5));
    assert_eq!(parse_err("0.5."), root_space("'.' (code 46)", 5));
    assert_eq!(parse_err("1/"), root_space("'/' (code 47)", 3));
    assert_eq!(parse_err("1\u{e9}"), root_space("'\u{e9}' (code 233)", 3));
}

#[test]
fn parse_errors_strings() {
    assert_eq!(
        parse_err("\"\\x\""),
        "Unrecognized character escape 'x' (code 120)\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("\"\\'\""),
        "Unrecognized character escape ''' (code 39)\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("\"\\U00e9\""),
        "Unrecognized character escape 'U' (code 85)\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("\"\\\u{e9}\""),
        "Unrecognized character escape '\u{e9}' (code 233)\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("\"\\u12\""),
        "Unexpected character ('\"' (code 34)): expected a hex-digit for character escape sequence\n at [line: 1, column: 7]"
    );
    assert_eq!(
        parse_err("\"\\uZZZZ\""),
        "Unexpected character ('Z' (code 90)): expected a hex-digit for character escape sequence\n at [line: 1, column: 5]"
    );
    assert_eq!(
        parse_err("\"\\ud800\\uZZZZ\""),
        "Unexpected character ('Z' (code 90)): expected a hex-digit for character escape sequence\n at [line: 1, column: 11]"
    );
    assert_eq!(
        parse_err("{\"a\\u12\""),
        "Unexpected character ('\"' (code 34)): expected a hex-digit for character escape sequence\n at [line: 1, column: 9]"
    );
    assert_eq!(
        parse_err("\"a\tb\""),
        "Illegal unquoted character ((CTRL-CHAR, code 9)): has to be escaped using backslash to be included in string value\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("\"a\nb\""),
        "Illegal unquoted character ((CTRL-CHAR, code 10)): has to be escaped using backslash to be included in string value\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("\"abc\0\""),
        "Illegal unquoted character ((CTRL-CHAR, code 0)): has to be escaped using backslash to be included in string value\n at [line: 1, column: 6]"
    );
    assert_eq!(
        parse_err("{\"a\tb\":1}"),
        "Illegal unquoted character ((CTRL-CHAR, code 9)): has to be escaped using backslash to be included in name\n at [line: 1, column: 5]"
    );
}

#[test]
fn parse_errors_structure() {
    assert_eq!(
        parse_err("[1}"),
        "Unexpected close marker '}': expected ']' (for Array starting at [line: 1, column: 1])\n at [line: 1, column: 4]"
    );
    assert_eq!(
        parse_err("[}"),
        "Unexpected close marker '}': expected ']' (for Array starting at [line: 1, column: 1])\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("{\"a\":[1,2}"),
        "Unexpected close marker '}': expected ']' (for Array starting at [line: 1, column: 6])\n at [line: 1, column: 11]"
    );
    assert_eq!(
        parse_err("{]"),
        "Unexpected close marker ']': expected '}' (for Object starting at [line: 1, column: 1])\n at [line: 1, column: 3]"
    );
    assert_eq!(
        parse_err("{\"a\":1]"),
        "Unexpected close marker ']': expected '}' (for Object starting at [line: 1, column: 1])\n at [line: 1, column: 8]"
    );
    assert_eq!(
        parse_err("[{\"a\":1]"),
        "Unexpected close marker ']': expected '}' (for Object starting at [line: 1, column: 2])\n at [line: 1, column: 9]"
    );
    assert_eq!(
        parse_err("]"),
        "Unexpected close marker ']': expected '}' (for root starting at [line: 1, column: 0])\n at [line: 1, column: 2]"
    );
    assert_eq!(
        parse_err("}"),
        "Unexpected close marker '}': expected ']' (for root starting at [line: 1, column: 0])\n at [line: 1, column: 2]"
    );
    assert_eq!(
        parse_err("[1]]"),
        "Unexpected close marker ']': expected '}' (for root starting at [line: 1, column: 0])\n at [line: 1, column: 5]"
    );
    assert_eq!(
        parse_err("{\"a\":1}}"),
        "Unexpected close marker '}': expected ']' (for root starting at [line: 1, column: 0])\n at [line: 1, column: 9]"
    );
    assert_eq!(
        parse_err("\"a\"]"),
        "Unexpected close marker ']': expected '}' (for root starting at [line: 1, column: 0])\n at [line: 1, column: 5]"
    );
    let trailing = |kind: &str, column: usize| {
        format!(
            "Trailing token (of type {kind}) found after value (bound as `java.lang.Object`): not allowed as per `DeserializationFeature.FAIL_ON_TRAILING_TOKENS`\n at [line: 1, column: {column}]"
        )
    };
    assert_eq!(parse_err("1 2"), trailing("VALUE_NUMBER_INT", 3));
    assert_eq!(parse_err("1 2.5"), trailing("VALUE_NUMBER_FLOAT", 3));
    assert_eq!(parse_err("1 NaN"), trailing("VALUE_NUMBER_FLOAT", 3));
    assert_eq!(parse_err("[1] [2]"), trailing("START_ARRAY", 5));
    assert_eq!(parse_err("[1][2]"), trailing("START_ARRAY", 4));
    assert_eq!(parse_err("[1] {"), trailing("START_OBJECT", 5));
    assert_eq!(parse_err("\"a\" \"b\""), trailing("VALUE_STRING", 5));
    assert_eq!(parse_err("true false"), trailing("VALUE_FALSE", 6));
    assert_eq!(parse_err("false true"), trailing("VALUE_TRUE", 7));
    assert_eq!(parse_err("1 null"), trailing("VALUE_NULL", 3));
    // Numbers and literals are read in full before being reported, strings are not.
    assert_eq!(parse_err("1 \"ab"), trailing("VALUE_STRING", 3));
    assert_eq!(parse_err("[1] \""), trailing("VALUE_STRING", 5));
    assert_eq!(
        parse_err("1 2x"),
        "Unexpected character ('x' (code 120)): Expected space separating root-level values\n at [line: 1, column: 5]"
    );
    assert_eq!(
        parse_err("[1] -x"),
        "Unexpected character ('x' (code 120)) in numeric value: expected digit (0-9) to follow minus sign, for valid numeric value\n at [line: 1, column: 7]"
    );
    assert_eq!(
        parse_err("[1] -"),
        "Unexpected end-of-inputNo digit following minus sign\n at [line: 1, column: 6]"
    );
    assert_eq!(
        parse_err("[1] +"),
        "Unexpected end-of-input in a Number value\n at [line: 1, column: 6]"
    );
    assert_eq!(
        parse_err("[1] truex"),
        format!("Unrecognized token 'truex': was expecting {VALID}\n at [line: 1, column: 10]")
    );
    assert_eq!(
        parse_err("[1] 'x'"),
        format!(
            "Unexpected character (''' (code 39)): expected a valid value {VALID}\n at [line: 1, column: 6]"
        )
    );
}

#[test]
fn parse_errors_whitespace_and_comments() {
    let illegal = |code: u32, column: usize| {
        format!(
            "Illegal character ((CTRL-CHAR, code {code})): only regular white space (\\r, \\n, \\t) is allowed between tokens\n at [line: 1, column: {column}]"
        )
    };
    assert_eq!(parse_err("\u{b}[1]"), illegal(11, 2));
    assert_eq!(parse_err("\u{c}[1]"), illegal(12, 2));
    assert_eq!(parse_err("[1\u{b}]"), illegal(11, 4));
    assert_eq!(parse_err("[\u{1}]"), illegal(1, 3));
    assert_eq!(parse_err("[1]\0"), illegal(0, 5));
    assert_eq!(parse_err("[1] \u{1}"), illegal(1, 6));
    assert_eq!(parse_err("[1,\u{1f}2]"), illegal(31, 5));
    assert_eq!(parse_err("{\u{b}}"), illegal(11, 3));
    assert_eq!(parse_err("{\"a\"\u{1}:1}"), illegal(1, 6));
    assert_eq!(parse_err("{\"a\":\u{b}1}"), illegal(11, 7));
    assert_eq!(parse_err("{\"a\":1\u{b}}"), illegal(11, 8));
    assert_eq!(parse_err("{\"a\":1,\u{b}}"), illegal(11, 9));
    let comment = |column: usize| {
        format!(
            "Unexpected character ('/' (code 47)): maybe a (non-standard) comment? (not recognized as one since Feature 'ALLOW_COMMENTS' not enabled for parser)\n at [line: 1, column: {column}]"
        )
    };
    assert_eq!(parse_err("/* c */ 1"), comment(2));
    assert_eq!(parse_err("1 // c"), comment(4));
    assert_eq!(parse_err("[1 /]"), comment(5));
    assert_eq!(parse_err("[1,/]"), comment(5));
    assert_eq!(parse_err("{\"a\"/:1}"), comment(6));
    assert_eq!(parse_err("\"a\" /"), comment(6));
}

#[test]
fn options_default() {
    let options = JsonOptions::default();
    assert!(!options.strict && !options.pretty);
    assert_eq!(options.max_size, i64::MAX);
    assert_eq!(to_json(&Value::Null, options).unwrap(), "null");
    assert_ne!(JsonError::TooBig, JsonError::TooDeep);
}

#[test]
fn numbers_are_read_as_jackson_reads_them() {
    assert_eq!(number_value("12", false), Value::Long(12));
    assert_eq!(number_value("1.5", true), Value::Double(1.5));
    // Beyond the `long` range Jackson yields a `BigInteger`: the closest value is a `DOUBLE`.
    assert_eq!(
        number_value("9223372036854775808", false),
        Value::Double(9_223_372_036_854_775_808.0)
    );
}

#[test]
fn an_encoder_is_written_up_to_its_corrupt_datapoint() {
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::encoder::{EncodedValue, GtsEncoder};
    use crate::gts::{NO_ELEVATION, NO_LOCATION};

    let mut whole = GtsEncoder::new(0);
    whole.add_value(1, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(5));
    whole.add_value(2, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(6));
    let stream = whole.stream_bytes();
    // The second datapoint is cut short.
    let cut = GtsEncoder::with_stream(&stream[..stream.len() - 1]);
    let mut stack = Stack::default();
    stack.push(Value::Encoder(Rc::new(RefCell::new(cut))));
    stack
        .exec("->JSON")
        .expect("the corrupt datapoint is not an error");
    let json = stack.pop().expect("the JSON is on the stack");
    assert_eq!(
        json,
        Value::from(r#"{"c":"","l":{},"a":{},"la":0,"v":[[1,5]]}"#)
    );
}
