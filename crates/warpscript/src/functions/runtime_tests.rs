#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the runtime introspection and control functions.

use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;

use super::runtime::{
    ATTRIBUTE_CHRONOSTATS, ATTRIBUTE_DEBUG_DEPTH, ATTRIBUTE_ELAPSED, ATTRIBUTE_EXPORTED_SYMBOLS,
    ATTRIBUTE_HEADERS, ATTRIBUTE_INTERACTIVE_ECHO, ATTRIBUTE_INTERACTIVE_JSON,
    ATTRIBUTE_INTERACTIVE_TIME, ATTRIBUTE_LINENO, ATTRIBUTE_TIMINGS,
    ATTRIBUTE_WRAPPED_STATEMENT_FACTORY, current_time,
};
use crate::error::Error;
use crate::stack::{Limits, REVISION, Stack};
use crate::tests::{assert_fails, assert_stack, assert_top, exec, fail_msg, test_stack, top};
use crate::value::Value;

fn strings(values: &[&str]) -> Value {
    Value::list(values.iter().map(|s| Value::from(*s)).collect())
}

// -----------------------------------------------------------------------------------------
// Symbols and functions
// -----------------------------------------------------------------------------------------

#[test]
fn symbols_lists_the_symbol_table() {
    assert_top("SYMBOLS", strings(&[]));
    assert_top(
        "1 'a' STORE 2 'b' STORE 'a' FORGET SYMBOLS",
        strings(&["b"]),
    );
    assert_top("1 'a' STORE NULL 'b' STORE SYMBOLS", strings(&["a", "b"]));
}

#[test]
fn functions_needs_the_inventory_capability() {
    assert_fails("FUNCTIONS", "FUNCTIONS missing capability 'inventory'.");
    let mut stack = test_stack();
    stack
        .capabilities_mut()
        .insert("inventory".to_owned(), String::new());
    stack.exec_checked("FUNCTIONS").unwrap();
    let Value::List(names) = stack.pop().unwrap() else {
        panic!("not a list");
    };
    let names = names.borrow();
    assert!(names.contains(&Value::from("DUP")));
    assert!(names.contains(&Value::from("FUNCTIONS")));
    assert_eq!(names.len(), super::registered_names().len());
}

#[test]
fn funcref_and_eval() {
    assert_top("1 2 'DUP' FUNCREF EVAL + +", 5);
    assert_top("'DUP' FUNCREF TYPEOF", "FUNCTION");
    assert_top("'DUP' FUNCREF SNAPSHOT", "'DUP' FUNCREF ");
    assert_top("'->HEX' FUNCREF SNAPSHOT", "'-%3EHEX' FUNCREF ");
    // Function objects are returned as they are.
    assert_top("'test.object' FUNCREF TYPEOF", "OPERATOR");
    assert_fails(
        "'test.object' FUNCREF EVAL",
        "EVAL expects a Macro, a function or a String.",
    );
    // Definitions come first.
    assert_top("<% DROP 42 %> 'DUP' DEF 7 'DUP' FUNCREF EVAL", 42);
    assert_fails("'nope' FUNCREF", "Unknown function 'nope'");
    assert_fails(
        "1 FUNCREF",
        "FUNCREF expects the function name as a STRING.",
    );
    assert_fails("NULL 'DUP' DEF 'DUP' FUNCREF EVAL", "DUP is undefined.");
    assert_fails(
        "<% 1 %> 'DUP' DEF false REDEFS 'DUP' FUNCREF",
        "Disallowed redefined function 'DUP'.",
    );
}

#[test]
fn ref_pushes_itself() {
    assert_top("REF TYPEOF", "FUNCTION");
    assert_top("REF SNAPSHOT", "'REF' FUNCREF ");
    assert_top("REF EVAL TYPEOF", "FUNCTION");
}

#[test]
fn macro_list_conversions() {
    assert_top(
        "<% 1 'a' STORE $a DUP + %> MACRO-> SIZE",
        7, // 1 'a' STORE 'a' LOAD DUP +
    );
    assert_top("<% 1 2 + %> MACRO-> ->MACRO EVAL", 3);
    assert_top("<% 1 2 + %> MACRO-> ->MACRO SNAPSHOT", "<% 1 2 + %> ");
    assert_top("[ 1 2 'DUP' FUNCREF ] ->MACRO EVAL + +", 5);
    assert_top(
        "[ 1 test.object ] ->MACRO SNAPSHOT",
        "<% 1 'test.object' FUNCREF %> ",
    );
    assert_top("<% <% 1 %> EVAL %> MACRO-> 0 GET TYPEOF", "MACRO");
    assert_fails("1 MACRO->", "MACRO-> operates on a MACRO.");
    assert_fails(
        "<% 1 %> MSEC MACRO->",
        "MACRO-> cannot operate on a secure MACRO.",
    );
    assert_fails("1 ->MACRO", "->MACRO operates on a LIST.");
    for f in [
        "MACRO->",
        "->MACRO",
        "DEREF",
        "REDEFS",
        "EXPORT",
        "HEADER",
        "STMTPOS",
        "NPEEK",
        "MINREV",
        "MAXREV",
        "EXTLOADED",
        "GUARD",
        "FUNCREF",
        "MSEC",
        "MRSEC",
        "IMPORT",
        "STACKATTRIBUTE",
        "TIMED",
        "ECHOON",
        "NDEBUGON",
        "AGO",
    ] {
        assert_fails(f, "Empty stack.");
    }
    assert_fails("NULL GUARD", "Empty stack.");
    assert_fails("[ ] GUARD", "Empty stack.");
    assert_fails("SAVE GUARD", "Empty stack.");
}

