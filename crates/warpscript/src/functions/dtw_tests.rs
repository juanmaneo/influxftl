// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Oracle-checked tests of `DTW`, `ZDTW`, `RAWDTW` and `OPTDTW`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const A: &str = r"NEWGTS 'a' RENAME 1 NaN NaN NaN 1.438 ADDVALUE 2 NaN NaN NaN 2.524 ADDVALUE 3 NaN NaN NaN 2.992 ADDVALUE 4 NaN NaN NaN 2.728 ADDVALUE 5 NaN NaN NaN 1.795 ADDVALUE 6 NaN NaN NaN 0.423 ADDVALUE 7 NaN NaN NaN -1.052 ADDVALUE 8 NaN NaN NaN -2.27 ADDVALUE 9 NaN NaN NaN -2.933 ADDVALUE 10 NaN NaN NaN -2.877 ADDVALUE 11 NaN NaN NaN -2.117 ADDVALUE 12 NaN NaN NaN -0.838 ADDVALUE";
const B: &str = r"NEWGTS 'b' RENAME 2 NaN NaN NaN 2.55 ADDVALUE 4 NaN NaN NaN 3.156 ADDVALUE 6 NaN NaN NaN 3.039 ADDVALUE 8 NaN NaN NaN 2.226 ADDVALUE 10 NaN NaN NaN 0.918 ADDVALUE 12 NaN NaN NaN -0.567 ADDVALUE 14 NaN NaN NaN -1.863 ADDVALUE 16 NaN NaN NaN -2.655 ADDVALUE 18 NaN NaN NaN -2.747 ADDVALUE";
const AL: &str = r"NEWGTS 'al' RENAME 1 NaN NaN NaN 14 ADDVALUE 2 NaN NaN NaN 25 ADDVALUE 3 NaN NaN NaN 29 ADDVALUE 4 NaN NaN NaN 27 ADDVALUE 5 NaN NaN NaN 17 ADDVALUE 6 NaN NaN NaN 4 ADDVALUE 7 NaN NaN NaN -10 ADDVALUE 8 NaN NaN NaN -22 ADDVALUE 9 NaN NaN NaN -29 ADDVALUE 10 NaN NaN NaN -28 ADDVALUE 11 NaN NaN NaN -21 ADDVALUE 12 NaN NaN NaN -8 ADDVALUE";
const R: &str = r"NEWGTS 'r' RENAME 5 NaN NaN NaN 7.5 ADDVALUE 4 NaN NaN NaN 6.0 ADDVALUE 3 NaN NaN NaN 4.5 ADDVALUE 2 NaN NaN NaN 3.0 ADDVALUE 1 NaN NaN NaN 1.5 ADDVALUE";
const K: &str =
    r"NEWGTS 'k' RENAME 1 NaN NaN NaN 2 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 3 NaN NaN NaN 2 ADDVALUE";
const S: &str = r"NEWGTS 's' RENAME 1 NaN NaN NaN 'x' ADDVALUE";
const E: &str = r"NEWGTS 'e' RENAME";
const G1: &str = r"NEWGTS 'g1' RENAME 1 48.0 -4.5 10 1 ADDVALUE 2 48.1 -4.4 20 2 ADDVALUE 3 48.3 -4.2 15 3 ADDVALUE 4 48.2 -4.0 30 4 ADDVALUE";
const G2: &str = r"NEWGTS 'g2' RENAME 1 48.05 -4.45 12 1 ADDVALUE 3 48.25 -4.3 18 2 ADDVALUE 5 48.2 -4.1 25 3 ADDVALUE";
const G3: &str = r"NEWGTS 'g3' RENAME 1 48.0 -4.5 10 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE";
const G4: &str = r"NEWGTS 'g4' RENAME 1 48.0 -4.5 NaN 1 ADDVALUE 2 48.1 -4.4 NaN 2 ADDVALUE";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@AL", AL)
        .replace("@G1", G1)
        .replace("@G2", G2)
        .replace("@G3", G3)
        .replace("@G4", G4)
        .replace("@A", A)
        .replace("@B", B)
        .replace("@R", R)
        .replace("@K", K)
        .replace("@S", S)
        .replace("@E", E)
}

