// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! `mapper.geo.approximate`, `mapper.geo.within`, `mapper.geo.outside` and `mapper.geo.fence`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const S: &str = r"NEWGTS 1 48.85 2.35 100 'a' ADDVALUE 2 -10.0 -10.0 NaN 2 ADDVALUE 3 NaN NaN 5 3.5 ADDVALUE 4 48.86 2.36 NaN T ADDVALUE 5 90.0 180.0 0 5 ADDVALUE 6 -90.0 -180.0 -7 6 ADDVALUE 7 0.0 0.0 NaN 'g' ADDVALUE 8 48.85 2.35 100 F ADDVALUE";
const ONE: &str = r"NEWGTS 1 48.85 2.35 100 'a' ADDVALUE";
const OUT: &str = r"NEWGTS 2 -10.0 -10.0 NaN 2 ADDVALUE";
const NOLOC: &str = r"NEWGTS 3 NaN NaN 5 3.5 ADDVALUE";
const EMPTY: &str = r"NEWGTS";
const A: &str = r"[ 'b9ad' ] ->GEOSHAPE";
const P: &str = r"[ 'b9' ] ->GEOSHAPE";
const K: &str = r"[ 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' 'b9af' ] ->GEOSHAPE";
const M: &str = r"[ 'b9ad' 'b8' 'f' 'a1' ] ->GEOSHAPE";
const D: &str = r"[ 'b9ad1ee0' 'b9ad1ee1' 'f01234567' ] ->GEOSHAPE";
const E: &str = r"[ ] ->GEOSHAPE";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@NOLOC", NOLOC)
        .replace("@EMPTY", EMPTY)
        .replace("@ONE", ONE)
        .replace("@OUT", OUT)
        .replace("@S", S)
        .replace("@A", A)
        .replace("@P", P)
        .replace("@K", K)
        .replace("@M", M)
        .replace("@D", D)
        .replace("@E", E)
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
fn approximate_0() {
    check(&[
        (
            r"[ @S 2 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,67.5,45.0,100,\"a\"],[2,-22.5,-45.0,\"2\"],[4,67.5,45.0,\"T\"],[5,67.5,45.0,0,\"5\"],[6,-67.5,-135.0,-7,\"6\"],[7,22.5,45.0,\"g\"],[8,67.5,45.0,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE 2 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,67.5,45.0,100,\"a\"]]}"]"#),
        ),
        (
            r"[ @NOLOC 2 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY 2 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"2 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["2 mapper.geo.approximate "]"#),
        ),
        (
            r"[ @S 4 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,50.625,11.25,100,\"a\"],[2,-5.625,-11.25,\"2\"],[4,50.625,11.25,\"T\"],[5,84.375,11.25,0,\"5\"],[6,-84.375,-168.75,-7,\"6\"],[7,5.625,11.25,\"g\"],[8,50.625,11.25,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE 4 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,50.625,11.25,100,\"a\"]]}"]"#),
        ),
        (
            r"[ @NOLOC 4 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY 4 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"4 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["4 mapper.geo.approximate "]"#),
        ),
        (
            r"[ @S 6 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,49.21875,2.8125,100,\"a\"],[2,-9.84375,-8.4375,\"2\"],[4,49.21875,2.8125,\"T\"],[5,88.59375,2.8125,0,\"5\"],[6,-88.59375,-177.1875,-7,\"6\"],[7,1.40625,2.8125,\"g\"],[8,49.21875,2.8125,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE 6 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,49.21875,2.8125,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @NOLOC 6 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY 6 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"6 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["6 mapper.geo.approximate "]"#),
        ),
        (
            r"[ @S 8 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8671875,2.109375,100,\"a\"],[2,-10.1953125,-10.546875,\"2\"],[4,48.8671875,2.109375,\"T\"],[5,89.6484375,0.703125,0,\"5\"],[6,-89.6484375,-179.296875,-7,\"6\"],[7,0.3515625,0.703125,\"g\"],[8,48.8671875,2.109375,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE 8 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8671875,2.109375,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @NOLOC 8 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY 8 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"8 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["8 mapper.geo.approximate "]"#),
        ),
        (
            r"[ @S 10 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.779296875,2.28515625,100,\"a\"],[2,-9.931640625,-10.01953125,\"2\"],[4,48.779296875,2.28515625,\"T\"],[5,89.912109375,0.17578125,0,\"5\"],[6,-89.912109375,-179.82421875,-7,\"6\"],[7,0.087890625,0.17578125,\"g\"],[8,48.779296875,2.28515625,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE 10 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.779296875,2.28515625,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @NOLOC 10 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY 10 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"10 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["10 mapper.geo.approximate "]"#),
        ),
        (
            r"[ @S 14 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8507080078125,2.340087890625,100,\"a\"],[2,-10.0030517578125,-10.008544921875,\"2\"],[4,48.8616943359375,2.362060546875,\"T\"],[5,89.9945068359375,0.010986328125,0,\"5\"],[6,-89.9945068359375,-179.989013671875,-7,\"6\"],[7,0.0054931640625,0.010986328125,\"g\"],[8,48.8507080078125,2.340087890625,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE 14 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8507080078125,2.340087890625,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @NOLOC 14 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY 14 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"14 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["14 mapper.geo.approximate "]"#),
        ),
        (
            r"[ @S 20 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.84993553161621,2.349872589111328,100,\"a\"],[2,-10.00004768371582,-10.000133514404297,\"2\"],[4,48.860063552856445,2.3598289489746094,\"T\"],[5,89.99991416931152,1.71661376953125E-4,0,\"5\"],[6,-89.99991416931152,-179.99982833862305,-7,\"6\"],[7,8.58306884765625E-5,1.71661376953125E-4,\"g\"],[8,48.84993553161621,2.349872589111328,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE 20 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.84993553161621,2.349872589111328,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @NOLOC 20 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY 20 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"20 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["20 mapper.geo.approximate "]"#),
        ),
        (
            r"[ @S 26 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.850001245737076,2.349998652935028,100,\"a\"],[2,-10.00000074505806,-10.000002086162567,\"2\"],[4,48.860000520944595,2.3599979281425476,\"T\"],[5,89.99999865889549,2.682209014892578E-6,0,\"5\"],[6,-89.99999865889549,-179.99999731779099,-7,\"6\"],[7,1.341104507446289E-6,2.682209014892578E-6,\"g\"],[8,48.850001245737076,2.349998652935028,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE 26 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.850001245737076,2.349998652935028,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @NOLOC 26 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY 26 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"26 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["26 mapper.geo.approximate "]"#),
        ),
        (
            r"[ @S 30 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.350000161677599,100,\"a\"],[2,-9.999999990686774,-9.999999906867743,\"2\"],[4,48.85999993421137,2.360000107437372,\"T\"],[5,89.99999991618097,1.6763806343078613E-7,0,\"5\"],[6,-89.99999991618097,-179.99999983236194,-7,\"6\"],[7,8.381903171539307E-8,1.6763806343078613E-7,\"g\"],[8,48.8499999884516,2.350000161677599,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE 30 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.350000161677599,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @NOLOC 30 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY 30 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"30 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["30 mapper.geo.approximate "]"#),
        ),
        (
            r"[ @S 32 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,-10.00000003259629,-10.000000074505806,\"2\"],[4,48.85999997612089,2.3599999397993088,\"T\"],[5,89.99999995809048,0.0,0,\"5\"],[6,-90.0,-180.0,-7,\"6\"],[7,0.0,0.0,\"g\"],[8,48.8499999884516,2.3499999940395355,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE 32 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @NOLOC 32 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY 32 mapper.geo.approximate 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"32 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["32 mapper.geo.approximate "]"#),
        ),
        (
            r"[ @S 8 mapper.geo.approximate 1 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8671875,2.109375,100,\"a\"],[2,48.8671875,2.109375,100,\"a\"],[3,-10.1953125,-10.546875,\"2\"],[5,48.8671875,2.109375,\"T\"],[6,89.6484375,0.703125,0,\"5\"],[7,-89.6484375,-179.296875,-7,\"6\"],[8,0.3515625,0.703125,\"g\"]]}"]"#,
            ),
        ),
        (
            r"[ @S 8 mapper.geo.approximate 0 1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8671875,2.109375,100,\"a\"],[2,-10.1953125,-10.546875,\"2\"],[4,48.8671875,2.109375,\"T\"],[5,89.6484375,0.703125,0,\"5\"],[6,-89.6484375,-179.296875,-7,\"6\"],[7,0.3515625,0.703125,\"g\"],[8,48.8671875,2.109375,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @S 8 mapper.geo.approximate 1 1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8671875,2.109375,100,\"a\"],[2,48.8671875,2.109375,100,\"a\"],[3,-10.1953125,-10.546875,\"2\"],[5,48.8671875,2.109375,\"T\"],[6,89.6484375,0.703125,0,\"5\"],[7,-89.6484375,-179.296875,-7,\"6\"],[8,0.3515625,0.703125,\"g\"]]}"]"#,
            ),
        ),
        (
            r"[ @S 8 mapper.geo.approximate 2 2 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8671875,2.109375,100,\"a\"],[2,48.8671875,2.109375,100,\"a\"],[3,48.8671875,2.109375,100,\"a\"],[4,-10.1953125,-10.546875,\"2\"],[6,48.8671875,2.109375,\"T\"],[7,89.6484375,0.703125,0,\"5\"],[8,-89.6484375,-179.296875,-7,\"6\"]]}"]"#,
            ),
        ),
        (
            r"[ @S 8 mapper.geo.approximate 3 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8671875,2.109375,100,\"a\"],[2,48.8671875,2.109375,100,\"a\"],[3,48.8671875,2.109375,100,\"a\"],[4,48.8671875,2.109375,100,\"a\"],[5,-10.1953125,-10.546875,\"2\"],[7,48.8671875,2.109375,\"T\"],[8,89.6484375,0.703125,0,\"5\"]]}"]"#,
            ),
        ),
        (
            r"[ @S 8 mapper.geo.approximate -1 -1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8671875,2.109375,100,\"a\"],[2,48.8671875,2.109375,100,\"a\"],[3,-10.1953125,-10.546875,\"2\"],[5,48.8671875,2.109375,\"T\"],[6,89.6484375,0.703125,0,\"5\"],[7,-89.6484375,-179.296875,-7,\"6\"],[8,0.3515625,0.703125,\"g\"]]}"]"#,
            ),
        ),
        (
            r"4.9 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["4 mapper.geo.approximate "]"#),
        ),
        (
            r"4 ->BD mapper.geo.approximate SNAPSHOT",
            Ok(r#"["4 mapper.geo.approximate "]"#),
        ),
        (
            r"4294967300 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["4 mapper.geo.approximate "]"#),
        ),
        (
            r"4.9 ->BD mapper.geo.approximate SNAPSHOT",
            Ok(r#"["4 mapper.geo.approximate "]"#),
        ),
        (
            r"6.1 ->BD mapper.geo.approximate SNAPSHOT",
            Ok(r#"["6 mapper.geo.approximate "]"#),
        ),
        (
            r"4294967298 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["2 mapper.geo.approximate "]"#),
        ),
        (
            r"4294967297 mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"3.0E9 mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"2.5 mapper.geo.approximate SNAPSHOT",
            Ok(r#"["2 mapper.geo.approximate "]"#),
        ),
        (
            r"NaN mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"Infinity mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"-1 mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"0 mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"1 mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"3 mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"31 mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"33 mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"34 mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"64 mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"'a' mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"NULL mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"T mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (
            r"[ 4 ] mapper.geo.approximate",
            Err(
                r"mapper.geo.approximate expects an even resolution between 2 (coarsest) and 32 (finest).",
            ),
        ),
        (r"mapper.geo.approximate", Err(r"Empty stack.")),
        (r"4 mapper.geo.approximate TYPEOF", Ok(r#"["AGGREGATOR"]"#)),
        (
            r"4 mapper.geo.approximate 4 mapper.geo.approximate ==",
            Ok(r"[false]"),
        ),
        (
            r"[ @S 4 mapper.geo.approximate 0 0 0 ] BUCKETIZE",
            Err(
                r"BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
            ),
        ),
        (
            r"[ @S 4 mapper.geo.approximate 0 0 0 ] MAP SIZE",
            Ok(r"[1]"),
        ),
    ]);
}

#[test]
fn within_outside_0() {
    check(&[
        (
            r"[ @S @A mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @P mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @K mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @M mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @D mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @E mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @ONE @A mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @ONE @P mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @ONE @K mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @ONE @M mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @ONE @D mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @ONE @E mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @OUT @A mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @OUT @P mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @OUT @K mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @OUT @M mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @OUT @D mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @OUT @E mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @A mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @P mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @K mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @M mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @D mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @E mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @A mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @P mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @K mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @M mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @D mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @E mapper.geo.within 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @A mapper.geo.within 1 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @P mapper.geo.within 1 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @A mapper.geo.within 0 1 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @P mapper.geo.within 0 1 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @A mapper.geo.within 1 1 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @P mapper.geo.within 1 1 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @A mapper.geo.within 2 2 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @P mapper.geo.within 2 2 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @A mapper.geo.within 3 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @P mapper.geo.within 3 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @A mapper.geo.within 0 0 2 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @P mapper.geo.within 0 0 2 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @A mapper.geo.within 1 1 3 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @P mapper.geo.within 1 1 3 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @A mapper.geo.within -1 -1 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @P mapper.geo.within -1 -1 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (r"@A mapper.geo.within TYPEOF", Ok(r#"["AGGREGATOR"]"#)),
        (
            r"@A mapper.geo.within SNAPSHOT",
            Ok(r#"["'T._ZHtfF.......L.V.' GEOUNPACK mapper.geo.within "]"#),
        ),
        (
            r"@P mapper.geo.within SNAPSHOT",
            Ok(r#"["'T._Z9t.........L.V.' GEOUNPACK mapper.geo.within "]"#),
        ),
        (
            r"@E mapper.geo.within SNAPSHOT",
            Ok(r#"["'T..L...' GEOUNPACK mapper.geo.within "]"#),
        ),
        (
            r"@D mapper.geo.within SNAPSHOT",
            Ok(r#"["'T0WZXtfGvV.....3V71.V.8Zbk3YGLR....L0V.' GEOUNPACK mapper.geo.within "]"#),
        ),
        (
            r"@M mapper.geo.within",
            Ok(
                r#"["'T1GZ6k........0Z9W........0Z9s........0ZHtfF.......L1..' GEOUNPACK mapper.geo.within"]"#,
            ),
        ),
        (
            r"@A mapper.geo.within @A mapper.geo.within == ",
            Ok(r"[false]"),
        ),
        (r"mapper.geo.within", Err(r"Empty stack.")),
        (
            r"1 mapper.geo.within",
            Err(r"mapper.geo.within expects a Geo Shape on top of the stack."),
        ),
        (
            r"'a' mapper.geo.within",
            Err(r"mapper.geo.within expects a Geo Shape on top of the stack."),
        ),
        (
            r"NULL mapper.geo.within",
            Err(r"mapper.geo.within expects a Geo Shape on top of the stack."),
        ),
        (
            r"[ @A ] mapper.geo.within",
            Err(r"mapper.geo.within expects a Geo Shape on top of the stack."),
        ),
        (
            r"@A @A mapper.geo.within DEPTH",
            Ok(r#"[2,"'T._ZHtfF.......L.V.' GEOUNPACK mapper.geo.within",null]"#),
        ),
        (
            r"T mapper.geo.within",
            Err(r"mapper.geo.within expects a Geo Shape on top of the stack."),
        ),
        (
            r"{ } mapper.geo.within",
            Err(r"mapper.geo.within expects a Geo Shape on top of the stack."),
        ),
        (
            r"[ @S @A mapper.geo.within 0 0 0 ] BUCKETIZE",
            Err(
                r"BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
            ),
        ),
        (
            r"[ @S @A mapper.geo.within 0 0 0 ] REDUCE",
            Err(r"REDUCE expects lists of Geo Time Series as first parameter."),
        ),
        (
            r"[ @S [ ] @A mapper.geo.within ] REDUCE",
            Err(r"REDUCE expects lists of Geo Time Series as first parameter."),
        ),
        (r"[ @S @A mapper.geo.within 0 0 0 ] MAP SIZE", Ok(r"[1]")),
        (
            r"[ @S @A mapper.geo.within 0 0 0 ] MAP TYPEOF",
            Ok(r#"["LIST"]"#),
        ),
        (
            r"[ @S @A mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,-10.00000003259629,-10.000000074505806,\"2\"],[4,48.85999997612089,2.3599999397993088,\"T\"],[5,89.99999995809048,0.0,0,\"5\"],[6,-90.0,-180.0,-7,\"6\"],[7,0.0,0.0,\"g\"],[8,48.8499999884516,2.3499999940395355,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,-10.00000003259629,-10.000000074505806,\"2\"],[4,48.85999997612089,2.3599999397993088,\"T\"],[5,89.99999995809048,0.0,0,\"5\"],[6,-90.0,-180.0,-7,\"6\"],[7,0.0,0.0,\"g\"],[8,48.8499999884516,2.3499999940395355,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @K mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,-10.00000003259629,-10.000000074505806,\"2\"],[4,48.85999997612089,2.3599999397993088,\"T\"],[5,89.99999995809048,0.0,0,\"5\"],[6,-90.0,-180.0,-7,\"6\"],[7,0.0,0.0,\"g\"],[8,48.8499999884516,2.3499999940395355,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @M mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,-10.00000003259629,-10.000000074505806,\"2\"],[4,48.85999997612089,2.3599999397993088,\"T\"],[5,89.99999995809048,0.0,0,\"5\"],[6,-90.0,-180.0,-7,\"6\"],[7,0.0,0.0,\"g\"],[8,48.8499999884516,2.3499999940395355,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @D mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,-10.00000003259629,-10.000000074505806,\"2\"],[4,48.85999997612089,2.3599999397993088,\"T\"],[5,89.99999995809048,0.0,0,\"5\"],[6,-90.0,-180.0,-7,\"6\"],[7,0.0,0.0,\"g\"],[8,48.8499999884516,2.3499999940395355,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @E mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,-10.00000003259629,-10.000000074505806,\"2\"],[4,48.85999997612089,2.3599999397993088,\"T\"],[5,89.99999995809048,0.0,0,\"5\"],[6,-90.0,-180.0,-7,\"6\"],[7,0.0,0.0,\"g\"],[8,48.8499999884516,2.3499999940395355,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE @A mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE @P mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE @K mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE @M mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE @D mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @ONE @E mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"]]}"]"#,
            ),
        ),
        (
            r"[ @OUT @A mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[2,-10.00000003259629,-10.000000074505806,2]]}"]"#,
            ),
        ),
        (
            r"[ @OUT @P mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[2,-10.00000003259629,-10.000000074505806,2]]}"]"#,
            ),
        ),
        (
            r"[ @OUT @K mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[2,-10.00000003259629,-10.000000074505806,2]]}"]"#,
            ),
        ),
        (
            r"[ @OUT @M mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[2,-10.00000003259629,-10.000000074505806,2]]}"]"#,
            ),
        ),
        (
            r"[ @OUT @D mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[2,-10.00000003259629,-10.000000074505806,2]]}"]"#,
            ),
        ),
        (
            r"[ @OUT @E mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[2,-10.00000003259629,-10.000000074505806,2]]}"]"#,
            ),
        ),
        (
            r"[ @NOLOC @A mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @P mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @K mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @M mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @D mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @E mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
    ]);
}

