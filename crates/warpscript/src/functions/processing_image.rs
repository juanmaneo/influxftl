// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Processing functions on images and pixels: the drawing of an image (`Pimage`), the size
//! and the pixels of an image (`Psize`, `Presize`, `Ppixels`, `PupdatePixels`, `Pmask`,
//! `PtoImage`), and the copies, blends, pixel access and filters between them (`Pget`, `Pset`,
//! `Pcopy`, `Pblend`, `Pfilter`).
//!
//! As in the other Processing functions, the arguments are the values above the `PGRAPHICS` the
//! function works on, and the graphics is pushed back under its result. A `PGRAPHICS` is taken for
//! the graphics of the call wherever it is among the arguments, so it never stands where a `PIMAGE`
//! is read as an argument. The functions that pop their values one by one, `Psize`, `Presize`,
//! `Ppixels` and `Pmask`, read a `PGRAPHICS` as a `PIMAGE`, as Warp 10 does.

use std::cell::RefCell;
use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::processing::{
    Params, SharedGraphics, answer, float_of, give_back, int_of, parse_params,
};
use super::processing_color::colour_long;
use crate::error::processing::ProcessingError;
use crate::error::{Class, Error, Result};
use crate::processing::Blend;
use crate::processing::blend::blit;
use crate::processing::filter::{self, Kind};

use crate::processing::image::{Image, Rect};
use crate::stack::Stack;
use crate::value::{Value, ValueKind};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("Pimage", Entry::Stack(pimage)),
    ("Psize", Entry::Stack(psize)),
    ("Presize", Entry::Stack(presize)),
    ("Pmask", Entry::Stack(pmask)),
    ("Ppixels", Entry::Stack(ppixels)),
    ("PupdatePixels", Entry::Stack(pupdatepixels)),
    ("PtoImage", Entry::Stack(ptoimage)),
    ("Pblend", Entry::Stack(pblend)),
    ("Pcopy", Entry::Stack(pcopy)),
    ("Pget", Entry::Stack(pget)),
    ("Pset", Entry::Stack(pset)),
    ("Pfilter", Entry::Stack(pfilter)),
];

/// A `PIMAGE` value holding `image`.
pub(super) fn image_value(image: Image) -> Value {
    Value::Image(Rc::new(RefCell::new(image)))
}

/// The pixels of an image as a LIST of `LONG`s, one per pixel, row by row.
fn pixel_list(pixels: &[u32]) -> Value {
    Value::list(pixels.iter().map(|pixel| colour_long(*pixel)).collect())
}

/// The pixels a LIST holds, as ARGB values: one `LONG` per pixel, each read as Java's `intValue()`.
/// The count is checked before any value is read, as Warp 10 does; `article` is `"an "` where the
/// function's message has it.
fn pixels_of(
    list: &[Value],
    expected: usize,
    function: &'static str,
    article: &'static str,
) -> Result<Vec<u32>> {
    if list.len() != expected {
        return Err(ProcessingError::PixelCount {
            function,
            article,
            expected,
            found: list.len(),
        }
        .into());
    }
    list.iter()
        .map(|value| match value {
            Value::Long(long) => Ok(*long as u32),
            _ => Err(Error::from(ProcessingError::PixelNotLong { function })),
        })
        .collect()
}

/// The `PIMAGE` the value at `index` above the graphics holds. A `null` is a
/// `NullPointerException`, and any other value is a failed cast, as Java's `(PImage)` cast.
fn image_at(params: &Params, index: usize) -> Result<Rc<RefCell<Image>>> {
    match params.args.get(index) {
        Some(Value::Image(image)) => Ok(Rc::clone(image)),
        Some(Value::Null) => Err(Error::null_pointer()),
        Some(other) => Err(Error::class_cast(other.kind(), Class::Image)),
        None => Err(params.arity()),
    }
}

/// The rectangle of `width` by `height` pixels at (`x`, `y`).
fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
    Rect {
        x: i64::from(x),
        y: i64::from(y),
        width: i64::from(width),
        height: i64::from(height),
    }
}

