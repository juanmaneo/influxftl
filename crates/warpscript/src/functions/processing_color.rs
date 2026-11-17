// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Processing colour functions: the colours `background`, `fill`, `stroke` and `tint` set,
//! `color` and `lerpColor` make, `colorMode` says how values are read, `clear` empties a
//! context, and the readers `red`, `green`, `blue`, `alpha`, `hue`, `saturation` and
//! `brightness` read a colour back.
//!
//! Java overloads each function by the kinds of its values. `fill`, `stroke` and `background`
//! ignore a first value that is neither a `LONG` nor a `DOUBLE`; `tint`, `color` and the readers
//! cast every value to a number, so a string is a `ClassCastException`.

use super::Entry;
use super::processing::{answer, float_of, give_back, int_of, parse_params};
use crate::error::processing::ProcessingError;
use crate::error::{Error, Result};
use crate::processing::color::{ColorMode, Component, Space};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("Pbackground", Entry::Stack(pbackground)),
    ("PcolorMode", Entry::Stack(pcolormode)),
    ("Pclear", Entry::Stack(pclear)),
    ("Pfill", Entry::Stack(pfill)),
    ("PnoFill", Entry::Stack(pnofill)),
    ("Pstroke", Entry::Stack(pstroke)),
    ("PnoStroke", Entry::Stack(pnostroke)),
    ("Ptint", Entry::Stack(ptint)),
    ("PnoTint", Entry::Stack(pnotint)),
    ("Pcolor", Entry::Stack(pcolor)),
    ("PlerpColor", Entry::Stack(plerpcolor)),
    (
        "Palpha",
        Entry::Stack(|stack| reader(stack, Component::Alpha)),
    ),
    ("Pred", Entry::Stack(|stack| reader(stack, Component::Red))),
    (
        "Pgreen",
        Entry::Stack(|stack| reader(stack, Component::Green)),
    ),
    (
        "Pblue",
        Entry::Stack(|stack| reader(stack, Component::Blue)),
    ),
    ("Phue", Entry::Stack(|stack| reader(stack, Component::Hue))),
    (
        "Psaturation",
        Entry::Stack(|stack| reader(stack, Component::Saturation)),
    ),
    (
        "Pbrightness",
        Entry::Stack(|stack| reader(stack, Component::Brightness)),
    ),
];

/// The arity failure of a colour function given `found` values (it takes one to four).
fn arity(found: usize) -> Error {
    ProcessingError::Arity {
        expected: &[1, 2, 3, 4],
        found,
    }
    .into()
}

/// A colour as the `LONG` a function pushes: the ARGB `int` read as a signed Java `int`.
fn colour_long(colour: u32) -> Value {
    Value::Long(i64::from(colour as i32))
}

/// The colour `fill`, `stroke` and `background` set from their values, as Java overloads them:
/// `None` when the first value is neither a `LONG` nor a `DOUBLE`, which Processing ignores.
fn paint_colour(mode: &ColorMode, args: &[Value]) -> Result<Option<u32>> {
    match args {
        [value] => match value {
            Value::Long(_) => Ok(Some(mode.from_int(int_of(value)?)?)),
            Value::Double(_) => Ok(Some(mode.gray(float_of(value)?, mode.max_alpha)?)),
            _ => Ok(None),
        },
        [first, alpha] => match first {
            Value::Long(_) => Ok(Some(mode.from_int_alpha(int_of(first)?, float_of(alpha)?)?)),
            Value::Double(_) => Ok(Some(mode.gray(float_of(first)?, float_of(alpha)?)?)),
            _ => Ok(None),
        },
        [red, green, blue] => Ok(Some(mode.calc(
            float_of(red)?,
            float_of(green)?,
            float_of(blue)?,
            mode.max_alpha,
        )?)),
        [red, green, blue, alpha] => Ok(Some(mode.calc(
            float_of(red)?,
            float_of(green)?,
            float_of(blue)?,
            float_of(alpha)?,
        )?)),
        _ => Err(arity(args.len())),
    }
}

/// The colour `tint` sets from its values: a `LONG` first is an ARGB `int`, anything else a gray
/// level (Java casts the value to a number, so a string fails).
fn tint_colour(mode: &ColorMode, args: &[Value]) -> Result<u32> {
    match args {
        [value] => match value {
            Value::Long(_) => mode.from_int(int_of(value)?),
            _ => mode.gray(float_of(value)?, mode.max_alpha),
        },
        [first, alpha] => match first {
            Value::Long(_) => mode.from_int_alpha(int_of(first)?, float_of(alpha)?),
            _ => mode.gray(float_of(first)?, float_of(alpha)?),
        },
        [red, green, blue] => mode.calc(
            float_of(red)?,
            float_of(green)?,
            float_of(blue)?,
            mode.max_alpha,
        ),
        [red, green, blue, alpha] => mode.calc(
            float_of(red)?,
            float_of(green)?,
            float_of(blue)?,
            float_of(alpha)?,
        ),
        _ => Err(arity(args.len())),
    }
}

