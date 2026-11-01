#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use indexmap::IndexSet;

use crate::functions::call;
use crate::stack::Stack;
use crate::tests::{assert_fails, assert_top, exec, top};
use crate::value::Value;

fn long(l: i64) -> Value {
    Value::Long(l)
}

/// Run `name` on a stack pre-loaded with `inputs` (bottom first) and return the top result.
fn apply(name: &str, inputs: &[Value]) -> Value {
    let mut stack = Stack::new();
    for value in inputs {
        stack.push(value.clone());
    }
    call(&mut stack, name).unwrap();
    stack.pop().unwrap()
}

#[test]
fn addition() {
    assert_top("1 2 +", 3);
    assert_top("1 2.5 +", Value::Double(3.5));
    assert_top("2.5 1 +", Value::Double(3.5));
    assert_top("1.5 2.5 +", Value::Double(4.0));
    assert_top("'a' 'b' +", "ab");
    assert_top("9223372036854775807 1 +", i64::MIN);
    assert_top("[ 1 2 ] 3 +", Value::list(vec![long(1), long(2), long(3)]));
    assert_top("( 1 ) 2 +", Value::set(IndexSet::from([long(1), long(2)])));
    assert_eq!(top("<% 1 %> <% 2 %> +").to_string(), "<% 1 2 %>");
    // Concatenating macros propagates the secure flag.
    let secure = top("<% 1 %>");
    if let Value::Macro(m) = &secure {
        m.set_secure_recursive(true);
    }
    let result = apply("+", &[secure, top("<% 2 %>")]);
    assert!(
        matches!(&result, Value::Macro(m) if m.is_secure()),
        "{result}"
    );
    // `+` on a list returns a NEW list (the original is unchanged).
    let stack = exec("[ 1 ] 'l' STORE $l 2 + DROP $l");
    assert_eq!(stack.elements(), &[Value::list(vec![long(1)])]);
    assert_fails("1 'a' +", "+ can only operate on numeric, string, lists");
    assert_fails("+", "Empty stack.");
    assert_fails("1 +", "Empty stack.");
}

#[test]
fn byte_arrays() {
    // BYTES values are not produced by any ported function yet, so exercise the byte-array
    // branches of `+`, the comparisons and `TOLONG` through the direct API.
    let a = Value::bytes([1u8, 2]);
    let b = Value::bytes([3u8]);
    assert_eq!(
        apply("+", &[a.clone(), b.clone()]),
        Value::bytes([1u8, 2, 3])
    );
    assert_eq!(apply("==", &[a.clone(), a.clone()]), Value::Boolean(true));
    assert_eq!(apply("==", &[a.clone(), b.clone()]), Value::Boolean(false));
    assert_eq!(apply("!=", &[a.clone(), b.clone()]), Value::Boolean(true));
    assert_eq!(apply("<", &[b.clone(), a.clone()]), Value::Boolean(false));
    assert_eq!(apply("<", &[a.clone(), b.clone()]), Value::Boolean(true));
    assert_eq!(
        apply("TOLONG", &[Value::bytes([0x01u8, 0x00])]),
        Value::Long(256)
    );
    assert_eq!(apply("TOLONG", &[Value::bytes([0xFFu8])]), Value::Long(-1));
    let mut stack = Stack::new();
    stack.push(Value::bytes([]));
    assert!(call(&mut stack, "TOLONG").is_err());
    let mut stack = Stack::new();
    stack.push(Value::bytes([0u8; 9]));
    assert!(call(&mut stack, "TOLONG").is_err());
}

#[test]
fn subtraction_multiplication_division() {
    assert_top("5 3 -", 2);
    assert_top("5.0 3 -", Value::Double(2.0));
    assert_top("2 3 *", 6);
    assert_top("2 3.0 *", Value::Double(6.0));
    assert_top("7 2 /", 3);
    assert_top("7.0 2 /", Value::Double(3.5));
    assert_top("7 2 %", 1);
    assert_top("7.5 2 %", Value::Double(1.5));
    assert_top("-9223372036854775808 1 -", i64::MAX);
    assert_fails("1 'a' -", "- can only operate on numeric values");
    assert_fails("1 'a' *", "* can only operate on numeric values");
    assert_fails("1 'a' /", "/ can only operate on numeric values");
    assert_fails("'a' 1 %", "% operates on numeric arguments.");
    assert_fails("5 0 /", "/ by zero");
    assert_fails("5 0 %", "/ by zero");
    assert_top("5.0 0 /", Value::Double(f64::INFINITY));
    for op in ["-", "*", "/", "%"] {
        assert_fails(op, "Empty stack.");
        assert_fails(&format!("1 {op}"), "Empty stack.");
    }
}

