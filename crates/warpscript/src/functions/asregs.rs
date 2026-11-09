// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `ASREGS`: rewrite a macro so that its variables live in registers.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::rc::Rc;

use super::Entry;
use super::args::{fail, fail_with_cause};
use super::runtime::macro_vars;
use crate::error::fault::BoundsError;
use crate::error::{Error, Result};
use crate::macros::{Callable, Macro, Statement};
use crate::stack::Stack;
use crate::value::{List, Value};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[("ASREGS", Entry::Stack(asregs))];

const NAME: &str = "ASREGS";

/// The error raised when an access cannot be resolved statically.
fn unable() -> Error {
    fail(NAME, "was unable to convert variables to registers.")
}

/// `ASREGS`: pop an optional list of variable names and a macro, and push a copy of the macro
/// where `x LOAD`, `x STORE` and `x CSTORE` on those variables (by default, the variables the
/// macro writes) use registers (`PUSHRn`, `POPRn`, `CPOPRn`). Accesses to register numbers
/// (`n LOAD`, `n STORE`, `n CSTORE`, `n RUN`) become register functions as well, and
/// `[ … ] STORE` on registers only becomes a sequence of `POPRn`.
fn asregs(stack: &mut Stack) -> Result<()> {
    let mut top = stack.pop()?;
    let vars = if let Value::List(list) = top {
        let vars = list.borrow().clone();
        top = stack.pop()?;
        Some(vars)
    } else {
        None
    };
    let Value::Macro(m) = top else {
        return Err(fail(
            NAME,
            "expects an optional list of variable names and a macro.",
        ));
    };
    let vars = match vars {
        Some(vars) => vars,
        // Only the variables the macro writes, so that global variables are left alone.
        None => macro_vars(&m, true).map_err(|err| fail_with_cause(NAME, "failed.", err))?,
    };
    let (arena, mut inuse) = parse(&m)?;
    let varregs = assign(&vars, &mut inuse, stack.registers().len())?;
    let rewritten = renamed(emit(&arena, 0, &varregs), &m);
    stack.push(Value::Macro(Rc::new(rewritten)));
    Ok(())
}

/// A macro made of `statements` carrying the name of `original`.
fn renamed(statements: Vec<Statement>, original: &Macro) -> Macro {
    let m = Macro::from_statements(statements);
    *m.name.borrow_mut() = original.name();
    m
}

/// How a statement accesses a symbol.
#[derive(Clone, Copy)]
enum Access {
    Load,
    Store,
    CStore,
    Run,
}

impl Access {
    /// Prefix of the register function performing this access (`PUSHRn`, `POPRn`…).
    fn register_prefix(self) -> &'static str {
        match self {
            Access::Load => "PUSHR",
            Access::Store => "POPR",
            Access::CStore => "CPOPR",
            Access::Run => "RUNR",
        }
    }
}

/// The symbol an access applies to: the `LONG` or `STRING` pushed right before it.
enum Operand<'a> {
    Register(i64),
    Name(&'a str),
}

impl Operand<'_> {
    /// The register this operand designates, once variables are assigned.
    fn register(&self, varregs: &HashMap<&str, i64>) -> Option<i64> {
        match self {
            Operand::Register(register) => Some(*register),
            Operand::Name(name) => varregs.get(name).copied(),
        }
    }
}

/// What an element of a `[ … ] STORE` list literal designates.
#[derive(Clone, Copy)]
enum Kind<'a> {
    Register(i64),
    Name(&'a str),
    /// `NULL`: the corresponding value is not stored anywhere.
    Null,
}

/// An element of a `[ … ] STORE` list literal and the statement pushing it.
struct Element<'a> {
    kind: Kind<'a>,
    statement: &'a Statement,
}

/// A statement of the macro once the accesses to symbols are resolved.
enum Op<'a> {
    /// A statement kept as is.
    Keep(&'a Statement),
    /// A nested macro and the slot of its own operations in the arena.
    Nested(&'a Macro, usize),
    /// `symbol LOAD`, `symbol STORE`, `symbol CSTORE` or `symbol RUN`.
    Access {
        access: Access,
        operand: Operand<'a>,
        symbol: &'a Statement,
        call: &'a Statement,
    },
    /// `list STORE` where the list is a value (a macro built with `->MACRO`).
    StoreList {
        list: &'a List,
        store: &'a Statement,
    },
    /// `[ … ] STORE` where the list is built by the macro.
    StoreLiteral {
        mark: &'a Statement,
        elements: Vec<Element<'a>>,
        end: &'a Statement,
        store: &'a Statement,
    },
}

/// Java's `Long.intValue()`: register numbers are 32-bit.
fn register_number(value: i64) -> i64 {
    i64::from(value as i32)
}

/// `BitSet.set` on the registers in use, which rejects negative indexes.
fn mark_used(inuse: &mut BTreeSet<i64>, register: i64) -> Result<()> {
    if register < 0 {
        return Err(BoundsError::NegativeBitIndex(register).into());
    }
    inuse.insert(register);
    Ok(())
}

/// The register a `PUSHRn`, `POPRn`, `CPOPRn` or `RUNRn` function uses.
fn register_function(name: &str) -> Option<i64> {
    ["PUSHR", "POPR", "CPOPR", "RUNR"]
        .into_iter()
        .find_map(|prefix| name.strip_prefix(prefix))
        .and_then(|digits| digits.parse().ok())
}

/// The operand of an access: the statement preceding it must push a `LONG` or a `STRING`.
fn operand(previous: Option<&Statement>) -> Option<Operand<'_>> {
    match previous {
        Some(Statement::Push(Value::Long(register))) => {
            Some(Operand::Register(register_number(*register)))
        }
        Some(Statement::Push(Value::String(name))) => Some(Operand::Name(name.as_ref())),
        _ => None,
    }
}

