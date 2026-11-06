// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Platform functions whose effects belong to the host: the manager switches (`UPDATEON`…),
//! WarpFleet repositories (`WFON`, `WF.*`), tokens (`TOKENINFO`, `GETHOOK`, `KVSTORE`,
//! `KVLOAD`, `DUMP`), runners (`RUNNER*`), the accelerator (`ACCEL.*`), logging (`LOGMSG`,
//! `NOLOG`, `LOGINIT`), auditing (`WSAUDIT`, `WSAUDITMODE`), `SETMACROCONFIG`, `PIGSCHEMA`
//! and `[[]]`.
//!
//! The runtime knows no Warp 10 token (InfluxDB authenticates the requests): every token is
//! invalid, as for a Warp 10 instance that did not issue it.

use indexmap::IndexMap;

use super::Entry;
use super::args::fail;
use crate::error::{Error, Result};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "UPDATEON",
        Entry::Stack(|s| manager(s, "UPDATEON", "update.disabled", true)),
    ),
    (
        "UPDATEOFF",
        Entry::Stack(|s| manager(s, "UPDATEOFF", "update.disabled", false)),
    ),
    (
        "METAON",
        Entry::Stack(|s| manager(s, "METAON", "meta.disabled", true)),
    ),
    (
        "METAOFF",
        Entry::Stack(|s| manager(s, "METAOFF", "meta.disabled", false)),
    ),
    (
        "DELETEON",
        Entry::Stack(|s| manager(s, "DELETEON", "delete.disabled", true)),
    ),
    (
        "DELETEOFF",
        Entry::Stack(|s| manager(s, "DELETEOFF", "delete.disabled", false)),
    ),
    ("WFON", Entry::Stack(|s| warpfleet(s, None))),
    (
        "WFOFF",
        Entry::Stack(|s| warpfleet(s, Some(Value::Boolean(true)))),
    ),
    ("WF.ADDREPO", Entry::Stack(wf_addrepo)),
    ("WF.GETREPOS", Entry::Stack(wf_getrepos)),
    ("WF.SETREPOS", Entry::Stack(wf_setrepos)),
    ("TOKENINFO", Entry::Stack(tokeninfo)),
    ("GETHOOK", Entry::Stack(gethook)),
    ("KVSTORE", Entry::Stack(|s| write_token(s, "KVSTORE"))),
    ("DUMP", Entry::Stack(|s| write_token(s, "DUMP"))),
    ("KVLOAD", Entry::Stack(kvload)),
    (
        "RUNNERNONCE",
        Entry::Stack(|s| runner_nonce(s, "RUNNERNONCE")),
    ),
    (
        "RUNNERPATH",
        Entry::Stack(|s| runner_nonce(s, "RUNNERPATH")),
    ),
    (
        "RUNNERAT",
        Entry::Stack(|s| runner_schedule(s, "RUNNERAT", true)),
    ),
    (
        "RUNNERIN",
        Entry::Stack(|s| runner_schedule(s, "RUNNERIN", false)),
    ),
    (
        "ACCEL.CACHE",
        Entry::Stack(|s| attribute(s, "accel.nocache", false)),
    ),
    (
        "ACCEL.NOCACHE",
        Entry::Stack(|s| attribute(s, "accel.nocache", true)),
    ),
    (
        "ACCEL.PERSIST",
        Entry::Stack(|s| attribute(s, "accel.nopersist", false)),
    ),
    (
        "ACCEL.NOPERSIST",
        Entry::Stack(|s| attribute(s, "accel.nopersist", true)),
    ),
    ("ACCEL.REPORT", Entry::Stack(accel_report)),
    ("LOGMSG", Entry::Stack(logmsg)),
    ("NOLOG", Entry::Stack(|_| Ok(()))),
    ("LOGINIT", Entry::Stack(|_| Ok(()))),
    ("WSAUDIT", Entry::Stack(wsaudit)),
    (
        "WSAUDITMODE",
        Entry::Stack(|s| attribute(s, "audit.mode", true)),
    ),
    ("SETMACROCONFIG", Entry::Stack(setmacroconfig)),
    ("PIGSCHEMA", Entry::Stack(pigschema)),
    (
        "[[]]",
        Entry::Stack(|s| {
            s.push(Value::list(Vec::new()));
            Ok(())
        }),
    ),
];

/// `Tokens.extractReadToken` / `extractWriteToken` of a token this runtime did not issue.
fn invalid_token() -> Error {
    Error::new("Invalid token.")
}

/// Set a boolean stack attribute.
fn attribute(stack: &mut Stack, name: &str, value: bool) -> Result<()> {
    stack.set_attribute(name, Some(Value::Boolean(value)));
    Ok(())
}