/// The stack left by `script`, top first, as the JSON array of the Warp 10 exec endpoint,
/// or the error message.
fn stack_json(script: &str) -> Result<String, String> {
    let mut stack = test_stack();
    stack
        .exec_checked(&format!("{script} DEPTH ->LIST REVERSE ->JSON"))
        .map_err(|err| err.to_string())?;
    match stack.pop().unwrap() {
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
fn dtw() {
    check(&[
        (r"@A @B 0 DTW", Ok(r"[0.8940960721538282]")),
        (r"@A @B 0 ZDTW", Ok(r"[6.922149041953139]")),
        (r"@A @B 0 RAWDTW", Ok(r"[6.377999999999999]")),
        (r"@A @B 5.0 DTW", Ok(r"[0.8940960721538282]")),
        (r"@A @B 5.0 ZDTW", Ok(r"[-1.0]")),
        (r"@A @B 5.0 RAWDTW", Ok(r"[-1.0]")),
        (r"@A @B 0.5 DTW", Ok(r"[-1.0]")),
        (r"@A @B 0.5 ZDTW", Ok(r"[-1.0]")),
        (r"@A @B 0.5 RAWDTW", Ok(r"[-1.0]")),
        (r"@B @A 0 DTW", Ok(r"[0.8940960721538282]")),
        (r"@B @A 0 ZDTW", Ok(r"[5.63693166573729]")),
        (r"@B @A 0 RAWDTW", Ok(r"[6.377999999999999]")),
        (r"@B @A 5.0 DTW", Ok(r"[0.8940960721538282]")),
        (r"@B @A 5.0 ZDTW", Ok(r"[-1.0]")),
        (r"@B @A 5.0 RAWDTW", Ok(r"[-1.0]")),
        (r"@B @A 0.5 DTW", Ok(r"[-1.0]")),
        (r"@B @A 0.5 ZDTW", Ok(r"[-1.0]")),
        (r"@B @A 0.5 RAWDTW", Ok(r"[-1.0]")),
        (r"@A @A 0 DTW", Ok(r"[0.0]")),
        (r"@A @A 0 ZDTW", Ok(r"[4.325346130400839]")),
        (r"@A @A 0 RAWDTW", Ok(r"[0.0]")),
        (r"@A @A 5.0 DTW", Ok(r"[0.0]")),
        (r"@A @A 5.0 ZDTW", Ok(r"[4.325346130400839]")),
        (r"@A @A 5.0 RAWDTW", Ok(r"[0.0]")),
        (r"@A @A 0.5 DTW", Ok(r"[0.0]")),
        (r"@A @A 0.5 ZDTW", Ok(r"[-1.0]")),
        (r"@A @A 0.5 RAWDTW", Ok(r"[0.0]")),
        (r"@A @AL 0 DTW", Ok(r"[0.0665677287938311]")),
        (r"@A @AL 0 ZDTW", Ok(r"[4.1966551300358255]")),
        (r"@A @AL 0 RAWDTW", Ok(r"[95.81300000000002]")),
        (r"@A @AL 5.0 DTW", Ok(r"[0.0665677287938311]")),
        (r"@A @AL 5.0 ZDTW", Ok(r"[4.1966551300358255]")),
        (r"@A @AL 5.0 RAWDTW", Ok(r"[-1.0]")),
        (r"@A @AL 0.5 DTW", Ok(r"[0.0665677287938311]")),
        (r"@A @AL 0.5 ZDTW", Ok(r"[-1.0]")),
        (r"@A @AL 0.5 RAWDTW", Ok(r"[-1.0]")),
        (r"@A @R 0 DTW", Ok(r"[4.690168776371308]")),
        (r"@A @R 0 ZDTW", Ok(r"[23.459435666987915]")),
        (r"@A @R 0 RAWDTW", Ok(r"[41.265]")),
        (r"@A @R 5.0 DTW", Ok(r"[4.690168776371308]")),
        (r"@A @R 5.0 ZDTW", Ok(r"[-1.0]")),
        (r"@A @R 5.0 RAWDTW", Ok(r"[-1.0]")),
        (r"@A @R 0.5 DTW", Ok(r"[-1.0]")),
        (r"@A @R 0.5 ZDTW", Ok(r"[-1.0]")),
        (r"@A @R 0.5 RAWDTW", Ok(r"[-1.0]")),
        (r"@B @R 0 DTW", Ok(r"[3.564458749788243]")),
        (r"@B @R 0 ZDTW", Ok(r"[21.824569423198717]")),
        (r"@B @R 0 RAWDTW", Ok(r"[32.333]")),
        (r"@B @R 5.0 DTW", Ok(r"[3.564458749788243]")),
        (r"@B @R 5.0 ZDTW", Ok(r"[-1.0]")),
        (r"@B @R 5.0 RAWDTW", Ok(r"[-1.0]")),
        (r"@B @R 0.5 DTW", Ok(r"[-1.0]")),
        (r"@B @R 0.5 ZDTW", Ok(r"[-1.0]")),
        (r"@B @R 0.5 RAWDTW", Ok(r"[-1.0]")),
        (r"@A @B 0 0 DTW", Ok(r"[0.8940960721538282]")),
        (r"@A @B 0 0 RAWDTW", Ok(r"[6.377999999999999]")),
        (r"@A @B 1 0 DTW", Ok(r"[0.8940960721538282]")),
        (r"@A @B 1 0 RAWDTW", Ok(r"[6.377999999999999]")),
        (r"@A @B 2 0 DTW", Ok(r"[0.8940960721538282]")),
        (r"@A @B 2 0 RAWDTW", Ok(r"[6.377999999999999]")),
        (r"@A @B 3 0 DTW", Ok(r"[0.8940960721538282]")),
        (r"@A @B 3 0 RAWDTW", Ok(r"[6.377999999999999]")),
        (r"@A @B 5 0 DTW", Ok(r"[0.8940960721538282]")),
        (r"@A @B 5 0 RAWDTW", Ok(r"[6.377999999999999]")),
        (r"@A @B 100 0 DTW", Ok(r"[0.8940960721538282]")),
        (r"@A @B 100 0 RAWDTW", Ok(r"[6.377999999999999]")),
        (r"@A @B -1 0 DTW", Ok(r"[0.8940960721538282]")),
        (r"@A @B -1 0 RAWDTW", Ok(r"[6.377999999999999]")),
        (
            r"@A @B -9223372036854775808 0 DTW",
            Ok(r"[0.8940960721538282]"),
        ),
        (
            r"@A @B -9223372036854775808 0 RAWDTW",
            Ok(r"[6.377999999999999]"),
        ),
        (
            r"@A @B 0 'manhattan' RAWDTW",
            Err(
                r"RAWDTW expects the characteristic of the GTS to compute the DTW on to be values, locations, elevations or timestamps.",
            ),
        ),
        (
            r"@A @B 0 'euclidean' RAWDTW",
            Err(
                r"RAWDTW expects the characteristic of the GTS to compute the DTW on to be values, locations, elevations or timestamps.",
            ),
        ),
        (
            r"@A @B 0 'squaredeuclidean' RAWDTW",
            Err(
                r"RAWDTW expects the characteristic of the GTS to compute the DTW on to be values, locations, elevations or timestamps.",
            ),
        ),
        (
            r"@A @B 0 'MANHATTAN' RAWDTW",
            Err(
                r"RAWDTW expects the characteristic of the GTS to compute the DTW on to be values, locations, elevations or timestamps.",
            ),
        ),
        (
            r"@A @B 0 'Euclidean' RAWDTW",
            Err(
                r"RAWDTW expects the characteristic of the GTS to compute the DTW on to be values, locations, elevations or timestamps.",
            ),
        ),
        (
            r"@A @B 0 'nope' RAWDTW",
            Err(
                r"RAWDTW expects the characteristic of the GTS to compute the DTW on to be values, locations, elevations or timestamps.",
            ),
        ),
        (r"@A @B 0 'values' RAWDTW", Ok(r"[6.377999999999999]")),
        (r"@A @B 0 'timestamps' RAWDTW", Ok(r"[18.0]")),
        (r"@A @B 0 'timestamps' DTW", Ok(r"[0.3409090909090907]")),
        (r"@A @B 0 'timestamps' ZDTW", Ok(r"[19.595540143366218]")),
        (r"@A @B 0 'manhattan' 'timestamps' RAWDTW", Ok(r"[18.0]")),
        (
            r"@A @B 0 'euclidean' 'values' ZDTW",
            Ok(r"[6.922149041953139]"),
        ),
        (
            r"@A @B 0 'foo' 'values' RAWDTW",
            Err(
                r"RAWDTW expects the distance to use in the DTW to be manhattan, euclidean, loxodromic or orthodromic.",
            ),
        ),
        (
            r"@A @B 0 'locations' RAWDTW",
            Err(
                r"RAWDTW failed. (RAWDTW expects GTSs to have locations when DTW is applied to them.)",
            ),
        ),
        (
            r"@A @B 0 'elevations' RAWDTW",
            Err(
                r"RAWDTW failed. (RAWDTW expects GTSs to have elevations when DTW is applied to them.)",
            ),
        ),
        (
            r"@A @B 0 'LOCATIONS' RAWDTW",
            Err(
                r"RAWDTW failed. (RAWDTW expects GTSs to have locations when DTW is applied to them.)",
            ),
        ),
        (r"@G1 @G2 0 'locations' RAWDTW", Ok(r"[0.4500001575797796]")),
        (r"@G1 @G2 0 'elevations' RAWDTW", Ok(r"[12.0]")),
        (
            r"@G1 @G2 0 'loxodromic' 'locations' RAWDTW",
            Ok(r"[30022.809284049312]"),
        ),
        (
            r"@G1 @G2 0 'orthodromic' 'locations' RAWDTW",
            Ok(r"[30022.806759348066]"),
        ),
        (
            r"@G1 @G2 0 'euclidean' 'locations' RAWDTW",
            Ok(r"[0.3532248820657328]"),
        ),
        (
            r"@G1 @G2 0 'loxodromic' 'values' RAWDTW",
            Err(r"RAWDTW failed. (Loxodromic distance must be given two dimensions: lat, lon.)"),
        ),
        (
            r"@G1 @G2 0 'orthodromic' 'elevations' RAWDTW",
            Err(r"RAWDTW failed. (Orthodromic distance must be given two dimensions: lat, lon.)"),
        ),
        (
            r"@G1 @G2 0 'squaredeuclidean' 'elevations' RAWDTW",
            Ok(r"[42.0]"),
        ),
        (
            r"@G1 @G2 2 0.5 'loxodromic' 'locations' RAWDTW",
            Ok(r"[-1.0]"),
        ),
        (
            r"@G1 @G3 0 'locations' RAWDTW",
            Err(
                r"RAWDTW failed. (RAWDTW expects GTSs to have locations when DTW is applied to them.)",
            ),
        ),
        (
            r"@G1 @G3 0 'elevations' RAWDTW",
            Err(
                r"RAWDTW failed. (RAWDTW expects GTSs to have elevations when DTW is applied to them.)",
            ),
        ),
        (
            r"@G3 @G1 0 'locations' RAWDTW",
            Err(
                r"RAWDTW failed. (RAWDTW expects GTSs to have locations when DTW is applied to them.)",
            ),
        ),
        (
            r"@G1 @G4 0 'elevations' RAWDTW",
            Err(
                r"RAWDTW failed. (RAWDTW expects GTSs to have elevations when DTW is applied to them.)",
            ),
        ),
        (
            r"@G1 @A 0 'locations' RAWDTW",
            Err(
                r"RAWDTW failed. (RAWDTW expects GTSs to have locations when DTW is applied to them.)",
            ),
        ),
        (
            r"@A @G1 0 'elevations' RAWDTW",
            Err(
                r"RAWDTW failed. (RAWDTW expects GTSs to have elevations when DTW is applied to them.)",
            ),
        ),
        (
            r"@K @A 0 DTW",
            Err(r"DTW failed. (DTW cannot normalize a constant GTS.)"),
        ),
        (
            r"@A @K 0 DTW",
            Err(r"DTW failed. (DTW cannot normalize a constant GTS.)"),
        ),
        (r"@K @A 0 RAWDTW", Ok(r"[28.675]")),
        (r"@K @A 0 ZDTW", Ok(r"[Infinity]")),
        (
            r"@S @A 0 DTW",
            Err(r"DTW failed. (DTW can only operate on numerical Geo Time Series.)"),
        ),
        (
            r"@A @S 0 DTW",
            Err(r"DTW failed. (DTW can only operate on numerical Geo Time Series.)"),
        ),
        (
            r"@E @A 0 DTW",
            Err(r"DTW failed. (DTW can only operate on numerical Geo Time Series.)"),
        ),
        (r"@E @A 0 'timestamps' RAWDTW", Err(r"NullPointerException")),
        (r"@E @E 0 'timestamps' RAWDTW", Err(r"NullPointerException")),
        (r"@E @A 0 'timestamps' ZDTW", Err(r"NullPointerException")),
        (r"@E @A 0 'timestamps' DTW", Err(r"NullPointerException")),
        (r"@E @E 0 'timestamps' DTW", Err(r"NullPointerException")),
        (r"@A @E 0 'timestamps' RAWDTW", Err(r"NullPointerException")),
        (r"'values' DTW", Err(r"Empty stack.")),
        (r"'manhattan' 'values' DTW", Err(r"Empty stack.")),
        (r"5 1.0 DTW", Err(r"Empty stack.")),
        (r"@A 5 1.0 DTW", Err(r"Empty stack.")),
        (
            r"@A @B 'x' DTW",
            Err(
                r"DTW expects the characteristic of the GTS to compute the DTW on to be values, locations, elevations or timestamps.",
            ),
        ),
        (
            r"@A @B DTW",
            Err(r"DTW expects a numeric threshold on top of the stack."),
        ),
        (r"@A 0 DTW", Err(r"Empty stack.")),
        (
            r"1 @B 0 DTW",
            Err(r"DTW expects two Geo Time Series below the threshold."),
        ),
        (r"@A 1 0 DTW", Err(r"Empty stack.")),
        (r"DTW", Err(r"Empty stack.")),
        (r"0 DTW", Err(r"Empty stack.")),
        (r"@A @B 0 NaN RAWDTW", Ok(r"[-1.0]")),
        (r"@A @B NaN RAWDTW", Ok(r"[-1.0]")),
        (
            r"@A @B 2.5 0 RAWDTW",
            Err(r"RAWDTW expects two Geo Time Series below the threshold."),
        ),
        (r"@A @B 0 1e-9 RAWDTW", Ok(r"[-1.0]")),
        (
            r"@A @B 0 DTW DROP @B",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[2,2.55],[4,3.156],[6,3.039],[8,2.226],[10,0.918],[12,-0.567],[14,-1.863],[16,-2.655],[18,-2.747]]}]"#,
            ),
        ),
        (
            r"@R @A 0 RAWDTW DROP @R",
            Ok(r#"[{"c":"r","l":{},"a":{},"la":0,"v":[[5,7.5],[4,6.0],[3,4.5],[2,3.0],[1,1.5]]}]"#),
        ),
        (
            r"@A @B 0 true RAWDTW",
            Err(r"RAWDTW expects a numeric threshold on top of the stack."),
        ),
        (
            r"@G1 @G2 0 'loxodromic' 'timestamps' RAWDTW",
            Err(r"RAWDTW failed. (Loxodromic distance must be given two dimensions: lat, lon.)"),
        ),
        (
            r"@E @G2 0 'loxodromic' 'timestamps' RAWDTW",
            Err(r"NullPointerException"),
        ),
    ]);
}

