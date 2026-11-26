// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! `PFONT`: a font, named by `createFont`, which `textFont` selects and `text` draws with. Its
//! metrics are those of Processing's families of fonts: the advance of each glyph, and the ascent
//! and descent of a size. The advances come from the faces of the family the name selects, not
//! from the font named (see `font_advances.rs` and `DIVERGENCES.md`).

use super::font_advances::{MONO, SANS, SERIF};

/// The name of the font `textSize` creates when no font was set.
pub const DEFAULT_NAME: &str = "Lucida Sans";

/// A font: its name and the size it was created with.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    /// The name the font was created with.
    pub name: String,
    /// The size the font was created with.
    pub size: f32,
}

/// The family of a font, which sets the advance widths of its glyphs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Sans-serif: the names that select no other family.
    Sans,
    /// Serif: `Serif`.
    Serif,
    /// Monospaced: `Monospaced` and `DialogInput`.
    Mono,
}

impl Font {
    /// The font `textSize` creates when no font was set, at `size`.
    pub fn default_at(size: f32) -> Self {
        Font {
            name: DEFAULT_NAME.to_owned(),
            size,
        }
    }

    /// The family the name selects. Processing matches the names without regard to case.
    pub fn family(&self) -> Family {
        let name = self.name.as_str();
        if name.eq_ignore_ascii_case("Serif") {
            Family::Serif
        } else if ["Monospaced", "DialogInput"]
            .iter()
            .any(|mono| name.eq_ignore_ascii_case(mono))
        {
            Family::Mono
        } else {
            Family::Sans
        }
    }
}

/// The size Processing rounds a font to: the nearest whole number, halves up.
pub fn rounded(size: f32) -> f32 {
    (f64::from(size) + 0.5).floor() as f32
}

/// The advance widths of a family, in units of the em (1000 per em), from U+0020 to U+00FF.
fn table(family: Family) -> &'static [u16] {
    match family {
        Family::Sans => &SANS,
        Family::Serif => &SERIF,
        Family::Mono => &MONO,
    }
}

/// The advance of `c` in units of the em. The controls and the combining marks take no room; a
/// character the tables do not hold takes one em.
fn units(family: Family, c: char) -> f64 {
    let code = u32::from(c);
    if code < 0x20 || (0x300..=0x36f).contains(&code) {
        return 0.0;
    }
    match (code as usize)
        .checked_sub(0x20)
        .and_then(|index| table(family).get(index))
    {
        Some(&units) => f64::from(units),
        None => 1000.0,
    }
}

/// The advance of `c` at `size`, in whole pixels: the width in 26.6 fixed point, rounded to
/// the nearest 1/64 pixel, then rounded to the nearest whole pixel.
pub fn advance(family: Family, c: char, size: f32) -> f64 {
    let scaled = units(family, c) * f64::from(size) * 64.0 / 1000.0;
    let fixed = (scaled + 0.5).floor();
    ((fixed + 32.0) / 64.0).floor()
}

/// The width of a line of text at `size`: the sum of the advances of its characters.
pub fn line_width(family: Family, line: &str, size: f32) -> f64 {
    line.chars().map(|c| advance(family, c, size)).sum()
}

/// The width of a text at `size`: the widest of its lines, which are separated by `\n`.
pub fn width(family: Family, text: &str, size: f32) -> f64 {
    text.split('\n')
        .map(|line| line_width(family, line, size))
        .fold(0.0, f64::max)
}

/// The distance from the baseline up to the top of the glyphs at `size`, in pixels.
pub fn ascent(size: f32) -> f64 {
    (1.069 * f64::from(size) - 0.05).ceil()
}

/// The distance from the baseline down to the bottom of the glyphs at `size`, in pixels.
pub fn descent(size: f32) -> f64 {
    (0.293 * f64::from(size) - 0.05).ceil()
}

#[cfg(test)]
#[path = "font_tests.rs"]
mod font_tests;
