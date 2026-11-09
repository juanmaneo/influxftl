// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::rc::Rc;
use std::time::{Duration, Instant};

use indexmap::IndexMap;

use super::error::Error;
use super::macros::Macro;
use super::stack::{Limits, SecureCodec, SecureScript, Stack};
use super::tests::{assert_fails, assert_stack, assert_top, exec, fail, fail_msg, top};
use super::value::Value;

fn long(l: i64) -> Value {
    Value::Long(l)
}

#[test]
fn literals() {
    assert_stack(
        "1 -2 +3 1.5 -2.5e3 0x1F 0b101 0x-ff true false T F TRUE False NaN",
        &[
            long(1),
            long(-2),
            long(3),
            Value::Double(1.5),
            Value::Double(-2500.0),
            long(31),
            long(5),
            long(-255),
            Value::from(true),
            Value::from(false),
            Value::from(true),
            Value::from(false),
            Value::from(true),
            Value::from(false),
            Value::Double(f64::NAN),
        ],
    );
    assert_top("0xffffffffffffffff", -1);
    assert_fails("-0xff", "Unknown function '-0xff'");
    assert_fails("0xzz", "For input string: \"zz\"");
    assert_fails("1e", "For input string: \"1e\"");
    assert_fails("0b12", "For input string: \"12\"");
    assert_fails("99999999999999999999", "For input string");
}

#[test]
fn strings() {
    assert_stack(
        "'a' \"b c\" 'd%20e' 'it''s' '' \"x\"",
        &[
            Value::from("a"),
            Value::from("b c"),
            Value::from("d e"),
            Value::from("it''s"),
            Value::from(""),
            Value::from("x"),
        ],
    );
    assert_top("'trailing'   ", "trailing");
    assert_fails("'unterminated", "Cannot find end of string");
    assert_fails("'", "Cannot find end of string");
    assert_fails("'a'b", "Cannot find end of string");
    assert_fails("'%zz'", "Illegal hex characters");
}

#[test]
fn multiline_strings() {
    assert_top("<'\nline 1\n  line 2\n'>", "line 1\n  line 2");
    assert_top("<'\n'>", "");
    assert_fails(
        "<' x",
        "Can only start multiline strings by using <' on a line by itself.",
    );
    assert_fails("<'\nabc", "Unbalanced <' construct.");
}

#[test]
fn comments() {
    assert_stack("1 # comment 2", &[long(1)]);
    assert_stack("1 // comment 2", &[long(1)]);
    assert_stack("1 /* block */ 2", &[long(1), long(2)]);
    assert_stack("1 /* multi\nline */ 2", &[long(1), long(2)]);
    assert_stack("1 /* a */x b\n*/ 2", &[long(1), long(2)]);
    assert_stack("1 /* a */x b */ 2", &[long(1), long(2)]);
    assert_stack("1 /* a */", &[long(1)]);
    assert_stack("/*\n\n*/ 3", &[long(3)]);
    assert_stack("1 /* a */x b\n\n5 */ 2", &[long(1), long(2)]);
    assert_fails("1 */ 2", "Not inside a comment.");
    assert_fails("/* open", "Unbalanced /* construct.");
}

#[test]
fn symbols_and_early_binding() {
    assert_top("1 'a' STORE $a", 1);
    assert_fails("$nope", "Unknown symbol 'nope'");
    assert_top("2 'a' STORE !$a", 2);
    assert_top("2 'a' STORE <% !$a %> EVAL", 2);
    assert_fails("!$nope", "Unknown symbol 'nope'");
    assert_fails("<% !$nope %>", "Unknown symbol 'nope'");
    assert_fails(
        "<% <% !$nope %> %>",
        "Early binding is not possible inside a macro.",
    );
    assert_top("<% 'a' STORE %> 'm' STORE 5 @m $a", 5);
    assert_top("<% 'a' STORE %> 'm' STORE <% 6 @m %> EVAL $a", 6);
    assert_fails("@nope", "Unknown macro 'nope'");
    assert_fails("1 'x' STORE @x", "'x' is not a macro.");
    assert_fails("<% $late %> EVAL", "LOAD symbol 'late' does not exist.");
}

