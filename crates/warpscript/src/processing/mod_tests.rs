// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the graphics context itself: the background taken from an image, the refusal of an
//! empty image and of a size the memory cannot hold, and the depth buffer of a `3D` context.

use super::Graphics;
use super::image::Image;
use crate::error::processing::ProcessingError;

#[test]
fn a_background_image_needs_pixels_to_take_them_from() {
    let mut g = Graphics::new(2, 2, false).expect("a small context fits in memory");
    assert_eq!(
        g.background_image(&Image::from_pixels(0, 0, Vec::new())),
        Err(ProcessingError::EmptyImage {
            width: 0,
            height: 0,
        }
        .into())
    );
}

#[test]
fn a_background_image_is_sampled_by_nearest_neighbour_and_resets_the_depth() {
    let mut graphics = Graphics::new(4, 4, true).expect("a small context fits in memory");
    graphics.depth.fill(0.0);
    let [a, b, c, d] = [0xff11_1111, 0xff22_2222, 0xff33_3333, 0xff44_4444];
    graphics
        .background_image(&Image::from_pixels(2, 2, vec![a, b, c, d]))
        .expect("a non-empty image");
    assert_eq!(
        graphics.image.pixels,
        vec![a, a, b, b, a, a, b, b, c, c, d, d, c, c, d, d]
    );
    assert_eq!(graphics.depth, vec![f32::MAX; 16]);
}

#[test]
fn a_resize_too_big_for_the_memory_leaves_the_context_as_it_was() {
    let mut g = Graphics::new(4, 4, true).expect("a small context fits in memory");
    assert!(g.resize(usize::MAX, 2).is_err());
    assert_eq!(g.image.width, 4);
}

#[test]
fn a_context_too_big_for_the_memory_is_refused() {
    assert!(Graphics::new(usize::MAX, 2, false).is_err());
}

#[test]
fn only_a_three_d_context_has_a_depth_buffer() {
    assert_eq!(
        Graphics::new(2, 2, true).expect("small").depth,
        vec![f32::MAX; 4]
    );
    assert_eq!(
        Graphics::new(2, 2, false).expect("small").depth,
        [] as [f32; 0]
    );
}