#[test]
fn deref_substitutes_loads_and_registers() {
    assert_top(
        "<% $a $b + %> { 'a' 1 'b' 2 } DEREF SNAPSHOT",
        "<% 1 2 + %> ",
    );
    assert_top(
        "<% $a $b + %> { 'a' 1 } DEREF SNAPSHOT",
        "<% 1 'b' LOAD + %> ",
    );
    // Macro values: a single statement is inlined, longer ones are evaluated.
    assert_top("<% $a %> { 'a' <% 7 %> } DEREF SNAPSHOT", "<% 7 %> ");
    assert_top(
        "<% $a %> { 'a' <% 3 4 %> } DEREF SNAPSHOT",
        "<% <% 3 4 %> EVAL %> ",
    );
    assert_top("<% $a %> { 'a' <% 3 4 %> } DEREF EVAL +", 7);
    // Registers.
    assert_top(
        "<% PUSHR3 PUSHR4 %> { 3 10 4 <% 1 2 %> } DEREF SNAPSHOT",
        "<% 10 <% 1 2 %> EVAL %> ",
    );
    assert_top("<% PUSHR3 %> { 2 10 } DEREF SNAPSHOT", "<% PUSHR3 %> ");
    // Nested macros are processed too; the original macro is left untouched.
    assert_top(
        "<% <% $a %> EVAL %> { 'a' 5 } DEREF SNAPSHOT",
        "<% <% 5 %> EVAL %> ",
    );
    assert_top(
        "<% $a %> 'm' STORE $m { 'a' 1 } DEREF DROP $m SNAPSHOT",
        "<% 'a' LOAD %> ",
    );
    // `LOAD` not preceded by a string literal, or with a non-string key, is kept.
    assert_top(
        "<% 'a' 'b' DROP LOAD %> { 'a' 1 } DEREF SNAPSHOT",
        "<% 'a' 'b' DROP LOAD %> ",
    );
    assert_top("<% LOAD %> { 'a' 1 } DEREF SNAPSHOT", "<% LOAD %> ");
    assert_fails(
        "<% 1 %> 1 DEREF",
        "DEREF expects a map of variable names or register id to values on top of the stack.",
    );
    assert_fails("1 { } DEREF", "DEREF operates on a Macro.");
    assert_fails(
        "<% 1 %> MSEC { } DEREF",
        "DEREF cannot operate on a secure Macro.",
    );
    // A nested secure macro is refused too.
    let inner = top("<% 1 %> MSEC");
    let outer = Value::from(Rc::new(crate::macros::Macro::from_statements(vec![
        crate::macros::Statement::Push(inner),
    ])));
    let mut stack = test_stack();
    stack.push(outer);
    stack.push(top("{ }"));
    let err = stack.exec("DEREF").unwrap_err().to_string();
    assert!(
        err.contains("DEREF cannot operate on a secure Macro."),
        "{err}"
    );
}

#[test]
fn redefs_toggles_redefinitions() {
    // Redefinitions are allowed by default.
    assert_top("<% DROP 1 %> 'DUP' DEF 5 DUP", 1);
    assert_fails(
        "<% DROP 1 %> 'DUP' DEF false REDEFS 5 DUP",
        "Disallowed redefined function 'DUP'.",
    );
    assert_top("<% DROP 1 %> 'DUP' DEF false REDEFS true REDEFS 5 DUP", 1);
    assert_fails("1 REDEFS", "REDEFS expects a boolean on top of the stack.");
}

#[test]
fn msec_and_mrsec() {
    assert_top("<% 1 <% 2 %> %> MSEC SNAPSHOT", "<% /* Secure Macro */ %> ");
    let m = top("<% 1 <% 2 %> %> MSEC");
    let Value::Macro(m) = m else {
        panic!("not a macro")
    };
    assert!(m.is_secure());
    let Some(crate::macros::Statement::Push(Value::Macro(inner))) = m.statements.get(1) else {
        panic!("no nested macro");
    };
    assert!(!inner.is_secure());
    let m = top("<% 1 <% 2 %> %> MRSEC");
    let Value::Macro(m) = m else {
        panic!("not a macro")
    };
    let Some(crate::macros::Statement::Push(Value::Macro(inner))) = m.statements.get(1) else {
        panic!("no nested macro");
    };
    assert!(inner.is_secure());
    assert_fails("1 MSEC", "MSEC operates on a Macro.");
    assert_fails("1 MRSEC", "MRSEC operates on a Macro.");
}

