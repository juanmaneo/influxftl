// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the mappers configured with constants. The expected values were checked against
//! Warp 10 3.5.0 (`warp10io/warp10:3.5.0-alpine`).

use super::arithmetic::{Arithmetic, Npdf, Op, ParseDouble, Replace, parse_number};
use super::tests::{Window, doubles, longs, series};
use super::{Aggregated, Aggregator, Num};
use crate::functions::stringformat::Locale;
use crate::tests::{assert_fails, assert_top, fail};
use crate::value::Value;

/// The values of the series of `values` mapped by `mapper`.
fn map_values(values: &str, mapper: &str) -> String {
    format!("[ {} {mapper} 0 0 0 ] MAP 0 GET VALUES", series(values))
}

fn arithmetic(op: Op, operand: Num) -> Arithmetic {
    Arithmetic {
        name: "mapper.x",
        op,
        operand,
    }
}

const ROOT: Locale = Locale::ROOT;

#[test]
fn add_and_mul() {
    // A `LONG` operand truncates a `DOUBLE` value, and the result is a `LONG`.
    assert_top(
        &map_values("2.7 -2.7", "3 mapper.add"),
        Value::list(longs(&[5, 1])),
    );
    assert_top(
        &map_values("9223372036854775807", "1 mapper.add"),
        Value::list(longs(&[i64::MIN])),
    );
    assert_top(
        &map_values("2", "0.5 mapper.add"),
        Value::list(doubles(&[2.5])),
    );
    assert_top(&map_values("2.7", "3 mapper.mul"), Value::list(longs(&[6])));
    assert_top(
        &map_values("3", "1.5 mapper.mul"),
        Value::list(doubles(&[4.5])),
    );
    assert_top(
        &map_values("4611686018427387904", "4 mapper.mul"),
        Value::list(longs(&[0])),
    );
    assert_fails(
        &map_values("'a'", "3 mapper.add"),
        "MAP was given invalid parameters. (mapper.add can only be applied to LONG or DOUBLE values.)",
    );
}

#[test]
fn mod_min_max() {
    assert_top(
        &map_values("-7 7", "3 mapper.mod"),
        Value::list(longs(&[-1, 1])),
    );
    assert_top(
        &map_values("-7.5", "3 mapper.mod"),
        Value::list(doubles(&[-1.5])),
    );
    assert_top(
        &map_values("7.5", "2.5 mapper.mod"),
        Value::list(doubles(&[0.0])),
    );
    assert_eq!(
        Window::of(&[Value::Long(i64::MIN)]).value(&arithmetic(Op::Mod, Num::Long(-1))),
        Value::Long(0)
    );
    assert_eq!(
        Window::of(&[Value::Long(7)])
            .value(&arithmetic(Op::Mod, Num::Double(0.0)))
            .as_f64()
            .map(f64::is_nan),
        Some(true)
    );
    assert_eq!(
        Window::of(&[Value::Double(7.0)])
            .value(&arithmetic(Op::Mod, Num::Long(0)))
            .as_f64()
            .map(f64::is_nan),
        Some(true)
    );
    // An `ArithmeticException`, which `MAP` lets through unwrapped.
    let err = Window::of(&[Value::Long(7)])
        .apply(&arithmetic(Op::Mod, Num::Long(0)))
        .expect_err("the value should fail");
    assert_eq!(
        crate::error::compat::exception_class(&err),
        "java.lang.ArithmeticException"
    );
    let message = fail(&map_values("7", "0 mapper.mod")).to_string();
    assert!(message.contains("(/ by zero)"), "{message}");
    assert!(!message.contains("invalid parameters"), "{message}");
    assert_top(
        &map_values("5", "3.5 mapper.min.x"),
        Value::list(doubles(&[3.5])),
    );
    assert_top(
        &map_values("2.5", "3 mapper.min.x"),
        Value::list(doubles(&[2.5])),
    );
    assert_top(
        &map_values("5 1", "3 mapper.min.x"),
        Value::list(longs(&[3, 1])),
    );
    assert_top(
        &map_values("5 1", "3 mapper.max.x"),
        Value::list(longs(&[5, 3])),
    );
    assert_top(
        &map_values("5", "7.5 mapper.max.x"),
        Value::list(doubles(&[7.5])),
    );

    assert!(
        Window::of(&[Value::Double(f64::NAN)])
            .value(&arithmetic(Op::MaxX, Num::Long(3)))
            .as_f64()
            .expect("the value should succeed")
            .is_nan()
    );
}

