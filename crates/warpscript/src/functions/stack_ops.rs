// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Stack manipulation functions: `DUP`, `SWAP`, `ROLL`, marks and the `[ ]` / `{ }` / `( )`
//! collection builders.

use indexmap::{IndexMap, IndexSet};

use super::Entry;
use super::args::{fail, getn};
use crate::error::{Error, Result};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("DUP", Entry::Stack(dup)),
    ("DUPN", Entry::Stack(dupn)),
    ("SWAP", Entry::Stack(swap)),
    ("DROP", Entry::Stack(drop)),
    ("DROPN", Entry::Stack(dropn)),
    ("CLEAR", Entry::Stack(clear)),
    ("ROT", Entry::Stack(rot)),
    ("ROLL", Entry::Stack(roll)),
    ("ROLLD", Entry::Stack(rolld)),
    ("PICK", Entry::Stack(pick)),
    ("DEPTH", Entry::Stack(depth)),
    ("RESET", Entry::Stack(reset)),
    ("MARK", Entry::Stack(mark)),
    ("CLEARTOMARK", Entry::Stack(cleartomark)),
    ("COUNTTOMARK", Entry::Stack(counttomark)),
    ("HIDE", Entry::Stack(hide)),
    ("SHOW", Entry::Stack(show)),
    ("SAVE", Entry::Stack(save)),
    ("RESTORE", Entry::Stack(restore)),
    ("NOOP", Entry::Stack(noop)),
    ("BOOTSTRAP", Entry::Stack(noop)),
    ("SAFECOMPILE", Entry::Stack(noop)),
    ("[", Entry::Stack(mark)),
    ("]", Entry::Stack(endlist)),
    ("{", Entry::Stack(mark)),
    ("}", Entry::Stack(endmap)),
    ("(", Entry::Stack(mark)),
    (")", Entry::Stack(endset)),
    ("[[", Entry::Stack(mark)),
    ("]]", Entry::Stack(endvector)),
    ("[]", Entry::Stack(emptylist)),
    ("{}", Entry::Stack(emptymap)),
    ("()", Entry::Stack(emptyset)),
    ("->LIST", Entry::Stack(tolist)),
    ("->MAP", Entry::Stack(tomap)),
    ("->SET", Entry::Stack(toset)),
    ("->V", Entry::Stack(tovector)),
    ("STACKTOLIST", Entry::Stack(stacktolist)),
];

fn dup(stack: &mut Stack) -> Result<()> {
    let top = stack.peek()?.clone();
    stack.push(top);
    Ok(())
}

fn dupn(stack: &mut Stack) -> Result<()> {
    let n = getn(stack)?;
    let start = stack.depth() - n;
    for i in 0..n {
        let value = stack.elements()[start + i].clone();
        stack.push(value);
    }
    Ok(())
}

fn swap(stack: &mut Stack) -> Result<()> {
    if stack.depth() == 0 {
        return Err(Error::empty_stack());
    }
    if stack.depth() < 2 {
        return Err(Error::new("Index out of bound."));
    }
    let elements = stack.elements_mut();
    let len = elements.len();
    elements.swap(len - 1, len - 2);
    Ok(())
}

pub(crate) fn drop(stack: &mut Stack) -> Result<()> {
    stack.pop().map(|_| ())
}

fn dropn(stack: &mut Stack) -> Result<()> {
    let n = getn(stack)?;
    stack.dropn(n)
}

fn clear(stack: &mut Stack) -> Result<()> {
    stack.clear();
    Ok(())
}

fn rot(stack: &mut Stack) -> Result<()> {
    if stack.depth() == 0 {
        return Err(Error::empty_stack());
    }
    if stack.depth() < 3 {
        return Err(Error::new("Index out of bound."));
    }
    let elements = stack.elements_mut();
    let len = elements.len();
    elements[len - 3..].rotate_left(1);
    Ok(())
}

fn roll(stack: &mut Stack) -> Result<()> {
    let n = getn(stack)?;
    if n > 1 {
        let elements = stack.elements_mut();
        let len = elements.len();
        elements[len - n..].rotate_left(1);
    }
    Ok(())
}

fn rolld(stack: &mut Stack) -> Result<()> {
    let n = getn(stack)?;
    if n > 1 {
        let elements = stack.elements_mut();
        let len = elements.len();
        elements[len - n..].rotate_right(1);
    }
    Ok(())
}

fn pick(stack: &mut Stack) -> Result<()> {
    let n = getn(stack)?;
    if n == 0 {
        return Err(Error::new("Index out of bound."));
    }
    let value = stack.elements()[stack.depth() - n].clone();
    stack.push(value);
    Ok(())
}

fn depth(stack: &mut Stack) -> Result<()> {
    let depth = stack.depth() as i64;
    stack.push(depth);
    Ok(())
}

fn reset(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::Long(l) => {
            if l < 0 {
                return Err(Error::new("Index out of bound."));
            }
            stack.reset(l as i32 as usize);
            Ok(())
        }
        _ => Err(fail("RESET", "expects a numeric (long) limit.")),
    }
}

fn mark(stack: &mut Stack) -> Result<()> {
    stack.push(Value::Mark);
    Ok(())
}

fn emptylist(stack: &mut Stack) -> Result<()> {
    stack.push(Value::list(Vec::new()));
    Ok(())
}

fn emptymap(stack: &mut Stack) -> Result<()> {
    stack.push(Value::map(IndexMap::new()));
    Ok(())
}