#[test]
fn functions_and_macros() {
    assert_stack("1 2 <% SWAP %> EVAL", &[long(2), long(1)]);
    assert_fails("NOPE", "Unknown function 'NOPE'");
    assert_fails("<% NOPE %>", "Unknown function 'NOPE'");
    assert_fails("%>", "Not inside a macro definition.");
    assert_fails("<% 1", "Unbalanced <% construct.");
    assert_eq!(top("<% 1 2 %>").to_string(), "<% 1 2 %>");
    assert_eq!(top("<% <% 1 %> %>").to_string(), "<% <% 1 %> %>");
    assert_eq!(top("test.object").type_name(), "OPERATOR");
    assert_eq!(
        top("<% test.object %>").to_string(),
        "<% 'test.object' FUNCREF %>"
    );
}

#[test]
fn redefinitions() {
    assert_top("<% DROP 42 %> 'DUP' DEF 1 DUP", 42);
    assert_fails("NULL 'DUP' DEF 1 DUP", "DUP is undefined.");
    assert_top("<% 7 %> 'SEVEN' DEF SEVEN", 7);
    assert_fails(
        "<% 1 %> 'SEVEN' DEF NULL 'SEVEN' DEF SEVEN",
        "SEVEN is undefined.",
    );
    assert_fails("1 'DUP' DEF", "DEF needs a macro below the statement name.");
    assert_fails("<% %> 1 DEF", "DEF expects statement name to be a string.");
    let mut stack = exec("<% 42 %> 'DUP' DEF");
    stack.set_allow_redefined(false);
    assert!(!stack.allow_redefined());
    let err = stack.exec("1 DUP").unwrap_err();
    assert!(
        err.to_string()
            .contains("Disallowed redefined function 'DUP'."),
        "{err}"
    );
}

#[test]
fn error_messages_locate_the_statement() {
    let msg = fail_msg("1 2 SWAP DROP DROP DROP");
    assert_eq!(
        msg,
        "Line #1 (Exception at '1 2 SWAP DROP DROP =>DROP<=' in section [TOP] (Empty stack.))"
    );
    let msg = fail_msg(
        "1\n'abcdefghijklmnopqrstuvwxyz0123456789' DROP DROP DROP 'abcdefghijklmnopqrstuvwxyz0123456789'",
    );
    assert_eq!(
        msg,
        "Line #2 (Exception at 'stuvwxyz0123456789' DROP DROP =>DROP<= 'abcdefghijklmnopqrstuvwxyz01' in section [TOP] (Empty stack.))"
    );
    let msg = fail_msg("<% DROP %> 'm' STORE @m");
    assert_eq!(
        msg,
        "Line #1 (Exception at '<% DROP %> 'm' STORE =>@m<=' in section [TOP] (Exception at 'DROP' in section '[TOP]' (Empty stack.)))"
    );
    // Macros loaded by name from a repository carry their name.
    let mut stack = exec("<% DROP %>");
    let Value::Macro(m) = stack.pop().unwrap() else {
        panic!("not a macro");
    };
    m.set_name_recursive("lib/m");
    stack.store("m", Value::Macro(m)).unwrap();
    let err = stack.exec("@m").unwrap_err();
    assert_eq!(
        err.to_string(),
        "Line #1 (Exception at '=>@m<=' in section [TOP] (Exception at 'DROP' in section '[TOP]' called from macro 'lib/m' (Empty stack.)))"
    );
    assert_eq!(stack.macro_name(), None);
    let msg = fail_msg("<% DROP %> EVAL");
    assert_eq!(
        msg,
        "Line #1 (Exception at '<% DROP %> =>EVAL<=' in section [TOP] (Exception at 'DROP' in section '[TOP]' (Empty stack.)))"
    );
    let msg = fail_msg("'sec' SECTION <% <% DROP %> EVAL %> EVAL");
    assert!(msg.contains("in section 'sec'"), "{msg}");
    assert!(
        msg.contains("Exception at 'EVAL' in section 'sec'"),
        "{msg}"
    );
}