/// Blend the rectangle `from` of `source` onto the rectangle `to` of the graphics, with `mode`. The
/// source is the graphics itself when `source` is `None`: its pixels are copied before they are
/// written, so that the two rectangles may overlap.
fn blit_onto(
    graphics: &SharedGraphics,
    source: Option<Rc<RefCell<Image>>>,
    from: Rect,
    to: Rect,
    mode: Blend,
) {
    let mut graphics = graphics.borrow_mut();
    match source {
        Some(image) => blit(&mut graphics.image, &image.borrow(), from, to, mode),
        None => {
            let own = graphics.image.clone();
            blit(&mut graphics.image, &own, from, to, mode);
        }
    }
}

/// `Pimage`: `graphics image x y`, or `graphics image x y width height`, draws the `PIMAGE` on the
/// graphics, placed by the image mode. The image is the value deepest above the graphics.
fn pimage(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "Pimage";
    let params = parse_params(stack, &[3, 5])?;
    let Some(Value::Image(source)) = params.args.first() else {
        return Err(fail(NAME, "expects an image as parameter deepest."));
    };
    let (x, y, size) = match params.len() {
        3 => {
            let [x, y] = params.floats_after::<2>(1)?;
            (x, y, None)
        }
        _ => {
            let [x, y, width, height] = params.floats_after::<4>(1)?;
            (x, y, Some([width, height]))
        }
    };
    params
        .graphics
        .borrow_mut()
        .draw_image(&source.borrow(), x, y, size)?;
    give_back(stack, params.graphics);
    Ok(())
}

/// `Psize`: the width, then the height, of a `PIMAGE` or a `PGRAPHICS`, as two `LONG`s (the height
/// on top).
fn psize(stack: &mut Stack) -> Result<()> {
    let (width, height) = match stack.pop()? {
        Value::Image(image) => dimensions(&image.borrow()),
        Value::Graphics(graphics) => dimensions(&graphics.borrow().image),
        _ => {
            return Err(fail("Psize", "expects a PIMAGE or a PGRAPHICS instance."));
        }
    };
    stack.push(Value::Long(width as i64));
    stack.push(Value::Long(height as i64));
    Ok(())
}

/// The width and the height of an image, in pixels.
fn dimensions(image: &Image) -> (usize, usize) {
    (image.width, image.height)
}

/// A height or a width `Presize` takes: from 0 to `Integer.MAX_VALUE`.
fn side(value: i64) -> Result<usize> {
    match i32::try_from(value) {
        Ok(side) if side >= 0 => Ok(side as usize),
        _ => Err(fail(
            "Presize",
            "expects the height to be positive and less than 2147483647.",
        )),
    }
}

/// `own` times `given / given_source`: the side of the image that keeps the proportion of the given
/// side. Processing's float arithmetic truncates the result, and so does this. An image with no
/// side has no proportion to keep. The sides are at most `Integer.MAX_VALUE`, so the arithmetic is
/// finite.
fn in_proportion(source: &Image, own: usize, given_source: usize, given: usize) -> Result<usize> {
    if given_source == 0 {
        return Err(ProcessingError::EmptyImage {
            width: source.width as i64,
            height: source.height as i64,
        }
        .into());
    }
    let factor = given as f32 / given_source as f32;
    Ok((own as f32 * factor) as usize)
}

/// The size `Presize` gives an image: a side given as zero takes the length that keeps the
/// proportion of the image, and two zero sides are refused. A side that comes out as zero is an
/// empty image, refused as Processing refuses it.
fn resized_size(source: &Image, width: usize, height: usize) -> Result<(usize, usize)> {
    let (width, height) = match (width, height) {
        (0, 0) => return Err(ProcessingError::ResizeWithoutSize.into()),
        (0, height) => (
            in_proportion(source, source.width, source.height, height)?,
            height,
        ),
        (width, 0) => (
            width,
            in_proportion(source, source.height, source.width, width)?,
        ),
        sizes => sizes,
    };
    if width == 0 || height == 0 {
        return Err(ProcessingError::EmptyImage {
            width: width as i64,
            height: height as i64,
        }
        .into());
    }
    Ok((width, height))
}

