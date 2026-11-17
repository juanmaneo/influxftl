// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Processing functions of WarpLib that are not about colours or transforms: the
//! `PGraphics` constructor, the style stack, `blendMode` and the clip, and the maths helpers
//! (`Pconstrain`, `Pdist`, `Plerp`, `Pmag`, `Pmap`, `Pnorm`).
//!
//! Every `P…` function takes its arguments from the stack up to the `PGRAPHICS` it draws on,
//! and leaves that graphics under its result. [`parse_params`] is `ProcessingUtil.parseParams`;
//! [`float_of`] and [`int_of`] are the `((Number) x).floatValue()` and `intValue()` casts.

use std::cell::RefCell;
use std::rc::Rc;

use super::Entry;
use super::args::fail;
use crate::error::fault::{BoundsError, NumberFormatError};
use crate::error::processing::ProcessingError;
use crate::error::{Class, Error, Result};
use crate::finite::ensure_finite;
use crate::processing::paint::Clip;
use crate::processing::{Blend, Graphics, finite};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PGraphics", Entry::Stack(pgraphics)),
    ("PpushStyle", Entry::Stack(ppushstyle)),
    ("PpopStyle", Entry::Stack(ppopstyle)),
    ("PblendMode", Entry::Stack(pblendmode)),
    ("Pclip", Entry::Stack(pclip)),
    ("PnoClip", Entry::Stack(pnoclip)),
    ("Pconstrain", Entry::Stack(pconstrain)),
    ("Pdist", Entry::Stack(pdist)),
    ("Plerp", Entry::Stack(plerp)),
    ("Pmag", Entry::Stack(pmag)),
    ("Pmap", Entry::Stack(pmap)),
    ("Pnorm", Entry::Stack(pnorm)),
];

/// A graphics context as the stack holds it: shared, and changed in place like the Java object.
pub type SharedGraphics = Rc<RefCell<Graphics>>;

/// The parameters of a Processing function: the graphics it draws on and the values given
/// above it, deepest first.
pub struct Params {
    /// The graphics context the function works on.
    pub graphics: SharedGraphics,
    /// The values above the graphics, deepest first.
    pub args: Vec<Value>,
    /// The counts of values the function accepts, for the arity failure.
    counts: &'static [usize],
}

/// `ProcessingUtil.parseParams`: pop values until a `PGRAPHICS` is found (inclusive) or more
/// than the largest accepted count have been popped, then check the count of values popped.
///
/// Without a `PGRAPHICS` on the stack, a right count still fails: the first value popped is cast
/// to a graphics context (a `null` is a `NullPointerException`), and an empty stack is an index
/// failure, as in Warp 10.
pub fn parse_params(stack: &mut Stack, counts: &'static [usize]) -> Result<Params> {
    let most = counts.iter().copied().fold(0, usize::max);
    let mut popped = Vec::new();
    let mut found = None;
    while stack.depth() != 0 && popped.len() <= most {
        match stack.pop()? {
            Value::Graphics(graphics) => {
                found = Some(graphics);
                break;
            }
            other => popped.push(other),
        }
    }
    if !counts.contains(&popped.len()) {
        return Err(ProcessingError::Arity {
            expected: counts,
            found: popped.len(),
        }
        .into());
    }
    if let Some(graphics) = found {
        popped.reverse();
        return Ok(Params {
            graphics,
            args: popped,
            counts,
        });
    }
    match popped.first() {
        None => Err(BoundsError::ListIndex {
            index: 0,
            length: 0,
        }
        .into()),
        Some(Value::Null) => Err(Error::null_pointer()),
        Some(other) => Err(Error::class_cast(other.kind(), Class::Graphics)),
    }
}

/// `((Number) value).floatValue()`: the `float` of a number. A `DOUBLE` is narrowed to `f32` as
/// Java does; a value that is not finite, or that overflows `f32`, is refused.
pub fn float_of(value: &Value) -> Result<f32> {
    match value {
        Value::Long(long) => finite(*long as f32),
        Value::Double(double) => finite(ensure_finite(*double)? as f32),
        Value::BigDecimal(decimal) => finite(ensure_finite(decimal.double_value())? as f32),
        Value::Null => Err(Error::null_pointer()),
        other => Err(Error::class_cast(other.kind(), Class::Number)),
    }
}

