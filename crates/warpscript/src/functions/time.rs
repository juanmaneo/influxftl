// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Date and time functions of WarpLib: `ISO8601`, `TSELEMENTS` / `->TSELEMENTS`,
//! `TSELEMENTS->`, `ADDDAYS`, `ADDMONTHS`, `ADDYEARS`, `ADDDURATION`, `DURATION`,
//! `ISODURATION`, `HUMANDURATION`, `TOTIMESTAMP`, `NOTBEFORE` and `NOTAFTER`.
//!
//! Warp 10 relies on Joda-Time (and `java.time` for `TOTIMESTAMP`). The calendar arithmetic
//! is reproduced on `chrono` with the IANA database of `chrono-tz`, including Joda's
//! resolution of local times around daylight saving transitions (see
//! [`Zone::local_to_utc`]) and its ISO period parser (see [`parse_period`]).

use std::str::FromStr;

use chrono::{
    DateTime, Datelike, Days, FixedOffset, LocalResult, Months, NaiveDate, NaiveDateTime, Offset,
    TimeZone, Timelike,
};
use chrono_tz::Tz;

use super::Entry;
use super::args::{fail, fail_with_cause, int_value};
use super::misc::TIME_UNITS_PER_MS;
use super::runtime::current_time;
use crate::error::fault::{BoundsError, NumberFormatError};
use crate::error::time::TimeError;
use crate::error::{Class, Error, Result};
use crate::format::decimal_value;
use crate::stack::Stack;
use crate::value::Value;

/// Platform time units per second.
pub const TIME_UNITS_PER_S: i64 = 1000 * TIME_UNITS_PER_MS;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("ISO8601", Entry::Stack(iso8601)),
    ("TSELEMENTS", Entry::Stack(tselements)),
    ("->TSELEMENTS", Entry::Stack(totselements)),
    ("TSELEMENTS->", Entry::Stack(fromtselements)),
    ("ADDDAYS", Entry::Stack(adddays)),
    ("ADDMONTHS", Entry::Stack(addmonths)),
    ("ADDYEARS", Entry::Stack(addyears)),
    ("ADDDURATION", Entry::Stack(addduration)),
    ("DURATION", Entry::Stack(duration)),
    ("ISODURATION", Entry::Stack(isoduration)),
    ("HUMANDURATION", Entry::Stack(humanduration)),
    ("TOTIMESTAMP", Entry::Stack(totimestamp)),
    ("NOTBEFORE", Entry::Stack(notbefore)),
    ("NOTAFTER", Entry::Stack(notafter)),
];

// -----------------------------------------------------------------------------------------
// Time zones (`org.joda.time.DateTimeZone`)
// -----------------------------------------------------------------------------------------

/// A time zone: an IANA zone or a fixed offset (`DateTimeZone.forID`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// A zone of the IANA database.
    Named(Tz),
    /// A fixed offset such as `+02:00`.
    Fixed(FixedOffset),
}

impl Zone {
    /// `DateTimeZone.UTC`.
    pub const UTC: Zone = Zone::Named(Tz::UTC);

    /// `DateTimeZone.forID`: `UTC`, an IANA identifier, or an offset (`+HH`, `+HH:mm`,
    /// `+HHmm`, `+HH:mm:ss`, `+HHmmss`). `None` means UTC.
    pub fn for_id(id: Option<&str>) -> Result<Zone> {
        let Some(id) = id else {
            return Ok(Zone::UTC);
        };
        if id == "UTC" {
            return Ok(Zone::Named(Tz::UTC));
        }
        if let Ok(tz) = Tz::from_str(id) {
            return Ok(Zone::Named(tz));
        }
        if id.starts_with(['+', '-']) {
            let seconds = parse_offset(id)?;
            // Offsets are below a day, which `FixedOffset` always accepts.
            return Ok(if seconds == 0 {
                Zone::Named(Tz::UTC)
            } else {
                FixedOffset::east_opt(seconds).map_or(Zone::Named(Tz::UTC), Zone::Fixed)
            });
        }
        Err(TimeError::UnknownZone(id.to_string()).into())
    }

    /// `DateTimeZone.getID()`, without the resolution of the IANA links Joda performs
    /// (`US/Pacific` stays `US/Pacific` where Joda reports `America/Los_Angeles`).
    pub fn id(self) -> String {
        match self {
            Zone::Named(tz) => tz.name().to_owned(),
            Zone::Fixed(offset) => format_offset(offset.local_minus_utc(), false),
        }
    }

    /// The local date-time in this zone at `millis` milliseconds since the epoch.
    pub fn local_at(self, millis: i64) -> Result<NaiveDateTime> {
        Ok(to_local(self, utc_from_millis(millis)?).0)
    }

    /// The UTC offset in effect at the UTC instant `utc`, in seconds.
    fn offset_at(self, utc: NaiveDateTime) -> i32 {
        match self {
            Zone::Named(tz) => tz.offset_from_utc_datetime(&utc).fix().local_minus_utc(),
            Zone::Fixed(offset) => offset.local_minus_utc(),
        }
    }

    /// The UTC offsets valid for the local date-time `local`: one, two (overlap) or none
    /// (gap), in seconds.
    fn offsets_for_local(self, local: NaiveDateTime) -> LocalResult<i32> {
        match self {
            Zone::Named(tz) => tz
                .offset_from_local_datetime(&local)
                .map(|offset| offset.fix().local_minus_utc()),
            Zone::Fixed(offset) => LocalResult::Single(offset.local_minus_utc()),
        }
    }

    /// `DateTimeZone.getOffsetFromLocal` applied to a local date-time (the conversion Joda
    /// uses after adding days, months or years): in an overlap the earlier instant is chosen,
    /// in a gap the time is shifted forward by the size of the gap.
    fn local_to_utc(self, local: NaiveDateTime) -> NaiveDateTime {
        match self.offsets_for_local(local) {
            LocalResult::Single(offset) | LocalResult::Ambiguous(offset, _) => {
                shift(local, -offset)
            }
            LocalResult::None => {
                // The offset before the gap is the smaller of the two surrounding offsets.
                let offset_local = self.offset_at(local);
                let offset_adjusted = self.offset_at(shift(local, -offset_local));
                shift(local, -offset_local.min(offset_adjusted))
            }
        }
    }

