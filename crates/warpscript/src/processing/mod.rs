// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Processing objects of WarpLib: graphics contexts (`PGRAPHICS`), images (`PIMAGE`), shapes
//! (`PSHAPE`) and fonts (`PFONT`), and the software renderer that draws on them.
//!
//! The renderer is written from the documented behaviour of the Processing API: a pixel
//! buffer of ARGB values, polygons filled with the non-zero rule, a z-buffer for the 3D
//! contexts. Anti-aliasing and the glyph shapes of the JDK fonts are not reproduced (see
//! `DIVERGENCES.md`).

pub mod color;
pub mod font;
pub mod geometry;
pub mod image;
pub mod paint;
pub mod shape;

use std::rc::Rc;

pub use font::Font;
pub use image::Image;
pub use shape::Shape;

use crate::error::processing::ProcessingError;
use color::ColorMode;
use geometry::Transform;

/// A `float` of a Processing computation, refused when it is `NaN` or infinite: a division by
/// a zero maximum or an overflow is an error, never a colour or a coordinate.
pub fn finite(value: f32) -> crate::error::Result<f32> {
    Ok(crate::finite::ensure_finite(f64::from(value))? as f32)
}

/// How coordinates position a rectangle, an ellipse, an image or a shape (`rectMode`,
/// `ellipseMode`, `imageMode`, `shapeMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `CORNER`: position and size.
    Corner,
    /// `CORNERS`: two opposite corners.
    Corners,
    /// `CENTER`: centre and size.
    Center,
    /// `RADIUS`: centre and half-size.
    Radius,
}

/// The blend modes of `blendMode` and `blend`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    /// `BLEND`: source over destination (alpha compositing).
    Blend,
    /// `ADD`: the channels are added.
    Add,
    /// `SUBTRACT`: the destination channels minus the source ones.
    Subtract,
    /// `DARKEST`: the darker channel.
    Darkest,
    /// `LIGHTEST`: the lighter channel.
    Lightest,
    /// `DIFFERENCE`: the absolute difference of the channels.
    Difference,
    /// `EXCLUSION`: a softer difference.
    Exclusion,
    /// `MULTIPLY`: the channels are multiplied.
    Multiply,
    /// `SCREEN`: the inverse of the multiplied inverses.
    Screen,
    /// `REPLACE`: the source replaces the destination.
    Replace,
    /// `OVERLAY` (`blend` only).
    Overlay,
    /// `HARD_LIGHT` (`blend` only).
    HardLight,
    /// `SOFT_LIGHT` (`blend` only).
    SoftLight,
    /// `DODGE` (`blend` only).
    Dodge,
    /// `BURN` (`blend` only).
    Burn,
}

/// The end of an open stroke (`strokeCap`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cap {
    /// `SQUARE`: the stroke stops at the end point.
    Square,
    /// `PROJECT`: a square cap extended by half the stroke weight.
    Project,
    /// `ROUND`: a half-disc.
    Round,
}

/// The corner of a stroke (`strokeJoin`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Join {
    /// `MITER`: the outer edges are extended to meet.
    Miter,
    /// `BEVEL`: the corner is cut.
    Bevel,
    /// `ROUND`: the corner is a disc.
    Round,
}

/// The horizontal alignment of text (`textAlign`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HAlign {
    /// `LEFT`.
    Left,
    /// `RIGHT`.
    Right,
    /// `CENTER`.
    Center,
}

/// The vertical alignment of text (`textAlign`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VAlign {
    /// `TOP`.
    Top,
    /// `BOTTOM`.
    Bottom,
    /// `CENTER`.
    Center,
    /// `BASELINE`.
    Baseline,
}

/// Whether text is drawn as glyphs of the font (`MODEL`, the default) or as shapes (`SHAPE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextMode {
    /// `MODEL`.
    Model,
    /// `SHAPE`.
    Shape,
}

/// The text settings of a graphics context.
#[derive(Clone, Debug)]
pub struct TextStyle {
    /// The font set by `textFont`; `None` is the default font.
    pub font: Option<Rc<Font>>,
    /// The size set by `textSize`.
    pub size: f32,
    /// The distance between lines set by `textLeading`.
    pub leading: f32,
    /// The horizontal alignment set by `textAlign`.
    pub align_x: HAlign,
    /// The vertical alignment set by `textAlign`.
    pub align_y: VAlign,
    /// The mode set by `textMode`.
    pub mode: TextMode,
}

