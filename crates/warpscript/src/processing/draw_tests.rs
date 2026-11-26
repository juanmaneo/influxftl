// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of drawing an outline on a small context: the fill, the stroke snapped to the pixel grid,
//! the dots, the clip, and the refusal of device coordinates that overflow. The pixels are worked
//! out by hand from the rules of `draw.rs` and `path.rs`.

use super::{Outline, pixel_ring, snap};
use crate::processing::Graphics;
use crate::processing::geometry::Transform;
use crate::processing::paint::Clip;
use crate::processing::path::Subpath;

const BLACK: u32 = 0xff00_0000;
const WHITE: u32 = 0xffff_ffff;

fn context(size: usize) -> Graphics {
    Graphics::new(size, size, false).expect("a small context fits in memory")
}

fn stroked(points: Vec<[f32; 2]>) -> Outline {
    Outline {
        stroke: vec![Subpath {
            points,
            closed: false,
        }],
        ..Outline::default()
    }
}

#[test]
fn a_filled_ring_covers_the_pixels_whose_centres_it_holds() {
    let mut g = context(4);
    g.style.stroke = None;
    let ring = vec![[1.0, 1.0], [3.0, 1.0], [3.0, 3.0], [1.0, 3.0]];
    g.draw(&Outline::closed(ring)).expect("finite");
    for y in 0..4 {
        for x in 0..4 {
            let inside = (1..3).contains(&x) && (1..3).contains(&y);
            let expected = if inside { WHITE } else { 0 };
            assert_eq!(g.image.get(x, y), expected, "pixel ({x}, {y})");
        }
    }
}

#[test]
fn a_thin_point_lights_the_one_pixel_it_falls_on() {
    let mut g = context(4);
    g.style.fill = None;
    g.draw(&Outline {
        points: vec![[2.3, 1.2]],
        ..Outline::default()
    })
    .expect("finite");
    assert_eq!(g.image.get(2, 1), BLACK);
    assert_eq!(g.image.pixels.iter().filter(|&&p| p != 0).count(), 1);
}

#[test]
fn a_thin_point_is_the_pixel_whose_centre_is_nearest_with_halves_up() {
    assert_eq!(
        pixel_ring(2.3, 1.2),
        vec![[2.0, 1.0], [3.0, 1.0], [3.0, 2.0], [2.0, 2.0]]
    );
    assert_eq!(pixel_ring(2.5, 0.0)[0], [3.0, 0.0]);
    assert_eq!(pixel_ring(f32::MAX, 0.0)[0], [f32::MAX, 0.0]);
}

#[test]
fn a_stroke_coordinate_snaps_to_a_pixel_centre_when_odd_and_to_an_edge_when_even() {
    assert_eq!(snap(2.4, true), 2.5);
    assert_eq!(snap(2.5, true), 3.5);
    assert_eq!(snap(2.5, false), 3.0);
    assert_eq!(snap(f32::MAX, true), f32::MAX);
}

#[test]
fn a_line_of_weight_one_is_drawn_on_the_pixel_row_of_its_centre() {
    let mut g = context(5);
    g.style.fill = None;
    g.draw(&stroked(vec![[0.0, 1.0], [4.0, 1.0]]))
        .expect("finite");
    assert_eq!(
        [g.image.get(1, 0), g.image.get(1, 1), g.image.get(1, 2)],
        [0, BLACK, 0]
    );
}

#[test]
fn a_line_of_even_weight_is_drawn_on_the_two_rows_either_side_of_its_edge() {
    let mut g = context(5);
    g.style.fill = None;
    g.style.stroke_weight = 2.0;
    g.draw(&stroked(vec![[0.0, 1.0], [4.0, 1.0]]))
        .expect("finite");
    assert_eq!(
        [g.image.get(1, 0), g.image.get(1, 1), g.image.get(1, 2)],
        [BLACK, BLACK, 0]
    );
}

