// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! `PIMAGE`: an image, a rectangle of ARGB pixels (`0xAARRGGBB`), row by row, and the pixel
//! operations of the Processing API on it (`get`, `set`, `copy`, `mask`, `resize`).

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

/// A rectangle of pixels: its top-left corner and its size, in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    /// The column of the left edge.
    pub x: i64,
    /// The row of the top edge.
    pub y: i64,
    /// The width in pixels.
    pub width: i64,
    /// The height in pixels.
    pub height: i64,
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

    /// An image of the given pixels, which must be `width * height` of them.
    pub fn from_pixels(width: usize, height: usize, pixels: Vec<u32>) -> Self {
        Image {
            width,
            height,
            pixels,
        }
    }

    /// The index of the pixel at (`x`, `y`), or `None` outside the image.
    fn index(&self, x: i64, y: i64) -> Option<usize> {
        if x < 0 || y < 0 {
            return None;
        }
        let (column, row) = (x as usize, y as usize);
        (column < self.width && row < self.height).then(|| row * self.width + column)
    }

    /// The pixel at (`x`, `y`), or `None` outside the image.
    pub fn pixel(&self, x: i64, y: i64) -> Option<u32> {
        self.index(x, y).map(|index| self.pixels[index])
    }

    /// The pixel at (`x`, `y`); outside the image there is no pixel and Processing reads `0`.
    pub fn get(&self, x: i64, y: i64) -> u32 {
        self.pixel(x, y).unwrap_or(0)
    }

    /// Set the pixel at (`x`, `y`); a pixel outside the image is not drawn.
    pub fn set(&mut self, x: i64, y: i64, colour: u32) {
        if let Some(index) = self.index(x, y) {
            self.pixels[index] = colour;
        }
    }

    /// A copy of the rectangle `rect`, whose pixels outside the image read as `0`. A negative size
    /// has no array to hold it.
    pub fn region(&self, rect: Rect) -> Result<Image> {
        let width = usize::try_from(rect.width).map_err(|_| ProcessingError::NegativeRegion)?;
        let height = usize::try_from(rect.height).map_err(|_| ProcessingError::NegativeRegion)?;
        let mut out = Image::new(width, height)?;
        for row in 0..height {
            for column in 0..width {
                let colour = self.get(rect.x + column as i64, rect.y + row as i64);
                out.pixels[row * width + column] = colour;
            }
        }
        Ok(out)
    }

    /// A copy scaled to `width` by `height` pixels: each pixel is the average of the source pixels
    /// it covers, channel by channel (an area average, close to Processing's smooth scaling). A
    /// source with no pixel gives a transparent image.
    pub fn resized(&self, width: usize, height: usize) -> Result<Image> {
        let mut out = Image::new(width, height)?;
        if self.pixels.is_empty() {
            return Ok(out);
        }
        for row in 0..height {
            let (top, bottom) = span(row, height, self.height);
            for column in 0..width {
                let (left, right) = span(column, width, self.width);
                let mut sums = [0u64; 4];
                for source_row in top..bottom {
                    for source_column in left..right {
                        let (alpha, red, green, blue) =
                            channels(self.pixels[source_row * self.width + source_column]);
                        for (sum, channel) in sums.iter_mut().zip([alpha, red, green, blue]) {
                            *sum += u64::from(channel);
                        }
                    }
                }
                let count = ((bottom - top) * (right - left)) as u64;
                let [alpha, red, green, blue] = sums.map(|sum| (sum / count) as u8);
                out.pixels[row * width + column] = argb(alpha, red, green, blue);
            }
        }
        Ok(out)
    }

    /// The alpha of every pixel becomes the blue channel of the same pixel of `mask`, which has
    /// the size of this image (checked by the caller). This is `PImage.mask`.
    pub fn mask(&mut self, mask: &Image) {
        for (pixel, mark) in self.pixels.iter_mut().zip(&mask.pixels) {
            *pixel = (*pixel & 0x00ff_ffff) | ((mark & 0xff) << 24);
        }
    }

    /// Paint `source` with its top-left corner at (`x`, `y`), pixel for pixel and clipped to this
    /// image, without blending (`PImage.set(x, y, PImage)`).
    pub fn paste(&mut self, source: &Image, x: i64, y: i64) {
        for row in 0..source.height {
            for column in 0..source.width {
                let colour = source.pixels[row * source.width + column];
                self.set(x + column as i64, y + row as i64, colour);
            }
        }
    }
}

/// The source rows (or columns) an output row (or column) `index` of `out` covers, out of `input`
/// ones. Upscaling repeats a source pixel, so every output pixel covers at least one.
fn span(index: usize, out: usize, input: usize) -> (usize, usize) {
    let start = index * input / out;
    let end = ((index + 1) * input / out).max(start + 1);
    (start, end.min(input))
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod image_tests;
