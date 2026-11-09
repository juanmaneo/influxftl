// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the platform routines (arithmetic, number parsing, sizes and indexes) that
//! WarpLib functions inherit from the JDK, worded like Warp 10's because a script reads them.

use super::Error;

/// An arithmetic failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArithmeticError {
    /// A division or remainder by zero of integers.
    #[error("/ by zero")]
    DivisionByZero,
    /// An addition, subtraction, product or negation that overflows 64 bits.
    #[error("long overflow")]
    LongOverflow,
    /// A value that does not fit the 32 bits it must.
    #[error("integer overflow")]
    IntegerOverflow,
}

/// A text that is not the number it must be.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NumberFormatError {
    /// A text without any character.
    #[error("empty String")]
    Empty,
    /// A text that is not a decimal number.
    #[error("For input string: \"{0}\"")]
    InvalidInput(String),
    /// A text that is not a hexadecimal number.
    #[error("For input string: \"{0}\" under radix 16")]
    InvalidHexInput(String),
    /// A negative text where an unsigned number is read.
    #[error("Illegal leading minus sign on unsigned string {0}.")]
    NegativeUnsigned(String),
    /// A decimal built from a NaN or an infinity.
    #[error("Infinite or NaN")]
    NotFinite,
}

/// A size or an index outside of what it must be.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BoundsError {
    /// A negative length for a new array.
    #[error("{0}")]
    NegativeSize(i64),
    /// An index outside of an array.
    #[error("Index {index} out of bounds for length {length}")]
    ArrayIndex {
        /// The index accessed.
        index: i64,
        /// The number of elements.
        length: usize,
    },
    /// An index outside of a list.
    #[error("Index {index} out of bounds for length {length}")]
    ListIndex {
        /// The index accessed.
        index: i64,
        /// The number of elements.
        length: usize,
    },
    /// The end of a sub-list past the end of its list.
    #[error("toIndex = {0}")]
    ListEnd(i32),
    /// An index outside of a string.
    #[error("Index {index} out of bounds for length {length}")]
    StringIndex {
        /// The index accessed.
        index: usize,
        /// The number of characters.
        length: usize,
    },
    /// A range outside of a string.
    #[error("Range [{start}, {end}) out of bounds for length {length}")]
    StringRange {
        /// The first index of the range.
        start: usize,
        /// The end of the range.
        end: usize,
        /// The number of characters.
        length: usize,
    },
}

/// An argument a platform routine refuses.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IllegalArgumentError {
    /// A negative initial capacity.
    #[error("Illegal Capacity: {0}")]
    Capacity(i64),
    /// A time zone offset of a day or more.
    #[error("Millis out of range: {0}")]
    MillisOutOfRange(i32),
    /// A character of a `%xx` escape that is not a hexadecimal digit.
    #[error(
        "URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: \"{digit}\" = {code}"
    )]
    EscapeNotHexadecimal {
        /// The character found.
        digit: char,
        /// Its UTF-16 code unit (the lead one for a supplementary character).
        code: u16,
    },
    /// A `%` ending a URL encoded text.
    #[error("URLDecoder: Incomplete trailing escape (%) pattern")]
    IncompleteEscape,
}

/// A limit of the machine a request exceeds.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResourceError {
    /// An array longer than any array can be.
    #[error("Requested array size exceeds VM limit")]
    ArrayTooLong,
    /// An array which does not fit in memory.
    #[error("Java heap space")]
    OutOfMemory,
}

impl Error {
    /// A division by zero of integers.
    pub fn division_by_zero() -> Self {
        ArithmeticError::DivisionByZero.into()
    }

    /// The failure of reading `index` in an array of `length`.
    pub fn index_out_of_bounds(index: i64, length: usize) -> Self {
        BoundsError::ArrayIndex { index, length }.into()
    }
}