#[test]
fn import_rewrites_macro_names() {
    assert_top(
        "<% 42 %> 'lib/util/answer' STORE 'lib/util/' 'u/' IMPORT @u/answer",
        42,
    );
    assert_top(
        "<% 42 %> 'lib/answer' STORE 'lib/' 'l/' IMPORT NULL 'l/' IMPORT <% 'l/answer' RUN %> <% 'unknown' %> <% %> TRY",
        "unknown",
    );
    // The longest alias wins.
    let stack = exec("'a' 'x/' IMPORT 'bb' 'x/y/' IMPORT 'ccc' 'zzzzz/' IMPORT");
    assert_eq!(
        stack.import_rules(),
        &[
            ("zzzzz/".to_owned(), "ccc".to_owned()),
            ("x/y/".to_owned(), "bb".to_owned()),
            ("x/".to_owned(), "a".to_owned()),
        ]
    );
    // Like Warp 10, aliases of the same length share one rule.
    let stack =
        exec("'a' 'x/' IMPORT 'b' 'y/' IMPORT 'c' 'zz/' IMPORT NULL 'zz/' IMPORT NULL 'q/' IMPORT");
    assert_eq!(stack.import_rules(), &[]);
    let stack = exec("'a' 'x/' IMPORT 'b' 'y/' IMPORT");
    assert_eq!(stack.import_rules(), &[("x/".to_owned(), "b".to_owned())]);
    assert_fails(
        "'x' 1 IMPORT",
        "IMPORT expects an alias for the imported namespace on top of the stack.",
    );
    assert_fails(
        "1 'x' IMPORT",
        "IMPORT expects the name of the imported namespace or null below its alias.",
    );
    assert_fails("@nope", "Unknown macro 'nope'");
    // Rules only apply to names starting with the alias; removing an unknown alias is a no-op.
    assert_top(
        "<% 1 %> 'plain' STORE 'lib/' 'u/' IMPORT NULL 'qqqqqqq/' IMPORT @plain",
        1,
    );
    assert_fails("'x' IMPORT", "Empty stack.");
}

// -----------------------------------------------------------------------------------------
// GUARD
// -----------------------------------------------------------------------------------------

#[test]
fn guard_runs_macros_and_clears_symbols() {
    assert_top("1 'a' STORE <% $a 1 + %> GUARD $a +", 3);
    // A list of symbols and registers to clear.
    assert_stack(
        "1 'a' STORE 2 'b' STORE 3 POPR0 <% 4 %> [ 'a' 0 'zz' 1.5 ] GUARD 'a' DEFINED PUSHR0 $b",
        &[
            Value::Long(4),
            Value::Boolean(false),
            Value::Null,
            Value::Long(2),
        ],
    );
    // An empty list clears everything.
    assert_stack(
        "1 'a' STORE 3 POPR0 <% 4 %> [ ] GUARD SYMBOLS PUSHR0",
        &[Value::Long(4), strings(&[]), Value::Null],
    );
    // A context restores symbols as they were.
    assert_stack(
        "1 'a' STORE SAVE <% 2 'a' STORE 3 'b' STORE %> SWAP GUARD $a 'b' DEFINED",
        &[Value::Long(1), Value::Boolean(false)],
    );
    assert_fails("'x' GUARD", "GUARD operates on a MACRO.");
    // A LONG on top is a number of levels to hide.
    assert_fails("1 GUARD", "Empty stack.");
}

#[test]
fn guard_hides_the_stack() {
    assert_stack(
        "1 2 3 <% DEPTH %> NULL GUARD DEPTH",
        &[
            Value::Long(1),
            Value::Long(2),
            Value::Long(3),
            Value::Long(0),
            Value::Long(4),
        ],
    );
    assert_stack(
        "1 2 3 <% DEPTH %> 2 GUARD",
        &[
            Value::Long(1),
            Value::Long(2),
            Value::Long(3),
            Value::Long(1),
        ],
    );
    assert_stack(
        "1 2 3 <% DEPTH %> 0 GUARD",
        &[
            Value::Long(1),
            Value::Long(2),
            Value::Long(3),
            Value::Long(3),
        ],
    );
}

#[test]
fn guard_failures_abort_without_details() {
    let err = fail_msg("1 2 'secret' 'a' STORE <% 3 'x' STORE FAIL %> [ 'x' ] GUARD");
    assert_eq!(err, "Line #1 (Exception in GUARDed macro.)");
    // The stack is cleared, the symbols listed are forgotten, the others kept.
    let mut stack = test_stack();
    let err = stack
        .exec("1 2 <% 3 'x' STORE 4 FAIL %> [ 'x' ] GUARD")
        .unwrap_err();
    assert!(matches!(err.root_cause(), Error::Abort(_)), "{err:?}");
    assert_eq!(err.root_cause().message(), "Exception in GUARDed macro.");
    assert_eq!(stack.depth(), 0);
    assert!(!stack.has_symbol("x"));
    // Hidden levels are shown again after a failure.
    let mut stack = test_stack();
    stack.exec("1 2 <% FAIL %> NULL GUARD").unwrap_err();
    assert_eq!(stack.depth(), 2);
    // The abort is catchable by TRY and reported by ERROR.
    assert_top(
        "<% <% FAIL %> GUARD %> <% ERROR 0 GET 'type' GET %> <% %> TRY",
        "class io.warp10.script.WarpScriptATCException",
    );
    // STOP inside the guarded macro is a failure too (RETURN ends the macro normally).
    assert_fails("<% STOP %> GUARD", "Exception in GUARDed macro.");
    assert_top("<% 1 RETURN 2 %> GUARD", 1);
}

/// A token resolver granting `x` to every token.
struct Granting;

impl crate::stack::TokenResolver for Granting {
    fn token_attributes(&self, _: &str) -> Option<IndexMap<String, String>> {
        Some(IndexMap::from([(".cap:x".to_owned(), "42".to_owned())]))
    }
}

