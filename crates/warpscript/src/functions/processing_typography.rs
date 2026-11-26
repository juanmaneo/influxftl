// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Processing functions of typography: `PcreateFont` makes a font, `PtextFont`, `PtextSize`,
//! `PtextLeading`, `PtextAlign` and `PtextMode` set the text settings of a graphics context,
//! `Ptext` draws text, and `PtextWidth`, `PtextAscent` and `PtextDescent` measure it.
//!
//! The glyphs are approximations, and the fonts are not loaded from files (see `DIVERGENCES.md`).

use std::rc::Rc;

use super::Entry;
use super::processing::{
    Params, answer, draw_and_return, float_of, give_back, mode_at, parse_params, show_warning,
};
use crate::error::processing::ProcessingError;
use crate::error::{Class, Error, Result};
use crate::processing::font::{self, Font};
use crate::processing::{HAlign, TextMode, VAlign, text};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PcreateFont", Entry::Stack(pcreatefont)),
    ("Ptext", Entry::Stack(ptext)),
    ("PtextAlign", Entry::Stack(ptextalign)),
    ("PtextAscent", Entry::Stack(ptextascent)),
    ("PtextDescent", Entry::Stack(ptextdescent)),
    ("PtextFont", Entry::Stack(ptextfont)),
    ("PtextLeading", Entry::Stack(ptextleading)),
    ("PtextMode", Entry::Stack(ptextmode)),
    ("PtextSize", Entry::Stack(ptextsize)),
    ("PtextWidth", Entry::Stack(ptextwidth)),
];

/// The size of a font when `createFont` is given none.
const DEFAULT_SIZE: f32 = 12.0;

/// The first value above the graphics. `parse_params` has checked the count, so a function that
/// takes one value always has it; `null` stands in for a value that cannot be missing.
fn first(params: &Params) -> &Value {
    params.args.first().unwrap_or(&Value::Null)
}

/// The text of a value, as `toString()` gives it: a `null` is a `NullPointerException`.
fn text_of(value: &Value) -> Result<String> {
    match value {
        Value::Null => Err(Error::null_pointer()),
        other => Ok(other.to_string()),
    }
}

/// The font a value is: a `PFONT`. A `null` is refused by `textFont` as a `RuntimeException`.
fn font_of(value: &Value) -> Result<Rc<Font>> {
    match value {
        Value::Font(font) => Ok(Rc::clone(font)),
        Value::Null => Err(ProcessingError::NullFont.into()),
        other => Err(Error::class_cast(other.kind(), Class::Font)),
    }
}

/// `true` for the name of a `.ttf` or `.otf` file, whatever its case.
fn is_font_file(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf"))
}

/// `PcreateFont(graphics, name)`, `(…, size)`, `(…, size, smooth)` and `(…, size, smooth,
/// charset)`: a font named `name` at `size`, rounded to a whole number of pixels. A name that is
/// a URL is refused, as no font resolver is set. A font file answers `null`, as the port does not
/// load fonts from files, and so does a size that rounds to zero or less.
fn pcreatefont(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 2, 3, 4])?;
    let name = text_of(first(&params))?;
    if ["http://", "https://", "file://"]
        .iter()
        .any(|scheme| name.starts_with(scheme))
    {
        return Err(ProcessingError::FontResolver.into());
    }
    let size = match params.args.get(1) {
        Some(value) => float_of(value)?,
        None => DEFAULT_SIZE,
    };
    if params.len() == 4 {
        params.keyword(3)?;
    }
    let size = font::rounded(size);
    let created = if size <= 0.0 || is_font_file(&name) {
        Value::Null
    } else {
        Value::Font(Rc::new(Font { name, size }))
    };
    answer(stack, params.graphics, created);
    Ok(())
}

/// `PtextFont(graphics, font)` and `PtextFont(graphics, font, size)`: selects the font for the
/// text. With the font alone, the text size becomes the size of the font; with a size, that size
/// is used, unless it is zero or less.
fn ptextfont(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 2])?;
    let font = font_of(first(&params))?;
    let size = match params.args.get(1) {
        None => Some(font.size),
        Some(value) => {
            let size = float_of(value)?;
            (size > 0.0).then_some(size)
        }
    };
    let mut graphics = params.graphics.borrow_mut();
    graphics.style.text.font = Some(font);
    if let Some(size) = size {
        graphics.style.text.size = size;
    }
    drop(graphics);
    give_back(stack, params.graphics);
    Ok(())
}