    /// `new DateTime(fields…, zone)`: the UTC date-time of a local date-time, which must not
    /// fall in a daylight saving gap (`IllegalInstantException`); in an overlap the earlier
    /// instant is chosen.
    fn local_to_utc_strict(self, local: NaiveDateTime) -> Result<NaiveDateTime> {
        let Zone::Named(tz) = self else {
            // Fixed offsets have no transitions.
            return Ok(self.local_to_utc(local));
        };
        match self.offsets_for_local(local) {
            LocalResult::Single(offset) | LocalResult::Ambiguous(offset, _) => {
                Ok(shift(local, -offset))
            }
            LocalResult::None => Err(TimeError::ZoneGap {
                local: local.format("%Y-%m-%dT%H:%M:%S%.3f").to_string(),
                zone: tz.name().to_string(),
            }
            .into()),
        }
    }
}

/// `local + seconds`, saturating at the bounds of `chrono`.
fn shift(instant: NaiveDateTime, seconds: i32) -> NaiveDateTime {
    instant
        .checked_add_signed(chrono::TimeDelta::seconds(i64::from(seconds)))
        .unwrap_or(instant)
}

/// `Invalid format: "+2:00" is malformed at "2:00"`.
fn malformed(id: &str, rest: &str) -> Error {
    TimeError::MalformedOffset {
        id: id.to_string(),
        rest: rest.to_string(),
    }
    .into()
}

/// Parse a Joda offset identifier (`+HH`, `+HH:mm`, `+HHmm`, `+HH:mm:ss`, `+HHmmss`) into
/// seconds.
fn parse_offset(id: &str) -> Result<i32> {
    let negative = id.starts_with('-');
    let body = &id[1..];
    let two_digits = |at: usize, max: i32| -> Result<i32> {
        let digits = body
            .get(at..at + 2)
            .filter(|d| d.bytes().all(|b| b.is_ascii_digit()))
            .ok_or_else(|| malformed(id, body.get(at..).unwrap_or_default()))?;
        let value = decimal_value(digits) as i32;
        if value > max {
            return Err(malformed(id, body.get(at..).unwrap_or_default()));
        }
        Ok(value)
    };
    let hours = two_digits(0, 23)?;
    let mut pos = 2;
    let mut minutes = 0;
    let mut seconds = 0;
    if pos < body.len() {
        if body[pos..].starts_with(':') {
            pos += 1;
        }
        minutes = two_digits(pos, 59)?;
        pos += 2;
        if pos < body.len() {
            if body[pos..].starts_with(':') {
                pos += 1;
            }
            seconds = two_digits(pos, 59)?;
            pos += 2;
        }
    }
    if pos != body.len() {
        return Err(malformed(id, &body[pos..]));
    }
    let total = hours * 3600 + minutes * 60 + seconds;
    Ok(if negative { -total } else { total })
}

/// Format an offset in seconds as `+HH:mm` (`+HH:mm:ss` when there are seconds), or `Z` for
/// zero when `zulu` is set.
fn format_offset(seconds: i32, zulu: bool) -> String {
    if seconds == 0 && zulu {
        return "Z".to_owned();
    }
    let sign = if seconds < 0 { '-' } else { '+' };
    let seconds = seconds.abs();
    let (hours, minutes, secs) = (seconds / 3600, (seconds / 60) % 60, seconds % 60);
    if secs == 0 {
        format!("{sign}{hours:02}:{minutes:02}")
    } else {
        format!("{sign}{hours:02}:{minutes:02}:{secs:02}")
    }
}

// -----------------------------------------------------------------------------------------
// Instants
// -----------------------------------------------------------------------------------------

/// Split a timestamp in time units into milliseconds (floored) and the sub-millisecond rest
/// (always non-negative), like Warp 10 does before handing the millis to Joda.
fn split_millis(ts: i64) -> (i64, i64) {
    let millis = ts.div_euclid(TIME_UNITS_PER_MS);
    (millis, ts.rem_euclid(TIME_UNITS_PER_MS))
}

/// The UTC date-time of a timestamp in milliseconds (`Joda` supports years far beyond
/// `chrono`'s ±262143; out of range values are an error).
fn utc_from_millis(millis: i64) -> Result<NaiveDateTime> {
    DateTime::from_timestamp_millis(millis)
        .map(|dt| dt.naive_utc())
        .ok_or_else(|| TimeError::InstantOutOfRange(millis).into())
}

/// Milliseconds since the epoch of a UTC date-time.
fn millis_of(utc: NaiveDateTime) -> i64 {
    utc.and_utc().timestamp_millis()
}

/// A local date-time (and its offset) at the UTC date-time `utc` in `zone`.
fn to_local(zone: Zone, utc: NaiveDateTime) -> (NaiveDateTime, i32) {
    let offset = zone.offset_at(utc);
    (shift(utc, offset), offset)
}

/// Pop the optional time zone on top of the stack, then a `LONG` timestamp.
fn pop_timestamp_and_zone(stack: &mut Stack, name: &str) -> Result<(i64, Zone)> {
    const MESSAGE: &str = "operates on a timestamp or a timestamp + timezone.";
    let mut top = stack.pop()?;
    let mut tz = None;
    if let Value::String(id) = &top {
        tz = Some(id.to_string());
        top = stack.pop()?;
    }
    let Value::Long(ts) = top else {
        return Err(fail(name, MESSAGE));
    };
    Ok((ts, Zone::for_id(tz.as_deref())?))
}

/// `ISO8601`: the ISO 8601 representation of a timestamp (`yyyy-MM-ddTHH:mm:ss.SSSSSSZZ`) in
/// UTC or in the time zone on top of the stack.
fn iso8601(stack: &mut Stack) -> Result<()> {
    let (ts, zone) = pop_timestamp_and_zone(stack, "ISO8601")?;
    let (millis, sub) = split_millis(ts);
    let utc = utc_from_millis(millis)?;
    let (local, offset) = to_local(zone, utc);
    let text = format!(
        "{}{}.{:03}{:03}{}",
        joda_year(local.year()),
        local.format("-%m-%dT%H:%M:%S"),
        local.and_utc().timestamp_subsec_millis(),
        sub,
        format_offset(offset, true)
    );
    stack.push(Value::from(text));
    Ok(())
}

