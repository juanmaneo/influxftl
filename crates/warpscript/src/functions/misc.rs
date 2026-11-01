//! Constants, `TYPEOF`, secure scripts and the small configuration functions.

use super::Entry;
use super::args::{fail, pop_long, pop_map, pop_string};
use crate::error::{Error, Result};
use crate::snapshot::value_snapshot;
use crate::stack::Stack;
use crate::value::Value;

/// Stack attribute holding the secure key (`ATTRIBUTE_SECURE_KEY`).
pub const ATTRIBUTE_SECURE_KEY: &str = "secure.key";
/// Stack attribute enabling the info mode (`ATTRIBUTE_INFOMODE`).
pub const ATTRIBUTE_INFOMODE: &str = "infomode";
/// Stack attribute set by `JSONSTRICT` (`ATTRIBUTE_JSON_STRICT`).
pub const ATTRIBUTE_JSON_STRICT: &str = "json.strict";
/// Stack attribute set by `MACROTTL` (`ATTRIBUTE_MACRO_TTL`), in ms.
pub const ATTRIBUTE_MACRO_TTL: &str = "macro.ttl";

/// Platform time units per millisecond (the platform runs in microseconds).
pub const TIME_UNITS_PER_MS: i64 = 1000;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("NULL", Entry::Stack(null)),
    ("NaN", Entry::Stack(nan)),
    ("PI", Entry::Stack(pi)),
    ("pi", Entry::Stack(pi)),
    ("E", Entry::Stack(e)),
    ("e", Entry::Stack(e)),
    ("MINLONG", Entry::Stack(minlong)),
    ("MAXLONG", Entry::Stack(maxlong)),
    ("TYPEOF", Entry::Stack(typeof_fn)),
    ("INFO", Entry::Stack(info)),
    ("JSONSTRICT", Entry::Stack(jsonstrict)),
    ("MACROTTL", Entry::Stack(macrottl)),
    ("MACROCONFIG", Entry::Stack(macroconfig)),
    ("MACROCONFIGDEFAULT", Entry::Stack(macroconfigdefault)),
    ("SECUREKEY", Entry::Stack(securekey)),
    ("SECURE", Entry::Stack(secure)),
    ("UNSECURE", Entry::Stack(unsecure)),
    ("EVALSECURE", Entry::Stack(evalsecure)),
];

fn null(stack: &mut Stack) -> Result<()> {
    stack.push(Value::Null);
    Ok(())
}

fn nan(stack: &mut Stack) -> Result<()> {
    stack.push(f64::NAN);
    Ok(())
}

fn pi(stack: &mut Stack) -> Result<()> {
    stack.push(std::f64::consts::PI);
    Ok(())
}

fn e(stack: &mut Stack) -> Result<()> {
    stack.push(std::f64::consts::E);
    Ok(())
}

fn minlong(stack: &mut Stack) -> Result<()> {
    stack.push(i64::MIN);
    Ok(())
}

fn maxlong(stack: &mut Stack) -> Result<()> {
    stack.push(i64::MAX);
    Ok(())
}

fn typeof_fn(stack: &mut Stack) -> Result<()> {
    let name = stack.pop()?.type_name();
    stack.push(name);
    Ok(())
}

/// `INFO`: drop the documentation map, unless the info mode is on, in which case a copy of the
/// map is left on the stack and the execution stops.
fn info(stack: &mut Stack) -> Result<()> {
    let doc = pop_map(stack, "INFO", "expects an info map on top of the stack.")?;
    if matches!(
        stack.attribute(ATTRIBUTE_INFOMODE),
        Some(Value::Boolean(true))
    ) {
        let source = value_snapshot(&Value::Map(doc));
        stack.exec_multi(&source)?;
        return Err(Error::Stop(String::new()));
    }
    Ok(())
}

fn jsonstrict(stack: &mut Stack) -> Result<()> {
    stack.set_attribute(ATTRIBUTE_JSON_STRICT, Some(Value::Boolean(true)));
    Ok(())
}

fn macrottl(stack: &mut Stack) -> Result<()> {
    let ttl = pop_long(stack, "MACROTTL", "expects a ttl on top of the stack.")?;
    stack.set_attribute(
        ATTRIBUTE_MACRO_TTL,
        Some(Value::Long(ttl / TIME_UNITS_PER_MS)),
    );
    Ok(())
}