#[test]
fn within_outside_1() {
    check(&[
        (
            r"[ @EMPTY @A mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @P mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @K mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @M mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @D mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @E mapper.geo.outside 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @A mapper.geo.outside 1 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,48.8499999884516,2.3499999940395355,100,\"a\"],[3,-10.00000003259629,-10.000000074505806,\"2\"],[5,48.85999997612089,2.3599999397993088,\"T\"],[6,89.99999995809048,0.0,0,\"5\"],[7,-90.0,-180.0,-7,\"6\"],[8,0.0,0.0,\"g\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.outside 1 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,48.8499999884516,2.3499999940395355,100,\"a\"],[3,-10.00000003259629,-10.000000074505806,\"2\"],[5,48.85999997612089,2.3599999397993088,\"T\"],[6,89.99999995809048,0.0,0,\"5\"],[7,-90.0,-180.0,-7,\"6\"],[8,0.0,0.0,\"g\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.outside 0 1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,-10.00000003259629,-10.000000074505806,\"2\"],[4,48.85999997612089,2.3599999397993088,\"T\"],[5,89.99999995809048,0.0,0,\"5\"],[6,-90.0,-180.0,-7,\"6\"],[7,0.0,0.0,\"g\"],[8,48.8499999884516,2.3499999940395355,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.outside 0 1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,-10.00000003259629,-10.000000074505806,\"2\"],[4,48.85999997612089,2.3599999397993088,\"T\"],[5,89.99999995809048,0.0,0,\"5\"],[6,-90.0,-180.0,-7,\"6\"],[7,0.0,0.0,\"g\"],[8,48.8499999884516,2.3499999940395355,100,\"F\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.outside 1 1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,48.8499999884516,2.3499999940395355,100,\"a\"],[3,-10.00000003259629,-10.000000074505806,\"2\"],[5,48.85999997612089,2.3599999397993088,\"T\"],[6,89.99999995809048,0.0,0,\"5\"],[7,-90.0,-180.0,-7,\"6\"],[8,0.0,0.0,\"g\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.outside 1 1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,48.8499999884516,2.3499999940395355,100,\"a\"],[3,-10.00000003259629,-10.000000074505806,\"2\"],[5,48.85999997612089,2.3599999397993088,\"T\"],[6,89.99999995809048,0.0,0,\"5\"],[7,-90.0,-180.0,-7,\"6\"],[8,0.0,0.0,\"g\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.outside 2 2 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,48.8499999884516,2.3499999940395355,100,\"a\"],[3,48.8499999884516,2.3499999940395355,100,\"a\"],[4,-10.00000003259629,-10.000000074505806,\"2\"],[6,48.85999997612089,2.3599999397993088,\"T\"],[7,89.99999995809048,0.0,0,\"5\"],[8,-90.0,-180.0,-7,\"6\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.outside 2 2 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,48.8499999884516,2.3499999940395355,100,\"a\"],[3,48.8499999884516,2.3499999940395355,100,\"a\"],[4,-10.00000003259629,-10.000000074505806,\"2\"],[6,48.85999997612089,2.3599999397993088,\"T\"],[7,89.99999995809048,0.0,0,\"5\"],[8,-90.0,-180.0,-7,\"6\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.outside 3 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,48.8499999884516,2.3499999940395355,100,\"a\"],[3,48.8499999884516,2.3499999940395355,100,\"a\"],[4,48.8499999884516,2.3499999940395355,100,\"a\"],[5,-10.00000003259629,-10.000000074505806,\"2\"],[7,48.85999997612089,2.3599999397993088,\"T\"],[8,89.99999995809048,0.0,0,\"5\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.outside 3 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,48.8499999884516,2.3499999940395355,100,\"a\"],[3,48.8499999884516,2.3499999940395355,100,\"a\"],[4,48.8499999884516,2.3499999940395355,100,\"a\"],[5,-10.00000003259629,-10.000000074505806,\"2\"],[7,48.85999997612089,2.3599999397993088,\"T\"],[8,89.99999995809048,0.0,0,\"5\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.outside 0 0 2 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,-10.00000003259629,-10.000000074505806,\"2\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.outside 0 0 2 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,-10.00000003259629,-10.000000074505806,\"2\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.outside 1 1 3 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,48.8499999884516,2.3499999940395355,100,\"a\"],[3,-10.00000003259629,-10.000000074505806,\"2\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.outside 1 1 3 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,48.8499999884516,2.3499999940395355,100,\"a\"],[3,-10.00000003259629,-10.000000074505806,\"2\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.outside -1 -1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,48.8499999884516,2.3499999940395355,100,\"a\"],[3,-10.00000003259629,-10.000000074505806,\"2\"],[5,48.85999997612089,2.3599999397993088,\"T\"],[6,89.99999995809048,0.0,0,\"5\"],[7,-90.0,-180.0,-7,\"6\"],[8,0.0,0.0,\"g\"]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.outside -1 -1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,\"a\"],[2,48.8499999884516,2.3499999940395355,100,\"a\"],[3,-10.00000003259629,-10.000000074505806,\"2\"],[5,48.85999997612089,2.3599999397993088,\"T\"],[6,89.99999995809048,0.0,0,\"5\"],[7,-90.0,-180.0,-7,\"6\"],[8,0.0,0.0,\"g\"]]}"]"#,
            ),
        ),
        (r"@A mapper.geo.outside TYPEOF", Ok(r#"["AGGREGATOR"]"#)),
        (
            r"@A mapper.geo.outside SNAPSHOT",
            Ok(r#"["'T._ZHtfF.......L.V.' GEOUNPACK mapper.geo.outside "]"#),
        ),
        (
            r"@P mapper.geo.outside SNAPSHOT",
            Ok(r#"["'T._Z9t.........L.V.' GEOUNPACK mapper.geo.outside "]"#),
        ),
        (
            r"@E mapper.geo.outside SNAPSHOT",
            Ok(r#"["'T..L...' GEOUNPACK mapper.geo.outside "]"#),
        ),
        (
            r"@D mapper.geo.outside SNAPSHOT",
            Ok(r#"["'T0WZXtfGvV.....3V71.V.8Zbk3YGLR....L0V.' GEOUNPACK mapper.geo.outside "]"#),
        ),
        (
            r"@M mapper.geo.outside",
            Ok(
                r#"["'T1GZ6k........0Z9W........0Z9s........0ZHtfF.......L1..' GEOUNPACK mapper.geo.outside"]"#,
            ),
        ),
        (
            r"@A mapper.geo.outside @A mapper.geo.outside == ",
            Ok(r"[false]"),
        ),
        (r"mapper.geo.outside", Err(r"Empty stack.")),
        (
            r"1 mapper.geo.outside",
            Err(r"mapper.geo.outside expects a Geo Shape on top of the stack."),
        ),
        (
            r"'a' mapper.geo.outside",
            Err(r"mapper.geo.outside expects a Geo Shape on top of the stack."),
        ),
        (
            r"NULL mapper.geo.outside",
            Err(r"mapper.geo.outside expects a Geo Shape on top of the stack."),
        ),
        (
            r"[ @A ] mapper.geo.outside",
            Err(r"mapper.geo.outside expects a Geo Shape on top of the stack."),
        ),
        (
            r"@A @A mapper.geo.outside DEPTH",
            Ok(r#"[2,"'T._ZHtfF.......L.V.' GEOUNPACK mapper.geo.outside",null]"#),
        ),
        (
            r"T mapper.geo.outside",
            Err(r"mapper.geo.outside expects a Geo Shape on top of the stack."),
        ),
        (
            r"{ } mapper.geo.outside",
            Err(r"mapper.geo.outside expects a Geo Shape on top of the stack."),
        ),
        (
            r"[ @S @A mapper.geo.outside 0 0 0 ] BUCKETIZE",
            Err(
                r"BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
            ),
        ),
        (
            r"[ @S @A mapper.geo.outside 0 0 0 ] REDUCE",
            Err(r"REDUCE expects lists of Geo Time Series as first parameter."),
        ),
        (
            r"[ @S [ ] @A mapper.geo.outside ] REDUCE",
            Err(r"REDUCE expects lists of Geo Time Series as first parameter."),
        ),
        (r"[ @S @A mapper.geo.outside 0 0 0 ] MAP SIZE", Ok(r"[1]")),
        (
            r"[ @S @A mapper.geo.outside 0 0 0 ] MAP TYPEOF",
            Ok(r#"["LIST"]"#),
        ),
    ]);
}

#[test]
fn fence_0() {
    check(&[
        (
            r"[ @S @A mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,-10.00000003259629,-10.000000074505806,false],[4,48.85999997612089,2.3599999397993088,false],[5,89.99999995809048,0.0,0,false],[6,-90.0,-180.0,-7,false],[7,0.0,0.0,false],[8,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,-10.00000003259629,-10.000000074505806,false],[4,48.85999997612089,2.3599999397993088,false],[5,89.99999995809048,0.0,0,false],[6,-90.0,-180.0,-7,false],[7,0.0,0.0,false],[8,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @K mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,-10.00000003259629,-10.000000074505806,false],[4,48.85999997612089,2.3599999397993088,false],[5,89.99999995809048,0.0,0,false],[6,-90.0,-180.0,-7,false],[7,0.0,0.0,false],[8,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @M mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,-10.00000003259629,-10.000000074505806,false],[4,48.85999997612089,2.3599999397993088,false],[5,89.99999995809048,0.0,0,false],[6,-90.0,-180.0,-7,false],[7,0.0,0.0,false],[8,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @D mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,-10.00000003259629,-10.000000074505806,false],[4,48.85999997612089,2.3599999397993088,false],[5,89.99999995809048,0.0,0,false],[6,-90.0,-180.0,-7,false],[7,0.0,0.0,false],[8,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @E mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,-10.00000003259629,-10.000000074505806,false],[4,48.85999997612089,2.3599999397993088,false],[5,89.99999995809048,0.0,0,false],[6,-90.0,-180.0,-7,false],[7,0.0,0.0,false],[8,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @ONE @A mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @ONE @P mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @ONE @K mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @ONE @M mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @ONE @D mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @ONE @E mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @OUT @A mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[2,-10.00000003259629,-10.000000074505806,false]]}"]"#,
            ),
        ),
        (
            r"[ @OUT @P mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[2,-10.00000003259629,-10.000000074505806,false]]}"]"#,
            ),
        ),
        (
            r"[ @OUT @K mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[2,-10.00000003259629,-10.000000074505806,false]]}"]"#,
            ),
        ),
        (
            r"[ @OUT @M mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[2,-10.00000003259629,-10.000000074505806,false]]}"]"#,
            ),
        ),
        (
            r"[ @OUT @D mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[2,-10.00000003259629,-10.000000074505806,false]]}"]"#,
            ),
        ),
        (
            r"[ @OUT @E mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[2,-10.00000003259629,-10.000000074505806,false]]}"]"#,
            ),
        ),
        (
            r"[ @NOLOC @A mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @P mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @K mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @M mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @D mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @NOLOC @E mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @A mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @P mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @K mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @M mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @D mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @EMPTY @E mapper.geo.fence 0 0 0 ] MAP 0 GET ->JSON",
            Ok(r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}"]"#),
        ),
        (
            r"[ @S @A mapper.geo.fence 1 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,48.8499999884516,2.3499999940395355,100,false],[3,-10.00000003259629,-10.000000074505806,false],[5,48.85999997612089,2.3599999397993088,false],[6,89.99999995809048,0.0,0,false],[7,-90.0,-180.0,-7,false],[8,0.0,0.0,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.fence 1 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,48.8499999884516,2.3499999940395355,100,false],[3,-10.00000003259629,-10.000000074505806,false],[5,48.85999997612089,2.3599999397993088,false],[6,89.99999995809048,0.0,0,false],[7,-90.0,-180.0,-7,false],[8,0.0,0.0,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.fence 0 1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,-10.00000003259629,-10.000000074505806,false],[4,48.85999997612089,2.3599999397993088,false],[5,89.99999995809048,0.0,0,false],[6,-90.0,-180.0,-7,false],[7,0.0,0.0,false],[8,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.fence 0 1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,-10.00000003259629,-10.000000074505806,false],[4,48.85999997612089,2.3599999397993088,false],[5,89.99999995809048,0.0,0,false],[6,-90.0,-180.0,-7,false],[7,0.0,0.0,false],[8,48.8499999884516,2.3499999940395355,100,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.fence 1 1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,48.8499999884516,2.3499999940395355,100,false],[3,-10.00000003259629,-10.000000074505806,false],[5,48.85999997612089,2.3599999397993088,false],[6,89.99999995809048,0.0,0,false],[7,-90.0,-180.0,-7,false],[8,0.0,0.0,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.fence 1 1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,48.8499999884516,2.3499999940395355,100,false],[3,-10.00000003259629,-10.000000074505806,false],[5,48.85999997612089,2.3599999397993088,false],[6,89.99999995809048,0.0,0,false],[7,-90.0,-180.0,-7,false],[8,0.0,0.0,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.fence 2 2 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,48.8499999884516,2.3499999940395355,100,false],[3,48.8499999884516,2.3499999940395355,100,false],[4,-10.00000003259629,-10.000000074505806,false],[6,48.85999997612089,2.3599999397993088,false],[7,89.99999995809048,0.0,0,false],[8,-90.0,-180.0,-7,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.fence 2 2 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,48.8499999884516,2.3499999940395355,100,false],[3,48.8499999884516,2.3499999940395355,100,false],[4,-10.00000003259629,-10.000000074505806,false],[6,48.85999997612089,2.3599999397993088,false],[7,89.99999995809048,0.0,0,false],[8,-90.0,-180.0,-7,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.fence 3 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,48.8499999884516,2.3499999940395355,100,false],[3,48.8499999884516,2.3499999940395355,100,false],[4,48.8499999884516,2.3499999940395355,100,false],[5,-10.00000003259629,-10.000000074505806,false],[7,48.85999997612089,2.3599999397993088,false],[8,89.99999995809048,0.0,0,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.fence 3 0 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,48.8499999884516,2.3499999940395355,100,false],[3,48.8499999884516,2.3499999940395355,100,false],[4,48.8499999884516,2.3499999940395355,100,false],[5,-10.00000003259629,-10.000000074505806,false],[7,48.85999997612089,2.3599999397993088,false],[8,89.99999995809048,0.0,0,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.fence 0 0 2 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,-10.00000003259629,-10.000000074505806,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.fence 0 0 2 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,-10.00000003259629,-10.000000074505806,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.fence 1 1 3 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,48.8499999884516,2.3499999940395355,100,false],[3,-10.00000003259629,-10.000000074505806,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.fence 1 1 3 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,48.8499999884516,2.3499999940395355,100,false],[3,-10.00000003259629,-10.000000074505806,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @A mapper.geo.fence -1 -1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,48.8499999884516,2.3499999940395355,100,false],[3,-10.00000003259629,-10.000000074505806,false],[5,48.85999997612089,2.3599999397993088,false],[6,89.99999995809048,0.0,0,false],[7,-90.0,-180.0,-7,false],[8,0.0,0.0,false]]}"]"#,
            ),
        ),
        (
            r"[ @S @P mapper.geo.fence -1 -1 0 ] MAP 0 GET ->JSON",
            Ok(
                r#"["{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,48.8499999884516,2.3499999940395355,100,false],[2,48.8499999884516,2.3499999940395355,100,false],[3,-10.00000003259629,-10.000000074505806,false],[5,48.85999997612089,2.3599999397993088,false],[6,89.99999995809048,0.0,0,false],[7,-90.0,-180.0,-7,false],[8,0.0,0.0,false]]}"]"#,
            ),
        ),
        (r"@A mapper.geo.fence TYPEOF", Ok(r#"["AGGREGATOR"]"#)),
        (
            r"@A mapper.geo.fence SNAPSHOT",
            Ok(r#"["'T._ZHtfF.......L.V.' GEOUNPACK mapper.geo.fence "]"#),
        ),
        (
            r"@P mapper.geo.fence SNAPSHOT",
            Ok(r#"["'T._Z9t.........L.V.' GEOUNPACK mapper.geo.fence "]"#),
        ),
        (
            r"@E mapper.geo.fence SNAPSHOT",
            Ok(r#"["'T..L...' GEOUNPACK mapper.geo.fence "]"#),
        ),
        (
            r"@D mapper.geo.fence SNAPSHOT",
            Ok(r#"["'T0WZXtfGvV.....3V71.V.8Zbk3YGLR....L0V.' GEOUNPACK mapper.geo.fence "]"#),
        ),
        (
            r"@M mapper.geo.fence",
            Ok(
                r#"["'T1GZ6k........0Z9W........0Z9s........0ZHtfF.......L1..' GEOUNPACK mapper.geo.fence"]"#,
            ),
        ),
        (
            r"@A mapper.geo.fence @A mapper.geo.fence == ",
            Ok(r"[false]"),
        ),
        (r"mapper.geo.fence", Err(r"Empty stack.")),
        (
            r"1 mapper.geo.fence",
            Err(r"mapper.geo.fence expects a Geo Shape."),
        ),
        (
            r"'a' mapper.geo.fence",
            Err(r"mapper.geo.fence expects a Geo Shape."),
        ),
        (
            r"NULL mapper.geo.fence",
            Err(r"mapper.geo.fence expects a Geo Shape."),
        ),
        (
            r"[ @A ] mapper.geo.fence",
            Err(r"mapper.geo.fence expects a Geo Shape."),
        ),
        (
            r"@A @A mapper.geo.fence DEPTH",
            Ok(r#"[2,"'T._ZHtfF.......L.V.' GEOUNPACK mapper.geo.fence",null]"#),
        ),
        (
            r"T mapper.geo.fence",
            Err(r"mapper.geo.fence expects a Geo Shape."),
        ),
        (
            r"{ } mapper.geo.fence",
            Err(r"mapper.geo.fence expects a Geo Shape."),
        ),
        (
            r"[ @S @A mapper.geo.fence 0 0 0 ] BUCKETIZE",
            Err(
                r"BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
            ),
        ),
        (
            r"[ @S @A mapper.geo.fence 0 0 0 ] REDUCE",
            Err(r"REDUCE expects lists of Geo Time Series as first parameter."),
        ),
        (
            r"[ @S [ ] @A mapper.geo.fence ] REDUCE",
            Err(r"REDUCE expects lists of Geo Time Series as first parameter."),
        ),
        (r"[ @S @A mapper.geo.fence 0 0 0 ] MAP SIZE", Ok(r"[1]")),
        (
            r"[ @S @A mapper.geo.fence 0 0 0 ] MAP TYPEOF",
            Ok(r#"["LIST"]"#),
        ),
    ]);
}
