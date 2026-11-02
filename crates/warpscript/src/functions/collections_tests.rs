// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use indexmap::IndexMap;

use super::collections::{labels_id, set_at, siphash24, siphash24_palindromic};
use crate::encoding::hex_decode;
use crate::stack::{Limits, Stack};
use crate::tests::{assert_fails, assert_stack, assert_top, exec, run, top};
use crate::value::Value;

fn bytes(hex: &str) -> Value {
    Value::bytes(hex_decode(hex).unwrap())
}

fn longs(items: &[i64]) -> Value {
    Value::list(items.iter().copied().map(Value::Long).collect())
}

/// Java `toString()` of the top of the stack, which shows the order of maps and sets.
fn show(script: &str) -> String {
    top(script).to_string()
}

/// Run `script` and return `(message of the error, …)` for the empty-stack checks.
fn empty_stack(name: &str, operands: &[&str]) {
    for prefix in operands {
        assert_fails(&format!("{prefix} {name}"), "Empty stack.");
    }
}

// -----------------------------------------------------------------------------------------
// Conversions between collections and the stack
// -----------------------------------------------------------------------------------------

#[test]
fn listto_and_unlist() {
    assert_stack(
        "[ 1 'a' ] LIST->",
        &[Value::Long(1), Value::from("a"), Value::Long(2)],
    );
    assert_stack("[ ] LIST->", &[Value::Long(0)]);
    assert_fails("'a' LIST->", "LIST-> operates on a list.");
    assert_stack(
        "[ 1 2 ] UNLIST",
        &[Value::Mark, Value::Long(1), Value::Long(2)],
    );
    assert_stack("[ 1 2 ] UNLIST ] ", &[longs(&[1, 2])]);
    assert_fails("1 UNLIST", "UNLIST expects a LIST.");
    empty_stack("LIST->", &[""]);
    empty_stack("UNLIST", &[""]);
}

#[test]
fn set_conversions() {
    assert_stack(
        "( 1 2 ) UNSET",
        &[Value::Mark, Value::Long(1), Value::Long(2)],
    );
    assert_fails("[ ] UNSET", "UNSET expects a SET.");
    assert_top("( 1 2 2 ) SET->", longs(&[1, 2]));
    assert_eq!(show("( 'b' 'a' ) SET->"), "[b, a]");
    assert_fails("[ ] SET->", "SET-> operates on a SET.");
    // Vectors are plain lists: V-> copies the list.
    let mut stack = exec("[ 1 2 ] DUP V->");
    let copy = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert_eq!(copy, longs(&[1, 2]));
    assert!(!copy.same_object(&original));
    assert_fails("1 V->", "V-> expects a vector on top of the stack.");
    empty_stack("UNSET", &[""]);
    empty_stack("SET->", &[""]);
    empty_stack("V->", &[""]);
}

#[test]
fn map_conversions() {
    assert_stack(
        "{ 'a' 1 'b' 2 } MAP->",
        &[
            Value::from("a"),
            Value::Long(1),
            Value::from("b"),
            Value::Long(2),
            Value::Long(4),
        ],
    );
    assert_stack("{ } MAP->", &[Value::Long(0)]);
    assert_fails("1 MAP->", "Invalid type, expected a map.");
    assert_stack(
        "{ 'a' 1 } UNMAP",
        &[Value::Mark, Value::from("a"), Value::Long(1)],
    );
    assert_eq!(show("{ 'a' 1 'b' 2 } UNMAP }"), "{a=1, b=2}");
    assert_fails("1 UNMAP", "UNMAP expects a MAP.");
    empty_stack("MAP->", &[""]);
    empty_stack("UNMAP", &[""]);
}

// -----------------------------------------------------------------------------------------
// MAPID
// -----------------------------------------------------------------------------------------

#[test]
fn siphash_vectors() {
    // Reference vector of SipHash-2-4: key 00..0f, message 00..0e.
    let key0 = u64::from_le_bytes([0, 1, 2, 3, 4, 5, 6, 7]);
    let key1 = u64::from_le_bytes([8, 9, 10, 11, 12, 13, 14, 15]);
    let message: Vec<u8> = (0..15).collect();
    assert_eq!(siphash24(key0, key1, &message), 0xa129_ca61_49be_45e5);
    // SipHashInline.hash24_palindromic on JDK 27 with the first MAPID key.
    let key = (0x39A9_DD7D_71B6_4E3C, 0xA14C_3749_DCAA_B408);
    assert_eq!(
        siphash24_palindromic(key, b"") as i64,
        -2_515_513_097_427_541_650
    );
    assert_eq!(
        siphash24_palindromic(key, b"abc") as i64,
        -8_581_497_351_342_100_984
    );
    assert_eq!(labels_id(key, &IndexMap::new()), 0xdd17_1a2a_67fd_216e);
}

#[test]
fn mapid_vectors() {
    // Reference values from GTSHelper.labelsId / MAPID on JDK 27.
    assert_top("{ } MAPID", "dd171a2a-67fd-216e-4720-302fd6845bd7");
    assert_top("{ 'a' 'b' } MAPID", "b29f0eda-5b3d-b6d5-3dcc-9d5a9388c61a");
    assert_top(
        "{ 'a' 'b' 'c' 1 } MAPID",
        "d277b560-85e4-ecd8-a279-90748e82b8fc",
    );
    assert_top(
        "{ 'c' 1 'a' 'b' } MAPID",
        "d277b560-85e4-ecd8-a279-90748e82b8fc",
    );
    // Keys and values go through toString(): 1 and '1' collapse into one label.
    assert_top(
        "{ 'c' '1' 'a' 'b' } MAPID",
        "d277b560-85e4-ecd8-a279-90748e82b8fc",
    );
    assert_top(
        "{ 1 2.5 true 'é€😀' 'x' [ 1 2 ] } MAPID",
        "6ab9af30-e93b-6b77-3703-dcaed8b565fb",
    );
    assert_top(
        "{ 'host' 'server1' 'dc' 'eu-west' 'rack' 'r12' } MAPID",
        "066ae0cd-42f2-39f5-c781-ec8d5aff7c39",
    );
    assert_top(
        "{ 'averyveryverylongkeyname_exceeding_sixteen_bytes' 'an_equally_long_value_that_exceeds_eight_bytes_for_sure' } MAPID",
        "74d67735-6d10-772a-6361-01df427520a1",
    );
    assert_fails("1 MAPID", "MAPID can only operate on a map.");
    assert_fails(
        "{ NULL 1 } MAPID",
        "MAPID cannot operate on a map with NULL keys or values.",
    );
    assert_fails(
        "{ 'a' NULL } MAPID",
        "MAPID cannot operate on a map with NULL keys or values.",
    );
    empty_stack("MAPID", &[""]);
}

