// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Runtime introspection and control functions of WarpLib: symbols and functions
//! (`SYMBOLS`, `FUNCTIONS`, `FUNCREF`, `REF`, `DEREF`, `MACRO->`, `->MACRO`, `REDEFS`,
//! `MSEC`, `MRSEC`, `IMPORT`, `GUARD`), stack attributes (`STACKATTRIBUTE`, `EXPORT`,
//! `GETSECTION`, `INFOMODE`, `HEADER`, the `ECHO` / `TIME` / `LINE` / `DEBUG` / `JSONSTACK`
//! toggles, `STMTPOS`), timing (`TIMINGS`, `NOTIMINGS`, `ELAPSED`, `TIMED`, `CHRONOSTART`,
//! `CHRONOEND`, `CHRONOSTATS`), the interactive printers (`PEEK`, `PEEKN`, `NPEEK`,
//! `PSTACK`), time (`NOW`, `AGO`, `MSTU`, `STU`), the platform (`REV`, `MINREV`, `MAXREV`,
//! `RTFM`, `MAN`, `EXTLOADED`, `REPORT`) and `CALL`.

use std::rc::Rc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;

use super::Entry;
use super::args::{fail, fail_with_cause, pop_macro, pop_string};
use super::misc::{ATTRIBUTE_INFOMODE, TIME_UNITS_PER_MS};
use crate::encoding::opb64_encode;
use crate::error::fault::ResourceError;
use crate::error::stack::StackError;
use crate::error::{Error, Result};
use crate::macros::{Callable, Macro, StackFunction, Statement};
use crate::snapshot::add_element;
use crate::stack::{REVISION, Stack};
use crate::value::Value;

/// Stack attribute selecting the JSON output of the interactive printers.
pub const ATTRIBUTE_INTERACTIVE_JSON: &str = "interactive.json";
/// Stack attribute set by `ECHOON` (`ATTRIBUTE_INTERACTIVE_ECHO`).
pub const ATTRIBUTE_INTERACTIVE_ECHO: &str = "interactive.echo";
/// Stack attribute set by `TIMEON` (`ATTRIBUTE_INTERACTIVE_TIME`).
pub const ATTRIBUTE_INTERACTIVE_TIME: &str = "interactive.time";
/// Stack attribute set by `LINEON` (`ATTRIBUTE_LINENO`).
pub const ATTRIBUTE_LINENO: &str = "lineno";
/// Stack attribute set by `DEBUGON` / `NDEBUGON` / `DEBUGOFF` (`ATTRIBUTE_DEBUG_DEPTH`).
pub const ATTRIBUTE_DEBUG_DEPTH: &str = "debug.depth";
/// Stack attribute set by `TIMINGS` / `NOTIMINGS` (`ATTRIBUTE_TIMINGS`).
pub const ATTRIBUTE_TIMINGS: &str = "timings";
/// Stack attribute holding the per-line elapsed times (`ATTRIBUTE_ELAPSED`).
pub const ATTRIBUTE_ELAPSED: &str = "elapsed";
/// Stack attribute holding the response headers set by `HEADER` (`ATTRIBUTE_HEADERS`).
pub const ATTRIBUTE_HEADERS: &str = "response.headers";
/// Stack attribute holding the symbols exported by `EXPORT` (`ATTRIBUTE_EXPORTED_SYMBOLS`).
pub const ATTRIBUTE_EXPORTED_SYMBOLS: &str = "exported.symbols";
/// Stack attribute set by `STMTPOS` (`ATTRIBUTE_WRAPPED_STATEMENT_FACTORY`).
pub const ATTRIBUTE_WRAPPED_STATEMENT_FACTORY: &str = "wrapped.statement.factory";
/// Stack attribute holding the `CHRONOSTATS` statistics.
pub const ATTRIBUTE_CHRONOSTATS: &str = "chronostats";
/// The pseudo attributes computed by the stack (`getAttribute`).
pub const ATTRIBUTE_OPS: &str = "stack.ops";
/// The section name pseudo attribute.
pub const ATTRIBUTE_SECTION_NAME: &str = "section.name";
/// The macro name pseudo attribute.
pub const ATTRIBUTE_MACRO_NAME: &str = "macro.name";
/// The secure macro pseudo attribute.
pub const ATTRIBUTE_IN_SECURE_MACRO: &str = "in.secure.macro";

/// `Constants.WARP10_DOC_URL`.
pub const WARP10_DOC_URL: &str = "http://www.warp10.io/";
/// `Constants.WARP10_FUNCTION_DOC_URL`.
pub const WARP10_FUNCTION_DOC_URL: &str = "http://www.warp10.io/doc/";

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("SYMBOLS", Entry::Stack(symbols)),
    ("FUNCTIONS", Entry::Stack(functions)),
    ("FUNCREF", Entry::Stack(funcref)),
    ("REF", Entry::Stack(reference)),
    ("DEREF", Entry::Stack(deref)),
    ("MACRO->", Entry::Stack(macroto)),
    ("->MACRO", Entry::Stack(tomacro)),
    ("REDEFS", Entry::Stack(redefs)),
    ("MSEC", Entry::Stack(msec)),
    ("MRSEC", Entry::Stack(mrsec)),
    ("IMPORT", Entry::Stack(import)),
    ("GUARD", Entry::Stack(guard)),
    ("CALL", Entry::Stack(call)),
    ("STACKATTRIBUTE", Entry::Stack(stackattribute)),
    ("EXPORT", Entry::Stack(export)),
    ("GETSECTION", Entry::Stack(getsection)),
    ("INFOMODE", Entry::Stack(infomode)),
    ("HEADER", Entry::Stack(header)),
    ("ECHOON", Entry::Stack(echoon)),
    ("ECHOOFF", Entry::Stack(echooff)),
    ("TIMEON", Entry::Stack(timeon)),
    ("TIMEOFF", Entry::Stack(timeoff)),
    ("LINEON", Entry::Stack(lineon)),
    ("LINEOFF", Entry::Stack(lineoff)),
    ("DEBUGON", Entry::Stack(debugon)),
    ("NDEBUGON", Entry::Stack(ndebugon)),
    ("DEBUGOFF", Entry::Stack(debugoff)),
    ("JSONSTACK", Entry::Stack(jsonstack)),
    ("WSSTACK", Entry::Stack(wsstack)),
    ("STMTPOS", Entry::Stack(stmtpos)),
    ("TIMINGS", Entry::Stack(timings)),
    ("NOTIMINGS", Entry::Stack(notimings)),
    ("ELAPSED", Entry::Stack(elapsed)),
    ("TIMED", Entry::Stack(timed)),
    ("CHRONOSTART", Entry::Stack(chronostart)),
    ("CHRONOEND", Entry::Stack(chronoend)),
    ("CHRONOSTATS", Entry::Stack(chronostats)),
    ("PEEK", Entry::Stack(peek)),
    ("PEEKN", Entry::Stack(peekn)),
    ("NPEEK", Entry::Stack(npeek)),
    ("PSTACK", Entry::Stack(pstack)),
    ("NOW", Entry::Stack(now)),
    ("AGO", Entry::Stack(ago)),
    ("MSTU", Entry::Stack(mstu)),
    ("STU", Entry::Stack(stu)),
    ("REV", Entry::Stack(rev)),
    ("MINREV", Entry::Stack(minrev)),
    ("MAXREV", Entry::Stack(maxrev)),
    ("RTFM", Entry::Stack(rtfm)),
    ("MAN", Entry::Stack(man)),
    ("EXTLOADED", Entry::Stack(extloaded)),
    ("REPORT", Entry::Stack(report)),
    ("TDESCRIBE", Entry::Stack(tdescribe)),
    ("VARS", Entry::Stack(vars)),
    ("STDOUT", Entry::Stack(stdout)),
    ("STDERR", Entry::Stack(stderr)),
    ("SLEEP", Entry::Stack(sleep)),
];