/// `((Number) value).intValue()`: a `LONG` keeps its low 32 bits, a `DOUBLE` saturates (as the
/// JVM's `d2i` does). A value that is not finite is refused.
pub fn int_of(value: &Value) -> Result<i32> {
    match value {
        Value::Long(long) => Ok(*long as i32),
        Value::Double(double) => Ok(ensure_finite(*double)? as i32),
        Value::BigDecimal(decimal) => Ok(decimal.long_value() as i32),
        Value::Null => Err(Error::null_pointer()),
        other => Err(Error::class_cast(other.kind(), Class::Number)),
    }
}

impl Params {
    /// The number of values above the graphics.
    pub fn len(&self) -> usize {
        self.args.len()
    }

    /// Whether no value was given above the graphics.
    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    /// The arity failure: the values given are not a count the function accepts.
    fn arity(&self) -> Error {
        ProcessingError::Arity {
            expected: self.counts,
            found: self.args.len(),
        }
        .into()
    }

    /// The `N` values from the `skip`-th one on (the first `skip` are left out), as they are.
    /// Asking for another count than the values given is an arity failure.
    pub fn values<const N: usize>(&self, skip: usize) -> Result<&[Value; N]> {
        let rest = self.args.get(skip..).ok_or_else(|| self.arity())?;
        rest.try_into().map_err(|_| self.arity())
    }

    /// The `N` values above the graphics, from the `skip`-th one on, as `float`s.
    pub fn floats_after<const N: usize>(&self, skip: usize) -> Result<[f32; N]> {
        let values = self.values::<N>(skip)?;
        let mut out = [0.0; N];
        for (slot, value) in out.iter_mut().zip(values) {
            *slot = float_of(value)?;
        }
        Ok(out)
    }

    /// The `N` values above the graphics, as `float`s.
    pub fn floats<const N: usize>(&self) -> Result<[f32; N]> {
        self.floats_after(0)
    }

    /// The `N` values above the graphics, as `int`s.
    pub fn ints<const N: usize>(&self) -> Result<[i32; N]> {
        let values = self.values::<N>(0)?;
        let mut out = [0; N];
        for (slot, value) in out.iter_mut().zip(values) {
            *slot = int_of(value)?;
        }
        Ok(out)
    }

    /// The keyword the value at `index` spells (`params.get(index).toString()`): the text of a
    /// `STRING`, `None` for any other value (which matches no keyword), and a `null` fails as a
    /// `NullPointerException`.
    pub fn keyword(&self, index: usize) -> Result<Option<&str>> {
        match self.args.get(index) {
            Some(Value::Null) => Err(Error::null_pointer()),
            Some(value) => Ok(value.as_str()),
            None => Err(self.arity()),
        }
    }
}

/// The mode a keyword names: the value above the graphics, as text, is compared with each name
/// of `modes` (Warp 10's `params.get(1).toString()`). Any other text is refused with `message`,
/// which carries Warp 10's own separator.
pub fn mode_of<T: Copy>(
    params: &Params,
    function: &'static str,
    message: &'static str,
    modes: &[(&str, T)],
) -> Result<T> {
    let keyword = params.keyword(0)?;
    modes
        .iter()
        .find(|(name, _)| Some(*name) == keyword)
        .map(|(_, mode)| *mode)
        .ok_or_else(|| ProcessingError::Refused { function, message }.into())
}

/// Push the graphics back, then the result above it.
pub fn answer(stack: &mut Stack, graphics: SharedGraphics, result: impl Into<Value>) {
    stack.push(Value::Graphics(graphics));
    stack.push(result);
}

/// Push the graphics back, with nothing above it.
pub fn give_back(stack: &mut Stack, graphics: SharedGraphics) {
    stack.push(Value::Graphics(graphics));
}

