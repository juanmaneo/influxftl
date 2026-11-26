// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Drawing an outline on a graphics context. An [`Outline`] is what a drawing function produces,
//! in the coordinates of the script. [`Graphics::draw`] moves it to the device under the current
//! transform, then paints its fill with the fill colour and its stroke and points with the stroke
//! colour. Each layer is one coverage painted once, so that a part covered twice is blended once.

use super::Graphics;
use super::finite;
use super::path::{Coverage, Ring, Subpath, cover, disc, stroke_outline};
use crate::error::Result;

/// The geometry a drawing function produces, in the coordinates of the script.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outline {
    /// The rings that are filled; a fill closes each ring by itself.
    pub fill: Vec<Ring>,
    /// The subpaths that are stroked, open or closed as they say.
    pub stroke: Vec<Subpath>,
    /// The points drawn as dots of the stroke weight (`point`, `POINTS`).
    pub points: Vec<[f32; 2]>,
}

impl Outline {
    /// A closed shape: its ring is filled, and its outline stroked.
    pub fn closed(ring: Ring) -> Self {
        Outline {
            fill: vec![ring.clone()],
            stroke: vec![Subpath {
                points: ring,
                closed: true,
            }],
            points: Vec::new(),
        }
    }
}

impl Outline {
    /// An open curve: a fill closes it with a chord, and a stroke leaves it open.
    pub fn open(points: Vec<[f32; 2]>) -> Self {
        Outline {
            fill: vec![points.clone()],
            stroke: vec![Subpath {
                points,
                closed: false,
            }],
            points: Vec::new(),
        }
    }
}

/// A stroke coordinate moved onto the pixel grid, as Java2D's normalising stroke does: a stroke of
/// odd width is centred on a pixel centre, one of even width lies on a pixel edge. Halves round
/// up.
fn snap(coordinate: f32, odd: bool) -> Result<f32> {
    let nearest = (coordinate + 0.5).floor();
    finite(if odd { nearest + 0.5 } else { nearest })
}

/// The pixel a point of a stroke weight of at most 1 lights: the one whose centre is nearest,
/// halves up, as a stroke of odd width is normalised onto the pixel grid.
fn pixel_ring(x: f32, y: f32) -> Result<Ring> {
    let (x, y) = (finite((x + 0.5).floor())?, finite((y + 0.5).floor())?);
    Ok(vec![[x, y], [x + 1.0, y], [x + 1.0, y + 1.0], [x, y + 1.0]])
}

impl Graphics {
    /// Draw `outline` with the transform, style and clip of this context.
    pub fn draw(&mut self, outline: &Outline) -> Result<()> {
        if let Some(colour) = self.style.fill
            && !outline.fill.is_empty()
        {
            let rings = outline
                .fill
                .iter()
                .map(|ring| {
                    ring.iter()
                        .map(|&p| self.device(p))
                        .collect::<Result<Ring>>()
                })
                .collect::<Result<Vec<Ring>>>()?;
            let coverage = cover(&rings, self.image.width, self.image.height)?;
            self.paint(&coverage, colour);
        }
        if let Some(colour) = self.style.stroke
            && (!outline.stroke.is_empty() || !outline.points.is_empty())
        {
            let rings = self.stroke_rings(outline)?;
            let coverage = cover(&rings, self.image.width, self.image.height)?;
            self.paint(&coverage, colour);
        }
        Ok(())
    }

    /// A point of the script moved to the device by the transform.
    fn device(&self, [x, y]: [f32; 2]) -> Result<[f32; 2]> {
        let [dx, dy, _] = self.transform.apply(x, y, 0.0)?;
        Ok([dx, dy])
    }

    /// The rings whose union is the stroke of `outline`, and the dots of its points, in the
    /// device. A weight of at most 1 draws a pixel for each point; a heavier weight draws a disc.
    /// The stroke of a weight of 0 is the thinnest line, one pixel wide.
    fn stroke_rings(&self, outline: &Outline) -> Result<Vec<Ring>> {
        let weight = self.style.stroke_weight;
        let odd = weight <= 0.0 || (weight.round() % 2.0) >= 1.0;
        let half = if weight <= 0.0 {
            0.5
        } else {
            finite(weight * self.transform.plane_scale()? / 2.0)?
        };
        let mut subpaths = Vec::with_capacity(outline.stroke.len());
        for subpath in &outline.stroke {
            let points = subpath
                .points
                .iter()
                .map(|&p| -> Result<[f32; 2]> {
                    let [x, y] = self.device(p)?;
                    Ok([snap(x, odd)?, snap(y, odd)?])
                })
                .collect::<Result<Vec<_>>>()?;
            subpaths.push(Subpath {
                points,
                closed: subpath.closed,
            });
        }
        let mut rings = stroke_outline(&subpaths, half, self.style.cap, self.style.join)?;
        for &p in &outline.points {
            let [x, y] = self.device(p)?;
            rings.push(if weight <= 1.0 {
                pixel_ring(x, y)?
            } else {
                disc([x, y], half)?
            });
        }
        Ok(rings)
    }

    /// Paint `colour` over each covered pixel that is inside the clip, blended with what is there.
    fn paint(&mut self, coverage: &Coverage, colour: u32) {
        let (blend, clip, columns) = (self.style.blend, self.clip, self.image.width);
        for (x, y) in coverage.pixels() {
            if clip.is_none_or(|clip| clip.contains(x as i64, y as i64)) {
                let index = y * columns + x;
                self.image.pixels[index] = blend.mix(self.image.pixels[index], colour);
            }
        }
    }
}

#[cfg(test)]
#[path = "draw_tests.rs"]
mod draw_tests;
