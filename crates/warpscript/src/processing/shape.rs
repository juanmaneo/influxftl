// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! A shape (`PSHAPE`): the pieces read from an SVG document, each with the paint of the element it
//! came from, drawn at a place and, when given, scaled to a size.

use super::Graphics;
use super::draw::Outline;
use super::finite;
use super::path::Subpath;
use crate::error::Result;

/// A shape: its size in its own units, and its pieces in drawing order.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// The width of the shape, in its own units.
    pub width: f32,
    /// The height of the shape, in its own units.
    pub height: f32,
    /// The pieces, each drawn with its own paint.
    pub pieces: Vec<Piece>,
}

/// A part of a shape, with the paint of the element it was read from. `None` is no paint.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    /// The geometry, in the units of the shape.
    pub outline: Outline,
    /// The fill colour (ARGB), if the element is filled.
    pub fill: Option<u32>,
    /// The stroke colour (ARGB), if the element is stroked.
    pub stroke: Option<u32>,
    /// The stroke weight of the element.
    pub stroke_weight: f32,
}

/// Where a shape is drawn: its origin in the script, and the size it is scaled to, if one is given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// The horizontal coordinate of the origin.
    pub x: f32,
    /// The vertical coordinate of the origin.
    pub y: f32,
    /// The width and height the shape is scaled to, or `None` for its own size.
    pub size: Option<(f32, f32)>,
}

/// The factors that take a coordinate of the shape to a coordinate of the script.
pub(super) type Scale = (f32, f32);

impl Shape {
    /// The factors that scale the shape to `size`; without a size, the shape keeps its own.
    fn scale(&self, size: Option<(f32, f32)>) -> Result<Scale> {
        match size {
            None => Ok((1.0, 1.0)),
            Some((width, height)) => {
                Ok((finite(width / self.width)?, finite(height / self.height)?))
            }
        }
    }
}

/// A point of the shape moved to the script: scaled, then moved by the origin.
fn map_point(
    [x, y]: [f32; 2],
    (scale_x, scale_y): Scale,
    [origin_x, origin_y]: [f32; 2],
) -> Result<[f32; 2]> {
    Ok([
        finite(origin_x + x * scale_x)?,
        finite(origin_y + y * scale_y)?,
    ])
}

/// An outline of the shape moved to the script, point by point.
pub(super) fn map_outline(outline: &Outline, scale: Scale, origin: [f32; 2]) -> Result<Outline> {
    let map = |points: &[[f32; 2]]| -> Result<Vec<[f32; 2]>> {
        points
            .iter()
            .map(|&p| map_point(p, scale, origin))
            .collect()
    };
    let fill = outline
        .fill
        .iter()
        .map(|ring| map(ring))
        .collect::<Result<Vec<_>>>()?;
    let stroke = outline
        .stroke
        .iter()
        .map(|subpath| {
            Ok(Subpath {
                points: map(&subpath.points)?,
                closed: subpath.closed,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Outline {
        fill,
        stroke,
        points: map(&outline.points)?,
    })
}

impl Graphics {
    /// Draw `shape` at `placement`, each piece with its own paint; the paint of the context is
    /// restored afterwards (`shape`).
    pub fn draw_shape(&mut self, shape: &Shape, placement: Placement) -> Result<()> {
        let scale = shape.scale(placement.size)?;
        let origin = [placement.x, placement.y];
        let saved = (self.style.fill, self.style.stroke, self.style.stroke_weight);
        let drawn = shape.pieces.iter().try_for_each(|piece| -> Result<()> {
            self.style.fill = piece.fill;
            self.style.stroke = piece.stroke;
            self.style.stroke_weight = piece.stroke_weight;
            let outline = map_outline(&piece.outline, scale, origin)?;
            self.draw(&outline)
        });
        (self.style.fill, self.style.stroke, self.style.stroke_weight) = saved;
        drawn
    }
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod shape_tests;