// -----------------------------------------------------------------------------------------
// Set algebra
// -----------------------------------------------------------------------------------------

#[test]
fn set_algebra() {
    assert_eq!(show("( 1 2 3 ) ( 2 3 4 ) UNION SET->"), "[2, 3, 4, 1]");
    assert_eq!(show("( 1 2 3 ) ( 2 3 4 ) INTERSECTION SET->"), "[2, 3]");
    assert_eq!(show("( 1 2 3 ) ( 2 3 4 ) DIFFERENCE SET->"), "[1]");
    assert_eq!(show("( 1 2 ) DUP DIFFERENCE SET->"), "[]");
    assert_eq!(show("( ) ( 1 ) UNION SET->"), "[1]");
    // 1 and 1.0 are different elements.
    assert_eq!(show("( 1 ) ( 1.0 ) INTERSECTION SET->"), "[]");
    for name in ["UNION", "INTERSECTION", "DIFFERENCE"] {
        let message = format!("{name} can only operate on sets.");
        assert_fails(&format!("( ) 1 {name}"), &message);
        assert_fails(&format!("1 ( ) {name}"), &message);
        assert_fails(&format!("[ ] ( ) {name}"), &message);
        empty_stack(name, &["", "( )"]);
    }
}

// -----------------------------------------------------------------------------------------
// GET, SET, PUT
// -----------------------------------------------------------------------------------------

#[test]
fn get_on_lists_and_maps() {
    assert_top("[ 1 2 3 ] 1 GET", 2);
    assert_top("[ 1 2 3 ] -1 GET", 3);
    assert_top("[ 1 2 3 ] 4294967297 GET", 2);
    assert_fails(
        "[ 1 2 3 ] 3 GET",
        "GET failed. (Index out of bound, 3 >= 3)",
    );
    assert_fails(
        "[ 1 2 3 ] -4 GET",
        "GET failed. (Index out of bound, -4 < -3)",
    );
    assert_fails(
        "[ 1 2 3 ] 'a' GET",
        "GET failed. (Getting on LIST requires a LONG.)",
    );
    assert_top("{ 'a' 1 } 'a' GET", 1);
    assert_top("{ 'a' 1 } 'b' GET", Value::Null);
    assert_top("{ 1 'long' 1.0 'double' } 1.0 GET", "double");
    // A list key on a map is a plain key, not a path.
    assert_top("{ [ 1 ] 'x' } [ 1 ] GET", "x");
    // A list key on a list is a path.
    assert_top("[ [ 1 2 ] [ 3 4 ] ] [ 1 0 ] GET", 3);
    assert_top("[ { 'a' 5 } ] [ 0 'a' ] GET", 5);
    assert_top("[ 'ab' ] [ 0 1 ] GET", "b");
    assert_top("[ 1 2 ] [ ] GET", longs(&[1, 2]));
    assert_fails(
        "[ 1 ] [ 0 0 ] GET",
        "GET failed. (Invalid OBJECT to GET on.)",
    );
    assert_fails(
        "[ [ 1 ] ] [ 1 0 ] GET",
        "GET failed. (Index out of bound, 1 >= 1)",
    );
}

#[test]
fn get_on_strings_bytes_and_others() {
    assert_top("'héllo' 1 GET", "é");
    assert_top("'hello' -1 GET", "o");
    // UTF-16 indexing: the second unit of a surrogate pair is a lone surrogate.
    assert_top("'a😀' 1 GET", "\u{fffd}");
    assert_top("'a😀' 2 GET", "\u{fffd}");
    assert_fails(
        "'abc' 'x' GET",
        "GET failed. (Getting on STRING requires a LONG.)",
    );
    assert_fails("'abc' 3 GET", "GET failed. (Index out of bound, 3 >= 3)");
    assert_top("'ff00' HEX-> 0 GET", 255);
    assert_top("'ff00' HEX-> -1 GET", 0);
    assert_fails(
        "'ff' HEX-> 'x' GET",
        "GET failed. (Getting on BYTES requires a LONG.)",
    );
    assert_fails(
        "'ff' HEX-> 1 GET",
        "GET failed. (Index out of bound, 1 >= 1)",
    );
    assert_fails("1 0 GET", "GET failed. (Invalid OBJECT to GET on.)");
    empty_stack("GET", &["", "1"]);
}

#[test]
fn set_on_lists() {
    assert_eq!(show("[ 1 2 3 ] 9 1 SET"), "[1, 9, 3]");
    assert_eq!(show("[ 1 2 3 ] 9 -1 SET"), "[1, 2, 9]");
    // In place: the stored list is modified and stays on the stack.
    assert_stack(
        "[ 1 2 ] 'l' STORE $l 9 0 SET $l",
        &[longs(&[9, 2]), longs(&[9, 2])],
    );
    assert_fails("[ 1 2 3 ] 9 3 SET", "Index out of bound, 3 >= 3");
    assert_fails("[ 1 2 3 ] 9 -4 SET", "Index out of bound, -4 < -3");
    assert_fails(
        "[ 1 ] 9 'a' SET",
        "SET expects a key which is an LONG or a list of LONG.",
    );
    assert_fails("1 9 0 SET", "SET operates on a list or byte array.");
    assert_fails("{ } 9 0 SET", "SET operates on a list or byte array.");
    empty_stack("SET", &["", "1", "1 1"]);
    // The container stays on the stack when SET fails (Java only peeks it).
    let mut stack = exec("[ 1 2 ]");
    stack.push(Value::Long(9));
    stack.push(Value::Long(5));
    assert!(crate::functions::call(&mut stack, "SET").is_err());
    assert_eq!(stack.elements(), &[longs(&[1, 2])]);
    // Out of range indices are ignored by the (pre-validated) element replacement.
    let mut items = vec![1, 2];
    set_at(&mut items, 5, 9);
    set_at(&mut items, 0, 9);
    assert_eq!(items, vec![9, 2]);
}

