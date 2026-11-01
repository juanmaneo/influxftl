//! Capability functions of WarpLib: `CAPGET`, `CAPCHECK`, `CAPDEL`, `CAPADD` and
//! `CAPEXPORT` (`io.warp10.warp.sdk.Capabilities`).
//!
//! Capabilities are `STRING` → `STRING` attributes granted by the token a script runs with;
//! functions such as `ARGON2` or `MAXOPS` consult them to allow more than the defaults. The
//! host installs them with [`Stack::capabilities_mut`]; default capabilities
//! (`warp.capabilities.default`) are merged there too (see `docs/DIVERGENCES.md`).

use indexmap::IndexMap;

use super::Entry;
use super::args::fail;
use crate::error::Result;
use crate::stack::{CAPABILITIES_PREFIX, Stack};
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("CAPGET", Entry::Stack(capget)),
    ("CAPCHECK", Entry::Stack(capcheck)),
    ("CAPDEL", Entry::Stack(capdel)),
    ("CAPADD", Entry::Stack(capadd)),
    ("CAPEXPORT", Entry::Stack(capexport)),
];

/// `CAPGET`: the value of a capability (`NULL` when it is not granted), or a map of the
/// requested capabilities for a `LIST` of names (every capability for `NULL` or an empty
/// list).
fn capget(stack: &mut Stack) -> Result<()> {
    let value = match stack.pop()? {
        Value::String(name) => stack.capability(&name).map_or(Value::Null, Value::from),
        Value::List(names) => capabilities_map(stack, &names.borrow()),
        Value::Null => capabilities_map(stack, &[]),
        _ => {
            return Err(fail(
                "CAPGET",
                "expects a capability name (STRING) or a LIST thereof.",
            ));
        }
    };
    stack.push(value);
    Ok(())
}

/// `Capabilities.get(stack, names)`: all the capabilities when `names` is empty, otherwise
/// an entry per `STRING` name whose value is `NULL` when the capability is not granted (unless
/// no capability at all was granted, like a stack without a `Capabilities` object).
fn capabilities_map(stack: &Stack, names: &[Value]) -> Value {
    let granted = stack.capabilities();
    let mut map = IndexMap::new();
    if names.is_empty() {
        for (name, value) in granted {
            map.insert(Value::from(name.as_str()), Value::from(value.as_str()));
        }
    } else if !granted.is_empty() {
        for name in names {
            if let Value::String(name) = name {
                let value = granted
                    .get(&**name)
                    .map_or(Value::Null, |v| Value::from(v.as_str()));
                map.insert(Value::String(name.clone()), value);
            }
        }
    }
    Value::map(map)
}

/// `CAPCHECK`: whether a capability is granted.
fn capcheck(stack: &mut Stack) -> Result<()> {
    let Value::String(name) = stack.pop()? else {
        return Err(fail("CAPCHECK", "expects a STRING capability name."));
    };
    let granted = stack.capability(&name).is_some();
    stack.push(Value::Boolean(granted));
    Ok(())
}

/// `CAPDEL`: revoke a capability, a `LIST` of capabilities, or all of them for `NULL`.
fn capdel(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::String(name) => {
            stack.capabilities_mut().shift_remove(&*name);
        }
        Value::List(names) => {
            for name in names.borrow().iter() {
                if let Value::String(name) = name {
                    stack.capabilities_mut().shift_remove(&**name);
                }
            }
        }
        Value::Null => stack.capabilities_mut().clear(),
        _ => {
            return Err(fail(
                "CAPDEL",
                "expects a capability name (STRING), a LIST thereof or NULL.",
            ));
        }
    }
    Ok(())
}

/// `CAPADD`: grant the capabilities carried by a token (its `.cap:` attributes), without
/// overriding those already granted.
fn capadd(stack: &mut Stack) -> Result<()> {
    let Value::String(token) = stack.pop()? else {
        return Err(fail("CAPADD", "expects a TOKEN."));
    };
    let attributes = stack
        .token_resolver()
        .and_then(|resolver| resolver.token_attributes(&token))
        .ok_or_else(|| crate::error::Error::new("invalid token."))?;
    for (key, value) in attributes {
        if let Some(name) = key.strip_prefix(CAPABILITIES_PREFIX) {
            stack
                .capabilities_mut()
                .entry(name.to_owned())
                .or_insert(value);
        }
    }
    Ok(())
}

/// `CAPEXPORT`: within a `GUARD`ed macro, mark a capability (or a `LIST` of them, or `NULL`)
/// as exported to the caller.
fn capexport(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "CAPEXPORT";
    const MESSAGE: &str = "expects NULL, a capability name (STRING) or a LIST thereof.";
    let top = stack.pop()?;
    let Some(exported) = stack.exported_capabilities_mut() else {
        return Err(fail(NAME, "can only be called from a GUARDed macro."));
    };
    let mut add = |name: Option<String>| {
        if !exported.contains(&name) {
            exported.push(name);
        }
    };
    match top {
        Value::String(name) => add(Some(name.to_string())),
        Value::Null => add(None),
        Value::List(names) => {
            for name in names.borrow().iter() {
                match name {
                    Value::String(name) => add(Some(name.to_string())),
                    Value::Null => add(None),
                    _ => return Err(fail(NAME, MESSAGE)),
                }
            }
        }
        _ => return Err(fail(NAME, MESSAGE)),
    }
    Ok(())
}