/// The list literal closed by a `]`: its opening mark and its elements, which must be
/// register numbers, variable names or `NULL` for the registers in use to be known.
fn list_literal(before: &[Statement]) -> Result<(&Statement, Vec<Element<'_>>)> {
    let mut elements = Vec::new();
    for statement in before.iter().rev() {
        let kind = match statement {
            Statement::Call(Callable::Native {
                name: "[" | "MARK" | "{" | "(" | "[[",
                ..
            }) => {
                elements.reverse();
                return Ok((statement, elements));
            }
            Statement::Push(Value::Long(register)) => Kind::Register(register_number(*register)),
            Statement::Push(Value::String(name)) => Kind::Name(name.as_ref()),
            Statement::Push(Value::Null)
            | Statement::Call(Callable::Native { name: "NULL", .. }) => Kind::Null,
            _ => return Err(unable()),
        };
        elements.push(Element { kind, statement });
    }
    Err(unable())
}

/// Phase 1: resolve the accesses of the macro and of its nested macros (breadth first, like
/// Warp 10) and collect the registers already in use. Fails on secure macros and when an
/// access cannot be resolved statically.
fn parse(root: &Macro) -> Result<(Vec<Vec<Op<'_>>>, BTreeSet<i64>)> {
    let mut arena = vec![Vec::new()];
    let mut queue = VecDeque::from([(root, 0)]);
    let mut inuse = BTreeSet::new();
    while let Some((m, slot)) = queue.pop_front() {
        if m.is_secure() {
            return Err(fail(NAME, "cannot operate on a secure Macro."));
        }
        arena[slot] = parse_macro(m, &mut inuse, &mut arena, &mut queue)?;
    }
    Ok((arena, inuse))
}

/// Resolve the accesses of one macro; nested macros get a slot in `arena` and are queued.
fn parse_macro<'a>(
    m: &'a Macro,
    inuse: &mut BTreeSet<i64>,
    arena: &mut Vec<Vec<Op<'a>>>,
    queue: &mut VecDeque<(&'a Macro, usize)>,
) -> Result<Vec<Op<'a>>> {
    let statements = &m.statements;
    let mut ops = Vec::with_capacity(statements.len());
    for (i, statement) in statements.iter().enumerate() {
        let access = match statement {
            Statement::Push(Value::Macro(inner)) => {
                arena.push(Vec::new());
                queue.push_back((inner, arena.len() - 1));
                ops.push(Op::Nested(inner, arena.len() - 1));
                continue;
            }
            Statement::Call(Callable::Native { name: "LOAD", .. }) => Access::Load,
            Statement::Call(Callable::Native { name: "STORE", .. }) => Access::Store,
            Statement::Call(Callable::Native { name: "CSTORE", .. }) => Access::CStore,
            Statement::Call(Callable::Native { name: "RUN", .. }) => Access::Run,
            // The symbols these functions write are not explicit.
            Statement::Call(Callable::Native {
                name: "LSTORE" | "MSTORE",
                ..
            }) => return Err(unable()),
            Statement::Call(Callable::Native { name, .. }) => {
                if let Some(register) = register_function(name) {
                    inuse.insert(register);
                }
                ops.push(Op::Keep(statement));
                continue;
            }
            Statement::Push(_) | Statement::Call(_) => {
                ops.push(Op::Keep(statement));
                continue;
            }
        };
        let previous = i.checked_sub(1).map(|p| &statements[p]);
        match (access, previous) {
            (Access::Store, Some(Statement::Push(Value::List(list)))) => {
                for element in list.borrow().iter() {
                    if let Value::Long(register) = element {
                        mark_used(inuse, register_number(*register))?;
                    }
                }
                ops.pop();
                ops.push(Op::StoreList {
                    list,
                    store: statement,
                });
            }
            (Access::Store, Some(end @ Statement::Call(Callable::Native { name: "]", .. }))) => {
                let (mark, elements) = list_literal(&statements[..i - 1])?;
                for element in &elements {
                    if let Kind::Register(register) = element.kind {
                        mark_used(inuse, register)?;
                    }
                }
                // The mark, the elements and the `]` were kept so far: replace them.
                ops.truncate(ops.len() - elements.len() - 2);
                ops.push(Op::StoreLiteral {
                    mark,
                    elements,
                    end,
                    store: statement,
                });
            }
            (access, previous) => {
                let operand = operand(previous).ok_or_else(unable)?;
                match operand {
                    Operand::Register(register) => mark_used(inuse, register)?,
                    // `name RUN` calls a macro which may use any symbol.
                    Operand::Name(_) if matches!(access, Access::Run) => return Err(unable()),
                    Operand::Name(_) => {}
                }
                ops.pop();
                ops.push(Op::Access {
                    access,
                    operand,
                    symbol: &statements[i - 1],
                    call: statement,
                });
            }
        }
    }
    Ok(ops)
}