/// Whether a graphics type is `3D`: `Some(false)` for `2D`, `Some(true)` for `3D`, `None` for
/// anything else. Either may be followed by a smoothing level, which Processing reads with
/// `Integer.parseInt` (so `'2D4'` and `'2D-1'` pass and `'2Dx'` is a `NumberFormatException`).
fn graphics_type(kind: &str) -> Result<Option<bool>> {
    let (three_d, level) = if let Some(level) = kind.strip_prefix("2D") {
        (false, level)
    } else if let Some(level) = kind.strip_prefix("3D") {
        (true, level)
    } else {
        return Ok(None);
    };
    if !level.is_empty() {
        level
            .parse::<i32>()
            .map_err(|_| NumberFormatError::InvalidInput(level.to_owned()))?;
    }
    Ok(Some(three_d))
}

/// `PGraphics`: a new graphics context from `width height type`, the type on top of the stack
/// (`'2D'` or `'3D'`, optionally followed by a smoothing level).
fn pgraphics(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PGraphics";
    let kind = match stack.pop()? {
        Value::String(kind) => graphics_type(&kind)?,
        _ => None,
    };
    let Some(three_d) = kind else {
        return Err(fail(
            NAME,
            "expects a type ('2D' or '3D') on top of the stack.",
        ));
    };
    let Value::Long(height) = stack.pop()? else {
        return Err(fail(NAME, "expects a height in pixels below the type."));
    };
    let Value::Long(width) = stack.pop()? else {
        return Err(fail(NAME, "expects a width in pixels below the height."));
    };
    // Processing keeps the `int` of each size: the low 32 bits of the `LONG`.
    let (width, height) = (width as i32, height as i32);
    let pixels = i64::from(width) * i64::from(height);
    let limit = stack.limits.max_pixels;
    if pixels > limit {
        return Err(ProcessingError::TooManyPixels {
            function: NAME,
            allowed: "PGRAPHICS",
            limit,
            width: i64::from(width),
            height: i64::from(height),
            pixels,
        }
        .into());
    }
    if width <= 0 || height <= 0 {
        return Err(ProcessingError::EmptyImage {
            width: i64::from(width),
            height: i64::from(height),
        }
        .into());
    }
    let graphics = Graphics::new(
        width.unsigned_abs() as usize,
        height.unsigned_abs() as usize,
        three_d,
    )?;
    stack.push(Value::Graphics(Rc::new(RefCell::new(graphics))));
    Ok(())
}

/// `PpushStyle`: save the drawing style, to be restored by `PpopStyle`.
fn ppushstyle(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    {
        let mut graphics = params.graphics.borrow_mut();
        let saved = graphics.style.clone();
        graphics.saved_styles.push(saved);
    }
    give_back(stack, params.graphics);
    Ok(())
}

/// `PpopStyle`: restore the style saved by the last `PpushStyle`.
fn ppopstyle(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    {
        let mut graphics = params.graphics.borrow_mut();
        let Some(style) = graphics.saved_styles.pop() else {
            return Err(ProcessingError::StyleUnderflow.into());
        };
        graphics.style = style;
    }
    give_back(stack, params.graphics);
    Ok(())
}

/// `PblendMode`: how a drawing is composed with the pixels under it.
fn pblendmode(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let mode = match params.keyword(0)? {
        Some("BLEND") => Blend::Blend,
        Some("ADD") => Blend::Add,
        Some("SUBTRACT") => Blend::Subtract,
        Some("DARKEST") => Blend::Darkest,
        Some("LIGHTEST") => Blend::Lightest,
        Some("DIFFERENCE") => Blend::Difference,
        Some("EXCLUSION") => Blend::Exclusion,
        Some("MULTIPLY") => Blend::Multiply,
        Some("SCREEN") => Blend::Screen,
        Some("REPLACE") => Blend::Replace,
        _ => {
            return Err(ProcessingError::Refused {
                function: "PblendMode",
                message: ": invalid mode, should be 'BLEND', 'ADD', 'SUBTRACT', 'DARKEST', 'LIGHTEST', 'DIFFERENCE', 'EXCLUSION', 'MULTIPLY', 'SCREEN', or 'REPLACE'.",
            }
            .into());
        }
    };
    params.graphics.borrow_mut().style.blend = mode;
    give_back(stack, params.graphics);
    Ok(())
}

