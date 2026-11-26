// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The filters of `PImage.filter` (`Pfilter`): thresholding, grey levels, opacity, inversion,
//! posterizing, blur, erosion and dilation of the pixels of an image.
//!
//! `THRESHOLD`, `GRAY`, `OPAQUE`, `INVERT` and `POSTERIZE` are Processing's formulas, bit for
//! bit. `BLUR` is a box blur (Processing uses its own kernel, so the values differ), and
//! `ERODE` and `DILATE` pick the darkest or lightest of a pixel and its four neighbours by
//! luminance, which is what the Processing results show.

use super::finite;
use super::image::{Image, argb, channels};
use crate::error::Result;
use crate::error::processing::ProcessingError;

/// The kinds of filter, with the names a script spells them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `THRESHOLD`: black or white by the brightest channel, against a level (0.5 by default).
    Threshold,
    /// `GRAY`: the luminance of each pixel.
    Gray,
    /// `OPAQUE`: every pixel made opaque.
    Opaque,
    /// `INVERT`: each colour channel inverted.
    Invert,
    /// `POSTERIZE`: each channel reduced to a number of levels, which the script gives.
    Posterize,
    /// `BLUR`: an average of the pixels around, over a radius (1 by default).
    Blur,
    /// `ERODE`: each pixel becomes the darkest of itself and its neighbours.
    Erode,
    /// `DILATE`: each pixel becomes the lightest of itself and its neighbours.
    Dilate,
}

impl Kind {
    /// The name of the kind, as the script spells it.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Threshold => "THRESHOLD",
            Kind::Gray => "GRAY",
            Kind::Opaque => "OPAQUE",
            Kind::Invert => "INVERT",
            Kind::Posterize => "POSTERIZE",
            Kind::Blur => "BLUR",
            Kind::Erode => "ERODE",
            Kind::Dilate => "DILATE",
        }
    }

    /// Whether the kind takes a parameter (`filter(kind, param)`). The others refuse one.
    fn takes_param(self) -> bool {
        matches!(self, Kind::Threshold | Kind::Posterize | Kind::Blur)
    }
}

/// The luminance of a pixel, in `0..=255`: `(77 red + 151 green + 28 blue) >> 8`.
fn luminance(colour: u32) -> u32 {
    let (_, red, green, blue) = channels(colour);
    (77 * u32::from(red) + 151 * u32::from(green) + 28 * u32::from(blue)) >> 8
}

/// `PImage.filter(kind)` when `param` is `None`, `filter(kind, param)` otherwise. A parameter
/// that is not finite was refused by the caller already, but is checked again where it is
/// multiplied.
pub fn apply(image: &mut Image, kind: Kind, param: Option<f32>) -> Result<()> {
    let param = param.map(finite).transpose()?;
    if param.is_some() && !kind.takes_param() {
        return Err(ProcessingError::FilterTakesNoParam { kind: kind.name() }.into());
    }
    match kind {
        Kind::Threshold => {
            let level = param.unwrap_or(0.5);
            let threshold = finite(level * 255.0)? as i32;
            threshold_pixels(image, threshold);
        }
        Kind::Gray => map_pixels(image, |colour| {
            let (alpha, _, _, _) = channels(colour);
            let lum = luminance(colour) as u8;
            argb(alpha, lum, lum, lum)
        }),
        Kind::Opaque => map_pixels(image, |colour| colour | 0xff00_0000),
        Kind::Invert => map_pixels(image, |colour| colour ^ 0x00ff_ffff),
        Kind::Posterize => {
            let Some(levels) = param else {
                return Err(ProcessingError::PosterizeNeedsLevels.into());
            };
            // `(int) levels`, as Processing casts the float.
            let levels = levels as i32;
            if !(2..=255).contains(&levels) {
                return Err(ProcessingError::PosterizeLevels.into());
            }
            map_pixels(image, |colour| posterize(colour, levels));
        }
        Kind::Blur => {
            let radius = param.unwrap_or(1.0) as i32;
            if radius > 0 {
                box_blur(image, radius as usize);
            }
        }
        Kind::Erode => extremum(image, Extremum::Darkest),
        Kind::Dilate => extremum(image, Extremum::Lightest),
    }
    Ok(())
}

