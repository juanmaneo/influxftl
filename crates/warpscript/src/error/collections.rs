// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the collection functions (`GET`, `->MAP`…), worded like Warp 10's.

/// A failure of a collection function.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CollectionError {
    /// A map expected where something else was found.
    #[error("Invalid type, expected a map.")]
    MapExpected,
    /// `GET` on a list with a key that is not a long.
    #[error("Getting on LIST requires a LONG.")]
    ListKeyNotLong,
    /// `GET` on a string with a key that is not a long.
    #[error("Getting on STRING requires a LONG.")]
    StringKeyNotLong,
    /// `GET` on a byte array with a key that is not a long.
    #[error("Getting on BYTES requires a LONG.")]
    BytesKeyNotLong,
    /// `GET` on something that is not a collection.
    #[error("Invalid OBJECT to GET on.")]
    InvalidGetTarget,
    /// A path into nested lists that leads nowhere.
    #[error("Tried to get an element at a nested path that does not exist in the input list.")]
    NestedPathMissing,
    /// A map built from an odd number of elements.
    #[error("Odd number of elements, cannot create map.")]
    OddMapElements,
    /// A map built with a key given twice.
    #[error("Duplicate map key '{0}'")]
    DuplicateMapKey(String),
}