// -----------------------------------------------------------------------------------------
// Symbols and functions
// -----------------------------------------------------------------------------------------

/// `SYMBOLS`: the names of the symbols currently defined.
fn symbols(stack: &mut Stack) -> Result<()> {
    let names: Vec<Value> = stack
        .symbols()
        .keys()
        .map(|k| Value::from(k.as_str()))
        .collect();
    stack.push(Value::list(names));
    Ok(())
}

/// `FUNCTIONS`: the names of every function of the library (requires the `inventory`
/// capability).
fn functions(stack: &mut Stack) -> Result<()> {
    if stack.capability("inventory").is_none() {
        return Err(fail("FUNCTIONS", "missing capability 'inventory'."));
    }
    let names: Vec<Value> = super::registered_names()
        .into_iter()
        .map(Value::from)
        .collect();
    stack.push(Value::list(names));
    Ok(())
}

/// `FUNCREF`: the function bound to a name, as a value.
fn funcref(stack: &mut Stack) -> Result<()> {
    let name = pop_string(stack, "FUNCREF", "expects the function name as a STRING.")?;
    let function = stack.function_ref(&name)?;
    stack.push(function);
    Ok(())
}

/// `REF`: pushes the `REF` function itself.
fn reference(stack: &mut Stack) -> Result<()> {
    stack.push(StackFunction::value(Callable::Native {
        name: "REF",
        func: reference,
    }));
    Ok(())
}

/// `DEREF`: a copy of a macro where `'x' LOAD` (and `$x`) is replaced by the value bound to
/// `x` in the map, and `PUSHRn` by the value bound to the `LONG` `n`. Macro values replace the
/// load by their statements (a single statement) or by `value EVAL`.
fn deref(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "DEREF";
    let Value::Map(values) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a map of variable names or register id to values on top of the stack.",
        ));
    };
    let macro_ = pop_macro(stack, NAME, "operates on a Macro.")?;
    let values = values.borrow();
    let result = deref_macro(&macro_, &values)?;
    stack.push(Value::Macro(result));
    Ok(())
}

/// The macro `m` with its loads and register pushes substituted (recursively).
fn deref_macro(m: &Macro, values: &IndexMap<Value, Value>) -> Result<Rc<Macro>> {
    if m.is_secure() {
        return Err(fail("DEREF", "cannot operate on a secure Macro."));
    }
    let mut statements: Vec<Statement> = Vec::with_capacity(m.statements.len());
    for stmt in &m.statements {
        match stmt {
            Statement::Push(Value::Macro(inner)) => {
                statements.push(Statement::Push(Value::Macro(deref_macro(inner, values)?)));
            }
            Statement::Call(callable) if callable.name() == "LOAD" => {
                let symbol = match statements.last() {
                    Some(Statement::Push(symbol @ Value::String(_))) => values.get(symbol),
                    _ => None,
                };
                match symbol {
                    Some(Value::Macro(value)) if value.statements.len() == 1 => {
                        statements.pop();
                        statements.push(value.statements[0].clone());
                    }
                    Some(Value::Macro(value)) => {
                        statements.pop();
                        statements.push(Statement::Push(Value::Macro(Rc::clone(value))));
                        statements.push(Statement::Call(Callable::Native {
                            name: "EVAL",
                            func: super::symbols::eval,
                        }));
                    }
                    Some(value) => {
                        statements.pop();
                        statements.push(Statement::Push(value.clone()));
                    }
                    None => statements.push(stmt.clone()),
                }
            }
            Statement::Call(callable) if callable.name().starts_with("PUSHR") => {
                let register = callable.name()[5..].parse::<i64>().ok().map(Value::Long);
                match register.and_then(|r| values.get(&r)) {
                    Some(Value::Macro(value)) => {
                        // `MacroHelper.wrap`: the macro is executed in place.
                        statements.push(Statement::Push(Value::Macro(Rc::clone(value))));
                        statements.push(Statement::Call(Callable::Native {
                            name: "EVAL",
                            func: super::symbols::eval,
                        }));
                    }
                    Some(value) => statements.push(Statement::Push(value.clone())),
                    None => statements.push(stmt.clone()),
                }
            }
            other => statements.push(other.clone()),
        }
    }
    let result = Macro::from_statements(statements);
    Ok(Rc::new(result))
}

/// `MACRO->`: the statements of a (non secure) macro as a list; functions become function
/// objects.
fn macroto(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MACRO->";
    let m = pop_macro(stack, NAME, "operates on a MACRO.")?;
    if m.is_secure() {
        return Err(fail(NAME, "cannot operate on a secure MACRO."));
    }
    let statements = m
        .statements
        .iter()
        .map(|stmt| match stmt {
            Statement::Push(value) => value.clone(),
            Statement::Call(callable) => StackFunction::value(callable.clone()),
        })
        .collect();
    stack.push(Value::list(statements));
    Ok(())
}

/// `->MACRO`: a macro made of the elements of a list; function objects become calls.
fn tomacro(stack: &mut Stack) -> Result<()> {
    let Value::List(list) = stack.pop()? else {
        return Err(fail("->MACRO", "operates on a LIST."));
    };
    let statements = list
        .borrow()
        .iter()
        .map(|value| match StackFunction::of(value) {
            Some(callable) => Statement::Call(callable.clone()),
            None => Statement::Push(value.clone()),
        })
        .collect();
    stack.push(Value::Macro(Rc::new(Macro::from_statements(statements))));
    Ok(())
}