/// `MANAGERONOFF`: with the `manager` capability, enable a datastore operation or disable it
/// with the message on top of the stack (recorded as the `manager.<attribute>` attribute).
fn manager(stack: &mut Stack, name: &str, attribute: &str, on: bool) -> Result<()> {
    if stack.capability("manager").is_none() {
        return Err(fail(name, "missing manager capability."));
    }
    let message = if on {
        None
    } else {
        Some(Value::from(stack.pop()?.to_string()))
    };
    stack.set_attribute(&format!("manager.{attribute}"), message);
    Ok(())
}

/// `WFON` / `WFOFF`.
fn warpfleet(stack: &mut Stack, disable: Option<Value>) -> Result<()> {
    stack.set_attribute("warpfleet.disable", disable);
    Ok(())
}

/// The WarpFleet repositories of the stack (none by default).
fn repos(stack: &Stack) -> Vec<Value> {
    match stack.attribute("warpfleet.repos") {
        Some(Value::List(list)) => list.borrow().clone(),
        _ => Vec::new(),
    }
}

/// Fail without the `wfset` capability.
fn require_wfset(stack: &Stack, name: &str) -> Result<()> {
    if stack.capability("wfset").is_none() {
        return Err(fail(name, "expects the capability 'wfset' to be set."));
    }
    Ok(())
}

/// `WF.ADDREPO`: append a repository URL.
fn wf_addrepo(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "WF.ADDREPO";
    require_wfset(stack, NAME)?;
    let Value::String(url) = stack.pop()? else {
        return Err(fail(NAME, "expects a repository URL on top of the stack."));
    };
    let mut list = repos(stack);
    list.push(Value::String(url));
    stack.set_attribute("warpfleet.repos", Some(Value::list(list)));
    Ok(())
}

/// `WF.GETREPOS`: the repositories, or an empty list without the `wfget` capability.
fn wf_getrepos(stack: &mut Stack) -> Result<()> {
    let list = if stack.capability("wfget").is_some() {
        repos(stack)
    } else {
        Vec::new()
    };
    stack.push(Value::list(list));
    Ok(())
}

/// `WF.SETREPOS`: replace the repositories (`NULL` restores the default ones).
fn wf_setrepos(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "WF.SETREPOS";
    const URLS: &str = "expects a list of repository URLs on top of the stack.";
    require_wfset(stack, NAME)?;
    let list = match stack.pop()? {
        Value::Null => None,
        Value::List(list) => {
            let urls = list.borrow().clone();
            if urls.iter().any(|u| !matches!(u, Value::String(_))) {
                return Err(fail(NAME, URLS));
            }
            Some(Value::list(urls))
        }
        _ => return Err(fail(NAME, URLS)),
    };
    stack.set_attribute("warpfleet.repos", list);
    Ok(())
}

/// `TOKENINFO`: the decoding errors of a token.
fn tokeninfo(stack: &mut Stack) -> Result<()> {
    let Value::String(_) = stack.pop()? else {
        return Err(fail("TOKENINFO", "expects a string on top of the stack."));
    };
    let mut info = IndexMap::new();
    info.insert(
        Value::from("ReadTokenDecodeError"),
        Value::from("Invalid token."),
    );
    info.insert(
        Value::from("WriteTokenDecodeError"),
        Value::from("Invalid token."),
    );
    stack.push(Value::map(info));
    Ok(())
}

/// `GETHOOK`: the hook of a read token.
fn gethook(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "GETHOOK";
    let Value::String(_) = stack.pop()? else {
        return Err(fail(NAME, "expects a hook name on top of the stack."));
    };
    let Value::String(_) = stack.pop()? else {
        return Err(fail(NAME, "expects a read token below the hook name."));
    };
    // `new WarpScriptException(ee)`: the message is the cause's `toString()`.
    Err(Error::with_cause(
        "io.warp10.script.WarpScriptException: Invalid token.",
        invalid_token(),
    ))
}

/// `KVSTORE` / `DUMP`: a write token on top of the stack.
fn write_token(stack: &mut Stack, name: &str) -> Result<()> {
    let Value::String(_) = stack.pop()? else {
        return Err(fail(name, "missing token."));
    };
    Err(invalid_token())
}

/// `KVLOAD`: a map of parameters with a read token under `token`.
fn kvload(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "KVLOAD";
    let Value::Map(params) = stack.pop()? else {
        return Err(fail(NAME, "operates on a map."));
    };
    if !matches!(
        params.borrow().get(&Value::from("token")),
        Some(Value::String(_))
    ) {
        return Err(fail(NAME, "expects a token under 'token'."));
    }
    Err(invalid_token())
}

/// `RUNNERNONCE` / `RUNNERPATH`: without the runner key of a Warp 10 instance, no nonce can be
/// decoded and the result is `NULL`.
fn runner_nonce(stack: &mut Stack, name: &str) -> Result<()> {
    let Value::String(_) = stack.pop()? else {
        return Err(fail(name, "expects a String."));
    };
    stack.push(Value::Null);
    Ok(())
}