#[test]
fn power() {
    assert_top("2 3 **", 8);
    assert_top("2.0 3 **", Value::Double(8.0));
    assert_top("2 0.5 **", Value::Double(std::f64::consts::SQRT_2));
    assert_top(
        "[ 1 2 3 ] 2 **",
        Value::list(vec![long(1), long(4), long(9)]),
    );
    assert_top(
        "2 [ 1 2 3 ] **",
        Value::list(vec![long(2), long(4), long(8)]),
    );
    assert_fails("[ 'a' ] 2 **", "** can only operate");
    assert_fails("2 [ 'a' ] **", "** can only operate");
    assert_fails("'a' 2 **", "** can only operate");
    assert_fails("[ 1 ] 'a' **", "** can only operate");
    assert_fails("'a' [ 1 ] **", "** can only operate");
    assert_fails("'a' 'b' **", "** can only operate");
    assert_fails("**", "Empty stack.");
    assert_fails("1 **", "Empty stack.");
    assert_fails("[ 1 ] **", "Empty stack.");
}

#[test]
fn inplace_add() {
    let stack = exec("[ 1 ] 'l' STORE $l 2 +! DROP $l");
    assert_eq!(stack.elements(), &[Value::list(vec![long(1), long(2)])]);
    assert_top("( ) 1 +! 1 +!", Value::set(IndexSet::from([long(1)])));
    assert_fails("1 2 +!", "+! can only operate on lists and sets.");
    assert_fails("+!", "Empty stack.");
    assert_fails("1 +!", "Empty stack.");
}

#[test]
fn comparisons() {
    assert_top("1 2 <", true);
    assert_top("2 1 <", false);
    assert_top("1 1 <", false);
    assert_top("2 1 >", true);
    assert_top("1 1 <=", true);
    assert_top("1 2 <=", true);
    assert_top("2 1 >=", true);
    assert_top("1 2 >=", false);
    assert_top("10 10.0 ==", true);
    assert_top("10 10.5 <", true);
    assert_top("10.5 10 >", true);
    assert_top("1 2.5 <", true);
    assert_top("-1 -1.5 >", true);
    assert_top("-1.5 -1 <", true);
    // Mixed LONG / DOUBLE comparisons are exact: no precision loss when the long is huge...
    assert_top("9223372036854775807 9223372036854775807.0 <", true);
    assert_top("9007199254740993 9007199254740992.0 >", true);
    // ...and doubles outside the long range sort beyond every long.
    assert_top("1 1e19 <", true);
    assert_top("1 -1e19 >", true);
    assert_top("1e19 1 >", true);
    assert_top("-1e19 1 <", true);
    assert_top("'a' 'b' <", true);
    assert_top("'b' 'a' >", true);
    assert_top("-0.0 0.0 <", true);
    assert_top("-0.0 0.0 ==", false);
    assert_fails(
        "true false <",
        "< can only operate when GTS values and the top stack operand have the same type.",
    );
    assert_fails("1 'a' <", "< can only operate");
    for op in ["<", ">", "<=", ">=", "==", "!="] {
        assert_fails(op, "Empty stack.");
        assert_fails(&format!("1 {op}"), "Empty stack.");
    }
}

#[test]
fn comparison_with_nan() {
    assert_top("NaN NaN ==", true);
    assert_top("NaN NaN <=", true);
    assert_top("NaN NaN >=", true);
    assert_top("NaN NaN !=", false);
    assert_top("NaN 1 ==", false);
    assert_top("1 NaN ==", false);
    assert_top("NaN 1 !=", true);
    assert_top("1 NaN !=", true);
    assert_top("NaN 1 <", false);
    assert_top("NaN 1 >", false);
    assert_top("NaN NaN <", false);
}

#[test]
fn equality() {
    assert_top("1 1 ==", true);
    assert_top("1 2 ==", false);
    assert_top("1 2 !=", true);
    assert_top("'a' 'a' ==", true);
    assert_top("'a' 'b' ==", false);
    assert_top("'a' 'b' !=", true);
    assert_top("true true ==", true);
    assert_top("true false !=", true);
    assert_top("1 'a' ==", false);
    assert_top("1 'a' !=", true);
    assert_top("NULL NULL ==", true);
    assert_top("NULL 1 ==", false);
    assert_top("[ 1 2 ] [ 1 2 ] ==", true);
    assert_top("[ 1 2 ] [ 1 3 ] ==", false);
    assert_top("[ 1 2 ] DUP ==", true);
}