/// `REDEFS`: allow (or forbid) calling redefined functions.
fn redefs(stack: &mut Stack) -> Result<()> {
    let Value::Boolean(allow) = stack.pop()? else {
        return Err(fail("REDEFS", "expects a boolean on top of the stack."));
    };
    stack.set_allow_redefined(allow);
    Ok(())
}

/// `MSEC`: mark a macro as secure.
fn msec(stack: &mut Stack) -> Result<()> {
    let m = pop_macro(stack, "MSEC", "operates on a Macro.")?;
    m.secure.set(true);
    stack.push(Value::Macro(m));
    Ok(())
}

/// `MRSEC`: mark a macro and its nested macros as secure.
fn mrsec(stack: &mut Stack) -> Result<()> {
    let m = pop_macro(stack, "MRSEC", "operates on a Macro.")?;
    m.set_secure_recursive(true);
    stack.push(Value::Macro(m));
    Ok(())
}

/// `IMPORT`: alias a macro namespace (`NULL` removes the alias).
fn import(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "IMPORT";
    let alias = pop_string(
        stack,
        NAME,
        "expects an alias for the imported namespace on top of the stack.",
    )?;
    let imported = match stack.pop()? {
        Value::Null => None,
        Value::String(s) => Some(s),
        _ => {
            return Err(fail(
                NAME,
                "expects the name of the imported namespace or null below its alias.",
            ));
        }
    };
    stack.import(&alias, imported.as_deref());
    Ok(())
}

/// `GUARD`: run a macro so that no information leaks out of it: the symbols (or registers)
/// of the list, or everything not in the context, are cleared afterwards, the stack below
/// the optional hidden levels is protected, and any error clears the stack and is replaced
/// by `Exception in GUARDed macro.`. Capabilities changed by the macro are reverted except
/// those exported with `CAPEXPORT`.
fn guard(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "GUARD";
    let mut top = stack.pop()?;
    let mut hide: Option<Option<i64>> = None;
    match top {
        Value::Null => {
            hide = Some(None);
            top = stack.pop()?;
        }
        Value::Long(levels) => {
            hide = Some(Some(levels));
            top = stack.pop()?;
        }
        _ => {}
    }
    let mut symbols = None;
    let mut context = None;
    match top {
        Value::List(list) => {
            symbols = Some(list);
            top = stack.pop()?;
        }
        Value::Context(saved) => {
            context = Some(saved);
            top = stack.pop()?;
        }
        _ => {}
    }
    let Value::Macro(m) = top else {
        return Err(fail(NAME, "operates on a MACRO."));
    };
    let hidden = hide.map(|levels| match levels {
        None => stack.hide_all(),
        Some(levels) => stack.hide(i64::from(levels as i32)),
    });
    let saved_capabilities = stack.capabilities().clone();
    let outer_exported = stack.set_exported_capabilities(Some(Vec::new()));

    let result = stack.exec_macro(&m);
    if result.is_err() {
        stack.clear();
    }

    if let Some(symbols) = symbols {
        let symbols = symbols.borrow();
        if symbols.is_empty() {
            stack.clear_registers();
            stack.forget_all();
        } else {
            for symbol in symbols.iter() {
                match symbol {
                    Value::String(name) => stack.forget(name),
                    Value::Long(register) => {
                        // Out of range registers are ignored like `store(int, null)` failures.
                        let _ = stack.store_register(i64::from(*register as i32), Value::Null);
                    }
                    _ => {}
                }
            }
        }
    } else if let Some(context) = context {
        stack.restore(&context);
    }
    if let Some(hidden) = hidden {
        stack.show(hidden as i64);
    }

    let exported = stack
        .set_exported_capabilities(outer_exported)
        .unwrap_or_default();
    let copy_exported = exported.contains(&None);
    let new_capabilities = std::mem::replace(stack.capabilities_mut(), saved_capabilities);
    if result.is_ok() {
        for cap in exported.iter().flatten() {
            if copy_exported
                && let Some(outer) = stack.exported_capabilities_mut()
                && !outer.contains(&Some(cap.clone()))
            {
                outer.push(Some(cap.clone()));
            }
            stack.capabilities_mut().shift_remove(cap);
            if let Some(value) = new_capabilities.get(cap) {
                stack.capabilities_mut().insert(cap.clone(), value.clone());
            }
        }
    }
    result.map_err(|_| Error::Abort("Exception in GUARDed macro.".to_owned()))
}

/// `CALL`: external subprocesses are not part of this runtime.
fn call(stack: &mut Stack) -> Result<()> {
    let Value::String(_) = stack.pop()? else {
        return Err(fail(
            "CALL",
            "expects a subprogram name on top of the stack.",
        ));
    };
    Err(fail("CALL", "Not supported."))
}

// -----------------------------------------------------------------------------------------
// Attributes
// -----------------------------------------------------------------------------------------

/// `STACKATTRIBUTE`: the value of a stack attribute (`NULL` when unset or internal).
fn stackattribute(stack: &mut Stack) -> Result<()> {
    let key = pop_string(
        stack,
        "STACKATTRIBUTE",
        "expects a string as the attribute key.",
    )?;
    let value = match &*key {
        ATTRIBUTE_OPS => Value::Long(stack.ops() as i64),
        ATTRIBUTE_SECTION_NAME => Value::from(stack.section_name()),
        ATTRIBUTE_MACRO_NAME => stack.macro_name().map_or(Value::Null, Value::from),
        ATTRIBUTE_IN_SECURE_MACRO => Value::Boolean(stack.in_secure_macro()),
        _ => stack.attribute(&key).cloned().unwrap_or(Value::Null),
    };
    stack.push(value);
    Ok(())
}

/// `EXPORT`: declare the symbols (a name or a list of names, `NULL` meaning all) to export at
/// the end of the execution.
fn export(stack: &mut Stack) -> Result<()> {
    const MESSAGE: &str = "expects a symbol name or a list thereof on top of the stack.";
    let exported = match stack.pop()? {
        Value::String(name) => {
            let mut set = match stack.attribute(ATTRIBUTE_EXPORTED_SYMBOLS) {
                Some(Value::Set(set)) => set.borrow().clone(),
                _ => indexmap::IndexSet::new(),
            };
            set.insert(Value::String(name));
            set
        }
        Value::List(names) => {
            let names = names.borrow();
            if names
                .iter()
                .any(|name| !matches!(name, Value::Null | Value::String(_)))
            {
                return Err(fail("EXPORT", MESSAGE));
            }
            let mut set: indexmap::IndexSet<Value> = names.iter().cloned().collect();
            if set.contains(&Value::Null) {
                set.clear();
                set.insert(Value::Null);
            }
            set
        }
        _ => return Err(fail("EXPORT", MESSAGE)),
    };
    stack.set_attribute(ATTRIBUTE_EXPORTED_SYMBOLS, Some(Value::set(exported)));
    Ok(())
}