#[test]
fn guard_restores_capabilities_unless_exported() {
    let mut stack = test_stack();
    stack
        .capabilities_mut()
        .insert("keep".to_owned(), "1".to_owned());
    stack
        .exec_checked("<% 'keep' CAPDEL 'a' CAPEXPORT %> GUARD")
        .unwrap();
    assert_eq!(stack.capability("keep"), Some("1"));
    // Exported capabilities survive (here `keep` is removed, `a` is absent anyway).
    stack
        .exec_checked("<% 'keep' CAPDEL 'keep' CAPEXPORT %> GUARD")
        .unwrap();
    assert_eq!(stack.capability("keep"), None);
    // Capabilities added by a token inside the guard are exported by name.
    stack.set_token_resolver(Some(Rc::new(Granting)));
    stack.exec_checked("<% 'tok' CAPADD %> GUARD").unwrap();
    assert_eq!(stack.capability("x"), None);
    stack
        .exec_checked("<% 'tok' CAPADD 'x' CAPEXPORT %> GUARD")
        .unwrap();
    assert_eq!(stack.capability("x"), Some("42"));
    // A NULL export propagates the exported names to the enclosing GUARD.
    let mut stack = test_stack();
    stack.set_token_resolver(Some(Rc::new(Granting)));
    stack
        .exec_checked("<% <% 'tok' CAPADD 'x' CAPEXPORT NULL CAPEXPORT %> GUARD %> GUARD")
        .unwrap();
    assert_eq!(stack.capability("x"), Some("42"));
    let mut stack = test_stack();
    stack.set_token_resolver(Some(Rc::new(Granting)));
    stack
        .exec_checked("<% <% 'tok' CAPADD 'x' CAPEXPORT %> GUARD %> GUARD")
        .unwrap();
    assert_eq!(stack.capability("x"), None);
    // Nothing is exported when the macro fails.
    let mut stack = test_stack();
    stack.set_token_resolver(Some(Rc::new(Granting)));
    stack
        .exec("<% 'tok' CAPADD 'x' CAPEXPORT FAIL %> GUARD")
        .unwrap_err();
    assert_eq!(stack.capability("x"), None);
    assert!(stack.exported_capabilities_mut().is_none());
}

#[test]
fn call_is_not_supported() {
    assert_fails("'prog' CALL", "CALL Not supported.");
    assert_fails(
        "1 CALL",
        "CALL expects a subprogram name on top of the stack.",
    );
    assert_fails("CALL", "Empty stack.");
}

// -----------------------------------------------------------------------------------------
// Attributes
// -----------------------------------------------------------------------------------------

#[test]
fn stackattribute_reads_attributes() {
    assert_top("'stack.ops' STACKATTRIBUTE", 2);
    assert_top("'section.name' STACKATTRIBUTE", "[TOP]");
    assert_top("'macro.name' STACKATTRIBUTE", Value::Null);
    // Macros from the symbol table carry no name (only repository macros do).
    assert_top(
        "<% 'macro.name' STACKATTRIBUTE %> 'm' STORE @m",
        Value::Null,
    );
    assert_top("'in.secure.macro' STACKATTRIBUTE", false);
    assert_top("'nope' STACKATTRIBUTE", Value::Null);
    assert_top("JSONSTRICT 'json.strict' STACKATTRIBUTE", true);
    assert_fails(
        "1 STACKATTRIBUTE",
        "STACKATTRIBUTE expects a string as the attribute key.",
    );
}

#[test]
fn export_collects_symbols() {
    let stack = exec("'a' EXPORT 'b' EXPORT 'a' EXPORT");
    assert_eq!(
        stack.attribute(ATTRIBUTE_EXPORTED_SYMBOLS),
        Some(&Value::set(
            [Value::from("a"), Value::from("b")].into_iter().collect()
        ))
    );
    // A list replaces the set; NULL means everything.
    let stack = exec("'a' EXPORT [ 'c' 'd' 'c' ] EXPORT");
    assert_eq!(
        stack.attribute(ATTRIBUTE_EXPORTED_SYMBOLS),
        Some(&Value::set(
            [Value::from("c"), Value::from("d")].into_iter().collect()
        ))
    );
    let stack = exec("[ 'c' NULL 'd' ] EXPORT");
    assert_eq!(
        stack.attribute(ATTRIBUTE_EXPORTED_SYMBOLS),
        Some(&Value::set([Value::Null].into_iter().collect()))
    );
    assert_fails(
        "[ 'c' 1 ] EXPORT",
        "EXPORT expects a symbol name or a list thereof on top of the stack.",
    );
    assert_fails(
        "1 EXPORT",
        "EXPORT expects a symbol name or a list thereof on top of the stack.",
    );
}

#[test]
fn sections_and_info_mode() {
    assert_top("GETSECTION", "[TOP]");
    assert_top("'work' SECTION GETSECTION", "work");
    assert_top("INFOMODE 'infomode' STACKATTRIBUTE", true);
    assert_top("INFOMODE INFOMODE 'infomode' STACKATTRIBUTE", Value::Null);
}