/// `Presize`: `image width height Presize` resizes a `PIMAGE` or a `PGRAPHICS` (pushed back) to the
/// given size. A `PGRAPHICS` keeps its pixels, scaled, and restarts its depth buffer.
fn presize(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "Presize";
    let Value::Long(height) = stack.pop()? else {
        return Err(fail(NAME, "expects a height in pixel."));
    };
    let height = side(height)?;
    let Value::Long(width) = stack.pop()? else {
        return Err(fail(NAME, "expects a width in pixel."));
    };
    let width = side(width)?;
    let pixels = width as i64 * height as i64;
    let limit = stack.limits.max_pixels;
    if pixels > limit {
        return Err(ProcessingError::TooManyPixels {
            function: NAME,
            allowed: "PGRAPHICS or PIMAGE",
            limit,
            width: width as i64,
            height: height as i64,
            pixels,
        }
        .into());
    }
    match stack.pop()? {
        Value::Graphics(graphics) => {
            let (width, height) = resized_size(&graphics.borrow().image, width, height)?;
            graphics.borrow_mut().resize(width, height)?;
            stack.push(Value::Graphics(graphics));
        }
        Value::Image(image) => {
            let resized = {
                let source = image.borrow();
                let (width, height) = resized_size(&source, width, height)?;
                source.resized(width, height)?
            };
            *image.borrow_mut() = resized;
            stack.push(Value::Image(image));
        }
        _ => return Err(fail(NAME, "expects a PIMAGE or a PGRAPHICS instance.")),
    }
    Ok(())
}

/// `Pmask`: `image mask Pmask` gives `image` the alpha of the blue channel of `mask`, pixel for
/// pixel. Both are a `PIMAGE` or a `PGRAPHICS`, as `PGraphics` extends `PImage` in Java, and they
/// must hold the same number of pixels (not the same dimensions). The image is pushed back.
fn pmask(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "Pmask";
    // The mask is copied before the image is borrowed for writing: both may be the same object.
    let alpha = match stack.pop()? {
        Value::Image(mask) => mask.borrow().clone(),
        Value::Graphics(mask) => mask.borrow().image.clone(),
        _ => return Err(fail(NAME, "expects two PIMAGEs.")),
    };
    let image = stack.pop()?;
    let apply = |target: &mut Image| -> Result<()> {
        if target.pixels.len() != alpha.pixels.len() {
            return Err(fail(
                NAME,
                "expects the two PIMAGEs to be of the same size.",
            ));
        }
        target.mask(&alpha);
        Ok(())
    };
    match &image {
        Value::Image(target) => apply(&mut target.borrow_mut())?,
        Value::Graphics(target) => apply(&mut target.borrow_mut().image)?,
        _ => return Err(fail(NAME, "expects two PIMAGEs.")),
    }
    stack.push(image);
    Ok(())
}

/// `Ppixels`: the pixels of a `PGRAPHICS` (pushed back under them) or of a `PIMAGE`, as a LIST of
/// `LONG`s, row by row.
fn ppixels(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::Graphics(graphics) => {
            let pixels = pixel_list(&graphics.borrow().image.pixels);
            answer(stack, graphics, pixels);
        }
        Value::Image(image) => {
            let pixels = pixel_list(&image.borrow().pixels);
            stack.push(pixels);
        }
        _ => {
            return Err(fail(
                "Ppixels",
                "can only be applied to PGraphics or PImage instances.",
            ));
        }
    }
    Ok(())
}