/// `GETSECTION`: the name of the current section.
fn getsection(stack: &mut Stack) -> Result<()> {
    let name = Value::from(stack.section_name());
    stack.push(name);
    Ok(())
}

/// `INFOMODE`: toggle the info mode.
fn infomode(stack: &mut Stack) -> Result<()> {
    if matches!(
        stack.attribute(ATTRIBUTE_INFOMODE),
        Some(Value::Boolean(true))
    ) {
        stack.set_attribute(ATTRIBUTE_INFOMODE, None);
    } else {
        stack.set_attribute(ATTRIBUTE_INFOMODE, Some(Value::Boolean(true)));
    }
    Ok(())
}

/// `HEADER`: set (or remove, with a `NULL` value) a response header; only `X-` headers may be
/// set.
fn header(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "HEADER";
    let value = match stack.pop()? {
        Value::Null => None,
        other => Some(other.to_string()),
    };
    let name = pop_string(
        stack,
        NAME,
        "expects a header name (a string) below the header value.",
    )?;
    let headers = match stack.attribute(ATTRIBUTE_HEADERS) {
        Some(Value::Map(headers)) => Rc::clone(headers),
        _ => {
            let headers = Rc::new(std::cell::RefCell::new(IndexMap::new()));
            stack.set_attribute(ATTRIBUTE_HEADERS, Some(Value::Map(Rc::clone(&headers))));
            headers
        }
    };
    match value {
        None => {
            headers.borrow_mut().shift_remove(&Value::String(name));
        }
        Some(value) => {
            if !name.to_uppercase().starts_with("X-") {
                return Err(fail(NAME, "only headers beginning with 'X-' can be set."));
            }
            headers
                .borrow_mut()
                .insert(Value::String(name), Value::from(value));
        }
    }
    Ok(())
}

/// Set `attribute` to `value` (a flag function).
fn set_flag(stack: &mut Stack, attribute: &str, value: Option<Value>) -> Result<()> {
    stack.set_attribute(attribute, value);
    Ok(())
}

/// `ECHOON`: echo the top `n` levels of the stack after each line in interactive mode.
fn echoon(stack: &mut Stack) -> Result<()> {
    let levels = super::args::pop_number(stack, "ECHOON", "expects a number of levels.")?;
    let levels = super::args::int_value(&levels).unwrap_or_default();
    set_flag(
        stack,
        ATTRIBUTE_INTERACTIVE_ECHO,
        Some(Value::Long(i64::from(levels))),
    )
}

fn echooff(stack: &mut Stack) -> Result<()> {
    set_flag(stack, ATTRIBUTE_INTERACTIVE_ECHO, None)
}

fn timeon(stack: &mut Stack) -> Result<()> {
    set_flag(
        stack,
        ATTRIBUTE_INTERACTIVE_TIME,
        Some(Value::Boolean(true)),
    )
}

fn timeoff(stack: &mut Stack) -> Result<()> {
    set_flag(stack, ATTRIBUTE_INTERACTIVE_TIME, None)
}

fn lineon(stack: &mut Stack) -> Result<()> {
    set_flag(stack, ATTRIBUTE_LINENO, Some(Value::Boolean(true)))
}

fn lineoff(stack: &mut Stack) -> Result<()> {
    set_flag(stack, ATTRIBUTE_LINENO, None)
}

fn debugon(stack: &mut Stack) -> Result<()> {
    set_flag(
        stack,
        ATTRIBUTE_DEBUG_DEPTH,
        Some(Value::Long(i64::from(i32::MAX))),
    )
}

/// `NDEBUGON`: show `n` levels of the stack in error reports.
fn ndebugon(stack: &mut Stack) -> Result<()> {
    let levels = super::args::pop_number(stack, "NDEBUGON", "expects a number as parameter.")?;
    let levels = super::args::int_value(&levels).unwrap_or_default();
    set_flag(
        stack,
        ATTRIBUTE_DEBUG_DEPTH,
        Some(Value::Long(i64::from(levels))),
    )
}

fn debugoff(stack: &mut Stack) -> Result<()> {
    set_flag(stack, ATTRIBUTE_DEBUG_DEPTH, Some(Value::Long(0)))
}

fn jsonstack(stack: &mut Stack) -> Result<()> {
    set_flag(
        stack,
        ATTRIBUTE_INTERACTIVE_JSON,
        Some(Value::Boolean(true)),
    )
}

fn wsstack(stack: &mut Stack) -> Result<()> {
    set_flag(
        stack,
        ATTRIBUTE_INTERACTIVE_JSON,
        Some(Value::Boolean(false)),
    )
}

/// `STMTPOS`: enable or disable the recording of statement positions. Positions are not
/// tracked by this runtime (see `docs/DIVERGENCES.md`); the flag is kept as an attribute.
fn stmtpos(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::Boolean(true) => set_flag(
            stack,
            ATTRIBUTE_WRAPPED_STATEMENT_FACTORY,
            Some(Value::Boolean(true)),
        ),
        Value::Boolean(false) => set_flag(stack, ATTRIBUTE_WRAPPED_STATEMENT_FACTORY, None),
        _ => Err(fail("STMTPOS", "expects a BOOLEAN.")),
    }
}

// -----------------------------------------------------------------------------------------
// Timing
// -----------------------------------------------------------------------------------------

fn timings(stack: &mut Stack) -> Result<()> {
    set_flag(stack, ATTRIBUTE_TIMINGS, Some(Value::Boolean(true)))
}

fn notimings(stack: &mut Stack) -> Result<()> {
    set_flag(stack, ATTRIBUTE_TIMINGS, Some(Value::Boolean(false)))
}

/// `ELAPSED`: the elapsed times recorded by the interpreter (an empty list when timings are
/// off).
fn elapsed(stack: &mut Stack) -> Result<()> {
    let elapsed = match stack.attribute(ATTRIBUTE_ELAPSED) {
        Some(Value::List(list)) => Value::list(list.borrow().clone()),
        _ => Value::list(Vec::new()),
    };
    stack.push(elapsed);
    Ok(())
}

