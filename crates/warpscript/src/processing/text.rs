// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Text drawn on a graphics context: the glyph of each character, laid out in lines. A glyph is
//! the 5 by 7 bitmap of `glyph.rs` scaled to the text size, so the shapes approximate Processing's
//! fonts (see `DIVERGENCES.md`). Its advance comes from the font's family, and a character with no
//! bitmap takes its advance and no ink.

use std::iter::once;

use super::draw::Outline;
use super::font::{self, Family, Font};
use super::glyph;
use super::path::Ring;
use super::{HAlign, TextStyle, VAlign};

/// The family of the font of `style`: the default font's when none was set.
fn family(style: &TextStyle) -> Family {
    style.font.as_deref().map_or(Family::Sans, Font::family)
}

/// The side of one cell of a glyph bitmap at `size`: seven of them make the height of a capital.
fn cell(size: f32) -> f64 {
    0.714 * f64::from(size) / 7.0
}

/// The share of a line's width that lies left of its anchor: none for `LEFT`, half for `CENTER`,
/// all of it for `RIGHT`.
fn across(align: HAlign) -> f64 {
    match align {
        HAlign::Left => 0.0,
        HAlign::Center => 0.5,
        HAlign::Right => 1.0,
    }
}

/// The baseline of the first line, placed at `y` as `textAlign` places the text vertically.
fn first_baseline(style: &TextStyle, y: f64) -> f64 {
    let size = style.size;
    match style.align_y {
        VAlign::Baseline => y,
        VAlign::Top => y + font::ascent(size),
        VAlign::Bottom => y - font::descent(size),
        VAlign::Center => y + (font::ascent(size) - font::descent(size)) / 2.0,
    }
}

/// The rectangle from `(x0, y0)` to `(x1, y1)`.
fn rect(x0: f64, x1: f64, y0: f64, y1: f64) -> Ring {
    vec![
        [x0 as f32, y0 as f32],
        [x1 as f32, y0 as f32],
        [x1 as f32, y1 as f32],
        [x0 as f32, y1 as f32],
    ]
}

/// Add the rings of the ink of `c`, drawn with `advance` pixels of room from `left`, on the
/// `baseline`. Each run of inked cells in a row is one rectangle.
fn glyph_rings(c: char, left: f64, advance: f64, baseline: f64, cell: f64, rings: &mut Vec<Ring>) {
    let Some(rows) = glyph::rows(c) else { return };
    let x0 = left + (advance - 5.0 * cell) / 2.0;
    let top = baseline - 7.0 * cell;
    for (row, bits) in rows.iter().enumerate() {
        let y0 = top + row as f64 * cell;
        let mut run = None;
        for (col, bit) in bits.bytes().chain(once(b'0')).enumerate() {
            match (bit == b'1', run) {
                (true, None) => run = Some(col),
                (false, Some(from)) => {
                    rings.push(rect(
                        x0 + from as f64 * cell,
                        x0 + col as f64 * cell,
                        y0,
                        y0 + cell,
                    ));
                    run = None;
                }
                _ => {}
            }
        }
    }
}

/// The outline of `lines`, each aligned on `anchor` as `textAlign` aligns it. The first baseline
/// is `first`, and each line lies one leading below the one before. A line whose baseline lies
/// below `bottom` is not drawn, nor the lines after it.
fn lay_out<S: AsRef<str>>(
    lines: &[S],
    style: &TextStyle,
    anchor: f64,
    first: f64,
    bottom: Option<f64>,
) -> Outline {
    let (family, size, cell) = (family(style), style.size, cell(style.size));
    let mut rings = Vec::new();
    let mut baseline = first;
    for line in lines {
        if let Some(limit) = bottom
            && baseline > limit
        {
            break;
        }
        let line = line.as_ref();
        let mut pen = anchor - across(style.align_x) * font::line_width(family, line, size);
        for c in line.chars() {
            let advance = font::advance(family, c, size);
            glyph_rings(c, pen, advance, baseline, cell, &mut rings);
            pen += advance;
        }
        baseline += f64::from(style.leading);
    }
    Outline {
        fill: rings,
        ..Outline::default()
    }
}

/// The lines of `text` in a box `width` pixels wide. The text breaks at each `\n`, and at each
/// space where a line would overflow. A word too wide for a line is broken where it overflows.
fn wrap(text: &str, width: f64, style: &TextStyle) -> Vec<String> {
    let (family, size) = (family(style), style.size);
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line: Option<String> = None;
        for word in paragraph.split(' ') {
            if let Some(current) = line.take() {
                let joined = format!("{current} {word}");
                if font::line_width(family, &joined, size) <= width {
                    line = Some(joined);
                    continue;
                }
                lines.push(current);
            }
            let mut rest = word;
            while rest.chars().count() > 1 && font::line_width(family, rest, size) > width {
                let split = fitting_prefix(rest, width, family, size);
                lines.push(rest[..split].to_owned());
                rest = &rest[split..];
            }
            line = Some(rest.to_owned());
        }
        lines.extend(line);
    }
    lines
}

/// The byte length of the longest prefix of `word` that fits in `width`, and at least one
/// character: characters are added until the next one overflows.
fn fitting_prefix(word: &str, width: f64, family: Family, size: f32) -> usize {
    let mut pen = 0.0;
    word.char_indices()
        .find(|&(index, c)| {
            pen += font::advance(family, c, size);
            index > 0 && pen > width
        })
        .map_or(word.len(), |(index, _)| index)
}

/// The width of `text` in the font of `style`, the widest of its lines (`textWidth`).
pub fn width(style: &TextStyle, text: &str) -> f64 {
    font::width(family(style), text, style.size)
}

/// `text(str, x, y)` and `text(str, x, y, z)`: the lines of `text`, aligned at `x` and placed at
/// `y` as `textAlign` places them. A `z` is not drawn: the plane of the graphics is the plane of
/// the text.
pub fn point(text: &str, style: &TextStyle, x: f32, y: f32) -> Outline {
    let lines: Vec<&str> = text.split('\n').collect();
    lay_out(
        &lines,
        style,
        f64::from(x),
        first_baseline(style, f64::from(y)),
        None,
    )
}

/// `text(str, x, y, width, height)`: the text wrapped into the box at `x` and `y`. Each line is
/// aligned within the box as `textAlign` aligns it, and the lines whose baseline lies below the
/// box are not drawn.
pub fn boxed(text: &str, style: &TextStyle, x: f32, y: f32, w: f32, h: f32) -> Outline {
    let (x, y, w, h) = (f64::from(x), f64::from(y), f64::from(w), f64::from(h));
    let lines = wrap(text, w, style);
    lay_out(
        &lines,
        style,
        x + w * across(style.align_x),
        first_baseline(style, y),
        Some(y + h),
    )
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod text_tests;