/// The drawing settings of a graphics context: what `pushStyle` saves and `popStyle` restores.
#[derive(Clone, Debug)]
pub struct Style {
    /// The fill colour (ARGB), `None` after `noFill`.
    pub fill: Option<u32>,
    /// The stroke colour (ARGB), `None` after `noStroke`.
    pub stroke: Option<u32>,
    /// The stroke weight (`strokeWeight`).
    pub stroke_weight: f32,
    /// The end of open strokes (`strokeCap`).
    pub cap: Cap,
    /// The corners of strokes (`strokeJoin`).
    pub join: Join,
    /// The tint of images (`tint`), `None` after `noTint`.
    pub tint: Option<u32>,
    /// The colour mode and its maxima (`colorMode`).
    pub color_mode: ColorMode,
    /// The mode of rectangles (`rectMode`).
    pub rect_mode: Mode,
    /// The mode of ellipses (`ellipseMode`).
    pub ellipse_mode: Mode,
    /// The mode of images (`imageMode`).
    pub image_mode: Mode,
    /// The mode of shapes (`shapeMode`).
    pub shape_mode: Mode,
    /// The blend mode (`blendMode`).
    pub blend: Blend,
    /// The text settings.
    pub text: TextStyle,
    /// The number of segments of a Bézier curve (`bezierDetail`), as Warp 10 stores it.
    pub bezier_detail: i32,
    /// The number of segments of a Catmull-Rom curve (`curveDetail`), as Warp 10 stores it.
    pub curve_detail: i32,
    /// The tightness of Catmull-Rom curves (`curveTightness`).
    pub curve_tightness: f32,
    /// The number of divisions of a sphere (`sphereDetail`): around, then from pole to pole.
    pub sphere_detail: (i32, i32),
}

impl Default for Style {
    /// The style of a new graphics context: white fill, black stroke, weight 1, round caps,
    /// mitered joins, RGB colours in 0..255.
    fn default() -> Self {
        Style {
            fill: Some(0xffff_ffff),
            stroke: Some(0xff00_0000),
            stroke_weight: 1.0,
            cap: Cap::Round,
            join: Join::Miter,
            tint: None,
            color_mode: ColorMode::default(),
            rect_mode: Mode::Corner,
            ellipse_mode: Mode::Center,
            image_mode: Mode::Corner,
            shape_mode: Mode::Corner,
            blend: Blend::Blend,
            text: TextStyle {
                font: None,
                size: 12.0,
                leading: 14.0,
                align_x: HAlign::Left,
                align_y: VAlign::Baseline,
                mode: TextMode::Model,
            },
            bezier_detail: 20,
            curve_detail: 20,
            curve_tightness: 0.0,
            sphere_detail: (30, 30),
        }
    }
}

/// A graphics context (`PGRAPHICS`): an image drawn on with the current style and transform.
#[derive(Clone, Debug)]
pub struct Graphics {
    /// The pixels drawn on.
    pub image: Image,
    /// `true` for a `3D` context, which has a depth buffer and a perspective projection.
    pub three_d: bool,
    /// The depth of each pixel of a `3D` context (`f32::MAX` where nothing was drawn).
    pub depth: Vec<f32>,
    /// The current drawing settings.
    pub style: Style,
    /// The settings saved by `pushStyle`.
    pub saved_styles: Vec<Style>,
    /// The current transform (`translate`, `rotate`, `scale`…).
    pub transform: Transform,
    /// The transforms saved by `pushMatrix`.
    pub saved_transforms: Vec<Transform>,
    /// The clipping rectangle set by `clip`, in pixels.
    pub clip: Option<paint::Clip>,
}

impl Graphics {
    /// Every pixel set to `colour`, and the depth buffer cleared (`background`).
    pub fn background(&mut self, colour: u32) {
        self.image.pixels.fill(colour);
        self.depth.fill(f32::MAX);
    }

    /// Every pixel transparent (`clear`).
    pub fn clear(&mut self) {
        self.background(0);
    }

    /// Every pixel taken from `source`, which is scaled to the size of this context by nearest
    /// neighbour (`background(PImage)`). An empty source has no pixel to take.
    pub fn background_image(&mut self, source: &Image) -> crate::error::Result<()> {
        if source.pixels.is_empty() {
            return Err(ProcessingError::EmptyImage {
                width: source.width as i64,
                height: source.height as i64,
            }
            .into());
        }
        let (width, height) = (self.image.width, self.image.height);
        for y in 0..height {
            let source_y = y * source.height / height;
            for x in 0..width {
                let source_x = x * source.width / width;
                self.image.pixels[y * width + x] =
                    source.pixels[source_y * source.width + source_x];
            }
        }
        self.depth.fill(f32::MAX);
        Ok(())
    }

    /// A new context of `width` by `height` pixels, transparent, with the default style.
    pub fn new(width: usize, height: usize, three_d: bool) -> crate::error::Result<Self> {
        let image = Image::new(width, height)?;
        let depth = if three_d {
            vec![f32::MAX; image.pixels.len()]
        } else {
            Vec::new()
        };
        Ok(Graphics {
            image,
            three_d,
            depth,
            style: Style::default(),
            saved_styles: Vec::new(),
            transform: Transform::identity(),
            saved_transforms: Vec::new(),
            clip: None,
        })
    }
}
