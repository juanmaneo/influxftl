// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The tree of the GeoJSON reader, which the oracle tables of `GEO.JSON` reach through scripts
//! only in part: the text that is not JSON, the kinds of value a visitor expects, and the
//! failures of positions and of arrays that are too short.

use serde::de::Unexpected;

use super::*;
use crate::error::compat::exception_class;

#[test]
fn a_visitor_says_what_it_expects() {
    let error =
        <serde::de::value::Error as serde::de::Error>::invalid_type(Unexpected::Unit, &JsonVisitor);
    assert_eq!(
        error.to_string(),
        "invalid type: unit value, expected a JSON value"
    );
}

#[test]
fn every_kind_of_value_is_found_as_its_kind() {
    assert_eq!(Json::Null.found(), Found::Null);
    assert_eq!(Json::Bool(true).found(), Found::True);
    assert_eq!(Json::Bool(false).found(), Found::False);
    assert_eq!(Json::Integer(1).found(), Found::Integer);
    assert_eq!(Json::Float(1.5).found(), Found::Float);
    assert_eq!(Json::String("x".to_owned()).found(), Found::String);
    assert_eq!(Json::Array(Vec::new()).found(), Found::Array);
    assert_eq!(Json::Object(Vec::new()).found(), Found::Object);
}

/// The class of the failure of reading `text`, which must fail.
fn class_of_failure(text: &str) -> &'static str {
    let error = read(text).expect_err("the text is not a valid GeoJSON geometry");
    exception_class(error.root_cause())
}

#[test]
fn a_text_that_is_not_json_fails_as_jackson_does() {
    assert_eq!(
        class_of_failure("{"),
        "com.fasterxml.jackson.core.io.JsonEOFException"
    );
    assert_eq!(
        class_of_failure("[1 2]"),
        "com.fasterxml.jackson.core.JsonParseException"
    );
    assert_eq!(
        class_of_failure("{\"a\" 1}"),
        "com.fasterxml.jackson.core.JsonParseException"
    );
}

#[test]
fn positions_and_rings_too_short_fail_with_an_index_out_of_bounds() {
    let index_out_of_bounds = "java.lang.ArrayIndexOutOfBoundsException";
    assert_eq!(
        class_of_failure(r#"{"type":"Point","coordinates":[]}"#),
        index_out_of_bounds
    );
    assert_eq!(
        class_of_failure(r#"{"type":"Point","coordinates":[1]}"#),
        index_out_of_bounds
    );
    assert_eq!(
        class_of_failure(r#"{"type":"Polygon","coordinates":[]}"#),
        index_out_of_bounds
    );
}

#[test]
fn a_string_where_a_collection_of_geometries_is_expected_is_refused() {
    assert_eq!(
        class_of_failure(r#"{"type":"GeometryCollection","geometries":"abc"}"#),
        "com.fasterxml.jackson.databind.exc.InvalidDefinitionException"
    );
}
