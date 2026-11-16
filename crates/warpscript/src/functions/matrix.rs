// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0) and Apache Commons Math
// 3.6.1 (Copyright 2001-2016 The Apache Software Foundation, Apache-2.0), ported to Rust and
// modified.

//! Matrices and vectors (Commons Math's `Array2DRowRealMatrix` / `ArrayRealVector`): `->MAT`,
//! `MAT->`, `TR`, `TRANSPOSE`, `DET`, `INV`, `->VEC`, `VEC->`, and their arithmetic for the
//! operators.

use std::rc::Rc;

use super::Entry;
use super::args::fail;
use crate::error::Result;
use crate::error::fault::BoundsError;
use crate::error::math::{MathError, message_number};
use crate::stack::Stack;
use crate::value::Value;

/// A dense matrix of doubles, row major.
#[derive(Clone, Debug)]
pub struct Matrix {
    /// The number of rows.
    pub rows: usize,
    /// The number of columns.
    pub cols: usize,
    /// The entries, row by row.
    pub data: Vec<f64>,
}

impl Matrix {
    /// The entry at `(row, col)`.
    pub fn get(&self, row: usize, col: usize) -> f64 {
        self.data[row * self.cols + col]
    }

    /// `AbstractRealMatrix.equals`: same dimensions and entries (`==`, so `NaN` differs).
    #[allow(clippy::float_cmp)]
    pub fn same_entries(&self, other: &Matrix) -> bool {
        self.rows == other.rows
            && self.cols == other.cols
            && self.data.iter().zip(&other.data).all(|(a, b)| a == b)
    }

    /// `Array2DRowRealMatrix.toString`: `Array2DRowRealMatrix{{1.0,2.0},{3.0,4.0}}`.
    pub fn matrix_text(&self) -> String {
        let rows: Vec<String> = (0..self.rows)
            .map(|r| {
                let entries: Vec<String> = (0..self.cols)
                    .map(|c| crate::format::double_to_text(self.get(r, c)))
                    .collect();
                format!("{{{}}}", entries.join(","))
            })
            .collect();
        format!("Array2DRowRealMatrix{{{}}}", rows.join(","))
    }
}

/// `ArrayRealVector.equals`: same dimension and entries (`NaN` vectors are equal).
#[allow(clippy::float_cmp)]
pub fn same_vector(a: &[f64], b: &[f64]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let nan = |v: &[f64]| v.iter().any(|x| x.is_nan());
    if nan(a) || nan(b) {
        return nan(a) && nan(b);
    }
    a.iter().zip(b).all(|(x, y)| x == y)
}

/// `ArrayRealVector.toString` (`RealVectorFormat`): `{1; 2.5}`, the entries with at most ten
/// fraction digits.
pub fn vector_string(v: &[f64]) -> String {
    let entries: Vec<String> = v
        .iter()
        .map(|x| {
            if x.is_finite() {
                let text = format!("{:.10}", x.abs());
                let text = text.trim_end_matches('0').trim_end_matches('.');
                let (int, frac) = text.split_once('.').unwrap_or((text, ""));
                let mut out = message_number(int.parse::<f64>().unwrap_or_default());
                if !frac.is_empty() {
                    out.push('.');
                    out.push_str(frac);
                }
                // `DecimalFormat` keeps the sign of a value rounding to zero (`-0`).
                if x.is_sign_negative() {
                    out.insert(0, '-');
                }
                out
            } else {
                // `CompositeFormat.formatDouble`: `(NaN)`, `(Infinity)`.
                format!("({})", Value::Double(*x))
            }
        })
        .collect();
    format!("{{{}}}", entries.join("; "))
}

/// A matrix value.
pub fn matrix_value(m: Matrix) -> Value {
    Value::Matrix(Rc::new(m))
}

/// A vector value.
pub fn vector_value(v: Vec<f64>) -> Value {
    Value::Vector(Rc::from(v))
}

