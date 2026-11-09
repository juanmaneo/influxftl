// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the matrix and vector functions and operators.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const A: &str = r"[ [ 4 3 ] [ 6 3 ] ] ->MAT";
const B: &str = r"[ [ 1 2 3 ] [ 4 5 6 ] ] ->MAT";
const C: &str = r"[ [ 2 -1 0 ] [ -1 2 -1 ] [ 0 -1 2 ] ] ->MAT";
const P: &str = r"[ [ 0 1 2 ] [ 1 0 3 ] [ 4 -3 8 ] ] ->MAT";
const S: &str = r"[ [ 1 2 ] [ 2 4 ] ] ->MAT";
const V: &str = r"[ 1 2 ] ->VEC";
const W: &str = r"[ 0.5 -1 3 ] ->VEC";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@A", A)
        .replace("@B", B)
        .replace("@C", C)
        .replace("@P", P)
        .replace("@S", S)
        .replace("@V", V)
        .replace("@W", W)
}

/// The stack left by `script`, top first, as the JSON array of the Warp 10 exec endpoint,
/// or the error message.
fn stack_json(script: &str) -> Result<String, String> {
    let mut stack = test_stack();
    stack
        .exec_checked(&format!("{script} DEPTH ->LIST REVERSE ->JSON"))
        .map_err(|err| err.to_string())?;
    match stack.pop().expect("pop should succeed") {
        Value::String(json) => Ok(json.to_string()),
        other => panic!("not a string: {other:?}"),
    }
}

