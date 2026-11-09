// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of `STRINGFORMAT`, worded like the messages of the `IllegalFormatException`
//! family of the JDK, which Warp 10 reports as they are.

use super::compat;
use crate::value::ValueKind;

/// A failure of a format specifier.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FormatError {
    /// A flag given twice.
    #[error("Flags = '{0}'")]
    DuplicateFlag(char),
    /// A flag that the conversion does not accept.
    #[error("Conversion = {conversion}, Flags = {flag}")]
    FlagMismatch {
        /// The conversion.
        conversion: char,
        /// The flag.
        flag: char,
    },
    /// Flags that cannot be combined.
    #[error("Flags = '{0}'")]
    IllegalFlags(String),
    /// A flag that needs a width, without one. The message is the text of the specifier.
    #[error("{0}")]
    MissingWidth(String),
    /// A precision given to a conversion that has none.
    #[error("{0}")]
    IllegalPrecision(usize),
    /// A width given to a conversion that has none.
    #[error("{0}")]
    IllegalWidth(usize),
    /// A date/time conversion suffix that does not exist.
    #[error("Conversion = 't{0}'")]
    UnknownDateTimeConversion(char),
    /// A conversion that does not exist.
    #[error("Conversion = '{0}'")]
    UnknownConversion(char),
    /// A `%0$s`.
    #[error("Illegal format argument index = 0")]
    ZeroArgumentIndex,
    /// A specifier with no argument left. The message is the text of the specifier.
    #[error("Format specifier '{0}'")]
    MissingArgument(String),
    /// A date/time conversion, which this port does not support.
    #[error("Date/time conversions (%t) are not supported.")]
    DateTimeUnsupported,
    /// An argument of a kind the conversion cannot format.
    #[error("{conversion} != {}", compat::value_class(*found))]
    ConversionMismatch {
        /// The conversion.
        conversion: char,
        /// The kind of the argument.
        found: ValueKind,
    },
}
