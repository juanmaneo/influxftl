#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::encoding::hex_decode;
use crate::tests::{assert_fails, assert_top, top};
use crate::value::Value;

fn bytes(hex: &str) -> Value {
    Value::bytes(hex_decode(hex).unwrap())
}

/// `PACK` the values of `list` (a WarpScript list literal) with `format`.
fn pack(list: &str, format: &str) -> Value {
    top(&format!("{list} '{format}' PACK"))
}

/// `UNPACK` the bytes `hex` with `format`, as a Java-style string.
fn unpack(hex: &str, format: &str) -> String {
    top(&format!("'{hex}' HEX-> '{format}' UNPACK")).to_string()
}

// Reference values from PACK / UNPACK on JDK 27 (see /tmp/jref/coll/Pack.java).

#[test]
fn pack_integers() {
    assert_eq!(pack("[ 1 ]", "<L8"), bytes("80"));
    assert_eq!(pack("[ 1 ]", ">L8"), bytes("01"));
    assert_eq!(pack("[ 1 2 3 ]", ">L8>L8>L8"), bytes("010203"));
    assert_eq!(pack("[ -1 255 256 ]", ">L8<U8>U16"), bytes("ffff0100"));
    assert_eq!(
        pack("[ 1311768467463790320 ]", ">L"),
        bytes("123456789abcdef0")
    );
    assert_eq!(
        pack("[ 1311768467463790320 ]", "<L"),
        bytes("0f7b3d591e6a2c48")
    );
    assert_eq!(
        pack("[ 1311768467463790320 ]", ">U64"),
        bytes("123456789abcdef0")
    );
    assert_eq!(
        pack("[ 1311768467463790320 ]", "<L64"),
        bytes("0f7b3d591e6a2c48")
    );
    assert_eq!(pack("[ -3 ]", ">L4"), bytes("d0"));
    assert_eq!(pack("[ -3 ]", "<L4"), bytes("b0"));
    assert_eq!(pack("[ -3 ]", "<L12"), bytes("bff0"));
    assert_eq!(pack("[ 1000 ]", ">L10"), bytes("fa00"));
    assert_eq!(pack("[ 1000 ]", "<L10"), bytes("17c0"));
    // A zero length means 64 bits; lengths wrap like a Java int.
    assert_eq!(pack("[ 1 ]", ">U0"), bytes("0000000000000001"));
    assert_eq!(pack("[ 1 ]", "<L0"), bytes("8000000000000000"));
    assert_eq!(pack("[ 1 ]", "<L4294967297"), bytes("80"));
    assert_eq!(pack("[ 1 ]", "<L4294967296"), bytes("8000000000000000"));
    // Extra values are ignored.
    assert_eq!(pack("[ 1 2 3 ]", ">L8"), bytes("01"));
    // Doubles are truncated to longs for L / U.
    assert_eq!(pack("[ 1e19 ]", ">L"), bytes("7fffffffffffffff"));
    assert_eq!(pack("[ -1.5 ]", ">L4"), bytes("f0"));
}

#[test]
fn pack_doubles_booleans_and_skips() {
    assert_eq!(pack("[ 1.5 ]", ">D"), bytes("3ff8000000000000"));
    assert_eq!(pack("[ 1.5 ]", "<D"), bytes("0000000000001ffc"));
    assert_eq!(pack("[ 2 ]", ">D"), bytes("4000000000000000"));
    assert_eq!(pack("[ NaN ]", ">D"), bytes("7ff8000000000000"));
    assert_eq!(pack("[ true false 2 0 0.5 ]", "BBBBB"), bytes("a0"));
    assert_eq!(pack("[ 5 7 ]", ">L3s2>L3"), bytes("a7"));
    assert_eq!(pack("[ 5 7 ]", ">L3S2>L3"), bytes("bf"));
    assert_eq!(pack("[ 5 7 ]", ">L3S13<L3"), bytes("bfffe0"));
    assert_eq!(
        pack("[ 2.5 3 true ]", ">L4<D>U1"),
        bytes("200000000000010028")
    );
    assert_eq!(pack("[ ]", ""), bytes(""));
    assert_eq!(pack("[ ]", "s8"), bytes("00"));
    assert_eq!(pack("[ ]", "S3"), bytes("e0"));
    assert_eq!(
        pack("[ MINLONG 1e300 ]", "<L<D"),
        bytes("000000000000000139ae00113c27ec7e")
    );
}

