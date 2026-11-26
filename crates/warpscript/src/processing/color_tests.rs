// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the colour modes at the edges of their rules: the HSB sectors of a hue, the hue of a
//! colour whose green is the largest channel, the refusal of a zero maximum, and the interpolation
//! of two colours. The values are worked out by hand from the conversions in `color.rs`.

use super::{ColorMode, Component, Space};
use crate::error::processing::ProcessingError;

fn rgb_with(max1: f32, max2: f32, max3: f32, max_alpha: f32) -> ColorMode {
    ColorMode {
        space: Space::Rgb,
        max1,
        max2,
        max3,
        max_alpha,
    }
}

#[test]
fn each_sector_of_a_hsb_hue_gives_its_colour() {
    let hsb = ColorMode::new(Space::Hsb, 1.0, 1.0, 1.0, 1.0).expect("finite positive maxima");
    assert_eq!(hsb.calc(0.25, 1.0, 1.0, 1.0).expect("finite"), 0xff7f_ff00);
    assert_eq!(hsb.calc(0.75, 1.0, 1.0, 1.0).expect("finite"), 0xff7f_00ff);
    assert_eq!(hsb.calc(0.875, 1.0, 1.0, 1.0).expect("finite"), 0xffff_00bf);
}

#[test]
fn the_hue_of_a_colour_whose_green_is_the_largest_channel_is_read_from_the_green_branch() {
    let rgb = ColorMode::default();
    assert_eq!(
        rgb.component(0xff00_ffff, Component::Hue).expect("finite"),
        127.5
    );
    let hue = rgb.component(0xffff_0080, Component::Hue).expect("finite");
    assert!((hue - 233.6667).abs() < 0.01, "hue {hue}");
}

#[test]
fn a_maximum_that_is_nan_or_not_positive_is_refused() {
    assert!(ColorMode::new(Space::Rgb, f32::NAN, 255.0, 255.0, 255.0).is_err());
    assert_eq!(
        ColorMode::new(Space::Rgb, 0.0, 255.0, 255.0, 255.0),
        Err(ProcessingError::NonPositiveMaximum.into())
    );
}

#[test]
fn a_zero_maximum_refuses_each_value_that_is_divided_by_it() {
    for mode in [
        rgb_with(0.0, 255.0, 255.0, 255.0),
        rgb_with(255.0, 0.0, 255.0, 255.0),
        rgb_with(255.0, 255.0, 0.0, 255.0),
        rgb_with(255.0, 255.0, 255.0, 0.0),
    ] {
        assert!(mode.calc(0.0, 0.0, 0.0, 0.0).is_err(), "{mode:?}");
    }
    assert!(rgb_with(0.0, 255.0, 255.0, 255.0).gray(0.0, 0.0).is_err());
    assert!(rgb_with(255.0, 255.0, 255.0, 0.0).gray(0.0, 0.0).is_err());
}

#[test]
fn lerp_halfway_between_black_and_white_is_the_middle_gray() {
    let rgb = ColorMode::default();
    assert_eq!(
        rgb.lerp(0xff00_0000, 0xffff_ffff, 0.5).expect("finite"),
        0xff80_8080
    );
    assert!(rgb.lerp(0xff00_0000, 0xffff_ffff, f32::NAN).is_err());
}
