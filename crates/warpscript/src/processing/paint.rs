// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The clipping rectangle of a graphics context (`clip`, `noClip`).

/// The rectangle drawings are limited to, in pixels: a pixel is inside when its centre is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clip {
    /// The left edge.
    pub x0: f32,
    /// The top edge.
    pub y0: f32,
    /// The right edge (excluded).
    pub x1: f32,
    /// The bottom edge (excluded).
    pub y1: f32,
}

impl Clip {
    /// The rectangle between two of its corners, given in any order. The corners are finite (the
    /// caller checked them), so their order is the only thing to settle.
    pub fn between(a: [f32; 2], b: [f32; 2]) -> Self {
        Clip {
            x0: a[0].min(b[0]),
            y0: a[1].min(b[1]),
            x1: a[0].max(b[0]),
            y1: a[1].max(b[1]),
        }
    }

    /// The part of this rectangle that `other` also covers: `clip` intersects with the clip
    /// already set, as Java2D does. Disjoint rectangles give an empty one, which no pixel is in.
    #[must_use]
    pub fn intersect(&self, other: &Clip) -> Clip {
        Clip {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        }
    }

    /// Whether the pixel at (`x`, `y`) lies inside the rectangle.
    pub fn contains(&self, x: i64, y: i64) -> bool {
        let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
        cx >= self.x0 && cx < self.x1 && cy >= self.y0 && cy < self.y1
    }
}