#[test]
fn set_at_nested_path() {
    assert_eq!(show("[ [ 1 2 ] 3 ] 9 [ 0 1 ] SET"), "[[1, 9], 3]");
    assert_eq!(show("[ [ 1 2 ] 3 ] 9 [ -2 1 ] SET"), "[[1, 9], 3]");
    assert_eq!(show("[ 1 2 ] 9 [ 1 ] SET"), "[1, 9]");
    assert_eq!(show("[ [ [ 1 ] ] ] 9 [ 0 0 0 ] SET"), "[[[9]]]");
    let nested =
        "SET tried to set an element at a nested path that does not exist in the input list.";
    assert_fails("[ [ 1 2 ] 3 ] 9 [ 1 0 ] SET", nested);
    assert_fails("[ [ 1 2 ] 3 ] 9 [ 1 0 0 ] SET", nested);
    assert_fails("[ [ 1 2 ] ] 9 [ 5 0 ] SET", nested);
    assert_fails("[ [ 1 2 ] ] 9 [ -3 0 ] SET", nested);
    // The last index does not support negative values.
    assert_fails(
        "[ [ 1 2 ] ] 9 [ 0 2 ] SET",
        "Index 2 out of bounds for length 2",
    );
    assert_fails(
        "[ [ 1 2 ] ] 9 [ 0 -1 ] SET",
        "Index -1 out of bounds for length 2",
    );
    assert_fails("[ 1 2 ] 9 [ ] SET", "Index -1 out of bounds for length 0");
    assert_fails(
        "[ 1 2 ] 9 [ 'a' ] SET",
        "SET expects the key to be an integer or a list of integers when operating on a List.",
    );
    assert_fails(
        "'00' HEX-> 1 [ 'a' ] SET",
        "SET expects the key to be an integer or a list of integers when operating on a List.",
    );
    assert_fails(
        "'00' HEX-> 1 [ ] SET",
        "Index -1 out of bounds for length 0",
    );
    assert_fails(
        "'00' HEX-> 1 [ 0 ] SET",
        "SET expects a LONG key when operating on a byte array.",
    );
}

#[test]
fn set_on_bytes() {
    assert_top("'0000' HEX-> 255 1 SET", bytes("00ff"));
    assert_top("'0000' HEX-> -128 0 SET", bytes("8000"));
    assert_top("'0000' HEX-> 127 -1 SET", bytes("007f"));
    assert_eq!(run("'0000' HEX-> 1 0 SET").len(), 1);
    let range = "SET expects the element to be a LONG in either [0,255] or [-128,127], when operating on a byte array.";
    assert_fails("'00' HEX-> 256 0 SET", range);
    assert_fails("'00' HEX-> -129 0 SET", range);
    assert_fails("'00' HEX-> 'x' 0 SET", range);
    // The value is checked before the index.
    assert_fails("'00' HEX-> 256 5 SET", range);
    assert_fails("'00' HEX-> 1 5 SET", "Index out of bound, 5 >= 1");
    assert_fails(
        "'00' HEX-> 1 'k' SET",
        "SET expects a key which is an LONG or a list of LONG.",
    );
}

#[test]
fn put_on_maps_and_lists() {
    assert_eq!(show("{ } 1 'a' PUT"), "{a=1}");
    assert_eq!(show("{ 'a' 1 'b' 2 } 3 'a' PUT"), "{a=3, b=2}");
    assert_eq!(show("[ 1 2 ] 9 0 PUT"), "[9, 2]");
    assert_eq!(show("[ 1 2 ] 9 4294967297 PUT"), "[1, 9]");
    assert_stack(
        "{ } 'm' STORE $m 1 'a' PUT $m",
        &[top("{ 'a' 1 }"), top("{ 'a' 1 }")],
    );
    assert_fails("[ 1 2 ] 9 -1 PUT", "Index -1 out of bounds for length 2");
    assert_fails("[ 1 2 ] 9 2 PUT", "Index 2 out of bounds for length 2");
    assert_fails(
        "[ 1 ] 9 'a' PUT",
        "PUT expects a key which is an integer when operating on a list.",
    );
    assert_fails("1 2 3 PUT", "PUT operates on a map or list.");
    assert_fails("( ) 2 3 PUT", "PUT operates on a map or list.");
    empty_stack("PUT", &["", "1", "1 1"]);
}

// -----------------------------------------------------------------------------------------
// SUBMAP, SUBLIST, KEYLIST, VALUELIST
// -----------------------------------------------------------------------------------------

#[test]
fn submap_keylist_valuelist() {
    assert_eq!(
        show("{ 'a' 1 'b' 2 'c' 3 } [ 'c' 'a' 'z' ] SUBMAP"),
        "{a=1, c=3}"
    );
    assert_eq!(show("{ 'a' 1 } [ ] SUBMAP"), "{}");
    assert_eq!(show("{ 1 'x' } [ 1.0 ] SUBMAP"), "{}");
    // A new map is built.
    let mut stack = exec("{ 'a' 1 } DUP [ 'a' ] SUBMAP");
    let sub = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert_eq!(sub, original);
    assert!(!sub.same_object(&original));
    assert_fails("1 [ ] SUBMAP", "SUBMAP operates on a map.");
    assert_fails(
        "{ } 1 SUBMAP",
        "SUBMAP expects a list of map keys on top of the stack.",
    );
    empty_stack("SUBMAP", &["", "[ ]"]);
    assert_eq!(show("{ 'b' 1 'a' 2 } KEYLIST"), "[b, a]");
    assert_eq!(show("{ 'b' 1 'a' 2 } VALUELIST"), "[1, 2]");
    assert_eq!(show("{ } KEYLIST"), "[]");
    assert_fails("1 KEYLIST", "KEYLIST operates on a map.");
    assert_fails("1 VALUELIST", "VALUELIST operates on a map.");
    empty_stack("KEYLIST", &[""]);
    empty_stack("VALUELIST", &[""]);
}

/// `SUBLIST` applied to the list `[ 0 1 2 3 4 5 6 7 ]`.
fn sub(args: &str) -> String {
    show(&format!("[ 0 1 2 3 4 5 6 7 ] {args} SUBLIST"))
}

