// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Colour modes of the Processing API: `colorMode`, `color`, the readers `red`, `hue`…, and
//! `lerpColor`. Colours are ARGB `int`s, as Processing packs them.
//!
//! The rules are those of the documented API, checked against Warp 10 one case at a time: an
//! `int` with no alpha byte that is not above the first maximum is a gray level, any other
//! `int` is an ARGB colour; every value is clamped to its maximum, scaled to `0..=1` and cut
//! to a byte with a truncation, never a rounding.

use super::finite;
use super::image::{argb, channels};
use crate::error::Result;
use crate::error::processing::ProcessingError;

/// The space the values of `color` are read in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    /// `RGB`: red, green, blue.
    Rgb,
    /// `HSB`: hue, saturation, brightness.
    Hsb,
}

/// The colour mode: the space, and the maximum of each of its four values. The maxima are kept
/// finite and positive, which is what lets the conversions below divide by them without a check.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorMode {
    space: Space,
    max1: f32,
    max2: f32,
    max3: f32,
    max_alpha: f32,
}

impl Default for ColorMode {
    /// `RGB` with every maximum at 255.
    fn default() -> Self {
        ColorMode {
            space: Space::Rgb,
            max1: 255.0,
            max2: 255.0,
            max3: 255.0,
            max_alpha: 255.0,
        }
    }
}

/// A unit value (`0..=1`) as a byte, truncated like Java's `(int) (255 * value)`.
fn byte_of(unit: f32) -> u8 {
    (255.0 * unit) as u8
}

/// Whether the `int` is read as a gray level rather than as an ARGB colour: it carries no
/// alpha byte and is not above the first maximum.
fn is_gray_level(color: i32, max1: f32) -> bool {
    (color as u32) & 0xff00_0000 == 0 && (color as f32) <= max1
}

/// The red, green and blue of the HSB colour `(hue, saturation, brightness)`, each in `0..=1`
/// (the conversion `colorCalc` applies in the `HSB` mode).
fn hsb_to_unit_rgb(hue: f32, saturation: f32, brightness: f32) -> (f32, f32, f32) {
    if saturation == 0.0 {
        return (brightness, brightness, brightness);
    }
    let which = (hue - hue.trunc()) * 6.0;
    let fraction = which - which.trunc();
    let p = brightness * (1.0 - saturation);
    let q = brightness * (1.0 - saturation * fraction);
    let t = brightness * (1.0 - saturation * (1.0 - fraction));
    match which as i32 {
        0 => (brightness, t, p),
        1 => (q, brightness, p),
        2 => (p, brightness, t),
        3 => (p, q, brightness),
        4 => (t, p, brightness),
        _ => (brightness, p, q),
    }
}

/// The HSB to RGB conversion of `java.awt.Color`, which rounds each channel to the nearest byte
/// (`lerpColor` uses it, where `colorCalc` truncates).
fn hsb_to_rgb_rounded(hue: f32, saturation: f32, brightness: f32) -> (u8, u8, u8) {
    let (red, green, blue) = hsb_to_unit_rgb(hue, saturation, brightness);
    let round = |unit: f32| (unit * 255.0 + 0.5) as u8;
    (round(red), round(green), round(blue))
}

/// The RGB to HSB conversion of `java.awt.Color`: hue, saturation and brightness in `0..=1`.
/// A byte difference is at most 255 and the divisions are by non-zero bytes, so every result
/// is finite.
fn rgb_to_hsb(red: u8, green: u8, blue: u8) -> (f32, f32, f32) {
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let brightness = f32::from(max) / 255.0;
    if max == min {
        // A gray (and black) has no hue and no saturation.
        return (0.0, 0.0, brightness);
    }
    let span = f32::from(max - min);
    let saturation = span / f32::from(max);
    let redc = f32::from(max - red) / span;
    let greenc = f32::from(max - green) / span;
    let bluec = f32::from(max - blue) / span;
    let hue = if red == max {
        bluec - greenc
    } else if green == max {
        2.0 + redc - bluec
    } else {
        4.0 + greenc - redc
    } / 6.0;
    (
        if hue < 0.0 { hue + 1.0 } else { hue },
        saturation,
        brightness,
    )
}

impl ColorMode {
    /// The colour mode with these maxima. A maximum must be a finite positive number, since
    /// the values are divided by it.
    pub fn new(space: Space, max1: f32, max2: f32, max3: f32, max_alpha: f32) -> Result<Self> {
        for max in [max1, max2, max3, max_alpha] {
            if finite(max)? <= 0.0 {
                return Err(ProcessingError::NonPositiveMaximum.into());
            }
        }
        Ok(ColorMode {
            space,
            max1,
            max2,
            max3,
            max_alpha,
        })
    }

    /// This mode in another space, with the same maxima.
    #[must_use]
    pub fn with_space(self, space: Space) -> Self {
        ColorMode { space, ..self }
    }

    /// The maximum of the alpha value.
    pub fn max_alpha(&self) -> f32 {
        self.max_alpha
    }

    /// Whether this is the mode of a new context: `RGB`, every maximum 255.
    fn is_default(&self) -> bool {
        *self == ColorMode::default()
    }