#[test]
fn pack_errors() {
    assert_fails(
        "[ 1 ] 1 PACK",
        "PACK expects a format string on top of the stack.",
    );
    assert_fails(
        "1 '>L8' PACK",
        "PACK operates on a list of numeric or boolean values.",
    );
    assert_fails(
        "[ 1 'a' ] '>L8' PACK",
        "PACK operates on a list of numeric or boolean values.",
    );
    assert_fails("[ 1 ] '>L8>L8' PACK", "Index 1 out of bounds for length 1");
    assert_fails(
        "[ 1 ] '<L65' PACK",
        "PACK encountered an invalid length for 'L', max length is 64.",
    );
    assert_fails(
        "[ 1 ] '>L8<L2147483641' PACK",
        "PACK encountered an invalid length for 'L', max length is 64.",
    );
    assert_fails(
        "[ 1 ] '<' PACK",
        "PACK encountered an invalid format specification.",
    );
    assert_fails(
        "[ 1 ] '>' PACK",
        "PACK encountered an invalid format specification.",
    );

    assert_fails(
        "[ 1 ] '<X' PACK",
        "PACK encountered an invalid format specification 'X'.",
    );
    assert_fails(
        "[ 1 ] 'L8' PACK",
        "PACK encountered an invalid format specification 'L'.",
    );
    assert_fails(
        "[ 1 ] 'D' PACK",
        "PACK encountered an invalid format specification 'D'.",
    );
    assert_fails(
        "[ 1 ] 'U' PACK",
        "PACK encountered an invalid format specification 'U'.",
    );
    assert_fails(
        "[ 1 ] '<B' PACK",
        "PACK encountered an invalid format specification 'B'.",
    );
    assert_fails(
        "[ 1 ] '>L8 ' PACK",
        "PACK encountered an invalid format specification ' '.",
    );
    assert_fails(
        "[ 1 ] '<L8é' PACK",
        "PACK encountered an invalid format specification 'é'.",
    );
    // Java prints the lone high surrogate of a non-BMP character; U+FFFD here.
    assert_fails(
        "[ 1 ] '😀' PACK",
        "PACK encountered an invalid format specification '\u{fffd}'.",
    );
    assert_fails(
        "[ 1 ] '<L😀' PACK",
        "PACK encountered an invalid format specification '\u{fffd}'.",
    );
    assert_fails(
        "[ 1 ] 'S' PACK",
        "PACK encountered an invalid Skip specification, length must be a strictly positive number.",
    );
    assert_fails(
        "[ 1 ] 's0' PACK",
        "PACK encountered an invalid Skip specification, length must be a strictly positive number.",
    );
    assert_fails(
        "[ ] 's4294967296' PACK",
        "PACK encountered an invalid Skip specification, length must be a strictly positive number.",
    );
    // A wrapped negative total length is rejected by the Java output buffer.
    assert_fails(
        "[ 1 ] '<L2147483648' PACK",
        "Negative initial size: -268435455",
    );
    assert_fails(
        "[ ] 's2147483648' PACK",
        "Negative initial size: -268435455",
    );
    assert_fails(
        "[ 1 ] '>L8<L2147483648' PACK",
        "Negative initial size: -268435454",
    );
    assert_fails("PACK", "Empty stack.");
    assert_fails("'>L8' PACK", "Empty stack.");
}