/// The colour `color` makes from its values: the first value is always read as an `int` (a
/// `DOUBLE` is truncated), and the other values as floats.
fn colour_of(mode: &ColorMode, args: &[Value]) -> Result<u32> {
    match args {
        [value] => mode.from_int(int_of(value)?),
        [value, alpha] => mode.from_int_alpha(int_of(value)?, float_of(alpha)?),
        [red, green, blue] => mode.calc(
            float_of(red)?,
            float_of(green)?,
            float_of(blue)?,
            mode.max_alpha,
        ),
        [red, green, blue, alpha] => mode.calc(
            float_of(red)?,
            float_of(green)?,
            float_of(blue)?,
            float_of(alpha)?,
        ),
        _ => Err(arity(args.len())),
    }
}

/// `background`: fill the context with a colour, or with an image scaled to its size.
fn pbackground(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 2, 3, 4])?;
    match params.args.as_slice() {
        [Value::Image(source)] => {
            let source = source.borrow();
            params.graphics.borrow_mut().background_image(&source)?;
        }
        args => {
            let mode = params.graphics.borrow().style.color_mode;
            if let Some(colour) = paint_colour(&mode, args)? {
                params.graphics.borrow_mut().background(colour);
            }
        }
    }
    give_back(stack, params.graphics);
    Ok(())
}

/// `colorMode`: the space colours are read in, and the maximum of each value. One maximum sets
/// them all, three set the first three (the alpha keeps its maximum) and four set every one.
fn pcolormode(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 2, 4, 5])?;
    let space = match params.keyword(0)? {
        Some("RGB") => Space::Rgb,
        Some("HSB") => Space::Hsb,
        _ => {
            return Err(ProcessingError::Refused {
                function: "PcolorMode",
                message: ": invalid mode, should be one of 'RGB' or 'HSB'",
            }
            .into());
        }
    };
    let current = params.graphics.borrow().style.color_mode;
    let mode = match params.len() {
        1 => ColorMode { space, ..current },
        2 => {
            let [max] = params.floats_after(1)?;
            ColorMode::new(space, max, max, max, max)?
        }
        4 => {
            let [max1, max2, max3] = params.floats_after(1)?;
            ColorMode::new(space, max1, max2, max3, current.max_alpha)?
        }
        _ => {
            let [max1, max2, max3, max_alpha] = params.floats_after(1)?;
            ColorMode::new(space, max1, max2, max3, max_alpha)?
        }
    };
    params.graphics.borrow_mut().style.color_mode = mode;
    give_back(stack, params.graphics);
    Ok(())
}

/// `clear`: every pixel transparent.
fn pclear(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    params.graphics.borrow_mut().clear();
    give_back(stack, params.graphics);
    Ok(())
}

/// `fill`: the colour of the shapes drawn.
fn pfill(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 2, 3, 4])?;
    let mode = params.graphics.borrow().style.color_mode;
    if let Some(colour) = paint_colour(&mode, &params.args)? {
        params.graphics.borrow_mut().style.fill = Some(colour);
    }
    give_back(stack, params.graphics);
    Ok(())
}

/// `noFill`: shapes are not filled.
fn pnofill(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    params.graphics.borrow_mut().style.fill = None;
    give_back(stack, params.graphics);
    Ok(())
}

/// `stroke`: the colour of the lines drawn.
fn pstroke(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 2, 3, 4])?;
    let mode = params.graphics.borrow().style.color_mode;
    if let Some(colour) = paint_colour(&mode, &params.args)? {
        params.graphics.borrow_mut().style.stroke = Some(colour);
    }
    give_back(stack, params.graphics);
    Ok(())
}

/// `noStroke`: lines are not drawn.
fn pnostroke(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    params.graphics.borrow_mut().style.stroke = None;
    give_back(stack, params.graphics);
    Ok(())
}

/// `tint`: the colour images are multiplied by when drawn.
fn ptint(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 2, 3, 4])?;
    let mode = params.graphics.borrow().style.color_mode;
    let tint = tint_colour(&mode, &params.args)?;
    params.graphics.borrow_mut().style.tint = Some(tint);
    give_back(stack, params.graphics);
    Ok(())
}

/// `noTint`: images are drawn as they are.
fn pnotint(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    params.graphics.borrow_mut().style.tint = None;
    give_back(stack, params.graphics);
    Ok(())
}

/// `color`: a colour made from its values, pushed as a `LONG` above the context.
fn pcolor(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 2, 3, 4])?;
    let mode = params.graphics.borrow().style.color_mode;
    let colour = colour_of(&mode, &params.args)?;
    answer(stack, params.graphics, colour_long(colour));
    Ok(())
}

/// `lerpColor`: the colour between two colours, by an amount from 0 to 1.
fn plerpcolor(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[3])?;
    let [from, to, amount] = params.values::<3>(0)?;
    let (from, to) = (int_of(from)?, int_of(to)?);
    let amount = float_of(amount)?;
    let mode = params.graphics.borrow().style.color_mode;
    let colour = mode.lerp(from as u32, to as u32, amount)?;
    answer(stack, params.graphics, colour_long(colour));
    Ok(())
}

/// The readers `red`, `green`, `blue`, `alpha`, `hue`, `saturation` and `brightness`: one
/// component of a colour, in the range of the colour mode, pushed as a `DOUBLE`.
fn reader(stack: &mut Stack, which: Component) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let [colour] = params.ints()?;
    let value = params
        .graphics
        .borrow()
        .style
        .color_mode
        .component(colour as u32, which)?;
    answer(stack, params.graphics, Value::Double(f64::from(value)));
    Ok(())
}
