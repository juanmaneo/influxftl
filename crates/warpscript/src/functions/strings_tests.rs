// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::rc::Rc;

use super::strings::{
    Charset, CompiledPattern, base64_decode, base64_decode_error, decode_utf8_java,
    java_url_decode, push_uuid, regex_replace, uuid_string,
};
use crate::bitset::BitSet;
use crate::encoding::{hex_decode, opb64_encode};
use crate::snapshot::value_snapshot;
use crate::stack::Stack;
use crate::tests::{assert_fails, assert_stack, assert_top, exec, top};
use crate::value::{FunctionObject, Value};

fn bytes(hex: &str) -> Value {
    Value::bytes(hex_decode(hex).unwrap())
}

fn strings(items: &[&str]) -> Value {
    Value::list(items.iter().map(Value::string).collect())
}

// -----------------------------------------------------------------------------------------
// Case, trimming, sub-strings
// -----------------------------------------------------------------------------------------

#[test]
fn case_and_trim() {
    // Reference values from String.toUpperCase() / toLowerCase() / trim() on JDK 27.
    assert_top(
        "'stra\u{df}e \u{131} i \u{1c5} \u{fb01}' TOUPPER",
        "STRASSE I I \u{1c4} FI",
    );
    assert_top(
        "'\u{130} \u{3a3}\u{39f}\u{3a3} \u{1e9e} ABC' TOLOWER",
        "i\u{307} \u{3c3}\u{3bf}\u{3c2} \u{df} abc",
    );
    assert_top("'  a b \t ' TRIM", "a b");
    assert_top("'%00 %09 x %C2%A0' TRIM", "x \u{a0}");
    assert_top("'' TRIM", "");
    assert_fails("1 TOUPPER", "TOUPPER operates on a String.");
    assert_fails("1 TOLOWER", "TOLOWER operates on a String.");
    assert_fails("1 TRIM", "TRIM operates on a String.");
    assert_fails("TRIM", "Empty stack.");
}

#[test]
fn substring_of_strings() {
    assert_top("'hello' 2 SUBSTRING", "llo");
    assert_top("'hello' -1 SUBSTRING", "o");
    assert_top("'hello' 0 SUBSTRING", "hello");
    assert_top("'' 0 SUBSTRING", "");
    assert_top("'hello' 4294967297 SUBSTRING", "ello");
    assert_fails("'hello' 5 SUBSTRING", "Index out of bound, 5 >= 5");
    assert_fails("'hello' -6 SUBSTRING", "Index out of bound, -6 < -5");
    assert_fails("'' 1 SUBSTRING", "Index out of bound, 1 >= 0");
    assert_top("'hello' 1 3 SUBSTRING", "ell");
    assert_top("'hello' 1 100 SUBSTRING", "ello");
    assert_top("'hello' 0 0 SUBSTRING", "");
    assert_top("'' 0 5 SUBSTRING", "");
    assert_top("'hello' -2 1 SUBSTRING", "l");
    assert_top("'hello' 4 1 SUBSTRING", "o");
    assert_fails("'hello' 5 1 SUBSTRING", "Index out of bound, 5 >= 5");
    assert_fails(
        "'hello' 1 -1 SUBSTRING",
        "Range [1, 0) out of bounds for length 5",
    );
    // `n + idx` overflows like a Java int.
    assert_fails(
        "'hello' 1 2147483647 SUBSTRING",
        "Range [1, -2147483648) out of bounds for length 5",
    );
    // Indices are UTF-16 code units: a split surrogate pair becomes U+FFFD.
    assert_top("'a\u{1f600}b' 1 SUBSTRING", "\u{1f600}b");
    assert_top("'a\u{1f600}b' 2 SUBSTRING", "\u{fffd}b");
    assert_top("'a\u{1f600}b' 0 2 SUBSTRING", "a\u{fffd}");
    assert_top("'a\u{1f600}b' -1 SUBSTRING", "b");
}

#[test]
fn substring_of_bytes() {
    assert_top("'hello' 'UTF-8' ->BYTES 2 SUBSTRING", Value::bytes(b"llo"));
    assert_top(
        "'hello' 'UTF-8' ->BYTES 0 SUBSTRING",
        Value::bytes(b"hello"),
    );
    assert_top("'' 'UTF-8' ->BYTES 0 SUBSTRING", Value::bytes(b""));
    assert_fails(
        "'hello' 'UTF-8' ->BYTES 5 SUBSTRING",
        "Index out of bound, 5 >= 5",
    );
    assert_top(
        "'hello' 'UTF-8' ->BYTES 1 3 SUBSTRING",
        Value::bytes(b"ell"),
    );
    assert_top(
        "'hello' 'UTF-8' ->BYTES -2 9 SUBSTRING",
        Value::bytes(b"lo"),
    );
    assert_top("'hello' 'UTF-8' ->BYTES 0 0 SUBSTRING", Value::bytes(b""));
    assert_fails("'hello' 'UTF-8' ->BYTES 1 -1 SUBSTRING", "1 > 0");
    assert_fails(
        "'hello' 'UTF-8' ->BYTES 6 1 SUBSTRING",
        "Index out of bound, 6 >= 5",
    );
}

#[test]
fn substring_errors() {
    assert_fails(
        "'hello' 1.5 SUBSTRING",
        "SUBSTRING expects a numeric length or 0-based start index on top of the stack.",
    );
    assert_fails(
        "'hello' 1.5 1 SUBSTRING",
        "SUBSTRING expects a 0-based start index below the length.",
    );
    assert_fails(
        "1.5 1 1 SUBSTRING",
        "SUBSTRING can only operate on strings or byte arrays.",
    );
    assert_fails("SUBSTRING", "Empty stack.");
    assert_fails("1 SUBSTRING", "Empty stack.");
    assert_fails("1 1 SUBSTRING", "Empty stack.");
}

// -----------------------------------------------------------------------------------------
// Splitting and joining
// -----------------------------------------------------------------------------------------