/// Every pixel replaced by `change(pixel)`.
fn map_pixels(image: &mut Image, change: impl Fn(u32) -> u32) {
    for pixel in &mut image.pixels {
        *pixel = change(*pixel);
    }
}

/// `THRESHOLD`: white where the brightest channel reaches the threshold, black elsewhere; the
/// alpha of each pixel is kept.
fn threshold_pixels(image: &mut Image, threshold: i32) {
    map_pixels(image, |colour| {
        let (alpha, red, green, blue) = channels(colour);
        let brightest = red.max(green).max(blue);
        let level = if i32::from(brightest) >= threshold {
            255
        } else {
            0
        };
        argb(alpha, level, level, level)
    });
}

/// `POSTERIZE`: each channel `c` becomes `((c * levels) >> 8) * 255 / (levels - 1)`.
fn posterize(colour: u32, levels: i32) -> u32 {
    let (alpha, red, green, blue) = channels(colour);
    let step = |channel: u8| {
        let level = (i32::from(channel) * levels) >> 8;
        (level * 255 / (levels - 1)) as u8
    };
    argb(alpha, step(red), step(green), step(blue))
}

/// The box blur of `radius`: each channel averaged over the pixels up to `radius` away in each
/// direction that the image holds, first along the rows and then along the columns.
fn box_blur(image: &mut Image, radius: usize) {
    let (width, height) = (image.width, image.height);
    let mut rows = vec![0; image.pixels.len()];
    for row in 0..height {
        for column in 0..width {
            let from = column.saturating_sub(radius);
            let to = (column + radius + 1).min(width);
            let span = &image.pixels[row * width + from..row * width + to];
            rows[row * width + column] = mean(span.iter().copied());
        }
    }
    for column in 0..width {
        for row in 0..height {
            let from = row.saturating_sub(radius);
            let to = (row + radius + 1).min(height);
            let span = (from..to).map(|r| rows[r * width + column]);
            image.pixels[row * width + column] = mean(span);
        }
    }
}

/// The channel-wise mean of some pixels, which must not be empty (the blur always has the pixel
/// it is computed for).
fn mean(colours: impl Iterator<Item = u32>) -> u32 {
    let mut sums = [0u32; 4];
    let mut count = 0;
    for colour in colours {
        for (sum, channel) in sums.iter_mut().zip(channels_of(colour)) {
            *sum += u32::from(channel);
        }
        count += 1;
    }
    let [alpha, red, green, blue] = sums.map(|sum| (sum / count.max(1)) as u8);
    argb(alpha, red, green, blue)
}

/// The four channels of a pixel as an array, alpha first.
fn channels_of(colour: u32) -> [u8; 4] {
    let (alpha, red, green, blue) = channels(colour);
    [alpha, red, green, blue]
}

/// Which pixel of a neighbourhood an erosion or a dilation keeps.
#[derive(Clone, Copy)]
enum Extremum {
    Darkest,
    Lightest,
}

/// `ERODE` or `DILATE`: each pixel becomes the neighbour (itself and its four neighbours inside
/// the image) with the least or the most luminance. The original pixels are read throughout, and
/// the pixel itself wins a tie.
fn extremum(image: &mut Image, pick: Extremum) {
    const NEIGHBOURS: [(i64, i64); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    let source = image.clone();
    for row in 0..source.height {
        for column in 0..source.width {
            let (x, y) = (column as i64, row as i64);
            let mut chosen = source.pixels[row * source.width + column];
            let mut chosen_luminance = luminance(chosen);
            for (dx, dy) in NEIGHBOURS {
                let Some(colour) = source.pixel(x + dx, y + dy) else {
                    continue;
                };
                let lum = luminance(colour);
                let better = match pick {
                    Extremum::Darkest => lum < chosen_luminance,
                    Extremum::Lightest => lum > chosen_luminance,
                };
                if better {
                    chosen = colour;
                    chosen_luminance = lum;
                }
            }
            image.pixels[row * source.width + column] = chosen;
        }
    }
}

#[cfg(test)]
#[path = "filter_tests.rs"]
mod filter_tests;
