// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! How Warp 10 concepts are laid out in InfluxDB 3 (`README.md` §2).
//!
//! A class is a table, its labels are tags and each datapoint is a row: the time of the tick in
//! nanoseconds, the type of the value (`value_type`: `l`, `d`, `b` or `s`), the value in the
//! column of its type (`value_l`, `value_d`, `value_b`, `value_s`), the location as a HHCode
//! (`hhcode`) and the elevation in millimetres (`elevation`). The columns of the other types
//! are left alone, so that writing a datapoint again with another type replaces it. The
//! attributes of the series, which Warp 10 can change at any time, are rows of a table of their
//! own, the latest row of a series being the current one.

use warpscript::gts::Type;

/// The table of the attributes of the series.
pub const META_TABLE: &str = "_influxftl_series";
/// The tag of the class in the table of the attributes.
pub const META_CLASS: &str = "class";
/// The tag of the labels (a JSON object) in the table of the attributes.
pub const META_LABELS: &str = "labels";
/// The field of the attributes (a JSON object).
pub const META_ATTRIBUTES: &str = "attributes";
/// The field that tells a series was deleted.
pub const META_DELETED: &str = "deleted";

/// The column of the type of the value.
pub const VALUE_TYPE: &str = "value_type";
/// The column of the location.
pub const HHCODE: &str = "hhcode";
/// The column of the elevation.
pub const ELEVATION: &str = "elevation";
/// The columns of the values.
pub const VALUE_LONG: &str = "value_l";
/// See [`VALUE_LONG`].
pub const VALUE_DOUBLE: &str = "value_d";
/// See [`VALUE_LONG`].
pub const VALUE_BOOLEAN: &str = "value_b";
/// See [`VALUE_LONG`].
pub const VALUE_STRING: &str = "value_s";

/// The names a label cannot have, because they are the columns of a datapoint.
pub const RESERVED_LABELS: [&str; 8] = [
    "time",
    VALUE_TYPE,
    VALUE_LONG,
    VALUE_DOUBLE,
    VALUE_BOOLEAN,
    VALUE_STRING,
    HHCODE,
    ELEVATION,
];

/// The marker of the type of a value in the column [`VALUE_TYPE`], and the column of the value.
pub fn type_columns(value_type: Type) -> Option<(&'static str, &'static str)> {
    match value_type {
        Type::Undefined => None,
        Type::Long => Some(("l", VALUE_LONG)),
        Type::Double => Some(("d", VALUE_DOUBLE)),
        Type::Boolean => Some(("b", VALUE_BOOLEAN)),
        Type::String => Some(("s", VALUE_STRING)),
    }
}

/// Whether a table has the name InfluxDB 3 gives a table it deleted (`name-20261110T132233`),
/// which it keeps listing until the data is purged.
pub fn is_deleted_table(name: &str) -> bool {
    let Some((_, suffix)) = name.rsplit_once('-') else {
        return false;
    };
    let bytes = suffix.as_bytes();
    bytes.len() == 15
        && bytes[8] == b'T'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 8 || byte.is_ascii_digit())
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod schema_tests;
