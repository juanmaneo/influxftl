// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the layout of the tables.

use warpscript::gts::Type;

use super::{
    RESERVED_LABELS, VALUE_BOOLEAN, VALUE_DOUBLE, VALUE_LONG, VALUE_STRING, is_deleted_table,
    type_columns,
};

#[test]
fn every_type_has_its_marker_and_column() {
    assert_eq!(type_columns(Type::Long), Some(("l", VALUE_LONG)));
    assert_eq!(type_columns(Type::Double), Some(("d", VALUE_DOUBLE)));
    assert_eq!(type_columns(Type::Boolean), Some(("b", VALUE_BOOLEAN)));
    assert_eq!(type_columns(Type::String), Some(("s", VALUE_STRING)));
    assert_eq!(type_columns(Type::Undefined), None);
}

#[test]
fn the_columns_of_the_values_are_reserved_labels() {
    for column in [VALUE_LONG, VALUE_DOUBLE, VALUE_BOOLEAN, VALUE_STRING] {
        assert!(RESERVED_LABELS.contains(&column), "{column}");
    }
}

#[test]
fn a_table_deleted_by_influxdb_is_recognised_by_its_name() {
    assert!(is_deleted_table("cpu-20261110T132233"));
    assert!(is_deleted_table("a-b-20261110T132233"));
    assert!(!is_deleted_table("cpu"));
    assert!(!is_deleted_table("cpu-"));
    assert!(!is_deleted_table("cpu-20261110X132233"));
    assert!(!is_deleted_table("cpu-2026111T0132233"));
    assert!(!is_deleted_table("cpu-20261110T13223"));
    assert!(!is_deleted_table("cpu-20261110T1322334"));
    assert!(!is_deleted_table("cpu-2026111aT132233"));
}