/// `TIMED`: wrap a macro so that its execution time is recorded under an alias
/// (`alias CHRONOSTART macro <% RETHROW %> <% alias CHRONOEND %> TRY`).
fn timed(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "TIMED";
    let alias = pop_string(stack, NAME, "expects a STRING alias.")?;
    let m = pop_macro(
        stack,
        NAME,
        "expects a macro under the top of the stack which will be timed.",
    )?;
    let native = |name: &'static str, func: crate::macros::StackFn| {
        Statement::Call(Callable::Native { name, func })
    };
    let catch = Macro::from_statements(vec![native("RETHROW", super::control::rethrow)]);
    let finally = Macro::from_statements(vec![
        Statement::Push(Value::String(Rc::clone(&alias))),
        native("CHRONOEND", chronoend),
    ]);
    let timed = Macro::from_statements(vec![
        Statement::Push(Value::String(alias)),
        native("CHRONOSTART", chronostart),
        Statement::Push(Value::Macro(m)),
        Statement::Push(Value::Macro(Rc::new(catch))),
        Statement::Push(Value::Macro(Rc::new(finally))),
        native("TRY", super::control::r#try),
    ]);
    stack.push(Value::Macro(Rc::new(timed)));
    Ok(())
}

/// Attribute keys of a chrono alias.
fn chrono_keys(alias: &str) -> (String, String, String) {
    (
        format!("chrono_start_{alias}"),
        format!("chrono_active_{alias}"),
        format!("chrono_calls_{alias}"),
    )
}

/// A `LONG` attribute, 0 when unset.
fn long_attribute(stack: &Stack, key: &str) -> i64 {
    match stack.attribute(key) {
        Some(Value::Long(l)) => *l,
        _ => 0,
    }
}

/// `CHRONOSTART`: start (or nest) a chronometer for an alias.
fn chronostart(stack: &mut Stack) -> Result<()> {
    let alias = pop_string(stack, "CHRONOSTART", "expects a STRING alias.")?;
    let (start, active, total) = chrono_keys(&alias);
    let active_count = long_attribute(stack, &active) + 1;
    stack.set_attribute(&active, Some(Value::Long(active_count)));
    let total_count = long_attribute(stack, &total) + 1;
    stack.set_attribute(&total, Some(Value::Long(total_count)));
    if active_count == 1 {
        stack.set_attribute(&start, Some(Value::Long(nanos_since_start())));
    }
    Ok(())
}

/// `CHRONOEND`: stop the chronometer of an alias and record its statistics.
fn chronoend(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "CHRONOEND";
    let end = nanos_since_start();
    let alias = pop_string(stack, NAME, "expects a STRING alias.")?;
    let (start, active, total) = chrono_keys(&alias);
    if stack.attribute(&active).is_none() {
        return Err(fail(
            NAME,
            &format!("called before CHRONOSTART for {alias}."),
        ));
    }
    let active_count = long_attribute(stack, &active) - 1;
    stack.set_attribute(&active, Some(Value::Long(active_count)));
    if active_count > 0 {
        return Ok(());
    }
    if active_count < 0 {
        return Err(fail(
            NAME,
            &format!("called more times than CHRONOSTART for {alias}."),
        ));
    }
    let started = long_attribute(stack, &start);
    let total_count = long_attribute(stack, &total);
    let stats = match stack.attribute(ATTRIBUTE_CHRONOSTATS) {
        Some(Value::Map(stats)) => Rc::clone(stats),
        _ => {
            let stats = Rc::new(std::cell::RefCell::new(IndexMap::new()));
            stack.set_attribute(ATTRIBUTE_CHRONOSTATS, Some(Value::Map(Rc::clone(&stats))));
            stats
        }
    };
    let mut stats = stats.borrow_mut();
    let entry = stats
        .entry(Value::String(Rc::clone(&alias)))
        .or_insert_with(|| Value::list(vec![Value::Long(0), Value::Long(0)]));
    if let Value::List(counters) = entry {
        let mut counters = counters.borrow_mut();
        if let [Value::Long(time), Value::Long(calls)] = counters.as_mut_slice() {
            *time += end - started;
            *calls += total_count;
        }
    }
    drop(stats);
    stack.set_attribute(&total, Some(Value::Long(0)));
    Ok(())
}

/// `CHRONOSTATS`: the statistics of every alias (`total_time` in ns and `total_calls`), or
/// `NULL` when no chronometer was ever stopped.
fn chronostats(stack: &mut Stack) -> Result<()> {
    let result = match stack.attribute(ATTRIBUTE_CHRONOSTATS) {
        Some(Value::Map(stats)) => {
            let mut result = IndexMap::new();
            for (alias, counters) in stats.borrow().iter() {
                let (time, calls) = match counters {
                    Value::List(counters) => match counters.borrow().as_slice() {
                        [time, calls] => (time.clone(), calls.clone()),
                        _ => (Value::Long(0), Value::Long(0)),
                    },
                    _ => (Value::Long(0), Value::Long(0)),
                };
                let mut entry = IndexMap::new();
                entry.insert(Value::from("total_time"), time);
                entry.insert(Value::from("total_calls"), calls);
                result.insert(alias.clone(), Value::map(entry));
            }
            Value::map(result)
        }
        _ => Value::Null,
    };
    stack.push(result);
    Ok(())
}

/// A monotonic nanosecond clock (`System.nanoTime`).
fn nanos_since_start() -> i64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_nanos() as i64
}

// -----------------------------------------------------------------------------------------
// Interactive printers
// -----------------------------------------------------------------------------------------

/// `PSTACK.print`: `/* level */ snapshot` (or JSON) of one element.
fn print_level(stack: &Stack, level: usize, value: &Value) -> Result<String> {
    let json = matches!(
        stack.attribute(ATTRIBUTE_INTERACTIVE_JSON),
        Some(Value::Boolean(true))
    );
    let rendered = if json {
        super::json::to_json(value, super::json::JsonOptions::default())
            .map_err(|_| ResourceError::StackOverflow)?
    } else {
        let mut out = String::new();
        add_element(&mut out, value, false)?;
        out
    };
    Ok(format!("/* {} */ {rendered}\n", level + 1))
}

/// Print the levels `levels` (indices from the top) to the interactive writer, if any.
fn print_levels(stack: &mut Stack, levels: impl Iterator<Item = usize>) -> Result<()> {
    let Some(writer) = stack.interactive_writer() else {
        return Ok(());
    };
    let mut text = String::new();
    for level in levels {
        let value = stack.get(level)?.clone();
        text.push_str(&print_level(stack, level, &value)?);
    }
    let mut writer = writer.borrow_mut();
    writer
        .write_all(text.as_bytes())
        .and_then(|()| writer.flush())
        .map_err(|err| Error::new(err.to_string()))
}

/// `PEEK`: print the top of the stack.
fn peek(stack: &mut Stack) -> Result<()> {
    stack.peek()?;
    print_levels(stack, std::iter::once(0))
}