/// `add` / `subtract` of two matrices of the same dimensions.
pub fn add_matrices(a: &Matrix, b: &Matrix, sign: f64) -> Result<Matrix> {
    if a.rows != b.rows || a.cols != b.cols {
        return Err(MathError::MatrixDimensionMismatch {
            rows: a.rows,
            cols: a.cols,
            expected_rows: b.rows,
            expected_cols: b.cols,
        }
        .into());
    }
    Ok(Matrix {
        rows: a.rows,
        cols: a.cols,
        data: a
            .data
            .iter()
            .zip(&b.data)
            .map(|(x, y)| if sign > 0.0 { x + y } else { x - y })
            .collect(),
    })
}

/// `scalarAdd` / `scalarMultiply`.
pub fn map_matrix(m: &Matrix, f: impl Fn(f64) -> f64) -> Matrix {
    Matrix {
        rows: m.rows,
        cols: m.cols,
        data: m.data.iter().map(|&x| f(x)).collect(),
    }
}

/// `Array2DRowRealMatrix.multiply`.
pub fn multiply(a: &Matrix, b: &Matrix) -> Result<Matrix> {
    if a.cols != b.rows {
        return Err(MathError::DimensionMismatch {
            wrong: a.cols,
            expected: b.rows,
        }
        .into());
    }
    let mut data = vec![0.0; a.rows * b.cols];
    for col in 0..b.cols {
        for row in 0..a.rows {
            let mut sum = 0.0;
            for i in 0..a.cols {
                sum += a.get(row, i) * b.get(i, col);
            }
            data[row * b.cols + col] = sum;
        }
    }
    Ok(Matrix {
        rows: a.rows,
        cols: b.cols,
        data,
    })
}

/// `operate`: the matrix times a column vector.
pub fn operate(m: &Matrix, v: &[f64]) -> Result<Vec<f64>> {
    if v.len() != m.cols {
        return Err(MathError::DimensionMismatch {
            wrong: v.len(),
            expected: m.cols,
        }
        .into());
    }
    Ok((0..m.rows)
        .map(|row| (0..m.cols).fold(0.0, |sum, i| sum + m.get(row, i) * v[i]))
        .collect())
}

/// `preMultiply`: a row vector times the matrix.
pub fn pre_multiply(m: &Matrix, v: &[f64]) -> Result<Vec<f64>> {
    if v.len() != m.rows {
        return Err(MathError::DimensionMismatch {
            wrong: v.len(),
            expected: m.rows,
        }
        .into());
    }
    Ok((0..m.cols)
        .map(|col| (0..m.rows).fold(0.0, |sum, i| sum + m.get(i, col) * v[i]))
        .collect())
}

/// `ArrayRealVector.add` / `subtract`.
pub fn add_vectors(a: &[f64], b: &[f64], sign: f64) -> Result<Vec<f64>> {
    if a.len() != b.len() {
        return Err(MathError::DimensionMismatch {
            wrong: a.len(),
            expected: b.len(),
        }
        .into());
    }
    Ok(a.iter()
        .zip(b)
        .map(|(x, y)| if sign > 0.0 { x + y } else { x - y })
        .collect())
}

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("->MAT", Entry::Stack(tomat)),
    ("MAT->", Entry::Stack(matto)),
    ("TR", Entry::Stack(tr)),
    ("TRANSPOSE", Entry::Stack(transpose)),
    ("DET", Entry::Stack(det)),
    ("INV", Entry::Stack(inv)),
    ("->VEC", Entry::Stack(tovec)),
    ("VEC->", Entry::Stack(vecto)),
];