/// `RUNNERAT` / `RUNNERIN`: reschedule the current runner script at a time or after a period,
/// no sooner than the `runner.reschedule.min.period` capability allows.
fn runner_schedule(stack: &mut Stack, name: &str, at: bool) -> Result<()> {
    const CAPABILITY: &str = "runner.reschedule.min.period";
    let Some(capability) = stack.capability(CAPABILITY).map(str::to_owned) else {
        return Err(fail(name, &format!("requires capability {CAPABILITY}.")));
    };
    let Ok(min_period) = capability.parse::<i64>() else {
        return Err(fail(
            name,
            &format!("cannot parse capability {CAPABILITY}: '{capability}' is not a valid LONG"),
        ));
    };
    if min_period <= 0 {
        return Err(fail(
            name,
            &format!(
                "requires capability {CAPABILITY} to be set to a value strictly greater than 0 ms."
            ),
        ));
    }
    let Value::Long(value) = stack.pop()? else {
        return Err(fail(
            name,
            if at {
                "expects a LONG timestamp as parameter."
            } else {
                "expects a LONG period as parameter."
            },
        ));
    };
    let ms = value / super::misc::TIME_UNITS_PER_MS;
    if at {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        if ms < now.wrapping_add(min_period) {
            return Err(fail(
                name,
                &format!(
                    "cannot schedule a task before now + {min_period} ms defined in {CAPABILITY} capability."
                ),
            ));
        }
        stack.set_attribute("runner.reschedule.timestamp", Some(Value::Long(ms)));
    } else {
        if ms < min_period {
            return Err(fail(
                name,
                &format!(
                    "cannot set period below {min_period} ms defined in {CAPABILITY} capability."
                ),
            ));
        }
        stack.set_attribute("runner.reschedule.period", Some(Value::Long(ms)));
    }
    Ok(())
}

/// `ACCEL.REPORT`: the status of the (absent) in-memory accelerator.
fn accel_report(stack: &mut Stack) -> Result<()> {
    let defaults = || Value::list(vec![Value::from("cache"), Value::from("persist")]);
    let mut report = IndexMap::new();
    report.insert(Value::from("status"), Value::Boolean(false));
    report.insert(Value::from("cache"), Value::Boolean(false));
    report.insert(Value::from("persist"), Value::Boolean(true));
    report.insert(Value::from("accelerated"), Value::Boolean(false));
    report.insert(Value::from("chunkcount"), Value::Long(0));
    report.insert(Value::from("chunkspan"), Value::Long(0));
    report.insert(Value::from("defaults.read"), defaults());
    report.insert(Value::from("defaults.write"), defaults());
    report.insert(Value::from("defaults.delete"), defaults());
    stack.push(Value::map(report));
    Ok(())
}

/// `LOGMSG`: the message is consumed (the host logs nothing).
fn logmsg(stack: &mut Stack) -> Result<()> {
    stack.pop()?;
    Ok(())
}

/// `WSAUDIT`: the audit statements collected while parsing (none: the parser does not audit).
fn wsaudit(stack: &mut Stack) -> Result<()> {
    stack.push(Value::list(Vec::new()));
    Ok(())
}

/// `SETMACROCONFIG`: set (or remove with `NULL`) a `name@scope` configuration key that the
/// `setmacroconfig` capability (a regular expression) allows.
fn setmacroconfig(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "SETMACROCONFIG";
    let Some(pattern) = stack.capability("setmacroconfig").map(str::to_owned) else {
        return Err(fail(NAME, "missing capability."));
    };
    let Value::String(key) = stack.pop()? else {
        return Err(fail(NAME, "expects the configuration key to be a STRING."));
    };
    if !key.contains('@') {
        return Err(fail(
            NAME,
            "expects the configuration key to adhere to the format name@scope.",
        ));
    }
    let regex = super::strings::CompiledPattern::compile(&pattern)?;
    if regex.full_match(&key)?.is_empty() {
        return Err(fail(
            NAME,
            "key is not modifiable using the current capability.",
        ));
    }
    match stack.pop()? {
        Value::Null => {
            stack.config_mut().shift_remove(key.as_ref());
        }
        value => {
            stack
                .config_mut()
                .insert(key.to_string(), value.to_string());
        }
    }
    Ok(())
}

/// `PIGSCHEMA`: the Pig schema of the stack (top first).
fn pigschema(stack: &mut Stack) -> Result<()> {
    let mut fields = Vec::new();
    for i in 0..stack.depth() {
        let kind = match stack.get(i).unwrap_or(&Value::Null) {
            Value::Long(_) => "long",
            Value::Double(_) => "double",
            Value::BigDecimal(_) => "bigdecimal",
            Value::String(_) => "chararray",
            Value::Boolean(_) => "boolean",
            Value::Set(_) => "bag{}",
            Value::List(_) => "tuple:()",
            Value::Map(_) => "map:[]",
            _ => "bytearray",
        };
        let field = if i == 0 {
            "top".to_owned()
        } else {
            format!("l{}", i + 1)
        };
        fields.push(format!("{field}: {kind}"));
    }
    stack.push(Value::from(format!("({})", fields.join(", "))));
    Ok(())
}