#[test]
fn a_point_heavier_than_one_pixel_is_drawn_as_a_disc() {
    let mut g = context(4);
    g.style.fill = None;
    g.style.stroke_weight = 3.0;
    g.draw(&Outline {
        points: vec![[2.5, 1.5]],
        ..Outline::default()
    })
    .expect("finite");
    assert_eq!(g.image.get(2, 1), BLACK);
    assert_eq!(g.image.get(0, 0), 0);
}

#[test]
fn a_clip_limits_the_pixels_a_fill_paints() {
    let mut g = context(4);
    g.style.stroke = None;
    g.clip = Some(Clip::between([0.0, 0.0], [2.0, 4.0]));
    let square = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
    g.draw(&Outline::closed(square)).expect("finite");
    assert_eq!(g.image.get(0, 0), WHITE);
    assert_eq!(g.image.get(1, 3), WHITE);
    assert_eq!(g.image.get(2, 0), 0);
    assert_eq!(g.image.get(3, 3), 0);
}

#[test]
fn a_fill_whose_device_points_overflow_is_refused() {
    let mut g = context(4);
    g.style.stroke = None;
    g.transform = Transform::scale(f32::MAX, 1.0, 1.0);
    let square = vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
    assert!(g.draw(&Outline::closed(square)).is_err());
}

#[test]
fn a_fill_whose_coverage_overflows_is_refused() {
    let mut g = context(2);
    g.style.stroke = None;
    let ring = vec![[-3e38, -3e38], [3e38, -1.0], [0.0, 3e38]];
    assert!(g.draw(&Outline::closed(ring)).is_err());
}

#[test]
fn a_stroke_with_a_nan_weight_is_refused() {
    let mut g = context(4);
    g.style.fill = None;
    g.style.stroke_weight = f32::NAN;
    assert!(g.draw(&stroked(vec![[0.0, 0.0], [2.0, 0.0]])).is_err());
}

#[test]
fn a_stroke_under_a_transform_whose_plane_factor_overflows_is_refused() {
    let mut g = context(4);
    g.style.fill = None;
    g.transform = Transform::scale(f32::MAX, f32::MAX, 1.0);
    assert!(g.draw(&stroked(vec![[0.0, 0.0], [2.0, 0.0]])).is_err());
}

#[test]
fn a_stroke_whose_device_points_overflow_is_refused() {
    let mut g = context(4);
    g.style.fill = None;
    g.transform = Transform::scale(f32::MAX, 1.0, 1.0);
    assert!(g.draw(&stroked(vec![[0.0, 0.0], [2.0, 0.0]])).is_err());
}

#[test]
fn a_stroke_whose_direction_overflows_is_refused() {
    let mut g = context(4);
    g.style.fill = None;
    assert!(g.draw(&stroked(vec![[-3e38, 0.0], [3e38, 0.0]])).is_err());
}

#[test]
fn a_stroke_whose_rings_overflow_their_coverage_is_refused() {
    let mut g = context(2);
    g.style.fill = None;
    g.style.stroke_weight = 2e38;
    assert!(g.draw(&stroked(vec![[0.0, 0.0], [1.0, 1.0]])).is_err());
}

#[test]
fn a_dot_whose_device_point_overflows_is_refused() {
    let mut g = context(4);
    g.style.fill = None;
    g.transform = Transform::scale(f32::MAX, 1.0, 1.0);
    let dot = Outline {
        points: vec![[2.0, 0.0]],
        ..Outline::default()
    };
    assert!(g.draw(&dot).is_err());
}

#[test]
fn a_disc_whose_centre_overflows_is_refused() {
    let mut g = context(4);
    g.style.fill = None;
    g.style.stroke_weight = 2e38;
    let dot = Outline {
        points: vec![[3e38, 0.0]],
        ..Outline::default()
    };
    assert!(g.draw(&dot).is_err());
}