/// `Pclip`: limit the drawing to the rectangle `x y width height`, in the current coordinates.
/// The clip set before is intersected with it, as Java2D does.
fn pclip(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[4])?;
    let [x, y, width, height] = params.floats()?;
    {
        let mut graphics = params.graphics.borrow_mut();
        let [x0, y0, _] = graphics.transform.apply(x, y, 0.0)?;
        let [x1, y1, _] = graphics.transform.apply(x + width, y + height, 0.0)?;
        let rectangle = Clip::between([x0, y0], [x1, y1]);
        graphics.clip = Some(match graphics.clip {
            Some(clip) => clip.intersect(&rectangle),
            None => rectangle,
        });
    }
    give_back(stack, params.graphics);
    Ok(())
}

/// `PnoClip`: lift the clip.
fn pnoclip(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    params.graphics.borrow_mut().clip = None;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PApplet.constrain`: `value < low ? low : (value > high ? high : value)`, in that order.
fn constrain<T: PartialOrd>(value: T, low: T, high: T) -> T {
    if value < low {
        low
    } else if value > high {
        high
    } else {
        value
    }
}

/// `Pconstrain`: a value limited to `low..=high`; a `LONG` when the value is a `LONG`.
fn pconstrain(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[3])?;
    let result = if matches!(params.args.first(), Some(Value::Long(_))) {
        let [value, low, high] = params.ints()?;
        Value::Long(i64::from(constrain(value, low, high)))
    } else {
        let [value, low, high] = params.floats()?;
        Value::Double(f64::from(constrain(value, low, high)))
    };
    answer(stack, params.graphics, result);
    Ok(())
}

/// A `float` result as a `DOUBLE`, refused when it is not finite (a division by zero, an
/// overflow).
fn double_of(result: f32) -> Result<Value> {
    Ok(Value::Double(f64::from(finite(result)?)))
}

/// `Pdist`: the distance between two points in the plane (4 values).
///
/// Warp 10 also accepts six values (two points in space), but its dispatch compares the number
/// of values with the length of the list it was given, which is one more: a six-value call never
/// computes a distance and pushes `0.0`. That is kept, so that scripts get the same results.
fn pdist(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[4, 6])?;
    let distance = if params.len() == 4 {
        let [x1, y1, x2, y2] = params.floats()?;
        let (dx, dy) = (x2 - x1, y2 - y1);
        double_of((dx * dx + dy * dy).sqrt())?
    } else {
        Value::Double(0.0)
    };
    answer(stack, params.graphics, distance);
    Ok(())
}

/// `Plerp`: `start + (stop - start) * amount`.
fn plerp(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[3])?;
    let [start, stop, amount] = params.floats()?;
    let lerped = start + (stop - start) * amount;
    answer(stack, params.graphics, double_of(lerped)?);
    Ok(())
}

/// `Pmag`: the length of a vector, in the plane (2 values) or in space (3).
fn pmag(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[2, 3])?;
    let length = if params.len() == 2 {
        let [a, b] = params.floats()?;
        (a * a + b * b).sqrt()
    } else {
        let [a, b, c] = params.floats()?;
        (a * a + b * b + c * c).sqrt()
    };
    answer(stack, params.graphics, double_of(length)?);
    Ok(())
}

/// `Pmap`: a value moved from the range `start1..stop1` to the range `start2..stop2`.
fn pmap(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[5])?;
    let [value, start1, stop1, start2, stop2] = params.floats()?;
    let mapped = start2 + (stop2 - start2) * ((value - start1) / (stop1 - start1));
    answer(stack, params.graphics, double_of(mapped)?);
    Ok(())
}

/// `Pnorm`: a value as a fraction of the range `start..stop`.
fn pnorm(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[3])?;
    let [value, start, stop] = params.floats()?;
    answer(
        stack,
        params.graphics,
        double_of((value - start) / (stop - start))?,
    );
    Ok(())
}
