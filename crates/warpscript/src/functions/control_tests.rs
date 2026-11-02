// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::stack::{Limits, Stack};
use crate::tests::{assert_fails, assert_stack, assert_top, exec, top};
use crate::value::Value;

fn long(l: i64) -> Value {
    Value::Long(l)
}

const IFTE_MESSAGE: &str =
    "IFTE expects three macros or two macros and a boolean on top of the stack.";
const STEP_MESSAGE: &str =
    "FORSTEP expects its 'STEP' macro to leave a number on top of the stack.";

#[test]
fn conditionals() {
    assert_top("true <% 1 %> IFT", 1);
    assert_stack("false <% 1 %> IFT", &[]);
    assert_top("<% true %> <% 1 %> IFT", 1);
    assert_fails(
        "true 1 IFT",
        "IFT expects a *THEN* macro on top of the stack.",
    );
    assert_fails(
        "1 <% %> IFT",
        "IFT expects an *IF* macro or a boolean below the *THEN* macro.",
    );
    assert_fails(
        "<% 1 %> <% %> IFT",
        "IFT expects its 'IF' macro to leave a boolean on top of the stack.",
    );
    assert_top("true <% 1 %> <% 2 %> IFTE", 1);
    assert_top("false <% 1 %> <% 2 %> IFTE", 2);
    assert_top("<% false %> <% 1 %> <% 2 %> IFTE", 2);
    assert_fails("true <% %> 1 IFTE", IFTE_MESSAGE);
    assert_fails("true 1 <% %> IFTE", IFTE_MESSAGE);
    assert_fails("1 <% %> <% %> IFTE", IFTE_MESSAGE);
    assert_fails(
        "<% 1 %> <% %> <% %> IFTE",
        "IFTE expects its 'IF' macro to leave a boolean on top of the stack.",
    );
}

#[test]
fn missing_arguments_and_failing_macros() {
    for script in [
        "IFT",
        "<% %> IFT",
        "<% DROP %> <% %> IFT",
        "true <% DROP %> IFT",
        "IFTE",
        "<% %> <% %> IFTE",
        "<% %> 1 SWITCH",
        "<% %> <% %> 1 SWITCH",
        "<% DROP %> <% %> <% %> 1 SWITCH",
        "<% %> <% %> <% %> 1 SWITCH",
        "WHILE",
        "true WHILE",
        "<% %> WHILE",
        "<% DROP %> <% %> WHILE",
        "UNTIL",
        "<% %> UNTIL",
        "<% %> <% DROP %> UNTIL",
        "FOR",
        "<% %> FOR",
        "1 <% %> FOR",
        "FORSTEP",
        "<% %> FORSTEP",
        "<% %> <% %> FORSTEP",
        "1 <% %> <% %> FORSTEP",
        "1 5 <% DROP DROP DROP %> <% %> FORSTEP",
        "FOREACH",
        "<% %> FOREACH",
        "RANGE",
        "TRY",
        "<% %> TRY",
        "<% %> <% %> TRY",
        "NRETURN",
        "ASSERT",
        "ASSERTMSG",
        "'m' ASSERTMSG",
        "SECTION",
        "TIMEBOX",
        "1 TIMEBOX",
    ] {
        assert_fails(script, "Empty stack.");
    }
}

#[test]
fn switch() {
    assert_top("<% false %> <% 1 %> <% true %> <% 2 %> <% 3 %> 2 SWITCH", 2);
    assert_top("<% true %> <% 1 %> <% true %> <% 2 %> <% 3 %> 2 SWITCH", 1);
    assert_top(
        "<% false %> <% 1 %> <% false %> <% 2 %> <% 3 %> 2 SWITCH",
        3,
    );
    assert_top("<% 3 %> 0 SWITCH", 3);
    assert_top("<% 1 %> <% 1 %> <% 3 %> 1 SWITCH", 3);
    assert_fails(
        "<% %> 'x' SWITCH",
        "SWITCH expects a number of cases on top of the stack.",
    );
    assert_fails(
        "1 1 SWITCH",
        "SWITCH expects a default macro after the number of cases.",
    );
    assert_fails(
        "<% %> -1 SWITCH",
        "SWITCH expects a number of cases on top of the stack.",
    );
    assert_fails(
        "1 <% %> <% %> 1 SWITCH",
        "SWITCH expects each case to be a set of two macros, a conditional macro with an 'exec' macro on top of it.",
    );
    assert_fails(
        "<% %> 1 <% %> 1 SWITCH",
        "SWITCH expects each case to be a set of two macros",
    );
}

