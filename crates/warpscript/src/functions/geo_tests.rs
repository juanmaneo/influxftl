// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Oracle-checked tests of the HHCode, geocell, `HAVERSINE` and `COPYGEO` functions.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const GEO: &str = r"NEWGTS 'g' RENAME 1 48.0 2.0 10 1 ADDVALUE 3 NaN NaN 30 3 ADDVALUE 2 -10.5 170.25 NaN 2 ADDVALUE 3 1.0 1.0 NaN 4 ADDVALUE 5 5.0 5.0 5 5 ADDVALUE";
const TO: &str = r"NEWGTS 't' RENAME 4 NaN NaN NaN 4 ADDVALUE 3 NaN NaN NaN 3 ADDVALUE 1 NaN NaN NaN 1 ADDVALUE 3 NaN NaN NaN 33 ADDVALUE 0 NaN NaN NaN 0 ADDVALUE";
const TOGEO: &str = r"NEWGTS 't' RENAME 3 NaN NaN 7 3 ADDVALUE 1 0.0 0.0 NaN 1 ADDVALUE";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@TOGEO", TOGEO)
        .replace("@GEO", GEO)
        .replace("@TO", TO)
}

/// The stack left by `script`, top first, as the JSON array of the Warp 10 exec endpoint,
/// or the error message.
fn stack_json(script: &str) -> Result<String, String> {
    let mut stack = test_stack();
    stack
        .exec_checked(&format!("{script} DEPTH ->LIST REVERSE ->JSON"))
        .map_err(|err| err.to_string())?;
    match stack.pop().unwrap() {
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
fn tohhcode() {
    check(&[
        (r"48.0 2.0 ->HHCODE", Ok(r#"["e021347021347021"]"#)),
        (r"-90.0 -180.0 ->HHCODE", Ok(r#"["0000000000000000"]"#)),
        (r"90.0 180.0 ->HHCODE", Ok(r#"["eaaaaaaaaaaaaaaa"]"#)),
        (r"0.0 0.0 ->HHCODE", Ok(r#"["c000000000000000"]"#)),
        (
            r"45.123456 -73.98765 ->HHCODE",
            Ok(r#"["b0451ca4ff19e9e8"]"#),
        ),
        (r"100.0 10.0 ->HHCODE", Ok(r#"["aa17a17a17a17a17"]"#)),
        (r"NaN NaN ->HHCODE", Ok(r#"["0000000000000000"]"#)),
        (r"NaN 1.0 ->HHCODE", Ok(r#"["4000451400451400"]"#)),
        (r"1 2 ->HHCODE", Ok(r#"["c0033cf0033cf003"]"#)),
        (
            r"'x' 1.0 ->HHCODE",
            Err(r"->HHCODE expects a latitude and a longitude on the stack."),
        ),
        (r"1.0 ->HHCODE", Err(r"Empty stack.")),
        (r"48.0 2.0 ->HHCODELONG", Ok(r"[-2296496678784176095]")),
        (r"-90.0 -180.0 ->HHCODELONG", Ok(r"[0]")),
        (r"90.0 180.0 ->HHCODELONG", Ok(r"[-1537228672809129302]")),
        (r"0.0 0.0 ->HHCODELONG", Ok(r"[-4611686018427387904]")),
        (
            r"45.123456 -73.98765 ->HHCODELONG",
            Ok(r"[-5745154254661096984]"),
        ),
        (r"100.0 10.0 ->HHCODELONG", Ok(r"[-6190301617042916841]")),
        (r"NaN NaN ->HHCODELONG", Ok(r"[0]")),
        (r"NaN 1.0 ->HHCODELONG", Ok(r"[4611761970633577472]")),
        (r"1 2 ->HHCODELONG", Ok(r"[-4610774591953113085]")),
        (
            r"'x' 1.0 ->HHCODELONG",
            Err(r"->HHCODELONG expects a latitude and a longitude on the stack."),
        ),
        (r"1.0 ->HHCODELONG", Err(r"Empty stack.")),
        (r"48.0 2.0 ->GTSHHCODE", Ok(r#"["e021347021347021"]"#)),
        (r"-90.0 -180.0 ->GTSHHCODE", Ok(r#"["0000000000000000"]"#)),
        (r"90.0 180.0 ->GTSHHCODE", Ok(r#"["eaaaaaaaaaaaaaaa"]"#)),
        (r"0.0 0.0 ->GTSHHCODE", Ok(r#"["c000000000000000"]"#)),
        (
            r"45.123456 -73.98765 ->GTSHHCODE",
            Ok(r#"["b0451ca4ff19e9e8"]"#),
        ),
        (r"100.0 10.0 ->GTSHHCODE", Ok(r#"["aa17a17a17a17a17"]"#)),
        (r"NaN NaN ->GTSHHCODE", Ok(r#"["0145014501450145"]"#)),
        (
            r"NaN 1.0 ->GTSHHCODE",
            Err(r"->GTSHHCODE expects latitude and longitude to both be NaN or both be not NaN"),
        ),
        (r"1 2 ->GTSHHCODE", Ok(r#"["c0033cf0033cf003"]"#)),
        (
            r"'x' 1.0 ->GTSHHCODE",
            Err(r"->GTSHHCODE expects a latitude and a longitude on the stack."),
        ),
        (r"1.0 ->GTSHHCODE", Err(r"Empty stack.")),
        (r"48.0 2.0 ->GTSHHCODELONG", Ok(r"[-2296496678784176095]")),
        (r"-90.0 -180.0 ->GTSHHCODELONG", Ok(r"[0]")),
        (r"90.0 180.0 ->GTSHHCODELONG", Ok(r"[-1537228672809129302]")),
        (r"0.0 0.0 ->GTSHHCODELONG", Ok(r"[-4611686018427387904]")),
        (
            r"45.123456 -73.98765 ->GTSHHCODELONG",
            Ok(r"[-5745154254661096984]"),
        ),
        (r"100.0 10.0 ->GTSHHCODELONG", Ok(r"[-6190301617042916841]")),
        (r"NaN NaN ->GTSHHCODELONG", Ok(r"[91480763316633925]")),
        (
            r"NaN 1.0 ->GTSHHCODELONG",
            Err(
                r"->GTSHHCODELONG expects latitude and longitude to both be NaN or both be not NaN",
            ),
        ),
        (r"1 2 ->GTSHHCODELONG", Ok(r"[-4610774591953113085]")),
        (
            r"'x' 1.0 ->GTSHHCODELONG",
            Err(r"->GTSHHCODELONG expects a latitude and a longitude on the stack."),
        ),
        (r"1.0 ->GTSHHCODELONG", Err(r"Empty stack.")),
    ]);
}

#[test]
fn hhcodeto() {
    check(&[
        (
            r"'b9ad1ee0c5a9e7a5' HHCODE->",
            Ok(r"[-62.706147925928235,77.51007763668895]"),
        ),
        (r"'b9ad' HHCODE->", Ok(r"[-63.28125,77.34375]")),
        (r"'b9a' HHCODE->", Ok(r"[-67.5,75.9375]")),
        (r"'' HHCODE->", Ok(r"[-180.0,-90.0]")),
        (r"'0' HHCODE->", Ok(r"[-180.0,-90.0]")),
        (
            r"'ffffffffffffffff' HHCODE->",
            Ok(r"[179.99999991618097,89.99999995809048]"),
        ),
        (
            r"'b9ad1ee0c5a9e7a5f' HHCODE->",
            Err(
                r"HHCODE-> was given unexpected arguments. (Hexadecimal HHCode string of must be of length <= 16)",
            ),
        ),
        (
            r"'zz' HHCODE->",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"'-1' HHCODE->",
            Err(r"Illegal leading minus sign on unsigned string -100000000000000."),
        ),
        (r"'+100' HHCODE->", Ok(r"[-157.5,-90.0]")),
        (
            r"'b9ad1ee0c5a9e7a5' HEX-> HHCODE->",
            Ok(r"[-62.706147925928235,77.51007763668895]"),
        ),
        (r"'b9ad' HEX-> HHCODE->", Ok(r"[-63.28125,77.34375]")),
        (r"'' HEX-> HHCODE->", Ok(r"[-180.0,-90.0]")),
        (
            r"'b9ad1ee0c5a9e7a5ff' HEX-> HHCODE->",
            Err(
                r"HHCODE-> was given unexpected arguments. (Byte array HHCode must be of length <= 8)",
            ),
        ),
        (
            r"-5064967386357852251 HHCODE->",
            Ok(r"[-57.041928665712476,76.46250067278743]"),
        ),
        (r"0 HHCODE->", Ok(r"[-180.0,-90.0]")),
        (
            r"1.5 HHCODE->",
            Err(
                r"HHCODE-> was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"T HHCODE->",
            Err(
                r"HHCODE-> was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"'b9ad1ee0c5a9e7a5' GTSHHCODE->",
            Ok(r"[-62.706147925928235,77.51007763668895]"),
        ),
        (r"'b9ad' GTSHHCODE->", Ok(r"[-63.28125,77.34375]")),
        (r"'b9a' GTSHHCODE->", Ok(r"[-67.5,75.9375]")),
        (r"'' GTSHHCODE->", Ok(r"[-180.0,-90.0]")),
        (r"'0' GTSHHCODE->", Ok(r"[-180.0,-90.0]")),
        (
            r"'ffffffffffffffff' GTSHHCODE->",
            Ok(r"[179.99999991618097,89.99999995809048]"),
        ),
        (
            r"'b9ad1ee0c5a9e7a5f' GTSHHCODE->",
            Err(
                r"GTSHHCODE-> was given unexpected arguments. (Hexadecimal HHCode string of must be of length <= 16)",
            ),
        ),
        (
            r"'zz' GTSHHCODE->",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"'-1' GTSHHCODE->",
            Err(r"Illegal leading minus sign on unsigned string -100000000000000."),
        ),
        (r"'+100' GTSHHCODE->", Ok(r"[-157.5,-90.0]")),
        (
            r"'b9ad1ee0c5a9e7a5' HEX-> GTSHHCODE->",
            Ok(r"[-62.706147925928235,77.51007763668895]"),
        ),
        (r"'b9ad' HEX-> GTSHHCODE->", Ok(r"[-63.28125,77.34375]")),
        (r"'' HEX-> GTSHHCODE->", Ok(r"[-180.0,-90.0]")),
        (
            r"'b9ad1ee0c5a9e7a5ff' HEX-> GTSHHCODE->",
            Err(
                r"GTSHHCODE-> was given unexpected arguments. (Byte array HHCode must be of length <= 8)",
            ),
        ),
        (
            r"-5064967386357852251 GTSHHCODE->",
            Ok(r"[-57.041928665712476,76.46250067278743]"),
        ),
        (r"0 GTSHHCODE->", Ok(r"[-180.0,-90.0]")),
        (
            r"1.5 GTSHHCODE->",
            Err(
                r"GTSHHCODE-> was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"T GTSHHCODE->",
            Err(
                r"GTSHHCODE-> was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"48.0 2.0 ->GTSHHCODELONG GTSHHCODE->",
            Ok(r"[1.9999999646097422,47.99999998882413]"),
        ),
        (r"NaN NaN ->GTSHHCODELONG GTSHHCODE->", Ok(r"[NaN,NaN]")),
        (
            r"NaN NaN ->GTSHHCODELONG HHCODE->",
            Ok(r"[-141.88235295005143,-90.0]"),
        ),
    ]);
}

#[test]
fn hhcodefunc() {
    check(&[
        (
            r"'b9ad1ee0c5a9e7a5' HHCODE.BBOX",
            Ok(r"[-62.706147925928235,77.51007763668895,-62.706147925928235,77.51007763668895]"),
        ),
        (
            r"'b9ad' HHCODE.BBOX",
            Ok(r"[-61.87500008381903,78.04687495809048,-63.28125,77.34375]"),
        ),
        (
            r"'b9a' HHCODE.BBOX",
            Ok(r"[-61.87500008381903,78.74999995809048,-67.5,75.9375]"),
        ),
        (
            r"'' HHCODE.BBOX",
            Ok(r"[179.99999991618097,89.99999995809048,-180.0,-90.0]"),
        ),
        (
            r"'b9ad' HEX-> HHCODE.BBOX",
            Ok(r"[-61.87500008381903,78.04687495809048,-63.28125,77.34375]"),
        ),
        (
            r"-5064967386357852251 4 HHCODE.BBOX",
            Ok(r"[-45.00000008381903,78.74999995809048,-67.5,67.5]"),
        ),
        (
            r"-5064967386357852251 0 HHCODE.BBOX",
            Ok(r"[179.99999991618097,89.99999995809048,-180.0,-90.0]"),
        ),
        (
            r"-5064967386357852251 32 HHCODE.BBOX",
            Ok(r"[-57.041928665712476,76.46250067278743,-57.041928665712476,76.46250067278743]"),
        ),
        (
            r"'b9ad' 33 HHCODE.BBOX",
            Err(r"HHCODE.BBOX expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (
            r"'b9ad' -1 HHCODE.BBOX",
            Err(r"HHCODE.BBOX expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (
            r"'b9ad' 7.9 HHCODE.BBOX",
            Ok(r"[-61.87500008381903,78.74999995809048,-64.6875,77.34375]"),
        ),
        (
            r"-5064967386357852251 HHCODE.BBOX",
            Err(r"HHCODE.BBOX expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (
            r"'zz' HHCODE.BBOX",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"T HHCODE.BBOX",
            Err(
                r"HHCODE.BBOX was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"'ffffffffffffffff' HHCODE.BBOX",
            Ok(r"[179.99999991618097,89.99999995809048,179.99999991618097,89.99999995809048]"),
        ),
        (
            r"'0000000000000000' HHCODE.BBOX",
            Ok(r"[-180.0,-90.0,-180.0,-90.0]"),
        ),
        (r"HHCODE.BBOX", Err(r"Empty stack.")),
        (
            r"'b9ad1ee0c5a9e7a5' HHCODE.CENTER",
            Ok(r"[-62.706147925928235,77.51007763668895]"),
        ),
        (r"'b9ad' HHCODE.CENTER", Ok(r"[-62.578125,77.6953125]")),
        (r"'b9a' HHCODE.CENTER", Ok(r"[-64.6875,77.34375]")),
        (r"'' HHCODE.CENTER", Ok(r"[0.0,0.0]")),
        (
            r"'b9ad' HEX-> HHCODE.CENTER",
            Ok(r"[-62.578125,77.6953125]"),
        ),
        (
            r"-5064967386357852251 4 HHCODE.CENTER",
            Ok(r"[-56.25,73.125]"),
        ),
        (r"-5064967386357852251 0 HHCODE.CENTER", Ok(r"[0.0,0.0]")),
        (
            r"-5064967386357852251 32 HHCODE.CENTER",
            Ok(r"[-57.041928665712476,76.46250067278743]"),
        ),
        (
            r"'b9ad' 33 HHCODE.CENTER",
            Err(
                r"HHCODE.CENTER expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (
            r"'b9ad' -1 HHCODE.CENTER",
            Err(
                r"HHCODE.CENTER expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (r"'b9ad' 7.9 HHCODE.CENTER", Ok(r"[-63.28125,78.046875]")),
        (
            r"-5064967386357852251 HHCODE.CENTER",
            Err(
                r"HHCODE.CENTER expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (
            r"'zz' HHCODE.CENTER",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"T HHCODE.CENTER",
            Err(
                r"HHCODE.CENTER was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"'ffffffffffffffff' HHCODE.CENTER",
            Ok(r"[179.99999991618097,89.99999995809048]"),
        ),
        (r"'0000000000000000' HHCODE.CENTER", Ok(r"[-180.0,-90.0]")),
        (r"HHCODE.CENTER", Err(r"Empty stack.")),
        (
            r"'b9ad1ee0c5a9e7a5' HHCODE.NORTH",
            Ok(r#"["b9ad1ee0c5a9e7a7"]"#),
        ),
        (r"'b9ad' HHCODE.NORTH", Ok(r#"["b9af"]"#)),
        (r"'b9a' HHCODE.NORTH", Ok(r#"["bb0"]"#)),
        (r"'' HHCODE.NORTH", Ok(r#"[""]"#)),
        (
            r"'b9ad' HEX-> HHCODE.NORTH",
            Ok(r#"["\u00B9\u00AF\u0000\u0000\u0000\u0000\u0000\u0000"]"#),
        ),
        (
            r"-5064967386357852251 4 HHCODE.NORTH",
            Ok(r"[-4920852198281996379]"),
        ),
        (
            r"-5064967386357852251 0 HHCODE.NORTH",
            Ok(r"[5989563573189819663]"),
        ),
        (
            r"-5064967386357852251 32 HHCODE.NORTH",
            Ok(r"[-5064967386357852249]"),
        ),
        (
            r"'b9ad' 33 HHCODE.NORTH",
            Err(r"HHCODE.NORTH expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (
            r"'b9ad' -1 HHCODE.NORTH",
            Err(r"HHCODE.NORTH expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (r"'b9ad' 7.9 HHCODE.NORTH", Ok(r#"["bb0"]"#)),
        (
            r"-5064967386357852251 HHCODE.NORTH",
            Err(r"HHCODE.NORTH expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (
            r"'zz' HHCODE.NORTH",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"T HHCODE.NORTH",
            Err(
                r"HHCODE.NORTH was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"'ffffffffffffffff' HHCODE.NORTH",
            Ok(r#"["bfffffffffffffff"]"#),
        ),
        (
            r"'0000000000000000' HHCODE.NORTH",
            Ok(r#"["0000000000000002"]"#),
        ),
        (r"HHCODE.NORTH", Err(r"Empty stack.")),
        (
            r"'b9ad1ee0c5a9e7a5' HHCODE.SOUTH",
            Ok(r#"["b9ad1ee0c5a9e78f"]"#),
        ),
        (r"'b9ad' HHCODE.SOUTH", Ok(r#"["b9a7"]"#)),
        (r"'b9a' HHCODE.SOUTH", Ok(r#"["b98"]"#)),
        (r"'' HHCODE.SOUTH", Ok(r#"[""]"#)),
        (
            r"'b9ad' HEX-> HHCODE.SOUTH",
            Ok(r#"["\u00B9\u00A7\u0000\u0000\u0000\u0000\u0000\u0000"]"#),
        ),
        (
            r"-5064967386357852251 4 HHCODE.SOUTH",
            Ok(r"[-5497312950585419867]"),
        ),
        (
            r"-5064967386357852251 0 HHCODE.SOUTH",
            Ok(r"[5989563573189819663]"),
        ),
        (
            r"-5064967386357852251 32 HHCODE.SOUTH",
            Ok(r"[-5064967386357852273]"),
        ),
        (
            r"'b9ad' 33 HHCODE.SOUTH",
            Err(r"HHCODE.SOUTH expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (
            r"'b9ad' -1 HHCODE.SOUTH",
            Err(r"HHCODE.SOUTH expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (r"'b9ad' 7.9 HHCODE.SOUTH", Ok(r#"["b9a"]"#)),
        (
            r"-5064967386357852251 HHCODE.SOUTH",
            Err(r"HHCODE.SOUTH expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (
            r"'zz' HHCODE.SOUTH",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"T HHCODE.SOUTH",
            Err(
                r"HHCODE.SOUTH was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"'ffffffffffffffff' HHCODE.SOUTH",
            Ok(r#"["fffffffffffffffd"]"#),
        ),
        (
            r"'0000000000000000' HHCODE.SOUTH",
            Ok(r#"["4000000000000000"]"#),
        ),
        (r"HHCODE.SOUTH", Err(r"Empty stack.")),
        (
            r"'b9ad1ee0c5a9e7a5' HHCODE.EAST",
            Ok(r#"["b9ad1ee0c5a9e7b0"]"#),
        ),
        (r"'b9ad' HHCODE.EAST", Ok(r#"["b9b8"]"#)),
        (r"'b9a' HHCODE.EAST", Ok(r#"["b9b"]"#)),
        (r"'' HHCODE.EAST", Ok(r#"[""]"#)),
        (
            r"'b9ad' HEX-> HHCODE.EAST",
            Ok(r#"["\u00B9\u00B8\u0000\u0000\u0000\u0000\u0000\u0000"]"#),
        ),
        (
            r"-5064967386357852251 4 HHCODE.EAST",
            Ok(r"[-4848794604244068443]"),
        ),
        (
            r"-5064967386357852251 0 HHCODE.EAST",
            Ok(r"[-5064967386357852251]"),
        ),
        (
            r"-5064967386357852251 32 HHCODE.EAST",
            Ok(r"[-5064967386357852240]"),
        ),
        (
            r"'b9ad' 33 HHCODE.EAST",
            Err(r"HHCODE.EAST expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (
            r"'b9ad' -1 HHCODE.EAST",
            Err(r"HHCODE.EAST expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (r"'b9ad' 7.9 HHCODE.EAST", Ok(r#"["b9b"]"#)),
        (
            r"-5064967386357852251 HHCODE.EAST",
            Err(r"HHCODE.EAST expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (
            r"'zz' HHCODE.EAST",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"T HHCODE.EAST",
            Err(
                r"HHCODE.EAST was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"'ffffffffffffffff' HHCODE.EAST",
            Ok(r#"["aaaaaaaaaaaaaaaa"]"#),
        ),
        (
            r"'0000000000000000' HHCODE.EAST",
            Ok(r#"["0000000000000001"]"#),
        ),
        (r"HHCODE.EAST", Err(r"Empty stack.")),
        (
            r"'b9ad1ee0c5a9e7a5' HHCODE.WEST",
            Ok(r#"["b9ad1ee0c5a9e7a4"]"#),
        ),
        (r"'b9ad' HHCODE.WEST", Ok(r#"["b9ac"]"#)),
        (r"'b9a' HHCODE.WEST", Ok(r#"["b8f"]"#)),
        (r"'' HHCODE.WEST", Ok(r#"[""]"#)),
        (
            r"'b9ad' HEX-> HHCODE.WEST",
            Ok(r#"["\u00B9\u00AC\u0000\u0000\u0000\u0000\u0000\u0000"]"#),
        ),
        (
            r"-5064967386357852251 4 HHCODE.WEST",
            Ok(r"[-5137024980395780187]"),
        ),
        (
            r"-5064967386357852251 0 HHCODE.WEST",
            Ok(r"[-5064967386357852251]"),
        ),
        (
            r"-5064967386357852251 32 HHCODE.WEST",
            Ok(r"[-5064967386357852252]"),
        ),
        (
            r"'b9ad' 33 HHCODE.WEST",
            Err(r"HHCODE.WEST expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (
            r"'b9ad' -1 HHCODE.WEST",
            Err(r"HHCODE.WEST expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (r"'b9ad' 7.9 HHCODE.WEST", Ok(r#"["b9a"]"#)),
        (
            r"-5064967386357852251 HHCODE.WEST",
            Err(r"HHCODE.WEST expects a resolution which is an long between 0 and 32, inclusive."),
        ),
        (
            r"'zz' HHCODE.WEST",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"T HHCODE.WEST",
            Err(
                r"HHCODE.WEST was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"'ffffffffffffffff' HHCODE.WEST",
            Ok(r#"["fffffffffffffffe"]"#),
        ),
        (
            r"'0000000000000000' HHCODE.WEST",
            Ok(r#"["5555555555555555"]"#),
        ),
        (r"HHCODE.WEST", Err(r"Empty stack.")),
        (
            r"'b9ad1ee0c5a9e7a5' HHCODE.NORTH.EAST",
            Ok(r#"["b9ad1ee0c5a9e7b2"]"#),
        ),
        (r"'b9ad' HHCODE.NORTH.EAST", Ok(r#"["b9ba"]"#)),
        (r"'b9a' HHCODE.NORTH.EAST", Ok(r#"["bb1"]"#)),
        (r"'' HHCODE.NORTH.EAST", Ok(r#"[""]"#)),
        (
            r"'b9ad' HEX-> HHCODE.NORTH.EAST",
            Ok(r#"["\u00B9\u00BA\u0000\u0000\u0000\u0000\u0000\u0000"]"#),
        ),
        (
            r"-5064967386357852251 4 HHCODE.NORTH.EAST",
            Ok(r"[-4704679416168212571]"),
        ),
        (
            r"-5064967386357852251 0 HHCODE.NORTH.EAST",
            Ok(r"[5989563573189819663]"),
        ),
        (
            r"-5064967386357852251 32 HHCODE.NORTH.EAST",
            Ok(r"[-5064967386357852238]"),
        ),
        (
            r"'b9ad' 33 HHCODE.NORTH.EAST",
            Err(
                r"HHCODE.NORTH.EAST expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (
            r"'b9ad' -1 HHCODE.NORTH.EAST",
            Err(
                r"HHCODE.NORTH.EAST expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (r"'b9ad' 7.9 HHCODE.NORTH.EAST", Ok(r#"["bb1"]"#)),
        (
            r"-5064967386357852251 HHCODE.NORTH.EAST",
            Err(
                r"HHCODE.NORTH.EAST expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (
            r"'zz' HHCODE.NORTH.EAST",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"T HHCODE.NORTH.EAST",
            Err(
                r"HHCODE.NORTH.EAST was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"'ffffffffffffffff' HHCODE.NORTH.EAST",
            Ok(r#"["eaaaaaaaaaaaaaaa"]"#),
        ),
        (
            r"'0000000000000000' HHCODE.NORTH.EAST",
            Ok(r#"["0000000000000003"]"#),
        ),
        (r"HHCODE.NORTH.EAST", Err(r"Empty stack.")),
        (
            r"'b9ad1ee0c5a9e7a5' HHCODE.NORTH.WEST",
            Ok(r#"["b9ad1ee0c5a9e7a6"]"#),
        ),
        (r"'b9ad' HHCODE.NORTH.WEST", Ok(r#"["b9ae"]"#)),
        (r"'b9a' HHCODE.NORTH.WEST", Ok(r#"["ba5"]"#)),
        (r"'' HHCODE.NORTH.WEST", Ok(r#"[""]"#)),
        (
            r"'b9ad' HEX-> HHCODE.NORTH.WEST",
            Ok(r#"["\u00B9\u00AE\u0000\u0000\u0000\u0000\u0000\u0000"]"#),
        ),
        (
            r"-5064967386357852251 4 HHCODE.NORTH.WEST",
            Ok(r"[-4992909792319924315]"),
        ),
        (
            r"-5064967386357852251 0 HHCODE.NORTH.WEST",
            Ok(r"[5989563573189819663]"),
        ),
        (
            r"-5064967386357852251 32 HHCODE.NORTH.WEST",
            Ok(r"[-5064967386357852250]"),
        ),
        (
            r"'b9ad' 33 HHCODE.NORTH.WEST",
            Err(
                r"HHCODE.NORTH.WEST expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (
            r"'b9ad' -1 HHCODE.NORTH.WEST",
            Err(
                r"HHCODE.NORTH.WEST expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (r"'b9ad' 7.9 HHCODE.NORTH.WEST", Ok(r#"["bb0"]"#)),
        (
            r"-5064967386357852251 HHCODE.NORTH.WEST",
            Err(
                r"HHCODE.NORTH.WEST expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (
            r"'zz' HHCODE.NORTH.WEST",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"T HHCODE.NORTH.WEST",
            Err(
                r"HHCODE.NORTH.WEST was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"'ffffffffffffffff' HHCODE.NORTH.WEST",
            Ok(r#"["bffffffffffffffe"]"#),
        ),
        (
            r"'0000000000000000' HHCODE.NORTH.WEST",
            Ok(r#"["5555555555555557"]"#),
        ),
        (r"HHCODE.NORTH.WEST", Err(r"Empty stack.")),
        (
            r"'b9ad1ee0c5a9e7a5' HHCODE.SOUTH.EAST",
            Ok(r#"["b9ad1ee0c5a9e79a"]"#),
        ),
        (r"'b9ad' HHCODE.SOUTH.EAST", Ok(r#"["b9b2"]"#)),
        (r"'b9a' HHCODE.SOUTH.EAST", Ok(r#"["b99"]"#)),
        (r"'' HHCODE.SOUTH.EAST", Ok(r#"[""]"#)),
        (
            r"'b9ad' HEX-> HHCODE.SOUTH.EAST",
            Ok(r#"["\u00B9\u00B2\u0000\u0000\u0000\u0000\u0000\u0000"]"#),
        ),
        (
            r"-5064967386357852251 4 HHCODE.SOUTH.EAST",
            Ok(r"[-5281140168471636059]"),
        ),
        (
            r"-5064967386357852251 0 HHCODE.SOUTH.EAST",
            Ok(r"[5989563573189819663]"),
        ),
        (
            r"-5064967386357852251 32 HHCODE.SOUTH.EAST",
            Ok(r"[-5064967386357852262]"),
        ),
        (
            r"'b9ad' 33 HHCODE.SOUTH.EAST",
            Err(
                r"HHCODE.SOUTH.EAST expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (
            r"'b9ad' -1 HHCODE.SOUTH.EAST",
            Err(
                r"HHCODE.SOUTH.EAST expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (r"'b9ad' 7.9 HHCODE.SOUTH.EAST", Ok(r#"["b9b"]"#)),
        (
            r"-5064967386357852251 HHCODE.SOUTH.EAST",
            Err(
                r"HHCODE.SOUTH.EAST expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (
            r"'zz' HHCODE.SOUTH.EAST",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"T HHCODE.SOUTH.EAST",
            Err(
                r"HHCODE.SOUTH.EAST was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"'ffffffffffffffff' HHCODE.SOUTH.EAST",
            Ok(r#"["aaaaaaaaaaaaaaa8"]"#),
        ),
        (
            r"'0000000000000000' HHCODE.SOUTH.EAST",
            Ok(r#"["4000000000000001"]"#),
        ),
        (r"HHCODE.SOUTH.EAST", Err(r"Empty stack.")),
        (
            r"'b9ad1ee0c5a9e7a5' HHCODE.SOUTH.WEST",
            Ok(r#"["b9ad1ee0c5a9e78e"]"#),
        ),
        (r"'b9ad' HHCODE.SOUTH.WEST", Ok(r#"["b9a6"]"#)),
        (r"'b9a' HHCODE.SOUTH.WEST", Ok(r#"["b8d"]"#)),
        (r"'' HHCODE.SOUTH.WEST", Ok(r#"[""]"#)),
        (
            r"'b9ad' HEX-> HHCODE.SOUTH.WEST",
            Ok(r#"["\u00B9\u00A6\u0000\u0000\u0000\u0000\u0000\u0000"]"#),
        ),
        (
            r"-5064967386357852251 4 HHCODE.SOUTH.WEST",
            Ok(r"[-5569370544623347803]"),
        ),
        (
            r"-5064967386357852251 0 HHCODE.SOUTH.WEST",
            Ok(r"[5989563573189819663]"),
        ),
        (
            r"-5064967386357852251 32 HHCODE.SOUTH.WEST",
            Ok(r"[-5064967386357852274]"),
        ),
        (
            r"'b9ad' 33 HHCODE.SOUTH.WEST",
            Err(
                r"HHCODE.SOUTH.WEST expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (
            r"'b9ad' -1 HHCODE.SOUTH.WEST",
            Err(
                r"HHCODE.SOUTH.WEST expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (r"'b9ad' 7.9 HHCODE.SOUTH.WEST", Ok(r#"["b9a"]"#)),
        (
            r"-5064967386357852251 HHCODE.SOUTH.WEST",
            Err(
                r"HHCODE.SOUTH.WEST expects a resolution which is an long between 0 and 32, inclusive.",
            ),
        ),
        (
            r"'zz' HHCODE.SOUTH.WEST",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"T HHCODE.SOUTH.WEST",
            Err(
                r"HHCODE.SOUTH.WEST was given unexpected arguments. (HHCode must be represented as a long, a string or a byte array.)",
            ),
        ),
        (
            r"'ffffffffffffffff' HHCODE.SOUTH.WEST",
            Ok(r#"["fffffffffffffffc"]"#),
        ),
        (
            r"'0000000000000000' HHCODE.SOUTH.WEST",
            Ok(r#"["1555555555555555"]"#),
        ),
        (r"HHCODE.SOUTH.WEST", Err(r"Empty stack.")),
    ]);
}

#[test]
fn geocells() {
    check(&[
        (r"'b9ad' ->GEOCELL", Ok(r"[5447895397676613632]")),
        (r"'b9' ->GEOCELL", Ok(r"[3139008940277235712]")),
        (r"'b9ad1ee0c5a9e7' ->GEOCELL", Ok(r"[-1469631508043358608]")),
        (
            r"'b9ad1ee0c5a9e7a5' ->GEOCELL",
            Err(
                r"->GEOCELL expects the geocell resolution to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"'' ->GEOCELL",
            Err(
                r"->GEOCELL expects the geocell resolution to be even and between 2 and 30, inclusive.",
            ),
        ),
        (r"'b9ad' HEX-> ->GEOCELL", Ok(r"[5447895397676613632]")),
        (r"'b9' HEX-> ->GEOCELL", Ok(r"[3139008940277235712]")),
        (
            r"1 ->GEOCELL",
            Err(r"->GEOCELL expects a STRING or a BYTES."),
        ),
        (
            r"'zz' ->GEOCELL",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"'b9ad1ee0c5a9e7a5f' ->GEOCELL",
            Err(
                r"->GEOCELL expects a STRING or a BYTES. (Hexadecimal HHCode string of must be of length <= 16)",
            ),
        ),
        (r"'b9ad' ->GEOCELL GEOCELL->", Ok(r#"["b9ad"]"#)),
        (
            r"'b9ad1ee0c5a9e7' ->GEOCELL GEOCELL->",
            Ok(r#"["b9ad1ee0c5a9e7"]"#),
        ),
        (r"0 GEOCELL->", Ok(r#"[""]"#)),
        (r"1 GEOCELL->", Ok(r#"[""]"#)),
        (r"4611686018427387904 GEOCELL->", Ok(r#"["0000"]"#)),
        (r"-1 GEOCELL->", Ok(r#"["fffffffffffffff"]"#)),
        (r"'x' GEOCELL->", Err(r"GEOCELL-> expects a LONG geocell.")),
        (r"GEOCELL->", Err(r"Empty stack.")),
        (r"->GEOCELL", Err(r"Empty stack.")),
        (r"HHCODE->", Err(r"Empty stack.")),
        (r"1.0 ->HHCODE", Err(r"Empty stack.")),
        (r"->HHCODE", Err(r"Empty stack.")),
        (r"4 HHCODE.NORTH", Err(r"Empty stack.")),
        (r"HHCODE.NORTH", Err(r"Empty stack.")),
        (r"9223372036854775807 GEOCELL->", Ok(r#"["fffffff"]"#)),
        (r"-8070450532247928832 GEOCELL->", Ok(r#"["000000000"]"#)),
    ]);
}

#[test]
fn haversine() {
    check(&[
        (r"48.0 2.0 40.7 -74.0 HAVERSINE", Ok(r"[5849234.166352454]")),
        (r"0 0 0 0 HAVERSINE", Ok(r"[0.0]")),
        (r"0 0 0 180 HAVERSINE", Ok(r"[2.001508679602057E7]")),
        (r"90 0 -90 0 HAVERSINE", Ok(r"[2.001508679602057E7]")),
        (
            r"48.85 2.35 51.5 -0.12 HAVERSINE",
            Ok(r"[343127.87782353547]"),
        ),
        (
            r"1 2 3 'x' HAVERSINE",
            Err(r"HAVERSINE expects two sets of lat/lon coordinates."),
        ),
        (r"1 2 3 HAVERSINE", Err(r"Empty stack.")),
        (r"NaN 0 0 0 HAVERSINE", Ok(r"[NaN]")),
        (r"HAVERSINE", Err(r"Empty stack.")),
        (r"1 HAVERSINE", Err(r"Empty stack.")),
        (r"1 2 HAVERSINE", Err(r"Empty stack.")),
    ]);
}

#[test]
fn copygeo() {
    check(&[
        (
            r"@TO @GEO COPYGEO",
            Ok(
                r#"[{"c":"t","l":{},"a":{},"la":0,"v":[[0,0],[1,47.99999998882413,1.9999999646097422,10,1],[3,30,33],[3,3],[4,4]]}]"#,
            ),
        ),
        (
            r"[ @TO @TOGEO ] @GEO COPYGEO",
            Ok(
                r#"[[{"c":"t","l":{},"a":{},"la":0,"v":[[0,0],[1,47.99999998882413,1.9999999646097422,10,1],[3,30,33],[3,3],[4,4]]},{"c":"t","l":{},"a":{},"la":0,"v":[[1,47.99999998882413,1.9999999646097422,10,1],[3,30,3]]}]]"#,
            ),
        ),
        (
            r"@TOGEO @GEO COPYGEO",
            Ok(
                r#"[{"c":"t","l":{},"a":{},"la":0,"v":[[1,47.99999998882413,1.9999999646097422,10,1],[3,30,3]]}]"#,
            ),
        ),
        (
            r"@GEO @TO COPYGEO",
            Ok(
                r#"[{"c":"g","l":{},"a":{},"la":0,"v":[[1,1],[2,-10.500000002793968,170.24999999441206,2],[3,4],[3,30,3],[5,4.999999995343387,4.999999953433871,5,5]]}]"#,
            ),
        ),
        (
            r"@GEO 'g' STORE $g $g COPYGEO",
            Ok(
                r#"[{"c":"g","l":{},"a":{},"la":0,"v":[[1,47.99999998882413,1.9999999646097422,10,1],[2,-10.500000002793968,170.24999999441206,2],[3,30,4],[3,30,3],[5,4.999999995343387,4.999999953433871,5,5]]}]"#,
            ),
        ),
        (
            r"@TO 1 COPYGEO",
            Err(r"COPYGEO expects a Geo Time Series on top of the stack."),
        ),
        (
            r"1 @GEO COPYGEO",
            Err(r"COPYGEO can only operate on Geo Time Series instances."),
        ),
        (
            r"[ 1 ] @GEO COPYGEO",
            Err(r"COPYGEO can only operate on Geo Time Series instances."),
        ),
        (
            r"NEWGTS @GEO COPYGEO",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@TO NEWGTS COPYGEO",
            Ok(r#"[{"c":"t","l":{},"a":{},"la":0,"v":[[0,0],[1,1],[3,33],[3,3],[4,4]]}]"#),
        ),
        (
            r"@TO 'a' STORE $a @GEO COPYGEO DROP $a",
            Ok(
                r#"[{"c":"t","l":{},"a":{},"la":0,"v":[[0,0],[1,47.99999998882413,1.9999999646097422,10,1],[3,30,33],[3,3],[4,4]]}]"#,
            ),
        ),
        (
            r"@TO NEWGTS 3 NaN NaN NaN 9 ADDVALUE COPYGEO",
            Ok(r#"[{"c":"t","l":{},"a":{},"la":0,"v":[[0,0],[1,1],[3,33],[3,3],[4,4]]}]"#),
        ),
        (r"COPYGEO", Err(r"Empty stack.")),
        (r"@GEO COPYGEO", Err(r"Empty stack.")),
    ]);
}