#[test]
fn sublist_with_index_list() {
    // Reference results from SUBLIST on JDK 27 (see /tmp/jref/coll/Sub.java).
    assert_eq!(sub("[ 1 3 ]"), "[1, 2, 3]");
    assert_eq!(sub("[ 3 1 ]"), "[1, 2, 3]");
    assert_eq!(sub("[ -3 -1 ]"), "[5, 6, 7]");
    assert_eq!(sub("[ -1 -3 ]"), "[5, 6, 7]");
    assert_eq!(sub("[ 5 100 ]"), "[5, 6, 7]");
    assert_eq!(sub("[ -100 2 ]"), "[0, 1, 2]");
    assert_eq!(sub("[ -100 -50 ]"), "[]");
    assert_eq!(sub("[ 8 9 ]"), "[]");
    assert_eq!(sub("[ 8 7 ]"), "[7]");
    assert_eq!(sub("[ 0 0 ]"), "[0]");
    assert_eq!(sub("[ 2 4294967297 ]"), "[1, 2]");
    assert_eq!(sub("[ ]"), "[]");
    assert_eq!(sub("[ 2 ]"), "[2]");
    assert_eq!(sub("[ -1 ]"), "[7]");
    assert_eq!(sub("[ -8 ]"), "[0]");
    assert_eq!(sub("[ 1 1 1 ]"), "[1, 1, 1]");
    assert_eq!(sub("[ 7 0 3 ]"), "[7, 0, 3]");
    assert_eq!(sub("[ 4294967297 1 1 ]"), "[1, 1, 1]");
    assert_eq!(show("[ ] [ 0 1 ] SUBLIST"), "[]");
    assert_eq!(show("[ ] [ ] SUBLIST"), "[]");
    let numeric = "SUBLIST expects a list of indices which are numeric integers.";
    assert_fails("[ 0 1 ] [ 1.0 3 ] SUBLIST", numeric);
    assert_fails("[ 0 1 2 3 4 5 6 7 ] [ 1 2 'a' ] SUBLIST", numeric);
    assert_fails("[ 0 1 ] [ 'a' ] SUBLIST", numeric);
    let oob = "SUBLIST reported an out of bound index.";
    assert_fails("[ 0 1 2 3 4 5 6 7 ] [ -9 ] SUBLIST", oob);
    assert_fails("[ 0 1 2 3 4 5 6 7 ] [ 8 ] SUBLIST", oob);
    assert_fails("[ ] [ 0 ] SUBLIST", oob);
    // The indices may be the list itself.
    assert_eq!(show("[ 1 0 2 ] DUP SUBLIST"), "[0, 1, 2]");
    // A new list is built.
    let mut stack = exec("[ 1 2 ] DUP [ 0 1 ] SUBLIST");
    let sub = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert_eq!(sub, original);
    assert!(!sub.same_object(&original));
}

#[test]
fn sublist_with_start_end_step() {
    assert_eq!(sub("2"), "[2, 3, 4, 5, 6, 7]");
    assert_eq!(sub("-2"), "[6, 7]");
    assert_eq!(sub("8"), "[]");
    assert_eq!(sub("100"), "[]");
    assert_eq!(sub("-8"), "[0, 1, 2, 3, 4, 5, 6, 7]");
    assert_eq!(sub("-9"), "[0, 1, 2, 3, 4, 5, 6, 7]");
    assert_eq!(sub("0"), "[0, 1, 2, 3, 4, 5, 6, 7]");
    assert_eq!(sub("7"), "[7]");
    assert_eq!(sub("2.9"), "[2, 3, 4, 5, 6, 7]");
    assert_eq!(show("[ ] 0 SUBLIST"), "[]");
    assert_eq!(show("[ ] -1 SUBLIST"), "[]");
    assert_eq!(show("[ ] 5 SUBLIST"), "[]");
    assert_eq!(sub("1 3"), "[1, 2, 3]");
    assert_eq!(sub("3 1"), "[3, 2, 1]");
    assert_eq!(sub("-3 -1"), "[5, 6, 7]");
    assert_eq!(sub("-1 -3"), "[7, 6, 5]");
    assert_eq!(sub("2 100"), "[2, 3, 4, 5, 6, 7]");
    assert_eq!(sub("100 2"), "[7, 6, 5, 4, 3, 2]");
    assert_eq!(sub("-100 2"), "[0, 1, 2]");
    assert_eq!(sub("2 -100"), "[2, 1, 0]");
    assert_eq!(sub("-100 -50"), "[]");
    assert_eq!(sub("8 9"), "[]");
    assert_eq!(sub("9 8"), "[]");
    assert_eq!(sub("0 0"), "[0]");
    assert_eq!(sub("-9 100"), "[0, 1, 2, 3, 4, 5, 6, 7]");
    assert_eq!(sub("100 -9"), "[7, 6, 5, 4, 3, 2, 1, 0]");
    assert_eq!(sub("0 7 2"), "[0, 2, 4, 6]");
    assert_eq!(sub("0 7 3"), "[0, 3, 6]");
    assert_eq!(sub("1 7 3"), "[1, 4, 7]");
    assert_eq!(sub("7 0 -3"), "[7, 4, 1]");
    assert_eq!(sub("7 0 -2"), "[7, 5, 3, 1]");
    assert_eq!(sub("-5 7 3"), "[3, 6]");
    assert_eq!(sub("-10 7 3"), "[1, 4, 7]");
    assert_eq!(sub("-10 -8 3"), "[]");
    assert_eq!(sub("10 0 -3"), "[7, 4, 1]");
    assert_eq!(sub("11 0 -3"), "[5, 2]");
    assert_eq!(sub("9 0 -3"), "[6, 3, 0]");
    assert_eq!(sub("0 100 3"), "[0, 3, 6]");
    assert_eq!(sub("100 0 -3"), "[7, 4, 1]");
    assert_eq!(sub("100 -100 -3"), "[7, 4, 1]");
    assert_eq!(sub("-100 100 3"), "[1, 4, 7]");
    assert_eq!(sub("0 7 100"), "[0]");
    assert_eq!(sub("7 0 -100"), "[7]");
    assert_eq!(sub("5 7 100"), "[5]");
    assert_eq!(sub("-3 7 100"), "[5]");
    assert_eq!(sub("20 0 -100"), "[]");
    assert_eq!(sub("0 7 MAXLONG"), "[0]");
    // `Math.addExact` overflows after the first element: the iteration stops.
    assert_eq!(sub("1 7 MAXLONG"), "[1]");
    assert_eq!(sub("2 7 MAXLONG"), "[2]");
    assert_eq!(sub("7 0 MINLONG"), "[7]");
    assert_eq!(sub("10 0 MINLONG"), "[]");
    assert_eq!(sub("7 0 -1"), "[7, 6, 5, 4, 3, 2, 1, 0]");
    assert_eq!(sub("0 7 1"), "[0, 1, 2, 3, 4, 5, 6, 7]");
    assert_eq!(sub("2 2 1"), "[2]");
    assert_eq!(sub("2 2 -1"), "[2]");
    assert_eq!(sub("MINLONG"), "[0, 1, 2, 3, 4, 5, 6, 7]");
    assert_eq!(sub("MINLONG MAXLONG"), "[0, 1, 2, 3, 4, 5, 6, 7]");
    assert_eq!(sub("MAXLONG MINLONG"), "[7, 6, 5, 4, 3, 2, 1, 0]");
    assert_eq!(sub("MINLONG MAXLONG 3"), "[0, 3, 6]");
    assert_eq!(sub("MAXLONG MINLONG -3"), "[7, 4, 1]");
    assert_eq!(sub("2.5 5.9 1.5"), "[2, 3, 4, 5]");
    assert_eq!(show("[ 'x' ] 0 0 1 SUBLIST"), "[x]");
    assert_eq!(show("[ 'x' ] -1 SUBLIST"), "[x]");
    assert_eq!(show("[ 'x' ] 0 0 -1 SUBLIST"), "[x]");
    // Only three numbers are consumed: a fourth operand must be the list.
    assert_stack(
        "99 [ 0 1 2 3 4 5 6 7 ] 0 7 3 SUBLIST",
        &[Value::Long(99), longs(&[0, 3, 6])],
    );
    assert_fails(
        "[ 0 1 2 3 4 5 6 7 ] 0 7 0 SUBLIST",
        "SUBLIST expects the step parameter to be a strictly positive or negative number.",
    );
    assert_fails(
        "[ 0 1 2 3 4 5 6 7 ] 3 1 2 SUBLIST",
        "SUBLIST expects start to be before end when step is positive.",
    );
    assert_fails(
        "[ 0 1 2 3 4 5 6 7 ] 1 3 -2 SUBLIST",
        "SUBLIST expects start to be after end when step is negative.",
    );
    assert_fails(
        "[ 0 1 2 3 4 5 6 7 ] MINLONG 3 -3 SUBLIST",
        "SUBLIST expects start to be after end when step is negative.",
    );
    let operands = "SUBLIST expects a list of indices on top of the stack or a start end step and will operate on the list below it.";
    assert_fails("[ 0 1 ] 0 1 2 3 SUBLIST", operands);
    assert_fails("1 2 3 4 SUBLIST", operands);
    assert_fails("[ 0 1 ] 'a' SUBLIST", operands);
    assert_fails("[ 0 1 ] 1 'a' SUBLIST", operands);
    assert_fails(
        "'x' [ 1 2 ] SUBLIST",
        "SUBLIST expects a list of indices on top of the stack and will operate on the list below it.",
    );
    empty_stack("SUBLIST", &["", "1 2 3", "[ 1 ]"]);
}