#[test]
fn logical() {
    assert_top("true true &&", true);
    assert_top("true false &&", false);
    assert_top("false true &&", false);
    assert_top("true false ||", true);
    assert_top("false false ||", false);
    assert_top("true true AND", true);
    assert_top("false true OR", true);
    assert_top("[ true true true ] AND", true);
    assert_top("[ true false true ] AND", false);
    assert_top("[ false false ] OR", false);
    assert_top("[ false true ] OR", true);
    assert_top("[ ] AND", true);
    assert_top("[ ] OR", false);
    assert_top("[ <% true %> <% false %> ] AND", false);
    assert_top("[ <% false %> <% true %> ] OR", true);
    assert_fails("1 true &&", "AND can only operate on two boolean values");
    assert_fails("[ 1 ] AND", "AND can only operate on two boolean values");
    assert_fails(
        "[ <% 1 %> ] AND",
        "AND can only operate on two boolean values",
    );
    // A macro in the list that errors or leaves nothing propagates the failure.
    assert_fails("[ <% DROP %> ] AND", "Empty stack.");
    assert_fails("[ <% %> ] AND", "Empty stack.");
    assert_fails("&&", "Empty stack.");
    assert_fails("true &&", "Empty stack.");
    assert_fails("||", "Empty stack.");
    assert_fails("true ||", "Empty stack.");
}

#[test]
fn not() {
    assert_top("true !", false);
    assert_top("false NOT", true);
    assert_fails(
        "1 !",
        "! can only operate on a boolean value or a boolean GTS.",
    );
    assert_fails("!", "Empty stack.");
}

#[test]
fn bitwise() {
    assert_top("12 10 &", 8);
    assert_top("12 10 |", 14);
    assert_top("12 10 ^", 6);
    assert_top("1 4 <<", 16);
    assert_top("-8 1 >>", -4);
    assert_top("-8 1 >>>", 9_223_372_036_854_775_804);
    assert_top("0 ~", -1);
    assert_top("1 REVBITS", i64::MIN);
    assert_fails("1 'a' &", "& can only operate on two LONG");
    assert_fails("1.0 2 |", "| can only operate on two LONG");
    assert_fails("1.0 ~", "~ can only operate on a LONG.");
    assert_fails("1.0 REVBITS", "REVBITS can only operate on a LONG.");
    for op in ["&", "|", "^", "<<", ">>", ">>>"] {
        assert_fails(op, "Empty stack.");
        assert_fails(&format!("1 {op}"), "Empty stack.");
    }
    assert_fails("~", "Empty stack.");
    assert_fails("REVBITS", "Empty stack.");
}

#[test]
fn numeric_conversions() {
    assert_top("3 TODOUBLE", Value::Double(3.0));
    assert_top("true TODOUBLE", Value::Double(1.0));
    assert_top("false TODOUBLE", Value::Double(0.0));
    assert_top("'1.5' TODOUBLE", Value::Double(1.5));
    assert_top("'-2' TODOUBLE", Value::Double(-2.0));
    assert_top("'NaN' TODOUBLE TYPEOF", "DOUBLE");
    assert_top("'nan' TODOUBLE TYPEOF", "DOUBLE");
    assert_top("'Infinity' TODOUBLE", Value::Double(f64::INFINITY));
    assert_top("'-Infinity' TODOUBLE", Value::Double(f64::NEG_INFINITY));
    assert_fails("'abc' TODOUBLE", "For input string: \"abc\"");
    assert_fails(
        "[ ] TODOUBLE",
        "TODOUBLE can only operate on numeric, boolean or string values.",
    );
    assert_top("3.9 TOLONG", 3);
    assert_top("true TOLONG", 1);
    assert_top("false TOLONG", 0);
    assert_top("'42' TOLONG", 42);
    assert_top("'-42' TOLONG", -42);
    assert_top("'0xFF' TOLONG", 255);
    assert_top("'010' TOLONG", 8);
    assert_top("'#1F' TOLONG", 31);
    assert_fails("'abc' TOLONG", "For input string: \"abc\"");
    assert_fails("'0x' TOLONG", "For input string: \"0x\"");
    assert_fails(
        "[ ] TOLONG",
        "TOLONG can only operate on numeric, boolean, byte array or string values.",
    );
    assert_top("3 TOBOOLEAN", true);
    assert_top("0 TOBOOLEAN", false);
    assert_top("true TOBOOLEAN", true);
    assert_top("'x' TOBOOLEAN", true);
    assert_top("'' TOBOOLEAN", false);
    assert_fails(
        "[ ] TOBOOLEAN",
        "TOBOOLEAN can only operate on numeric, boolean or string values.",
    );
    assert_top("42 TOSTRING", "42");
    assert_top("1.5 TOSTRING", "1.5");
    assert_top("true TOSTRING", "true");
    assert_top("'x' TOSTRING", "x");
    assert_fails(
        "[ ] TOSTRING",
        "TOSTRING can only operate on numeric, boolean or string values.",
    );
    for f in ["TODOUBLE", "TOLONG", "TOBOOLEAN", "TOSTRING"] {
        assert_fails(f, "Empty stack.");
    }
}

