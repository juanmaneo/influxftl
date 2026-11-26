// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The layout of text: the glyph rings, the alignment and leading of lines, the wrapping of
//! boxes, and the widths of texts. The glyph shapes are approximations, so the tests check the
//! geometry of the layout rather than the shape of any glyph.

use std::rc::Rc;

use super::*;
use crate::processing::Style;

fn style() -> TextStyle {
    Style::default().text
}

/// The lowest and highest coordinates of the filled rings, as `[x, y]` pairs.
fn bounds(outline: &Outline) -> ([f32; 2], [f32; 2]) {
    let mut min = [f32::MAX; 2];
    let mut max = [f32::MIN; 2];
    for &[x, y] in outline.fill.iter().flatten() {
        min = [min[0].min(x), min[1].min(y)];
        max = [max[0].max(x), max[1].max(y)];
    }
    (min, max)
}

fn close(actual: f32, expected: f64) -> bool {
    (f64::from(actual) - expected).abs() < 1e-3
}

#[test]
fn blank_and_unknown_characters_draw_no_ink() {
    assert_eq!(point("", &style(), 0.0, 0.0).fill.len(), 0);
    assert_eq!(point(" ", &style(), 0.0, 0.0).fill.len(), 0);
    assert_eq!(point("中", &style(), 0.0, 0.0).fill.len(), 0);
    assert_eq!(point("\u{1}", &style(), 0.0, 0.0).fill.len(), 0);
}

#[test]
fn text_is_drawn_in_the_fill_only() {
    let outline = point("#", &style(), 0.0, 0.0);
    assert_eq!(outline.stroke.len(), 0);
    assert_eq!(outline.points.len(), 0);
}

#[test]
fn each_run_of_inked_cells_is_one_rectangle() {
    // '!' inks one run in each of its seven rows except the gap above the baseline.
    assert_eq!(point("!", &style(), 0.0, 0.0).fill.len(), 6);
    // '#' has two runs in each row of `01010` and one in each row of `11111`.
    assert_eq!(point("#", &style(), 0.0, 0.0).fill.len(), 12);
}

#[test]
fn each_ring_is_an_axis_aligned_rectangle() {
    for ring in point("#", &style(), 3.0, 4.0).fill {
        assert_eq!(ring.len(), 4);
        let xs: Vec<f32> = ring.iter().map(|p| p[0]).collect();
        let ys: Vec<f32> = ring.iter().map(|p| p[1]).collect();
        assert_eq!(xs[0], xs[3]);
        assert_eq!(xs[1], xs[2]);
        assert_eq!(ys[0], ys[1]);
        assert_eq!(ys[2], ys[3]);
        assert!(xs[0] < xs[1] && ys[0] < ys[2]);
    }
}

#[test]
fn a_line_is_aligned_on_its_anchor() {
    let advance = font::advance(Family::Sans, '!', 12.0);
    let left = bounds(&point("!", &style(), 10.0, 0.0)).0[0];
    let at = |align_x| TextStyle { align_x, ..style() };
    let right = bounds(&point("!", &at(HAlign::Right), 10.0, 0.0)).0[0];
    let center = bounds(&point("!", &at(HAlign::Center), 10.0, 0.0)).0[0];
    assert!(close(right, f64::from(left) - advance));
    assert!(close(center, f64::from(left) - advance / 2.0));
}

#[test]
fn the_first_baseline_follows_the_vertical_alignment() {
    let at = |align_y| TextStyle { align_y, ..style() };
    // The bottom row of '!' lies on the baseline.
    let baseline = |align_y, y| f64::from(bounds(&point("!", &at(align_y), 0.0, y)).1[1]);
    assert!(close(baseline(VAlign::Baseline, 20.0) as f32, 20.0));
    assert!(close(baseline(VAlign::Top, 0.0) as f32, font::ascent(12.0)));
    assert!(close(
        baseline(VAlign::Bottom, 0.0) as f32,
        -font::descent(12.0)
    ));
    assert!(close(
        baseline(VAlign::Center, 0.0) as f32,
        (font::ascent(12.0) - font::descent(12.0)) / 2.0
    ));
}