// -----------------------------------------------------------------------------------------
// SIZE, SHRINK, REMOVE, INDEXOF
// -----------------------------------------------------------------------------------------

#[test]
fn size_of_everything() {
    assert_top("{ 'a' 1 'b' 2 } SIZE", 2);
    assert_top("[ 1 2 3 ] SIZE", 3);
    assert_top("( 1 1 2 ) SIZE", 2);
    assert_top("'héllo' SIZE", 5);
    // UTF-16 code units.
    assert_top("'a😀' SIZE", 3);
    assert_top("'' SIZE", 0);
    assert_top("'ff00' HEX-> SIZE", 2);
    assert_top("<% 1 2 + %> SIZE", 3);
    assert_top("<% %> SIZE", 0);
    assert_fails(
        "1 SIZE",
        "SIZE operates on a MAP, a collection, a STRING, a byte array, a GTS or GTS Encoder, a GEOSHAPE or a Macro.",
    );
    empty_stack("SIZE", &[""]);
}

#[test]
fn shrink_only_handles_nested_empty_lists() {
    assert_eq!(show("[ ] 2 SHRINK"), "[]");
    assert_eq!(show("[ [ ] [ [ ] ] ] -1 SHRINK"), "[[], [[]]]");
    let mut stack = exec("[ [ ] ] DUP 1 SHRINK");
    let shrunk = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert!(!shrunk.same_object(&original));
    assert_fails(
        "[ 1 ] 1 SHRINK",
        "SHRINK can only handle GTS and list thereof.",
    );
    assert_fails("1 1 SHRINK", "SHRINK can only handle GTS and list thereof.");
    assert_fails(
        "[ ] 'a' SHRINK",
        "SHRINK expects a size on top of the stack.",
    );
    empty_stack("SHRINK", &["", "1"]);
}

#[test]
fn remove_from_maps_lists_and_sets() {
    assert_stack(
        "{ 'a' 1 'b' 2 } 'a' REMOVE",
        &[top("{ 'b' 2 }"), Value::Long(1)],
    );
    assert_stack("{ 'a' 1 } 'z' REMOVE", &[top("{ 'a' 1 }"), Value::Null]);
    assert_eq!(show("{ 'a' 1 'b' 2 'c' 3 } 'b' REMOVE DROP"), "{a=1, c=3}");
    assert_stack("[ 1 2 3 ] 1 REMOVE", &[longs(&[1, 3]), Value::Long(2)]);
    assert_stack("[ 1 2 3 ] -1 REMOVE", &[longs(&[1, 2]), Value::Long(3)]);
    assert_stack(
        "[ 1 2 3 ] 4294967297 REMOVE",
        &[longs(&[1, 3]), Value::Long(2)],
    );
    assert_stack("[ 1 2 3 ] 5 REMOVE", &[longs(&[1, 2, 3]), Value::Null]);
    assert_stack("[ 1 2 3 ] -4 REMOVE", &[longs(&[1, 2, 3]), Value::Null]);
    // In place.
    assert_stack(
        "[ 1 2 ] 'l' STORE $l 0 REMOVE DROP $l",
        &[longs(&[2]), longs(&[2])],
    );
    assert_fails("[ 1 ] 'a' REMOVE", "REMOVE expects an integer as key.");
    assert_fails("[ 1 ] 1.0 REMOVE", "REMOVE expects an integer as key.");
    assert_eq!(
        run("( 1 2 ) 1 REMOVE")
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["[2]", "1"]
    );
    assert_eq!(
        run("( 1 ) 5 REMOVE")
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["[1]", "null"]
    );
    assert_fails("1 1 REMOVE", "REMOVE operates on a map, a list or a GTS.");
    empty_stack("REMOVE", &["", "1"]);
}

