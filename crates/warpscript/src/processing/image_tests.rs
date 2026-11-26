// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the pixel operations of an image that the functions do not reach: an image or a
//! region too big for the memory is refused rather than aborted, a resize of an empty image is
//! transparent, and a paste is clipped at each edge of the image it goes to.

use super::{Image, Rect};
use crate::error::Error;
use crate::error::processing::ProcessingError;

#[test]
fn an_image_too_big_for_the_memory_is_refused_with_its_pixel_count() {
    assert!(matches!(
        Image::new(usize::MAX, 2),
        Err(Error::Processing(ProcessingError::OutOfMemory {
            pixels: usize::MAX,
            ..
        }))
    ));
}

#[test]
fn a_region_too_wide_for_the_memory_is_refused() {
    let image = Image::from_pixels(2, 2, vec![0; 4]);
    assert!(
        image
            .region(Rect {
                x: 0,
                y: 0,
                width: i64::MAX,
                height: 2,
            })
            .is_err()
    );
}

#[test]
fn a_resize_too_big_for_the_memory_is_refused() {
    assert!(
        Image::from_pixels(2, 2, vec![0; 4])
            .resized(usize::MAX, 2)
            .is_err()
    );
}

#[test]
fn a_resize_of_an_image_with_no_pixels_is_transparent() {
    assert_eq!(
        Image::from_pixels(0, 0, Vec::new())
            .resized(2, 2)
            .expect("a small image fits in memory"),
        Image::new(2, 2).expect("a small image fits in memory")
    );
}

#[test]
fn a_paste_past_the_left_and_top_edges_is_clipped() {
    let source = Image::from_pixels(2, 2, vec![1, 2, 3, 4]);
    let mut canvas = Image::new(3, 3).expect("a small image fits in memory");
    canvas.paste(&source, -1, 1);
    assert_eq!(
        canvas,
        Image::from_pixels(3, 3, vec![0, 0, 0, 2, 0, 0, 4, 0, 0])
    );
}

#[test]
fn a_paste_past_the_right_and_bottom_edges_is_clipped() {
    let source = Image::from_pixels(2, 2, vec![1, 2, 3, 4]);
    let mut canvas = Image::new(3, 3).expect("a small image fits in memory");
    canvas.paste(&source, 2, 2);
    assert_eq!(
        canvas,
        Image::from_pixels(3, 3, vec![0, 0, 0, 0, 0, 0, 0, 0, 1])
    );
}