/// Phase 2: assign the lowest free register to each variable name (`LONG`s, i.e. registers,
/// are skipped) until every register is in use.
fn assign<'a>(
    vars: &'a [Value],
    inuse: &mut BTreeSet<i64>,
    nregs: usize,
) -> Result<HashMap<&'a str, i64>> {
    let mut varregs = HashMap::new();
    for var in vars {
        let name = match var {
            Value::Long(_) => continue,
            _ if inuse.len() == nregs => break,
            Value::String(name) => name.as_ref(),
            _ => {
                return Err(fail(
                    NAME,
                    "expects a list of variable names on top of the stack.",
                ));
            }
        };
        varregs.entry(name).or_insert_with(|| {
            let mut register = 0;
            while inuse.contains(&register) {
                register += 1;
            }
            inuse.insert(register);
            register
        });
    }
    Ok(varregs)
}

/// What an element of a list literal stores into once registers are assigned.
enum Target {
    Register(i64),
    /// `NULL`: nothing.
    Nothing,
    /// A variable without a register.
    Variable,
}

fn target(kind: Kind<'_>, varregs: &HashMap<&str, i64>) -> Target {
    match kind {
        Kind::Register(register) => Target::Register(register),
        Kind::Null => Target::Nothing,
        Kind::Name(name) => varregs
            .get(name)
            .copied()
            .map_or(Target::Variable, Target::Register),
    }
}

/// Phase 3: the statements of the macro in `slot` with the resolved accesses rewritten.
fn emit(arena: &[Vec<Op<'_>>], slot: usize, varregs: &HashMap<&str, i64>) -> Vec<Statement> {
    let mut out = Vec::new();
    for op in &arena[slot] {
        match op {
            Op::Keep(statement) => out.push((*statement).clone()),
            Op::Nested(inner, inner_slot) => {
                let rewritten = renamed(emit(arena, *inner_slot, varregs), inner);
                out.push(Statement::Push(Value::Macro(Rc::new(rewritten))));
            }
            Op::Access {
                access,
                operand,
                symbol,
                call,
            } => match operand.register(varregs) {
                Some(register) => {
                    emit_register(&mut out, access.register_prefix(), register, call);
                }
                None => {
                    out.push((*symbol).clone());
                    out.push((*call).clone());
                }
            },
            Op::StoreList { list, store } => {
                let mapped = list
                    .borrow()
                    .iter()
                    .map(|value| match value {
                        Value::String(name) => varregs
                            .get(name.as_ref())
                            .map_or_else(|| value.clone(), |register| Value::Long(*register)),
                        other => other.clone(),
                    })
                    .collect();
                out.push(Statement::Push(Value::list(mapped)));
                out.push((*store).clone());
            }
            Op::StoreLiteral {
                mark,
                elements,
                end,
                store,
            } => {
                let targets: Vec<Target> = elements
                    .iter()
                    .map(|element| target(element.kind, varregs))
                    .collect();
                if targets.iter().any(|t| matches!(t, Target::Variable)) {
                    // Some variable keeps its name: the list is rebuilt with the assigned
                    // names replaced by their register.
                    out.push((*mark).clone());
                    for (element, target) in elements.iter().zip(&targets) {
                        out.push(match (element.kind, target) {
                            (Kind::Name(_), Target::Register(register)) => {
                                Statement::Push(Value::Long(*register))
                            }
                            _ => element.statement.clone(),
                        });
                    }
                    out.push((*end).clone());
                    out.push((*store).clone());
                } else {
                    // `[ 3 7 NULL 9 ] STORE` becomes `POPR9 DROP POPR7 POPR3`: the top of the
                    // stack goes to the last register; `NULL`s and registers stored again
                    // later in the list are dropped.
                    let mut seen = BTreeSet::new();
                    for target in targets.iter().rev() {
                        match target {
                            Target::Register(register) if seen.insert(*register) => {
                                emit_register(&mut out, "POPR", *register, store);
                            }
                            _ => out.push(Statement::Call(Callable::Native {
                                name: "DROP",
                                func: super::stack_ops::drop,
                            })),
                        }
                    }
                }
            }
        }
    }
    out
}

/// Emit `<prefix><register>` or, when no such function exists because the register number is
/// out of range, the original `register call` pair (which fails the same way at run time).
fn emit_register(out: &mut Vec<Statement>, prefix: &str, register: i64, call: &Statement) {
    match super::lookup(&format!("{prefix}{register}")) {
        Some((name, Entry::Stack(func))) => {
            out.push(Statement::Call(Callable::Native { name, func: *func }));
        }
        _ => {
            out.push(Statement::Push(Value::Long(register)));
            out.push(call.clone());
        }
    }
}
