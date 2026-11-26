// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Unit tests of the placement of a shape. Its coordinates overflow only for the values a script
//! can give, so the refusals are checked here on the mapping itself (see `DIVERGENCES.md`
//! section 5.4 for the cases Warp 10 accepts).

use super::{Shape, map_outline, map_point};
use crate::processing::draw::Outline;
use crate::processing::path::Subpath;

fn refusal<T: std::fmt::Debug>(result: crate::error::Result<T>) -> String {
    result
        .expect_err("the coordinates should overflow")
        .to_string()
}

#[test]
fn a_scale_whose_factor_overflows_is_refused() {
    let no_width = Shape {
        width: 0.0,
        height: 4.0,
        pieces: Vec::new(),
    };
    assert!(refusal(no_width.scale(Some((2.0, 2.0)))).contains("not a finite number"));
    let no_height = Shape {
        width: 4.0,
        height: 0.0,
        pieces: Vec::new(),
    };
    assert!(refusal(no_height.scale(Some((2.0, 2.0)))).contains("not a finite number"));
    assert_eq!(
        no_height.scale(None).expect("the shape keeps its size"),
        (1.0, 1.0)
    );
}

#[test]
fn a_placed_coordinate_that_overflows_is_refused_on_either_axis() {
    let across = refusal(map_point([4.0, 0.0], (1e38, 1.0), [3e38, 0.0]));
    assert!(across.contains("not a finite number"), "{across}");
    let down = refusal(map_point([0.0, 4.0], (1.0, 1e38), [0.0, 3e38]));
    assert!(down.contains("not a finite number"), "{down}");
}

#[test]
fn a_point_of_an_outline_that_overflows_is_refused() {
    let dots = Outline {
        points: vec![[3e38, 0.0]],
        ..Outline::default()
    };
    let message = refusal(map_outline(&dots, (4.0, 1.0), [0.0, 0.0]));
    assert!(message.contains("not a finite number"), "{message}");
}

#[test]
fn a_stroke_point_that_overflows_is_refused() {
    let stroked = Outline {
        stroke: vec![Subpath {
            points: vec![[3e38, 0.0]],
            closed: false,
        }],
        ..Outline::default()
    };
    let message = refusal(map_outline(&stroked, (4.0, 1.0), [0.0, 0.0]));
    assert!(message.contains("not a finite number"), "{message}");
}
