// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

use crate::bigdecimal::{BigDecimal, RoundingMode, bad_group, parse_bigint};
use crate::error::fault::NumberFormatError;

#[test]
fn divide_rounded_keeps_the_scale_of_the_dividend() {
    // `new BigDecimal(a).divide(new BigDecimal(b), RoundingMode.HALF_UP)` on JDK 27.
    for (a, b, expected) in [
        ("12.34", "-1E+2", "-0.12"),
        ("7.5", "1E+1", "0.8"),
        ("-7.5", "-3", "2.5"),
    ] {
        let a = BigDecimal::parse(a).expect("parse should succeed");
        let b = BigDecimal::parse(b).expect("parse should succeed");
        let quotient = a
            .divide_rounded(&b, RoundingMode::HalfUp)
            .expect("divide_rounded should succeed");
        assert_eq!(quotient.to_string(), expected);
    }
}

#[test]
fn a_text_that_is_not_a_number_fails_on_the_group_java_reads() {
    // `new BigInteger(text, radix)` reads 9 decimal, 7 hexadecimal or 30 binary digits at a time,
    // the first group being the short one, after the leading zeroes.
    let ones_and_a_two = format!("{}2", "1".repeat(31));
    let group_of_ones = format!("{}2", "1".repeat(29));
    for (text, radix, group) in [
        ("x", 10, "x"),
        ("abcdefghij", 10, "a"),
        ("123456789x", 10, "23456789x"),
        ("1234567890123x", 10, "67890123x"),
        ("000123456789x", 10, "23456789x"),
        ("-12x", 10, "12x"),
        ("+12x", 10, "12x"),
        ("1_2", 10, "1_2"),
        ("g", 16, "g"),
        ("123456789abcdefg", 16, "abcdefg"),
        (ones_and_a_two.as_str(), 2, group_of_ones.as_str()),
    ] {
        assert_eq!(
            parse_bigint(text, radix),
            Err(NumberFormatError::InvalidRadixInput {
                input: group.to_owned(),
                radix,
            }),
            "{text} in radix {radix}"
        );
    }
}

#[test]
fn the_group_of_a_text_that_has_no_bad_digit_is_the_text() {
    assert_eq!(bad_group("00123", 10), "123");
    assert_eq!(bad_group("", 2), "");
}

#[test]
fn a_number_is_read_in_its_radix() {
    for (text, radix, expected) in [
        (
            "123456789012345678901234567890",
            10,
            "123456789012345678901234567890",
        ),
        ("-0042", 10, "-42"),
        ("+7", 10, "7"),
        ("ff", 16, "255"),
        ("-101", 2, "-5"),
        ("0", 10, "0"),
    ] {
        assert_eq!(
            parse_bigint(text, radix).expect("a number").to_string(),
            expected,
            "{text} in radix {radix}"
        );
    }
}
