// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Processing transform functions: the matrix stack (`pushMatrix`, `popMatrix`,
//! `resetMatrix`) and the transforms that compose into the current matrix (`translate`,
//! `rotate` and its per-axis forms, `scale`, `shearX`, `shearY`).

use super::Entry;
use super::processing::{Params, give_back, parse_params};
use crate::error::Result;
use crate::error::processing::ProcessingError;
use crate::processing::geometry::Transform;
use crate::stack::Stack;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PpushMatrix", Entry::Stack(ppushmatrix)),
    ("PpopMatrix", Entry::Stack(ppopmatrix)),
    ("PresetMatrix", Entry::Stack(presetmatrix)),
    (
        "Protate",
        Entry::Stack(|stack| turn(stack, Transform::rotate_z)),
    ),
    (
        "ProtateX",
        Entry::Stack(|stack| turn(stack, Transform::rotate_x)),
    ),
    (
        "ProtateY",
        Entry::Stack(|stack| turn(stack, Transform::rotate_y)),
    ),
    (
        "ProtateZ",
        Entry::Stack(|stack| turn(stack, Transform::rotate_z)),
    ),
    ("Pscale", Entry::Stack(pscale)),
    (
        "PshearX",
        Entry::Stack(|stack| shear(stack, Transform::shear_x)),
    ),
    (
        "PshearY",
        Entry::Stack(|stack| shear(stack, Transform::shear_y)),
    ),
    ("Ptranslate", Entry::Stack(ptranslate)),
];

/// The depth of the matrix stack, which Processing does not let `pushMatrix` exceed.
const MATRIX_STACK_DEPTH: usize = 32;

/// Compose `next` after the current matrix (`current = current * next`), keeping the result
/// finite.
fn compose(params: &Params, next: &Transform) -> Result<()> {
    let mut graphics = params.graphics.borrow_mut();
    graphics.transform = graphics.transform.then(next)?;
    Ok(())
}

/// A transform of one angle (`rotate`, `rotateX`…), composed into the current matrix.
fn turn(stack: &mut Stack, transform: fn(f32) -> Transform) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let [angle] = params.floats()?;
    compose(&params, &transform(angle))?;
    give_back(stack, params.graphics);
    Ok(())
}

/// A shear of one angle, composed into the current matrix. The tangent of the angle may be
/// refused, which is why the transform is built fallibly.
fn shear(stack: &mut Stack, transform: fn(f32) -> Result<Transform>) -> Result<()> {
    let params = parse_params(stack, &[1])?;
    let [angle] = params.floats()?;
    compose(&params, &transform(angle)?)?;
    give_back(stack, params.graphics);
    Ok(())
}

/// `pushMatrix`: save the current matrix, to be restored by `popMatrix`.
fn ppushmatrix(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    {
        let mut graphics = params.graphics.borrow_mut();
        if graphics.saved_transforms.len() == MATRIX_STACK_DEPTH {
            return Err(ProcessingError::MatrixOverflow.into());
        }
        let current = graphics.transform;
        graphics.saved_transforms.push(current);
    }
    give_back(stack, params.graphics);
    Ok(())
}

/// `popMatrix`: restore the matrix saved by the last `pushMatrix`.
fn ppopmatrix(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    {
        let mut graphics = params.graphics.borrow_mut();
        let Some(saved) = graphics.saved_transforms.pop() else {
            return Err(ProcessingError::MatrixUnderflow.into());
        };
        graphics.transform = saved;
    }
    give_back(stack, params.graphics);
    Ok(())
}

/// `resetMatrix`: the identity becomes the current matrix.
fn presetmatrix(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[0])?;
    params.graphics.borrow_mut().transform = Transform::identity();
    give_back(stack, params.graphics);
    Ok(())
}

/// `scale`: one value scales every axis alike, two scale the plane, three scale space.
fn pscale(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[1, 2, 3])?;
    let transform = match params.len() {
        1 => {
            let [factor] = params.floats()?;
            Transform::scale(factor, factor, factor)
        }
        2 => {
            let [x, y] = params.floats()?;
            Transform::scale(x, y, 1.0)
        }
        _ => {
            let [x, y, z] = params.floats()?;
            Transform::scale(x, y, z)
        }
    };
    compose(&params, &transform)?;
    give_back(stack, params.graphics);
    Ok(())
}

/// `translate`: a move in the plane (two values) or in space (three).
fn ptranslate(stack: &mut Stack) -> Result<()> {
    let params = parse_params(stack, &[2, 3])?;
    let transform = match params.len() {
        2 => {
            let [x, y] = params.floats()?;
            Transform::translate(x, y, 0.0)
        }
        _ => {
            let [x, y, z] = params.floats()?;
            Transform::translate(x, y, z)
        }
    };
    compose(&params, &transform)?;
    give_back(stack, params.graphics);
    Ok(())
}