#[test]
fn remove_from_bytes_and_strings() {
    assert_stack("'010203' HEX-> 1 REMOVE", &[bytes("0103"), Value::Long(2)]);
    assert_stack(
        "'0102ff' HEX-> -1 REMOVE",
        &[bytes("0102"), Value::Long(255)],
    );
    assert_stack("'0102' HEX-> 2 REMOVE", &[bytes("0102"), Value::Null]);
    assert_fails("'01' HEX-> 'a' REMOVE", "REMOVE expects an integer as key.");
    assert_stack("'abc' 1 REMOVE", &[Value::from("ac"), Value::from("b")]);
    assert_stack("'abc' -3 REMOVE", &[Value::from("bc"), Value::from("a")]);
    assert_stack("'abc' 5 REMOVE", &[Value::from("abc"), Value::Null]);
    assert_stack("'' 0 REMOVE", &[Value::from(""), Value::Null]);
    // UTF-16 units: removing half of a surrogate pair leaves U+FFFD.
    assert_stack(
        "'a😀' 1 REMOVE",
        &[Value::from("a\u{fffd}"), Value::from("\u{fffd}")],
    );
    assert_fails("'abc' 'a' REMOVE", "REMOVE expects an integer as key.");
}

#[test]
fn indexof_in_lists_and_maps() {
    assert_top("[ 1 2 1 ] 1 0 INDEXOF", longs(&[0, 2]));
    assert_top("[ 1 2 1 ] 1 -5 INDEXOF", longs(&[0, 2]));
    assert_top("[ 1 2 1 ] 1 1 INDEXOF", longs(&[0]));
    assert_top("[ 1 2 1 ] 1.0 0 INDEXOF", longs(&[]));
    assert_top("[ NULL 1 NULL ] NULL 0 INDEXOF", longs(&[0, 2]));
    assert_top("[ [ 1 ] [ 2 ] ] [ 2 ] 0 INDEXOF", longs(&[1]));
    assert_eq!(show("{ 'a' 1 'b' 2 'c' 1 } 1 0 INDEXOF"), "[a, c]");
    assert_eq!(show("{ 'a' 1 'b' 2 'c' 1 } 1 1 INDEXOF"), "[a]");
    assert_eq!(show("{ 'a' 1 } 3 0 INDEXOF"), "[]");
    assert_fails(
        "[ ] 1 'a' INDEXOF",
        "INDEXOF expects a LONG number of indexes to return. 0 to return all indexes.",
    );
    assert_fails(
        "1 1 0 INDEXOF",
        "INDEXOF expects a LIST, MAP, STRING, GTS or BYTES.",
    );
    empty_stack("INDEXOF", &["", "1", "1 1"]);
}

#[test]
fn indexof_in_strings_and_bytes() {
    assert_top("'abcabc' 'bc' 0 INDEXOF", longs(&[1, 4]));
    assert_top("'abcabc' 'bc' 1 INDEXOF", longs(&[1]));
    assert_top("'aaa' 'aa' 0 INDEXOF", longs(&[0, 1]));
    assert_top("'abc' '' 0 INDEXOF", longs(&[0, 1, 2, 3]));
    assert_top("'ab' 'abc' 0 INDEXOF", longs(&[]));
    assert_top("'' '' 0 INDEXOF", longs(&[0]));
    // UTF-16 units.
    assert_top("'😀a' 'a' 0 INDEXOF", longs(&[2]));
    assert_fails(
        "'abc' 1 0 INDEXOF",
        "INDEXOF must be given a STRING to look for in a STRING",
    );
    assert_top("'01020102' HEX-> 2 0 INDEXOF", longs(&[1, 3]));
    assert_top("'01020102' HEX-> 2 1 INDEXOF", longs(&[1]));
    assert_top("'01020102' HEX-> '0201' HEX-> 0 INDEXOF", longs(&[1]));
    assert_top("'ff' HEX-> 255 0 INDEXOF", longs(&[0]));
    assert_top("'ff' HEX-> '' HEX-> 0 INDEXOF", longs(&[0, 1]));
    assert_fails(
        "'0102' HEX-> 256 0 INDEXOF",
        "INDEXOF expects the LONG to look for to have only its least significant byte set when looking into BYTES.",
    );
    assert_fails(
        "'0102' HEX-> -1 0 INDEXOF",
        "INDEXOF expects the LONG to look for to have only its least significant byte set when looking into BYTES.",
    );
    assert_fails(
        "'0102' HEX-> 'x' 0 INDEXOF",
        "INDEXOF must be given a BYTES to look for in a BYTES.",
    );
}

// -----------------------------------------------------------------------------------------
// UNIQUE, CONTAINS, CONTAINSKEY, CONTAINSVALUE
// -----------------------------------------------------------------------------------------

#[test]
fn unique_and_contains() {
    assert_eq!(show("[ 1 2 1 3 2 ] UNIQUE"), "[1, 2, 3]");
    assert_eq!(show("[ 1 1.0 NULL NULL ] UNIQUE"), "[1, 1.0, null]");
    assert_eq!(show("[ ] UNIQUE"), "[]");
    assert_fails("1 UNIQUE", "UNIQUE operates on a list.");
    empty_stack("UNIQUE", &[""]);
    assert_stack(
        "[ 1 2 ] 2 CONTAINS",
        &[longs(&[1, 2]), Value::Boolean(true)],
    );
    assert_stack(
        "[ 1 2 ] 2.0 CONTAINS",
        &[longs(&[1, 2]), Value::Boolean(false)],
    );
    assert_eq!(
        run("( 1 2 ) 1 CONTAINS")
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["[1, 2]", "true"]
    );
    // The string is consumed.
    assert_stack("'abc' 'bc' CONTAINS", &[Value::Boolean(true)]);
    assert_stack("'abc' 'x' CONTAINS", &[Value::Boolean(false)]);
    assert_stack("'abc' '' CONTAINS", &[Value::Boolean(true)]);
    assert_fails(
        "'abc' 1 CONTAINS",
        "CONTAINS operates on a list, set or STRING.",
    );
    assert_fails(
        "{ } 1 CONTAINS",
        "CONTAINS operates on a list, set or STRING.",
    );
    empty_stack("CONTAINS", &["", "1"]);
    // The collection stays on the stack when CONTAINS fails (Java only peeks it).
    let mut stack = exec("'abc' 1");
    assert!(crate::functions::call(&mut stack, "CONTAINS").is_err());
    assert_eq!(stack.elements(), &[Value::from("abc")]);
}