#[test]
fn stack_limits() {
    let mut stack = Stack::new();
    stack.limits.max_depth = 2;
    assert!(stack.exec("1 2").is_ok());
    let err = stack.exec("3").unwrap_err();
    assert!(
        err.to_string().contains(
            "Stack depth would exceed set limit of 2. Consider raising the limit or using capabilities."
        ),
        "{err}"
    );
    let mut stack = Stack::new();
    stack.limits.max_ops = 3;
    let err = stack.exec("1 2 3 4").unwrap_err();
    assert!(
        err.to_string().contains(
            "Operation count (4) exceeded maximum of 3. Consider raising the limit or using capabilities."
        ),
        "{err}"
    );
    assert!(err.to_string().contains("'1 2 3 =><=4'"), "{err}");
    assert_eq!(stack.ops(), 4);
    stack.set_ops(0);
    assert_eq!(stack.ops(), 0);
    let mut stack = Stack::new();
    stack.limits.max_symbols = 1;
    assert!(stack.exec("1 'a' STORE 2 'a' STORE").is_ok());
    let err = stack.exec("3 'b' STORE").unwrap_err();
    assert!(
        err.to_string()
            .contains("Symbol table has reached its maximum number of entries: 1"),
        "{err}"
    );
    let mut stack = Stack::new();
    let err = stack.exec("<% 'm' RUN %> 'm' STORE 'm' RUN").unwrap_err();
    assert!(
        err.root_cause()
            .message()
            .starts_with("Maximum recursion level reached (17)"),
        "{err}"
    );
    assert_eq!(stack.recursion_level(), 0);
}

#[test]
fn depth_limit_is_checked_after_every_statement() {
    let mut stack = Stack::new();
    stack.limits.max_depth = 1;
    let err = stack.exec("1 DUP").unwrap_err();
    assert!(
        err.to_string()
            .contains("Stack depth would exceed set limit of 1."),
        "{err}"
    );
    let mut stack = Stack::new();
    stack.limits.max_depth = 0;
    assert!(stack.exec("'a'").is_err());
    let mut stack = Stack::new();
    stack.limits.max_depth = 0;
    assert!(stack.exec("<'\nx\n'>").is_err());
    let mut stack = Stack::new();
    stack.limits.max_depth = 1;
    assert!(stack.exec("<% 1 1 %> EVAL").is_err());
    let mut stack = Stack::new();
    stack.limits.max_depth = 1;
    assert!(stack.exec("<% 1 DUP %> EVAL").is_err());
}

#[test]
fn recursion_limit_applies_to_eval() {
    let mut stack = Stack::new();
    let err = stack.exec("'$s EVAL' 's' STORE $s EVAL").unwrap_err();
    assert!(
        err.root_cause()
            .message()
            .starts_with("Maximum recursion level reached"),
        "{err}"
    );
    assert_eq!(stack.recursion_level(), 0);
}

#[test]
fn ops_limit_inside_macros() {
    let mut stack = Stack::with_limits(Limits::unlimited());
    stack.exec("<% 1 2 3 %>").unwrap();
    let Value::Macro(m) = stack.pop().unwrap() else {
        panic!("not a macro");
    };
    stack.limits.max_ops = 2;
    stack.set_ops(0);
    let err = stack.exec_macro(&m).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Exception in section '[TOP]' (Operation count (4) exceeded maximum of 2. Consider raising the limit or using capabilities.)"
    );
}

#[test]
fn element_access() {
    let mut stack = Stack::new();
    assert_eq!(stack.depth(), 0);
    assert_eq!(stack.pop().unwrap_err().to_string(), "Empty stack.");
    assert_eq!(stack.peek().unwrap_err().to_string(), "Empty stack.");
    assert_eq!(stack.get(0).unwrap_err().to_string(), "Invalid level.");
    assert_eq!(
        stack.popn(1).unwrap_err().to_string(),
        "Index out of bound."
    );
    assert_eq!(
        stack.dropn(1).unwrap_err().to_string(),
        "Index out of bound."
    );
    stack.push(1i64);
    stack.push(2i64);
    stack.push(3i64);
    assert_eq!(stack.peek().unwrap(), &long(3));
    assert_eq!(stack.get(2).unwrap(), &long(1));
    assert_eq!(stack.elements(), &[long(1), long(2), long(3)]);
    stack.elements_mut()[0] = long(10);
    assert_eq!(stack.popn(2).unwrap(), vec![long(2), long(3)]);
    assert_eq!(stack.depth(), 1);
    stack.push(0i64);
    stack.dropn(1).unwrap();
    assert_eq!(stack.elements(), &[long(10)]);
    stack.reset(5);
    assert_eq!(stack.depth(), 1);
    stack.reset(0);
    assert_eq!(stack.depth(), 0);
    stack.push(1i64);
    stack.clear();
    assert_eq!(stack.depth(), 0);
    let stack = Stack::default();
    assert_eq!(stack.depth(), 0);
}

