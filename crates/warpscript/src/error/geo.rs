// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the geo functions, worded like Warp 10's.

/// A failure of a geo function.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GeoError {
    /// A hexadecimal HHCode of more than 16 digits.
    #[error("Hexadecimal HHCode string of must be of length <= 16")]
    HexHhcodeTooLong,
    /// A byte array HHCode of more than 8 bytes.
    #[error("Byte array HHCode must be of length <= 8")]
    BytesHhcodeTooLong,
    /// A GeoHash with a character that is not in the base 32 alphabet.
    #[error("Invalid GeoHash '{geohash}' at index {index}")]
    InvalidGeoHash {
        /// The GeoHash.
        geohash: String,
        /// The UTF-16 index of the character.
        index: usize,
    },
    /// An HHCode that is neither a long, a string nor a byte array.
    #[error("HHCode must be represented as a long, a string or a byte array.")]
    HhcodeType,
}