/// `MACROCONFIG` / `MACROCONFIGDEFAULT`: look `key@macro/name` up in the configuration,
/// falling back to `key@macro`, then `key@` of each parent path.
fn macro_config(stack: &mut Stack, name: &str, with_default: bool) -> Result<()> {
    let Some(macro_name) = stack.macro_name().map(str::to_owned) else {
        return Err(fail(name, "can only be used from named macro."));
    };
    let default = if with_default {
        Some(match stack.pop()? {
            Value::Null => Value::Null,
            other => Value::from(other.to_string()),
        })
    } else {
        None
    };
    let key = pop_string(stack, name, "expects a macro configuration key name.")?;
    let key = key.trim();
    let mut attempts = Vec::new();
    let mut path = macro_name.as_str();
    loop {
        let attempt = format!("{key}@{path}");
        if let Some(value) = stack.config_property(&attempt) {
            let value = value.to_owned();
            stack.push(value);
            return Ok(());
        }
        attempts.push(attempt);
        match path.rfind('/') {
            Some(idx) => path = &path[..idx],
            None => break,
        }
    }
    if let Some(default) = default {
        stack.push(default);
        return Ok(());
    }
    let mut message = format!("{name} macro configuration '{key}' not found, need to set one of [");
    for attempt in &attempts {
        message.push_str(&format!(" '{attempt}'"));
    }
    message.push_str(" ].");
    Err(Error::new(message))
}

fn macroconfig(stack: &mut Stack) -> Result<()> {
    macro_config(stack, "MACROCONFIG", false)
}

fn macroconfigdefault(stack: &mut Stack) -> Result<()> {
    macro_config(stack, "MACROCONFIGDEFAULT", true)
}

fn securekey(stack: &mut Stack) -> Result<()> {
    let key = pop_string(stack, "SECUREKEY", "only accepts a string secure key.")?;
    stack.set_attribute(ATTRIBUTE_SECURE_KEY, Some(Value::String(key)));
    Ok(())
}

/// The secure key attribute, when set.
fn secure_key(stack: &Stack) -> Option<String> {
    stack.attribute(ATTRIBUTE_SECURE_KEY).map(Value::to_string)
}

fn secure(stack: &mut Stack) -> Result<()> {
    let script = stack.pop()?;
    let Some(key) = secure_key(stack) else {
        return Err(Error::new("You need to set the secure key first."));
    };
    let Value::String(script) = script else {
        return Err(fail("SECURE", "operates on a string."));
    };
    let Some(codec) = stack.secure_codec() else {
        return Err(Error::new("Missing secure script encryption key."));
    };
    let encoded = codec.secure(&key, &script)?;
    stack.push(encoded);
    Ok(())
}

/// Decode a secure script, checking its key against the stack's secure key when `check_key`.
fn unsecure_script(stack: &mut Stack, name: &str, check_key: bool) -> Result<String> {
    let encoded = stack.pop()?;
    let key = secure_key(stack);
    if check_key && key.is_none() {
        return Err(Error::new("You need to set the secure key first."));
    }
    let Value::String(encoded) = encoded else {
        return Err(fail(name, "operates on a string."));
    };
    let Some(codec) = stack.secure_codec() else {
        return Err(Error::new("Missing secure script encryption key."));
    };
    let decoded = codec.unsecure(&encoded)?;
    if check_key && key.as_deref() != Some(decoded.key.as_str()) {
        return Err(Error::new("Invalid secure key."));
    }
    Ok(decoded.script)
}

fn unsecure(stack: &mut Stack) -> Result<()> {
    let script = unsecure_script(stack, "UNSECURE", true)?;
    stack.push(script);
    Ok(())
}

/// `EVALSECURE`: decode and execute a secure script in secure mode, with redefined functions
/// disabled. On error the stack is cleared so that nothing leaks.
fn evalsecure(stack: &mut Stack) -> Result<()> {
    if !matches!(stack.peek()?, Value::String(_)) {
        return Err(fail(
            "EVALSECURE",
            "can only evaluate a secure script stored as a string.",
        ));
    }
    let was_secure = stack.set_in_secure_macro(true);
    let allowed = stack.set_allow_redefined(false);
    let result = unsecure_script(stack, "", false).and_then(|script| stack.exec_multi(&script));
    stack.set_in_secure_macro(was_secure);
    stack.set_allow_redefined(allowed);
    match result {
        Ok(()) => Ok(()),
        Err(Error::Return) => {
            if stack.decrement_return_depth() > 0 {
                Err(Error::Return)
            } else {
                Ok(())
            }
        }
        Err(err) if err.is_signal() => Err(err),
        Err(err) => {
            stack.clear();
            Err(err)
        }
    }
}