#[test]
fn hide_and_show() {
    let mut stack = Stack::new();
    for i in 0..5 {
        stack.push(i);
    }
    assert_eq!(stack.hide(0), 0);
    assert_eq!(stack.hide(2), 2);
    assert_eq!(stack.depth(), 3);
    assert_eq!(stack.elements(), &[long(2), long(3), long(4)]);
    stack.show(0);
    assert_eq!(stack.depth(), 3);
    stack.show(5);
    assert_eq!(stack.depth(), 5);
    assert_eq!(stack.hide(10), 5);
    assert_eq!(stack.depth(), 0);
    stack.show(-2);
    assert_eq!(stack.depth(), 2);
    stack.show(-1);
    assert_eq!(stack.depth(), 2);
    stack.show(-10);
    assert_eq!(stack.depth(), 5);
    stack.show(-3);
    assert_eq!(stack.depth(), 5);
    assert_eq!(stack.hide(-2), 3);
    assert_eq!(stack.depth(), 2);
    assert_eq!(stack.hide(-5), 0);
    assert_eq!(stack.depth(), 2);
    stack.show_all();
    assert_eq!(stack.depth(), 5);
    assert_eq!(stack.hide_all(), 5);
    assert_eq!(stack.depth(), 0);
    stack.show(1);
    assert_eq!(stack.depth(), 1);
    stack.show(-3);
    assert_eq!(stack.depth(), 3);
    stack.show_all();
    stack.hide(3);
    stack.reset(1);
    assert_eq!(stack.depth(), 1);
    stack.show_all();
    assert_eq!(stack.elements(), &[long(0), long(1), long(2), long(3)]);
}

#[test]
fn symbols_registers_and_context() {
    let mut stack = Stack::new();
    assert!(!stack.has_symbol("a"));
    stack.store("a", long(1)).unwrap();
    assert!(stack.has_symbol("a"));
    assert_eq!(stack.load("a"), Some(&long(1)));
    assert_eq!(stack.symbols().len(), 1);
    stack.symbols_mut().insert("b".to_owned(), long(2));
    stack.forget("a");
    assert_eq!(stack.load("a"), None);
    stack.forget_all();
    assert_eq!(stack.symbols().len(), 0);
    assert_eq!(stack.registers().len(), 256);
    stack.store_register(3, long(3)).unwrap();
    assert_eq!(stack.load_register(3).unwrap(), &long(3));
    assert_eq!(
        stack.load_register(256).unwrap_err().to_string(),
        "Invalid register number, must be between 0 and 255"
    );
    assert_eq!(
        stack
            .store_register(-1, Value::Null)
            .unwrap_err()
            .to_string(),
        "Invalid register number, must be between 0 and 255"
    );
    stack.define("X", Some(Rc::new(Macro::new())));
    let context = stack.save();
    assert_eq!(context.registers[3], long(3));
    assert_eq!(context.defined.len(), 1);
    stack.clear_registers();
    stack.define("Y", None);
    assert_eq!(stack.defined().len(), 2);
    stack.restore(&context);
    assert_eq!(stack.load_register(3).unwrap(), &long(3));
    assert_eq!(stack.defined().len(), 1);
}

#[test]
fn attributes_and_state() {
    let mut stack = Stack::new();
    assert_eq!(stack.attribute("k"), None);
    assert_eq!(stack.set_attribute("k", Some(long(1))), None);
    assert_eq!(stack.set_attribute("k", Some(long(2))), Some(long(1)));
    assert_eq!(stack.attribute("k"), Some(&long(2)));
    assert_eq!(stack.set_attribute("k", None), Some(long(2)));
    assert_eq!(stack.attribute("k"), None);
    assert_eq!(stack.section_name(), "[TOP]");
    stack.set_section_name("s");
    assert_eq!(stack.section_name(), "s");
    assert_eq!(stack.macro_name(), None);
    assert!(!stack.in_secure_macro());
    assert!(!stack.set_in_secure_macro(true));
    assert!(stack.in_secure_macro());
    assert_eq!(stack.deadline(), None);
    let now = Instant::now();
    assert_eq!(stack.set_deadline(Some(now)), None);
    assert_eq!(stack.deadline(), Some(now));
    assert_eq!(stack.last_error(), None);
    stack.set_last_error(Some(Error::usage("TEST", "e")));
    assert_eq!(
        stack.last_error().expect("an error was set").message(),
        "TEST e"
    );
    assert_eq!(stack.macro_depth(), 0);
    assert_eq!(stack.line_number(), None);
    stack.set_return_depth(2);
    assert_eq!(stack.decrement_return_depth(), 1);
    assert_eq!(stack.config_property("x"), None);
    stack.set_config(IndexMap::from([("x".to_owned(), "y".to_owned())]));
    assert_eq!(stack.config_property("x"), Some("y"));
    assert!(stack.secure_codec().is_none());
}

