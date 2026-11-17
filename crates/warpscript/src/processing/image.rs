// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! `PIMAGE`: an image, a rectangle of ARGB pixels (`0xAARRGGBB`), row by row.

use crate::error::Result;
use crate::error::processing::ProcessingError;

/// A pixel of ARGB channels, packed as Java's `int` colour: alpha in the top byte.
pub fn argb(alpha: u8, red: u8, green: u8, blue: u8) -> u32 {
    (u32::from(alpha) << 24) | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

/// The four channels of an ARGB pixel: alpha, red, green, blue.
pub fn channels(color: u32) -> (u8, u8, u8, u8) {
    (
        (color >> 24) as u8,
        (color >> 16) as u8,
        (color >> 8) as u8,
        color as u8,
    )
}

/// An image: `width` × `height` pixels, the first row first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// The width in pixels.
    pub width: usize,
    /// The height in pixels.
    pub height: usize,
    /// The pixels, `width * height` of them, row by row.
    pub pixels: Vec<u32>,
}

impl Image {
    /// A transparent image of `width` by `height` pixels. The buffer is reserved first, so a size
    /// the memory cannot hold is a typed failure rather than an abort.
    pub fn new(width: usize, height: usize) -> Result<Self> {
        let count = width.saturating_mul(height);
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(count)
            .map_err(|source| ProcessingError::OutOfMemory {
                pixels: count,
                source,
            })?;
        pixels.resize(count, 0);
        Ok(Image {
            width,
            height,
            pixels,
        })
    }
}
