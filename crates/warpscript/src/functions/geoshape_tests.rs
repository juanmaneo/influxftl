// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the `GEOSHAPE` functions: `->GEOSHAPE`, `GEOSHAPE->`, the GeoHash conversions and the packing.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const BH: &str = r"'b9ad'";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template.replace("@BH", BH)
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
fn toshape() {
    check(&[
        (
            r"[ 'b9ad' ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[5447895397676613632]]"),
        ),
        (r"[ 'b9ad' ] ->GEOSHAPE T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"[ 'b9ad' ] ->GEOSHAPE SIZE", Ok(r"[1]")),
        (
            r"[ 'b9ad' 'b9' ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[3139008940277235712,5447895397676613632]]"),
        ),
        (
            r"[ 'b9ad' 'b9' ] ->GEOSHAPE T GEOSHAPE->",
            Ok(r#"[["b9","b9ad"]]"#),
        ),
        (r"[ 'b9ad' 'b9' ] ->GEOSHAPE SIZE", Ok(r"[2]")),
        (
            r"[ 'b9ad' 'b9' 'b8' 'f' ] ->GEOSHAPE GEOSHAPE->",
            Ok(
                r"[[2233785415175766016,3134505340649865216,3139008940277235712,5447895397676613632]]",
            ),
        ),
        (
            r"[ 'b9ad' 'b9' 'b8' 'f' ] ->GEOSHAPE T GEOSHAPE->",
            Ok(r#"[["f","b8","b9","b9ad"]]"#),
        ),
        (r"[ 'b9ad' 'b9' 'b8' 'f' ] ->GEOSHAPE SIZE", Ok(r"[4]")),
        (
            r"[ 'b9ad' 'b9ad' ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[5447895397676613632]]"),
        ),
        (
            r"[ 'b9ad' 'b9ad' ] ->GEOSHAPE T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (r"[ 'b9ad' 'b9ad' ] ->GEOSHAPE SIZE", Ok(r"[1]")),
        (
            r"[ 'b9ad' HEX-> ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[5447895397676613632]]"),
        ),
        (
            r"[ 'b9ad' HEX-> ] ->GEOSHAPE T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (r"[ 'b9ad' HEX-> ] ->GEOSHAPE SIZE", Ok(r"[1]")),
        (
            r"[ 'b9' HEX-> 'b9ad' ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[3139008940277235712,5447895397676613632]]"),
        ),
        (
            r"[ 'b9' HEX-> 'b9ad' ] ->GEOSHAPE T GEOSHAPE->",
            Ok(r#"[["b9","b9ad"]]"#),
        ),
        (r"[ 'b9' HEX-> 'b9ad' ] ->GEOSHAPE SIZE", Ok(r"[2]")),
        (
            r"[ 5447895397676613632 ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[5447895397676613632]]"),
        ),
        (
            r"[ 5447895397676613632 ] ->GEOSHAPE T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (r"[ 5447895397676613632 ] ->GEOSHAPE SIZE", Ok(r"[1]")),
        (
            r"[ 5447895397676613632 -1 ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[-1,5447895397676613632]]"),
        ),
        (
            r"[ 5447895397676613632 -1 ] ->GEOSHAPE T GEOSHAPE->",
            Ok(r#"[["fffffffffffffff","b9ad"]]"#),
        ),
        (r"[ 5447895397676613632 -1 ] ->GEOSHAPE SIZE", Ok(r"[2]")),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[-1469631508043358608]]"),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0c5a9e7"]]"#),
        ),
        (r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE SIZE", Ok(r"[1]")),
        (
            r"[ 'b9ad1ee0c5a9e7a5' ] ->GEOSHAPE GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7a5' ] ->GEOSHAPE T GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7a5' ] ->GEOSHAPE SIZE",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7a5f' ] ->GEOSHAPE GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects the given list to contain valid STRING or BYTES HHCodes. (Hexadecimal HHCode string of must be of length <= 16)",
            ),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7a5f' ] ->GEOSHAPE T GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects the given list to contain valid STRING or BYTES HHCodes. (Hexadecimal HHCode string of must be of length <= 16)",
            ),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7a5f' ] ->GEOSHAPE SIZE",
            Err(
                r"->GEOSHAPE expects the given list to contain valid STRING or BYTES HHCodes. (Hexadecimal HHCode string of must be of length <= 16)",
            ),
        ),
        (
            r"[ 'b9a' ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[4294745194651189248]]"),
        ),
        (r"[ 'b9a' ] ->GEOSHAPE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"[ 'b9a' ] ->GEOSHAPE SIZE", Ok(r"[1]")),
        (
            r"[ 'b' ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[1945555039024054272]]"),
        ),
        (r"[ 'b' ] ->GEOSHAPE T GEOSHAPE->", Ok(r#"[["b"]]"#)),
        (r"[ 'b' ] ->GEOSHAPE SIZE", Ok(r"[1]")),
        (
            r"[ 'ffff' ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[5764589930848190464]]"),
        ),
        (r"[ 'ffff' ] ->GEOSHAPE T GEOSHAPE->", Ok(r#"[["ffff"]]"#)),
        (r"[ 'ffff' ] ->GEOSHAPE SIZE", Ok(r"[1]")),
        (
            r"[ '0000' ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[4611686018427387904]]"),
        ),
        (r"[ '0000' ] ->GEOSHAPE T GEOSHAPE->", Ok(r#"[["0000"]]"#)),
        (r"[ '0000' ] ->GEOSHAPE SIZE", Ok(r"[1]")),
        (
            r"[ '0' ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[1152921504606846976]]"),
        ),
        (r"[ '0' ] ->GEOSHAPE T GEOSHAPE->", Ok(r#"[["0"]]"#)),
        (r"[ '0' ] ->GEOSHAPE SIZE", Ok(r"[1]")),
        (
            r"[ '' ] ->GEOSHAPE GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ '' ] ->GEOSHAPE T GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ '' ] ->GEOSHAPE SIZE",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 1 ] ->GEOSHAPE GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 1 ] ->GEOSHAPE T GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 1 ] ->GEOSHAPE SIZE",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 0 ] ->GEOSHAPE GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 0 ] ->GEOSHAPE T GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 0 ] ->GEOSHAPE SIZE",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 0 1 ] ->GEOSHAPE GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 0 1 ] ->GEOSHAPE T GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 0 1 ] ->GEOSHAPE SIZE",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 'x' ] ->GEOSHAPE GEOSHAPE->",
            Err(r#"For input string: "x000000000000000" under radix 16"#),
        ),
        (
            r"[ 'x' ] ->GEOSHAPE T GEOSHAPE->",
            Err(r#"For input string: "x000000000000000" under radix 16"#),
        ),
        (
            r"[ 'x' ] ->GEOSHAPE SIZE",
            Err(r#"For input string: "x000000000000000" under radix 16"#),
        ),
        (
            r"[ 1.0 ] ->GEOSHAPE GEOSHAPE->",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ 1.0 ] ->GEOSHAPE T GEOSHAPE->",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ 1.0 ] ->GEOSHAPE SIZE",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ T ] ->GEOSHAPE GEOSHAPE->",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ T ] ->GEOSHAPE T GEOSHAPE->",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ T ] ->GEOSHAPE SIZE",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ NULL ] ->GEOSHAPE GEOSHAPE->",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ NULL ] ->GEOSHAPE T GEOSHAPE->",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ NULL ] ->GEOSHAPE SIZE",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ [ 1 ] ] ->GEOSHAPE GEOSHAPE->",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ [ 1 ] ] ->GEOSHAPE T GEOSHAPE->",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ [ 1 ] ] ->GEOSHAPE SIZE",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ 'b9ad' 1.0 ] ->GEOSHAPE GEOSHAPE->",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ 'b9ad' 1.0 ] ->GEOSHAPE T GEOSHAPE->",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ 'b9ad' 1.0 ] ->GEOSHAPE SIZE",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"[ 'zz' ] ->GEOSHAPE GEOSHAPE->",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"[ 'zz' ] ->GEOSHAPE T GEOSHAPE->",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"[ 'zz' ] ->GEOSHAPE SIZE",
            Err(r#"For input string: "zz00000000000000" under radix 16"#),
        ),
        (
            r"[ 'b9ad' 'x' ] ->GEOSHAPE GEOSHAPE->",
            Err(r#"For input string: "x000000000000000" under radix 16"#),
        ),
        (
            r"[ 'b9ad' 'x' ] ->GEOSHAPE T GEOSHAPE->",
            Err(r#"For input string: "x000000000000000" under radix 16"#),
        ),
        (
            r"[ 'b9ad' 'x' ] ->GEOSHAPE SIZE",
            Err(r#"For input string: "x000000000000000" under radix 16"#),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7a5ff' HEX-> ] ->GEOSHAPE GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects the given list to contain valid STRING or BYTES HHCodes. (Byte array HHCode must be of length <= 8)",
            ),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7a5ff' HEX-> ] ->GEOSHAPE T GEOSHAPE->",
            Err(
                r"->GEOSHAPE expects the given list to contain valid STRING or BYTES HHCodes. (Byte array HHCode must be of length <= 8)",
            ),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7a5ff' HEX-> ] ->GEOSHAPE SIZE",
            Err(
                r"->GEOSHAPE expects the given list to contain valid STRING or BYTES HHCodes. (Byte array HHCode must be of length <= 8)",
            ),
        ),
        (
            r"[ 'b9ad' 'b9ad' HEX-> ] ->GEOSHAPE GEOSHAPE->",
            Ok(r"[[5447895397676613632]]"),
        ),
        (
            r"[ 'b9ad' 'b9ad' HEX-> ] ->GEOSHAPE T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (r"[ 'b9ad' 'b9ad' HEX-> ] ->GEOSHAPE SIZE", Ok(r"[1]")),
        (r"[ ] ->GEOSHAPE GEOSHAPE->", Ok(r"[[]]")),
        (
            r"1 ->GEOSHAPE",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (
            r"'a' ->GEOSHAPE",
            Err(r"->GEOSHAPE expects a LIST of LONG,STRING or BYTES representing geocells."),
        ),
        (r"->GEOSHAPE", Err(r"Empty stack.")),
        (r"GEOSHAPE->", Err(r"Empty stack.")),
        (r"1 GEOSHAPE->", Err(r"GEOSHAPE-> expects a GEOSHAPE.")),
        (r"T GEOSHAPE->", Err(r"Empty stack.")),
        (r"F 1 GEOSHAPE->", Err(r"GEOSHAPE-> expects a GEOSHAPE.")),
        (
            r"[ 'b9ad' ] ->GEOSHAPE F GEOSHAPE->",
            Ok(r"[[5447895397676613632]]"),
        ),
        (
            r"[ 'b9ad' ] ->GEOSHAPE DUP T SWAP GEOSHAPE->",
            Ok(r"[[5447895397676613632],true,null]"),
        ),
        (r"[ 'b9ad' ] ->GEOSHAPE TYPEOF", Ok(r#"["GEOSHAPE"]"#)),
        (r"[ 'b9ad' ] ->GEOSHAPE ->JSON", Ok(r#"["null"]"#)),
        (
            r"[ 'b9ad' ] ->GEOSHAPE ->JSON ->JSON",
            Ok(r#"["\"null\""]"#),
        ),
        (
            r"[ 'b9ad' ] ->GEOSHAPE [ 'b9ad' ] ->GEOSHAPE ==",
            Ok(r"[true]"),
        ),
        (
            r"[ 'b9ad' ] ->GEOSHAPE [ 'b9ad' ] ->GEOSHAPE !=",
            Ok(r"[false]"),
        ),
        (
            r"[ 'b9ad' ] ->GEOSHAPE [ 'b9ac' ] ->GEOSHAPE ==",
            Ok(r"[false]"),
        ),
        (
            r"[ 'b9ad' ] ->GEOSHAPE [ 'b9ac' ] ->GEOSHAPE !=",
            Ok(r"[true]"),
        ),
        (
            r"[ 'b9ad' ] ->GEOSHAPE [ 'b9ad' 'b9' ] ->GEOSHAPE ==",
            Ok(r"[false]"),
        ),
        (r"[ 'b9ad' ] ->GEOSHAPE 1 ==", Ok(r"[false]")),
        (r"1 [ 'b9ad' ] ->GEOSHAPE ==", Ok(r"[false]")),
        (r"[ 'b9ad' ] ->GEOSHAPE DUP ==", Ok(r"[true]")),
        (r"[ ] ->GEOSHAPE [ ] ->GEOSHAPE ==", Ok(r"[true]")),
        (
            r"[ 'b9ad' ] ->GEOSHAPE SNAPSHOT",
            Ok(r#"["'T._ZHtfF.......L.V.' GEOUNPACK "]"#),
        ),
        (
            r"[ ] ->GEOSHAPE SNAPSHOT",
            Ok(r#"["'T..L...' GEOUNPACK "]"#),
        ),
        (
            r"[ 'b9ad' 'b9' 'b8' 'f' ] ->GEOSHAPE SNAPSHOT",
            Ok(r#"["'T1GZ6k........0Z9s........0Z9t........0ZHtfF.......L1..' GEOUNPACK "]"#),
        ),
        (
            r"[ 'b9ad' 'b9' 'b8' 'f' ] ->GEOSHAPE SNAPSHOT EVAL GEOSHAPE->",
            Ok(
                r"[[2233785415175766016,3134505340649865216,3139008940277235712,5447895397676613632]]",
            ),
        ),
        (r"[ 'b9ad' ] ->GEOSHAPE SIZE", Ok(r"[1]")),
        (r"[ ] ->GEOSHAPE SIZE", Ok(r"[0]")),
        (
            r"[ 'b9ad' ] ->GEOSHAPE ->LIST",
            Err(r"->LIST expects a LIST, SET or a LONG number of elements to pack into a LIST."),
        ),
        (r"[ 'b9ad' ] ->GEOSHAPE 'a' STORE $a", Ok(r"[null]")),
        (r"{ 'k' [ 'b9ad' ] ->GEOSHAPE } SIZE", Ok(r"[1]")),
        (
            r"[ 'b9ad' ] ->GEOSHAPE 1 ->LIST ->JSON",
            Ok(r#"["[null]"]"#),
        ),
        (
            r"[ 'b9ad' ] ->GEOSHAPE ->STRING",
            Err(r"Unknown function '->STRING'"),
        ),
        (r"[ 'b9ad' ] ->GEOSHAPE ->PICKLE SIZE", Ok(r"[60]")),
        (
            r"[ 'b9ad' ] ->GEOSHAPE TOSTRING SIZE",
            Err(r"TOSTRING can only operate on numeric, boolean or string values."),
        ),
        (
            r"[ 'b9ad' ] ->GEOSHAPE ->SET SIZE",
            Err(r"->SET operates on a LIST or SET."),
        ),
        (
            r"[ 'b9ad' ] ->GEOSHAPE [ 'b9ad' ] ->GEOSHAPE 2 ->LIST ->SET SIZE",
            Ok(r"[1]"),
        ),
        (
            r"[ 'b9ad' ] ->GEOSHAPE [ 'b9ad' ] ->GEOSHAPE 2 ->LIST UNIQUE SIZE",
            Ok(r"[1]"),
        ),
    ]);
}

#[test]
fn geohash() {
    check(&[
        (
            r"[ 'b9ad' ] ->GEOSHAPE ->GEOHASH",
            Ok(
                r#"[["ftg0","ftg1","ftg2","ftg3","ftg4","ftg5","ftg6","ftg7","ftg8","ftg9","ftgb","ftgc","ftgd","ftge","ftgf","ftgg"]]"#,
            ),
        ),
        (r"[ 'b9ad' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[16]")),
        (
            r"[ 'b9' ] ->GEOSHAPE ->GEOHASH",
            Ok(r#"[["fv","fs","ft","fu"]]"#),
        ),
        (r"[ 'b9' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[4]")),
        (
            r"[ 'b9a1' 'b9a3' ] ->GEOSHAPE ->GEOHASH",
            Ok(r#"[["ft9"]]"#),
        ),
        (r"[ 'b9a1' 'b9a3' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[1]")),
        (r"[ 'b' ] ->GEOSHAPE ->GEOHASH", Ok(r#"[["f","g"]]"#)),
        (r"[ 'b' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[2]")),
        (
            r"[ 'b9ad' 'b9ae' ] ->GEOSHAPE ->GEOHASH",
            Ok(
                r#"[["ftg0","ftg1","ftg2","ftg3","ftg4","ftg5","ftg6","ftg7","ftg8","ftfx","ftfy","ftfz","ftgb","ftgc","ftgd","ftge","ftgf","ftgg","ftfh","ftfj","ftfk","ftfm","ftfn","ftfp","ftfq","ftfr","ftfs","ftft","ftfu","ftfv","ftfw","ftg9"]]"#,
            ),
        ),
        (r"[ 'b9ad' 'b9ae' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[32]")),
        (r"[ 'f' ] ->GEOSHAPE ->GEOHASH", Ok(r#"[["y","z"]]"#)),
        (r"[ 'f' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[2]")),
        (r"[ '0' ] ->GEOSHAPE ->GEOHASH", Ok(r#"[["0","1"]]"#)),
        (r"[ '0' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[2]")),
        (
            r"[ '0' '1' '2' '3' '4' '5' '6' '7' '8' '9' 'a' 'b' 'c' 'd' 'e' 'f' ] ->GEOSHAPE ->GEOHASH",
            Ok(r#"[[""]]"#),
        ),
        (
            r"[ '0' '1' '2' '3' '4' '5' '6' '7' '8' '9' 'a' 'b' 'c' 'd' 'e' 'f' ] ->GEOSHAPE ->GEOHASH SIZE",
            Ok(r"[1]"),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE ->GEOHASH",
            Ok(
                r#"[["ftg2vn6bbvex","ftg2vn6bbvey","ftg2vn6bbvez","ftg2vn6bbvet","ftg2vn6bbveu","ftg2vn6bbvev","ftg2vn6bbvew","ftg2vn6bbveh","ftg2vn6bbvej","ftg2vn6bbvek","ftg2vn6bbvep","ftg2vn6bbveq","ftg2vn6bbver","ftg2vn6bbves","ftg2vn6bbvem","ftg2vn6bbven"]]"#,
            ),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE ->GEOHASH SIZE",
            Ok(r"[16]"),
        ),
        (
            r"[ 'b9ad1ee0' ] ->GEOSHAPE ->GEOHASH",
            Ok(
                r#"[["ftg2vn2","ftg2vn1","ftg2vn4","ftg2vn3","ftg2vn6","ftg2vn5","ftg2vn7","ftg2vn0"]]"#,
            ),
        ),
        (r"[ 'b9ad1ee0' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[8]")),
        (
            r"[ 'b9ad1e' ] ->GEOSHAPE ->GEOHASH",
            Ok(r#"[["ftg2v","ftg2u"]]"#),
        ),
        (r"[ 'b9ad1e' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[2]")),
        (
            r"[ 'b9ad1ee0c5' ] ->GEOSHAPE ->GEOHASH",
            Ok(r#"[["ftg2vn6b"]]"#),
        ),
        (r"[ 'b9ad1ee0c5' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[1]")),
        (
            r"[ 'b9ad1ee0c5a9' ] ->GEOSHAPE ->GEOHASH",
            Ok(r#"[["ftg2vn6bbt","ftg2vn6bbs","ftg2vn6bbv","ftg2vn6bbu"]]"#),
        ),
        (r"[ 'b9ad1ee0c5a9' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[4]")),
        (
            r"[ 'b9ad1ee0c5a9e7a5' ] ->GEOSHAPE ->GEOHASH",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7a5' ] ->GEOSHAPE ->GEOHASH SIZE",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7a5' ] ->GEOSHAPE ->GEOHASH",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7a5' ] ->GEOSHAPE ->GEOHASH SIZE",
            Err(
                r"->GEOSHAPE expects geocells resolutions to be even and between 2 and 30, inclusive.",
            ),
        ),
        (
            r"[ 'ffff' ] ->GEOSHAPE ->GEOHASH",
            Ok(
                r#"[["zzzs","zzzr","zzzu","zzzt","zzzw","zzzv","zzzy","zzzx","zzzk","zzzj","zzzm","zzzn","zzzq","zzzp","zzzh","zzzz"]]"#,
            ),
        ),
        (r"[ 'ffff' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[16]")),
        (
            r"[ '0000' ] ->GEOSHAPE ->GEOHASH",
            Ok(
                r#"[["000f","000g","000d","000e","000b","000c","0006","0007","0004","0005","0002","0003","0000","0001","0008","0009"]]"#,
            ),
        ),
        (r"[ '0000' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[16]")),
        (
            r"[ 'a0' ] ->GEOSHAPE ->GEOHASH",
            Ok(r#"[["b2","b3","b0","b1"]]"#),
        ),
        (r"[ 'a0' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[4]")),
        (
            r"[ 'a0' 'a1' 'a2' 'a3' ] ->GEOSHAPE ->GEOHASH",
            Ok(
                r#"[["bb","bc","bd","be","bf","bg","b0","b1","b2","b3","b4","b5","b6","b7","b8","b9"]]"#,
            ),
        ),
        (
            r"[ 'a0' 'a1' 'a2' 'a3' ] ->GEOSHAPE ->GEOHASH SIZE",
            Ok(r"[16]"),
        ),
        (
            r"[ 'e0' 'e1' ] ->GEOSHAPE ->GEOHASH",
            Ok(r#"[["u8","u9","u0","u1","u2","ub","u3","uc"]]"#),
        ),
        (r"[ 'e0' 'e1' ] ->GEOSHAPE ->GEOHASH SIZE", Ok(r"[8]")),
        (
            r"[ 'b9ad' 'b9ac' 'b9ab' 'b9aa' 'b9a9' 'b9a8' 'b9a7' 'b9a6' ] ->GEOSHAPE ->GEOHASH",
            Ok(
                r#"[["ftgb","ftgc","ftgd","ftge","ftgf","ftgg","ftew","ftg9","ftex","ftey","ftez","ftfb","ftfc","ftfd","ftfe","ftff","ftfg","fteh","ftej","ftek","ftem","ften","ftg0","ftg1","ftep","ftg2","fteq","ftg3","fter","ftg4","ftes","ftg5","ftet","ftg6","fteu","ftg7","ftev","ftg8","ftdv","ftf8","ftdw","ftf9","ftdx","ftdy","ftc","ftdz","ftb","ftdh","ftdj","ftdk","ftdm","ftdn","ftf0","ftf1","ftdp","ftf2","ftdq","ftf3","ftdr","ftf4","ftds","ftf5","ftdt","ftf6","ftdu","ftf7"]]"#,
            ),
        ),
        (
            r"[ 'b9ad' 'b9ac' 'b9ab' 'b9aa' 'b9a9' 'b9a8' 'b9a7' 'b9a6' ] ->GEOSHAPE ->GEOHASH SIZE",
            Ok(r"[66]"),
        ),
        (r"[ ] ->GEOSHAPE ->GEOHASH", Ok(r"[[]]")),
        (r"48.0 2.0 ->GEOHASH", Ok(r#"["u093jd0k72s1"]"#)),
        (r"48.85 2.35 ->GEOHASH", Ok(r#"["u09tvkz5yup0"]"#)),
        (r"-90.0 -180.0 ->GEOHASH", Ok(r#"["000000000000"]"#)),
        (r"90.0 180.0 ->GEOHASH", Ok(r#"["upbpbpbpbpbp"]"#)),
        (r"0.0 0.0 ->GEOHASH", Ok(r#"["s00000000000"]"#)),
        (r"-0.0 -0.0 ->GEOHASH", Ok(r#"["s00000000000"]"#)),
        (r"45.123456 -73.98765 ->GEOHASH", Ok(r#"["f252sq7z4vce"]"#)),
        (
            r"1 2.0 ->GEOHASH",
            Err(r"->GEOHASH expects a latitude and a longitude on the stack."),
        ),
        (r"1.0 2 ->GEOHASH", Ok(r#"["000000000000",1.0]"#)),
        (
            r"1.0 'a' ->GEOHASH",
            Err(r"->GEOHASH operates on a HHCode, a GEOSHAPE or a latitude and longitude."),
        ),
        (r"1.0 ->GEOHASH", Err(r"Empty stack.")),
        (
            r"'a' ->GEOHASH",
            Err(r"->GEOHASH operates on a HHCode, a GEOSHAPE or a latitude and longitude."),
        ),
        (r"->GEOHASH", Err(r"Empty stack.")),
        (
            r"T ->GEOHASH",
            Err(r"->GEOHASH operates on a HHCode, a GEOSHAPE or a latitude and longitude."),
        ),
        (
            r"NULL ->GEOHASH",
            Err(r"->GEOHASH operates on a HHCode, a GEOSHAPE or a latitude and longitude."),
        ),
        (r"0 ->GEOHASH", Ok(r#"["000000000000"]"#)),
        (r"-1 ->GEOHASH", Ok(r#"["zzzzzzzzzzzz"]"#)),
        (r"-5064967386357852251 ->GEOHASH", Ok(r#"["ftx6vzx66epp"]"#)),
        (r"9223372036854775807 ->GEOHASH", Ok(r#"["rzzzzzzzzzzz"]"#)),
        (r"-9223372036854775808 ->GEOHASH", Ok(r#"["800000000000"]"#)),
        (
            r"[ 1 ] ->GEOHASH",
            Err(r"->GEOHASH operates on a HHCode, a GEOSHAPE or a latitude and longitude."),
        ),
        (
            r"'u09tvw0' GEOHASH->",
            Ok(r"[2.35107421875,48.856201171875]"),
        ),
        (r"'u09tvw0' T GEOHASH->", Ok(r#"["e0236ef000000000"]"#)),
        (r"'u09tvw0' F GEOHASH->", Ok(r"[-2295869407957614592]")),
        (
            r"'U09TVW0' GEOHASH->",
            Ok(r"[2.35107421875,48.856201171875]"),
        ),
        (r"'u09' GEOHASH->", Ok(r"[1.40625,47.8125]")),
        (r"'' GEOHASH->", Ok(r"[-180.0,-90.0]")),
        (r"'' T GEOHASH->", Ok(r#"["0000000000000000"]"#)),
        (r"'' F GEOHASH->", Ok(r"[0]")),
        (r"'a' GEOHASH->", Err(r"Invalid GeoHash 'a' at index 0")),
        (
            r"'u09a' GEOHASH->",
            Err(r"Invalid GeoHash 'u09a' at index 3"),
        ),
        (
            r"'u09tvw0abcdefghjk' GEOHASH->",
            Err(r"Invalid GeoHash 'u09tvw0abcdefghjk' at index 7"),
        ),
        (
            r"'u09tvw0bcdefg' GEOHASH->",
            Ok(r"[2.352173589169979,48.856343664228916]"),
        ),
        (
            r"'u09tvkz5yup0' GEOHASH->",
            Ok(r"[2.3499999940395355,48.84999990463257]"),
        ),
        (
            r"'u09tvkz5yup0z' GEOHASH->",
            Ok(r"[2.3499999940395355,48.84999990463257]"),
        ),
        (
            r"'zzzzzzzzzzzz' GEOHASH->",
            Ok(r"[179.99999966472387,89.99999983236194]"),
        ),
        (r"'000000000000' GEOHASH->", Ok(r"[-180.0,-90.0]")),
        (r"'u09tvkz5yup0' F GEOHASH->", Ok(r"[-2295869580377500416]")),
        (r"'u09tvkz5yup0' T GEOHASH->", Ok(r#"["e0236ec7daf95500"]"#)),
        (
            r"1 GEOHASH->",
            Err(r"GEOHASH-> expects a GeoHash to be a string."),
        ),
        (r"T GEOHASH->", Err(r"Empty stack.")),
        (r"GEOHASH->", Err(r"Empty stack.")),
        (
            r"'u09' T F GEOHASH->",
            Err(r"GEOHASH-> expects a GeoHash to be a string."),
        ),
        (
            r"NULL GEOHASH->",
            Err(r"GEOHASH-> expects a GeoHash to be a string."),
        ),
        (
            r"1.0 GEOHASH->",
            Err(r"GEOHASH-> expects a GeoHash to be a string."),
        ),
        (
            r"[ 'u09' ] GEOHASH-> GEOSHAPE->",
            Ok(r"[[5621072877097844736,5621108061469933568]]"),
        ),
        (
            r"[ 'U09' ] GEOHASH-> GEOSHAPE->",
            Ok(r"[[5621072877097844736,5621108061469933568]]"),
        ),
        (r"[ ] GEOHASH-> GEOSHAPE->", Ok(r"[[]]")),
        (
            r"[ 'u' ] GEOHASH-> GEOSHAPE->",
            Ok(
                r"[[3314649325744685056,3319152925372055552,3323656524999426048,3328160124626796544,3350678122763649024,3355181722391019520,3359685322018390016,3364188921645760512]]",
            ),
        ),
        (
            r"[ 'u0' 'u1' ] GEOHASH-> GEOSHAPE->",
            Ok(
                r"[[4467570830351532032,4467852305328242688,4468133780304953344,4468415255281664000,4469822630165217280,4470104105141927936,4470385580118638592,4470667055095349248]]",
            ),
        ),
        (
            r"[ 'u0' 'u1' 'u2' 'u3' 'u4' 'u5' 'u6' 'u7' 'u8' 'u9' 'ub' 'uc' 'ud' 'ue' 'uf' 'ug' 'uh' 'uj' 'uk' 'um' 'un' 'up' 'uq' 'ur' 'us' 'ut' 'uu' 'uv' 'uw' 'ux' 'uy' 'uz' ] GEOHASH-> GEOSHAPE->",
            Ok(
                r"[[3314649325744685056,3319152925372055552,3323656524999426048,3328160124626796544,3350678122763649024,3355181722391019520,3359685322018390016,3364188921645760512]]",
            ),
        ),
        (
            r"[ 'u09tv' ] GEOHASH-> GEOSHAPE->",
            Ok(
                r"[[9079880151612784640,9079880155907751936,9079880160202719232,9079880164497686528,9079880185972523008,9079880190267490304,9079880194562457600,9079880198857424896]]",
            ),
        ),
        (
            r"[ 'u09tvw' ] GEOHASH-> GEOSHAPE->",
            Ok(
                r"[[-8213942370245279744,-8213942369976844288,-8213942369708408832,-8213942369439973376]]",
            ),
        ),
        (
            r"[ 'u09tvw0' ] GEOHASH-> GEOSHAPE->",
            Ok(r"[[-7061020865638432768,-7061020865604878336]]"),
        ),
        (
            r"[ 'u09tvw0b' ] GEOHASH-> GEOSHAPE->",
            Ok(r"[[-5908099361026342912]]"),
        ),
        (
            r"[ 'u09tvw0b2' ] GEOHASH-> GEOSHAPE->",
            Ok(
                r"[[-3602256351812517888,-3602256351812513792,-3602256351812509696,-3602256351812505600,-3602256351812485120,-3602256351812481024,-3602256351812476928,-3602256351812472832]]",
            ),
        ),
        (
            r"[ 'u09tvw0b2d' ] GEOHASH-> GEOSHAPE->",
            Ok(
                r"[[-2449334847205658624,-2449334847205658368,-2449334847205658112,-2449334847205657856]]",
            ),
        ),
        (
            r"[ 'u09tvw0b2de' ] GEOHASH-> GEOSHAPE->",
            Ok(r"[[-1296413342598811056,-1296413342598811024]]"),
        ),
        (
            r"[ 'u09tvw0b2dez' ] GEOHASH-> GEOSHAPE->",
            Ok(r"[[-143491837991964033]]"),
        ),
        (
            r"[ 'u09tvw0b2dezzzz' ] GEOHASH-> GEOSHAPE->",
            Ok(r"[[-143491837991964033]]"),
        ),
        (
            r"[ 'u09tvw0b2dezzzz' 'u09tvw0b2dez' ] GEOHASH-> GEOSHAPE->",
            Ok(r"[[-143491837991964033]]"),
        ),
        (
            r"[ 'u09tvw0b2d' 'u09tvw0b2dez' 'u09tvw0b2dp' ] GEOHASH-> GEOSHAPE->",
            Ok(
                r"[[-2449334847205658624,-2449334847205658368,-2449334847205658112,-2449334847205657856]]",
            ),
        ),
        (
            r"[ 'u09' 'u09t' ] GEOHASH-> GEOSHAPE->",
            Ok(r"[[5621072877097844736,5621108061469933568]]"),
        ),
        (
            r"[ 'u09t' 'u09' ] GEOHASH-> GEOSHAPE->",
            Ok(r"[[5621072877097844736,5621108061469933568,6774036163146547200]]"),
        ),
        (
            r"[ 'ezs42' ] GEOHASH-> GEOSHAPE->",
            Ok(
                r"[[8790753931182473216,8790753935477440512,8790753939772407808,8790753944067375104,8790753965542211584,8790753969837178880,8790753974132146176,8790753978427113472]]",
            ),
        ),
        (r"[ 'a' ] GEOHASH->", Err(r"Invalid GeoHash 'a' at index 0")),
        (
            r"[ 1 ] GEOHASH->",
            Err(r"GEOHASH-> expects a GeoHash to be a string."),
        ),
        (
            r"[ 'u0' 1 ] GEOHASH->",
            Err(r"GEOHASH-> expects a GeoHash to be a string."),
        ),
        (
            r"[ 'u0' 'a' ] GEOHASH->",
            Err(r"Invalid GeoHash 'a' at index 0"),
        ),
        (r"[ '' ] GEOHASH-> GEOSHAPE->", Ok(r"[[]]")),
        (
            r"[ '' 'u0' ] GEOHASH-> GEOSHAPE->",
            Ok(
                r"[[4467570830351532032,4467852305328242688,4468133780304953344,4468415255281664000]]",
            ),
        ),
        (r"[ 'u09' ] GEOHASH-> ->GEOHASH", Ok(r#"[["u09"]]"#)),
        (r"[ 'u09' ] GEOHASH-> ->GEOHASH SIZE", Ok(r"[1]")),
        (r"[ 'u0' ] GEOHASH-> ->GEOHASH", Ok(r#"[["u0"]]"#)),
        (r"[ 'u' ] GEOHASH-> ->GEOHASH", Ok(r#"[["u"]]"#)),
        (r"[ 'u09tv' ] GEOHASH-> ->GEOHASH", Ok(r#"[["u09tv"]]"#)),
        (r"[ 'u09tvw0' ] GEOHASH-> ->GEOHASH", Ok(r#"[["u09tvw0"]]"#)),
        (
            r"[ 'u09tvw0b2dez' ] GEOHASH-> ->GEOHASH",
            Ok(r#"[["u09tvw0b2dez"]]"#),
        ),
        (r"'u09tvw0' GEOHASH-> ->GEOHASH", Ok(r#"["u09tvw000000"]"#)),
        (
            r"'u09tvw0' F GEOHASH-> ->GEOHASH",
            Ok(r#"["u09tvw000000"]"#),
        ),
        (
            r"48.0 2.0 ->GEOHASH GEOHASH->",
            Ok(r"[1.9999998807907104,47.99999998882413]"),
        ),
        (
            r"48.0 2.0 ->GEOHASH F GEOHASH-> ->GEOHASH",
            Ok(r#"["u093jd0k72s1"]"#),
        ),
    ]);
}

#[test]
fn pack() {
    check(&[
        (
            r"[ 'b9ad' ] ->GEOSHAPE GEOPACK",
            Ok(r#"["T._ZHtfF.......L.V."]"#),
        ),
        (r"[ ] ->GEOSHAPE GEOPACK", Ok(r#"["T..L..."]"#)),
        (
            r"[ 'b9ad' 'b9' 'b8' 'f' ] ->GEOSHAPE GEOPACK",
            Ok(r#"["T1GZ6k........0Z9s........0Z9t........0ZHtfF.......L1.."]"#),
        ),
        (
            r"[ 'b9ad' ] ->GEOSHAPE GEOPACK GEOUNPACK GEOSHAPE->",
            Ok(r"[[5447895397676613632]]"),
        ),
        (r"[ ] ->GEOSHAPE GEOPACK GEOUNPACK GEOSHAPE->", Ok(r"[[]]")),
        (
            r"[ 'b9ad' 'b9' 'b8' 'f' ] ->GEOSHAPE GEOPACK GEOUNPACK GEOSHAPE->",
            Ok(
                r"[[2233785415175766016,3134505340649865216,3139008940277235712,5447895397676613632]]",
            ),
        ),
        (
            r"[ 'b9ad' 'b9' 'b8' 'f' ] ->GEOSHAPE GEOPACK OPB64-> GEOUNPACK GEOSHAPE->",
            Ok(
                r"[[2233785415175766016,3134505340649865216,3139008940277235712,5447895397676613632]]",
            ),
        ),
        (
            r"'T._ZHtfF.......L.V.' GEOUNPACK GEOSHAPE->",
            Ok(r"[[5447895397676613632]]"),
        ),
        (
            r"'T._ZHtfF.......L.V.' OPB64-> 3 SUBSTRING GEOUNPACK",
            Err(
                r"org.apache.thrift.protocol.TProtocolException: Unrecognized type 0 (Unrecognized type 0)",
            ),
        ),
        (
            r"'T._ZHtfF.......L.V.' OPB64-> 0 5 SUBSTRING GEOUNPACK",
            Err(
                r"org.apache.thrift.transport.TTransportException: Cannot read. Remote side has closed. Tried to read 9 bytes, but only got 3 bytes. (This is often indicative of an internal error on the server side. Please check your server logs.) (Cannot read. Remote side has closed. Tried to read 9 bytes, but only got 3 bytes. (This is often indicative of an internal error on the server side. Please check your server logs.))",
            ),
        ),
        (
            r"'abc' GEOUNPACK",
            Err(
                r"org.apache.thrift.transport.TTransportException: Cannot read. Remote side has closed. Tried to read 1 bytes, but only got 0 bytes. (This is often indicative of an internal error on the server side. Please check your server logs.) (Cannot read. Remote side has closed. Tried to read 1 bytes, but only got 0 bytes. (This is often indicative of an internal error on the server side. Please check your server logs.))",
            ),
        ),
        (
            r"'' GEOUNPACK",
            Err(
                r"org.apache.thrift.transport.TTransportException: Cannot read. Remote side has closed. Tried to read 1 bytes, but only got 0 bytes. (This is often indicative of an internal error on the server side. Please check your server logs.) (Cannot read. Remote side has closed. Tried to read 1 bytes, but only got 0 bytes. (This is often indicative of an internal error on the server side. Please check your server logs.))",
            ),
        ),
        (
            r"'zzzz' GEOUNPACK",
            Err(
                r"org.apache.thrift.protocol.TProtocolException: don't know what type: 15 (don't know what type: 15)",
            ),
        ),
        (
            r"'T._ZHtfF.......L.V.' 1 ->LIST GEOUNPACK",
            Err(r"GEOUNPACK expects a packed shape on top of the stack."),
        ),
        (
            r"T GEOUNPACK",
            Err(r"GEOUNPACK expects a packed shape on top of the stack."),
        ),
        (
            r"1 GEOUNPACK",
            Err(r"GEOUNPACK expects a packed shape on top of the stack."),
        ),
        (r"GEOUNPACK", Err(r"Empty stack.")),
        (
            r"NULL GEOUNPACK",
            Err(r"GEOUNPACK expects a packed shape on top of the stack."),
        ),
        (
            r"'780b644b9ad000000000000402160400' HEX-> GEOUNPACK GEOSHAPE->",
            Ok(r"[[5447895397676613632,5447895397676613633]]"),
        ),
        (
            r"'780b644b9ad0000000000004021600' HEX-> GEOUNPACK GEOSHAPE->",
            Err(
                r"org.apache.thrift.transport.TTransportException: Cannot read. Remote side has closed. Tried to read 1 bytes, but only got 0 bytes. (This is often indicative of an internal error on the server side. Please check your server logs.) (Cannot read. Remote side has closed. Tried to read 1 bytes, but only got 0 bytes. (This is often indicative of an internal error on the server side. Please check your server logs.))",
            ),
        ),
        (
            r"'780b644b9ad000000000000402160200' HEX-> GEOUNPACK GEOSHAPE->",
            Ok(r"[[5447895397676613632,5447895397676613633]]"),
        ),
        (
            r"'780b644b9ad000000000000402160100' HEX-> GEOUNPACK GEOSHAPE->",
            Err(r"-1"),
        ),
        (
            r"'780b644b9ad000000000000402160400' HEX-> OPB64-> GEOUNPACK",
            Err(r"OPB64-> operates on a String."),
        ),
        (r"'160400' HEX-> GEOUNPACK", Err(r"NullPointerException")),
        (
            r"'7802ffff160400' HEX-> GEOUNPACK GEOSHAPE->",
            Err(r"BufferUnderflowException"),
        ),
        (
            r"'7802ffff160200' HEX-> GEOUNPACK GEOSHAPE->",
            Err(r"BufferUnderflowException"),
        ),
        (
            r"NEWGTS 5 NaN NaN NaN T ADDVALUE WRAPRAW GEOUNPACK GEOSHAPE->",
            Ok(r"[[]]"),
        ),
        (
            r"NEWGTS 5 NaN NaN NaN T ADDVALUE 5447895397676613632 NaN NaN NaN T ADDVALUE WRAPRAW GEOUNPACK GEOSHAPE->",
            Ok(r"[[5447895397676613632]]"),
        ),
        (
            r"'!!' GEOUNPACK",
            Err(r"Invalid OPB64 character '!' at offset 0."),
        ),
        (
            r"'é' GEOUNPACK",
            Err(r"Invalid OPB64 character '?' at offset 0."),
        ),
        (
            r"'T.é' GEOUNPACK",
            Err(r"Invalid OPB64 character '?' at offset 2."),
        ),
        (r"[ 'b9ad' ] ->GEOSHAPE GEOPACK SIZE", Ok(r"[19]")),
        (
            r"1 GEOPACK",
            Err(r"GEOPACK expects a shape on top of the stack."),
        ),
        (r"GEOPACK", Err(r"Empty stack.")),
        (
            r"[ 1 ] GEOPACK",
            Err(r"GEOPACK expects a shape on top of the stack."),
        ),
        (
            r"'a' GEOPACK",
            Err(r"GEOPACK expects a shape on top of the stack."),
        ),
        (
            r"NULL GEOPACK",
            Err(r"GEOPACK expects a shape on top of the stack."),
        ),
    ]);
}