/// Check every case, reporting all the mismatches at once.
fn check(cases: &[(&str, Result<&str, &str>)]) {
    let mut failures = Vec::new();
    for (template, expected) in cases {
        let script = expand(template);
        match (expected, stack_json(&script)) {
            (Ok(json), Ok(actual)) if actual == *json => {}
            (Err(message), Err(actual)) if actual.contains(message) => {}
            (expected, actual) => failures.push(format!(
                "{template}\n  expected {expected:?}\n  actual   {actual:?}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn matrices() {
    check(&[
        (r"@A", Ok(r"[[[4.0,3.0],[6.0,3.0]]]")),
        (r"@A MAT->", Ok(r"[[[4.0,3.0],[6.0,3.0]]]")),
        (
            r"@A SNAPSHOT",
            Ok(r#"["[] [] 4.0 +! 3.0 +! +! [] 6.0 +! 3.0 +! +! ->MAT "]"#),
        ),
        (r"@A TYPEOF", Ok(r#"["MATRIX"]"#)),
        (r"@A ->JSON", Ok(r#"["[[4.0,3.0],[6.0,3.0]]"]"#)),
        (r"@V", Ok(r"[[1.0,2.0]]")),
        (r"@V VEC->", Ok(r"[[1.0,2.0]]")),
        (r"@V SNAPSHOT", Ok(r#"["[] 1.0 +! 2.0 +! ->VEC "]"#)),
        (r"@V TYPEOF", Ok(r#"["VECTOR"]"#)),
        (r"@V ->JSON", Ok(r#"["[1.0,2.0]"]"#)),
        (r"@A TR", Ok(r"[7.0]")),
        (r"@C TR", Ok(r"[6.0]")),
        (r"@B TR", Err(r"non square (2x3) matrix")),
        (r"@A TRANSPOSE", Ok(r"[[[4.0,6.0],[3.0,3.0]]]")),
        (r"@B TRANSPOSE", Ok(r"[[[1.0,4.0],[2.0,5.0],[3.0,6.0]]]")),
        (r"@A DET", Ok(r"[-6.0]")),
        (r"@C DET", Ok(r"[4.0]")),
        (r"@P DET", Ok(r"[-2.0]")),
        (r"@S DET", Ok(r"[0.0]")),
        (r"@B DET", Err(r"non square (2x3) matrix")),
        (r"@A INV", Ok(r"[[[-0.5,0.5],[1.0,-0.6666666666666666]]]")),
        (
            r"@C INV",
            Ok(
                r"[[[0.75,0.5,0.24999999999999997],[0.5,1.0,0.49999999999999994],[0.24999999999999997,0.49999999999999994,0.7499999999999999]]]",
            ),
        ),
        (
            r"@P INV",
            Ok(r"[[[-4.5,7.0,-1.5],[-2.0,4.0,-1.0],[1.5,-2.0,0.5]]]"),
        ),
        (r"@S INV", Err(r"matrix is singular")),
        (r"@B INV", Err(r"non square (2x3) matrix")),
        (r"[ [ 5 ] ] ->MAT INV", Ok(r"[[[0.2]]]")),
        (r"[ [ 0 ] ] ->MAT DET", Ok(r"[0.0]")),
        (r"[ [ 1e-12 ] ] ->MAT INV", Err(r"matrix is singular")),
        (r"@A @A +", Ok(r"[[[8.0,6.0],[12.0,6.0]]]")),
        (r"@A @B +", Err(r"got 2x2 but expected 2x3")),
        (r"@A 2 +", Ok(r"[[[6.0,5.0],[8.0,5.0]]]")),
        (r"2 @A +", Ok(r"[[[6.0,5.0],[8.0,5.0]]]")),
        (r"@A 1.5 +", Ok(r"[[[5.5,4.5],[7.5,4.5]]]")),
        (r"@V @V +", Ok(r"[[2.0,4.0]]")),
        (r"@V @W +", Err(r"2 != 3")),
        (r"@V 2 +", Ok(r"[[3.0,4.0]]")),
        (r"2 @V +", Ok(r"[[3.0,4.0]]")),
        (r"@A @A -", Ok(r"[[[0.0,0.0],[0.0,0.0]]]")),
        (r"@B @B -", Ok(r"[[[0.0,0.0,0.0],[0.0,0.0,0.0]]]")),
        (r"@A @B -", Err(r"got 2x2 but expected 2x3")),
        (r"@V @V -", Ok(r"[[0.0,0.0]]")),
        (
            r"@A 2 -",
            Err(
                r"- can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"@V 2 -",
            Err(
                r"- can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (r"@A @A *", Ok(r"[[[34.0,21.0],[42.0,27.0]]]")),
        (r"@A @B *", Ok(r"[[[16.0,23.0,30.0],[18.0,27.0,36.0]]]")),
        (r"@B @A *", Err(r"3 != 2")),
        (r"@A 2 *", Ok(r"[[[8.0,6.0],[12.0,6.0]]]")),
        (r"2 @A *", Ok(r"[[[8.0,6.0],[12.0,6.0]]]")),
        (r"@V @A *", Ok(r"[[16.0,9.0]]")),
        (r"@A @V *", Ok(r"[[10.0,12.0]]")),
        (r"@W @B *", Err(r"3 != 2")),
        (r"@B @W *", Ok(r"[[7.5,15.0]]")),
        (r"@V @B *", Ok(r"[[9.0,12.0,15.0]]")),
        (r"@B @V *", Err(r"2 != 3")),
        (r"@V 2 *", Ok(r"[[2.0,4.0]]")),
        (r"2 @V *", Ok(r"[[2.0,4.0]]")),
        (r"@V 2 /", Ok(r"[[0.5,1.0]]")),
        (
            r"@A 2 /",
            Err(r"/ can only operate on numeric values, vectors and numeric Geo Time Series."),
        ),
        (
            r"2 @V /",
            Err(r"/ can only operate on numeric values, vectors and numeric Geo Time Series."),
        ),
        (
            r"@V @V *",
            Err(
                r"* can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (r"@A '2' ->BD *", Ok(r"[[[8.0,6.0],[12.0,6.0]]]")),
        (r"@A @A ==", Ok(r"[true]")),
        (r"@A @B ==", Ok(r"[false]")),
        (r"@A [ [ 4 3 ] [ 6 3 ] ] ->MAT ==", Ok(r"[true]")),
        (r"@V [ 1 2 ] ->VEC ==", Ok(r"[true]")),
        (r"@V [ 1 3 ] ->VEC ==", Ok(r"[false]")),
        (r"[ NaN ] ->VEC [ NaN ] ->VEC ==", Ok(r"[true]")),
        (r"[ [ NaN ] ] ->MAT [ [ NaN ] ] ->MAT ==", Ok(r"[false]")),
        (r"[ [ NaN ] ] ->MAT DUP ==", Ok(r"[true]")),
        (
            r"{ @A 'a' }",
            Ok(r#"[{"Array2DRowRealMatrix{{4.0,3.0},{6.0,3.0}}":"a"}]"#),
        ),
        (r"{ @V 'v' }", Ok(r#"[{"{1; 2}":"v"}]"#)),
        (
            r"{ [ 1.25 -3 1234.5 ] ->VEC 'v' }",
            Ok(r#"[{"{1.25; -3; 1,234.5}":"v"}]"#),
        ),
        (r"@V ->MAT", Ok(r"[[[1.0],[2.0]]]")),
        (r"@W ->MAT MAT->", Ok(r"[[[0.5],[-1.0],[3.0]]]")),
        (
            r"[ ] ->VEC ->MAT",
            Err(r"0 is smaller than, or equal to, the minimum (0)"),
        ),
        (
            r"@A ->VEC",
            Err(r"->VEC expects a matrix with a single column on top of the stack."),
        ),
        (r"[ [ 1 ] [ 2 ] ] ->MAT ->VEC", Ok(r"[[1.0,2.0]]")),
        (r"[ ] ->VEC", Ok(r"[[]]")),
        (
            r"[ 1 'x' ] ->VEC",
            Err(r"->VEC expects a list of numbers onto the stack."),
        ),
        (r"1 ->VEC", Err(r"->VEC expects a list onto the stack.")),
        (r"[ ] ->MAT", Err(r"-1")),
        (
            r"[ [ ] ] ->MAT",
            Err(r"matrix must have at least one column"),
        ),
        (
            r"[ [ 1 2 ] [ 3 ] ] ->MAT",
            Err(r"->MAT expects a common number of columns throughout the 2D array."),
        ),
        (
            r"[ [ 1 'x' ] ] ->MAT",
            Err(r"->MAT expects a numeric 2D array onto the stack."),
        ),
        (
            r"[ 1 ] ->MAT",
            Err(r"->MAT expects a 2D array onto the stack."),
        ),
        (r"1 ->MAT", Err(r"->MAT expects a 2D array onto the stack.")),
        (
            r"[ [ 1 ] 2 ] ->MAT",
            Err(r"->MAT expects a 2D array onto the stack."),
        ),
        (
            r"1 MAT->",
            Err(r"MAT-> expects a matrix on top of the stack."),
        ),
        (r"1 TR", Err(r"TR expects a matrix on top of the stack.")),
        (
            r"1 TRANSPOSE",
            Err(r"TRANSPOSE expects a matrix on top of the stack."),
        ),
        (r"1 DET", Err(r"DET expects a matrix on top of the stack.")),
        (r"1 INV", Err(r"INV expects a matrix on top of the stack.")),
        (
            r"1 VEC->",
            Err(r"VEC-> expects a vector on top of the stack."),
        ),
        (r"->MAT", Err(r"Empty stack.")),
        (r"->VEC", Err(r"Empty stack.")),
        (r"MAT->", Err(r"Empty stack.")),
        (r"VEC->", Err(r"Empty stack.")),
        (r"TR", Err(r"Empty stack.")),
        (r"DET", Err(r"Empty stack.")),
        (r"INV", Err(r"Empty stack.")),
        (r"TRANSPOSE", Err(r"Empty stack.")),
        (
            r"@A TOSTRING",
            Err(r"TOSTRING can only operate on numeric, boolean or string values."),
        ),
        (r"[ [ 1 2 ] ] ->MAT [ [ 1 2 ] ] ->MAT *", Err(r"2 != 1")),
        (
            r"[ [ 1 2 ] ] ->MAT [ [ 1 2 3 ] ] ->MAT +",
            Err(r"got 1x2 but expected 1x3"),
        ),
        (r"@V [ 1 2 3 ] ->VEC +", Err(r"2 != 3")),
        (
            r"[ [ 1 2 3 ] [ 3 4 5 ] ] ->MAT TR",
            Err(r"non square (2x3) matrix"),
        ),
        (
            r"[ [ 1 2 3 ] [ 3 4 5 ] ] ->MAT DET",
            Err(r"non square (2x3) matrix"),
        ),
        (r"[ [ 1 2 ] [ 2 4 ] ] ->MAT INV", Err(r"matrix is singular")),
        (
            r"[ [ 1 2 ] [ 3 4 ] ] ->MAT [ [ 1 0 ] [ 0 1 ] ] ->MAT * MAT->",
            Ok(r"[[[1.0,2.0],[3.0,4.0]]]"),
        ),
        (
            r"@C INV @C * MAT->",
            Ok(
                r"[[[1.0,2.7755575615628914E-17,-5.551115123125783E-17],[0.0,1.0,-1.1102230246251565E-16],[0.0,0.0,0.9999999999999998]]]",
            ),
        ),
        (
            r"[ @A @V ] ->JSON",
            Ok(r#"["[[[4.0,3.0],[6.0,3.0]],[1.0,2.0]]"]"#),
        ),
        (
            r"@A 'x' +",
            Err(
                r"+ can only operate on numeric, string, lists, matrices, vectors, Geo Time Series, byte array and macro values.",
            ),
        ),
        (
            r"'x' @A +",
            Err(
                r"+ can only operate on numeric, string, lists, matrices, vectors, Geo Time Series, byte array and macro values.",
            ),
        ),
        (
            r"@A [ 1 ] +",
            Err(
                r"+ can only operate on numeric, string, lists, matrices, vectors, Geo Time Series, byte array and macro values.",
            ),
        ),
        (r"[ 1 ] @A +", Ok(r"[[1,[[4.0,3.0],[6.0,3.0]]]]")),
        (
            r"@A 'x' *",
            Err(
                r"* can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"@V 'x' *",
            Err(
                r"* can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"@V 'x' /",
            Err(r"/ can only operate on numeric values, vectors and numeric Geo Time Series."),
        ),
        (
            r"@A @V -",
            Err(
                r"- can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (r"@V [ 1 2 3 ] ->VEC ==", Ok(r"[false]")),
        (
            r"{ [ NaN Infinity -Infinity 0.0 -0.0 ] ->VEC 'v' }",
            Ok(r#"[{"{(NaN); (Infinity); (-Infinity); 0; -0}":"v"}]"#),
        ),
        (
            r"'%25d' [ @A ] STRINGFORMAT",
            Err(
                r"STRINGFORMAT expects a valid format. (d != org.apache.commons.math3.linear.Array2DRowRealMatrix)",
            ),
        ),
        (
            r"'%25d' [ @V ] STRINGFORMAT",
            Err(
                r"STRINGFORMAT expects a valid format. (d != org.apache.commons.math3.linear.ArrayRealVector)",
            ),
        ),
        (r"5 MAXJSON @A ->JSON", Err(r"Resulting JSON is too big.")),
        (r"5 MAXJSON @V ->JSON", Err(r"Resulting JSON is too big.")),
        (
            r"[ [ 1.23456789012345 ] ] ->MAT TOSTRING",
            Err(r"TOSTRING can only operate on numeric, boolean or string values."),
        ),
        (
            r"@V TOSTRING",
            Err(r"TOSTRING can only operate on numeric, boolean or string values."),
        ),
    ]);
}
