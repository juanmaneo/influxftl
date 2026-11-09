// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the quaternion functions against the Java implementation.

use super::quaternions::{from_quaternion, to_quaternion};
use crate::tests::{assert_fails, assert_stack, assert_top};
use crate::value::Value;

fn doubles(values: &[f64]) -> Vec<Value> {
    values.iter().map(|&v| Value::Double(v)).collect()
}

#[test]
fn packing_matches_java() {
    assert_top("1.0 0.0 0.0 0.0 ->Q", -140_739_635_871_745);
    assert_top("0.0 0.0 0.0 0.0 ->Q", 9_223_231_297_218_904_063);
    assert_top("1.0 1.0 1.0 1.0 ->Q", -4_611_756_388_245_323_777);
    assert_top("0.5 -0.5 0.25 2.0 ->Q", -7_064_632_647_267_715_112);
    assert_top("-1.0 0.0 0.0 0.0 ->Q", 140_735_340_838_911);
    assert_eq!(to_quaternion(1.0, 0.0, 0.0, 0.0), -140_739_635_871_745);
    assert_stack(
        "0.5 -0.5 0.25 2.0 ->Q Q->",
        &doubles(&[
            0.234_058_136_873_426_47,
            -0.234_088_654_917_219_75,
            0.117_021_438_925_764_89,
            0.936_308_842_603_189_1,
        ]),
    );
    assert_stack(
        "1.0 0.0 0.0 0.0 ->Q Q->",
        &doubles(&[
            1.0,
            -1.525_902_189_669_636_8E-5,
            -1.525_902_189_669_636_8E-5,
            -1.525_902_189_669_636_8E-5,
        ]),
    );
    assert_fails(
        "0 Q->",
        "Q-> failed. (Input does not appear to be a unit quaternion, norm was 4.0.)",
    );
    assert_fails(
        "123456789 Q->",
        "Input does not appear to be a unit quaternion, norm was 3.2510496718649615.",
    );
    assert!(from_quaternion(0).is_err());
    assert_fails(
        "1.0 0.0 0.0 0 ->Q",
        "->Q expects the quaternion components w x y z as doubles on the stack (z on top).",
    );
    assert_fails(
        "1.0 0.0 0 0.0 ->Q",
        "->Q expects the quaternion components w x y z as doubles on the stack (z on top).",
    );
    assert_fails("1.0 NaN 0.0 0.0 ->Q", "->Q expects finite arguments.");
    assert_fails(
        "1.5 Q->",
        "Q-> expects a quaternion as a LONG on top of the stack.",
    );
    assert_fails("->Q", "Empty stack.");
    assert_fails("Q->", "Empty stack.");
}

#[test]
fn rotations_match_java() {
    const ROTATION: i64 = -2_701_737_566_104_724_863;
    assert_top("0.0 0.0 1.0 90.0 ROTATIONQ", ROTATION);
    assert_top(
        "0.0 0.0 0.0 90.0 ROTATIONQ",
        to_quaternion(45f64.to_radians().cos(), 0.0, 0.0, 0.0),
    );
    assert_stack(
        &format!("1.0 0.0 0.0 {ROTATION} QROTATE"),
        &doubles(&[
            5.364_139_350_871_877E-5,
            -0.999_946_357_675_140_3,
            -4.315_787_384_884_491_5E-5,
        ]),
    );
    assert_stack(
        &format!("{ROTATION} QROTATION"),
        &doubles(&[
            -2.157_893_695_546_860_4E-5,
            -2.157_893_695_546_860_4E-5,
            0.999_946_359_579_463_1,
            90.003_073_452_137_97,
        ]),
    );
    // The identity has a zero angle: the axis is left as it is.
    assert_stack(
        "1.0 0.0 0.0 0.0 ->Q QROTATION",
        &doubles(&[
            -1.525_902_189_669_636_8E-5,
            -1.525_902_189_669_636_8E-5,
            -1.525_902_189_669_636_8E-5,
            0.0,
        ]),
    );
    assert_top(
        &format!("{ROTATION} {ROTATION} QMULTIPLY"),
        9_223_231_292_923_903_998,
    );
    assert_top(
        &format!("{ROTATION} {ROTATION} QDIVIDE"),
        -140_739_635_871_745,
    );
    assert_top(
        &format!("{ROTATION} QCONJUGATE"),
        -2_701_737_561_809_738_371,
    );
    assert_fails(
        "0.0 0.0 1.0 90 ROTATIONQ",
        "ROTATIONQ expects a vector coordinates and an angle in degrees on the stack.",
    );
    assert_fails(
        "1.0 0.0 0.0 1.5 QROTATE",
        "QROTATE expects a quaternion on top of the stack.",
    );
    assert_fails(
        &format!("1 0.0 0.0 {ROTATION} QROTATE"),
        "QROTATE operates on a vector whose x,y,z coordinates are doubles below the quaternion.",
    );
    assert_fails(
        "1.0 0.0 0.0 0 QROTATE",
        "Input does not appear to be a unit quaternion",
    );
    assert_fails(
        "1.5 QROTATION",
        "QROTATION can only operate on a quaternion.",
    );
    assert_fails(
        "1.5 QCONJUGATE",
        "QCONJUGATE can only operate on a quaternion.",
    );
    assert_fails(
        &format!("{ROTATION} 1.5 QMULTIPLY"),
        "QMULTIPLY expects a quaternion on top of the stack.",
    );
    assert_fails(
        &format!("1.5 {ROTATION} QMULTIPLY"),
        "QMULTIPLY expects another quaternion below the quaternion on top of the stack.",
    );
    assert_fails(
        &format!("{ROTATION} 1.5 QDIVIDE"),
        "QDIVIDE expects a quaternion on top of the stack.",
    );
    assert_fails(
        &format!("1.5 {ROTATION} QDIVIDE"),
        "QDIVIDE expects another quaternion below the quaternion on top of the stack.",
    );
    for f in [
        "QROTATE",
        "QROTATION",
        "QCONJUGATE",
        "QMULTIPLY",
        "QDIVIDE",
        "ROTATIONQ",
    ] {
        assert_fails(f, "Empty stack.");
    }
    assert_fails(&format!("{ROTATION} QROTATE"), "Empty stack.");
    assert_fails(&format!("1.0 {ROTATION} QROTATE"), "Empty stack.");
    assert_fails(&format!("1.0 1.0 {ROTATION} QROTATE"), "Empty stack.");
}
