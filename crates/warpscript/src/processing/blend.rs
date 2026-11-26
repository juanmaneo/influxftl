// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! How a source pixel is painted over a destination pixel in each blend mode, and the copy of a
//! rectangle of pixels from one image to another (`copy`, `blend`).

use super::Blend;
use super::image::{Image, Rect, argb, channels};

impl Blend {
    /// The pixel that results from painting `source` over `destination` in this mode. `REPLACE`
    /// keeps the source; `BLEND` composites it by its alpha; the other modes combine the colours
    /// first and then composite the result by the alpha of the source.
    pub fn mix(self, destination: u32, source: u32) -> u32 {
        match self {
            Blend::Replace => source,
            Blend::Blend => over(destination, source),
            mode => {
                let (alpha, _, _, _) = channels(source);
                let (_, dr, dg, db) = channels(destination);
                let (_, sr, sg, sb) = channels(source);
                let combined = argb(
                    alpha,
                    mode.channel(dr, sr),
                    mode.channel(dg, sg),
                    mode.channel(db, sb),
                );
                over(destination, combined)
            }
        }
    }

    /// One channel of a mode that combines colours (every mode but `BLEND` and `REPLACE`), from
    /// the destination channel `d` and the source channel `s`, as Processing's `blendColor`.
    fn channel(self, d: u8, s: u8) -> u8 {
        let (d, s) = (u32::from(d), u32::from(s));
        let value = match self {
            Blend::Add => (d + s).min(255),
            Blend::Subtract => d.saturating_sub(s),
            Blend::Darkest => d.min(s),
            Blend::Lightest => d.max(s),
            Blend::Difference => d.abs_diff(s),
            Blend::Exclusion => d + s - 2 * d * s / 255,
            Blend::Multiply => d * s / 255,
            Blend::Screen => 255 - (255 - d) * (255 - s) / 255,
            Blend::Overlay => overlay(d, s),
            Blend::HardLight => overlay(s, d),
            Blend::SoftLight => soft_light(d, s),
            Blend::Dodge => {
                if s == 255 {
                    255
                } else {
                    (d * 255 / (255 - s)).min(255)
                }
            }
            Blend::Burn => {
                // A black source saturates the quotient, which burns the destination to black.
                let quotient = ((255 - d) * 255).checked_div(s).unwrap_or(255);
                255 - quotient.min(255)
            }
            Blend::Blend | Blend::Replace => s,
        };
        // Every case is at most 255: the `min` and `saturating_sub` above keep it in range.
        value as u8
    }
}

/// `BLEND`: the source composited over the destination by the alpha of the source, with the
/// alpha of the destination kept under it. A fully opaque source replaces the pixel.
fn over(destination: u32, source: u32) -> u32 {
    let (sa, sr, sg, sb) = channels(source);
    let (da, dr, dg, db) = channels(destination);
    if sa == 255 {
        return source;
    }
    if sa == 0 {
        return destination;
    }
    let (sa, da) = (u32::from(sa), u32::from(da));
    let alpha = sa + da * (255 - sa) / 255;
    // Both weights are rounded down, so the mix can overshoot 255 by one: it is capped there.
    let mix = |sc: u8, dc: u8| {
        let (sc, dc) = (u32::from(sc), u32::from(dc));
        ((sc * sa + dc * da * (255 - sa) / 255) / alpha).min(255) as u8
    };
    argb(alpha as u8, mix(sr, dr), mix(sg, dg), mix(sb, db))
}

/// `OVERLAY` of the base `d` and the blend `s`: the base is multiplied in its dark half and
/// screened in its light half.
fn overlay(d: u32, s: u32) -> u32 {
    if d < 128 {
        2 * d * s / 255
    } else {
        255 - 2 * (255 - d) * (255 - s) / 255
    }
}

/// `SOFT_LIGHT` (the formula of Pegtop): `(1 - 2s)d² + 2sd`, in integers scaled by 255.
fn soft_light(d: u32, s: u32) -> u32 {
    let squared = d * d / 255;
    (squared + 2 * s * (d - squared) / 255).min(255)
}

/// Paint the rectangle `from` of `source` over the rectangle `to` of `destination`, the source
/// sampled by nearest neighbour when the sizes differ, with `mode` (`copy` and `blend`). A
/// rectangle with no area paints nothing, and pixels outside either image are skipped.
pub fn blit(destination: &mut Image, source: &Image, from: Rect, to: Rect, mode: Blend) {
    if from.width <= 0 || from.height <= 0 || to.width <= 0 || to.height <= 0 {
        return;
    }
    for row in 0..to.height {
        for column in 0..to.width {
            let source_x = from.x + column * from.width / to.width;
            let source_y = from.y + row * from.height / to.height;
            let (x, y) = (to.x + column, to.y + row);
            if let Some(colour) = source.pixel(source_x, source_y) {
                let mixed = mode.mix(destination.get(x, y), colour);
                destination.set(x, y, mixed);
            }
        }
    }
}

#[cfg(test)]
#[path = "blend_tests.rs"]
mod blend_tests;