/// `PupdatePixels`: the pixels of an image replaced by a LIST of `LONG`s, one per pixel. `image
/// list` leaves the image on top; `graphics list` pushes the graphics back.
fn pupdatepixels(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PupdatePixels";
    if let [.., Value::Image(image), Value::List(list)] = stack.elements() {
        let count = image.borrow().pixels.len();
        let pixels = pixels_of(&list.borrow(), count, NAME, "")?;
        image.borrow_mut().pixels = pixels;
        stack.discard_top(1);
        return Ok(());
    }
    let params = parse_params(stack, &[1])?;
    let Some(Value::List(list)) = params.args.first() else {
        return Err(fail(
            NAME,
            "expects an array of pixels on top of the stack.",
        ));
    };
    let count = params.graphics.borrow().image.pixels.len();
    let pixels = pixels_of(&list.borrow(), count, NAME, "an ")?;
    params.graphics.borrow_mut().image.pixels = pixels;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PtoImage`: the `PIMAGE` copy of the `PGRAPHICS` on top of the stack, which is consumed.
fn ptoimage(stack: &mut Stack) -> Result<()> {
    let Value::Graphics(graphics) = stack.pop()? else {
        return Err(fail("PtoImage", "operates on a PGraphics instance."));
    };
    let copy = graphics.borrow().image.clone();
    stack.push(image_value(copy));
    Ok(())
}

/// The blend mode a `Pblend` names in its last value: every mode of `blendMode` but `REPLACE`. A
/// `null` names no mode, and is refused as any other text is.
fn blend_mode(params: &Params) -> Result<Blend> {
    let keyword = match params.args.last() {
        Some(Value::String(keyword)) => Some(&**keyword),
        Some(Value::Null) => None,
        Some(other) => {
            return Err(Error::class_cast(
                other.kind(),
                Class::Value(ValueKind::String),
            ));
        }
        None => return Err(params.arity()),
    };
    Ok(match keyword {
        Some("BLEND") => Blend::Blend,
        Some("ADD") => Blend::Add,
        Some("SUBTRACT") => Blend::Subtract,
        Some("DARKEST") => Blend::Darkest,
        Some("LIGHTEST") => Blend::Lightest,
        Some("DIFFERENCE") => Blend::Difference,
        Some("EXCLUSION") => Blend::Exclusion,
        Some("MULTIPLY") => Blend::Multiply,
        Some("SCREEN") => Blend::Screen,
        Some("OVERLAY") => Blend::Overlay,
        Some("HARD_LIGHT") => Blend::HardLight,
        Some("SOFT_LIGHT") => Blend::SoftLight,
        Some("DODGE") => Blend::Dodge,
        Some("BURN") => Blend::Burn,
        _ => {
            return Err(ProcessingError::Refused {
                function: "Pblend",
                message: ": invalid mode, should be 'BLEND', 'ADD', 'SUBTRACT', 'DARKEST', 'LIGHTEST', 'DIFFERENCE', 'EXCLUSION', 'MULTIPLY', 'SCREEN', 'OVERLAY', 'HARD_LIGHT', 'SOFT_LIGHT', 'DODGE' or 'BURN'.",
            }
            .into());
        }
    })
}

/// `Pblend`: `graphics sx sy sw sh dx dy dw dh mode` blends the rectangle (`sx`, `sy`, `sw`, `sh`)
/// of the graphics onto the rectangle (`dx`, `dy`, `dw`, `dh`), the source sampled by nearest
/// neighbour when the sizes differ. With an image first, the source is that `PIMAGE`.
fn pblend(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[9, 10])?;
    let source = if params.len() == 10 {
        Some(image_at(&params, 0)?)
    } else {
        None
    };
    let [sx, sy, sw, sh, dx, dy, dw, dh] =
        params.ints_window::<8>(usize::from(source.is_some()))?;
    let mode = blend_mode(&params)?;
    blit_onto(
        &params.graphics,
        source,
        rect(sx, sy, sw, sh),
        rect(dx, dy, dw, dh),
        mode,
    );
    give_back(stack, params.graphics);
    Ok(())
}

/// `Pcopy`: with no value, the graphics is pushed back under a `PIMAGE` copy of it. `graphics sx sy
/// sw sh dx dy dw dh` copies a rectangle of the graphics onto another one, and `graphics image sx
/// sy sw sh dx dy dw dh` copies from a `PIMAGE`. A copy replaces the pixels, without blending.
fn pcopy(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0, 8, 9])?;
    if params.is_empty() {
        let copy = params.graphics.borrow().image.clone();
        answer(stack, params.graphics, image_value(copy));
        return Ok(());
    }
    let source = if params.len() == 9 {
        Some(image_at(&params, 0)?)
    } else {
        None
    };
    let [sx, sy, sw, sh, dx, dy, dw, dh] =
        params.ints_window::<8>(usize::from(source.is_some()))?;
    blit_onto(
        &params.graphics,
        source,
        rect(sx, sy, sw, sh),
        rect(dx, dy, dw, dh),
        Blend::Replace,
    );
    give_back(stack, params.graphics);
    Ok(())
}

