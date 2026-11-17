// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Drawing an image on a graphics context (`image`, `Pimage`): the rectangle the image takes is
//! placed by the image mode, moved by the transform, clipped, tinted and blended into the pixels
//! under it. Each pixel of the context samples the image at the nearest pixel to its centre.
//!
//! A `3D` context draws images flat, in the plane of its transform: neither the perspective nor
//! the depth buffer applies to an image (see `DIVERGENCES.md`).

use super::image::{Image, argb, channels};
use super::{Graphics, Mode, finite};
use crate::error::Result;

impl Graphics {
    /// `image(source, x, y)` and `image(source, x, y, width, height)`: `source` drawn at (`x`, `y`)
    /// with the size `size`, or its own size when `None`. The position and the size are read by
    /// the image mode. An empty source, or a rectangle with no width or no height, draws nothing.
    pub fn draw_image(
        &mut self,
        source: &Image,
        x: f32,
        y: f32,
        size: Option<[f32; 2]>,
    ) -> Result<()> {
        if source.width == 0 || source.height == 0 {
            return Ok(());
        }
        // `CORNERS` without a second corner places the image as `CORNER` does.
        let mode = match (self.style.image_mode, size) {
            (Mode::Corners, None) => Mode::Corner,
            (mode, _) => mode,
        };
        let [extent_x, extent_y] = size.unwrap_or([source.width as f32, source.height as f32]);
        let (left, width) = place(mode, x, extent_x)?;
        let (top, height) = place(mode, y, extent_y)?;
        if width == 0.0 || height == 0.0 {
            return Ok(());
        }
        // Each device pixel centre maps back to the image plane through `inverse`, see
        // `Transform::inverse_plane`.
        let Some(inverse) = self.transform.inverse_plane()? else {
            return Ok(());
        };

        // The device pixels the image can cover: the bounding box of its transformed corners.
        let corners = [
            (left, top),
            (left + width, top),
            (left, top + height),
            (left + width, top + height),
        ];
        let mut device = [[0.0_f32; 2]; 4];
        for (slot, (cx, cy)) in device.iter_mut().zip(corners) {
            let [dx, dy, _] = self.transform.apply(cx, cy, 0.0)?;
            *slot = [dx, dy];
        }
        let min_x = device.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let max_x = device
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = device.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let max_y = device
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let (columns, rows) = (self.image.width, self.image.height);
        let (x_from, x_to) = (edge(min_x.floor(), columns), edge(max_x.ceil(), columns));
        let (y_from, y_to) = (edge(min_y.floor(), rows), edge(max_y.ceil(), rows));

        let clip = self.clip;
        let (blend, tint) = (self.style.blend, self.style.tint);
        let (source_width, source_height) = (source.width, source.height);
        for py in y_from..y_to {
            let cy = py as f32 + 0.5;
            for px in x_from..x_to {
                if !clip.is_none_or(|clip| clip.contains(px as i64, py as i64)) {
                    continue;
                }
                let cx = px as f32 + 0.5;
                let local_x = finite(inverse[0] * cx + inverse[1] * cy + inverse[2])?;
                let local_y = finite(inverse[3] * cx + inverse[4] * cy + inverse[5])?;
                let u = finite((local_x - left) / width)?;
                let v = finite((local_y - top) / height)?;
                if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                    continue;
                }
                let sx = ((u * source_width as f32) as usize).min(source_width - 1);
                let sy = ((v * source_height as f32) as usize).min(source_height - 1);
                let colour = tinted(source.pixels[sy * source_width + sx], tint);
                let index = py * columns + px;
                self.image.pixels[index] = blend.mix(self.image.pixels[index], colour);
            }
        }
        Ok(())
    }
}

/// The start and the length of an image on one axis, placed at `origin` with `extent` by `mode`
/// (`imageMode`). The extent is the size of the image, or for `CORNERS` the coordinate of its
/// second corner. `CENTER` puts `origin` at the middle; the rectangle rule of `RADIUS` (`origin`
/// at the middle, twice `extent` long) is kept so that the match is total, although `imageMode`
/// never sets it.
fn place(mode: Mode, origin: f32, extent: f32) -> Result<(f32, f32)> {
    Ok(match mode {
        Mode::Corner => (origin, extent),
        Mode::Corners => (origin, finite(extent - origin)?),
        Mode::Center => (finite(origin - extent / 2.0)?, extent),
        Mode::Radius => (finite(origin - extent)?, finite(2.0 * extent)?),
    })
}

/// The pixel index a device coordinate starts or ends at, kept within `0..=limit`. The coordinate
/// is finite (the transform checked it), and a cast of a negative or huge value saturates.
fn edge(coordinate: f32, limit: usize) -> usize {
    (coordinate.max(0.0) as usize).min(limit)
}

/// The pixel `colour` multiplied channel by channel by the tint, alpha included (`tint`); a pixel
/// without a tint is unchanged.
fn tinted(colour: u32, tint: Option<u32>) -> u32 {
    let Some(tint) = tint else {
        return colour;
    };
    let (alpha, red, green, blue) = channels(colour);
    let (tint_alpha, tint_red, tint_green, tint_blue) = channels(tint);
    argb(
        scaled(alpha, tint_alpha),
        scaled(red, tint_red),
        scaled(green, tint_green),
        scaled(blue, tint_blue),
    )
}

/// One channel times the same channel of a tint, out of 255.
fn scaled(channel: u8, tint: u8) -> u8 {
    (u32::from(channel) * u32::from(tint) / 255) as u8
}