/// Joda `yyyy`: at least four digits, a minus sign for negative years and no plus sign.
fn joda_year(year: i32) -> String {
    if year < 0 {
        format!("-{:04}", -i64::from(year))
    } else {
        format!("{year:04}")
    }
}

/// The components of a timestamp in `zone`: year, month, day, hour, minute, second, time
/// units within the second, day of year, day of week (1 = Monday) and ISO week.
fn ts_to_elements(ts: i64, zone: Zone) -> Result<Vec<Value>> {
    let (millis, sub) = split_millis(ts);
    let utc = utc_from_millis(millis)?;
    Ok(elements_of(utc, sub, zone))
}

/// The components of the UTC date-time `utc` (plus `sub` time units below the millisecond)
/// in `zone`.
fn elements_of(utc: NaiveDateTime, sub: i64, zone: Zone) -> Vec<Value> {
    let (local, _) = to_local(zone, utc);
    vec![
        Value::Long(i64::from(local.year())),
        Value::Long(i64::from(local.month())),
        Value::Long(i64::from(local.day())),
        Value::Long(i64::from(local.hour())),
        Value::Long(i64::from(local.minute())),
        Value::Long(i64::from(local.second())),
        Value::Long(i64::from(local.and_utc().timestamp_subsec_millis()) * TIME_UNITS_PER_MS + sub),
        Value::Long(i64::from(local.ordinal())),
        Value::Long(i64::from(local.weekday().number_from_monday())),
        Value::Long(i64::from(local.iso_week().week())),
    ]
}

/// `TSELEMENTS`: the components of a timestamp (see [`ts_to_elements`]).
fn tselements_impl(stack: &mut Stack, name: &str) -> Result<()> {
    let (ts, zone) = pop_timestamp_and_zone(stack, name)?;
    let elements = ts_to_elements(ts, zone)?;
    stack.push(Value::list(elements));
    Ok(())
}

fn tselements(stack: &mut Stack) -> Result<()> {
    tselements_impl(stack, "TSELEMENTS")
}

fn totselements(stack: &mut Stack) -> Result<()> {
    tselements_impl(stack, "->TSELEMENTS")
}

/// `IllegalFieldValueException` text.
fn field_error(value: i32, field: &str, min: i32, max: i32, suffix: &str) -> Error {
    TimeError::FieldOutOfRange {
        value,
        field: field.to_string(),
        min,
        max,
        suffix: suffix.to_string(),
    }
    .into()
}

/// Number of days in `month` of `year` (ISO chronology).
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// `new DateTime(year, month, day, hour, minute, second, zone)` as a UTC date-time, with
/// Joda's field validation messages.
fn datetime_from_fields(fields: [i32; 6], zone: Zone) -> Result<NaiveDateTime> {
    let [year, month, day, hour, minute, second] = fields;
    if !(1..=12).contains(&month) {
        return Err(field_error(month, "monthOfYear", 1, 12, ""));
    }
    let max_day = days_in_month(year, month as u32) as i32;
    if !(1..=max_day).contains(&day) {
        return Err(field_error(
            day,
            "dayOfMonth",
            1,
            max_day,
            &format!(": year: {year} month: {month}"),
        ));
    }
    if !(0..=23).contains(&hour) {
        return Err(field_error(hour, "hourOfDay", 0, 23, ""));
    }
    if !(0..=59).contains(&minute) {
        return Err(field_error(minute, "minuteOfHour", 0, 59, ""));
    }
    if !(0..=59).contains(&second) {
        return Err(field_error(second, "secondOfMinute", 0, 59, ""));
    }
    let local = NaiveDate::from_ymd_opt(year, month as u32, day as u32)
        .and_then(|date| date.and_hms_opt(hour as u32, minute as u32, second as u32))
        .ok_or_else(|| field_error(year, "year", -262_143, 262_142, ""))?;
    zone.local_to_utc_strict(local)
}

/// Java `((Number) o).intValue()` of a list element, or the Java exception texts.
fn int_element(elements: &[Value], index: usize) -> Result<i32> {
    let value = elements.get(index).ok_or(BoundsError::ListIndex {
        index: index as i64,
        length: elements.len(),
    })?;
    int_value(value).ok_or_else(|| Error::class_cast(value.kind(), Class::Number))
}

/// `TSELEMENTS->`: the timestamp of a list of components (year, month, day, hour, minute,
/// second, time units within the second; missing components default to the epoch's), in UTC
/// or in the time zone on top of the stack.
fn fromtselements(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "TSELEMENTS->";
    const MESSAGE: &str =
        "operates on a list of timestamp elements or a list of timestamp elements + timezone.";
    let mut top = stack.pop()?;
    let mut tz = None;
    if let Value::String(id) = &top {
        tz = Some(id.to_string());
        top = stack.pop()?;
    }
    let Value::List(elements) = top else {
        return Err(fail(NAME, MESSAGE));
    };
    let zone = Zone::for_id(tz.as_deref())?;
    let ts = elements_to_ts(&elements.borrow(), zone)?;
    stack.push(Value::Long(ts));
    Ok(())
}

/// The timestamp of a list of components in `zone` (see [`fromtselements`]).
fn elements_to_ts(elements: &[Value], zone: Zone) -> Result<i64> {
    let mut fields = [1970, 1, 1, 0, 0, 0];
    for (index, field) in fields.iter_mut().enumerate() {
        if index < elements.len() {
            *field = int_element(elements, index)?;
        }
    }
    let utc = datetime_from_fields(fields, zone)?;
    let mut ts = millis_of(utc).div_euclid(1000) * TIME_UNITS_PER_S;
    if elements.len() > 6 {
        ts += i64::from(int_element(elements, 6)?) % TIME_UNITS_PER_S;
    }
    Ok(ts)
}