/// `PEEKN`: pop a level and print the element at that level.
fn peekn(stack: &mut Stack) -> Result<()> {
    if !stack.peek()?.is_number() {
        return Err(fail("PEEKN", "expects a numeric value."));
    }
    if stack.interactive_writer().is_none() {
        return Ok(());
    }
    // The top was just peeked: it is there.
    let level = stack.pop().unwrap_or(Value::Null);
    let level = super::args::int_value(&level).unwrap_or_default();
    let level = usize::try_from(level).map_err(|_| StackError::InvalidLevel)?;
    print_levels(stack, std::iter::once(level))
}

/// `NPEEK`: print the top `n` levels of the stack.
fn npeek(stack: &mut Stack) -> Result<()> {
    let count = stack.pop()?;
    let Some(count) = super::args::int_value(&count) else {
        return Err(fail("NPEEK", "expects a numeric value."));
    };
    let count = usize::try_from(count)
        .unwrap_or_default()
        .min(stack.depth());
    print_levels(stack, (0..count).rev())
}

/// `PSTACK`: print the whole stack.
fn pstack(stack: &mut Stack) -> Result<()> {
    let depth = stack.depth();
    print_levels(stack, (0..depth).rev())
}

// -----------------------------------------------------------------------------------------
// Time and platform
// -----------------------------------------------------------------------------------------

/// `TimeSource.getTime()`: the current time in platform time units (microseconds).
pub fn current_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_micros() as i64)
}

/// `NOW`: the current time in time units.
fn now(stack: &mut Stack) -> Result<()> {
    stack.push(Value::Long(current_time()));
    Ok(())
}

/// `AGO`: the current time minus an offset.
fn ago(stack: &mut Stack) -> Result<()> {
    let offset = super::args::pop_number(
        stack,
        "AGO",
        "expects an offset in time units on top of the stack.",
    )?;
    let offset = offset.as_i64().unwrap_or_default();
    stack.push(Value::Long(current_time().wrapping_sub(offset)));
    Ok(())
}

/// `MSTU`: the number of time units in a millisecond.
fn mstu(stack: &mut Stack) -> Result<()> {
    stack.push(Value::Long(TIME_UNITS_PER_MS));
    Ok(())
}

/// `STU`: the number of time units in a second.
fn stu(stack: &mut Stack) -> Result<()> {
    stack.push(Value::Long(1000 * TIME_UNITS_PER_MS));
    Ok(())
}

/// `REV`: the platform revision.
fn rev(stack: &mut Stack) -> Result<()> {
    stack.push(Value::from(REVISION));
    Ok(())
}

/// `CHECKREV.SplitRev`: the numeric components of a revision such as `1.2.3-42-abcd`.
fn split_rev(revision: &str, error: &str) -> Result<Vec<i32>> {
    let mut parts = Vec::new();
    let head = revision.split('-').next().unwrap_or_default();
    for part in head.split('.') {
        parts.push(part.parse::<i32>().map_err(|_| Error::new(error))?);
    }
    Ok(parts)
}

/// `CHECKREV.checkRev`: compare the platform revision `platform` with `rev` (negative when
/// the platform is older).
pub(crate) fn check_rev(name: &str, platform: &str, rev: &str) -> Result<i32> {
    let given = split_rev(
        rev,
        &format!(
            "{name} expects the given revision to be of the form 'X', 'X.Y' or 'X.Y.Z'. X, Y and Z are integers."
        ),
    )?;
    let current = split_rev(
        platform,
        &format!(
            "{name} expects the revision to be set on the platform to a string of the form 'X.Y.Z'. X, Y and Z are integers."
        ),
    )?;
    let mut comparison = 0;
    for (ours, theirs) in current.iter().zip(&given) {
        comparison = ours.cmp(theirs) as i32;
        if comparison != 0 {
            break;
        }
    }
    if comparison == 0 && given.len() > current.len() {
        comparison = -1;
    }
    Ok(comparison)
}

/// `MINREV` / `MAXREV`: whether the platform revision is at least / at most the given one.
fn checkrev(stack: &mut Stack, name: &str, min: bool) -> Result<()> {
    let Value::String(rev) = stack.pop()? else {
        return Err(fail(name, "expects a revision string on top of the stack."));
    };
    let comparison = check_rev(name, REVISION, &rev)?;
    stack.push(Value::Boolean(if min {
        comparison >= 0
    } else {
        comparison <= 0
    }));
    Ok(())
}

fn minrev(stack: &mut Stack) -> Result<()> {
    checkrev(stack, "MINREV", true)
}

fn maxrev(stack: &mut Stack) -> Result<()> {
    checkrev(stack, "MAXREV", false)
}

/// `RTFM`: the documentation URL.
fn rtfm(stack: &mut Stack) -> Result<()> {
    stack.push(Value::from(WARP10_DOC_URL));
    Ok(())
}

/// `MAN`: the documentation URL of the function named on top of the stack (or the general
/// documentation when the stack is empty or its top is not a string).
fn man(stack: &mut Stack) -> Result<()> {
    let mut function_name: Option<String> = None;
    if stack.depth() > 0
        && let Value::String(name) = stack.pop().unwrap_or(Value::Null)
    {
        function_name = Some(if super::lookup(&name).is_some() {
            name.to_string()
        } else {
            String::new()
        });
    }
    let url = match function_name.as_deref() {
        None => WARP10_DOC_URL.to_owned(),
        Some("") => format!("Unknown function name, please check {WARP10_DOC_URL}"),
        Some(name) => {
            let forbidden = name.contains([
                '!', '%', '&', '(', ')', '*', '+', '/', '<', '=', '>', '[', ']', '^', '{', '|',
                '}', '~',
            ]);
            let doc_name = if forbidden || ["-", "pi", "PI", "e", "E", "Pfilter"].contains(&name) {
                opb64_encode(name.as_bytes())
            } else {
                name.to_owned()
            };
            format!("{WARP10_FUNCTION_DOC_URL}{doc_name}")
        }
    };
    stack.push(Value::from(url));
    Ok(())
}

/// `EXTLOADED`: whether an extension is loaded; this runtime loads no Java extension.
fn extloaded(stack: &mut Stack) -> Result<()> {
    let Value::String(_) = stack.pop()? else {
        return Err(fail("EXTLOADED", "expects a String extension name."));
    };
    stack.push(Value::Boolean(false));
    Ok(())
}

