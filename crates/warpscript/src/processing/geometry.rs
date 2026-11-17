// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Transforms of a graphics context: `translate`, `rotate`, `scale` and `shear` compose into
//! the current matrix, which maps each drawn point. A point is multiplied by the most recent
//! transform first, as in Processing.

use super::finite;
use crate::error::Result;

/// A 4 × 4 matrix of `f32`, row by row, acting on the column vector `(x, y, z, 1)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    rows: [[f32; 4]; 4],
}

impl Transform {
    /// The identity: points are unchanged.
    pub fn identity() -> Self {
        Transform {
            rows: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// The transform that applies `self` after `next`: `self * next` (`applyMatrix` composes
    /// this way). Every entry must stay finite.
    pub fn then(&self, next: &Transform) -> Result<Transform> {
        let mut rows = [[0.0; 4]; 4];
        for (row, out) in rows.iter_mut().enumerate() {
            for (col, cell) in out.iter_mut().enumerate() {
                let sum: f32 = (0..4).map(|k| self.rows[row][k] * next.rows[k][col]).sum();
                *cell = finite(sum)?;
            }
        }
        Ok(Transform { rows })
    }

    /// `translate(x, y, z)`.
    pub fn translate(x: f32, y: f32, z: f32) -> Self {
        let mut t = Transform::identity();
        t.rows[0][3] = x;
        t.rows[1][3] = y;
        t.rows[2][3] = z;
        t
    }

    /// `scale(x, y, z)`.
    pub fn scale(x: f32, y: f32, z: f32) -> Self {
        let mut t = Transform::identity();
        t.rows[0][0] = x;
        t.rows[1][1] = y;
        t.rows[2][2] = z;
        t
    }

    /// `rotate(angle)` and `rotateZ(angle)`: a turn in the plane of the screen.
    pub fn rotate_z(angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        let mut t = Transform::identity();
        t.rows[0][0] = cos;
        t.rows[0][1] = -sin;
        t.rows[1][0] = sin;
        t.rows[1][1] = cos;
        t
    }

    /// `rotateX(angle)`: a turn around the horizontal axis.
    pub fn rotate_x(angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        let mut t = Transform::identity();
        t.rows[1][1] = cos;
        t.rows[1][2] = -sin;
        t.rows[2][1] = sin;
        t.rows[2][2] = cos;
        t
    }

    /// `rotateY(angle)`: a turn around the vertical axis.
    pub fn rotate_y(angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        let mut t = Transform::identity();
        t.rows[0][0] = cos;
        t.rows[0][2] = sin;
        t.rows[2][0] = -sin;
        t.rows[2][2] = cos;
        t
    }

    /// `shearX(angle)`: `x` moves by `tan(angle)` times `y`. The tangent is checked: a
    /// finite angle has a finite tangent, but the check keeps an infinite one out of the matrix.
    pub fn shear_x(angle: f32) -> Result<Self> {
        let mut t = Transform::identity();
        t.rows[0][1] = finite(angle.tan())?;
        Ok(t)
    }

    /// `shearY(angle)`: `y` moves by `tan(angle)` times `x` (see [`Transform::shear_x`]).
    pub fn shear_y(angle: f32) -> Result<Self> {
        let mut t = Transform::identity();
        t.rows[1][0] = finite(angle.tan())?;
        Ok(t)
    }

    /// The inverse of the transform on the plane `z = 0`, as the six coefficients `[a, b, c, d,
    /// e, f]` of `x = a·x' + b·y' + c` and `y = d·x' + e·y' + f`: the drawing point `(x', y')` in
    /// the plane of the context. `None` when the transform flattens the plane (no inverse).
    pub fn inverse_plane(&self) -> Result<Option<[f32; 6]>> {
        let rows = self.rows;
        let (offset_x, offset_y) = (rows[0][3], rows[1][3]);
        let det = finite(rows[0][0] * rows[1][1] - rows[0][1] * rows[1][0])?;
        if det == 0.0 {
            return Ok(None);
        }
        let coefficients = [
            rows[1][1] / det,
            -rows[0][1] / det,
            (rows[0][1] * offset_y - rows[1][1] * offset_x) / det,
            -rows[1][0] / det,
            rows[0][0] / det,
            (rows[1][0] * offset_x - rows[0][0] * offset_y) / det,
        ];
        let mut inverse = [0.0; 6];
        for (slot, value) in inverse.iter_mut().zip(coefficients) {
            *slot = finite(value)?;
        }
        Ok(Some(inverse))
    }

    /// The transformed point `(x, y, z)`: each coordinate must stay finite.
    pub fn apply(&self, x: f32, y: f32, z: f32) -> Result<[f32; 3]> {
        let row = |r: usize| {
            let m = &self.rows[r];
            finite(m[0] * x + m[1] * y + m[2] * z + m[3])
        };
        Ok([row(0)?, row(1)?, row(2)?])
    }
}