// -----------------------------------------------------------------------------------------
// ADDDAYS / ADDMONTHS / ADDYEARS
// -----------------------------------------------------------------------------------------

/// What `ADDDAYS` & co. operate on.
enum Target {
    /// A timestamp in a time zone.
    Instant(i64, Zone),
    /// A list of time stamp elements.
    Elements(Vec<Value>),
}

/// Pop the operand of `ADDDAYS` & co.: a tselements list, a timestamp, or a timestamp below a
/// time zone.
fn pop_target(stack: &mut Stack, name: &str) -> Result<Target> {
    const MESSAGE: &str = "operates on a tselements list, timestamp, or timestamp and timezone.";
    match stack.pop()? {
        Value::String(tz) => {
            let Value::Long(ts) = stack.pop()? else {
                return Err(fail(name, MESSAGE));
            };
            Ok(Target::Instant(ts, Zone::for_id(Some(&tz))?))
        }
        Value::Long(ts) => Ok(Target::Instant(ts, Zone::UTC)),
        Value::List(list) => Ok(Target::Elements(list.borrow().clone())),
        _ => Err(fail(name, MESSAGE)),
    }
}

/// Apply `step` (a Joda `plusXxx` on the local date) to a timestamp in `zone`, keeping the
/// sub-millisecond part like Warp 10 (`instant % TIME_UNITS_PER_MS`, which is negative for
/// negative instants).
fn add_to_instant(
    ts: i64,
    zone: Zone,
    step: impl FnOnce(NaiveDateTime) -> Option<NaiveDateTime>,
) -> Result<i64> {
    let utc = utc_from_millis(ts / TIME_UNITS_PER_MS)?;
    let (local, _) = to_local(zone, utc);
    let shifted = step(local).ok_or(TimeError::CalculationOverflow)?;
    let result = zone.local_to_utc(shifted);
    Ok(millis_of(result) * TIME_UNITS_PER_MS + ts % TIME_UNITS_PER_MS)
}

/// Joda `plusDays`.
fn plus_days(local: NaiveDateTime, days: i32) -> Option<NaiveDateTime> {
    if days >= 0 {
        local.checked_add_days(Days::new(days as u64))
    } else {
        local.checked_sub_days(Days::new(days.unsigned_abs() as u64))
    }
}

/// Joda `plusMonths` (the day of month is clamped to the end of the target month).
fn plus_months(local: NaiveDateTime, months: i32) -> Option<NaiveDateTime> {
    if months >= 0 {
        local.checked_add_months(Months::new(months as u32))
    } else {
        local.checked_sub_months(Months::new(months.unsigned_abs()))
    }
}

/// `ADDDAYS` / `ADDMONTHS` / `ADDYEARS` on a tselements list: the Warp 10 field arithmetic.
fn add_to_elements(mut elements: Vec<Value>, unit: Unit, count: i32) -> Result<Value> {
    let mut year = int_element(&elements, 0)?;
    let mut month = int_element(&elements, 1)?;
    match unit {
        Unit::Days => {
            let mut day = int_element(&elements, 2)?;
            let mut days = count;
            while days < 0 {
                days += 1;
                day -= 1;
                if day < 1 {
                    month -= 1;
                    if month < 1 {
                        year -= 1;
                        month = 12;
                    }
                    day = days_in_month(year, month as u32) as i32;
                }
            }
            while days > 0 {
                days -= 1;
                day += 1;
                if (1..=12).contains(&month) && day > days_in_month(year, month as u32) as i32 {
                    month += 1;
                    day = 1;
                }
                if month > 12 {
                    month = 1;
                    year += 1;
                }
            }
            elements[2] = Value::Long(i64::from(day));
        }
        Unit::Months => {
            let total = i64::from(month) - 1 + i64::from(count);
            year += total.div_euclid(12) as i32;
            month = total.rem_euclid(12) as i32 + 1;
        }
        Unit::Years => year += count,
    }
    elements[0] = Value::Long(i64::from(year));
    elements[1] = Value::Long(i64::from(month));
    // `ADDMONTHS` / `ADDYEARS` clamp the day to the length of the month.
    if unit != Unit::Days && elements.len() > 2 {
        let day = int_element(&elements, 2)?;
        let max = days_in_month(year, month as u32) as i32;
        if day > max && (month == 2 || [4, 6, 9, 11].contains(&month)) {
            elements[2] = Value::Long(i64::from(max));
        }
    }
    Ok(Value::list(elements))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Unit {
    Days,
    Months,
    Years,
}

fn add_unit(stack: &mut Stack, name: &str, unit: Unit, what: &str) -> Result<()> {
    let Value::Long(count) = stack.pop()? else {
        return Err(fail(
            name,
            &format!("expects a number of {what} on top of the stack."),
        ));
    };
    let count = count as i32;
    let result = match pop_target(stack, name)? {
        Target::Instant(ts, zone) => Value::Long(add_to_instant(ts, zone, |local| match unit {
            Unit::Days => plus_days(local, count),
            Unit::Months => plus_months(local, count),
            Unit::Years => plus_months(local, count.saturating_mul(12)),
        })?),
        Target::Elements(elements) => add_to_elements(elements, unit, count)?,
    };
    stack.push(result);
    Ok(())
}

fn adddays(stack: &mut Stack) -> Result<()> {
    add_unit(stack, "ADDDAYS", Unit::Days, "days")
}

fn addmonths(stack: &mut Stack) -> Result<()> {
    add_unit(stack, "ADDMONTHS", Unit::Months, "months")
}

fn addyears(stack: &mut Stack) -> Result<()> {
    add_unit(stack, "ADDYEARS", Unit::Years, "years")
}

// -----------------------------------------------------------------------------------------
// ISO 8601 durations (`org.joda.time.format.ISOPeriodFormat.standard()`)
// -----------------------------------------------------------------------------------------

/// A Joda `Period` of the standard type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Period {
    /// Years.
    pub years: i64,
    /// Months.
    pub months: i64,
    /// Weeks.
    pub weeks: i64,
    /// Days.
    pub days: i64,
    /// Hours.
    pub hours: i64,
    /// Minutes.
    pub minutes: i64,
    /// Seconds.
    pub seconds: i64,
    /// Milliseconds.
    pub millis: i64,
}

