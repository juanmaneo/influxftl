// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Macros: compiled sequences of statements (`<% … %>`).

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use crate::error::{Error, Result};
use crate::snapshot;
use crate::stack::Stack;
use crate::value::{FunctionObject, Value};

/// Signature of a native stack function: it consumes and produces values on the stack.
pub type StackFn = fn(&mut Stack) -> Result<()>;

/// Signature of a native stack function that is told the name it was registered under (the
/// builders of parameterized function objects, `bucketizer.var`, `mapper.percentile`…).
pub type BuilderFn = fn(&mut Stack, &'static str) -> Result<()>;

/// Something that can be invoked as a stack function.
#[derive(Clone, Debug)]
pub enum Callable {
    /// A WarpLib function implemented in Rust.
    Native {
        /// WarpScript name, e.g. `BUCKETIZE`.
        name: &'static str,
        /// Implementation.
        func: StackFn,
    },
    /// A WarpLib builder implemented in Rust, invoked with its registered name.
    Builder {
        /// WarpScript name, e.g. `bucketizer.var`.
        name: &'static str,
        /// Implementation.
        func: BuilderFn,
    },
    /// A function (re)defined with `DEF`: the macro is executed when the name is invoked.
    Defined {
        /// The (re)defined name.
        name: Rc<str>,
        /// The macro bound to the name at compile time.
        body: Rc<Macro>,
    },
    /// A function undefined with `UNDEF` (or `CLEARDEFS`): calling it fails with
    /// `<name> is undefined.`, it does **not** fall back to the WarpLib function.
    Undefined {
        /// The undefined name.
        name: Rc<str>,
    },
}

impl Callable {
    /// The WarpScript name of this callable.
    pub fn name(&self) -> &str {
        match self {
            Callable::Native { name, .. } | Callable::Builder { name, .. } => name,
            Callable::Defined { name, .. } | Callable::Undefined { name } => name,
        }
    }

    /// Invoke the callable on `stack`.
    pub fn apply(&self, stack: &mut Stack) -> Result<()> {
        match self {
            Callable::Native { func, .. } => func(stack),
            Callable::Builder { name, func } => func(stack, name),
            Callable::Defined { body, .. } => stack.exec_macro(body),
            Callable::Undefined { name } => Err(Error::new(format!("{name} is undefined."))),
        }
    }
}

/// A stack function as a value (`FUNCREF`, `MACRO->`): a `WarpScriptStackFunction` object
/// on the stack, which `EVAL` applies and `->MACRO` turns back into a call.
#[derive(Clone, Debug)]
pub struct StackFunction(pub Callable);

impl FunctionObject for StackFunction {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn type_name(&self) -> &'static str {
        "FUNCTION"
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }
}

impl StackFunction {
    /// The function object wrapping `callable`.
    pub fn value(callable: Callable) -> Value {
        Value::Function(Rc::new(StackFunction(callable)))
    }

    /// The callable behind `value`, when it is a stack function object.
    pub fn of(value: &Value) -> Option<&Callable> {
        match value {
            Value::Function(f) => f
                .as_any()
                .and_then(|any| any.downcast_ref::<StackFunction>())
                .map(|f| &f.0),
            _ => None,
        }
    }
}

/// One statement of a macro: either push a value or call a function.
#[derive(Clone, Debug)]
pub enum Statement {
    /// Push a literal, a function object, a nested macro…
    Push(Value),
    /// Invoke a stack function.
    Call(Callable),
}

impl Statement {
    /// Short description used in error messages (`Exception at '…'`).
    pub fn describe(&self) -> String {
        match self {
            Statement::Push(value) => snapshot::value_snapshot(value).trim_end().to_owned(),
            Statement::Call(callable) => callable.name().to_owned(),
        }
    }
}

/// A macro: an executable sequence of statements.
pub struct Macro {
    /// The statements, in execution order.
    pub statements: Vec<Statement>,
    /// A secure macro's content cannot be displayed (`SECURE`).
    pub secure: Cell<bool>,
    /// The name of the macro, when it was loaded by name (`@name`, `RUN`, `DEF`…).
    pub name: RefCell<Option<String>>,
}

impl Macro {
    /// An empty macro.
    pub fn new() -> Self {
        Macro::from_statements(Vec::new())
    }

    /// A macro made of `statements`.
    pub fn from_statements(statements: Vec<Statement>) -> Self {
        Macro {
            statements,
            secure: Cell::new(false),
            name: RefCell::new(None),
        }
    }

    /// Number of statements.
    pub fn len(&self) -> usize {
        self.statements.len()
    }

    /// `true` when the macro has no statement.
    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }

    /// `true` when the macro is secure.
    pub fn is_secure(&self) -> bool {
        self.secure.get()
    }

    /// The name of the macro, if any.
    pub fn name(&self) -> Option<String> {
        self.name.borrow().clone()
    }

    /// Set the name of this macro and, recursively, of every unnamed nested macro
    /// (`Macro.setNameRecursive`).
    pub fn set_name_recursive(&self, name: &str) {
        if self.name().is_some() {
            return;
        }
        *self.name.borrow_mut() = Some(name.to_owned());
        for stmt in &self.statements {
            if let Statement::Push(Value::Macro(inner)) = stmt {
                inner.set_name_recursive(name);
            }
        }
    }

    /// Set the secure flag of this macro and of every nested macro (`Macro.setSecureRecursive`).
    pub fn set_secure_recursive(&self, secure: bool) {
        self.secure.set(secure);
        for stmt in &self.statements {
            if let Statement::Push(Value::Macro(inner)) = stmt {
                inner.set_secure_recursive(secure);
            }
        }
    }

    /// WarpScript source that recreates the macro (`Macro.snapshot`). Secure macros are
    /// rendered as `<% /* Secure Macro */ %>` unless `hide_secure` is `false`.
    pub fn snapshot(&self, hide_secure: bool) -> String {
        snapshot::macro_snapshot(self, hide_secure)
    }
}

impl Default for Macro {
    fn default() -> Self {
        Macro::new()
    }
}

impl fmt::Display for Macro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.snapshot(true))
    }
}

impl fmt::Debug for Macro {
    /// The full source of the macro, secure or not.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Macro({})", self.snapshot(false))
    }
}