#[test]
fn unpack_values() {
    assert_eq!(unpack("80", "<L8"), "[1]");
    assert_eq!(unpack("01", ">L8"), "[1]");
    assert_eq!(unpack("010203", ">L8>L8>L8"), "[1, 2, 3]");
    assert_eq!(unpack("ffff0100", ">L8<U8>U16"), "[-1, 255, 256]");
    assert_eq!(unpack("123456789abcdef0", ">L"), "[1311768467463790320]");
    assert_eq!(unpack("123456789abcdef0", "<L"), "[1115552785675988040]");
    assert_eq!(unpack("123456789abcdef0", ">U64"), "[1311768467463790320]");
    assert_eq!(unpack("3ff8000000000000", ">D"), "[1.5]");
    assert_eq!(unpack("000000000000f83f", "<D"), "[-7.552618358834999E289]");
    assert_eq!(unpack("4000000000000000", ">D"), "[2.0]");
    assert_eq!(unpack("a0", "BBBBB"), "[true, false, true, false, false]");
    assert_eq!(
        unpack("a0", "BBBBBBBBBBBB"),
        "[true, false, true, false, false, false, false, false, false, false, false, false]"
    );
    assert_eq!(unpack("a0e0", ">L3s2>L3"), "[-3, 0]");
    assert_eq!(unpack("a0e0", ">L3S2>L3"), "[-3, 0]");
    assert_eq!(unpack("d0", ">L4"), "[-3]");
    assert_eq!(unpack("b0", "<L4"), "[-3]");
    assert_eq!(unpack("fd", ">L4>L4"), "[-1, -3]");
    assert_eq!(unpack("fd", ">U4>U4"), "[15, 13]");
    assert_eq!(unpack("fd", "<L4<L4"), "[-1, -5]");
    assert_eq!(unpack("fd", "<U4<U4"), "[15, 11]");
    assert_eq!(unpack("fa00", ">L10"), "[-24]");
    assert_eq!(unpack("5e00", "<L10"), "[122]");
    // Bits past the end of the data read as zeros.
    assert_eq!(unpack("ff", ">L16"), "[-256]");
    assert_eq!(unpack("ff", ">U16"), "[65280]");
    assert_eq!(unpack("", ">L8"), "[0]");
    assert_eq!(unpack("", "B"), "[false]");
    assert_eq!(unpack("", ""), "[]");
    // Wrapped lengths.
    assert_eq!(unpack("ff", ">L2147483648"), "[0]");
    assert_eq!(unpack("ff", ">L8<L2147483648"), "[-1, 0]");
    assert_eq!(unpack("ff", "<L4294967296"), "[255]");
    assert_eq!(
        top("'ff' HEX-> '>L8' UNPACK"),
        Value::list(vec![Value::Long(-1)])
    );
}

#[test]
fn unpack_errors() {
    assert_fails(
        "'' HEX-> 1 UNPACK",
        "UNPACK expects a format string on top of the stack.",
    );
    assert_fails("1 '>L8' UNPACK", "UNPACK operates on an array of bytes.");
    assert_fails(
        "'' HEX-> '<L65' UNPACK",
        "UNPACK encountered an invalid length for 'L', max length is 64.",
    );
    assert_fails(
        "'' HEX-> 'Z' UNPACK",
        "UNPACK encountered an invalid format specification 'Z'.",
    );
    assert_fails(
        "'ff' HEX-> 's4294967296B' UNPACK",
        "UNPACK encountered an invalid Skip specification, length must be a strictly positive number.",
    );
    // A negative bit index fails like BitSet.get.
    assert_fails(
        "'00' HEX-> 's2147483647s2147483647B' UNPACK",
        "bitIndex < 0: -2",
    );
    assert_fails(
        "'00' HEX-> 's2147483647s2147483647>L4' UNPACK",
        "bitIndex < 0: -2",
    );
    assert_fails(
        "'ff' HEX-> '<L2147483648>L8' UNPACK",
        "bitIndex < 0: -2147483648",
    );
    assert_fails(
        "'ff' HEX-> 's2147483648B' UNPACK",
        "bitIndex < 0: -2147483648",
    );
    assert_fails(
        "'ff' HEX-> 's2147483648<L4' UNPACK",
        "bitIndex < 0: -2147483645",
    );
    assert_fails(
        "'ff' HEX-> 's2147483648<D' UNPACK",
        "bitIndex < 0: -2147483585",
    );
    assert_fails("UNPACK", "Empty stack.");
    assert_fails("'>L8' UNPACK", "Empty stack.");
}

#[test]
fn pack_then_unpack_roundtrip() {
    assert_top(
        "[ -5 300 true 2.25 ] '>L8>U16B<D' PACK '>L8>U16B<D' UNPACK",
        top("[ -5 300 true 2.25 ]"),
    );
}