/// `PtextSize(graphics, size)`: the size of the text. A size that is zero or less is ignored.
/// With no font set, the default font is set at the size rounded to a whole number, which
/// Processing refuses (`NullPointerException`) when the rounding gives zero or less.
fn ptextsize(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let [size] = params.floats()?;
    if size > 0.0 {
        let mut graphics = params.graphics.borrow_mut();
        if graphics.style.text.font.is_none() {
            let rounded = font::rounded(size);
            if rounded <= 0.0 {
                return Err(Error::null_pointer());
            }
            graphics.style.text.font = Some(Rc::new(Font::default_at(rounded)));
        }
        graphics.style.text.size = size;
    }
    give_back(stack, params.graphics);
    Ok(())
}

/// `PtextLeading(graphics, leading)`: the distance between the baselines of lines of text.
fn ptextleading(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let [leading] = params.floats()?;
    params.graphics.borrow_mut().style.text.leading = leading;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PtextAlign(graphics, x)` and `PtextAlign(graphics, x, y)`: how text is aligned on its anchor.
/// `x` is `LEFT`, `RIGHT` or `CENTER`, and `y` is `TOP`, `BOTTOM`, `CENTER` or `BASELINE`. `x` is
/// checked first.
fn ptextalign(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 2])?;
    let align_x = mode_at(
        &params,
        0,
        "PtextAlign",
        " invalid alignment for X, should be 'LEFT', 'RIGHT' or 'CENTER'.",
        &[
            ("LEFT", HAlign::Left),
            ("RIGHT", HAlign::Right),
            ("CENTER", HAlign::Center),
        ],
    )?;
    let align_y = if params.len() == 2 {
        Some(mode_at(
            &params,
            1,
            "PtextAlign",
            " invalid alignment for Y, should be 'TOP', 'BOTTOM', 'CENTER' or 'BASELINE'.",
            &[
                ("TOP", VAlign::Top),
                ("BOTTOM", VAlign::Bottom),
                ("CENTER", VAlign::Center),
                ("BASELINE", VAlign::Baseline),
            ],
        )?)
    } else {
        None
    };
    let mut graphics = params.graphics.borrow_mut();
    graphics.style.text.align_x = align_x;
    if let Some(align_y) = align_y {
        graphics.style.text.align_y = align_y;
    }
    drop(graphics);
    give_back(stack, params.graphics);
    Ok(())
}

/// `PtextMode(graphics, mode)`: `MODEL` draws text with the glyphs of the font, `SHAPE` as
/// shapes. The mode is compared as a `STRING`, so any other value is refused.
fn ptextmode(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let mode = match params.args.first().and_then(Value::as_str) {
        Some("MODEL") => TextMode::Model,
        Some("SHAPE") => {
            show_warning("textMode(SHAPE) is not supported by this renderer.");
            TextMode::Shape
        }
        _ => {
            return Err(ProcessingError::Refused {
                function: "PtextMode",
                message: " invalid mode, should be 'MODEL' or 'SHAPE'.",
            }
            .into());
        }
    };
    params.graphics.borrow_mut().style.text.mode = mode;
    give_back(stack, params.graphics);
    Ok(())
}

/// `Ptext(graphics, str, x, y)`, `Ptext(graphics, str, x, y, z)` and `Ptext(graphics, str, x,
/// y, width, height)`: draws `str` at `x` and `y`, or wrapped into the box of `width` and
/// `height`. A `z` is read, and a non-zero one on a `2D` graphics is reported as a warning.
fn ptext(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[3, 4, 5])?;
    let content = text_of(first(&params))?;
    let style = params.graphics.borrow().style.text.clone();
    let outline = match params.len() {
        5 => {
            let [x, y, width, height] = params.floats_after::<4>(1)?;
            text::boxed(&content, &style, x, y, width, height)
        }
        4 => {
            let [x, y, z] = params.floats_after::<3>(1)?;
            if z != 0.0 && !params.graphics.borrow().three_d {
                show_warning(
                    "translate(), or this particular variation of it, is not available with this renderer.",
                );
            }
            text::point(&content, &style, x, y)
        }
        _ => {
            let [x, y] = params.floats_after::<2>(1)?;
            text::point(&content, &style, x, y)
        }
    };
    draw_and_return(stack, params, &outline)
}

/// `PtextWidth(graphics, str)`: the width of `str` in the font of the graphics, at the text size,
/// in pixels.
fn ptextwidth(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let content = text_of(first(&params))?;
    let width = text::width(&params.graphics.borrow().style.text, &content);
    answer(stack, params.graphics, Value::Double(width));
    Ok(())
}

/// `PtextAscent(graphics)`: the height of the glyphs above the baseline, at the text size.
fn ptextascent(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    let ascent = font::ascent(params.graphics.borrow().style.text.size);
    answer(stack, params.graphics, Value::Double(ascent));
    Ok(())
}

/// `PtextDescent(graphics)`: the depth of the glyphs below the baseline, at the text size.
fn ptextdescent(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    let descent = font::descent(params.graphics.borrow().style.text.size);
    answer(stack, params.graphics, Value::Double(descent));
    Ok(())
}