#[test]
fn while_loops() {
    // The condition macro leaves whatever is on top of the stack: pre-pushed booleans drive
    // the loop (comparison operators belong to a later module).
    assert_stack(
        "false true true <% %> <% 'last' STORE %> true WHILE $last",
        &[long(1)],
    );
    assert_stack("false true <% %> <% %> WHILE", &[]);
    assert_stack("false true <% %> <% %> false WHILE", &[]);
    assert_stack("<% true %> <% BREAK %> WHILE", &[]);
    assert_stack("<% true %> <% 1 BREAK %> true WHILE", &[long(0), long(1)]);
    assert_stack("false true <% %> <% CONTINUE 'never' %> WHILE", &[]);
    assert_fails(
        "<% 1 %> <% %> WHILE",
        "WHILE expects its 'WHILE' macro to leave a boolean on top of the stack.",
    );
    assert_fails(
        "1 <% %> WHILE",
        "WHILE expects two macros on top of the stack.",
    );
    assert_fails(
        "<% %> 1 WHILE",
        "WHILE expects two macros on top of the stack.",
    );
    assert_fails("<% true %> <% DROP %> WHILE", "Empty stack.");
}

#[test]
fn until_loops() {
    assert_stack("true false <% 'n' STORE %> <% %> true UNTIL $n", &[long(1)]);
    assert_stack("true false <% %> <% %> UNTIL", &[]);
    assert_stack("<% BREAK %> <% false %> UNTIL", &[]);
    assert_stack("true <% CONTINUE 'never' %> <% %> UNTIL", &[]);
    assert_fails(
        "<% %> <% 1 %> UNTIL",
        "UNTIL expects its 'UNTIL' macro to leave a boolean on top of the stack.",
    );
    assert_fails(
        "1 <% %> UNTIL",
        "UNTIL expects two macros on top of the stack.",
    );
    assert_fails(
        "<% %> 1 UNTIL",
        "UNTIL expects two macros on top of the stack.",
    );
    assert_fails("<% DROP %> <% true %> UNTIL", "Empty stack.");
}

#[test]
fn for_loops() {
    assert_stack("1 3 <% %> FOR", &[long(1), long(2), long(3)]);
    assert_stack("1 3 <% DROP %> FOR", &[]);
    assert_stack("1 3 <% 9 %> false FOR", &[long(9), long(9), long(9)]);
    assert_stack("3 1 <% %> FOR", &[]);
    assert_stack("1.0 2 <% %> FOR", &[Value::Double(1.0), Value::Double(2.0)]);
    assert_stack("1 2.5 <% %> FOR", &[Value::Double(1.0), Value::Double(2.0)]);
    assert_stack("1 5 <% 'x' STORE $x BREAK %> FOR", &[long(1)]);
    assert_stack(
        "0 3 <% CONTINUE 'never' %> FOR",
        &[long(0), long(1), long(2), long(3)],
    );
    assert_fails("1 3 1 FOR", "FOR expects a macro on top of the stack.");
    assert_fails("'a' 3 <% %> FOR", "FOR expects numbers as its range.");
    assert_fails("1 'b' <% %> FOR", "FOR expects numbers as its range.");
    assert_fails("1 3 <% DROP DROP %> FOR", "Empty stack.");
}

