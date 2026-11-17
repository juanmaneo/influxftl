// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the geometry readers and writers (`GEO.WKT`, `->WKB`, `GEO.JSON`…), worded
//! like the JTS Topology Suite 1.13 and the GeoJSON binding Warp 10 bundles: a script reads
//! them in the cause chain of the failure of the function.

use std::num::ParseFloatError;

/// Why a word of a WKT or GeoJSON text is not a number.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NumberError {
    /// A word that is not one of the decimal forms `Double.parseDouble` reads: a letter other
    /// than the exponent, a sign inside the number, or no digit at all.
    #[error("not a decimal number")]
    NotDecimal,
    /// A decimal form that the float parser refuses (`1e`, `1.2.3`).
    #[error(transparent)]
    Parse(#[from] ParseFloatError),
}

/// A failure of a geometry reader, writer or predicate.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GeometryError {
    /// A WKT geometry type that is not one of the 8 `WKTReader` knows (`ParseException`).
    #[error("Unknown geometry type: {name} (line {line})")]
    UnknownType {
        /// The word found where a geometry type is expected.
        name: String,
        /// The line of the text the reader was at.
        line: usize,
    },
    /// A WKT token other than the one the grammar expects (`ParseException`).
    #[error("Expected {expected} but found {found} (line {line})")]
    Expected {
        /// What the grammar expects (`number`, `EMPTY or (`…).
        expected: &'static str,
        /// The token found, quoted, or `End-of-Stream`.
        found: String,
        /// The line of the text the reader was at.
        line: usize,
    },
    /// A WKT word that is not a number where a coordinate is expected (`ParseException`).
    #[error("Invalid number: {text} (line {line})")]
    InvalidNumber {
        /// The word.
        text: String,
        /// The line of the text the reader was at.
        line: usize,
        /// Why the word is not a number.
        #[source]
        source: NumberError,
    },
    /// A WKB geometry type that is not one of the 7 `WKBReader` knows (`ParseException`).
    #[error("Unknown WKB type {0}")]
    UnknownWkbType(u8),
    /// A member of a WKB multi-geometry which is not of the type of the others
    /// (`ParseException`).
    #[error("Invalid geometry type encountered in {0}")]
    WkbMemberType(&'static str),
    /// A line string of one point (`IllegalArgumentException`).
    #[error("Invalid number of points in LineString (found 1 - must be 0 or >= 2)")]
    LineStringTooShort,
    /// A linear ring that is not closed (`IllegalArgumentException`).
    #[error("Points of LinearRing do not form a closed linestring")]
    RingNotClosed,
    /// A linear ring of 1 to 3 points (`IllegalArgumentException`).
    #[error("Invalid number of points in LinearRing (found {0} - must be 0 or >= 4)")]
    RingTooShort(usize),
    /// A polygon without shell but with holes (`IllegalArgumentException`).
    #[error("shell is empty but holes are not")]
    HolesWithoutShell,
    /// A geometry collection of a geometry that could not be read (`IllegalArgumentException`).
    #[error("geometries must not contain null elements")]
    NullElement,
    /// A topological predicate (`covers`, `intersects`) given a `GeometryCollection`
    /// (`IllegalArgumentException`).
    #[error("This method does not support GeometryCollection arguments")]
    CollectionArgument,
    /// An empty point written as WKB (`IllegalArgumentException`).
    #[error("Empty Points cannot be represented in WKB")]
    EmptyPointWkb,
    /// A value that is neither a `STRING` nor `BYTES` where a geometry is expected
    /// (`WarpScriptException`, the one failure of `->WKT` and its kind that is not an
    /// "invalid input").
    #[error("Unknown geometry format.")]
    UnknownFormat,
    /// A `null` geometry given to the WKB writer, which has no case for it
    /// (`AssertionFailedException`).
    #[error("Should never reach here: Unknown Geometry type")]
    UnknownGeometryType,
    /// A GeoJSON feature (which the reader does not convert), a `null` where a geometry is
    /// expected, or a linear ring (which the writer does not convert)
    /// (`UnsupportedOperationException`).
    #[error("UnsupportedOperationException")]
    Unsupported,
}

impl GeometryError {
    /// `true` for the failures Warp 10 raises as runtime exceptions (the `IllegalArgumentException`
    /// of the JTS constructors and predicates): the functions that wrap script failures let them
    /// through unwrapped.
    pub fn is_runtime(&self) -> bool {
        matches!(
            self,
            GeometryError::LineStringTooShort
                | GeometryError::RingNotClosed
                | GeometryError::RingTooShort(_)
                | GeometryError::HolesWithoutShell
                | GeometryError::NullElement
                | GeometryError::CollectionArgument
                | GeometryError::EmptyPointWkb
                | GeometryError::UnknownGeometryType
                | GeometryError::Unsupported
        )
    }
}
