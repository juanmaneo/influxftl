// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Control flow: conditionals, loops, errors and the `TRY` / `RETURN` / `STOP` machinery.

use std::rc::Rc;
use std::time::Instant;

use indexmap::IndexMap;

use super::Entry;
use super::args::{fail, pop_bool, pop_long, pop_macro, pop_string};
use crate::error::{Error, Result, ScriptError};
use crate::macros::Macro;
use crate::stack::{DEADLINE_MESSAGE, Stack};
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("IFT", Entry::Stack(ift)),
    ("IFTE", Entry::Stack(ifte)),
    ("SWITCH", Entry::Stack(switch)),
    ("WHILE", Entry::Stack(r#while)),
    ("UNTIL", Entry::Stack(until)),
    ("FOR", Entry::Stack(r#for)),
    ("FORSTEP", Entry::Stack(forstep)),
    ("FOREACH", Entry::Stack(foreach)),
    ("RANGE", Entry::Stack(range)),
    ("BREAK", Entry::Stack(r#break)),
    ("CONTINUE", Entry::Stack(r#continue)),
    ("RETURN", Entry::Stack(r#return)),
    ("NRETURN", Entry::Stack(nreturn)),
    ("STOP", Entry::Stack(stop)),
    ("TRY", Entry::Stack(r#try)),
    ("RETHROW", Entry::Stack(rethrow)),
    ("ERROR", Entry::Stack(error)),
    ("FAIL", Entry::Stack(fail_fn)),
    ("COMPILE", Entry::Stack(not_supported_compile)),
    ("COMPILED", Entry::Stack(not_supported_compiled)),
    ("MSGFAIL", Entry::Stack(msgfail)),
    ("ASSERT", Entry::Stack(assert)),
    ("ASSERTMSG", Entry::Stack(assertmsg)),
    ("SECTION", Entry::Stack(section)),
    ("TIMEBOX", Entry::Stack(timebox)),
];

/// Default `TIMEBOX` cap (`warpscript.timebox.maxtime`, 30 s) in platform time units (µs).
pub const TIMEBOX_MAXTIME_US: i64 = 30_000 * 1_000;

/// Evaluate an `IF` condition which is either a boolean or a macro leaving a boolean.
fn condition(stack: &mut Stack, cond: Value, name: &str, message: &str) -> Result<bool> {
    match cond {
        Value::Boolean(b) => Ok(b),
        Value::Macro(m) => {
            stack.exec_macro(&m)?;
            pop_bool(
                stack,
                name,
                "expects its 'IF' macro to leave a boolean on top of the stack.",
            )
        }
        _ => Err(fail(name, message)),
    }
}

fn ift(stack: &mut Stack) -> Result<()> {
    let then = pop_macro(stack, "IFT", "expects a *THEN* macro on top of the stack.")?;
    let cond = stack.pop()?;
    if condition(
        stack,
        cond,
        "IFT",
        "expects an *IF* macro or a boolean below the *THEN* macro.",
    )? {
        stack.exec_macro(&then)?;
    }
    Ok(())
}

fn ifte(stack: &mut Stack) -> Result<()> {
    const MESSAGE: &str = "expects three macros or two macros and a boolean on top of the stack.";
    let otherwise = pop_macro(stack, "IFTE", MESSAGE)?;
    let then = pop_macro(stack, "IFTE", MESSAGE)?;
    let cond = stack.pop()?;
    if condition(stack, cond, "IFTE", MESSAGE)? {
        stack.exec_macro(&then)
    } else {
        stack.exec_macro(&otherwise)
    }
}

fn switch(stack: &mut Stack) -> Result<()> {
    let ncases = pop_long(
        stack,
        "SWITCH",
        "expects a number of cases on top of the stack.",
    )?;
    let default = pop_macro(
        stack,
        "SWITCH",
        "expects a default macro after the number of cases.",
    )?;
    if ncases < 0 {
        return Err(fail(
            "SWITCH",
            "expects a number of cases on top of the stack.",
        ));
    }
    let mut cases: Vec<(Rc<Macro>, Rc<Macro>)> = Vec::with_capacity(ncases as usize);
    for _ in 0..ncases {
        let exec = stack.pop()?;
        let cond = stack.pop()?;
        match (cond, exec) {
            (Value::Macro(cond), Value::Macro(exec)) => cases.push((cond, exec)),
            _ => {
                return Err(fail(
                    "SWITCH",
                    "expects each case to be a set of two macros, a conditional macro with an 'exec' macro on top of it.",
                ));
            }
        }
    }
    let mut exec = default;
    for (cond, case_exec) in cases.iter().rev() {
        stack.exec_macro(cond)?;
        if matches!(stack.pop()?, Value::Boolean(true)) {
            exec = Rc::clone(case_exec);
            break;
        }
    }
    stack.exec_macro(&exec)
}

/// Outcome of one loop iteration.
enum Flow {
    Next,
    Break,
}

/// Run a loop body, translating `BREAK` / `CONTINUE` signals.
fn loop_body(stack: &mut Stack, body: &Rc<Macro>) -> Result<Flow> {
    match stack.exec_macro(body) {
        Ok(()) | Err(Error::Continue) => Ok(Flow::Next),
        Err(Error::Break) => Ok(Flow::Break),
        Err(err) => Err(err),
    }
}

/// Enforces the maximum loop duration (`ATTRIBUTE_LOOP_MAXDURATION`).
struct LoopTimer {
    start: Instant,
    max_ms: i64,
}

impl LoopTimer {
    fn new(stack: &Stack) -> Self {
        LoopTimer {
            start: Instant::now(),
            max_ms: stack.limits.max_loop_duration,
        }
    }

    fn check(&self, name: &str) -> Result<()> {
        if self.max_ms > 0 && self.start.elapsed().as_millis() as i64 > self.max_ms {
            return Err(Error::new(format!(
                "{name} executed for too long (> {} ms). Consider raising the limit or using capabilities.",
                self.max_ms
            )));
        }
        Ok(())
    }
}

/// Pop the optional trailing boolean of a loop (`pushCounter` / `pushIndex`).
fn pop_flag(stack: &mut Stack, default: bool) -> Result<(bool, Value)> {
    let top = stack.pop()?;
    if let Value::Boolean(flag) = top {
        let next = stack.pop()?;
        Ok((flag, next))
    } else {
        Ok((default, top))
    }
}

fn r#while(stack: &mut Stack) -> Result<()> {
    let (push_counter, run) = pop_flag(stack, false)?;
    let cond = stack.pop()?;
    let (Value::Macro(run), Value::Macro(cond)) = (run, cond) else {
        return Err(fail("WHILE", "expects two macros on top of the stack."));
    };
    let timer = LoopTimer::new(stack);
    let mut counter = 0i64;
    loop {
        timer.check("WHILE")?;
        stack.exec_macro(&cond)?;
        if !pop_bool(
            stack,
            "WHILE",
            "expects its 'WHILE' macro to leave a boolean on top of the stack.",
        )? {
            break;
        }
        if push_counter {
            stack.push(counter);
            counter += 1;
        }
        if let Flow::Break = loop_body(stack, &run)? {
            break;
        }
    }
    Ok(())
}

fn until(stack: &mut Stack) -> Result<()> {
    let (push_counter, cond) = pop_flag(stack, false)?;
    let run = stack.pop()?;
    let (Value::Macro(run), Value::Macro(cond)) = (run, cond) else {
        return Err(fail("UNTIL", "expects two macros on top of the stack."));
    };
    let timer = LoopTimer::new(stack);
    let mut counter = 0i64;
    loop {
        timer.check("UNTIL")?;
        if push_counter {
            stack.push(counter);
            counter += 1;
        }
        if let Flow::Break = loop_body(stack, &run)? {
            break;
        }
        stack.exec_macro(&cond)?;
        if pop_bool(
            stack,
            "UNTIL",
            "expects its 'UNTIL' macro to leave a boolean on top of the stack.",
        )? {
            break;
        }
    }
    Ok(())
}

/// The counter and limit of a `FOR` / `FORSTEP` loop: `LONG`s unless one of the bounds is a
/// `DOUBLE`, in which case both are `DOUBLE`s.
enum Range {
    Long { counter: i64, limit: i64 },
    Double { counter: f64, limit: f64 },
}

impl Range {
    /// Parse the `FROM` / `TO` bounds of a loop.
    fn new(from: &Value, to: &Value, name: &str) -> Result<Range> {
        match (from, to) {
            (Value::Long(f), Value::Long(t)) => Ok(Range::Long {
                counter: *f,
                limit: *t,
            }),
            (Value::Long(_) | Value::Double(_), Value::Long(_) | Value::Double(_)) => {
                Ok(Range::Double {
                    counter: from.as_f64().unwrap_or(f64::NAN),
                    limit: to.as_f64().unwrap_or(f64::NAN),
                })
            }
            _ => Err(fail(name, "expects numbers as its range.")),
        }
    }

    fn value(&self) -> Value {
        match self {
            Range::Long { counter, .. } => Value::Long(*counter),
            Range::Double { counter, .. } => Value::Double(*counter),
        }
    }

    /// `true` when the loop starts below (or at) its limit.
    fn ascending(&self) -> bool {
        match self {
            Range::Long { counter, limit } => counter <= limit,
            Range::Double { counter, limit } => counter <= limit,
        }
    }

    /// `true` when the counter went past the limit in the loop direction.
    fn done(&self, ascending: bool) -> bool {
        match self {
            Range::Long { counter, limit } => {
                (ascending && counter > limit) || (!ascending && counter < limit)
            }
            Range::Double { counter, limit } => {
                (ascending && counter > limit) || (!ascending && counter < limit)
            }
        }
    }

    fn increment(&mut self) {
        match self {
            Range::Long { counter, .. } => *counter = counter.wrapping_add(1),
            Range::Double { counter, .. } => *counter += 1.0,
        }
    }

    /// Set the counter from the value left by a `FORSTEP` step macro.
    fn set(&mut self, next: &Value) -> Result<()> {
        let converted = match self {
            Range::Long { counter, .. } => next.as_i64().map(|l| *counter = l),
            Range::Double { counter, .. } => next.as_f64().map(|d| *counter = d),
        };
        converted.ok_or_else(|| {
            fail(
                "FORSTEP",
                "expects its 'STEP' macro to leave a number on top of the stack.",
            )
        })
    }
}

fn r#for(stack: &mut Stack) -> Result<()> {
    let (push_counter, run) = pop_flag(stack, true)?;
    let to = stack.pop()?;
    let from = stack.pop()?;
    let Value::Macro(run) = run else {
        return Err(fail("FOR", "expects a macro on top of the stack."));
    };
    let mut range = Range::new(&from, &to, "FOR")?;
    let timer = LoopTimer::new(stack);
    loop {
        timer.check("FOR")?;
        if range.done(true) {
            break;
        }
        if push_counter {
            stack.push(range.value());
        }
        if let Flow::Break = loop_body(stack, &run)? {
            break;
        }
        range.increment();
    }
    Ok(())
}

fn forstep(stack: &mut Stack) -> Result<()> {
    let (push_counter, run) = pop_flag(stack, true)?;
    let step = stack.pop()?;
    let to = stack.pop()?;
    let from = stack.pop()?;
    let (Value::Macro(run), Value::Macro(step)) = (run, step) else {
        return Err(fail("FORSTEP", "expects two macros on top of the stack."));
    };
    let mut range = Range::new(&from, &to, "FORSTEP")?;
    let ascending = range.ascending();
    let timer = LoopTimer::new(stack);
    loop {
        timer.check("FORSTEP")?;
        if range.done(ascending) {
            break;
        }
        if push_counter {
            stack.push(range.value());
        }
        if let Flow::Break = loop_body(stack, &run)? {
            break;
        }
        stack.push(range.value());
        stack.exec_macro(&step)?;
        let next = stack.pop()?;
        range.set(&next)?;
    }
    Ok(())
}

/// The elements a `FOREACH` iterates over, snapshotted so the body may mutate the collection.
fn foreach_items(obj: &Value) -> Option<Vec<Vec<Value>>> {
    match obj {
        Value::List(list) => Some(list.borrow().iter().map(|v| vec![v.clone()]).collect()),
        Value::Set(set) => Some(set.borrow().iter().map(|v| vec![v.clone()]).collect()),
        Value::Map(map) => Some(
            map.borrow()
                .iter()
                .map(|(k, v)| vec![k.clone(), v.clone()])
                .collect(),
        ),
        Value::String(s) => Some(
            s.chars()
                .map(|c| vec![Value::from(c.to_string())])
                .collect(),
        ),
        _ => None,
    }
}

fn foreach(stack: &mut Stack) -> Result<()> {
    let (push_index, run) = pop_flag(stack, false)?;
    let obj = stack.pop()?;
    let Value::Macro(run) = run else {
        return Err(fail("FOREACH", "expects a macro on top of the stack."));
    };
    let Some(items) = foreach_items(&obj) else {
        return Err(fail(
            "FOREACH",
            "operates on a list, map, Geo Time Series, ENCODER, STRING, iterator or iterable.",
        ));
    };
    for (index, item) in items.into_iter().enumerate() {
        for value in item {
            stack.push(value);
        }
        if push_index {
            stack.push(index as i64);
        }
        if let Flow::Break = loop_body(stack, &run)? {
            break;
        }
    }
    Ok(())
}

fn range(stack: &mut Stack) -> Result<()> {
    let Value::List(params) = stack.pop()? else {
        return Err(fail(
            "RANGE",
            "expects a list of parameters on top of the stack.",
        ));
    };
    let params: Vec<Option<i64>> = params.borrow().iter().map(Value::as_i64).collect();
    let bounds: Option<Vec<i64>> = params.into_iter().collect();
    let (start, stop, step) = match bounds.as_deref() {
        Some([stop]) => (0, *stop, 1),
        Some([start, stop]) => (*start, *stop, 1),
        Some([start, stop, step]) => (*start, *stop, *step),
        _ => {
            return Err(fail(
                "RANGE",
                "expects a list of 1 to 3 longs on top of the stack.",
            ));
        }
    };
    if step <= 0 {
        return Err(fail("RANGE", "step MUST be > 0."));
    }
    let mut items = Vec::new();
    let mut current = start;
    while current < stop {
        items.push(Value::Long(current));
        current = current.saturating_add(step);
    }
    stack.push(Value::list(items));
    Ok(())
}

fn r#break(_stack: &mut Stack) -> Result<()> {
    Err(Error::Break)
}

fn r#continue(_stack: &mut Stack) -> Result<()> {
    Err(Error::Continue)
}

fn r#return(stack: &mut Stack) -> Result<()> {
    stack.set_return_depth(1);
    Err(Error::Return)
}

fn nreturn(stack: &mut Stack) -> Result<()> {
    let levels = pop_long(stack, "NRETURN", "expects a number of levels.")?;
    stack.set_return_depth(levels);
    Err(Error::Return)
}

fn stop(_stack: &mut Stack) -> Result<()> {
    Err(Error::Stop("STOP".to_owned()))
}

pub(crate) fn r#try(stack: &mut Stack) -> Result<()> {
    let finally = pop_macro(
        stack,
        "TRY",
        "expects a 'finally' macro on top of the stack.",
    )?;
    let catch = pop_macro(
        stack,
        "TRY",
        "expects a 'catch' macro below the 'finally' macro.",
    )?;
    let body = pop_macro(
        stack,
        "TRY",
        "expects a 'try' macro below the 'catch' macro.",
    )?;
    let result = match stack.exec_macro(&body) {
        Ok(()) => Ok(()),
        Err(err) => {
            stack.set_last_error(Some(err));
            stack.exec_macro(&catch)
        }
    };
    if !finally.is_empty() {
        stack.exec_macro(&finally)?;
    }
    result
}

pub(crate) fn rethrow(stack: &mut Stack) -> Result<()> {
    match stack.last_error() {
        Some(err) => Err(err.clone()),
        None => Ok(()),
    }
}

/// Java class name reported by `ERROR` for each kind of error.
fn error_type(err: &Error) -> String {
    format!("class {}", err.java_class())
}

/// `ERROR`: the last caught error and its causes as a list of maps.
fn error(stack: &mut Stack) -> Result<()> {
    let mut errors = Vec::new();
    let mut current = stack.last_error();
    while let Some(err) = current {
        let mut map = IndexMap::new();
        map.insert(Value::from("type"), Value::from(error_type(err)));
        map.insert(Value::from("message"), Value::from(err.message()));
        map.insert(Value::from("stacktrace"), Value::list(Vec::new()));
        errors.push(Value::map(map));
        current = match err {
            Error::Script(ScriptError {
                cause: Some(cause), ..
            }) => Some(cause),
            _ => None,
        };
    }
    stack.push(Value::list(errors));
    Ok(())
}

fn fail_fn(_stack: &mut Stack) -> Result<()> {
    Err(Error::new("FAIL failed."))
}

fn not_supported_compile(_stack: &mut Stack) -> Result<()> {
    Err(Error::new("COMPILE Not supported."))
}

fn not_supported_compiled(_stack: &mut Stack) -> Result<()> {
    Err(Error::new("COMPILED Not supported."))
}

fn msgfail(stack: &mut Stack) -> Result<()> {
    let top = stack.pop()?;
    Err(Error::new(format!("MSGFAIL {top}")))
}

fn assert(stack: &mut Stack) -> Result<()> {
    if pop_bool(stack, "ASSERT", "expects a boolean on top of the stack.")? {
        Ok(())
    } else {
        Err(Error::new("ASSERT failed."))
    }
}

fn assertmsg(stack: &mut Stack) -> Result<()> {
    let message = pop_string(stack, "ASSERTMSG", "expects a STRING message.")?;
    if pop_bool(stack, "ASSERTMSG", "expects a boolean below the message.")? {
        Ok(())
    } else {
        Err(Error::new(format!("ASSERTMSG failed '{message}'")))
    }
}

fn section(stack: &mut Stack) -> Result<()> {
    let name = pop_string(stack, "SECTION", "expected a section name as a string.")?;
    stack.set_section_name(&name);
    Ok(())
}

/// `TIMEBOX`: run a macro with a deadline. The macro runs on the calling thread; the deadline is
/// checked by the interpreter between statements and the execution is aborted when it passes.
fn timebox(stack: &mut Stack) -> Result<()> {
    let requested = pop_long(
        stack,
        "TIMEBOX",
        "expects a maximum execution time on top of the stack.",
    )?
    .max(0);
    let body = pop_macro(stack, "TIMEBOX", "operates on a macro.")?;
    let maxtime = if requested > 0 {
        requested.min(TIMEBOX_MAXTIME_US)
    } else {
        TIMEBOX_MAXTIME_US
    };
    let start = Instant::now();
    let deadline = start + std::time::Duration::from_micros(maxtime as u64);
    let previous = stack.set_deadline(Some(match stack.deadline() {
        Some(outer) if outer < deadline => outer,
        _ => deadline,
    }));
    let result = stack.exec_macro(&body);
    stack.set_deadline(previous);
    let expired = Instant::now() > deadline;
    match result {
        Ok(()) if !expired => Ok(()),
        Err(Error::Stop(_) | Error::Return) => Ok(()),
        Err(err) if err.is_signal() => Err(err),
        Err(err) if !expired && err.root_cause().message() != DEADLINE_MESSAGE => {
            Err(Error::with_cause(
                "TIMEBOX encountered an exception while executing macro",
                err,
            ))
        }
        _ => Err(Error::new(format!(
            "TIMEBOX reached the execution time limit ({maxtime} MICROSECONDS)."
        ))),
    }
}
