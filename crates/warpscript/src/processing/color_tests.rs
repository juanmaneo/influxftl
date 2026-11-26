// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the colour modes at the edges of their rules: the HSB sectors of a hue, the hue of a
//! colour whose green is the largest channel, the refusal of a maximum that is not positive, and
//! the interpolation of two colours. The values are worked out by hand from the conversions in
//! `color.rs`.

use super::{ColorMode, Component, Space};
use crate::error::processing::ProcessingError;

#[test]
fn each_sector_of_a_hsb_hue_gives_its_colour() {
    let hsb = ColorMode::new(Space::Hsb, 1.0, 1.0, 1.0, 1.0).expect("finite positive maxima");
    assert_eq!(hsb.calc(0.25, 1.0, 1.0, 1.0), 0xff7f_ff00);
    assert_eq!(hsb.calc(0.75, 1.0, 1.0, 1.0), 0xff7f_00ff);
    assert_eq!(hsb.calc(0.875, 1.0, 1.0, 1.0), 0xffff_00bf);
}

#[test]
fn the_hue_of_a_colour_whose_green_is_the_largest_channel_is_read_from_the_green_branch() {
    let rgb = ColorMode::default();
    assert_eq!(rgb.component(0xff00_ffff, Component::Hue), 127.5);
    let hue = rgb.component(0xffff_0080, Component::Hue);
    assert!((hue - 233.6667).abs() < 0.01, "hue {hue}");
}

#[test]
fn a_maximum_that_is_nan_or_not_positive_is_refused() {
    assert!(ColorMode::new(Space::Rgb, f32::NAN, 255.0, 255.0, 255.0).is_err());
    assert_eq!(
        ColorMode::new(Space::Rgb, 0.0, 255.0, 255.0, 255.0),
        Err(ProcessingError::NonPositiveMaximum.into())
    );
    for maxima in [
        [255.0, 0.0, 255.0, 255.0],
        [255.0, 255.0, 0.0, 255.0],
        [255.0, 255.0, 255.0, 0.0],
        [255.0, 255.0, 255.0, -1.0],
    ] {
        let [max1, max2, max3, max_alpha] = maxima;
        assert_eq!(
            ColorMode::new(Space::Rgb, max1, max2, max3, max_alpha),
            Err(ProcessingError::NonPositiveMaximum.into()),
            "{maxima:?}"
        );
    }
}

#[test]
fn lerp_halfway_between_black_and_white_is_the_middle_gray() {
    let rgb = ColorMode::default();
    assert_eq!(rgb.lerp(0xff00_0000, 0xffff_ffff, 0.5), 0xff80_8080);
}