#[test]
fn header_sets_response_headers() {
    let stack =
        exec("'X-Foo' 'bar' HEADER 'x-num' 42 HEADER 'X-Gone' 1 HEADER 'X-Gone' NULL HEADER");
    let Some(Value::Map(headers)) = stack.attribute(ATTRIBUTE_HEADERS) else {
        panic!("no headers");
    };
    let headers = headers.borrow();
    assert_eq!(
        headers.get(&Value::from("X-Foo")),
        Some(&Value::from("bar"))
    );
    assert_eq!(headers.get(&Value::from("x-num")), Some(&Value::from("42")));
    assert_eq!(headers.get(&Value::from("X-Gone")), None);
    assert_eq!(headers.len(), 2);
    // Removing a header that was never set is fine.
    let stack = exec("'Foo' NULL HEADER");
    assert!(matches!(
        stack.attribute(ATTRIBUTE_HEADERS),
        Some(Value::Map(_))
    ));
    assert_fails(
        "'Foo' 'bar' HEADER",
        "HEADER only headers beginning with 'X-' can be set.",
    );
    assert_fails(
        "1 'bar' HEADER",
        "HEADER expects a header name (a string) below the header value.",
    );
}

#[test]
fn interactive_and_debug_flags() {
    let stack = exec("3 ECHOON");
    assert_eq!(
        stack.attribute(ATTRIBUTE_INTERACTIVE_ECHO),
        Some(&Value::Long(3))
    );
    let stack = exec("3 ECHOON ECHOOFF");
    assert_eq!(stack.attribute(ATTRIBUTE_INTERACTIVE_ECHO), None);
    assert_fails("'x' ECHOON", "ECHOON expects a number of levels.");
    let stack = exec("TIMEON");
    assert_eq!(
        stack.attribute(ATTRIBUTE_INTERACTIVE_TIME),
        Some(&Value::Boolean(true))
    );
    assert_eq!(
        exec("TIMEON TIMEOFF").attribute(ATTRIBUTE_INTERACTIVE_TIME),
        None
    );
    assert_eq!(
        exec("LINEON").attribute(ATTRIBUTE_LINENO),
        Some(&Value::Boolean(true))
    );
    assert_eq!(exec("LINEON LINEOFF").attribute(ATTRIBUTE_LINENO), None);
    assert_eq!(
        exec("DEBUGON").attribute(ATTRIBUTE_DEBUG_DEPTH),
        Some(&Value::Long(i64::from(i32::MAX)))
    );
    assert_eq!(
        exec("4 NDEBUGON").attribute(ATTRIBUTE_DEBUG_DEPTH),
        Some(&Value::Long(4))
    );
    assert_eq!(
        exec("DEBUGON DEBUGOFF").attribute(ATTRIBUTE_DEBUG_DEPTH),
        Some(&Value::Long(0))
    );
    assert_fails("'x' NDEBUGON", "NDEBUGON expects a number as parameter.");
    assert_eq!(
        exec("JSONSTACK").attribute(ATTRIBUTE_INTERACTIVE_JSON),
        Some(&Value::Boolean(true))
    );
    assert_eq!(
        exec("JSONSTACK WSSTACK").attribute(ATTRIBUTE_INTERACTIVE_JSON),
        Some(&Value::Boolean(false))
    );
    assert_eq!(
        exec("true STMTPOS").attribute(ATTRIBUTE_WRAPPED_STATEMENT_FACTORY),
        Some(&Value::Boolean(true))
    );
    assert_eq!(
        exec("true STMTPOS false STMTPOS").attribute(ATTRIBUTE_WRAPPED_STATEMENT_FACTORY),
        None
    );
    assert_fails("1 STMTPOS", "STMTPOS expects a BOOLEAN.");
}

// -----------------------------------------------------------------------------------------
// Timing
// -----------------------------------------------------------------------------------------

#[test]
fn timings_and_elapsed() {
    assert_eq!(
        exec("TIMINGS").attribute(ATTRIBUTE_TIMINGS),
        Some(&Value::Boolean(true))
    );
    assert_eq!(
        exec("TIMINGS NOTIMINGS").attribute(ATTRIBUTE_TIMINGS),
        Some(&Value::Boolean(false))
    );
    assert_top("ELAPSED", Value::list(Vec::new()));
    let mut stack = test_stack();
    stack.set_attribute(
        ATTRIBUTE_ELAPSED,
        Some(Value::list(vec![Value::Long(5), Value::Long(9)])),
    );
    stack.exec_checked("ELAPSED").unwrap();
    assert_eq!(
        stack.pop().unwrap(),
        Value::list(vec![Value::Long(5), Value::Long(9)])
    );
}