/// `->MAT`: the matrix of a list of rows of numbers, or the column matrix of a vector.
fn tomat(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "->MAT";
    const ARRAY: &str = "expects a 2D array onto the stack.";
    let rows = match stack.pop()? {
        Value::Vector(v) => {
            if v.is_empty() {
                return Err(MathError::NotStrictlyPositive {
                    value: 0,
                    minimum: 0,
                }
                .into());
            }
            stack.push(matrix_value(Matrix {
                rows: v.len(),
                cols: 1,
                data: v.to_vec(),
            }));
            return Ok(());
        }
        Value::List(rows) => rows.borrow().clone(),
        _ => return Err(fail(NAME, ARRAY)),
    };
    let mut cols: Option<usize> = None;
    let mut lists = Vec::with_capacity(rows.len());
    for row in &rows {
        let Value::List(row) = row else {
            return Err(fail(NAME, ARRAY));
        };
        lists.push(Rc::clone(row));
        let len = row.borrow().len();
        match cols {
            None => cols = Some(len),
            Some(c) if c != len => {
                return Err(fail(
                    NAME,
                    "expects a common number of columns throughout the 2D array.",
                ));
            }
            Some(_) => {}
        }
    }
    let Some(cols) = cols else {
        // `new double[0][-1]`.
        return Err(BoundsError::NegativeSize(-1).into());
    };
    let mut data = Vec::with_capacity(rows.len() * cols);
    for row in &lists {
        for value in row.borrow().iter() {
            let Some(x) = value.as_f64() else {
                return Err(fail(NAME, "expects a numeric 2D array onto the stack."));
            };
            data.push(x);
        }
    }
    if cols == 0 {
        return Err(MathError::MatrixWithoutColumn.into());
    }
    stack.push(matrix_value(Matrix {
        rows: rows.len(),
        cols,
        data,
    }));
    Ok(())
}

/// Pop a matrix.
fn pop_matrix(stack: &mut Stack, name: &str) -> Result<Rc<Matrix>> {
    match stack.pop()? {
        Value::Matrix(m) => Ok(m),
        _ => Err(fail(name, "expects a matrix on top of the stack.")),
    }
}

/// `MAT->`: the rows of a matrix as lists of doubles.
fn matto(stack: &mut Stack) -> Result<()> {
    let m = pop_matrix(stack, "MAT->")?;
    let rows = (0..m.rows)
        .map(|r| Value::list((0..m.cols).map(|c| Value::Double(m.get(r, c))).collect()))
        .collect();
    stack.push(Value::list(rows));
    Ok(())
}

/// `TR`: the trace of a square matrix.
fn tr(stack: &mut Stack) -> Result<()> {
    let m = pop_matrix(stack, "TR")?;
    if m.rows != m.cols {
        return Err(MathError::NonSquareMatrix {
            rows: m.rows,
            cols: m.cols,
        }
        .into());
    }
    let trace = (0..m.rows).fold(0.0, |sum, i| sum + m.get(i, i));
    stack.push(Value::Double(trace));
    Ok(())
}

/// `TRANSPOSE`.
fn transpose(stack: &mut Stack) -> Result<()> {
    let m = pop_matrix(stack, "TRANSPOSE")?;
    let mut data = Vec::with_capacity(m.data.len());
    for c in 0..m.cols {
        for r in 0..m.rows {
            data.push(m.get(r, c));
        }
    }
    stack.push(matrix_value(Matrix {
        rows: m.cols,
        cols: m.rows,
        data,
    }));
    Ok(())
}

/// `LUDecomposition` (partial pivoting, singularity threshold `1e-11`).
struct Lu {
    factors: Vec<Vec<f64>>,
    pivot: Vec<usize>,
    even: bool,
    singular: bool,
}