#[test]
fn bit_representations() {
    assert_top("255 TOHEX", "00000000000000ff");
    assert_top("-1 ->HEX", "ffffffffffffffff");
    assert_top("5 TOBIN", format!("{:064b}", 5));
    assert_top("'ff' FROMHEX", 255);
    assert_top("'ffffffffffffffff' FROMHEX", -1);
    assert_top("'101' FROMBIN", 5);
    assert_fails("1.0 TOHEX", "TOHEX can only operate on long values.");
    assert_fails("1.0 TOBIN", "TOBIN can only operate on long values.");
    assert_fails(
        "1 FROMHEX",
        "FROMHEX can only operate on string of hexadecimal representations.",
    );
    assert_fails(
        "1 FROMBIN",
        "FROMBIN can only operate on string of binary representations.",
    );
    assert_fails(
        "'00000000000000000' FROMHEX",
        "FROMHEX can only operate on hexadecimal representations of 64 bits or less.",
    );
    assert_fails(
        "'00000000000000000000000000000000000000000000000000000000000000000' FROMBIN",
        "FROMBIN can only operate on binary representations of 64 bits or less.",
    );
    assert_fails("'gg' FROMHEX", "For input string: \"gg\"");
    assert_fails("'22' FROMBIN", "For input string: \"22\"");
    for f in ["TOHEX", "TOBIN", "FROMHEX", "FROMBIN"] {
        assert_fails(f, "Empty stack.");
    }
}

#[test]
fn ieee_bit_conversions() {
    assert_top("1.0 ->DOUBLEBITS", 4_607_182_418_800_017_408);
    assert_top("4607182418800017408 DOUBLEBITS->", Value::Double(1.0));
    assert_top("1 ->DOUBLEBITS DOUBLEBITS->", Value::Double(1.0));
    assert_top("1.0 ->FLOATBITS", i64::from(1.0f32.to_bits()));
    assert_top("1.0 ->FLOATBITS FLOATBITS->", Value::Double(1.0));
    assert_fails(
        "'x' ->DOUBLEBITS",
        "->DOUBLEBITS operates on a DOUBLE, LONG or Geo Time Series thereof.",
    );
    assert_fails(
        "1.0 DOUBLEBITS->",
        "DOUBLEBITS-> operates on a LONG or a Geo Time Series thereof.",
    );
    assert_fails(
        "'x' ->FLOATBITS",
        "->FLOATBITS operates on a DOUBLE, LONG or Geo Time Series thereof.",
    );
    assert_fails(
        "1.0 FLOATBITS->",
        "FLOATBITS-> operates on a LONG or a Geo Time Series thereof.",
    );
    for f in ["->DOUBLEBITS", "DOUBLEBITS->", "->FLOATBITS", "FLOATBITS->"] {
        assert_fails(f, "Empty stack.");
    }
}

#[test]
fn time_units() {
    assert_top("1 s", 1_000_000);
    assert_top("1 ms", 1000);
    assert_top("1 us", 1);
    assert_top("1 m", 60_000_000);
    assert_top("1 h", 3_600_000_000);
    assert_top("1 d", 86_400_000_000);
    assert_top("1 w", 604_800_000_000);
    assert_top("1000 ns", 1);
    assert_top("1000000 ps", 1);
    assert_top("1.5 s", 1_500_000);
    assert_fails("'x' s", "s can only operate on numeric values.");
    assert_fails("s", "Empty stack.");
}