impl Period {
    /// `Period.toDurationFrom` for a period without years or months: the exact length in
    /// milliseconds.
    fn millis(&self) -> i64 {
        (((self.weeks * 7 + self.days) * 24 + self.hours) * 60 + self.minutes) * 60_000
            + self.seconds * 1000
            + self.millis
    }
}

/// Parse a signed integer field at `pos`; returns the value and the position after it
/// (`None` when there is no digit or the value overflows).
fn parse_signed(text: &str, pos: usize) -> Option<(i64, usize)> {
    let bytes = text.as_bytes();
    let mut end = pos;
    let negative = matches!(bytes.get(end), Some(b'-'));
    if matches!(bytes.get(end), Some(b'-' | b'+')) {
        end += 1;
    }
    let digits_start = end;
    while matches!(bytes.get(end), Some(b) if b.is_ascii_digit()) {
        end += 1;
    }
    if end == digits_start {
        return None;
    }
    let value = bytes[digits_start..end].iter().try_fold(0i64, |acc, &b| {
        acc.checked_mul(10)?.checked_add(i64::from(b - b'0'))
    })?;
    Some((if negative { -value } else { value }, end))
}

/// `ISOPeriodFormat.standard().getParser().parseInto`: parse `PnYnMnWnDTnHnMnS` into
/// `period`, returning the position reached, or the bitwise complement of the failure
/// position (Joda's convention). Fields parsed before a failure are kept.
pub fn parse_period(text: &str, period: &mut Period) -> i32 {
    let bytes = text.as_bytes();
    if !matches!(bytes.first(), Some(b'P')) {
        return !0;
    }
    let mut pos = 1;
    let field = |pos: &mut usize, suffix: u8| -> Option<i64> {
        let (value, end) = parse_signed(text, *pos)?;
        if bytes.get(end) == Some(&suffix) {
            *pos = end + 1;
            Some(value)
        } else {
            None
        }
    };
    if let Some(v) = field(&mut pos, b'Y') {
        period.years = v;
    }
    if let Some(v) = field(&mut pos, b'M') {
        period.months = v;
    }
    if let Some(v) = field(&mut pos, b'W') {
        period.weeks = v;
    }
    if let Some(v) = field(&mut pos, b'D') {
        period.days = v;
    }
    if bytes.get(pos) != Some(&b'T') {
        return pos as i32;
    }
    pos += 1;
    let after_t = pos;
    if let Some(v) = field(&mut pos, b'H') {
        period.hours = v;
    }
    if let Some(v) = field(&mut pos, b'M') {
        period.minutes = v;
    }
    // Seconds with an optional fraction of up to three digits (`appendSecondsWithOptionalMillis`).
    if let Some((seconds, end)) = parse_signed(text, pos) {
        let mut end = end;
        let mut millis = 0;
        if matches!(bytes.get(end), Some(b'.' | b',')) {
            let fraction_start = end + 1;
            let mut fraction_end = fraction_start;
            while matches!(bytes.get(fraction_end), Some(b) if b.is_ascii_digit()) {
                fraction_end += 1;
            }
            let digits = &text[fraction_start..(fraction_start + 3).min(fraction_end)];
            millis = decimal_value(&format!("{digits:0<3}"));
            if seconds < 0 || text[pos..end].starts_with('-') {
                millis = -millis;
            }
            end = fraction_end;
        }
        if bytes.get(end) == Some(&b'S') {
            period.seconds = seconds;
            period.millis = millis;
            pos = end + 1;
        }
    }
    if pos == after_t {
        return !(after_t as i32);
    }
    pos as i32
}

/// `DURATION.parseDuration`: the length in time units of an ISO 8601 duration (with an
/// optional fraction of seconds), which may not contain years or months. The whole period
/// may be prefixed by `-`.
pub fn parse_duration(text: &str) -> Result<i64> {
    let (sign, unsigned) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text),
    };
    let tokens: Vec<&str> = unsigned.split('.').collect();
    if tokens.len() > 2 {
        return Err(TimeError::InvalidIsoDuration.into());
    }
    let mut subseconds = 0i64;
    let head = tokens[0];
    let mut period_text = unsigned.to_owned();
    if tokens.len() == 2 {
        let fraction = &tokens[1][..tokens[1].len().saturating_sub(1)];
        period_text = format!("{head}S");
        let value: f64 = format!("0.{fraction}")
            .parse()
            .map_err(|_| NumberFormatError::InvalidInput(format!("0.{fraction}")))?;
        subseconds = (value * TIME_UNITS_PER_S as f64) as i64;
    }
    let mut period = Period::default();
    parse_period(&period_text, &mut period);
    if period.months != 0 || period.years != 0 {
        return Err(TimeError::AmbiguousDuration.into());
    }
    let negative_subseconds = if period.seconds < 0 {
        true
    } else if period.seconds == 0 && subseconds != 0 {
        let trimmed = head.trim_end_matches('0');
        trimmed.len() < head.len() && trimmed.ends_with('-')
    } else {
        false
    };
    if negative_subseconds {
        subseconds = -subseconds;
    }
    Ok(sign * (period.millis() * TIME_UNITS_PER_MS + subseconds))
}

/// `DURATION`: the number of time units of an ISO 8601 duration.
fn duration(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "DURATION";
    let Value::String(text) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects an ISO8601 duration (a string) on top of the stack. See http://en.wikipedia.org/wiki/ISO_8601#Durations",
        ));
    };
    let value = parse_duration(&text).map_err(|err| {
        fail_with_cause(NAME, "encountered an error while parsing duration.", err)
    })?;
    stack.push(Value::Long(value));
    Ok(())
}