#[test]
fn split() {
    assert_top("'a,b,c' ',' SPLIT", strings(&["a", "b", "c"]));
    assert_top("'a,b,c' ',' 2 SPLIT", strings(&["a", "b,c"]));
    assert_top("'a,b,c' ',' 1 SPLIT", strings(&["a,b,c"]));
    assert_top("'a,b,c' ',' 10 SPLIT", strings(&["a", "b", "c"]));
    assert_top("'a,' ',' SPLIT", strings(&["a", ""]));
    assert_top("',' ',' SPLIT", strings(&["", ""]));
    assert_top("'' ',' SPLIT", strings(&[""]));
    assert_top("'abc' ',' SPLIT", strings(&["abc"]));
    assert_top("'a\u{e9}b' '\u{e9}' SPLIT", strings(&["a", "b"]));
    assert_fails(
        "'a,b' ',' 0 SPLIT",
        "SPLIT expects the limit to be a strictly positive integer.",
    );
    assert_fails(
        "'a,b' ',' -1 SPLIT",
        "SPLIT expects the limit to be a strictly positive integer.",
    );
    assert_fails("'a,b' ',' 4294967296 SPLIT", "integer overflow");
    assert_fails(
        "'a,b' ',,' SPLIT",
        "SPLIT expects a string delimiter of length 1.",
    );
    assert_fails(
        "'a,b' '' SPLIT",
        "SPLIT expects a string delimiter of length 1.",
    );
    // A non-BMP delimiter has a UTF-16 length of 2.
    assert_fails(
        "'a,b' '\u{1f600}' SPLIT",
        "SPLIT expects a string delimiter of length 1.",
    );
    assert_fails(
        "'a,b' 1.5 SPLIT",
        "SPLIT expects a string delimiter of length 1.",
    );
    assert_fails("1 ',' SPLIT", "SPLIT operates on a String.");
    assert_fails("SPLIT", "Empty stack.");
    assert_fails("2 SPLIT", "Empty stack.");
}

#[test]
fn join_list() {
    assert_top("[ 'a' 'b' 'c' ] ', ' JOIN", "a, b, c");
    assert_top("[ ] ',' JOIN", "");
    assert_top("[ 'a' ] ',' JOIN", "a");
    // The separator is only inserted when the buffer is not empty, like Warp 10.
    assert_top("[ '' 'a' 'b' ] ',' JOIN", "a,b");
    assert_top("[ NULL 1.0 [ 1 2 ] true ] ',' JOIN", "null,1.0,[1, 2],true");
    assert_fails(
        "'x' ',' JOIN",
        "JOIN expects a separator and a list of strings to join on the stack.",
    );
    assert_fails("',' JOIN", "Empty stack.");
}

#[test]
fn join_count() {
    assert_top("'a' 'b' 'c' ', ' 3 JOIN", "a, b, c");
    assert_top("'a' 1 2.5 ', ' 3 JOIN", "a, 1, 2.5");
    assert_top("', ' 0 JOIN", "");
    assert_stack(
        "'x' 'a' 'b' ',' 2 JOIN",
        &[Value::from("x"), Value::from("a,b")],
    );
    assert_fails("'a' NULL ',' 2 JOIN", "JOIN cannot join a NULL element.");
    assert_fails("',' -1 JOIN", "Index out of bound.");
    assert_fails("'a' ',' 5 JOIN", "Index out of bound.");
    assert_fails(
        "'a' 1 2 JOIN",
        "JOIN expects the separator string below the number of strings to join.",
    );
    assert_fails(
        "1.5 JOIN",
        "JOIN expects the number of strings to join on top of the stack.",
    );
    assert_fails("JOIN", "Empty stack.");
    assert_fails("2 JOIN", "Empty stack.");
}

// -----------------------------------------------------------------------------------------
// URL encoding
// -----------------------------------------------------------------------------------------

#[test]
fn urlencode() {
    assert_top("'a b+c/\u{e9}' URLENCODE", "a%20b%2Bc%2F%C3%A9");
    assert_top("'a-b.c_d*e' URLENCODE", "a-b.c_d*e");
    assert_fails("1 URLENCODE", "URLENCODE operates on a String.");
}

#[test]
fn urldecode() {
    // Reference values from java.net.URLDecoder.decode(s, "UTF-8") on JDK 27.
    assert_top("'a+b%2520c' URLDECODE", "a b c");
    assert_top("'%25C3%25A9' URLDECODE", "\u{e9}");
    assert_top("'%25FF' URLDECODE", "\u{fffd}");
    assert_top("'%25C3x' URLDECODE", "\u{fffd}x");
    assert_top("'%25E2%2582%25ACx' URLDECODE", "\u{20ac}x");
    assert_top("'%25ED%25A0%2580' URLDECODE", "\u{fffd}");
    assert_top("'%252B' URLDECODE", "+");
    assert_top("'a%2541%2542b' URLDECODE", "aABb");
    assert_top("'\u{e9}+%2541' URLDECODE", "\u{e9} A");
    assert_top("'plain' URLDECODE", "plain");
    assert_top("'' URLDECODE", "");
    for script in [
        "'%25' URLDECODE",
        "'ab%252' URLDECODE",
        "'%2541%25' URLDECODE",
        "'%25%25' URLDECODE",
    ] {
        assert_fails(script, "URLDecoder: Incomplete trailing escape (%) pattern");
    }
    assert_fails(
        "'%25zz' URLDECODE",
        "URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: \"z\" = 122",
    );
    assert_fails(
        "'%254z' URLDECODE",
        "URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: \"z\" = 122",
    );
    assert_fails(
        "'%25%2541' URLDECODE",
        "URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: \"%\" = 37",
    );
    assert_fails(
        "'%254\u{e9}' URLDECODE",
        "URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: \"\u{e9}\" = 233",
    );
    assert_eq!(
        java_url_decode("%4\u{1f600}").unwrap_err().to_string(),
        "URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: \"\u{1f600}\" = 55357"
    );
    assert_fails("1 URLDECODE", "URLDECODE operates on a String.");
}

// -----------------------------------------------------------------------------------------
// UUID
// -----------------------------------------------------------------------------------------