#[test]
fn line_numbers_are_tracked_while_executing() {
    let mut stack = Stack::new();
    stack.exec("1\n2").unwrap();
    assert_eq!(stack.line_number(), None);
    assert_eq!(stack.macro_depth(), 0);
    stack.exec("<%").unwrap();
    assert_eq!(stack.macro_depth(), 1);
    assert_eq!(
        stack.check_balanced().unwrap_err().to_string(),
        "Unbalanced <% construct."
    );
}

#[test]
fn stop_and_return_at_top_level() {
    assert_stack("1 STOP 2", &[long(1)]);
    assert_stack("1 RETURN 2", &[long(1)]);
    assert_stack("1 <% 2 RETURN 3 %> EVAL 4", &[long(1), long(2), long(4)]);
    assert_stack("1 <% 2 STOP 3 %> EVAL 4", &[long(1), long(2)]);
    assert_stack(
        "<% <% 1 2 NRETURN 9 %> EVAL 8 %> EVAL 7",
        &[long(1), long(7)],
    );
    assert_stack(
        "<% <% 1 1 NRETURN 9 %> EVAL 8 %> EVAL 7",
        &[long(1), long(8), long(7)],
    );
    let mut stack = Stack::new();
    assert_eq!(
        stack.exec_multi("1 STOP").unwrap_err(),
        Error::Stop("STOP".to_owned())
    );
    assert_eq!(stack.exec_multi("RETURN").unwrap_err(), Error::Return);
    assert_fails(
        "BREAK",
        "Line #1 (Exception at BREAK: must be used in a one of",
    );
    assert_fails(
        "CONTINUE",
        "Line #1 (Exception at CONTINUE: must be used in a one of",
    );
    assert_fails("<% BREAK %> EVAL", "Line #1 (Exception at BREAK:");
}

#[test]
fn secure_macros_do_not_wrap_errors() {
    let mut stack = Stack::new();
    stack.exec("<% DROP %>").unwrap();
    let Value::Macro(m) = stack.pop().unwrap() else {
        panic!("not a macro");
    };
    m.set_secure_recursive(true);
    let err = stack.exec_macro(&m).unwrap_err();
    assert_eq!(err.to_string(), "Empty stack.");
    assert!(!stack.in_secure_macro());
}

#[test]
fn forced_macros() {
    let mut stack = Stack::new();
    stack.macro_close().unwrap();
    stack.macro_open().unwrap();
    assert_eq!(
        stack.macro_open().unwrap_err().to_string(),
        "Already in a forced Macro."
    );
    stack.exec("1 2 <% 3 %>").unwrap();
    assert_eq!(stack.depth(), 0);
    let err = stack.exec("%>").unwrap_err();
    assert!(
        err.to_string().contains("Not inside a macro definition."),
        "{err}"
    );
    stack.exec("<%").unwrap();
    assert_eq!(
        stack.macro_close().unwrap_err().to_string(),
        "Invalid level for closing forced Macro, check that all macros are correctly closed."
    );
    stack.exec("%>").unwrap();
    stack.exec("<'").unwrap();
    assert_eq!(
        stack.macro_close().unwrap_err().to_string(),
        "Unbalanced <' construct."
    );
    stack.exec("'>").unwrap();
    stack.exec("/*").unwrap();
    assert_eq!(
        stack.macro_close().unwrap_err().to_string(),
        "Unbalanced /* construct."
    );
    stack.exec("*/").unwrap();
    stack.exec("<S").unwrap();
    assert_eq!(
        stack.macro_close().unwrap_err().to_string(),
        "Unbalanced <S construct."
    );
    assert_eq!(
        stack.check_balanced().unwrap_err().to_string(),
        "Unbalanced <S construct."
    );
    let mut stack = Stack::new();
    stack.macro_open().unwrap();
    stack.exec("1 2 <% 3 %>").unwrap();
    stack.macro_close().unwrap();
    assert_eq!(stack.depth(), 1);
    let Value::Macro(m) = stack.pop().unwrap() else {
        panic!("not a macro");
    };
    assert_eq!(m.len(), 3);
    assert_eq!(m.to_string(), "<% 1 2 <% 3 %> %>");
}

