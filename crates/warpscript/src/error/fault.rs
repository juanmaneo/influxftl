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
    /// A decimal whose scale is below what 32 bits hold.
    #[error("Underflow")]
    Underflow,
    /// A decimal whose scale is above what 32 bits hold, or a number too large for its target.
    #[error("Overflow")]
    Overflow,
    /// A rounding that needs a rounding mode.
    #[error("Rounding necessary")]
    RoundingNecessary,
    /// A decimal operation whose operands are out of its domain.
    #[error("Invalid operation")]
    InvalidOperation,
    /// A quotient with no terminating decimal expansion.
    #[error("Non-terminating decimal expansion; no exact representable decimal result.")]
    NonTerminatingExpansion,
    /// A division of zero by zero.
    #[error("Division undefined")]
    DivisionUndefined,
    /// A division of a decimal by zero.
    #[error("Division by zero")]
    DecimalDivisionByZero,
    /// A bit index below zero.
    #[error("Negative bit address")]
    NegativeBitAddress,
    /// A modulus that is zero or negative.
    #[error("BigInteger: modulus not positive")]
    ModulusNotPositive,
    /// A value with no inverse for the modulus.
    #[error("BigInteger not invertible.")]
    NotInvertible,
    /// The search of a prime from a negative value.
    #[error("start < 0: {0}")]
    NegativeStart(num_bigint::BigInt),
    /// An integer too large for 32 bits.
    #[error("BigInteger out of int range")]
    BigIntegerOutOfIntRange,
    /// A bit length below two.
    #[error("bitLength < 2")]
    BitLengthTooSmall,
}

/// What JDK 11 adds to the text of a `NumberFormatException` when the radix is not 10.
fn radix_suffix(radix: u32) -> String {
    if radix == 10 {
        String::new()
    } else {
        format!(" under radix {radix}")
    }
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
    /// A text that is not a number in `radix` (JDK 11 tells the radix when it is not 10).
    #[error("For input string: \"{input}\"{}", radix_suffix(*.radix))]
    InvalidRadixInput {
        /// The text read.
        input: String,
        /// The radix it should be written in.
        radix: u32,
    },
    /// A negative text where an unsigned number is read.
    #[error("Illegal leading minus sign on unsigned string {0}.")]
    NegativeUnsigned(String),
    /// A decimal built from a NaN or an infinity.
    #[error("Infinite or NaN")]
    NotFinite,
    /// A decimal text with an empty exponent.
    #[error("NumberFormatException")]
    Bare,
    /// A decimal text with two decimal points.
    #[error("Character array contains more than one decimal point.")]
    MultipleDecimalPoints,
    /// A decimal text with a character it cannot hold.
    #[error(
        "Character {0} is neither a decimal digit number, decimal point, nor \"e\" notation exponential mark."
    )]
    InvalidDecimalCharacter(char),
    /// A decimal text without any digit.
    #[error("No digits found.")]
    NoDigits,
    /// A decimal exponent with a character that is not a digit.
    #[error("Not a digit.")]
    NotADigit,
    /// A decimal exponent too large to be read.
    #[error("Too many nonzero exponent digits.")]
    ExponentTooLong,
    /// A decimal whose scale does not fit 32 bits.
    #[error("Scale out of range.")]
    ScaleOutOfRange,
    /// An integer without any byte or digit.
    #[error("Zero length BigInteger")]
    ZeroLengthBigInteger,
    /// An integer text with a sign that is not leading.
    #[error("Illegal embedded sign character")]
    EmbeddedSign,
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
    /// A range that ends past the end of an array.
    #[error("Array index out of range: {0}")]
    ArrayRange(usize),
    /// A bit index below zero.
    #[error("bitIndex < 0: {0}")]
    NegativeBitIndex(i64),
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
    /// A slice outside of a string.
    #[error("begin {begin}, end {end}, length {length}")]
    StringSlice {
        /// The first index of the slice.
        begin: usize,
        /// The end of the slice.
        end: usize,
        /// The number of characters.
        length: usize,
    },
    /// A slice from the beginning of a string that ends before it.
    #[error("begin 0, end {end}, length {length}")]
    StringEnd {
        /// The end of the slice.
        end: i64,
        /// The number of characters.
        length: usize,
    },
    /// A range outside of a string.
    #[error("Range [{start}, {end}) out of bounds for length {length}")]
    StringRange {
        /// The first index of the range.
        start: i64,
        /// The end of the range.
        end: i64,
        /// The number of characters.
        length: i64,
    },
}

/// An argument a platform routine refuses.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IllegalArgumentError {
    /// A negative initial capacity.
    #[error("Illegal Capacity: {0}")]
    Capacity(i64),
    /// A negative initial size for a buffer.
    #[error("Negative initial size: {0}")]
    NegativeInitialSize(i64),
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
    /// A number that is not a Unicode code point.
    #[error("Not a valid Unicode code point: 0x{0:X}")]
    InvalidCodePoint(u32),
    /// A `%` ending a URL encoded text.
    #[error("URLDecoder: Incomplete trailing escape (%) pattern")]
    IncompleteEscape,
    /// A Fourier transform of no value.
    #[error("n must be greater than 0")]
    TransformLength,
}

/// A limit of the machine a request exceeds.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResourceError {
    /// Structures nested so deep that the stack of the machine overflows.
    #[error("StackOverflowError")]
    StackOverflow,
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