#[test]
fn lines_are_one_leading_apart() {
    let two = point("!\n!", &style(), 0.0, 0.0);
    assert_eq!(two.fill.len(), 12);
    assert!(close(bounds(&two).1[1], 14.0));
    let leading = TextStyle {
        leading: 20.0,
        ..style()
    };
    assert!(close(bounds(&point("!\n!", &leading, 0.0, 0.0)).1[1], 20.0));
}

#[test]
fn a_box_aligns_its_lines_within_its_width() {
    let at = |align_x| TextStyle { align_x, ..style() };
    let centred = boxed("!", &at(HAlign::Center), 0.0, 0.0, 10.0, 20.0);
    assert_eq!(centred.fill, point("!", &at(HAlign::Center), 5.0, 0.0).fill);
    let right = boxed("!", &at(HAlign::Right), 0.0, 0.0, 10.0, 20.0);
    assert_eq!(right.fill, point("!", &at(HAlign::Right), 10.0, 0.0).fill);
}

#[test]
fn a_box_draws_the_lines_that_start_inside_it() {
    let lines = |h| boxed("!\n!\n!", &style(), 0.0, 0.0, 100.0, h).fill.len();
    assert_eq!(lines(13.0), 6);
    assert_eq!(lines(14.0), 12);
    assert_eq!(lines(15.0), 12);
    assert_eq!(lines(28.0), 18);
}

#[test]
fn a_text_is_wrapped_at_its_line_breaks_and_its_spaces() {
    let style = style();
    assert_eq!(wrap("", 100.0, &style), [""]);
    assert_eq!(wrap("a\n\nb", 100.0, &style), ["a", "", "b"]);
    assert_eq!(wrap("Hello World", 1000.0, &style), ["Hello World"]);
    let (a, b) = (font::line_width(Family::Sans, "a", 12.0), 0.0);
    assert_eq!(wrap("a b", a + b, &style), ["a", "b"]);
    let abc = font::line_width(Family::Sans, "a b c", 12.0);
    assert_eq!(wrap("a b c", abc, &style), ["a b c"]);
}

#[test]
fn a_word_too_wide_for_a_line_is_broken_where_it_overflows() {
    let style = style();
    assert_eq!(wrap("ab cd", 0.0, &style), ["a", "b", "c", "d"]);
    assert_eq!(wrap("Hello", 0.0, &style), ["H", "e", "l", "l", "o"]);
    let lines = wrap("Hello World again", 40.0, &style);
    assert_eq!(lines.concat(), "HelloWorldagain");
    for line in &lines {
        let fits = font::line_width(Family::Sans, line, 12.0) <= 40.0;
        assert!(fits || line.chars().count() == 1, "{line:?}");
    }
}

#[test]
fn a_prefix_fits_up_to_its_last_character_that_does() {
    let (h, e, l) = (
        font::advance(Family::Sans, 'H', 12.0),
        font::advance(Family::Sans, 'e', 12.0),
        font::advance(Family::Sans, 'l', 12.0),
    );
    assert_eq!(fitting_prefix("Hello", 0.0, Family::Sans, 12.0), 1);
    assert_eq!(fitting_prefix("Hello", h + e, Family::Sans, 12.0), 2);
    assert_eq!(fitting_prefix("Hello", h + e + l, Family::Sans, 12.0), 3);
    assert_eq!(fitting_prefix("Hello", 1e9, Family::Sans, 12.0), 5);
}

#[test]
fn a_prefix_is_cut_on_a_character_boundary() {
    assert_eq!(fitting_prefix("中文", 12.0, Family::Sans, 12.0), 3);
    assert_eq!(fitting_prefix("中", 0.0, Family::Sans, 12.0), 3);
}

#[test]
fn the_width_of_a_text_follows_its_font() {
    let font = |name: &str, size| {
        Some(Rc::new(Font {
            name: name.to_owned(),
            size,
        }))
    };
    let serif = TextStyle {
        font: font("Serif", 12.0),
        ..style()
    };
    assert_eq!(width(&serif, "Hello"), 31.0);
    assert_eq!(width(&style(), "Hello"), 29.0);
    let large = TextStyle {
        size: 24.0,
        ..style()
    };
    assert_eq!(width(&large, "Hello"), 59.0);
    let mono = TextStyle {
        font: font("Monospaced", 12.0),
        ..style()
    };
    assert_eq!(width(&mono, "iiii"), 28.0);
}