#[test]
fn chronometers() {
    assert_top("CHRONOSTATS", Value::Null);
    let stack = exec(
        "'a' CHRONOSTART 'a' CHRONOSTART 'a' CHRONOEND 'a' CHRONOEND 'b' CHRONOSTART 'b' CHRONOEND CHRONOSTATS",
    );
    let mut stack = stack;
    let Value::Map(stats) = stack.pop().unwrap() else {
        panic!("not a map");
    };
    let stats = stats.borrow();
    let Some(Value::Map(a)) = stats.get(&Value::from("a")) else {
        panic!("no stats for a");
    };
    let a = a.borrow();
    assert_eq!(a.get(&Value::from("total_calls")), Some(&Value::Long(2)));
    assert!(a.get(&Value::from("total_time")).unwrap().as_i64().unwrap() >= 0);
    let Some(Value::Map(b)) = stats.get(&Value::from("b")) else {
        panic!("no stats for b");
    };
    assert_eq!(
        b.borrow().get(&Value::from("total_calls")),
        Some(&Value::Long(1))
    );
    // Statistics accumulate across runs and the call count is reset.
    let stack = exec(
        "'a' CHRONOSTART 'a' CHRONOEND 'a' CHRONOSTART 'a' CHRONOEND CHRONOSTATS 'a' GET 'total_calls' GET",
    );
    assert_eq!(stack.elements().last(), Some(&Value::Long(2)));
    assert_fails(
        "'a' CHRONOEND",
        "CHRONOEND called before CHRONOSTART for a.",
    );
    assert_fails(
        "'a' CHRONOSTART 'a' CHRONOEND 'a' CHRONOEND",
        "CHRONOEND called more times than CHRONOSTART for a.",
    );
    assert_fails("1 CHRONOSTART", "CHRONOSTART expects a STRING alias.");
    assert_fails("1 CHRONOEND", "CHRONOEND expects a STRING alias.");
    // Corrupt statistics are tolerated.
    let mut stack = test_stack();
    stack.set_attribute(
        ATTRIBUTE_CHRONOSTATS,
        Some(Value::map(IndexMap::from([
            (Value::from("x"), Value::Long(1)),
            (Value::from("y"), Value::list(vec![Value::Long(1)])),
        ]))),
    );
    stack
        .exec_checked("CHRONOSTATS 'x' GET 'total_time' GET")
        .unwrap();
    assert_eq!(stack.pop().unwrap(), Value::Long(0));
    stack
        .exec_checked("'x' CHRONOSTART 'x' CHRONOEND CHRONOSTATS 'x' GET 'total_calls' GET")
        .unwrap();
    assert_eq!(stack.pop().unwrap(), Value::Long(0));
    stack
        .exec_checked("'y' CHRONOSTART 'y' CHRONOEND CHRONOSTATS 'y' GET 'total_time' GET")
        .unwrap();
    assert_eq!(stack.pop().unwrap(), Value::Long(0));
}

#[test]
fn timed_wraps_a_macro() {
    assert_top("<% 1 2 + %> 'add' TIMED EVAL", 3);
    assert_top(
        "<% 1 2 + %> 'add' TIMED SNAPSHOT",
        "<% 'add' CHRONOSTART <% 1 2 + %> <% RETHROW %> <% 'add' CHRONOEND %> TRY %> ",
    );
    assert_top(
        "<% 1 2 + %> 'add' TIMED EVAL DROP CHRONOSTATS 'add' GET 'total_calls' GET",
        1,
    );
    // Errors are rethrown after the chronometer is stopped.
    assert_fails("<% FAIL %> 'f' TIMED EVAL", "FAIL failed.");
    assert_top(
        "<% <% FAIL %> 'f' TIMED EVAL %> <% %> <% %> TRY CHRONOSTATS 'f' GET 'total_calls' GET",
        1,
    );
    assert_fails("<% 1 %> 1 TIMED", "TIMED expects a STRING alias.");
    assert_fails(
        "1 'x' TIMED",
        "TIMED expects a macro under the top of the stack which will be timed.",
    );
}

// -----------------------------------------------------------------------------------------
// Interactive printers
// -----------------------------------------------------------------------------------------

/// A stack whose interactive output is captured.
fn interactive() -> (Stack, Rc<RefCell<Vec<u8>>>) {
    let buffer = Rc::new(RefCell::new(Vec::new()));
    let mut stack = test_stack();
    stack.set_interactive_writer(Some(buffer.clone()));
    (stack, buffer)
}

fn output(buffer: &Rc<RefCell<Vec<u8>>>) -> String {
    String::from_utf8(buffer.borrow().clone()).unwrap()
}

#[test]
fn printers_write_to_the_interactive_writer() {
    // Without a writer nothing happens.
    // (`PEEKN` only consumes its level when there is a writer, like Warp 10.)
    assert_stack(
        "1 2 PEEK PSTACK 1 NPEEK 0 PEEKN",
        &[Value::Long(1), Value::Long(2), Value::Long(0)],
    );
    assert_fails("PEEK", "Empty stack.");
    assert_fails("'x' PEEKN", "PEEKN expects a numeric value.");
    assert_fails("'x' NPEEK", "NPEEK expects a numeric value.");
    assert_fails("PEEKN", "Empty stack.");

    let (mut stack, buffer) = interactive();
    stack.exec_checked("1 'a' PEEK").unwrap();
    assert_eq!(output(&buffer), "/* 1 */ 'a' \n");
    buffer.borrow_mut().clear();
    stack.exec_checked("PSTACK").unwrap();
    assert_eq!(output(&buffer), "/* 2 */ 1 \n/* 1 */ 'a' \n");
    buffer.borrow_mut().clear();
    stack.exec_checked("1 PEEKN").unwrap();
    assert_eq!(output(&buffer), "/* 2 */ 1 \n");
    assert_eq!(stack.depth(), 2);
    buffer.borrow_mut().clear();
    stack.exec_checked("5 NPEEK").unwrap();
    assert_eq!(output(&buffer), "/* 2 */ 1 \n/* 1 */ 'a' \n");
    buffer.borrow_mut().clear();
    stack.exec_checked("0 NPEEK -1 NPEEK").unwrap();
    assert_eq!(output(&buffer), "");
    let err = stack.exec("7 PEEKN").unwrap_err().to_string();
    assert!(err.contains("Invalid level."), "{err}");
    let err = stack.exec("-1 PEEKN").unwrap_err().to_string();
    assert!(err.contains("Invalid level."), "{err}");
    assert_eq!(stack.depth(), 2);
    // JSON output.
    buffer.borrow_mut().clear();
    stack.exec_checked("JSONSTACK [ 1 'b' ] PEEK").unwrap();
    assert_eq!(output(&buffer), "/* 1 */ [1,\"b\"]\n");
    // Structures that cannot be rendered are errors.
    stack.exec_checked("WSSTACK [ ] DUP DUP +! ").unwrap();
    let err = stack.exec("PEEK").unwrap_err().to_string();
    assert!(
        err.contains("Recursive data structures exceeded 16 levels."),
        "{err}"
    );
    stack.exec_checked("JSONSTACK").unwrap();
    let err = stack.exec("PEEK").unwrap_err().to_string();
    assert!(err.contains("StackOverflowError"), "{err}");
}