#[test]
fn containskey_and_containsvalue() {
    assert_stack(
        "{ 'a' 1 } 'a' CONTAINSKEY",
        &[top("{ 'a' 1 }"), Value::Boolean(true)],
    );
    assert_stack(
        "{ 'a' 1 } 'b' CONTAINSKEY",
        &[top("{ 'a' 1 }"), Value::Boolean(false)],
    );
    assert_stack(
        "{ 'a' 1 } 1 CONTAINSVALUE",
        &[top("{ 'a' 1 }"), Value::Boolean(true)],
    );
    assert_stack(
        "{ 'a' 1 } 1.0 CONTAINSVALUE",
        &[top("{ 'a' 1 }"), Value::Boolean(false)],
    );
    assert_fails("[ ] 1 CONTAINSKEY", "CONTAINSKEY operates on a map.");
    assert_fails("[ ] 1 CONTAINSVALUE", "CONTAINSVALUE operates on a map.");
    empty_stack("CONTAINSKEY", &["", "1"]);
    empty_stack("CONTAINSVALUE", &["", "1"]);
}

// -----------------------------------------------------------------------------------------
// REVERSE, CLONEREVERSE, APPEND, FLATTEN, ZIP, CLONE
// -----------------------------------------------------------------------------------------

#[test]
fn reverse_and_clonereverse() {
    // REVERSE reverses the list in place.
    let mut stack = exec("[ 1 2 3 ] DUP REVERSE");
    let reversed = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert!(reversed.same_object(&original));
    assert_eq!(reversed, longs(&[3, 2, 1]));
    // CLONEREVERSE leaves the original alone.
    let mut stack = exec("[ 1 2 3 ] DUP CLONEREVERSE");
    let reversed = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert!(!reversed.same_object(&original));
    assert_eq!(reversed, longs(&[3, 2, 1]));
    assert_eq!(original, longs(&[1, 2, 3]));
    assert_top("'abc' REVERSE", "cba");
    assert_top("'abc' CLONEREVERSE", "cba");
    assert_top("'' REVERSE", "");
    // Characters are kept whole (Java swaps UTF-16 units and breaks surrogate pairs).
    assert_top("'a😀' REVERSE", "😀a");
    assert_top("'010203' HEX-> REVERSE", bytes("030201"));
    assert_top("'0102' HEX-> CLONEREVERSE", bytes("0201"));
    assert_fails(
        "1 REVERSE",
        "REVERSE operates on a list, byte array or String.",
    );
    assert_fails(
        "{ } CLONEREVERSE",
        "CLONEREVERSE operates on a list, byte array or String.",
    );
    empty_stack("REVERSE", &[""]);
    empty_stack("CLONEREVERSE", &[""]);
}

#[test]
fn append_collections_and_maps() {
    assert_eq!(show("[ 1 2 ] [ 3 ] APPEND"), "[1, 2, 3]");
    assert_eq!(show("[ 1 2 ] ( 3 1 ) APPEND"), "[1, 2, 3, 1]");
    assert_eq!(show("( 1 2 ) [ 3 1 ] APPEND"), "[1, 2, 3]");
    assert_eq!(show("( 1 2 ) ( 2 3 ) APPEND"), "[1, 2, 3]");
    assert_eq!(
        show("{ 'a' 1 'b' 2 } { 'b' 3 'c' 4 } APPEND"),
        "{a=1, b=3, c=4}"
    );
    assert_eq!(show("[ 1 2 ] DUP APPEND"), "[1, 2, 1, 2]");
    assert_eq!(show("( 1 2 ) DUP APPEND"), "[1, 2]");
    assert_eq!(show("{ 'a' 1 } DUP APPEND"), "{a=1}");
    // In place: the first collection is returned.
    let mut stack = exec("[ 1 ] DUP [ 2 ] APPEND");
    let appended = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert!(appended.same_object(&original));
    let message = "APPEND can only operate on 2 collections (LIST, SET, VECTOR), 2 MAPs or 2 GTSs.";
    assert_fails("[ ] 1 APPEND", message);
    assert_fails("1 [ ] APPEND", message);
    assert_fails("{ } [ ] APPEND", message);
    assert_fails("[ ] { } APPEND", message);
    assert_fails("{ } 1 APPEND", message);
    empty_stack("APPEND", &["", "[ ]"]);
}

#[test]
fn flatten_nested_lists() {
    assert_eq!(show("[ 1 [ 2 [ 3 [ ] ] ] 4 ] FLATTEN"), "[1, 2, 3, 4]");
    assert_eq!(show("[ ] FLATTEN"), "[]");
    assert_top("1 FLATTEN", 1);
    assert_top("'a' FLATTEN", "a");
    // A scalar 16 levels deep is reachable, 17 levels are not.
    let deep = |levels: usize| format!("{}1 {}FLATTEN", "[ ".repeat(levels), "] ".repeat(levels));
    assert_eq!(show(&deep(15)), "[1]");
    assert_fails(
        &deep(16),
        "FLATTEN cannot flatten lists nested more than 16 levels",
    );
    let empty = |levels: usize| format!("{}{}FLATTEN", "[ ".repeat(levels), "] ".repeat(levels));
    assert_eq!(show(&empty(16)), "[]");
    assert_fails(
        &empty(17),
        "FLATTEN cannot flatten lists nested more than 16 levels",
    );
    // A list containing itself cannot be flattened.
    assert_fails(
        "[ 1 ] 'l' STORE $l $l 0 PUT DROP $l FLATTEN",
        "FLATTEN cannot flatten lists nested more than 16 levels",
    );
    empty_stack("FLATTEN", &[""]);
}

#[test]
fn zip_lists() {
    assert_eq!(show("[ [ 1 2 ] [ 3 4 ] ] ZIP"), "[[1, 3], [2, 4]]");
    assert_eq!(
        show("[ [ 1 2 ] [ 9 ] [ 3 4 ] ] ZIP"),
        "[[1, 9, 3], [2, 9, 4]]"
    );
    assert_eq!(show("[ [ 1 ] [ 2 ] ] ZIP"), "[[1, 2]]");
    assert_eq!(show("[ [ 1 2 3 ] ] ZIP"), "[[1], [2], [3]]");
    assert_eq!(show("[ ] ZIP"), "[[]]");
    assert_fails("1 ZIP", "ZIP operates on lists of lists.");
    assert_fails("[ [ 1 ] 2 ] ZIP", "ZIP operates on lists of lists.");
    assert_fails("[ [ ] 2 ] ZIP", "ZIP cannot operate on empty lists.");
    assert_fails(
        "[ [ 1 2 ] [ 1 2 3 ] ] ZIP",
        "ZIP operates on lists of lists. All lists which are not singletons must be of the same size.",
    );
    empty_stack("ZIP", &[""]);
}