#[test]
fn forstep_loops() {
    // `from to <% STEP %> <% RUN %> FORSTEP`: the RUN macro is on top of the STEP macro.
    // Without arithmetic yet, the step macro derives the next counter from the stack depth
    // (the body leaves the counter on the stack, so the depth grows by one per iteration).
    assert_stack(
        "1 3 <% DROP DEPTH %> <% %> FORSTEP",
        &[long(1), long(1), long(2), long(3)],
    );
    assert_stack("1 5 <% DROP 9 %> <% %> FORSTEP", &[long(1)]);
    assert_stack("3 1 <% DROP 0 %> <% %> FORSTEP", &[long(3)]);
    assert_stack("3 1 <% DROP 0 %> <% %> false FORSTEP", &[]);
    assert_stack("1.0 5 <% DROP 9.5 %> <% %> FORSTEP", &[Value::Double(1.0)]);
    assert_stack("5.0 1 <% DROP 0.5 %> <% %> FORSTEP", &[Value::Double(5.0)]);
    assert_stack("1 5 <% DROP 9.9 %> <% %> FORSTEP", &[long(1)]);
    assert_stack("1 5 <% DROP 3 %> <% BREAK %> FORSTEP", &[long(1)]);
    assert_stack("1 2 <% DROP 3 %> <% CONTINUE %> FORSTEP", &[long(1)]);
    assert_fails("1 5 <% DROP 'x' %> <% %> FORSTEP", STEP_MESSAGE);
    assert_fails("1.0 5 <% DROP 'x' %> <% %> FORSTEP", STEP_MESSAGE);
    assert_fails(
        "1 5 1 <% %> FORSTEP",
        "FORSTEP expects two macros on top of the stack.",
    );
    assert_fails(
        "1 5 <% %> 1 FORSTEP",
        "FORSTEP expects two macros on top of the stack.",
    );
    assert_fails(
        "'a' 5 <% %> <% %> FORSTEP",
        "FORSTEP expects numbers as its range.",
    );
    assert_fails("1 5 <% DROP DROP %> <% %> FORSTEP", "Empty stack.");
    assert_fails(
        "1 5 1 <% %> FORSTEP",
        "FORSTEP expects two macros on top of the stack.",
    );
    assert_fails(
        "1 5 <% %> 1 FORSTEP",
        "FORSTEP expects two macros on top of the stack.",
    );
    assert_fails(
        "'a' 5 <% %> <% %> FORSTEP",
        "FORSTEP expects numbers as its range.",
    );
    assert_fails("1 5 <% %> <% DROP DROP %> FORSTEP", "Empty stack.");
}

#[test]
fn foreach_loops() {
    assert_stack("[ 1 2 ] <% %> FOREACH", &[long(1), long(2)]);
    assert_stack(
        "[ 1 2 ] <% %> true FOREACH",
        &[long(1), long(0), long(2), long(1)],
    );
    assert_stack("[ 1 2 ] <% %> false FOREACH", &[long(1), long(2)]);
    assert_stack("( 1 2 ) <% %> FOREACH", &[long(1), long(2)]);
    assert_stack(
        "{ 'a' 1 } <% %> true FOREACH",
        &[Value::from("a"), long(1), long(0)],
    );
    assert_stack("'ab' <% %> FOREACH", &[Value::from("a"), Value::from("b")]);
    assert_stack("[ 1 2 3 ] <% DUP BREAK %> FOREACH", &[long(1), long(1)]);
    assert_stack(
        "[ 1 2 ] <% CONTINUE 'never' %> FOREACH",
        &[long(1), long(2)],
    );
    assert_stack("[ ] <% %> FOREACH", &[]);
    assert_fails(
        "[ ] 1 FOREACH",
        "FOREACH expects a macro on top of the stack.",
    );
    assert_fails(
        "1 <% %> FOREACH",
        "FOREACH operates on a list, map, Geo Time Series, ENCODER, STRING, iterator or iterable.",
    );
    assert_fails("[ 1 ] <% DROP DROP %> FOREACH", "Empty stack.");
}

#[test]
fn range() {
    assert_top("[ 3 ] RANGE", Value::list(vec![long(0), long(1), long(2)]));
    assert_top("[ 1 3 ] RANGE", Value::list(vec![long(1), long(2)]));
    assert_top(
        "[ 1 6 2 ] RANGE",
        Value::list(vec![long(1), long(3), long(5)]),
    );
    assert_top("[ 3 1 ] RANGE", Value::list(Vec::new()));
    assert_top("[ 1.5 3 ] RANGE", Value::list(vec![long(1), long(2)]));
    assert_fails(
        "1 RANGE",
        "RANGE expects a list of parameters on top of the stack.",
    );
    assert_fails(
        "[ ] RANGE",
        "RANGE expects a list of 1 to 3 longs on top of the stack.",
    );
    assert_fails(
        "[ 1 2 3 4 ] RANGE",
        "RANGE expects a list of 1 to 3 longs on top of the stack.",
    );
    assert_fails(
        "[ 'a' ] RANGE",
        "RANGE expects a list of 1 to 3 longs on top of the stack.",
    );
    assert_fails("[ 1 6 0 ] RANGE", "RANGE step MUST be > 0.");
    assert_top(
        "[ 9223372036854775806 9223372036854775807 5 ] RANGE",
        Value::list(vec![long(9_223_372_036_854_775_806)]),
    );
}

