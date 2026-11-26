// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the image filters on images of a few pixels. The names and the parameter rules are
//! the ones `Pfilter` takes. The blur, erosion and dilation values are worked out by hand from the
//! rules the doc comments of `filter.rs` give, not from Warp 10.

use super::{Kind, apply};
use crate::error::Error;
use crate::processing::image::{Image, argb};

/// An opaque grey pixel, whose luminance is `level` itself.
fn grey(level: u8) -> u32 {
    argb(255, level, level, level)
}

/// A black 3 by 3 image with a white pixel in the middle.
fn white_in_black() -> Image {
    let (black, white) = (grey(0), grey(255));
    Image::from_pixels(
        3,
        3,
        vec![
            black, black, black, black, white, black, black, black, black,
        ],
    )
}

#[test]
fn each_kind_has_the_name_a_script_spells() {
    for (kind, name) in [
        (Kind::Threshold, "THRESHOLD"),
        (Kind::Gray, "GRAY"),
        (Kind::Opaque, "OPAQUE"),
        (Kind::Invert, "INVERT"),
        (Kind::Posterize, "POSTERIZE"),
        (Kind::Blur, "BLUR"),
        (Kind::Erode, "ERODE"),
        (Kind::Dilate, "DILATE"),
    ] {
        assert_eq!(kind.name(), name);
    }
}

#[test]
fn a_kind_that_takes_no_parameter_refuses_one() {
    for kind in [
        Kind::Gray,
        Kind::Opaque,
        Kind::Invert,
        Kind::Erode,
        Kind::Dilate,
    ] {
        let mut image = Image::from_pixels(1, 1, vec![grey(128)]);
        let err = apply(&mut image, kind, Some(1.0)).expect_err("a parameter is refused");
        let name = kind.name();
        assert_eq!(
            err.to_string(),
            format!("Use filter({name}) instead of filter({name}, param)")
        );
    }
}

#[test]
fn a_parameter_that_is_not_finite_is_refused() {
    for param in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut image = Image::from_pixels(1, 1, vec![grey(128)]);
        let err = apply(&mut image, Kind::Blur, Some(param)).expect_err("not finite");
        assert!(
            matches!(err, Error::NotFinite(_)),
            "unexpected error: {err}"
        );
    }
}

#[test]
fn a_threshold_whose_level_overflows_is_refused() {
    let mut image = Image::from_pixels(1, 1, vec![grey(128)]);
    let err = apply(&mut image, Kind::Threshold, Some(1e37)).expect_err("level * 255 overflows");
    assert!(
        matches!(err, Error::NotFinite(_)),
        "unexpected error: {err}"
    );
}

#[test]
fn blur_averages_the_rows_then_the_columns_of_the_pixels_in_reach() {
    let mut image = white_in_black();
    apply(&mut image, Kind::Blur, None).expect("the blur");
    // The middle row averages to 127, 85 and 127 (rounded down), and the others to black. Each
    // column of those averages is then averaged again: 63, 42 and 63 at the top and bottom, 42,
    // 28 and 42 in the middle.
    let levels: [u8; 9] = [63, 42, 63, 42, 28, 42, 63, 42, 63];
    assert_eq!(image.pixels, levels.map(grey));
}

#[test]
fn a_radius_of_two_reaches_the_whole_of_a_three_by_three_image() {
    let mut image = white_in_black();
    apply(&mut image, Kind::Blur, Some(2.0)).expect("the blur");
    // Every pixel is in reach of every other: the row averages are 0, 85 and 0, and their
    // average is 28.
    assert_eq!(image.pixels, vec![grey(28); 9]);
}

#[test]
fn blur_without_a_parameter_has_a_radius_of_one_and_a_radius_below_one_does_nothing() {
    let original = white_in_black();

    let mut by_default = original.clone();
    apply(&mut by_default, Kind::Blur, None).expect("the blur");
    let mut radius_one = original.clone();
    apply(&mut radius_one, Kind::Blur, Some(1.0)).expect("the blur");
    assert_ne!(by_default, original);
    assert_eq!(by_default, radius_one);

    for radius in [0.0, 0.5, -1.0] {
        let mut image = original.clone();
        apply(&mut image, Kind::Blur, Some(radius)).expect("the blur");
        assert_eq!(image, original, "radius {radius}");
    }
}

#[test]
fn erode_keeps_the_darkest_of_a_pixel_and_its_neighbours() {
    // The luminance of a grey pixel is its level, so the row is 200, 100 and 50.
    let mut row = Image::from_pixels(3, 1, vec![grey(200), grey(100), grey(50)]);
    apply(&mut row, Kind::Erode, None).expect("erode");
    assert_eq!(row.pixels, vec![grey(100), grey(50), grey(50)]);
}

#[test]
fn dilate_keeps_the_lightest_of_a_pixel_and_its_neighbours() {
    let mut row = Image::from_pixels(3, 1, vec![grey(200), grey(100), grey(50)]);
    apply(&mut row, Kind::Dilate, None).expect("dilate");
    assert_eq!(row.pixels, vec![grey(200), grey(200), grey(100)]);
}

#[test]
fn erode_takes_the_neighbours_above_and_below_but_not_the_diagonal_ones() {
    // A dark pixel at the top middle of a 3 by 3 image. The original pixels are read throughout,
    // so the pixel below the dark one is not darkened again.
    let mut image = Image::from_pixels(
        3,
        3,
        vec![
            grey(200),
            grey(50),
            grey(200),
            grey(200),
            grey(200),
            grey(200),
            grey(200),
            grey(200),
            grey(200),
        ],
    );
    apply(&mut image, Kind::Erode, None).expect("erode");
    assert_eq!(
        image.pixels,
        vec![
            grey(50),
            grey(50),
            grey(50),
            grey(200),
            grey(50),
            grey(200),
            grey(200),
            grey(200),
            grey(200),
        ]
    );
}

#[test]
fn dilate_takes_the_neighbours_above_and_below_but_not_the_diagonal_ones() {
    // A light pixel in the middle of a 3 by 3 image.
    let mut image = Image::from_pixels(
        3,
        3,
        vec![
            grey(50),
            grey(50),
            grey(50),
            grey(50),
            grey(250),
            grey(50),
            grey(50),
            grey(50),
            grey(50),
        ],
    );
    apply(&mut image, Kind::Dilate, None).expect("dilate");
    assert_eq!(
        image.pixels,
        vec![
            grey(50),
            grey(250),
            grey(50),
            grey(250),
            grey(250),
            grey(250),
            grey(50),
            grey(250),
            grey(50),
        ]
    );
}

#[test]
fn a_pixel_wins_a_tie_with_its_neighbour() {
    // Black and a blue of 1 both have luminance 0, so neither is darker or lighter than the other.
    let (black, blue) = (argb(255, 0, 0, 0), argb(255, 0, 0, 1));
    for kind in [Kind::Erode, Kind::Dilate] {
        let mut row = Image::from_pixels(2, 1, vec![black, blue]);
        apply(&mut row, kind, None).expect("the filter");
        assert_eq!(row.pixels, vec![black, blue], "{}", kind.name());
    }
}