/// `Pget`: `graphics` gives a `PIMAGE` copy of the graphics; `graphics x y` gives the pixel at
/// (`x`, `y`) as a `LONG` (`0` outside the graphics); `graphics x y width height` gives that
/// rectangle as a `PIMAGE`. The value is left on top, the graphics under it.
fn pget(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0, 2, 4])?;
    let value = match params.len() {
        0 => image_value(params.graphics.borrow().image.clone()),
        2 => {
            let [x, y] = params.ints::<2>()?;
            let colour = params
                .graphics
                .borrow()
                .image
                .get(i64::from(x), i64::from(y));
            colour_long(colour)
        }
        _ => {
            let [x, y, width, height] = params.ints::<4>()?;
            let region = params
                .graphics
                .borrow()
                .image
                .region(rect(x, y, width, height))?;
            image_value(region)
        }
    };
    answer(stack, params.graphics, value);
    Ok(())
}

/// `Pset`: `graphics x y colour` sets the pixel at (`x`, `y`) to the colour, an integer, and
/// `graphics x y image` pastes the `PIMAGE` with its corner at (`x`, `y`). Pixels outside the
/// graphics are not drawn.
fn pset(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[3])?;
    set_or_paste(&params)?;
    give_back(stack, params.graphics);
    Ok(())
}

/// The write of `Pset`: the values above the graphics are `x`, `y` and what is written there, a
/// `PIMAGE` pasted at (`x`, `y`) or an integer colour set on the pixel.
fn set_or_paste(params: &Params) -> Result<()> {
    let [x, y] = params.ints_window::<2>(0)?;
    let (x, y) = (i64::from(x), i64::from(y));
    let value = params.args.get(2).ok_or_else(|| params.arity())?;
    match value {
        Value::Image(image) => params
            .graphics
            .borrow_mut()
            .image
            .paste(&image.borrow(), x, y),
        _ => {
            let colour = int_of(value)?;
            params.graphics.borrow_mut().image.set(x, y, colour as u32);
        }
    }
    Ok(())
}

/// The filter a `Pfilter` names.
fn filter_kind(keyword: Option<&str>) -> Result<Kind> {
    Ok(match keyword {
        Some("THRESHOLD") => Kind::Threshold,
        Some("GRAY") => Kind::Gray,
        Some("OPAQUE") => Kind::Opaque,
        Some("INVERT") => Kind::Invert,
        Some("POSTERIZE") => Kind::Posterize,
        Some("BLUR") => Kind::Blur,
        Some("ERODE") => Kind::Erode,
        Some("DILATE") => Kind::Dilate,
        _ => return Err(fail("Pfilter", "invalid mode.")),
    })
}

/// `Pfilter`: `graphics kind` or `graphics kind param` applies a filter to every pixel of the
/// graphics. The kinds are `THRESHOLD`, `GRAY`, `OPAQUE`, `INVERT`, `POSTERIZE`, `BLUR`, `ERODE` and
/// `DILATE`.
fn pfilter(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 2])?;
    let kind = filter_kind(params.keyword(0)?)?;
    let param = params.args.get(1).map(float_of).transpose()?;
    filter::apply(&mut params.graphics.borrow_mut().image, kind, param)?;
    give_back(stack, params.graphics);
    Ok(())
}

#[cfg(test)]
#[path = "processing_image_private_tests.rs"]
mod private_tests;