#[test]
fn uuid() {
    assert_eq!(uuid_string(0, 0), "00000000-0000-4000-8000-000000000000");
    assert_eq!(
        uuid_string(u64::MAX, u64::MAX),
        "ffffffff-ffff-4fff-bfff-ffffffffffff"
    );
    assert_eq!(
        uuid_string(0x0123_4567_89AB_CDEF, 0x0123_4567_89AB_CDEF),
        "01234567-89ab-4def-8123-456789abcdef"
    );
    let Value::String(uuid) = top("UUID") else {
        panic!("not a string");
    };
    let parts: Vec<&str> = uuid.split('-').collect();
    assert_eq!(
        parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
        vec![8, 4, 4, 4, 12]
    );
    assert!(uuid.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
    assert!(parts[2].starts_with('4'));
    assert!(parts[3].starts_with(['8', '9', 'a', 'b']));
    assert_ne!(top("UUID"), top("UUID"));
    let mut stack = Stack::new();
    assert_eq!(
        push_uuid(&mut stack, Err(getrandom::Error::UNSUPPORTED))
            .unwrap_err()
            .to_string(),
        format!(
            "Unable to obtain random data from the operating system ({}).",
            getrandom::Error::UNSUPPORTED
        )
    );
}

// -----------------------------------------------------------------------------------------
// Hexadecimal, binary, base64 and OPB64
// -----------------------------------------------------------------------------------------

#[test]
fn hex_conversions() {
    assert_top("'00abFF' HEX->", bytes("00abff"));
    assert_top("'abc' HEX->", bytes("0abc"));
    assert_top("'' HEX->", bytes(""));
    assert_fails("'zz' HEX->", "Invalid hex character.");
    assert_fails("'z' HEX->", "Invalid hex character.");
    assert_fails("1 HEX->", "HEX-> operates on a String.");
    assert_top("'0f' HEXTOBIN", "00001111");
    assert_top(
        "'aBcDeF0123456789' HEXTOBIN",
        "1010101111001101111011110000000100100011010001010110011110001001",
    );
    assert_top("'' HEXTOBIN", "");
    assert_fails(
        "'0g' HEXTOBIN",
        "HEXTOBIN encountered an invalid hex character 'g'.",
    );
    assert_fails("1 HEXTOBIN", "HEXTOBIN operates on a String.");
    assert_top("'616263' HEXTOB64", "YWJj");
    assert_top("'' HEXTOB64", "");
    assert_fails("'abc' HEXTOB64", "Odd number of hexadecimal digits.");
    assert_fails("1 HEXTOB64", "HEXTOB64 operates on a String.");
}

#[test]
fn binary_conversions() {
    assert_top("'1111' BINTOHEX", "0F");
    assert_top("'100000001' BINTOHEX", "0101");
    assert_top("'1010101111001101' BINTOHEX", "ABCD");
    assert_top("'' BINTOHEX", "");
    assert_fails(
        "'102' BINTOHEX",
        "BINTOHEX expects a valid binary representation. \"102\" is invalid.",
    );
    assert_fails("1 BINTOHEX", "BINTOHEX operates on a String.");
    assert_top("'1111' BIN->", bytes("0f"));
    assert_top("'100000001' BIN->", bytes("0101"));
    assert_top("'0000000100000010' BIN->", bytes("0102"));
    assert_top("'' BIN->", bytes(""));
    assert_fails(
        "'1x' BIN->",
        "BIN-> expects a valid binary representation. \"1x\" is invalid.",
    );
    assert_fails("1 BIN->", "BIN-> operates on a String.");
}

#[test]
fn base64_encoding() {
    // Reference values from Guava BaseEncoding.base64() / base64Url().
    assert_top("'a' ->B64", "YQ==");
    assert_top("'ab' ->B64URL", "YWI=");
    assert_top("'fbff01' HEX-> ->B64", "+/8B");
    assert_top("'fbff01' HEX-> ->B64URL", "-_8B");
    assert_top("'' ->B64", "");
    assert_top("'' HEX-> ->B64URL", "");
    assert_top("'\u{e9}' ->B64", "w6k=");
    assert_fails("1 ->B64", "->B64 operates on a String or a byte array.");
    assert_fails(
        "1 ->B64URL",
        "->B64URL operates on a String or a byte array.",
    );
    assert_fails("->B64", "Empty stack.");
}

#[test]
fn base64_decoding() {
    for script in [
        "'YQ' B64->",
        "'YQ==' B64->",
        "'YQ====' B64->",
        "'YR' B64->",
        "'Y Q%0A=%0D=' B64->",
    ] {
        assert_top(script, bytes("61"));
    }
    assert_top("'YWJj' B64->", bytes("616263"));
    assert_top("'YWJjZA' B64->", bytes("61626364"));
    assert_top("'YWJjZA=' B64->", bytes("61626364"));
    assert_top("'=' B64->", bytes(""));
    assert_top("'' B64->", bytes(""));
    assert_top("'YW+/' B64->", bytes("616fbf"));
    assert_top("'YW-_' B64URL->", bytes("616fbf"));
    assert_top("'YQ==' B64URL->", bytes("61"));
    assert_fails("'Y' B64->", "Invalid input length 1");
    assert_fails("'YWJjZ' B64URL->", "Invalid input length 5");
    assert_fails("'YQ==YQ==' B64->", "Unrecognized character: =");
    assert_fails("'Y=Q' B64->", "Unrecognized character: =");
    assert_fails("'Y!' B64->", "Unrecognized character: !");
    assert_fails("'YW-_' B64->", "Unrecognized character: -");
    assert_fails("'YW+/' B64URL->", "Unrecognized character: +");
    assert_fails("'Y Q' B64URL->", "Unrecognized character: 0x20");
    assert_fails("'YQ%0A' B64URL->", "Unrecognized character: 0xa");
    assert_fails("'Y\u{e9}' B64->", "Unrecognized character: 0xe9");
    assert_fails("'YQ\u{1f600}=' B64->", "Unrecognized character: 0xd83d");
    // Guava reports the first invalid character in reading order, whatever the crate found.
    assert_eq!(
        base64_decode("Y-J\u{e9}", false).unwrap_err().to_string(),
        "Unrecognized character: -"
    );
    assert_eq!(
        base64_decode_error("YWJj", "ABC", &base64::DecodeError::InvalidLength(4)).to_string(),
        "Unrecognized character: Y"
    );
    assert_eq!(
        base64_decode_error("", "", &base64::DecodeError::InvalidPadding).to_string(),
        base64::DecodeError::InvalidPadding.to_string()
    );
    assert_fails("1 B64->", "B64-> operates on a String.");
    assert_fails("1 B64URL->", "B64URL-> operates on a String.");
    assert_top("'YWJj' B64TOHEX", "616263");
    assert_top("'+/8B' B64TOHEX", "fbff01");
    assert_fails("'Y' B64TOHEX", "Invalid input length 1");
    assert_fails("1 B64TOHEX", "B64TOHEX operates on a String.");
}

#[test]
fn opb64_conversions() {
    assert_top("'foobar' ->OPB64", "OaxjNa4m");
    assert_top("'foobar' 'UTF-8' ->BYTES ->OPB64", "OaxjNa4m");
    assert_top("'' ->OPB64", "");
    assert_fails("1 ->OPB64", "->OPB64 operates on a String or a byte array.");
    assert_top("'OaxjNa4m' OPB64->", Value::bytes(b"foobar"));
    assert_top("'' OPB64->", bytes(""));
    assert_fails("'Oax!' OPB64->", "Invalid OPB64 character '!' at offset 3.");
    assert_fails("1 OPB64->", "OPB64-> operates on a String.");
    assert_top("'OaxjNa4m' OPB64TOHEX", "666f6f626172");
    assert_fails(
        "'Oax!' OPB64TOHEX",
        "Invalid OPB64 character '!' at offset 3.",
    );
    assert_fails("1 OPB64TOHEX", "OPB64TOHEX operates on a String.");
}

// -----------------------------------------------------------------------------------------
// Charsets
// -----------------------------------------------------------------------------------------

#[test]
fn charset_names() {
    for (name, charset) in [
        ("UTF-8", Charset::Utf8),
        ("utf8", Charset::Utf8),
        ("ISO-8859-1", Charset::Latin1),
        ("latin1", Charset::Latin1),
        ("ISO8859_1", Charset::Latin1),
        ("US-ASCII", Charset::Ascii),
        ("ascii", Charset::Ascii),
        ("UTF-16", Charset::Utf16),
        ("utf16", Charset::Utf16),
        ("UTF-16BE", Charset::Utf16Be),
        ("X-UTF-16BE", Charset::Utf16Be),
        ("UTF-16LE", Charset::Utf16Le),
        ("UTF_16LE", Charset::Utf16Le),
    ] {
        assert_eq!(Charset::for_name(name), Some(charset), "{name}");
    }
    for name in ["UTF-32", "FOO", "", " x", "UTF-8 "] {
        assert_eq!(Charset::for_name(name), None, "{name:?}");
    }
}

#[test]
fn charset_encoding() {
    // Reference values from String.getBytes(charset) on JDK 27.
    let sample = "a\u{e9}\u{20ac}\u{1f600}";
    for (name, hex) in [
        ("UTF-8", "61c3a9e282acf09f9880"),
        ("ISO-8859-1", "61e93f3f"),
        ("US-ASCII", "613f3f3f"),
        ("UTF-16", "feff006100e920acd83dde00"),
        ("UTF-16BE", "006100e920acd83dde00"),
        ("UTF-16LE", "6100e900ac203dd800de"),
    ] {
        assert_eq!(
            Charset::for_name(name).unwrap().encode(sample),
            hex_decode(hex).unwrap(),
            "{name}"
        );
    }
    assert_eq!(Charset::Utf16.encode(""), Vec::<u8>::new());
    assert_top("'a\u{e9}' 'ISO-8859-1' ->BYTES", bytes("61e9"));
    assert_top("'ab' 'UTF-16' ->BYTES", bytes("feff00610062"));
    assert_fails(
        "'a' 'UTF-32' ->BYTES",
        "->BYTES unsupported encoding 'UTF-32' (UTF-32)",
    );
    assert_fails(
        "'a' 1 ->BYTES",
        "->BYTES operates on a String and expects a charset name on top of the stack.",
    );
    assert_fails("1 'UTF-8' ->BYTES", "->BYTES operates on a String.");
    assert_fails("->BYTES", "Empty stack.");
}

#[test]
fn charset_decoding() {
    // Reference values from new String(bytes, charset) on JDK 27.
    for (hex, expected) in [
        ("ff", "\u{fffd}"),
        ("c080", "\u{fffd}\u{fffd}"),
        ("eda080", "\u{fffd}"),
        ("eda08080", "\u{fffd}\u{fffd}"),
        ("eda0", "\u{fffd}"),
        ("eda041", "\u{fffd}A"),
        ("f09f98", "\u{fffd}"),
        ("e282", "\u{fffd}"),
        ("e28241", "\u{fffd}A"),
        ("f09f9880", "\u{1f600}"),
        ("f888808080", "\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}"),
        ("f4908080", "\u{fffd}\u{fffd}\u{fffd}\u{fffd}"),
        ("e08080", "\u{fffd}\u{fffd}\u{fffd}"),
        ("80", "\u{fffd}"),
        ("c3", "\u{fffd}"),
        ("c341", "\u{fffd}A"),
        ("f09f41", "\u{fffd}A"),
        ("efbfbd", "\u{fffd}"),
        ("61c3a9", "a\u{e9}"),
        ("", ""),
    ] {
        assert_eq!(
            decode_utf8_java(&hex_decode(hex).unwrap()),
            expected,
            "{hex}"
        );
    }
    for (hex, utf16, be, le) in [
        ("feff0041", "A", "\u{feff}A", "\u{fffe}\u{4100}"),
        ("fffe4100", "A", "\u{fffe}\u{4100}", "\u{feff}A"),
        ("0041", "A", "A", "\u{4100}"),
        ("004142", "A\u{fffd}", "A\u{fffd}", "\u{4100}\u{fffd}"),
        ("d83dde00", "\u{1f600}", "\u{1f600}", "\u{3dd8}\u{de}"),
        ("d83d0041", "\u{fffd}", "\u{fffd}", "\u{3dd8}\u{4100}"),
        ("de000041", "\u{fffd}A", "\u{fffd}A", "\u{de}\u{4100}"),
        ("feff", "", "\u{feff}", "\u{fffe}"),
        ("41", "\u{fffd}", "\u{fffd}", "\u{fffd}"),
        (
            "fefffeff0041",
            "\u{feff}A",
            "\u{feff}\u{feff}A",
            "\u{fffe}\u{fffe}\u{4100}",
        ),
        ("fffe", "", "\u{fffe}", "\u{feff}"),
        ("d83d", "\u{fffd}", "\u{fffd}", "\u{3dd8}"),
        ("d83d00", "\u{fffd}", "\u{fffd}", "\u{3dd8}\u{fffd}"),
        (
            "d83dd83dde00",
            "\u{fffd}\u{fffd}",
            "\u{fffd}\u{fffd}",
            "\u{3dd8}\u{3dd8}\u{de}",
        ),
        ("", "", "", ""),
    ] {
        let data = hex_decode(hex).unwrap();
        assert_eq!(Charset::Utf16.decode(&data), utf16, "UTF-16 {hex}");
        assert_eq!(Charset::Utf16Be.decode(&data), be, "UTF-16BE {hex}");
        assert_eq!(Charset::Utf16Le.decode(&data), le, "UTF-16LE {hex}");
    }
    assert_eq!(
        Charset::Ascii.decode(&[0x41, 0x80, 0xFF]),
        "A\u{fffd}\u{fffd}"
    );
    assert_eq!(Charset::Latin1.decode(&[0x41, 0x80, 0xFF]), "A\u{80}\u{ff}");
    assert_top("'61e9' HEX-> 'ISO-8859-1' BYTES->", "a\u{e9}");
    assert_top("'61c3a9' HEX-> 'UTF-8' BYTES->", "a\u{e9}");
    assert_top("'feff0041' HEX-> 'UTF-16' BYTES->", "A");
    assert_fails(
        "'61' HEX-> 'UTF-32' BYTES->",
        "BYTES-> unsupported encoding 'UTF-32' (UTF-32)",
    );
    assert_fails(
        "'61' HEX-> 1 BYTES->",
        "BYTES-> operates on a byte array and expects a charset name on top of the stack.",
    );
    assert_fails("'61' 'UTF-8' BYTES->", "BYTES-> operates on a byte array.");
    assert_fails("BYTES->", "Empty stack.");
    assert_fails("'UTF-8' BYTES->", "Empty stack.");
}

// -----------------------------------------------------------------------------------------
// Regular expressions
// -----------------------------------------------------------------------------------------

#[test]
fn match_groups() {
    assert_top(
        "'ab' '(a)(x)?(b)' MATCH",
        Value::list(vec![
            Value::from("ab"),
            Value::from("a"),
            Value::Null,
            Value::from("b"),
        ]),
    );
    assert_top("'ab' 'a|ab' MATCH", strings(&["ab"]));
    assert_top("'abc' 'ab' MATCH", strings(&[]));
    assert_top("'abc%0A' 'abc$' MATCH", strings(&[]));
    assert_top("'a%0Ac' 'a.c' MATCH", strings(&[]));
    assert_top("'' '' MATCH", strings(&[""]));
    assert_top("'abc' '(?i)ABC' MATCH", strings(&["abc"]));
    assert_top(
        "'2024-01' '(\\d{4})-(\\d{2})' MATCH",
        strings(&["2024-01", "2024", "01"]),
    );
    assert_top("'abab' '(ab)\\1' MATCH", strings(&["abab", "ab"]));
    assert_top("'a.b' 'a\\Q.\\Eb' MATCH", strings(&["a.b"]));
    assert_top("'axb' 'a\\Q.\\Eb' MATCH", strings(&[]));
    assert_top("'a.b' 'a\\Q.' MATCH", strings(&[]));
    assert_top("'a.' 'a\\Q.' MATCH", strings(&["a."]));
    assert_top("'a\\Qb' 'a\\\\Qb' MATCH", strings(&["a\\Qb"]));
    assert_fails("'abc' '\\Qa\\Ebc\\' MATCH", "Parsing error");
    assert_top("'a\u{e9}' 'a\\\u{e9}' MATCH", strings(&["a\u{e9}"]));
    // A verbose pattern ending with a comment still works.
    assert_top("'abc' '(?x)abc #comment' MATCH", strings(&["abc"]));
    assert_top("'ab c' '(?x)ab\\ c #comment' MATCH", strings(&["ab c"]));
    assert_fails(
        "'abc' '(a' MATCH",
        "Parsing error at position 2: Opening parenthesis without closing parenthesis",
    );
    assert_fails("'abc' 'a)' MATCH", "Parsing error at position 1");
    assert_fails(
        "'abc' 1 MATCH",
        "MATCH regular expression MUST be a string, a compiled pattern or a matcher.",
    );
    assert_fails(
        "'abc' test.object MATCH",
        "MATCH regular expression MUST be a string, a compiled pattern or a matcher.",
    );
    assert_fails("1 'a' MATCH", "MATCH can only be applied to a string.");
    assert_fails("MATCH", "Empty stack.");
    assert_fails("'a' MATCH", "Empty stack.");
    // The backtracking limit of the engine is reported as an error.
    assert_fails(
        "'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' '(a|a)*\\1b' MATCH",
        "Max limit for backtracking count exceeded",
    );
}

#[test]
fn matcher_objects() {
    assert_top("'ab' '(a)(b)' MATCHER MATCH", strings(&["ab", "a", "b"]));
    let matcher = top("'a(b)c' MATCHER");
    assert_eq!(matcher.type_name(), "MATCHER");
    assert_eq!(matcher.to_string(), "'a(b)c' MATCHER");
    assert_eq!(value_snapshot(&matcher), "'a(b)c' MATCHER ");
    assert_top("'a(b)c' MATCHER TYPEOF", "MATCHER");
    // The same matcher can be used several times.
    assert_stack(
        "'a(b)c' MATCHER 'm' STORE 'abc' $m MATCH 'xyz' $m MATCH",
        &[strings(&["abc", "b"]), strings(&[])],
    );
    let compiled = CompiledPattern::compile("x+").unwrap();
    assert_eq!(compiled.name(), "x+");
    assert_eq!(compiled.source(), "x+");
    assert!(compiled.as_any().is_some());
    assert_eq!(compiled.full_match("xx").unwrap(), vec![Value::from("xx")]);
    assert_eq!(compiled.full_match("xy").unwrap(), Vec::<Value>::new());
    assert_fails("1 MATCHER", "MATCHER expects a STRING.");
    assert_fails("'(' MATCHER", "Parsing error at position 1");
    assert_fails("MATCHER", "Empty stack.");
    // A matcher is only equal to itself.
    assert_stack("'a' MATCHER DUP ==", &[Value::from(true)]);
    assert_stack("'a' MATCHER 'a' MATCHER ==", &[Value::from(false)]);
}

#[test]
fn replace_all() {
    // Reference values from String.replaceAll / replaceFirst on JDK 27.
    assert_top("'hello world' 'o' '0' REPLACEALL", "hell0 w0rld");
    assert_top("'hello world' 'o' '0' REPLACE", "hell0 world");
    assert_top("'a.b' '.' 'x' REPLACEALL", "xxx");
    assert_top("'a.b' '.' 'x' REPLACE", "x.b");
    assert_top("'a.b' '\\Q.\\E' 'x' REPLACEALL", "axb");
    assert_top("'abc' '(b)' '[$1]' REPLACEALL", "a[b]c");
    assert_top("'abc' '(b)' '\\$1' REPLACEALL", "a$1c");
    assert_top("'abc' '(b)' '\\\\' REPLACEALL", "a\\c");
    assert_top("'abc' '(?<x>b)' '${x}${x}' REPLACEALL", "abbc");
    assert_top("'abc' '(b)' '$10' REPLACEALL", "ab0c");
    assert_top(
        "'abcdefghijk' '(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)(k)' '$11$1' REPLACEALL",
        "ka",
    );
    assert_top("'abc' '(x)?b' '[$1]' REPLACEALL", "a[]c");
    assert_top("'abc' 'x' '$' REPLACEALL", "abc");
    assert_top("'abc' '' '-' REPLACEALL", "-a-b-c-");
    assert_top("'abc' '' '-' REPLACE", "-abc");
    assert_top("'aaa' 'a*' '-' REPLACEALL", "--");
    assert_top("'aaa' 'a*' '-' REPLACE", "-");
    assert_top("'a\u{e9}b' '' '-' REPLACEALL", "-a-\u{e9}-b-");
    assert_top("'xax' 'a' '\u{e9}' REPLACE", "x\u{e9}x");
    assert_top("'aaa' '^a' '-' REPLACEALL", "-aa");
    assert_top("'ab' '(?<n>a)(b)' '${n}$2$1' REPLACEALL", "aba");
    assert_top("'' 'a' 'b' REPLACEALL", "");
    assert_top("'' '' 'b' REPLACEALL", "b");
    // Any operand is converted with toString().
    assert_top("123 2 'x' REPLACEALL", "1x3");
    assert_top("1.5 '\\.' 1 REPLACE", "115");
    for (replacement, message) in [
        ("[$2]", "No group 2"),
        ("$", "Illegal group reference: group index is missing"),
        ("\\", "character to be escaped is missing"),
        ("$$", "Illegal group reference"),
        ("$a", "Illegal group reference"),
        ("${y}", "No group with name {y}"),
        ("${", "named capturing group has 0 length name"),
        ("${x", "named capturing group is missing trailing '}'"),
        (
            "${1x}",
            "capturing group name {1x} starts with digit character",
        ),
    ] {
        assert_eq!(
            regex_replace("abc", "(?<x>b)", replacement, true)
                .unwrap_err()
                .to_string(),
            message,
            "{replacement}"
        );
        assert_eq!(
            regex_replace("abc", "(?<x>b)", replacement, false)
                .unwrap_err()
                .to_string(),
            message,
            "{replacement}"
        );
    }
    assert_fails("'abc' '(' 'x' REPLACEALL", "Parsing error at position 1");
    assert_fails("'abc' '(' 'x' REPLACE", "Parsing error at position 1");
    assert_fails(
        "'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' '(a|a)*\\1b' 'x' REPLACEALL",
        "Max limit for backtracking count exceeded",
    );
    assert_fails(
        "'abc' 'b' NULL REPLACEALL",
        "REPLACEALL cannot operate on NULL.",
    );
    assert_fails("NULL 'b' 'x' REPLACE", "REPLACE cannot operate on NULL.");
    assert_fails("REPLACE", "Empty stack.");
    assert_fails("'x' REPLACE", "Empty stack.");
    assert_fails("'b' 'x' REPLACEALL", "Empty stack.");
}

#[test]
fn almost_equal() {
    assert_top("1.0 1.1 0.2 ~=", true);
    assert_top("1.0 1.1 -0.2 ~=", true);
    assert_top("1.0 1.5 0.2 ~=", false);
    assert_top("1 2 1 ~=", true);
    assert_top("1 3 1 ~=", false);
    assert_top("NaN NaN 1 ~=", true);
    assert_top("NaN 1.0 1 ~=", false);
    assert_top("1.0 NaN 1 ~=", false);
    assert_fails("1 2 'x' ~=", "~= only accepts numeric parameters.");
    assert_fails("1 'x' 1 ~=", "~= only accepts numeric parameters.");
    assert_fails("'x' 1 1 ~=", "~= only accepts numeric parameters.");
    assert_fails("1 1 ~=", "Empty stack.");
    assert_fails("1 ~=", "Empty stack.");
}

#[test]
fn reoptalt() {
    // Reference values from REOPTALT.buildOptAlt (Warp 10).
    for (alternatives, expected) in [
        (&[][..], ""),
        (&["a"][..], "a"),
        (&["a", "a"][..], "[a]"),
        (&["", "a"][..], "(|a)"),
        (&["a", "b"][..], "[ab]"),
        (&["a", "b", "c"][..], "[abc]"),
        (&["ab", "ac"][..], "(a[bc])"),
        (&["abc", "abd"][..], "(ab[cd])"),
        (&["abc", "xyz"][..], "(abc|xyz)"),
        (&["foo", "foobar", "foobaz"][..], "(foo(|ba[rz]))"),
        (&["a.b", "a*b"][..], "(a(\\*|\\.)b)"),
        (
            &["(x)", "[y]", "{z}", "|", "?", "+", "\\"][..],
            "(\\(x\\)|\\+|\\?|\\[y\\]|\\|\\{z\\}|\\|)",
        ),
        (&["a-", "b-"][..], "(a-|b-)"),
        (&["-", "a"][..], "(-|a)"),
        (&["abc", "xbc", "ybc"][..], "(abc|xbc|ybc)"),
        (&["abcd", "xbcd", "abce"][..], "(abc[de]|xbcd)"),
        (&["a", "ab", "abc"][..], "(a(|b(|c)))"),
        (&["", ""][..], ""),
        (&["", "a", "b"][..], "(|a|b)"),
        (&["cat", "car", "cab", "dog"][..], "(ca[brt]|dog)"),
        (&["aaa", "aab", "aba", "abb"][..], "(a(a[ab]|b[ab]))"),
        (&["x1y", "x2y", "x3y"][..], "(x[123]y)"),
        (&["ab", "a"][..], "(a(|b))"),
        (&["prefix1", "prefix2"][..], "(prefix[12])"),
        (
            &["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"][..],
            "[0123456789]",
        ),
        (&["ba", "ca", "da"][..], "(ba|ca|da)"),
        (&["ba", "ca", "d"][..], "(ba|ca|d)"),
        (&["\u{e9}", "\u{e8}"][..], "[\u{e8}\u{e9}]"),
        (&["aXb", "aYb", "aZb", "c"][..], "(a[XYZ]b|c)"),
    ] {
        let list: Vec<String> = alternatives.iter().map(|s| format!("'{s}'")).collect();
        let script = format!("[ {} ] REOPTALT", list.join(" "));
        assert_top(&script, expected);
    }
    // Elements are converted with String.valueOf.
    assert_top("[ 1 2 NULL ] REOPTALT", "(1|2|null)");
    assert_fails(
        "'a' REOPTALT",
        "REOPTALT expects a list of strings on top of the stack.",
    );
    assert_fails("REOPTALT", "Empty stack.");
    // The generated expressions match exactly the alternatives.
    assert_top(
        "'foobaz' [ 'foo' 'foobar' 'foobaz' ] REOPTALT MATCH",
        strings(&["foobaz", "foobaz", "baz"]),
    );
    assert_top(
        "'foob' [ 'foo' 'foobar' 'foobaz' ] REOPTALT MATCH",
        strings(&[]),
    );
}

// -----------------------------------------------------------------------------------------
// Bit sets
// -----------------------------------------------------------------------------------------

#[test]
fn bitsets() {
    // Reference values from java.util.BitSet on JDK 27.
    assert_top("'050080' HEX-> BYTESTOBITS 0 BITGET", true);
    assert_top("'050080' HEX-> BYTESTOBITS 1 BITGET", false);
    assert_top("'050080' HEX-> BYTESTOBITS 23 BITGET", true);
    assert_top("'050080' HEX-> BYTESTOBITS 100 BITGET", false);
    assert_top("'050080' HEX-> BYTESTOBITS 4294967296 BITGET", true);
    assert_fails("'050080' HEX-> BYTESTOBITS -1 BITGET", "bitIndex < 0: -1");
    assert_stack(
        "'050080' HEX-> BYTESTOBITS BITCOUNT",
        &[Value::Long(24), Value::Long(3)],
    );
    assert_stack(
        "'' HEX-> BYTESTOBITS BITCOUNT",
        &[Value::Long(0), Value::Long(0)],
    );
    assert_stack(
        "'000000000000000001' HEX-> BYTESTOBITS BITCOUNT",
        &[Value::Long(65), Value::Long(1)],
    );
    assert_top("'05008000' HEX-> BYTESTOBITS BITSTOBYTES", bytes("050080"));
    assert_top("'0000' HEX-> BYTESTOBITS BITSTOBYTES", bytes(""));
    assert_top("'050080' HEX-> BYTESTOBITS TYPEOF", "BITSET");
    assert_top("'0500' HEX-> BYTESTOBITS '05' HEX-> BYTESTOBITS ==", true);
    let set = top("'050080' HEX-> BYTESTOBITS");
    assert_eq!(set.to_string(), "{0, 2, 23}");
    let snapshot = value_snapshot(&set);
    assert_eq!(
        snapshot,
        format!(
            "'{}' OPB64-> BYTESTOBITS ",
            opb64_encode(&[0x05, 0x00, 0x80])
        )
    );
    assert_eq!(exec(&snapshot).pop().unwrap(), set);
    assert_fails(
        "'050080' HEX-> BYTESTOBITS 'x' BITGET",
        "BITGET expects a bit index on top of the stack.",
    );
    assert_fails(
        "'x' 1 BITGET",
        "BITGET expects a bit set below the bit index.",
    );
    assert_fails(
        "'x' BITCOUNT",
        "BITCOUNT expects a bit set below the bit index.",
    );
    assert_fails(
        "'x' BITSTOBYTES",
        "BITSTOBYTES expects a bit set on top of the stack.",
    );
    assert_fails(
        "'x' BYTESTOBITS",
        "BYTESTOBITS expects a byte array on top of the stack.",
    );
    assert_fails("BITGET", "Empty stack.");
    assert_fails("1 BITGET", "Empty stack.");
    assert_fails("BITCOUNT", "Empty stack.");
    assert_fails("BITSTOBYTES", "Empty stack.");
    assert_fails("BYTESTOBITS", "Empty stack.");
    let set = Value::BitSet(Rc::new(BitSet::from_bytes(&[1])));
    assert_eq!(set.to_string(), "{0}");
}

// -----------------------------------------------------------------------------------------
// ->LONGBYTES, varints, RLP
// -----------------------------------------------------------------------------------------

#[test]
fn tolongbytes() {
    const MESSAGE: &str = "->LONGBYTES operates on a LONG or a list of LONG and expects a number of output bytes per LONG between 1 and 8 on top of the stack.";
    assert_top("258 8 ->LONGBYTES", bytes("0000000000000102"));
    assert_top("258 2 ->LONGBYTES", bytes("0102"));
    assert_top("258 1 ->LONGBYTES", bytes("02"));
    assert_top("-1 3 ->LONGBYTES", bytes("ffffff"));
    assert_top("-1 4294967300 ->LONGBYTES", bytes("ffffffff"));
    assert_top("[ 258 -1 ] 2 ->LONGBYTES", bytes("0102ffff"));
    assert_top(
        "[ 258 -1 ] 8 ->LONGBYTES",
        bytes("0000000000000102ffffffffffffffff"),
    );
    assert_top("[ 258 3 ] 4 ->LONGBYTES", bytes("0000010200000003"));
    assert_top("[ 258 3 ] 1 ->LONGBYTES", bytes("0203"));
    assert_top("[ 258 3 ] 3 ->LONGBYTES", bytes("000102000003"));
    assert_top("[ ] 3 ->LONGBYTES", bytes(""));
    assert_fails("1 0 ->LONGBYTES", MESSAGE);
    assert_fails("1 9 ->LONGBYTES", MESSAGE);
    assert_fails("1 -1 ->LONGBYTES", MESSAGE);
    assert_fails("'x' 1 ->LONGBYTES", MESSAGE);
    assert_fails("1 'x' ->LONGBYTES", MESSAGE);
    assert_fails(
        "[ 1 'x' ] 1 ->LONGBYTES",
        "(Invalid element type STRING, only LONG elements are supported.)",
    );
    assert_fails("[ 1 'x' ] 1 ->LONGBYTES", MESSAGE);
    assert_fails("->LONGBYTES", "Empty stack.");
    assert_fails("1 ->LONGBYTES", "Empty stack.");
}

#[test]
fn varints() {
    assert_top("0 ->VARINT", bytes("00"));
    assert_top("127 ->VARINT", bytes("7f"));
    assert_top("128 ->VARINT", bytes("8001"));
    assert_top("300 ->VARINT", bytes("ac02"));
    assert_top("-1 ->VARINT", bytes("ffffffffffffffffff01"));
    assert_top("[ 1 300 ] ->VARINT", bytes("01ac02"));
    assert_top("[ ] ->VARINT", bytes(""));
    assert_fails(
        "[ 1 'x' ] ->VARINT",
        "->VARINT operates on a LIST of LONGs.",
    );
    assert_fails(
        "'x' ->VARINT",
        "->VARINT operates on a LONG or LIST of LONGs.",
    );
    assert_fails("->VARINT", "Empty stack.");
    assert_top(
        "'01ac02ffffffffffffffffff01' HEX-> VARINT->",
        Value::list(vec![Value::Long(1), Value::Long(300), Value::Long(-1)]),
    );
    assert_top("'' HEX-> VARINT->", Value::list(vec![]));
    assert_stack(
        "'01ac0203' HEX-> 2 VARINT->",
        &[
            Value::list(vec![Value::Long(1), Value::Long(300)]),
            Value::Long(3),
        ],
    );
    assert_stack(
        "'01ac0203' HEX-> 0 VARINT->",
        &[Value::list(vec![]), Value::Long(0)],
    );
    assert_stack(
        "'01' HEX-> 5 VARINT->",
        &[Value::list(vec![Value::Long(1)]), Value::Long(1)],
    );
    assert_fails(
        "'80' HEX-> VARINT->",
        "VARINT-> error while decoding values. (Buffer underflow.)",
    );
    assert_fails(
        "'ffffffffffffffffffff01' HEX-> VARINT->",
        "VARINT-> error while decoding values. (Variable length quantity is too long)",
    );
    assert_fails("'x' VARINT->", "VARINT-> operates on a byte array.");
    assert_fails("'x' 1 VARINT->", "VARINT-> operates on a byte array.");
    assert_fails("VARINT->", "Empty stack.");
    assert_fails("1 VARINT->", "Empty stack.");
}

#[test]
fn rlp_encoding() {
    assert_top("'dog' ->RLP", bytes("83646f67"));
    assert_top("[ 'cat' 'dog' ] ->RLP", bytes("c88363617483646f67"));
    assert_top("'' ->RLP", bytes("80"));
    assert_top("[ ] ->RLP", bytes("c0"));
    assert_top("0 ->RLP", bytes("80"));
    assert_top("15 ->RLP", bytes("0f"));
    assert_top("1024 ->RLP", bytes("820400"));
    assert_top("-1 ->RLP", bytes("88ffffffffffffffff"));
    assert_top("'7f' HEX-> ->RLP", bytes("7f"));
    assert_top("'80' HEX-> ->RLP", bytes("8180"));
    assert_top(
        "[ [ ] [ [ ] ] [ [ ] [ [ ] ] ] ] ->RLP",
        bytes("c7c0c1c0c3c0c1c0"),
    );
    // Long payloads carry their length explicitly.
    let long = "a".repeat(56);
    assert_top(
        &format!("'{long}' ->RLP"),
        bytes(&format!("b838{}", "61".repeat(56))),
    );
    let longer = "a".repeat(300);
    assert_top(
        &format!("'{longer}' ->RLP"),
        bytes(&format!("b9012c{}", "61".repeat(300))),
    );
    assert_top(
        &format!("[ '{long}' ] ->RLP"),
        bytes(&format!("f83ab838{}", "61".repeat(56))),
    );
    assert_fails(
        "1.5 ->RLP",
        "->RLP encountered an error while encoding RLP. (Invalid item type, only LIST, LONG, BYTES and STRING are supported.)",
    );
    assert_fails(
        "[ 1.5 ] ->RLP",
        "->RLP encountered an error while encoding RLP. (Invalid item type, only LIST, LONG, BYTES and STRING are supported.)",
    );
    assert_fails("->RLP", "Empty stack.");
}

#[test]
fn rlp_decoding() {
    assert_top("'83646f67' HEX-> RLP->", Value::bytes(b"dog"));
    assert_top(
        "'c88363617483646f67' HEX-> RLP->",
        Value::list(vec![Value::bytes(b"cat"), Value::bytes(b"dog")]),
    );
    assert_top("'80' HEX-> RLP->", bytes(""));
    assert_top("'c0' HEX-> RLP->", Value::list(vec![]));
    assert_top("'0f' HEX-> RLP->", bytes("0f"));
    assert_top("'8180' HEX-> RLP->", bytes("80"));
    assert_top(
        "'c7c0c1c0c3c0c1c0' HEX-> RLP->",
        Value::list(vec![
            Value::list(vec![]),
            Value::list(vec![Value::list(vec![])]),
            Value::list(vec![
                Value::list(vec![]),
                Value::list(vec![Value::list(vec![])]),
            ]),
        ]),
    );
    // Trailing bytes are ignored.
    assert_top("'0fff' HEX-> RLP->", bytes("0f"));
    let long = "61".repeat(56);
    assert_top(&format!("'b838{long}' HEX-> RLP->"), bytes(&long));
    assert_top(
        &format!("'f83ab838{long}' HEX-> RLP->"),
        Value::list(vec![bytes(&long)]),
    );
    let longer = "61".repeat(300);
    assert_top(&format!("'b9012c{longer}' HEX-> RLP->"), bytes(&longer));
    assert_top(
        "'c2808080' HEX-> RLP->",
        Value::list(vec![bytes(""), bytes("")]),
    );
    assert_top("'83646f67' HEX-> RLP-> 'UTF-8' BYTES->", "dog");
    assert_fails(
        "'' HEX-> RLP->",
        "RLP-> encountered an error while decoding RLP. (Index 0 out of bounds for length 0)",
    );
    assert_fails(
        "'83' HEX-> RLP->",
        "RLP-> encountered an error while decoding RLP. (Index 3 out of bounds for length 1)",
    );
    assert_fails(
        "'8361' HEX-> RLP->",
        "RLP-> encountered an error while decoding RLP. (Index 3 out of bounds for length 2)",
    );
    assert_fails(
        "'b8' HEX-> RLP->",
        "RLP-> encountered an error while decoding RLP. (Index 1 out of bounds for length 1)",
    );
    assert_fails(
        "'b83861' HEX-> RLP->",
        "RLP-> encountered an error while decoding RLP. (Index 57 out of bounds for length 3)",
    );
    assert_fails(
        "'c2' HEX-> RLP->",
        "RLP-> encountered an error while decoding RLP. (Index 1 out of bounds for length 1)",
    );
    assert_fails(
        "'f83a' HEX-> RLP->",
        "RLP-> encountered an error while decoding RLP. (Index 2 out of bounds for length 2)",
    );
    assert_fails(
        "'bc0100000000' HEX-> RLP->",
        "RLP-> encountered an error while decoding RLP. (Unable to support size over 2**32.)",
    );
    assert_fails(
        "'fc0100000000' HEX-> RLP->",
        "RLP-> encountered an error while decoding RLP. (Unable to support size over 2**32.)",
    );
    assert_fails("'x' RLP->", "RLP-> operates on a byte array.");
    assert_fails("RLP->", "Empty stack.");
    // Round trips.
    assert_top(
        "[ 'cat' [ 1 2 ] '' 'ff' HEX-> ] ->RLP RLP->",
        Value::list(vec![
            Value::bytes(b"cat"),
            Value::list(vec![bytes("01"), bytes("02")]),
            bytes(""),
            bytes("ff"),
        ]),
    );
}

#[test]
fn empty_stack() {
    let mut stack = Stack::new();
    for name in [
        "TOUPPER",
        "URLENCODE",
        "URLDECODE",
        "B64TOHEX",
        "HEXTOB64",
        "BINTOHEX",
        "HEXTOBIN",
        "BIN->",
        "HEX->",
        "B64->",
        "B64URL->",
        "->B64URL",
        "->OPB64",
        "OPB64->",
        "OPB64TOHEX",
        "~=",
        "->RLP",
    ] {
        assert_eq!(
            crate::functions::call(&mut stack, name)
                .unwrap_err()
                .to_string(),
            "Empty stack.",
            "{name}"
        );
    }
}
