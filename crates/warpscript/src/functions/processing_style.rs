// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Processing settings of the drawing style that are not colours: the weight, caps and joins
//! of strokes, the modes that place rectangles, ellipses, images and shapes, and the detail of
//! curves and spheres. Each one replaces a setting of the graphics context and pushes it back.

use super::Entry;
use super::processing::{give_back, mode_of, parse_params};
use crate::error::Result;
use crate::error::processing::ProcessingError;
use crate::processing::{Cap, Join, Mode};
use crate::stack::Stack;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PstrokeWeight", Entry::Stack(pstrokeweight)),
    ("PstrokeCap", Entry::Stack(pstrokecap)),
    ("PstrokeJoin", Entry::Stack(pstrokejoin)),
    ("PellipseMode", Entry::Stack(pellipsemode)),
    ("PrectMode", Entry::Stack(prectmode)),
    ("PimageMode", Entry::Stack(pimagemode)),
    ("PshapeMode", Entry::Stack(pshapemode)),
    ("PbezierDetail", Entry::Stack(pbezierdetail)),
    ("PcurveDetail", Entry::Stack(pcurvedetail)),
    ("PcurveTightness", Entry::Stack(pcurvetightness)),
    ("PsphereDetail", Entry::Stack(psspheredetail)),
];

/// `PstrokeWeight`: the width of the lines drawn, which Java2D refuses when it is negative.
fn pstrokeweight(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let [weight] = params.floats()?;
    if weight < 0.0 {
        return Err(ProcessingError::NegativeWidth.into());
    }
    params.graphics.borrow_mut().style.stroke_weight = weight;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PstrokeCap`: how the end of an open stroke is drawn.
fn pstrokecap(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let cap = mode_of(
        &params,
        "PstrokeCap",
        ": invalid mode, should be 'SQUARE', 'PROJECT' or 'ROUND'.",
        &[
            ("SQUARE", Cap::Square),
            ("PROJECT", Cap::Project),
            ("ROUND", Cap::Round),
        ],
    )?;
    params.graphics.borrow_mut().style.cap = cap;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PstrokeJoin`: how the corners of strokes are drawn.
fn pstrokejoin(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let join = mode_of(
        &params,
        "PstrokeJoin",
        ": invalid mode, should be 'MITER', 'BEVEL' or 'ROUND'.",
        &[
            ("MITER", Join::Miter),
            ("BEVEL", Join::Bevel),
            ("ROUND", Join::Round),
        ],
    )?;
    params.graphics.borrow_mut().style.join = join;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PellipseMode`: how the coordinates of an ellipse place it.
fn pellipsemode(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let mode = mode_of(
        &params,
        "PellipseMode",
        ": invalid mode, should be 'CENTER', 'RADIUS', 'CORNER' or 'CORNERS'.",
        &[
            ("CENTER", Mode::Center),
            ("RADIUS", Mode::Radius),
            ("CORNER", Mode::Corner),
            ("CORNERS", Mode::Corners),
        ],
    )?;
    params.graphics.borrow_mut().style.ellipse_mode = mode;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PrectMode`: how the coordinates of a rectangle place it.
fn prectmode(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let mode = mode_of(
        &params,
        "PrectMode",
        ": invalid mode, should be 'CENTER', 'RADIUS', 'CORNER' or 'CORNERS'.",
        &[
            ("CENTER", Mode::Center),
            ("RADIUS", Mode::Radius),
            ("CORNER", Mode::Corner),
            ("CORNERS", Mode::Corners),
        ],
    )?;
    params.graphics.borrow_mut().style.rect_mode = mode;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PimageMode`: how the coordinates of an image place it.
fn pimagemode(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let mode = mode_of(
        &params,
        "PimageMode",
        ": invalid mode, should be 'CENTER', 'CORNER' or 'CORNERS'.",
        &[
            ("CENTER", Mode::Center),
            ("CORNER", Mode::Corner),
            ("CORNERS", Mode::Corners),
        ],
    )?;
    params.graphics.borrow_mut().style.image_mode = mode;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PshapeMode`: how the coordinates of a shape place it.
fn pshapemode(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let mode = mode_of(
        &params,
        "PshapeMode",
        ": invalid mode, should be 'CENTER', 'CORNER' or 'CORNERS'.",
        &[
            ("CENTER", Mode::Center),
            ("CORNER", Mode::Corner),
            ("CORNERS", Mode::Corners),
        ],
    )?;
    params.graphics.borrow_mut().style.shape_mode = mode;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PbezierDetail`: the number of segments of each Bézier curve drawn, as Warp 10 stores it.
fn pbezierdetail(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let [detail] = params.ints()?;
    params.graphics.borrow_mut().style.bezier_detail = detail;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PcurveDetail`: the number of segments of each Catmull-Rom curve drawn, as Warp 10 stores it.
fn pcurvedetail(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let [detail] = params.ints()?;
    params.graphics.borrow_mut().style.curve_detail = detail;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PcurveTightness`: how tightly Catmull-Rom curves follow their points.
fn pcurvetightness(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let [tightness] = params.floats()?;
    params.graphics.borrow_mut().style.curve_tightness = tightness;
    give_back(stack, params.graphics);
    Ok(())
}

/// `PsphereDetail`: the divisions of spheres, one value for both directions or one for each.
fn psspheredetail(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 2])?;
    let detail = if params.len() == 1 {
        let [divisions] = params.ints()?;
        (divisions, divisions)
    } else {
        let [around, pole_to_pole] = params.ints()?;
        (around, pole_to_pole)
    };
    params.graphics.borrow_mut().style.sphere_detail = detail;
    give_back(stack, params.graphics);
    Ok(())
}