#[test]
fn pow_exp_log() {
    assert_top(
        &map_values("4 2", "mapper.sqrt"),
        Value::list(doubles(&[2.0, std::f64::consts::SQRT_2])),
    );
    assert_top(
        &map_values("1.5", "2 mapper.pow"),
        Value::list(doubles(&[2.25])),
    );
    // `Math.pow`: a `NaN` exponent gives `NaN`, even for 1.
    assert!(
        Window::of(&[Value::Long(1)])
            .value(&arithmetic(Op::Pow, Num::Double(f64::NAN)))
            .as_f64()
            .expect("the value should succeed")
            .is_nan()
    );
    assert_top(
        &map_values("3", "2 mapper.exp"),
        Value::list(doubles(&[8.0])),
    );
    assert_top(
        &map_values("0.5", "4.0 mapper.exp"),
        Value::list(doubles(&[2.0])),
    );
    assert_top(
        &map_values("100 8", "10 mapper.log"),
        Value::list(doubles(&[2.0, 8f64.ln() / 10f64.ln()])),
    );
    assert_top(
        &map_values("8.0", "2 mapper.log"),
        Value::list(doubles(&[8f64.ln() / 2f64.ln()])),
    );
}

#[test]
fn snapshots_and_builders() {
    for (script, snapshot) in [
        ("mapper.sqrt", "0.5 mapper.sqrt "),
        // The natural logarithm of the base, which is what the object stores.
        ("10 mapper.log", "2.302585092994046 mapper.log "),
        ("0 mapper.log", "-Infinity mapper.log "),
        ("'fr' mapper.parsedouble", "'mapper.parsedouble' FUNCREF "),
        ("'a b' mapper.replace", "'a%20b' mapper.replace "),
        ("true mapper.replace", "true mapper.replace "),
        ("NaN mapper.replace", "NaN mapper.replace "),
        ("1 2 mapper.npdf", "1.0 2.0 mapper.npdf "),
        ("1 NaN mapper.npdf", "1.0 NaN mapper.npdf "),
        ("2.5 mapper.mod", "2.5 mapper.mod "),
        ("3 mapper.add", "3 mapper.add "),
        ("-0.0 mapper.mul", "-0.0 mapper.mul "),
        ("2 mapper.pow", "2.0 mapper.pow "),
        ("1.0 0.0 / mapper.pow", "Infinity mapper.pow "),
        ("2 mapper.exp", "2.0 mapper.exp "),
        ("7 mapper.min.x", "7 mapper.min.x "),
        ("7.5 mapper.max.x", "7.5 mapper.max.x "),
    ] {
        assert_top(&format!("{script} SNAPSHOT"), snapshot);
        assert_top(&format!("{script} TYPEOF"), "AGGREGATOR");
        let object = crate::tests::top(script);
        let roles = super::aggregator_of(&object)
            .expect("aggregator_of should succeed")
            .roles();
        assert_eq!(roles, super::Roles::MAPPER, "{script}");
    }
    for name in [
        "mapper.add",
        "mapper.mul",
        "mapper.mod",
        "mapper.min.x",
        "mapper.max.x",
        "mapper.pow",
        "mapper.exp",
        "mapper.log",
    ] {
        assert_fails(
            &format!("'a' {name}"),
            &format!("Invalid parameter for {name}"),
        );
    }
    for script in [
        "mapper.add",
        "mapper.replace",
        "mapper.npdf",
        "1 mapper.npdf",
        "mapper.parsedouble",
    ] {
        assert_fails(script, "Empty stack");
    }
    assert_fails("[ ] mapper.replace", "Invalid value type.");
    assert_fails("NULL mapper.replace", "Invalid value type.");
    assert_fails(
        "'a' 1 mapper.npdf",
        "mapper.npdf expects a mean (mu) below the standard deviation.",
    );
    assert_fails(
        "1 0 mapper.npdf",
        "mapper.npdf expects a positive standard deviation (sigma) on top of the stack.",
    );
    assert_fails(
        "1 'a' mapper.npdf",
        "mapper.npdf expects a standard deviation (sigma) on top of the stack.",
    );
    assert_fails(
        "1 mapper.parsedouble",
        "Invalid parameter type for mapper.parsedouble, expects a language tag (a string) on top of the stack.",
    );
    assert_fails(
        &format!("[ {} 'z' mapper.replace 0 10 0 ] BUCKETIZE", series("1")),
        "BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
    );
}

#[test]
fn single_value_windows() {
    let mappers: Vec<Box<dyn Aggregator>> = vec![
        Box::new(arithmetic(Op::Add, Num::Long(1))),
        Box::new(Npdf {
            name: "mapper.x",
            mu: 0.0,
            sigma: 1.0,
        }),
        Box::new(ParseDouble {
            name: "mapper.x",
            locale: ROOT,
        }),
    ];
    for mapper in &mappers {
        assert_eq!(
            Window::of(&[])
                .apply(mapper.as_ref())
                .expect("apply should succeed"),
            Aggregated {
                tick: 0,
                ..Aggregated::none()
            }
        );
        assert_eq!(
            Window::of(&longs(&[1, 2])).error(mapper.as_ref()),
            "mapper.x can only be applied to a single value."
        );
    }
}