#[test]
fn clone_is_shallow() {
    let mut stack = exec("[ 1 [ 2 ] ] CLONE");
    let clone = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert_eq!(clone, original);
    assert!(!clone.same_object(&original));
    // Shallow like Java: the inner list is shared.
    assert_eq!(show("[ 1 [ 2 ] ] CLONE 1 GET 9 0 SET DROP"), "[1, [9]]");
    let mut stack = exec("{ 'a' 1 } CLONE");
    let clone = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert_eq!(clone, original);
    assert!(!clone.same_object(&original));
    let mut stack = exec("( 1 2 ) CLONE");
    let clone = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert_eq!(clone, original);
    assert!(!clone.same_object(&original));
    assert_fails(
        "1 CLONE",
        "CLONE operates on List, Map, Set, Vector, Geo Time Series or GTS Encoder.",
    );
    empty_stack("CLONE", &[""]);
}

// -----------------------------------------------------------------------------------------
// PIVOT / PIVOTSTRICT
// -----------------------------------------------------------------------------------------

#[test]
fn pivot_is_not_supported() {
    for name in ["PIVOT", "PIVOTSTRICT"] {
        assert_fails(
            &format!("1 {name}"),
            &format!("{name} expects a non empty list of labeling Geo Time Series."),
        );
        assert_fails(
            &format!("[ ] {name}"),
            &format!("{name} expects a non empty list of labeling Geo Time Series."),
        );
        assert_fails(
            &format!("[ ] [ 1 ] {name}"),
            &format!("{name} expects a list of labeling Geo Time Series."),
        );
        empty_stack(name, &[""]);
    }
}

// -----------------------------------------------------------------------------------------
// LSTORE / LCSTORE / MSTORE / MCSTORE
// -----------------------------------------------------------------------------------------

#[test]
fn lstore_and_lcstore() {
    assert_stack(
        "[ 1 2 3 ] [ 'a' 'b' ] LSTORE $a $b",
        &[Value::Long(1), Value::Long(2)],
    );
    // NULL names are skipped, extra names are ignored.
    let stack = exec("[ 1 2 ] [ 'a' NULL 'b' ] LSTORE");
    assert_eq!(stack.load("a"), Some(&Value::Long(1)));
    assert!(!stack.has_symbol("b"));
    // Registers, with a Java int conversion of the number.
    assert_stack(
        "[ 7 8 ] [ 0 4294967297 ] LSTORE 0 LOAD 1 LOAD",
        &[Value::Long(7), Value::Long(8)],
    );
    assert_stack(
        "1 'a' STORE 5 0 STORE [ 2 3 4 ] [ 'a' 'b' 0 ] LCSTORE $a $b 0 LOAD",
        &[Value::Long(1), Value::Long(3), Value::Long(5)],
    );
    assert_stack(
        "[ 2 3 ] [ 'c' 1 ] LCSTORE $c 1 LOAD",
        &[Value::Long(2), Value::Long(3)],
    );
    let names = "LSTORE expects a list of variable names or register numbers as second argument.";
    assert_fails("[ ] 1 LSTORE", names);
    assert_fails("[ ] [ 1.5 ] LSTORE", names);
    assert_fails(
        "1 [ 'a' ] LSTORE",
        "LSTORE expects a list as first argument.",
    );
    assert_fails(
        "[ 1 ] [ 256 ] LSTORE",
        "Invalid register number, must be between 0 and 255",
    );
    assert_fails(
        "[ 1 ] [ -1 ] LCSTORE",
        "Invalid register number, must be between 0 and 255",
    );
    assert_fails(
        "[ ] [ 1.5 ] LCSTORE",
        "LCSTORE expects a list of variable names or register numbers as second argument.",
    );
    empty_stack("LSTORE", &["", "[ ]"]);
    empty_stack("LCSTORE", &["", "[ ]"]);
}

#[test]
fn mstore_and_mcstore() {
    assert_stack(
        "{ 'a' 1 'b' 2 } MSTORE $a $b",
        &[Value::Long(1), Value::Long(2)],
    );
    let stack = exec("{ 'a' 1 'b' 2 } [ 'a' 'z' NULL ] MSTORE");
    assert_eq!(stack.load("a"), Some(&Value::Long(1)));
    assert!(!stack.has_symbol("b"));
    assert!(!stack.has_symbol("z"));
    // NULL keys are skipped.
    let stack = exec("{ NULL 1 'a' 2 } MSTORE");
    assert_eq!(stack.load("a"), Some(&Value::Long(2)));
    assert_stack(
        "{ 0 7 4294967297 8 } MSTORE 0 LOAD 1 LOAD",
        &[Value::Long(7), Value::Long(8)],
    );
    assert_stack(
        "1 'a' STORE 5 0 STORE { 'a' 2 'b' 3 0 4 } MCSTORE $a $b 0 LOAD",
        &[Value::Long(1), Value::Long(3), Value::Long(5)],
    );
    assert_stack(
        "{ 'c' 2 1 3 } [ 'c' 1 ] MCSTORE $c 1 LOAD",
        &[Value::Long(2), Value::Long(3)],
    );
    let names = "MSTORE expects a list of variable names or register numbers as second argument.";
    assert_fails("{ 1.5 1 } MSTORE", names);
    assert_fails("{ } [ 1.5 ] MSTORE", names);
    assert_fails(
        "{ true 1 } MCSTORE",
        "MCSTORE expects a list of variable names or register numbers as second argument.",
    );
    assert_fails("1 MSTORE", "MSTORE expects a Map as argument.");
    assert_fails("1 [ 'a' ] MSTORE", "MSTORE expects a Map as argument.");
    assert_fails(
        "{ 256 1 } MSTORE",
        "Invalid register number, must be between 0 and 255",
    );
    empty_stack("MSTORE", &["", "[ ]"]);
    empty_stack("MCSTORE", &["", "[ ]"]);
}

#[test]
fn lstore_api_paths() {
    // Storing a NULL value into a symbol binds it to NULL.
    let stack = exec("[ NULL ] [ 'a' ] LSTORE");
    assert_eq!(stack.load("a"), Some(&Value::Null));
    // A full symbol table is reported.
    let mut stack = Stack::with_limits(Limits {
        max_symbols: 0,
        ..Limits::unlimited()
    });
    let err = stack.exec("[ 1 ] [ 'a' ] LSTORE").unwrap_err().to_string();
    assert!(
        err.contains("Symbol table has reached its maximum number of entries: 0"),
        "{err}"
    );
    let mut stack = Stack::new();
    stack.push(longs(&[1]));
    stack.push(Value::list(vec![Value::from("x")]));
    crate::functions::call(&mut stack, "LSTORE").unwrap();
    assert_eq!(stack.load("x"), Some(&Value::Long(1)));
}