#[test]
fn loop_duration_limit() {
    let mut stack = Stack::with_limits(Limits::unlimited());
    stack.limits.max_loop_duration = 1;
    for (name, script) in [
        ("WHILE", "<% true %> <% %> WHILE"),
        ("UNTIL", "<% %> <% false %> UNTIL"),
        ("FOR", "0 9223372036854775807 <% DROP %> FOR"),
        ("FORSTEP", "0 9223372036854775807 <% %> <% DROP %> FORSTEP"),
    ] {
        let err = stack.exec(script).unwrap_err();
        assert!(
            err.to_string().contains(&format!(
                "{name} executed for too long (> 1 ms). Consider raising the limit or using capabilities."
            )),
            "{name}: {err}"
        );
        stack.clear();
    }
}

#[test]
fn break_continue_return_stop() {
    assert_stack("1 <% 2 RETURN 3 %> EVAL 4", &[long(1), long(2), long(4)]);
    assert_stack(
        "<% <% 1 2 NRETURN 9 %> EVAL 8 %> EVAL 7",
        &[long(1), long(7)],
    );
    assert_fails("'x' NRETURN", "NRETURN expects a number of levels.");
    assert_stack("1 <% 2 STOP 3 %> EVAL 4", &[long(1), long(2)]);
}

#[test]
fn try_catch_finally() {
    assert_stack("<% 1 %> <% 2 %> <% 3 %> TRY", &[long(1), long(3)]);
    assert_stack(
        "<% 'boom' MSGFAIL %> <% 2 %> <% 3 %> TRY",
        &[long(2), long(3)],
    );
    assert_stack("<% 'boom' MSGFAIL %> <% 2 %> <% %> TRY", &[long(2)]);
    assert_fails(
        "<% 'boom' MSGFAIL %> <% 'again' MSGFAIL %> <% 3 %> TRY",
        "MSGFAIL again",
    );
    assert_fails("<% 1 %> <% %> <% 'fin' MSGFAIL %> TRY", "MSGFAIL fin");
    assert_fails(
        "<% %> <% %> 1 TRY",
        "TRY expects a 'finally' macro on top of the stack.",
    );
    assert_fails(
        "<% %> 1 <% %> TRY",
        "TRY expects a 'catch' macro below the 'finally' macro.",
    );
    assert_fails(
        "1 <% %> <% %> TRY",
        "TRY expects a 'try' macro below the 'catch' macro.",
    );
    // Signals are caught too (Warp 10 catches every Throwable).
    assert_stack("<% BREAK %> <% 2 %> <% %> TRY", &[long(2)]);
    assert_stack("<% 2 NRETURN %> <% 2 %> <% %> TRY", &[long(2)]);
    assert_stack("<% STOP %> <% 2 %> <% %> TRY", &[long(2)]);
    assert_stack("<% RETURN %> <% 2 %> <% %> TRY", &[]);
}

#[test]
fn error_and_rethrow() {
    let errors = top("<% 'boom' MSGFAIL %> <% %> <% %> TRY ERROR");
    let Value::List(errors) = errors else {
        panic!("not a list");
    };
    let errors = errors.borrow();
    assert_eq!(errors.len(), 2);
    let Value::Map(outer) = &errors[0] else {
        panic!("not a map");
    };
    let outer = outer.borrow();
    assert_eq!(
        outer.get(&Value::from("type")),
        Some(&Value::from("class io.warp10.script.WarpScriptException"))
    );
    assert_eq!(
        outer.get(&Value::from("message")),
        Some(&Value::from("Exception at 'MSGFAIL' in section '[TOP]'"))
    );
    assert_eq!(
        outer.get(&Value::from("stacktrace")),
        Some(&Value::list(Vec::new()))
    );
    let Value::Map(inner) = &errors[1] else {
        panic!("not a map");
    };
    assert_eq!(
        inner.borrow().get(&Value::from("message")),
        Some(&Value::from("MSGFAIL boom"))
    );
    let loop_message = "must be used in a one of FOR, FOREACH, FORSTEP, WHILE, UNTIL loop.";
    for (script, kind, message) in [
        (
            "<% BREAK %>",
            "WarpScriptLoopBreakException",
            format!("Exception at BREAK: {loop_message}"),
        ),
        (
            "<% CONTINUE %>",
            "WarpScriptLoopContinueException",
            format!("Exception at CONTINUE: {loop_message}"),
        ),
        ("<% STOP %>", "WarpScriptStopException", "STOP".to_owned()),
        (
            "<% 2 NRETURN %>",
            "WarpScriptReturnException",
            "RETURN".to_owned(),
        ),
    ] {
        let errors = top(&format!("{script} <% %> <% %> TRY ERROR"));
        assert_eq!(
            errors.to_string(),
            format!("[{{type=class io.warp10.script.{kind}, message={message}, stacktrace=[]}}]")
        );
    }
    assert_top("ERROR", Value::list(Vec::new()));
    assert_stack("RETHROW 1", &[long(1)]);
    assert_fails(
        "<% 'boom' MSGFAIL %> <% %> <% %> TRY RETHROW",
        "MSGFAIL boom",
    );
    let stack = exec("<% 'boom' MSGFAIL %> <% %> <% %> TRY");
    assert!(stack.last_error().is_some());
}