/// `ISODURATION`: the ISO 8601 representation of a number of time units, split into hours,
/// minutes and seconds like `new MutablePeriod(millis)` (`PT25H1M1.001S`, `PT0S`).
fn isoduration(stack: &mut Stack) -> Result<()> {
    let Value::Long(duration) = stack.pop()? else {
        return Err(fail(
            "ISODURATION",
            "expects a number of time units (LONG) on top of the stack.",
        ));
    };
    let millis = duration / TIME_UNITS_PER_MS;
    let hours = millis / 3_600_000;
    let minutes = (millis % 3_600_000) / 60_000;
    let seconds = (millis % 60_000) / 1000;
    let rest = millis % 1000;
    let mut out = String::from("PT");
    if hours != 0 {
        out.push_str(&format!("{hours}H"));
    }
    if minutes != 0 {
        out.push_str(&format!("{minutes}M"));
    }
    if seconds != 0 || rest != 0 || (hours == 0 && minutes == 0) {
        if rest == 0 {
            out.push_str(&format!("{seconds}S"));
        } else {
            let sign = if seconds < 0 || rest < 0 { "-" } else { "" };
            out.push_str(&format!("{sign}{}.{:03}S", seconds.abs(), rest.abs()));
        }
    }
    stack.push(Value::from(out));
    Ok(())
}