#[test]
fn check_balanced_reports_every_construct() {
    let mut stack = Stack::new();
    stack.exec("<'").unwrap();
    assert_eq!(
        stack.check_balanced().unwrap_err().to_string(),
        "Unbalanced <' construct."
    );
    let mut stack = Stack::new();
    stack.exec("/*").unwrap();
    assert_eq!(
        stack.check_balanced().unwrap_err().to_string(),
        "Unbalanced /* construct."
    );
    stack.exec("").unwrap();
    stack.exec("*/ 1").unwrap();
    assert!(stack.check_balanced().is_ok());
    assert_eq!(stack.depth(), 1);
}

struct FakeCodec;

impl SecureCodec for FakeCodec {
    fn secure(&self, key: &str, script: &str) -> super::error::Result<String> {
        Ok(format!("{key}|{script}"))
    }

    fn unsecure(&self, encoded: &str) -> super::error::Result<SecureScript> {
        let (key, script) = encoded
            .split_once('|')
            .ok_or_else(|| Error::usage("TEST", "Unable to unsecure script."))?;
        Ok(SecureScript {
            key: key.to_owned(),
            script: script.to_owned(),
        })
    }
}

fn secure_stack() -> Stack {
    let mut stack = Stack::with_limits(Limits::unlimited());
    stack.set_secure_codec(Some(Rc::new(FakeCodec)));
    stack.exec("'k' SECUREKEY").unwrap();
    stack
}

#[test]
fn secure_script_syntax() {
    let mut stack = secure_stack();
    stack.exec("<S 1 'a b'\n<'\nmulti\nline\n'>\nS>").unwrap();
    assert_eq!(
        stack.pop().unwrap(),
        Value::from("k| 1 'a b' 'multi%0Aline'")
    );
    let mut stack = secure_stack();
    stack.exec("<S 'it%27s%0A' S>").unwrap();
    assert_eq!(stack.pop().unwrap(), Value::from("k| 'it%27s%0a'"));
    let mut stack = secure_stack();
    stack.exec("<S\n2 3\nS>").unwrap();
    assert_eq!(stack.pop().unwrap(), Value::from("k| 2 3"));
    assert_fails("S>", "Not inside a secure script definition.");
    assert_fails("<S <S", "Already inside a secure script definition.");
    assert_fails("<S 1", "Unbalanced <S construct.");
    let mut stack = secure_stack();
    stack.exec("<S").unwrap();
    assert_eq!(
        stack.check_balanced().unwrap_err().to_string(),
        "Unbalanced <S construct."
    );
}

#[test]
fn deadline_aborts_macros() {
    let mut stack = Stack::with_limits(Limits::unlimited());
    stack.exec("<% <% true %> <% %> WHILE %>").unwrap();
    let Value::Macro(m) = stack.pop().unwrap() else {
        panic!("not a macro");
    };
    stack.set_deadline(Some(Instant::now() + Duration::from_millis(5)));
    let err = stack.exec_macro(&m).unwrap_err();
    assert_eq!(err.root_cause().message(), "Execution time limit reached.");
}

#[test]
fn run_and_find_macro() {
    let mut stack = exec("<% 1 %> 'm' STORE");
    assert!(stack.find_macro("m").is_ok());
    stack.run("m").unwrap();
    assert_eq!(stack.pop().unwrap(), long(1));
    assert!(fail("@m").to_string().contains("Unknown macro 'm'"));
}

#[test]
fn exec_line_recursion_is_released() {
    let mut stack = Stack::new();
    stack.exec_line("1").unwrap();
    assert_eq!(stack.recursion_level(), 0);
    let _ = stack.exec_line("DROP DROP");
    assert_eq!(stack.recursion_level(), 0);
}
