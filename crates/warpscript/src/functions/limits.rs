//! Limit functions of WarpLib: `MAXDEPTH`, `MAXOPS`, `MAXLOOP`, `MAXBUCKETS`, `MAXGEOCELLS`,
//! `MAXPIXELS`, `MAXRECURSION`, `MAXSYMBOLS`, `LIMIT`, `MAXGTS` (and `MAXJSON`, which lives
//! with the JSON functions) raise or lower a soft limit of the stack, and `OPS` reports the
//! operation count.
//!
//! Raising a limit requires the `limits` capability or the limit's own capability
//! (`maxops`, `maxdepth`…); the new value may not exceed the capability value when it is
//! set, or the hard limit of the stack otherwise.

use super::Entry;
use super::args::{fail, pop_long};
use crate::error::{Error, Result};
use crate::stack::{Limits, Stack};
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("MAXDEPTH", Entry::Stack(maxdepth)),
    ("MAXOPS", Entry::Stack(maxops)),
    ("MAXLOOP", Entry::Stack(maxloop)),
    ("MAXBUCKETS", Entry::Stack(maxbuckets)),
    ("MAXGEOCELLS", Entry::Stack(maxgeocells)),
    ("MAXPIXELS", Entry::Stack(maxpixels)),
    ("MAXRECURSION", Entry::Stack(maxrecursion)),
    ("MAXSYMBOLS", Entry::Stack(maxsymbols)),
    ("LIMIT", Entry::Stack(limit)),
    ("MAXGTS", Entry::Stack(maxgts)),
    ("OPS", Entry::Stack(ops)),
];

/// The capability allowing every limit to be raised (`CAPABILITY_LIMITS`).
pub const CAPABILITY_LIMITS: &str = "limits";

/// Pop the new value of a limit and check it against the capabilities (`MAXOPS` & co.):
/// `capability` or `limits` must be granted; the value may not exceed the parsed
/// `capability` when it is set (`Capabilities.getLong`), or `hard` otherwise.
pub(crate) fn pop_limit(
    stack: &mut Stack,
    name: &str,
    capability: &str,
    hard: i64,
    expects: &str,
) -> Result<i64> {
    if stack.capability(CAPABILITY_LIMITS).is_none() && stack.capability(capability).is_none() {
        return Err(fail(
            name,
            &format!("missing capability '{capability}' or '{CAPABILITY_LIMITS}'."),
        ));
    }
    let limit = pop_long(stack, name, expects)?;
    let max = match stack.capability(capability) {
        Some(value) => value
            .parse::<i64>()
            .map_err(|_| Error::new("Error parsing capability."))?,
        None => hard,
    };
    if limit > max {
        return Err(fail(name, &format!("cannot extend limit past {max}")));
    }
    Ok(limit)
}

/// `pop_limit` for the limits that are Java `long` attributes, applied with `set`.
fn set_long_limit(
    stack: &mut Stack,
    name: &str,
    capability: &str,
    hard: fn(&Limits) -> i64,
    set: fn(&mut Limits, i64),
) -> Result<()> {
    let hard = hard(&stack.hard_limits);
    let limit = pop_limit(
        stack,
        name,
        capability,
        hard,
        "expects a numeric (long) limit.",
    )?;
    set(&mut stack.limits, limit);
    Ok(())
}

/// Java `(int) limit` for the limits that are `int` attributes: the low 32 bits, and a
/// negative result (which lets nothing through) becomes 0.
fn int_limit(limit: i64) -> usize {
    usize::try_from(limit as i32).unwrap_or_default()
}

/// A hard limit as a `LONG` (saturating: `Limits::unlimited` uses values beyond `i64`).
fn hard_long<T: TryInto<i64>>(hard: T) -> i64 {
    hard.try_into().unwrap_or(i64::MAX)
}