#[test]
fn failures_and_assertions() {
    assert_fails("FAIL", "FAIL failed.");
    assert_fails("COMPILE", "COMPILE Not supported.");
    assert_fails("COMPILED", "COMPILED Not supported.");
    assert_fails("'custom' MSGFAIL", "MSGFAIL custom");
    assert_fails("42 MSGFAIL", "MSGFAIL 42");
    assert_fails("MSGFAIL", "Empty stack.");
    assert_stack("true ASSERT", &[]);
    assert_fails("false ASSERT", "ASSERT failed.");
    assert_fails("1 ASSERT", "ASSERT expects a boolean on top of the stack.");
    assert_stack("true 'msg' ASSERTMSG", &[]);
    assert_fails("false 'msg' ASSERTMSG", "ASSERTMSG failed 'msg'");
    assert_fails("false 1 ASSERTMSG", "ASSERTMSG expects a STRING message.");
    assert_fails(
        "1 'msg' ASSERTMSG",
        "ASSERTMSG expects a boolean below the message.",
    );
}

#[test]
fn sections() {
    let stack = exec("'s' SECTION");
    assert_eq!(stack.section_name(), "s");
    assert_fails("1 SECTION", "SECTION expected a section name as a string.");
    assert_fails(
        "'s' SECTION <% DROP %> EVAL",
        "Exception at 'DROP' in section 's'",
    );
    let stack = exec("<% 'inner' SECTION %> EVAL");
    assert_eq!(stack.section_name(), "[TOP]");
}

#[test]
fn timebox() {
    assert_top("<% 1 %> 1000000 TIMEBOX", 1);
    assert_top("<% 2 %> 0 TIMEBOX", 2);
    assert_top("<% 3 %> -5 TIMEBOX", 3);
    assert_stack("<% 1 STOP 2 %> 1000000 TIMEBOX", &[long(1)]);
    assert_stack("<% 1 2 NRETURN 2 %> 1000000 TIMEBOX", &[long(1)]);
    assert_fails(
        "<% 'boom' MSGFAIL %> 1000000 TIMEBOX",
        "TIMEBOX encountered an exception while executing macro",
    );
    assert_fails("<% 'boom' MSGFAIL %> 1000000 TIMEBOX", "MSGFAIL boom");
    assert_fails("1 1000 TIMEBOX", "TIMEBOX operates on a macro.");
    assert_fails(
        "<% %> 'x' TIMEBOX",
        "TIMEBOX expects a maximum execution time on top of the stack.",
    );
    assert_stack(
        "[ 1 2 ] <% <% BREAK %> 1000000 TIMEBOX %> FOREACH",
        &[long(1)],
    );
    assert_fails(
        "<% <% true %> <% %> WHILE %> 2000 TIMEBOX",
        "TIMEBOX reached the execution time limit (2000 MICROSECONDS).",
    );
    // An expired deadline swallowed by a TRY still fails the TIMEBOX.
    assert_fails(
        "<% <% <% true %> <% %> WHILE %> <% %> <% %> TRY %> 2000 TIMEBOX",
        "TIMEBOX reached the execution time limit (2000 MICROSECONDS).",
    );
    // A nested TIMEBOX keeps the tighter (outer) deadline.
    assert_fails(
        "<% <% <% true %> <% %> WHILE %> 100000000 TIMEBOX %> 2000 TIMEBOX",
        "TIMEBOX reached the execution time limit",
    );
    assert_top("<% <% 4 %> 1000 TIMEBOX %> 1000000 TIMEBOX", 4);
}