fn emptyset(stack: &mut Stack) -> Result<()> {
    stack.push(Value::set(IndexSet::new()));
    Ok(())
}

fn cleartomark(stack: &mut Stack) -> Result<()> {
    while !matches!(stack.pop()?, Value::Mark) {}
    Ok(())
}

/// Number of elements above the topmost mark (`COUNTTOMARK`), without touching the stack.
fn count_to_mark(stack: &Stack) -> Result<usize> {
    let mut count = 0;
    while !matches!(stack.get(count)?, Value::Mark) {
        count += 1;
    }
    Ok(count)
}

fn counttomark(stack: &mut Stack) -> Result<()> {
    let count = count_to_mark(stack)? as i64;
    stack.push(count);
    Ok(())
}

fn hide(stack: &mut Stack) -> Result<()> {
    let hidden = match stack.pop()? {
        Value::Null => stack.hide_all(),
        Value::Long(l) => stack.hide(i64::from(l as i32)),
        _ => return Err(fail("HIDE", "expects a number of levels to hide.")),
    };
    stack.push(hidden as i64);
    Ok(())
}

fn show(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::Null => stack.show_all(),
        Value::Long(l) => stack.show(i64::from(l as i32)),
        _ => return Err(fail("SHOW", "expects a number of levels to show.")),
    }
    Ok(())
}

fn save(stack: &mut Stack) -> Result<()> {
    let context = stack.save();
    stack.push(Value::Context(std::rc::Rc::new(context)));
    Ok(())
}

fn restore(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::Context(context) => {
            stack.restore(&context);
            Ok(())
        }
        _ => Err(Error::new("Invalid stack context.")),
    }
}

fn noop(_stack: &mut Stack) -> Result<()> {
    Ok(())
}

/// `ENDLIST`: pack everything above the mark into a list and remove the mark.
fn end_collection(stack: &mut Stack, collection: &str) -> Result<Vec<Value>> {
    let count = count_to_mark(stack).map_err(|err| {
        Error::with_cause(
            format!("Cannot find a matching start of {collection}."),
            err,
        )
    })?;
    // `count` elements sit above the mark, so neither call can fail.
    let items = stack.popn(count).unwrap_or_default();
    let _ = stack.pop();
    Ok(items)
}

fn endlist(stack: &mut Stack) -> Result<()> {
    let items = end_collection(stack, "list")?;
    stack.push(Value::list(items));
    Ok(())
}

fn endvector(stack: &mut Stack) -> Result<()> {
    let items = end_collection(stack, "vector")?;
    stack.push(Value::list(items));
    Ok(())
}

fn endset(stack: &mut Stack) -> Result<()> {
    let items = end_collection(stack, "set")?;
    stack.push(Value::set(items.into_iter().collect()));
    Ok(())
}

fn endmap(stack: &mut Stack) -> Result<()> {
    let items = end_collection(stack, "map")?;
    let map = build_map(items)?;
    stack.push(Value::map(map));
    Ok(())
}

/// `TOMAP`: alternate keys and values.
fn build_map(items: Vec<Value>) -> Result<IndexMap<Value, Value>> {
    if !items.len().is_multiple_of(2) {
        return Err(Error::new("Odd number of elements, cannot create map."));
    }
    let mut map = IndexMap::with_capacity(items.len() / 2);
    let mut iter = items.into_iter();
    while let (Some(key), Some(value)) = (iter.next(), iter.next()) {
        if map.contains_key(&key) {
            return Err(Error::new(format!("Duplicate map key '{key}'")));
        }
        map.insert(key, value);
    }
    Ok(map)
}

fn tolist(stack: &mut Stack) -> Result<()> {
    match stack.peek()? {
        Value::List(_) => Ok(()),
        Value::Set(set) => {
            let items: Vec<Value> = set.borrow().iter().cloned().collect();
            let _ = stack.pop();
            stack.push(Value::list(items));
            Ok(())
        }
        Value::Long(_) => {
            let n = getn(stack)?;
            let items = stack.popn(n).unwrap_or_default();
            stack.push(Value::list(items));
            Ok(())
        }
        _ => Err(fail(
            "->LIST",
            "expects a LIST, SET or a LONG number of elements to pack into a LIST.",
        )),
    }
}

fn tomap(stack: &mut Stack) -> Result<()> {
    let n = getn(stack)?;
    let items = stack.popn(n).unwrap_or_default();
    let map = build_map(items)?;
    stack.push(Value::map(map));
    Ok(())
}

fn toset(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::List(list) => {
            let set: IndexSet<Value> = list.borrow().iter().cloned().collect();
            stack.push(Value::set(set));
            Ok(())
        }
        set @ Value::Set(_) => {
            stack.push(set);
            Ok(())
        }
        _ => Err(fail("->SET", "operates on a LIST or SET.")),
    }
}

fn tovector(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::List(list) => {
            let items: Vec<Value> = list.borrow().clone();
            stack.push(Value::list(items));
            Ok(())
        }
        Value::Set(set) => {
            let items: Vec<Value> = set.borrow().iter().cloned().collect();
            stack.push(Value::list(items));
            Ok(())
        }
        _ => Err(fail("->V", "expects a collection on top of the stack.")),
    }
}

/// `STACKTOLIST`: replace the whole (visible) stack with a list of its elements, bottom first.
fn stacktolist(stack: &mut Stack) -> Result<()> {
    let items = stack.popn(stack.depth()).unwrap_or_default();
    stack.push(Value::list(items));
    Ok(())
}
