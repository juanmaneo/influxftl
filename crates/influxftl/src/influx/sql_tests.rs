// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the pieces of SQL.

use super::{identifier, literal, nanoseconds, timestamp};

#[test]
fn an_identifier_is_quoted() {
    assert_eq!(identifier("cpu"), "\"cpu\"");
    assert_eq!(identifier("a\"b; DROP"), "\"a\"\"b; DROP\"");
}

#[test]
fn a_literal_is_quoted() {
    assert_eq!(literal("host"), "'host'");
    assert_eq!(literal("it's"), "'it''s'");
    assert_eq!(literal("\\"), "'\\'");
}

#[test]
fn a_timestamp_is_cast_from_nanoseconds() {
    assert_eq!(
        timestamp(-5),
        "arrow_cast(-5, 'Timestamp(Nanosecond, None)')"
    );
    assert_eq!(
        timestamp(i64::MIN),
        "arrow_cast(-9223372036854775808, 'Timestamp(Nanosecond, None)')"
    );
}

#[test]
fn ticks_are_nanoseconds_when_they_fit() {
    assert_eq!(nanoseconds(3), Some(3000));
    assert_eq!(nanoseconds(-3), Some(-3000));
    assert_eq!(nanoseconds(i64::MAX / 1000), Some(i64::MAX / 1000 * 1000));
    assert_eq!(nanoseconds(i64::MAX / 1000 + 1), None);
    assert_eq!(nanoseconds(i64::MIN), None);
}
