// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the clipping rectangle: a pixel is inside when its centre is, and the corners may be
//! given in either order.

use super::Clip;

#[test]
fn a_whole_pixel_range_holds_the_pixels_from_its_first_to_its_last() {
    let clip = Clip::between([0.0, 0.0], [10.0, 10.0]);
    assert!(clip.contains(0, 0));
    assert!(clip.contains(9, 9));
    assert!(!clip.contains(-1, 0));
    assert!(!clip.contains(0, -1));
    assert!(!clip.contains(10, 0));
    assert!(!clip.contains(0, 10));
}

#[test]
fn a_clip_with_fractional_edges_holds_the_pixels_whose_centres_it_holds() {
    let clip = Clip::between([2.5, 2.5], [0.5, 0.5]);
    assert!(clip.contains(0, 0));
    assert!(!clip.contains(-1, 0));
    assert!(clip.contains(1, 1));
    assert!(!clip.contains(2, 2));
}
