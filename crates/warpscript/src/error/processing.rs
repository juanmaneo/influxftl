// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Failures of the Processing functions (`P…`, `PGRAPHICS`, `PIMAGE`, `PSHAPE`, `PFONT`).
//!
//! Each message is Warp 10's wording, copied as the script reads it. The separator before the
//! message differs from one function to another (`PcolorMode: invalid mode`, `PtextMode invalid
//! mode`), so a refusal carries its message whole and Rust code tells refusals apart by function.

use std::collections::TryReserveError;
use std::fmt;
use std::sync::Arc;

/// A failure of a third-party library (an image codec, a base64 decoder), shared so that
/// [`ProcessingError`] stays `Clone + Eq`. Two of them are equal when they read the same.
#[derive(Debug, Clone)]
pub struct SharedError(Arc<dyn std::error::Error + Send + Sync>);

impl SharedError {
    /// The failure of a library, kept for the chain of sources.
    pub fn new(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        SharedError(Arc::new(err))
    }
}

impl PartialEq for SharedError {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_string() == other.0.to_string()
    }
}

impl Eq for SharedError {}

impl fmt::Display for SharedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for SharedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}

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
    /// A list of pixels of another length than the image it is written to.
    #[error("{function} expected {article}array of {expected} pixels, found {found}")]
    PixelCount {
        /// The WarpScript function that refused.
        function: &'static str,
        /// `"an "` when the function's wording has it, `""` when it does not.
        article: &'static str,
        /// The number of pixels of the image.
        expected: usize,
        /// The number of values in the list.
        found: usize,
    },
    /// A list of pixels with a value that is not a `LONG`.
    #[error("{function} expected an array of LONG.")]
    PixelNotLong {
        /// The WarpScript function that refused.
        function: &'static str,
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
    /// A resize to zero in both directions, which Processing refuses.
    #[error("width or height must be > 0 for resize")]
    ResizeWithoutSize,
    /// A filter that takes no parameter, given one (`filter(GRAY, param)`).
    #[error("Use filter({kind}) instead of filter({kind}, param)")]
    FilterTakesNoParam {
        /// The kind of filter.
        kind: &'static str,
    },
    /// `POSTERIZE` given no number of levels.
    #[error("Use filter(POSTERIZE, int levels) instead of filter(POSTERIZE)")]
    PosterizeNeedsLevels,
    /// `POSTERIZE` given fewer than two levels or more than 255.
    #[error("Levels must be between 2 and 255 for filter(POSTERIZE, levels)")]
    PosterizeLevels,
    /// A region with a negative width or height (`Pget` of a negative size).
    #[error("-1")]
    NegativeRegion,
    /// A JPEG quality outside 0.0 to 1.0.
    #[error("Quality out-of-bounds!")]
    QualityOutOfBounds,
    /// A data URI or a byte array that does not hold an image.
    #[error("Pdecode cannot read the image")]
    Decode(#[source] SharedError),
    /// An image that the encoder refused to write.
    #[error("Error while encoding PGraphics or PImage.")]
    Encode(#[source] SharedError),
    /// A `Pencode` format other than `png` and `jpeg`.
    #[error("Only formats 'png' and 'jpeg' are supported.")]
    UnsupportedFormat,
    /// A `Pencode` chunk type whose value is not a list of chunks.
    #[error("Chunk type must be associated with a list of chunks.")]
    ChunkNotList,
    /// A `Pencode` text chunk that is not a MAP; `kinds` names the chunk types of the message.
    #[error("{kinds} chunks must be MAP instances.")]
    ChunkNotMap {
        /// The chunk types the message is about, as Warp 10 writes them.
        kinds: &'static str,
    },
    /// A `Pencode` text chunk without a `STRING` keyword or text; `kinds` names the chunk types.
    #[error("{kinds} chunks MUST contains 'keyword' and 'text' entries of type STRING.")]
    ChunkFields {
        /// The chunk types the message is about, as Warp 10 writes them.
        kinds: &'static str,
    },
    /// A `Pencode` chunk type that is none of `tEXt`, `zTXt` and `iTXt`.
    #[error("Only 'tEXt', 'zTXt' and 'iTXt' chunks can be specified.")]
    UnknownChunk,
    /// A JPEG `quality` that is neither a `DOUBLE` nor a `LONG`.
    #[error(
        "Parameter 'quality' for format 'jpeg' must be a DOUBLE between 0.0 and 1.0, or a LONG between 0 and 100."
    )]
    QualityType,
    /// A vertex function used where the shape it needs is not begun: Processing's own message,
    /// which names no WarpScript function.
    #[error("{message}")]
    ShapeState {
        /// The message, as Processing words it.
        message: &'static str,
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
            | ProcessingError::PixelCount { .. }
            | ProcessingError::PixelNotLong { .. }
            | ProcessingError::NonPositiveMaximum
            | ProcessingError::Decode(_)
            | ProcessingError::Encode(_)
            | ProcessingError::UnsupportedFormat
            | ProcessingError::ChunkNotList
            | ProcessingError::ChunkNotMap { .. }
            | ProcessingError::ChunkFields { .. }
            | ProcessingError::UnknownChunk
            | ProcessingError::QualityType => false,
            ProcessingError::ShapeState { .. } => true,
            ProcessingError::EmptyImage { .. }
            | ProcessingError::StyleUnderflow
            | ProcessingError::MatrixUnderflow
            | ProcessingError::MatrixOverflow
            | ProcessingError::NegativeWidth
            | ProcessingError::OutOfMemory { .. }
            | ProcessingError::ResizeWithoutSize
            | ProcessingError::FilterTakesNoParam { .. }
            | ProcessingError::PosterizeNeedsLevels
            | ProcessingError::PosterizeLevels
            | ProcessingError::NegativeRegion
            | ProcessingError::QualityOutOfBounds => true,
        }
    }
}