/// `HUMANDURATION`: a duration as `[-][Nd][Nh][Nm]S.ffffffs`.
fn humanduration(stack: &mut Stack) -> Result<()> {
    let Value::Long(mut duration) = stack.pop()? else {
        return Err(fail(
            "HUMANDURATION",
            "expects a number of time units (LONG) on top of the stack.",
        ));
    };
    let mut out = String::new();
    let days = duration / (TIME_UNITS_PER_S * 86_400);
    if duration < 0 {
        out.push('-');
    }
    if days.abs() > 0 {
        out.push_str(&format!("{}d", days.abs()));
        duration -= days * (TIME_UNITS_PER_S * 86_400);
    }
    let hours = duration / (TIME_UNITS_PER_S * 3600);
    if hours.abs() > 0 || !out.is_empty() {
        out.push_str(&format!("{}h", hours.abs()));
        duration -= hours * (TIME_UNITS_PER_S * 3600);
    }
    let minutes = duration / (TIME_UNITS_PER_S * 60);
    if minutes.abs() > 0 || !out.is_empty() {
        out.push_str(&format!("{}m", minutes.abs()));
        duration -= minutes * (TIME_UNITS_PER_S * 60);
    }
    let seconds = duration / TIME_UNITS_PER_S;
    duration -= seconds * TIME_UNITS_PER_S;
    let padded = (TIME_UNITS_PER_S + duration.abs()).to_string();
    out.push_str(&format!("{}.{}s", seconds.abs(), &padded[1..]));
    stack.push(Value::from(out));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// ADDDURATION
// -----------------------------------------------------------------------------------------

/// `ADDDURATION.durationToPeriod`: a period and its sub-second offset in time units.
pub(crate) fn duration_to_period(text: &str) -> Result<(Period, i64)> {
    let tokens: Vec<&str> = text.split('.').collect();
    if tokens.len() > 2 {
        return Err(TimeError::InvalidDuration.into());
    }
    let mut offset = 0;
    let mut text = text.to_owned();
    if tokens.len() == 2 {
        let fraction = &tokens[1][..tokens[1].len().saturating_sub(1)];
        text = format!("{}S", tokens[0]);
        let value: f64 = format!("0.{fraction}").parse().map_err(|_| {
            Error::with_cause(
                format!(
                    "Parsing of sub second precision part of duration has failed. tried to parse: {fraction}"
                ),
                NumberFormatError::InvalidInput(format!("0.{fraction}")).into(),
            )
        })?;
        offset = (value * TIME_UNITS_PER_S as f64) as i64;
    }
    let mut period = Period::default();
    if parse_period(&text, &mut period) < 0 {
        return Err(TimeError::UnparsableDuration(text.clone()).into());
    }
    Ok((period, offset))
}

/// A Joda `plusXxx` on a local date-time.
type DateStep = fn(NaiveDateTime, i32) -> Option<NaiveDateTime>;

/// `ADDDURATION.addPeriod`: add `n` times a period (and its sub-second offset) to a timestamp
/// in `zone`, field by field like Joda's `withPeriodAdded`: years, months, weeks and days are
/// added to the local date (each converted back with the overlap / gap rules), hours and
/// smaller are exact durations added to the instant.
pub fn add_period(ts: i64, period: &Period, offset: i64, zone: Zone, n: i64) -> Result<i64> {
    let (result, sub) = add_period_utc(ts, period, offset, zone, n)?;
    Ok(millis_of(result) * TIME_UNITS_PER_MS + sub)
}

/// [`add_period`] returning the UTC date-time and the time units below the millisecond.
fn add_period_utc(
    ts: i64,
    period: &Period,
    offset: i64,
    zone: Zone,
    n: i64,
) -> Result<(NaiveDateTime, i64)> {
    let overflow = || TimeError::CalculationOverflow;
    let scaled = |value: i64| -> core::result::Result<i32, TimeError> {
        value
            .checked_mul(n)
            .ok_or_else(overflow)
            .and_then(|v| i32::try_from(v).map_err(|_| overflow()))
    };
    let mut utc = utc_from_millis(ts / TIME_UNITS_PER_MS)?;
    let date_steps: [(i64, DateStep); 4] = [
        (
            period.years.checked_mul(12).ok_or_else(overflow)?,
            plus_months,
        ),
        (period.months, plus_months),
        (period.weeks.checked_mul(7).ok_or_else(overflow)?, plus_days),
        (period.days, plus_days),
    ];
    for (amount, step) in date_steps {
        if amount == 0 {
            continue;
        }
        let (local, _) = to_local(zone, utc);
        let shifted = step(local, scaled(amount)?).ok_or_else(overflow)?;
        utc = zone.local_to_utc(shifted);
    }
    let millis = period
        .hours
        .checked_mul(3_600_000)
        .and_then(|h| h.checked_add(period.minutes.checked_mul(60_000)?))
        .and_then(|h| h.checked_add(period.seconds.checked_mul(1000)?))
        .and_then(|h| h.checked_add(period.millis))
        .and_then(|h| h.checked_mul(n))
        .ok_or_else(overflow)?;
    let result = chrono::TimeDelta::try_milliseconds(millis)
        .and_then(|delta| utc.checked_add_signed(delta))
        .ok_or_else(overflow)?;
    let offset = if period.seconds < 0 { -offset } else { offset };
    Ok((result, ts % TIME_UNITS_PER_MS + offset * n))
}

/// `ADDDURATION`: add an ISO 8601 duration (optionally `N` times) to a timestamp (in UTC or
/// a time zone) or to a tselements list.
fn addduration(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ADDDURATION";
    const OPERATES: &str = "operates on a tselements list, timestamp, or timestamp and timezone.";
    let mut n = 1i64;
    let text = match stack.pop()? {
        Value::String(text) => text,
        Value::Long(count) => {
            n = count;
            let Value::String(text) = stack.pop()? else {
                return Err(fail(
                    NAME,
                    "expects an ISO8601 duration (a string) in the second level of the stack (see http://en.wikipedia.org/wiki/ISO_8601#Durations).",
                ));
            };
            text
        }
        _ => {
            return Err(fail(
                NAME,
                "expects an ISO8601 duration (a string) on top of the stack (see http://en.wikipedia.org/wiki/ISO_8601#Durations), or a number of durations (a Long).",
            ));
        }
    };
    let (tz, target) = match stack.pop()? {
        Value::String(id) => match stack.pop()? {
            Value::Long(ts) => (Some(id), Target::Instant(ts, Zone::UTC)),
            _ => return Err(fail(NAME, OPERATES)),
        },
        Value::List(list) => (None, Target::Elements(list.borrow().clone())),
        Value::Long(ts) => (None, Target::Instant(ts, Zone::UTC)),
        _ => return Err(fail(NAME, OPERATES)),
    };
    let zone = Zone::for_id(tz.as_deref())?;
    let (period, offset) = duration_to_period(&text)
        .map_err(|err| fail_with_cause(NAME, "encountered an exception.", err))?;
    let result = match target {
        // The elements are converted in UTC, like Warp 10 (the zone only applies to the
        // period arithmetic).
        Target::Elements(elements) => {
            let ts = elements_to_ts(&elements, Zone::UTC)?;
            let (shifted, sub) = add_period_utc(ts, &period, offset, zone, n)?;
            // Like `TSELEMENTS` on `shifted * 1000 + sub`: the sub-millisecond part is folded
            // into the milliseconds first.
            let (millis, sub) = split_millis(millis_of(shifted) * TIME_UNITS_PER_MS + sub);
            Value::list(elements_of(
                utc_from_millis(millis).unwrap_or(shifted),
                sub,
                Zone::UTC,
            ))
        }
        Target::Instant(ts, _) => Value::Long(add_period(ts, &period, offset, zone, n)?),
    };
    stack.push(result);
    Ok(())
}

// -----------------------------------------------------------------------------------------
// TOTIMESTAMP / NOTBEFORE / NOTAFTER (`java.time.ZonedDateTime.parse`)
// -----------------------------------------------------------------------------------------

/// `DateTimeParseException` for a failure at `index`.
fn unparsable(text: &str, index: usize) -> Error {
    TimeError::Unparsable {
        text: text.to_string(),
        index,
    }
    .into()
}

/// Read exactly `n` digits at `pos`: `None` when they are not all digits or their value does not
/// fit a `u32`.
pub(super) fn digits(text: &str, pos: usize, n: usize) -> Option<u32> {
    let slice = text.get(pos..pos + n)?;
    if slice.len() == n && slice.bytes().all(|b| b.is_ascii_digit()) {
        slice.bytes().try_fold(0u32, |acc, b| {
            acc.checked_mul(10)?.checked_add(u32::from(b - b'0'))
        })
    } else {
        None
    }
}

/// `ZonedDateTime.parse` → time units since the epoch (`TOTIMESTAMP.parseTimestamp`).
pub fn parse_timestamp(text: &str) -> Result<i64> {
    let error = |index: usize| unparsable(text, index);
    let invalid = |detail: String| {
        TimeError::UnparsableBecause {
            text: text.to_string(),
            detail,
        }
        .into()
    };
    let bytes = text.as_bytes();
    // Year: 4 digits, or a sign followed by at least 4 digits.
    let mut pos = 0;
    let signed = matches!(bytes.first(), Some(b'+' | b'-'));
    if signed {
        pos += 1;
    }
    let year_start = pos;
    while matches!(bytes.get(pos), Some(b) if b.is_ascii_digit()) {
        pos += 1;
    }
    let year_digits = pos - year_start;
    if year_digits < 4 || (!signed && year_digits > 4) {
        return Err(error(if year_digits > 4 { 0 } else { year_start }));
    }
    let year: i64 = text[year_start..pos]
        .parse()
        .map_err(|_| error(year_start))?;
    let year = if bytes.first() == Some(&b'-') {
        -year
    } else {
        year
    };
    let year = i32::try_from(year).map_err(|_| error(year_start))?;
    let expect = |pos: usize, chars: &[u8]| -> Result<()> {
        match bytes.get(pos) {
            Some(b) if chars.contains(b) => Ok(()),
            _ => Err(error(pos)),
        }
    };
    expect(pos, b"-")?;
    let month = digits(text, pos + 1, 2).ok_or_else(|| error(pos + 1))?;
    pos += 3;
    expect(pos, b"-")?;
    let day = digits(text, pos + 1, 2).ok_or_else(|| error(pos + 1))?;
    pos += 3;
    expect(pos, b"Tt")?;
    let hour = digits(text, pos + 1, 2).ok_or_else(|| error(pos + 1))?;
    pos += 3;
    expect(pos, b":")?;
    let minute = digits(text, pos + 1, 2).ok_or_else(|| error(pos + 1))?;
    pos += 3;
    let mut second = 0;
    let mut nanos: u32 = 0;
    if bytes.get(pos) == Some(&b':') {
        second = digits(text, pos + 1, 2).ok_or_else(|| error(pos + 1))?;
        pos += 3;
        if bytes.get(pos) == Some(&b'.') {
            pos += 1;
            let start = pos;
            while matches!(bytes.get(pos), Some(b) if b.is_ascii_digit()) {
                pos += 1;
            }
            if pos - start > 9 {
                return Err(error(start + 9));
            }
            if pos > start {
                nanos = format!("{:0<9}", &text[start..pos])
                    .parse()
                    .unwrap_or_default();
            }
        }
    }
    // Offset: `Z`, `+HH`, `+HH:MM` or `+HH:MM:SS`.
    let offset_seconds: i32 = match bytes.get(pos) {
        Some(b'Z' | b'z') => {
            pos += 1;
            0
        }
        Some(sign @ (b'+' | b'-')) => {
            let hours = digits(text, pos + 1, 2).ok_or_else(|| error(pos + 1))?;
            pos += 3;
            let mut minutes = 0;
            let mut seconds = 0;
            if bytes.get(pos) == Some(&b':')
                && let Some(m) = digits(text, pos + 1, 2)
            {
                minutes = m;
                pos += 3;
                if bytes.get(pos) == Some(&b':')
                    && let Some(s) = digits(text, pos + 1, 2)
                {
                    seconds = s;
                    pos += 3;
                }
            }
            if hours > 23 || minutes > 59 || seconds > 59 {
                return Err(invalid(
                    "Value out of range: Hour[0-23], Minute[0-59], Second[0-59]".to_owned(),
                ));
            }
            let total = (hours * 3600 + minutes * 60 + seconds) as i32;
            if total > 18 * 3600 {
                return Err(invalid(
                    "Zone offset not in valid range: -18:00 to +18:00".to_owned(),
                ));
            }
            if *sign == b'-' { -total } else { total }
        }
        _ => return Err(error(pos)),
    };
    // Optional region: `[Europe/Paris]`.
    if bytes.get(pos) == Some(&b'[')
        && let Some(end) = text[pos..].find(']')
    {
        let id = &text[pos + 1..pos + end];
        if Tz::from_str(id).is_err() {
            return Err(unparsed(text, pos));
        }
        pos += end + 1;
    }
    if pos != text.len() {
        return Err(unparsed(text, pos));
    }
    // Field validation.
    if !(1..=12).contains(&month) {
        return Err(invalid(format!(
            "Invalid value for MonthOfYear (valid values 1 - 12): {month}"
        )));
    }
    if !(1..=31).contains(&day) {
        return Err(invalid(format!(
            "Invalid value for DayOfMonth (valid values 1 - 28/31): {day}"
        )));
    }
    if hour > 23 {
        return Err(invalid(format!(
            "Invalid value for HourOfDay (valid values 0 - 23): {hour}"
        )));
    }
    if minute > 59 {
        return Err(invalid(format!(
            "Invalid value for MinuteOfHour (valid values 0 - 59): {minute}"
        )));
    }
    if second > 59 {
        return Err(invalid(format!(
            "Invalid value for SecondOfMinute (valid values 0 - 59): {second}"
        )));
    }
    // `chrono` stops at ±262143 where `java.time` goes to ±999999999.
    if NaiveDate::from_ymd_opt(year, 1, 1).is_none() {
        return Err(invalid(format!(
            "Invalid value for Year (valid values -262143 - 262142): {year}"
        )));
    }
    let date = NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| {
        invalid(format!(
            "Invalid date '{} {day}'",
            MONTH_NAMES.get(month as usize - 1).unwrap_or(&"")
        ))
    })?;
    // The time fields were validated above.
    let local = date
        .and_hms_nano_opt(hour, minute, second, nanos)
        .unwrap_or(NaiveDateTime::MIN);
    // The instant is the local date-time at the offset; a region only has to exist
    // (`ZonedDateTime.from` uses the instant when an offset is available).
    let utc = shift(local, -offset_seconds);
    let seconds = utc.and_utc().timestamp();
    Ok(seconds * TIME_UNITS_PER_S + i64::from(nanos) / (1_000_000 / TIME_UNITS_PER_MS))
}

