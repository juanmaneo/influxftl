// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of drawing an image on a context: the empty and zero-sized images, the corners of the
//! `CORNERS` mode, a transform that flattens the plane, the clip, the tint, and the refusal of a
//! position or a scale that overflows. The pixels are worked out by hand from `raster.rs`.

use super::{scaled, tinted};
use crate::processing::geometry::Transform;
use crate::processing::image::Image;
use crate::processing::paint::Clip;
use crate::processing::{Graphics, Mode};

const RED: u32 = 0xffff_0000;
const GREEN: u32 = 0xff00_ff00;
const BLUE: u32 = 0xff00_00ff;
const WHITE: u32 = 0xffff_ffff;

fn context(size: usize) -> Graphics {
    Graphics::new(size, size, false).expect("a small context fits in memory")
}

fn source() -> Image {
    Image::from_pixels(2, 2, vec![RED, GREEN, BLUE, WHITE])
}

#[test]
fn an_image_drawn_at_its_own_size_is_copied_pixel_for_pixel() {
    let mut g = context(2);
    g.draw_image(&source(), 0.0, 0.0, None).expect("finite");
    assert_eq!(g.image.pixels, vec![RED, GREEN, BLUE, WHITE]);
}

#[test]
fn an_empty_image_or_a_rectangle_with_no_width_draws_nothing() {
    let mut g = context(2);
    g.draw_image(&Image::from_pixels(0, 2, Vec::new()), 0.0, 0.0, None)
        .expect("nothing to draw");
    g.draw_image(&source(), 0.0, 0.0, Some([0.0, 2.0]))
        .expect("nothing to draw");
    assert_eq!(g.image.pixels, vec![0; 4]);
}

#[test]
fn corners_whose_size_overflows_are_refused_in_either_axis() {
    let mut g = context(2);
    g.style.image_mode = Mode::Corners;
    assert!(
        g.draw_image(&source(), f32::MAX, 0.0, Some([-f32::MAX, 2.0]))
            .is_err()
    );
    assert!(
        g.draw_image(&source(), 0.0, f32::MAX, Some([2.0, -f32::MAX]))
            .is_err()
    );
}

#[test]
fn a_transform_that_flattens_the_plane_draws_nothing() {
    let mut g = context(2);
    g.transform = Transform::scale(0.0, 1.0, 1.0);
    g.draw_image(&source(), 0.0, 0.0, Some([2.0, 2.0]))
        .expect("a flat transform draws nothing");
    assert_eq!(g.image.pixels, vec![0; 4]);
}

#[test]
fn a_transform_whose_determinant_overflows_is_refused() {
    let mut g = context(2);
    g.transform = Transform::scale(f32::MAX, f32::MAX, 1.0);
    assert!(g.draw_image(&source(), 0.0, 0.0, Some([2.0, 2.0])).is_err());
}

#[test]
fn a_device_corner_that_overflows_is_refused() {
    let mut g = context(2);
    g.transform = Transform::scale(f32::MAX, 1.0, 1.0);
    assert!(g.draw_image(&source(), 2.0, 0.0, Some([2.0, 2.0])).is_err());
}

#[test]
fn a_pixel_centre_whose_inverse_overflows_is_refused_in_each_axis() {
    let mut wide = context(4);
    wide.transform = Transform::scale(1e-38, 1.0, 1.0);
    assert!(
        wide.draw_image(&source(), 0.0, 0.0, Some([f32::MAX, 1.0]))
            .is_err()
    );

    let mut tall = context(4);
    tall.transform = Transform::scale(1.0, 1e-38, 1.0);
    assert!(
        tall.draw_image(&source(), 0.0, 0.0, Some([1.0, f32::MAX]))
            .is_err()
    );
}

#[test]
fn a_position_whose_fraction_across_the_image_overflows_is_refused_in_each_axis() {
    let mut g = context(2);
    assert!(
        g.draw_image(&source(), 0.0, 0.0, Some([1e-39, 1.0]))
            .is_err()
    );
    assert!(
        g.draw_image(&source(), 0.0, 0.0, Some([1.0, 1e-39]))
            .is_err()
    );
}

#[test]
fn a_clip_limits_the_pixels_an_image_paints() {
    let mut g = context(2);
    g.clip = Some(Clip::between([0.0, 0.0], [1.0, 2.0]));
    g.draw_image(&source(), 0.0, 0.0, None).expect("finite");
    assert_eq!(g.image.pixels, vec![RED, 0, BLUE, 0]);
}

#[test]
fn a_tint_multiplies_each_channel_of_the_pixels_drawn() {
    let mut g = context(2);
    g.style.tint = Some(0x8080_8080);
    g.draw_image(&source(), 0.0, 0.0, None).expect("finite");
    assert_eq!(
        g.image.pixels,
        vec![0x8080_0000, 0x8000_8000, 0x8000_0080, 0x8080_8080]
    );
}

#[test]
fn a_tint_scales_each_channel_by_the_same_channel_out_of_255() {
    assert_eq!(tinted(RED, Some(0x8080_8080)), 0x8080_0000);
    assert_eq!(tinted(RED, None), RED);
    assert_eq!(scaled(0x40, 0x80), 0x20);
}