#[test]
fn printer_write_errors_are_reported() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut stack = test_stack();
    stack.set_interactive_writer(Some(Rc::new(RefCell::new(Broken))));
    assert!(stack.interactive_writer().is_some());
    let err = stack.exec("1 PEEK").unwrap_err().to_string();
    assert!(err.contains("(closed)"), "{err}");
}

// -----------------------------------------------------------------------------------------
// Time and platform
// -----------------------------------------------------------------------------------------

#[test]
fn time_functions() {
    let before = current_time();
    let now = top("NOW").as_i64().unwrap();
    assert!(now >= before && now <= current_time());
    let ago = top("1000000 AGO").as_i64().unwrap();
    assert!(ago <= current_time() - 1_000_000);
    assert!(top("1.5 AGO").as_i64().unwrap() <= current_time());
    assert_fails(
        "'x' AGO",
        "AGO expects an offset in time units on top of the stack.",
    );
    assert_top("MSTU", 1000);
    assert_top("STU", 1_000_000);
}

#[test]
fn revisions() {
    assert_top("REV", REVISION);
    assert_top("'3' MINREV", true);
    assert_top("'3.5' MINREV", true);
    assert_top("'3.5.0' MINREV", true);
    assert_top("'3.5.1' MINREV", false);
    assert_top("'3.5.0.1' MINREV", false);
    assert_top("'4' MINREV", false);
    assert_top("'2.9-12-abcd' MINREV", true);
    assert_top("'3.5.0' MAXREV", true);
    assert_top("'3.4.9' MAXREV", false);
    assert_top("'3.6' MAXREV", true);
    assert_top("'3.5.0.1' MAXREV", true);
    assert_fails(
        "'x.y' MINREV",
        "MINREV expects the given revision to be of the form 'X', 'X.Y' or 'X.Y.Z'. X, Y and Z are integers.",
    );
    assert_fails(
        "'' MAXREV",
        "MAXREV expects the given revision to be of the form 'X', 'X.Y' or 'X.Y.Z'. X, Y and Z are integers.",
    );
    assert_fails(
        "1 MINREV",
        "MINREV expects a revision string on top of the stack.",
    );
    // A platform without a proper revision cannot be compared.
    assert_eq!(
        super::runtime::check_rev("MINREV", "UNAVAILABLE", "3")
            .unwrap_err()
            .message(),
        "MINREV expects the revision to be set on the platform to a string of the form 'X.Y.Z'. X, Y and Z are integers."
    );
    assert_eq!(
        super::runtime::check_rev("MINREV", "3.5.0-1-gb576436", "3.5").unwrap(),
        0
    );
    assert_fails(
        "1 MAXREV",
        "MAXREV expects a revision string on top of the stack.",
    );
}

#[test]
fn documentation_urls() {
    assert_top("RTFM", "http://www.warp10.io/");
    assert_top("MAN", "http://www.warp10.io/");
    assert_top("1 MAN", "http://www.warp10.io/");
    assert_top("'DUP' MAN", "http://www.warp10.io/doc/DUP");
    assert_top(
        "'nope' MAN",
        "Unknown function name, please check http://www.warp10.io/",
    );
    // Names with special characters, and the documented exceptions, are OPB64 encoded.
    assert_top("'+' MAN", "http://www.warp10.io/doc/9k");
    assert_top(
        "'->HEX' MAN",
        format!(
            "http://www.warp10.io/doc/{}",
            crate::encoding::opb64_encode(b"->HEX")
        ),
    );
    assert_top(
        "'pi' MAN",
        format!(
            "http://www.warp10.io/doc/{}",
            crate::encoding::opb64_encode(b"pi")
        ),
    );
    assert_top(
        "'-' MAN",
        format!(
            "http://www.warp10.io/doc/{}",
            crate::encoding::opb64_encode(b"-")
        ),
    );
    assert_top("'NaN' MAN", "http://www.warp10.io/doc/NaN");
}

#[test]
fn extensions_and_report() {
    assert_top(
        "'io.warp10.script.ext.debug.DebugWarpScriptExtension' EXTLOADED",
        false,
    );
    assert_fails("1 EXTLOADED", "EXTLOADED expects a String extension name.");
    assert_fails("REPORT", "REPORT missing capability.");
    let mut stack = Stack::with_limits(Limits::unlimited());
    stack
        .capabilities_mut()
        .insert("report".to_owned(), String::new());
    stack.exec_checked("REPORT").unwrap();
    let report = stack.pop().unwrap().to_string();
    assert!(
        report.starts_with(&format!("\n[revision]\n{REVISION}\n")),
        "{report}"
    );
    assert!(report.contains("[runtime]\nname=warpscript\n"), "{report}");
}