#[test]
fn replace_and_npdf() {
    let replace = Replace {
        value: Value::string("z"),
    };
    // No single value check, and the tick of the empty window.
    assert_eq!(
        Window::of(&[])
            .apply(&replace)
            .expect("apply should succeed"),
        Aggregated {
            tick: 100,
            ..Aggregated::none()
        }
    );
    assert_eq!(
        Window::of(&longs(&[1, 2])).value(&replace),
        Value::string("z")
    );
    assert_top(
        "[ NEWGTS 1 48 2 NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 'z' mapper.replace 1 0 0 ] MAP 0 GET 'r' STORE $r VALUES $r LOCATIONS DROP 2 ->LIST ->JSON",
        // The window of tick 2 starts at tick 1, whose location is the first.
        r#"[["z","z"],[47.99999998882413,47.99999998882413]]"#,
    );
    assert_top(
        &format!(
            "[ {} 'z' mapper.replace 1 0 [ 5 ] ] MAP 0 GET VALUES",
            series("1 2")
        ),
        Value::list(vec![Value::string("z")]),
    );
    assert_top(
        &format!(
            "[ {} 'z' mapper.replace 0 0 [ 5 ] ] MAP 0 GET VALUES",
            series("1 2")
        ),
        Value::list(vec![]),
    );
    assert_top(
        &map_values("1", "0 1 mapper.npdf"),
        Value::list(doubles(&[0.241_970_724_519_143_37])),
    );
    assert_fails(
        &map_values("'x'", "0 1 mapper.npdf"),
        "MAP was given invalid parameters. (mapper.npdf can only be applied to LONG or DOUBLE values.)",
    );
}

#[test]
fn parse_double_mapper() {
    assert_top(
        &map_values("'1,234.5' 'abc' '12abc'", "'en' mapper.parsedouble"),
        Value::list(doubles(&[1234.5, 12.0])),
    );
    for (tag, text, expected) in [
        ("de", "1.234,5", 1234.5),
        ("fr", "1.234,5", 1.0),
        ("fr_FR", "1.234,5", 1.234),
        ("xx", "1.234,5", 1.234),
        ("de", "1,5", 1.5),
        ("fr-FR", "1,5", 1.5),
        ("fr_FR", "1,5", 15.0),
        ("de", "1.5", 15.0),
        ("fr", "1.5", 1.0),
        ("fr", "1\u{202F}234,5", 1234.5),
        ("ru", "1\u{A0}234,5", 1234.5),
    ] {
        assert_top(
            &format!(
                "[ NEWGTS 1 NaN NaN NaN '{}' ADDVALUE '{tag}' mapper.parsedouble 0 0 0 ] MAP 0 GET VALUES",
                crate::format::url_encode(text)
            ),
            Value::list(doubles(&[expected])),
        );
    }
    let mapper = ParseDouble {
        name: "mapper.x",
        locale: ROOT,
    };
    assert_eq!(Window::of(&longs(&[42])).value(&mapper), Value::Null);
}

#[test]
fn number_format_parsing() {
    for (text, expected) in [
        ("1,234.5", 1234.5),
        ("12abc", 12.0),
        ("-5", -5.0),
        (".5", 0.5),
        ("5.", 5.0),
        ("1E3", 1000.0),
        ("1e3", 1.0),
        ("1E-2", 0.01),
        ("1E+2", 1.0),
        ("1E", 1.0),
        ("1E-", 1.0),
        ("1,,2", 12.0),
        (",5", 5.0),
        ("1.2.3", 1.2),
        ("1.5,3", 1.5),
        ("12345678901234567890", 1.234_567_890_123_456_7E19),
        ("1E4294967297", 10.0),
        ("1E99999999999999999999", 1.0),
        ("1E-9223372036854775808", 1.0),
        ("1E9223372036854775807", 0.1),
        ("0.1", 0.1),
        ("007", 7.0),
        ("\u{221E}", f64::INFINITY),
        ("-\u{221E}", f64::NEG_INFINITY),
    ] {
        assert_eq!(parse_number(text, ROOT), Some(expected), "{text}");
    }
    let negative_zero = parse_number("-0", ROOT).expect("parse_number should succeed");
    assert!(negative_zero == 0.0 && negative_zero.is_sign_negative());
    assert!(
        parse_number("NaN", ROOT)
            .expect("parse_number should succeed")
            .is_nan()
    );
    for text in [" 5", "+5", "-NaN", "Infinity", "", "-", "abc", ".", "E5"] {
        assert_eq!(parse_number(text, ROOT), None, "{text}");
    }
}