#[test]
fn optdtw() {
    check(&[
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 0 OPTDTW",
            Ok(
                r"[[[1,1.2414095213926508],[20,1.6300888457357172],[0,3.014280209087686],[18,3.577497838498095],[19,3.7688790494367987],[17,4.160966278841997],[16,4.605721784026222],[13,4.636026173770259],[15,4.754778280871807],[14,5.0114825160716565],[32,5.0389620951818435],[2,5.700676760888773],[21,5.956836758853435],[12,6.028178378046852],[31,7.125641167975732],[3,7.524305124718532],[22,8.551360786249997],[23,12.214511987331093],[4,12.36662972075093],[11,13.08627290463072],[5,13.711232473473583],[24,13.79043540109264],[30,14.558796036886216],[6,15.744144101569262],[25,16.060359395181223],[26,16.152670685277453],[7,16.43237045596246],[10,17.136278371332647],[29,17.154271483142267],[28,17.966160718453686],[27,18.418950868800046],[9,19.089992740459973],[8,19.164303754828445]]]",
            ),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 1 OPTDTW",
            Ok(r"[[[1,1.2414095213926508]]]"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 2 OPTDTW",
            Ok(r"[[[1,1.2414095213926508],[20,1.6300888457357172]]]"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 3 OPTDTW",
            Ok(r"[[[1,1.2414095213926508],[20,1.6300888457357172],[0,3.014280209087686]]]"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 5 OPTDTW",
            Ok(
                r"[[[1,1.2414095213926508],[20,1.6300888457357172],[0,3.014280209087686],[18,3.577497838498095],[19,3.7688790494367987]]]",
            ),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 10 OPTDTW",
            Ok(
                r"[[[1,1.2414095213926508],[20,1.6300888457357172],[0,3.014280209087686],[18,3.577497838498095],[19,3.7688790494367987],[17,4.160966278841997],[16,4.605721784026222],[13,4.636026173770259],[15,4.754778280871807],[14,5.0114825160716565]]]",
            ),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 100 OPTDTW",
            Ok(
                r"[[[1,1.2414095213926508],[20,1.6300888457357172],[0,3.014280209087686],[18,3.577497838498095],[19,3.7688790494367987],[17,4.160966278841997],[16,4.605721784026222],[13,4.636026173770259],[15,4.754778280871807],[14,5.0114825160716565],[32,5.0389620951818435],[2,5.700676760888773],[21,5.956836758853435],[12,6.028178378046852],[31,7.125641167975732],[3,7.524305124718532],[22,8.551360786249997],[23,12.214511987331093],[4,12.36662972075093],[11,13.08627290463072],[5,13.711232473473583],[24,13.79043540109264],[30,14.558796036886216],[6,15.744144101569262],[25,16.060359395181223],[26,16.152670685277453],[7,16.43237045596246],[10,17.136278371332647],[29,17.154271483142267],[28,17.966160718453686],[27,18.418950868800046],[9,19.089992740459973],[8,19.164303754828445]]]",
            ),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] -1 OPTDTW",
            Ok(
                r"[[[1,1.2414095213926508],[20,1.6300888457357172],[0,3.014280209087686],[18,3.577497838498095],[19,3.7688790494367987],[17,4.160966278841997],[16,4.605721784026222],[13,4.636026173770259],[15,4.754778280871807],[14,5.0114825160716565],[32,5.0389620951818435],[2,5.700676760888773],[21,5.956836758853435],[12,6.028178378046852],[31,7.125641167975732],[3,7.524305124718532],[22,8.551360786249997],[23,12.214511987331093],[4,12.36662972075093],[11,13.08627290463072],[5,13.711232473473583],[24,13.79043540109264],[30,14.558796036886216],[6,15.744144101569262],[25,16.060359395181223],[26,16.152670685277453],[7,16.43237045596246],[10,17.136278371332647],[29,17.154271483142267],[28,17.966160718453686],[27,18.418950868800046],[9,19.089992740459973],[8,19.164303754828445]]]",
            ),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 0 3 OPTDTW",
            Ok(r"[[[1,1.8827503509056058],[20,2.4918828589584034],[0,3.0992589781627515]]]"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 1 3 OPTDTW",
            Ok(r"[[[1,1.2414095213926508],[20,1.6300888457357172],[0,3.014280209087686]]]"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 2 3 OPTDTW",
            Ok(r"[[[1,1.2414095213926508],[20,1.6300888457357172],[0,3.014280209087686]]]"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 4 3 OPTDTW",
            Ok(r"[[[1,1.2414095213926508],[20,1.6300888457357172],[0,3.014280209087686]]]"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] -1 3 OPTDTW",
            Ok(r"[[[1,1.2414095213926508],[20,1.6300888457357172],[0,3.014280209087686]]]"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 3.7 OPTDTW",
            Ok(r"[[[1,1.2414095213926508],[20,1.6300888457357172],[0,3.014280209087686]]]"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 'x' OPTDTW",
            Err(r"OPTDTW expects a count of best results to return on top of the stack."),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] 'x' 3 OPTDTW",
            Err(r"OPTDTW expects a numeric list to use as query below the count."),
        ),
        (
            r"'x' [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 3 OPTDTW",
            Err(
                r"OPTDTW expects a numeric list as the sequence in which to find best matches below the 'query' list.",
            ),
        ),
        (
            r"[ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] [ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] 3 OPTDTW",
            Err(r"OPTDTW expects the query list to be shorter than the sequence list."),
        ),
        (
            r"[ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 0 OPTDTW",
            Ok(r"[[[0,0.0]]]"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ ] 3 OPTDTW",
            Err(r"Index 0 out of bounds for length 0"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1 2 'x' ] 3 OPTDTW",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1 NULL ] 3 OPTDTW",
            Err(r"NullPointerException"),
        ),
        (
            r"[ 1 'x' 3 ] [ 1 2 ] 3 OPTDTW",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.5 ] 3 OPTDTW",
            Ok(r"[[]]"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1 2 ] 3 OPTDTW",
            Ok(r"[[[1,0.4390460201439095],[21,0.892079762870722],[19,0.9653148058004735]]]"),
        ),
        (
            r"[ 1 1 1 1 ] [ 1 2 ] 0 OPTDTW",
            Ok(r"[[[0,Infinity],[2,Infinity],[1,Infinity]]]"),
        ),
        (
            r"[ 0 1 0 1 0 1 ] [ 0 1 ] 0 OPTDTW",
            Ok(r"[[[0,0.0],[4,0.0],[2,0.0],[3,1.414213562373095],[1,1.414213562373095]]]"),
        ),
        (
            r"[ 0 1 0 1 0 1 ] [ 0 1 ] 2 OPTDTW",
            Ok(r"[[[0,0.0],[4,0.0]]]"),
        ),
        (r"OPTDTW", Err(r"Empty stack.")),
        (r"3 OPTDTW", Err(r"Empty stack.")),
        (r"5 3 OPTDTW", Err(r"Empty stack.")),
        (
            r"[ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 3 OPTDTW",
            Err(r"Empty stack."),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 4294967298 OPTDTW",
            Ok(r"[[[1,1.2414095213926508],[20,1.6300888457357172]]]"),
        ),
        (
            r"[ -0.262 1.68 2.962 4.311 4.985 4.543 4.06 3.953 2.046 0.44 -0.457 -2.536 -3.448 -4.669 -4.856 -5.144 -3.932 -2.523 -1.374 0.492 2.042 2.849 4.596 5.004 4.748 3.967 3.803 2.033 0.675 -0.819 -2.506 -3.522 -4.837 -4.699 -4.773 -3.48 -2.304 -1.557 0.137 1.818 ] [ 1.478 2.959 4.115 4.818 4.99 4.613 3.729 2.433 ] 2.5e10 OPTDTW",
            Ok(
                r"[[[1,1.2414095213926508],[20,1.6300888457357172],[0,3.014280209087686],[18,3.577497838498095],[19,3.7688790494367987],[17,4.160966278841997],[16,4.605721784026222],[13,4.636026173770259],[15,4.754778280871807],[14,5.0114825160716565],[32,5.0389620951818435],[2,5.700676760888773],[21,5.956836758853435],[12,6.028178378046852],[31,7.125641167975732],[3,7.524305124718532],[22,8.551360786249997],[23,12.214511987331093],[4,12.36662972075093],[11,13.08627290463072],[5,13.711232473473583],[24,13.79043540109264],[30,14.558796036886216],[6,15.744144101569262],[25,16.060359395181223],[26,16.152670685277453],[7,16.43237045596246],[10,17.136278371332647],[29,17.154271483142267],[28,17.966160718453686],[27,18.418950868800046],[9,19.089992740459973],[8,19.164303754828445]]]",
            ),
        ),
    ]);
}
