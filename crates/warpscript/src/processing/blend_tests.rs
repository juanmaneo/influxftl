// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the blend modes of a pixel and of the copy of a rectangle between images: the alpha
//! cases of `BLEND`, the modes that combine colours, and the rectangles with no area. The values
//! are worked out by hand from the formulas in `blend.rs`.

use super::blit;
use crate::processing::Blend;
use crate::processing::image::{Image, Rect};

fn rect(x: i64, y: i64, width: i64, height: i64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn dodge_by_a_white_source_is_white() {
    assert_eq!(Blend::Dodge.mix(0xff10_2030, 0xffff_ffff), 0xffff_ffff);
}

#[test]
fn blend_keeps_the_destination_under_a_clear_source_and_mixes_a_partly_clear_one() {
    assert_eq!(Blend::Blend.mix(0xff10_2030, 0x00ff_ffff), 0xff10_2030);
    assert_eq!(Blend::Blend.mix(0xff00_00ff, 0x80ff_0000), 0xff80_007f);
}

#[test]
fn overlay_of_a_light_base_is_screened() {
    assert_eq!(Blend::Overlay.mix(0xffc8_c8c8, 0xff64_6464), 0xffbd_bdbd);
}

#[test]
fn blend_and_replace_give_the_source_channel_when_their_channel_is_asked_for() {
    assert_eq!(Blend::Blend.channel(10, 200), 200);
    assert_eq!(Blend::Replace.channel(10, 200), 200);
}

#[test]
fn a_rectangle_with_no_width_or_no_height_copies_nothing() {
    let source = Image::from_pixels(2, 2, vec![0xffff_0000; 4]);
    let before = vec![0xff00_ff00; 4];
    for (from, to) in [
        (rect(0, 0, 0, 2), rect(0, 0, 2, 2)),
        (rect(0, 0, 2, 0), rect(0, 0, 2, 2)),
        (rect(0, 0, 2, 2), rect(0, 0, 0, 2)),
        (rect(0, 0, 2, 2), rect(0, 0, 2, 0)),
    ] {
        let mut destination = Image::from_pixels(2, 2, before.clone());
        blit(&mut destination, &source, from, to, Blend::Replace);
        assert_eq!(destination.pixels, before, "from {from:?} to {to:?}");
    }
}

#[test]
fn a_copy_samples_the_source_by_nearest_neighbour_and_skips_what_is_outside_it() {
    let source = Image::from_pixels(2, 2, vec![1, 2, 3, 4]);
    let mut destination = Image::new(2, 2).expect("a small image fits in memory");
    blit(
        &mut destination,
        &source,
        rect(1, 1, 2, 2),
        rect(0, 0, 2, 2),
        Blend::Replace,
    );
    assert_eq!(destination.pixels, vec![4, 0, 0, 0]);
}