// -----------------------------------------------------------------------------------------
// SNAPSHOT family
// -----------------------------------------------------------------------------------------

#[test]
fn snapshot_variants() {
    assert_stack("1 'a' [ 2 ] SNAPSHOT", &[Value::from("1 'a' [] 2 +! ")]);
    assert_stack("SNAPSHOT", &[Value::from("")]);
    assert_stack(
        "1 'a' SNAPSHOTCOPY",
        &[Value::Long(1), Value::from("a"), Value::from("1 'a' ")],
    );
    // Readable output.
    assert_stack(
        "'it%27s' { 'k' [ 1 2 ] } SNAPSHOTREADABLE",
        &[Value::from("'it%27s' {\n'k' [ 1 2 ] \n} ")],
    );
    // Snapshots re-evaluate to the same stack.
    assert_stack(
        "1 'a%25' [ 2 ( 3 ) { 'x' NULL } ] SNAPSHOT EVAL SNAPSHOT",
        &[Value::from(
            "1 'a%25' [] 2 +! () 3 +! +! {} NULL 'x' PUT +! ",
        )],
    );
}

#[test]
fn snapshot_to_mark() {
    assert_stack(
        "1 MARK 2 3 SNAPSHOTTOMARK",
        &[Value::Long(1), Value::from("2 3 ")],
    );
    assert_stack(
        "1 MARK 2 3 SNAPSHOTCOPYTOMARK",
        &[
            Value::Long(1),
            Value::Mark,
            Value::Long(2),
            Value::Long(3),
            Value::from("2 3 "),
        ],
    );
    // The mark at the bottom: the stack is cleared.
    assert_stack("MARK 2 3 SNAPSHOTTOMARK", &[Value::from("2 3 ")]);
    assert_stack("1 MARK SNAPSHOTTOMARK", &[Value::Long(1), Value::from("")]);
    assert_fails(
        "1 2 SNAPSHOTTOMARK",
        "SNAPSHOTTOMARK expects a MARK on the stack.",
    );
    assert_fails(
        "SNAPSHOTCOPYTOMARK",
        "SNAPSHOTCOPYTOMARK expects a MARK on the stack.",
    );
}

#[test]
fn snapshot_n_levels() {
    assert_stack("1 2 3 2 SNAPSHOTN", &[Value::Long(1), Value::from("2 3 ")]);
    assert_stack(
        "1 2 3 2 SNAPSHOTCOPYN",
        &[
            Value::Long(1),
            Value::Long(2),
            Value::Long(3),
            Value::from("2 3 "),
        ],
    );
    // More levels than the stack holds, zero or negative levels.
    assert_stack("1 2 5 SNAPSHOTN", &[Value::from("1 2 ")]);
    assert_stack(
        "1 2 0 SNAPSHOTN",
        &[Value::Long(1), Value::Long(2), Value::from("")],
    );
    assert_stack(
        "1 2 -3 SNAPSHOTN",
        &[Value::Long(1), Value::Long(2), Value::from("")],
    );
    assert_stack("3 SNAPSHOTN", &[Value::from("")]);
    assert_fails(
        "1 'x' SNAPSHOTN",
        "SNAPSHOTN expects a number of stack levels to snapshot.",
    );
    assert_fails("SNAPSHOTCOPYN", "Empty stack.");
}

#[test]
fn snapshot_all_includes_symbols_and_registers() {
    assert_stack(
        "1 'a' STORE <% 2 %> 'm' STORE 3 POPR1 4 SNAPSHOTALL",
        &[Value::from(
            "4 1 'a' STORE <% 2 %> 'm' STORE CLEARREGS 3 POPR1 ",
        )],
    );
    assert_stack(
        "5 SNAPSHOTCOPYALL",
        &[Value::Long(5), Value::from("5 CLEARREGS ")],
    );
    assert_stack(
        "1 'a' STORE 7 MARK 8 SNAPSHOTALLTOMARK",
        &[Value::Long(7), Value::from("8 1 'a' STORE CLEARREGS ")],
    );
    assert_stack(
        "1 'a' STORE 7 MARK 8 SNAPSHOTCOPYALLTOMARK",
        &[
            Value::Long(7),
            Value::Mark,
            Value::Long(8),
            Value::from("8 1 'a' STORE CLEARREGS "),
        ],
    );
    // The snapshot recreates the symbols and registers.
    let stack = exec("1 'a' STORE 3 POPR1 SNAPSHOTALL 'a' FORGET CLEARREGS EVAL");
    assert_eq!(stack.load("a"), Some(&Value::Long(1)));
    assert_eq!(stack.load_register(1).unwrap(), &Value::Long(3));
}

#[test]
fn snapshot_errors_on_deep_structures() {
    let mut nested = Value::Long(1);
    for _ in 0..16 {
        nested = Value::list(vec![nested]);
    }
    let mut stack = test_stack();
    stack.push(nested.clone());
    let err = stack.exec("SNAPSHOT").unwrap_err().to_string();
    assert!(
        err.contains("Recursive data structures exceeded 16 levels."),
        "{err}"
    );
    // Also through the symbols and registers.
    let mut stack = test_stack();
    stack.store("deep", nested.clone()).unwrap();
    assert!(stack.exec("SNAPSHOTALL").is_err());
    let mut stack = test_stack();
    stack.store_register(0, nested).unwrap();
    assert!(stack.exec("SNAPSHOTALL").is_err());
}
