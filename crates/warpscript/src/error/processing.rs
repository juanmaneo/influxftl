// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Failures of the Processing functions (`P…`, `PGRAPHICS`, `PIMAGE`, `PSHAPE`, `PFONT`).
//!
//! Each message is Warp 10's wording, copied as the script reads it. The separator before the
//! message differs from one function to another (`PcolorMode: invalid mode`, `PtextMode invalid
//! mode`), so a refusal carries its message whole and Rust code tells refusals apart by function.

use std::collections::TryReserveError;

/// A failure of a Processing function.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProcessingError {
    /// A function given a number of parameters it does not take.
    #[error("Invalid number of parameters, expected one of {expected:?} but found {found}")]
    Arity {
        /// The counts the function accepts, sorted.
        expected: &'static [usize],
        /// The number of parameters found.
        found: usize,
    },
    /// A function refused an argument: its name followed by Warp 10's message, which carries
    /// its own separator (`: invalid mode…`, ` expects…`).
    #[error("{function}{message}")]
    Refused {
        /// The WarpScript function that refused.
        function: &'static str,
        /// The message, with its separator.
        message: &'static str,
    },
    /// A graphics or image that would have more pixels than the stack allows.
    #[error(
        "{function} only allows {allowed} with a total number of pixels less than {limit} requested size was {width}x{height} ({pixels})."
    )]
    TooManyPixels {
        /// The WarpScript function that refused.
        function: &'static str,
        /// The kinds of object the function accepts, in Warp 10's wording.
        allowed: &'static str,
        /// The limit on the number of pixels.
        limit: i64,
        /// The requested width.
        width: i64,
        /// The requested height.
        height: i64,
        /// The requested number of pixels.
        pixels: i64,
    },
    /// An image with a side that is zero or negative.
    #[error("Width ({width}) and height ({height}) cannot be <= 0")]
    EmptyImage {
        /// The requested width.
        width: i64,
        /// The requested height.
        height: i64,
    },
    /// A `popStyle` with no `pushStyle` before it.
    #[error("Too many popStyle() without enough pushStyle()")]
    StyleUnderflow,
    /// A `popMatrix` with no `pushMatrix` before it.
    #[error("missing a pushMatrix() to go with that popMatrix()")]
    MatrixUnderflow,
    /// A `pushMatrix` beyond the depth of the matrix stack.
    #[error("pushMatrix() cannot use push more than 32 times")]
    MatrixOverflow,
    /// A stroke weight below zero, which Java2D refuses when it is set.
    #[error("negative width")]
    NegativeWidth,
    /// A colour maximum that is not a finite positive number.
    #[error("a colour maximum must be a finite positive number.")]
    NonPositiveMaximum,
    /// A pixel buffer that the memory cannot hold.
    #[error("not enough memory for {pixels} pixels")]
    OutOfMemory {
        /// The number of pixels requested.
        pixels: usize,
        /// The failed reservation.
        #[source]
        source: TryReserveError,
    },
}

impl ProcessingError {
    /// `true` for the failures Processing or the JDK throws rather than Warp 10 (see
    /// [`Error::is_runtime`](super::Error::is_runtime)).
    pub fn is_runtime(&self) -> bool {
        match self {
            ProcessingError::Arity { .. }
            | ProcessingError::Refused { .. }
            | ProcessingError::TooManyPixels { .. }
            | ProcessingError::NonPositiveMaximum => false,
            ProcessingError::EmptyImage { .. }
            | ProcessingError::StyleUnderflow
            | ProcessingError::MatrixUnderflow
            | ProcessingError::MatrixOverflow
            | ProcessingError::NegativeWidth
            | ProcessingError::OutOfMemory { .. } => true,
        }
    }
}
