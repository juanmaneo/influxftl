// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The metrics of fonts. The widths are Warp 10 3.5.0's answers to the same scripts (`PtextWidth`,
//! `PtextAscent`, `PtextDescent`, `PcreateFont`).

use super::*;

fn named(name: &str) -> Font {
    Font {
        name: name.to_owned(),
        size: 12.0,
    }
}

#[test]
fn family_is_chosen_by_name_without_regard_to_case() {
    assert_eq!(named("Serif").family(), Family::Serif);
    assert_eq!(named("serif").family(), Family::Serif);
    assert_eq!(named("SERIF").family(), Family::Serif);
    assert_eq!(named("Monospaced").family(), Family::Mono);
    assert_eq!(named("monospaced").family(), Family::Mono);
    assert_eq!(named("DialogInput").family(), Family::Mono);
    assert_eq!(named("dialoginput").family(), Family::Mono);
    assert_eq!(named("SansSerif").family(), Family::Sans);
    assert_eq!(named("Dialog").family(), Family::Sans);
    assert_eq!(named("Symbol").family(), Family::Sans);
    assert_eq!(named("Lucida Sans").family(), Family::Sans);
    assert_eq!(named("").family(), Family::Sans);
}

#[test]
fn default_font_is_lucida_sans_at_the_size_given() {
    let font = Font::default_at(14.0);
    assert_eq!(font.name, "Lucida Sans");
    assert_eq!(font.size, 14.0);
    assert_eq!(font.family(), Family::Sans);
}

#[test]
fn sizes_round_half_up() {
    assert_eq!(rounded(12.0), 12.0);
    assert_eq!(rounded(12.4), 12.0);
    assert_eq!(rounded(12.5), 13.0);
    assert_eq!(rounded(0.4), 0.0);
    assert_eq!(rounded(0.5), 1.0);
    assert_eq!(rounded(-3.0), -3.0);
}

#[test]
fn ascent_and_descent_follow_the_size() {
    assert_eq!(ascent(12.0), 13.0);
    assert_eq!(descent(12.0), 4.0);
    assert_eq!(ascent(24.0), 26.0);
    assert_eq!(descent(24.0), 7.0);
    assert_eq!(ascent(30.0), 33.0);
    assert_eq!(descent(0.0), 0.0);
}

#[test]
fn texts_have_the_widths_of_warp_10() {
    let sans = Family::Sans;
    assert_eq!(width(sans, "Hello", 12.0), 29.0);
    assert_eq!(width(sans, "Hello", 24.0), 59.0);
    assert_eq!(width(Family::Serif, "Hello", 12.0), 31.0);
    assert_eq!(width(Family::Mono, "iiii", 12.0), 28.0);
    assert_eq!(width(sans, "iiii", 12.0), 12.0);
    assert_eq!(width(sans, "%", 12.0), 10.0);
    assert_eq!(width(sans, "'", 12.0), 3.0);
    assert_eq!(width(sans, "\\", 12.0), 4.0);
    assert_eq!(width(sans, "\"", 12.0), 5.0);
    assert_eq!(width(sans, " ", 12.0), 3.0);
    assert_eq!(width(sans, "中", 12.0), 12.0);
    assert_eq!(width(sans, "42", 12.0), 14.0);
}

#[test]
fn the_widest_line_of_a_text_gives_its_width() {
    assert_eq!(width(Family::Sans, "Hi\nthere", 12.0), 30.0);
    assert_eq!(width(Family::Sans, "there\nHi", 12.0), 30.0);
    assert_eq!(width(Family::Sans, "", 12.0), 0.0);
    assert_eq!(width(Family::Sans, "\n", 12.0), 0.0);
    assert_eq!(line_width(Family::Sans, "", 12.0), 0.0);
}

#[test]
fn controls_and_combining_marks_take_no_room() {
    assert_eq!(advance(Family::Sans, '\u{1}', 12.0), 0.0);
    assert_eq!(advance(Family::Sans, '\u{301}', 12.0), 0.0);
    assert_eq!(advance(Family::Sans, '\u{36f}', 12.0), 0.0);
}

#[test]
fn a_code_point_outside_the_table_takes_one_em() {
    assert_eq!(advance(Family::Sans, '中', 12.0), 12.0);
    assert_eq!(advance(Family::Mono, '\u{100}', 12.0), 12.0);
}

#[test]
fn a_code_point_of_the_table_with_no_glyph_takes_no_room() {
    assert_eq!(advance(Family::Sans, '\u{7f}', 12.0), 0.0);
    assert_eq!(advance(Family::Serif, '\u{9f}', 12.0), 0.0);
}

#[test]
fn a_code_point_of_the_table_takes_its_advance() {
    assert!(advance(Family::Sans, '\u{ff}', 12.0) > 0.0);
    assert!(advance(Family::Sans, 'A', 12.0) > 0.0);
}
