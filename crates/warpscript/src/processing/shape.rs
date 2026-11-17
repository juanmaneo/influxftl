// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! `PSHAPE`: a shape, read from SVG (`loadShape`) and drawn with `shape`.

/// A shape: its size in its own units, the origin of its drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// The width of the shape.
    pub width: f32,
    /// The height of the shape.
    pub height: f32,
}