    /// `value` clamped to `0..=max`, as a fraction of `max`, which is in `0..=1`. The callers
    /// pass a finite value (`float_of` refuses the rest) and this mode's maxima are positive.
    fn fraction(value: f32, max: f32) -> f32 {
        value.clamp(0.0, max) / max
    }

    /// The ARGB colour of four values in this mode (`color(v1, v2, v3, a)`): each value is
    /// clamped to its maximum, scaled to `0..=1`, converted to RGB when the space is `HSB`,
    /// then cut to a byte. The values must be finite.
    pub fn calc(&self, v1: f32, v2: f32, v3: f32, alpha: f32) -> u32 {
        let x = Self::fraction(v1, self.max1);
        let y = Self::fraction(v2, self.max2);
        let z = Self::fraction(v3, self.max3);
        let alpha = Self::fraction(alpha, self.max_alpha);
        let (red, green, blue) = match self.space {
            Space::Rgb => (x, y, z),
            Space::Hsb => hsb_to_unit_rgb(x, y, z),
        };
        argb(byte_of(alpha), byte_of(red), byte_of(green), byte_of(blue))
    }

    /// The ARGB colour of a gray level and an alpha (`color(gray, alpha)`): the three channels
    /// are the level over the first maximum, whatever the space. The values must be finite.
    pub fn gray(&self, gray: f32, alpha: f32) -> u32 {
        let level = byte_of(Self::fraction(gray, self.max1));
        let alpha = byte_of(Self::fraction(alpha, self.max_alpha));
        argb(alpha, level, level, level)
    }

    /// The colour an `int` stands for (`fill(int rgb)`, `color(int c)`): a gray level when it
    /// has no alpha byte and is not above the first maximum, the ARGB colour itself otherwise.
    pub fn from_int(&self, color: i32) -> u32 {
        if is_gray_level(color, self.max1) {
            self.gray(color as f32, self.max_alpha)
        } else {
            color as u32
        }
    }

    /// The colour an `int` and an alpha stand for (`color(int gray, float alpha)`): the `int` is
    /// a gray level, clamped to the first maximum even when its bits look like an ARGB colour
    /// (Warp 10 gives `color(0xff102030, 127)` the colour `0x7f000000`).
    pub fn from_int_alpha(&self, gray: i32, alpha: f32) -> u32 {
        self.gray(gray as f32, alpha)
    }

    /// One component of a colour (`red`, `hue`…), in this mode's range. A component of a colour
    /// is at most its maximum, so the result is finite.
    pub fn component(&self, color: u32, which: Component) -> f32 {
        let (alpha, red, green, blue) = channels(color);
        let scaled = |channel: u8, max: f32| {
            if self.is_default() {
                f32::from(channel)
            } else {
                f32::from(channel) / 255.0 * max
            }
        };
        match which {
            Component::Red => scaled(red, self.max1),
            Component::Green => scaled(green, self.max2),
            Component::Blue => scaled(blue, self.max3),
            Component::Alpha => scaled(alpha, self.max_alpha),
            Component::Hue => rgb_to_hsb(red, green, blue).0 * self.max1,
            Component::Saturation => rgb_to_hsb(red, green, blue).1 * self.max2,
            Component::Brightness => rgb_to_hsb(red, green, blue).2 * self.max3,
        }
    }

    /// The interpolation of two colours by `amount` (`lerpColor`): each channel in `RGB`, hue,
    /// saturation and brightness in `HSB` (the hue is not wrapped, so red to blue goes through
    /// green). The alpha is interpolated alike, and every result is rounded. The amount must be
    /// finite.
    pub fn lerp(&self, from: u32, to: u32, amount: f32) -> u32 {
        let amount = amount.clamp(0.0, 1.0);
        let (a1, r1, g1, b1) = channels(from);
        let (a2, r2, g2, b2) = channels(to);
        let mix = |x: f32, y: f32| x + (y - x) * amount;
        let alpha = mix(f32::from(a1), f32::from(a2)).round() as u8;
        match self.space {
            Space::Rgb => {
                let channel = |x: u8, y: u8| mix(f32::from(x), f32::from(y)).round() as u8;
                argb(alpha, channel(r1, r2), channel(g1, g2), channel(b1, b2))
            }
            Space::Hsb => {
                let (h1, s1, v1) = rgb_to_hsb(r1, g1, b1);
                let (h2, s2, v2) = rgb_to_hsb(r2, g2, b2);
                let (red, green, blue) = hsb_to_rgb_rounded(mix(h1, h2), mix(s1, s2), mix(v1, v2));
                argb(alpha, red, green, blue)
            }
        }
    }
}

/// The component of a colour that a reader returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    /// `red`.
    Red,
    /// `green`.
    Green,
    /// `blue`.
    Blue,
    /// `alpha`.
    Alpha,
    /// `hue`.
    Hue,
    /// `saturation`.
    Saturation,
    /// `brightness`.
    Brightness,
}

#[cfg(test)]
#[path = "color_tests.rs"]
mod color_tests;