const MONTH_NAMES: [&str; 12] = [
    "JANUARY",
    "FEBRUARY",
    "MARCH",
    "APRIL",
    "MAY",
    "JUNE",
    "JULY",
    "AUGUST",
    "SEPTEMBER",
    "OCTOBER",
    "NOVEMBER",
    "DECEMBER",
];

/// `DateTimeParseException` for trailing text.
fn unparsed(text: &str, index: usize) -> Error {
    TimeError::UnparsedText {
        text: text.to_string(),
        index,
    }
    .into()
}

/// `TOTIMESTAMP.parseTimestamp`: the timestamp of an ISO 8601 date-time string with a time
/// zone, the parse error wrapped in the message Warp 10 adds.
pub fn parse_iso8601(text: &str) -> Result<i64> {
    parse_timestamp(text).map_err(|err| {
        Error::with_cause(
            "Can only parse an ISO8601 timestamp (YYYY-MM-DDThh:mm:ss.sssssssss) with a specified time zone.",
            err,
        )
    })
}

/// `TOTIMESTAMP`: the timestamp of an ISO 8601 date-time string with a time zone.
fn totimestamp(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "TOTIMESTAMP";
    let Value::String(text) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects an ISO8601 timestamp or a Date instance on top of the stack.",
        ));
    };
    let ts = parse_iso8601(&text)?;
    stack.push(Value::Long(ts));
    Ok(())
}

/// `NOTBEFORE` / `NOTAFTER`: fail when the current time is before / after the timestamp (or
/// ISO 8601 string) on top of the stack.
fn not_before_after(stack: &mut Stack, name: &str, before: bool) -> Result<()> {
    let top = stack.pop()?;
    let instant = match &top {
        Value::String(text) => parse_iso8601(text)?,
        Value::Long(ts) => *ts,
        _ => {
            return Err(fail(
                name,
                "expects a timestamp or ISO8601 datetime string on top of the stack.",
            ));
        }
    };
    let now = current_time();
    if (before && now < instant) || (!before && now > instant) {
        return Err(TimeError::WrongTime {
            before,
            instant: top.to_string(),
        }
        .into());
    }
    Ok(())
}

fn notbefore(stack: &mut Stack) -> Result<()> {
    not_before_after(stack, "NOTBEFORE", true)
}

fn notafter(stack: &mut Stack) -> Result<()> {
    not_before_after(stack, "NOTAFTER", false)
}