/// `MAXDEPTH`: set the maximum stack depth; fails when the stack already holds more
/// elements (`IndexOutOfBoundsException` in `setAttribute`).
fn maxdepth(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MAXDEPTH";
    let hard = hard_long(stack.hard_limits.max_depth);
    let limit = pop_limit(
        stack,
        NAME,
        "maxdepth",
        hard,
        "expects a numeric (long) limit.",
    )?;
    let limit = int_limit(limit);
    if stack.total_depth() > limit {
        return Err(super::args::fail_with_cause(
            NAME,
            "failed.",
            Error::new("The stack depth is over the requested maximum depth."),
        ));
    }
    stack.limits.max_depth = limit;
    Ok(())
}

/// `MAXOPS`: set the maximum number of operations.
fn maxops(stack: &mut Stack) -> Result<()> {
    let hard = hard_long(stack.hard_limits.max_ops);
    let limit = pop_limit(
        stack,
        "MAXOPS",
        "maxops",
        hard,
        "expects a numeric (long) limit.",
    )?;
    stack.limits.max_ops = u64::try_from(limit).unwrap_or_default();
    Ok(())
}

/// `MAXLOOP`: set the maximum duration (ms) of a loop.
fn maxloop(stack: &mut Stack) -> Result<()> {
    set_long_limit(
        stack,
        "MAXLOOP",
        "maxloop",
        |l| l.max_loop_duration,
        |l, v| l.max_loop_duration = v,
    )
}

/// `MAXBUCKETS`: set the maximum number of buckets of `BUCKETIZE`.
fn maxbuckets(stack: &mut Stack) -> Result<()> {
    set_long_limit(
        stack,
        "MAXBUCKETS",
        "maxbuckets",
        |l| l.max_buckets,
        |l, v| l.max_buckets = v,
    )
}

/// `MAXGEOCELLS`: set the maximum number of cells of a geo shape.
fn maxgeocells(stack: &mut Stack) -> Result<()> {
    set_long_limit(
        stack,
        "MAXGEOCELLS",
        "maxgeocells",
        |l| l.max_geocells,
        |l, v| l.max_geocells = v,
    )
}

/// `MAXPIXELS`: set the maximum number of pixels of an image.
fn maxpixels(stack: &mut Stack) -> Result<()> {
    set_long_limit(
        stack,
        "MAXPIXELS",
        "maxpixels",
        |l| l.max_pixels,
        |l, v| l.max_pixels = v,
    )
}

/// `MAXRECURSION`: set the maximum recursion level.
fn maxrecursion(stack: &mut Stack) -> Result<()> {
    let hard = hard_long(stack.hard_limits.max_recursion);
    let limit = pop_limit(
        stack,
        "MAXRECURSION",
        "maxrecursion",
        hard,
        "expects a numeric (long) limit.",
    )?;
    stack.limits.max_recursion = int_limit(limit) as u64;
    Ok(())
}

/// `MAXSYMBOLS`: set the maximum number of symbols.
fn maxsymbols(stack: &mut Stack) -> Result<()> {
    let hard = hard_long(stack.hard_limits.max_symbols);
    let limit = pop_limit(
        stack,
        "MAXSYMBOLS",
        "maxsymbols",
        hard,
        "expects a numeric (long) limit.",
    )?;
    stack.limits.max_symbols = int_limit(limit);
    Ok(())
}

/// `LIMIT`: set the maximum number of datapoints `FETCH` may read.
fn limit(stack: &mut Stack) -> Result<()> {
    let hard = stack.hard_limits.fetch_limit;
    let limit = pop_limit(
        stack,
        "LIMIT",
        "limit",
        hard,
        "expects a numeric (LONG) limit.",
    )?;
    stack.limits.fetch_limit = limit;
    Ok(())
}

/// `MAXGTS`: set the maximum number of Geo Time Series `FETCH` may read.
fn maxgts(stack: &mut Stack) -> Result<()> {
    set_long_limit(
        stack,
        "MAXGTS",
        "maxgts",
        |l| l.gts_limit,
        |l, v| l.gts_limit = v,
    )
}

/// `OPS`: the number of operations performed so far.
fn ops(stack: &mut Stack) -> Result<()> {
    let ops = stack.ops();
    stack.push(Value::Long(ops as i64));
    Ok(())
}