impl Lu {
    #[allow(clippy::needless_range_loop)] // the loops of Commons Math
    fn new(m: &Matrix) -> Result<Lu> {
        if m.rows != m.cols {
            return Err(MathError::NonSquareMatrix {
                rows: m.rows,
                cols: m.cols,
            }
            .into());
        }
        let n = m.cols;
        let mut lu: Vec<Vec<f64>> = (0..n)
            .map(|r| (0..n).map(|c| m.get(r, c)).collect())
            .collect();
        let mut pivot: Vec<usize> = (0..n).collect();
        let mut even = true;
        for col in 0..n {
            for row in 0..col {
                let mut sum = lu[row][col];
                for i in 0..row {
                    sum -= lu[row][i] * lu[i][col];
                }
                lu[row][col] = sum;
            }
            let mut max = col;
            let mut largest = f64::NEG_INFINITY;
            for row in col..n {
                let mut sum = lu[row][col];
                for i in 0..col {
                    sum -= lu[row][i] * lu[i][col];
                }
                lu[row][col] = sum;
                if sum.abs() > largest {
                    largest = sum.abs();
                    max = row;
                }
            }
            if lu[max][col].abs() < 1e-11 {
                return Ok(Lu {
                    factors: lu,
                    pivot,
                    even,
                    singular: true,
                });
            }
            if max != col {
                lu.swap(max, col);
                pivot.swap(max, col);
                even = !even;
            }
            let diagonal = lu[col][col];
            for row in lu.iter_mut().skip(col + 1) {
                row[col] /= diagonal;
            }
        }
        Ok(Lu {
            factors: lu,
            pivot,
            even,
            singular: false,
        })
    }

    fn determinant(&self) -> f64 {
        if self.singular {
            return 0.0;
        }
        let sign = if self.even { 1.0 } else { -1.0 };
        (0..self.factors.len()).fold(sign, |det, i| det * self.factors[i][i])
    }

    /// `getSolver().getInverse()`: the solution of `A X = I`.
    #[allow(clippy::needless_range_loop)] // the loops of Commons Math
    fn inverse(&self) -> Result<Matrix> {
        if self.singular {
            return Err(MathError::SingularMatrix.into());
        }
        let n = self.pivot.len();
        let mut bp: Vec<Vec<f64>> = (0..n)
            .map(|row| {
                (0..n)
                    .map(|col| if self.pivot[row] == col { 1.0 } else { 0.0 })
                    .collect()
            })
            .collect();
        for col in 0..n {
            for i in col + 1..n {
                let factor = self.factors[i][col];
                for j in 0..n {
                    bp[i][j] -= bp[col][j] * factor;
                }
            }
        }
        for col in (0..n).rev() {
            let diagonal = self.factors[col][col];
            for j in 0..n {
                bp[col][j] /= diagonal;
            }
            for i in 0..col {
                let factor = self.factors[i][col];
                for j in 0..n {
                    bp[i][j] -= bp[col][j] * factor;
                }
            }
        }
        Ok(Matrix {
            rows: n,
            cols: n,
            data: bp.into_iter().flatten().collect(),
        })
    }
}

/// `DET`: the determinant (`0.0` for a singular matrix).
fn det(stack: &mut Stack) -> Result<()> {
    let m = pop_matrix(stack, "DET")?;
    stack.push(Value::Double(Lu::new(&m)?.determinant()));
    Ok(())
}

/// `INV`: the inverse of a non singular square matrix.
fn inv(stack: &mut Stack) -> Result<()> {
    let m = pop_matrix(stack, "INV")?;
    stack.push(matrix_value(Lu::new(&m)?.inverse()?));
    Ok(())
}

/// `->VEC`: the vector of a list of numbers, or of a single column matrix.
fn tovec(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "->VEC";
    match stack.pop()? {
        Value::Matrix(m) => {
            if m.cols != 1 {
                return Err(fail(
                    NAME,
                    "expects a matrix with a single column on top of the stack.",
                ));
            }
            stack.push(vector_value(m.data.clone()));
            Ok(())
        }
        Value::List(list) => {
            let mut data = Vec::new();
            for value in list.borrow().iter() {
                let Some(x) = value.as_f64() else {
                    return Err(fail(NAME, "expects a list of numbers onto the stack."));
                };
                data.push(x);
            }
            stack.push(vector_value(data));
            Ok(())
        }
        _ => Err(fail(NAME, "expects a list onto the stack.")),
    }
}

/// `VEC->`: the entries of a vector.
fn vecto(stack: &mut Stack) -> Result<()> {
    let Value::Vector(v) = stack.pop()? else {
        return Err(fail("VEC->", "expects a vector on top of the stack."));
    };
    stack.push(Value::list(v.iter().map(|&x| Value::Double(x)).collect()));
    Ok(())
}