/// `REPORT`: a report on the platform (requires the `report` capability). The JVM and
/// Sensision sections of Warp 10 have no equivalent here.
fn report(stack: &mut Stack) -> Result<()> {
    if stack.capability("report").is_none() {
        return Err(fail("REPORT", "missing capability."));
    }
    let mut report = String::from("\n[revision]\n");
    report.push_str(REVISION);
    report.push_str("\n\n[runtime]\n");
    report.push_str(&format!(
        "name={}\nversion={}\n",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    ));
    report.push_str("\n[extensions]\n\n[plugins]\n");
    stack.push(Value::from(report));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// SNAPSHOT family
// -----------------------------------------------------------------------------------------

/// The flags of one `SNAPSHOT` variant.
#[derive(Clone, Copy)]
struct SnapshotFlags {
    /// Also snapshot the symbols and registers (`SNAPSHOTALL`…).
    symbols: bool,
    /// Snapshot down to the first `MARK` (`SNAPSHOTTOMARK`…).
    to_mark: bool,
    /// Remove the snapshot elements from the stack (not the `COPY` variants).
    pop: bool,
    /// Snapshot the number of levels given on top of the stack (`SNAPSHOTN`…).
    count_based: bool,
    /// Readable output (`SNAPSHOTREADABLE`).
    readable: bool,
}

/// `SNAPSHOT` and its variants: the WarpScript source that recreates (part of) the stack.
fn snapshot_impl(stack: &mut Stack, name: &str, flags: SnapshotFlags) -> Result<()> {
    let mut last: i64;
    if flags.count_based {
        let Value::Long(levels) = stack.pop()? else {
            return Err(fail(name, "expects a number of stack levels to snapshot."));
        };
        last = i64::from(levels as i32) - 1;
        last = last.min(stack.depth() as i64 - 1);
    } else if !flags.to_mark {
        last = stack.depth() as i64 - 1;
    } else {
        let mark = (0..stack.depth()).find(|&i| matches!(stack.get(i), Ok(Value::Mark)));
        let Some(mark) = mark else {
            return Err(fail(name, "expects a MARK on the stack."));
        };
        last = mark as i64;
    }
    let mut out = String::new();
    let mut level = last;
    while level >= 0 {
        if !(flags.to_mark && level == last) {
            // `level` is within the stack by construction.
            let value = stack.get(level as usize).cloned().unwrap_or(Value::Null);
            add_element(&mut out, &value, flags.readable)?;
        }
        level -= 1;
    }
    if flags.symbols {
        let symbols: Vec<(String, Value)> = stack
            .symbols()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (symbol, value) in &symbols {
            add_element(&mut out, value, flags.readable)?;
            // A string never fails to render.
            add_element(&mut out, &Value::from(symbol.as_str()), flags.readable)
                .unwrap_or_default();
            out.push_str("STORE ");
        }
        out.push_str("CLEARREGS ");
        let registers: Vec<(usize, Value)> = stack
            .registers()
            .iter()
            .enumerate()
            .filter(|(_, v)| !v.is_null())
            .map(|(i, v)| (i, v.clone()))
            .collect();
        for (index, value) in &registers {
            add_element(&mut out, value, flags.readable)?;
            out.push_str(&format!("POPR{index} "));
        }
    }
    if flags.pop {
        if stack.depth() as i64 - 1 == last {
            stack.clear();
        } else {
            while last >= 0 {
                // There are at least `last + 1` elements.
                let _ = stack.pop();
                last -= 1;
            }
        }
    }
    stack.push(Value::from(out));
    Ok(())
}

/// Define the `SNAPSHOT` variants.
macro_rules! snapshot_functions {
    ($($rust:ident => $name:literal, $symbols:literal, $to_mark:literal, $pop:literal, $count:literal, $readable:literal;)*) => {
        $(
            fn $rust(stack: &mut Stack) -> Result<()> {
                snapshot_impl(
                    stack,
                    $name,
                    SnapshotFlags {
                        symbols: $symbols,
                        to_mark: $to_mark,
                        pop: $pop,
                        count_based: $count,
                        readable: $readable,
                    },
                )
            }
        )*

        /// The `SNAPSHOT` functions.
        pub const SNAPSHOT_FUNCTIONS: &[(&str, Entry)] = &[
            $( ($name, Entry::Stack($rust)), )*
        ];
    };
}

snapshot_functions! {
    snapshot => "SNAPSHOT", false, false, true, false, false;
    snapshotreadable => "SNAPSHOTREADABLE", false, false, true, false, true;
    snapshotall => "SNAPSHOTALL", true, false, true, false, false;
    snapshottomark => "SNAPSHOTTOMARK", false, true, true, false, false;
    snapshotalltomark => "SNAPSHOTALLTOMARK", true, true, true, false, false;
    snapshotcopy => "SNAPSHOTCOPY", false, false, false, false, false;
    snapshotcopyall => "SNAPSHOTCOPYALL", true, false, false, false, false;
    snapshotcopytomark => "SNAPSHOTCOPYTOMARK", false, true, false, false, false;
    snapshotcopyalltomark => "SNAPSHOTCOPYALLTOMARK", true, true, false, false, false;
    snapshotn => "SNAPSHOTN", false, false, true, true, false;
    snapshotcopyn => "SNAPSHOTCOPYN", false, false, false, true, false;
}

// -----------------------------------------------------------------------------------------
// TDESCRIBE / VARS / STDOUT / STDERR / SLEEP
// -----------------------------------------------------------------------------------------

/// `TDESCRIBE.rtypeof`: the type of a value, with the type of the first element of lists
/// and of the last entry of maps, down to the default recursion limit.
fn recursive_type(value: &Value, level: usize) -> String {
    if level > crate::stack::DEFAULT_MAX_RECURSION_LEVEL as usize {
        return "... (recursion limit reached)".to_owned();
    }
    match value {
        Value::List(list) => match list.borrow().first() {
            None => "LIST []".to_owned(),
            Some(first) => format!("LIST [ {} ]", recursive_type(first, level + 1)),
        },
        Value::Map(map) => match map.borrow().last() {
            None => "MAP {}".to_owned(),
            Some((key, entry)) => format!(
                "MAP {{ {} : {} }} ",
                recursive_type(key, level + 1),
                recursive_type(entry, level + 1)
            ),
        },
        other => other.type_name().to_owned(),
    }
}

/// `TDESCRIBE`: the recursive type description of the value on top of the stack.
fn tdescribe(stack: &mut Stack) -> Result<()> {
    let value = stack.pop()?;
    stack.push(Value::from(recursive_type(&value, 0)));
    Ok(())
}

/// The variables and registers found while scanning macros (`VARS.getVars`).
#[derive(Default)]
struct VarScan {
    symbols: Vec<Value>,
    occurrences: IndexMap<Value, usize>,
    pending: Vec<Rc<Macro>>,
}

impl VarScan {
    /// Count one occurrence of `symbol`, adding it to the result when `keep` is set.
    fn count(&mut self, symbol: &Value, keep: bool) {
        if keep && !self.symbols.contains(symbol) {
            self.symbols.push(symbol.clone());
        }
        *self.occurrences.entry(symbol.clone()).or_insert(0) += 1;
    }

    /// Scan the statements of one macro.
    fn scan(&mut self, statements: &[Statement], only_store_and_popr: bool) -> Result<()> {
        let abort = || StackError::NonExplicitSymbol.into();
        let register = |name: &str, prefix: &str| -> Option<Value> {
            name.strip_prefix(prefix)
                .and_then(|digits| digits.parse::<i64>().ok())
                .map(Value::Long)
        };
        for (i, stmt) in statements.iter().enumerate() {
            let Statement::Call(callable) = stmt else {
                if let Statement::Push(Value::Macro(inner)) = stmt {
                    self.pending.push(Rc::clone(inner));
                }
                continue;
            };
            let name = callable.name();
            if let Some(reg) = register(name, "POPR").or_else(|| register(name, "CPOPR")) {
                self.count(&reg, true);
            } else if let Some(reg) = register(name, "PUSHR").or_else(|| register(name, "RUNR")) {
                self.count(&reg, !only_store_and_popr);
            } else if matches!(name, "LOAD" | "CSTORE" | "RUN") {
                let Some(Statement::Push(previous @ (Value::String(_) | Value::Long(_)))) =
                    i.checked_sub(1).map(|p| &statements[p])
                else {
                    return Err(abort());
                };
                // `RUN` symbols may name repository macros: they are never returned.
                let keep = name == "CSTORE" || (name == "LOAD" && !only_store_and_popr);
                self.count(previous, keep);
            } else if name == "STORE" {
                let Some(previous) = i.checked_sub(1).map(|p| &statements[p]) else {
                    return Err(abort());
                };
                match previous {
                    Statement::Push(Value::List(list)) => {
                        for element in list.borrow().iter() {
                            match element {
                                Value::String(_) | Value::Long(_) => self.count(element, true),
                                Value::Null => {}
                                _ => return Err(abort()),
                            }
                        }
                    }
                    Statement::Call(end) if end.name() == "]" => {
                        // Walk back to the matching `[`, collecting the names.
                        let mut idx = i.checked_sub(2);
                        while let Some(at) = idx {
                            match &statements[at] {
                                Statement::Call(c) if matches!(c.name(), "[" | "MARK") => break,
                                Statement::Push(element @ (Value::String(_) | Value::Long(_))) => {
                                    self.count(element, true);
                                }
                                Statement::Push(Value::Null) => {}
                                Statement::Call(c) if c.name() == "NULL" => {}
                                _ => return Err(abort()),
                            }
                            idx = at.checked_sub(1);
                        }
                    }
                    Statement::Push(symbol @ (Value::String(_) | Value::Long(_))) => {
                        self.count(symbol, true);
                    }
                    _ => return Err(abort()),
                }
            }
        }
        Ok(())
    }
}

/// `VARS.getVars`: the variables and registers a macro (and its nested macros) uses, most
/// used first (ties broken by name). With `only_store_and_popr` only the symbols written by
/// `STORE`, `CSTORE` and `POPRn` are returned (the other uses still count as occurrences).
pub fn macro_vars(m: &Macro, only_store_and_popr: bool) -> Result<Vec<Value>> {
    let mut scan = VarScan::default();
    scan.scan(&m.statements, only_store_and_popr)?;
    let mut index = 0;
    while index < scan.pending.len() {
        let next = Rc::clone(&scan.pending[index]);
        scan.scan(&next.statements, only_store_and_popr)?;
        index += 1;
    }
    let VarScan {
        mut symbols,
        occurrences,
        ..
    } = scan;
    symbols.sort_by(|a, b| {
        let (oa, ob) = (
            occurrences.get(a).copied().unwrap_or_default(),
            occurrences.get(b).copied().unwrap_or_default(),
        );
        ob.cmp(&oa).then_with(|| a.to_string().cmp(&b.to_string()))
    });
    Ok(symbols)
}

/// `VARS`: the variables and registers used by a macro (optionally only those written to).
fn vars(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "VARS";
    let mut top = stack.pop()?;
    let mut only_store_and_popr = false;
    if let Value::Boolean(flag) = top {
        only_store_and_popr = flag;
        top = stack.pop()?;
    }
    let Value::Macro(m) = top else {
        return Err(fail(NAME, "operates on a macro."));
    };
    let vars =
        macro_vars(&m, only_store_and_popr).map_err(|err| fail_with_cause(NAME, "failed.", err))?;
    stack.push(Value::list(vars));
    Ok(())
}

/// `STDOUT` / `STDERR`: print the top of the stack to the process output when the `debug`
/// capability is granted.
fn print_to(stack: &mut Stack, stderr: bool) -> Result<()> {
    let top = stack.pop()?;
    if stack.capability("debug").is_some() {
        if stderr {
            eprintln!("{top}");
        } else {
            println!("{top}");
        }
    }
    Ok(())
}

fn stdout(stack: &mut Stack) -> Result<()> {
    print_to(stack, false)
}

fn stderr(stack: &mut Stack) -> Result<()> {
    print_to(stack, true)
}

/// `SLEEP`: pause for a number of time units, bounded by the `sleep.maxtime` capability (in
/// milliseconds).
fn sleep(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "SLEEP";
    const CAPABILITY: &str = "sleep.maxtime";
    let Some(max) = stack.capability(CAPABILITY) else {
        return Err(fail(NAME, &format!("requires capability {CAPABILITY}.")));
    };
    let max_ms: i64 = max.parse().map_err(|_| {
        fail(
            NAME,
            &format!("cannot parse capability {CAPABILITY}: '{max}' is not a valid LONG"),
        )
    })?;
    if max_ms <= 0 {
        return Err(fail(
            NAME,
            &format!(
                "requires capability {CAPABILITY} to be set to a value strictly greater than 0 ms."
            ),
        ));
    }
    let Value::Long(duration) = stack.pop()? else {
        return Err(fail(NAME, "expects a LONG period as parameter."));
    };
    let millis = duration / TIME_UNITS_PER_MS;
    if millis > max_ms {
        return Err(fail(
            NAME,
            &format!(
                "cannot sleep during more than {max_ms} ms, as defined in {CAPABILITY} capability."
            ),
        ));
    }
    if millis > 0 {
        std::thread::sleep(std::time::Duration::from_millis(millis as u64));
    }
    Ok(())
}
