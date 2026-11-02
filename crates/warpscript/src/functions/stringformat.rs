//! `STRINGFORMAT`: `String.format(locale, format, args…)`, a port of `java.util.Formatter`
//! for the conversions WarpScript values support (`%b %h %s %c %d %o %x %e %f %g %a %% %n`
//! and their upper case forms), with Java's flags, width, precision, argument indexes,
//! rounding (half-up on the shortest decimal representation) and error messages.

use super::Entry;
use super::args::{fail, fail_with_cause};
use crate::error::{Error, Result};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[("STRINGFORMAT", Entry::Stack(stringformat))];

/// Locale-dependent symbols of `DecimalFormatSymbols` (JDK CLDR data) for the languages
/// WarpScript scripts commonly use; other locales use the root symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Locale {
    /// Decimal separator.
    pub decimal: char,
    /// Grouping separator.
    pub grouping: char,
}

impl Locale {
    /// `Locale.ROOT` (and English).
    pub const ROOT: Locale = Locale {
        decimal: '.',
        grouping: ',',
    };

    /// The symbols of the language of a BCP 47 / Java locale tag (`fr`, `fr_FR`, `de-DE`…),
    /// or `None` when the language is not an available locale.
    pub fn for_tag(tag: &str) -> Option<Locale> {
        let language = tag
            .split(['_', '-'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let symbols = match language.as_str() {
            "" | "en" | "ja" | "zh" | "ko" | "th" | "he" | "iw" | "hi" | "ms" | "id" | "in" => {
                Locale::ROOT
            }
            "fr" => Locale {
                decimal: ',',
                grouping: '\u{202F}',
            },
            "de" | "it" | "es" | "pt" | "nl" | "da" | "tr" | "el" | "ro" | "sl" | "hr" | "sr"
            | "ca" | "vi" => Locale {
                decimal: ',',
                grouping: '.',
            },
            "ru" | "sv" | "pl" | "fi" | "nb" | "no" | "nn" | "cs" | "hu" | "uk" | "bg" | "sk"
            | "lt" | "lv" | "et" => Locale {
                decimal: ',',
                grouping: '\u{00A0}',
            },
            _ => return None,
        };
        Some(symbols)
    }
}

/// The flags of a format specifier (`Formatter.Flags`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Flags {
    left_justify: bool,
    uppercase: bool,
    alternate: bool,
    plus: bool,
    leading_space: bool,
    zero_pad: bool,
    group: bool,
    parentheses: bool,
    previous: bool,
}

impl Flags {
    /// `Flags.toString()`: the flag characters in Java's order.
    fn to_java_string(self) -> String {
        let mut out = String::new();
        for (set, c) in [
            (self.left_justify, '-'),
            (self.uppercase, '^'),
            (self.alternate, '#'),
            (self.plus, '+'),
            (self.leading_space, ' '),
            (self.zero_pad, '0'),
            (self.group, ','),
            (self.parentheses, '('),
            (self.previous, '<'),
        ] {
            if set {
                out.push(c);
            }
        }
        out
    }

    /// Set the flag of `c`; `Err` on a duplicate (`DuplicateFormatFlagsException`).
    fn add(&mut self, c: char) -> Result<()> {
        let slot = match c {
            '-' => &mut self.left_justify,
            '#' => &mut self.alternate,
            '+' => &mut self.plus,
            ' ' => &mut self.leading_space,
            '0' => &mut self.zero_pad,
            ',' => &mut self.group,
            '(' => &mut self.parentheses,
            _ => &mut self.previous,
        };
        if *slot {
            return Err(Error::new(format!(
                "Flags = '{}'",
                Flags::default().with(c).to_java_string()
            )));
        }
        *slot = true;
        Ok(())
    }

    fn with(mut self, c: char) -> Flags {
        let _ = self.add(c);
        self
    }
}

/// One `%…` specifier.
#[derive(Debug, Clone)]
struct Spec {
    /// Explicit argument index (`2$`), 1-based.
    index: Option<usize>,
    flags: Flags,
    width: Option<usize>,
    precision: Option<usize>,
    /// Date/time conversion (`%tY`).
    date_time: bool,
    /// The conversion character, lower cased.
    conversion: char,
    /// The original specifier text (`FormatSpecifier.toString()`).
    text: String,
}

/// `FormatFlagsConversionMismatchException`.
fn mismatch(flag: char, conversion: char) -> Error {
    Error::new(format!("Conversion = {conversion}, Flags = {flag}"))
}

impl Spec {
    /// Reject the flags of `bad` (`checkBadFlags`).
    fn check_bad_flags(&self, bad: &[char]) -> Result<()> {
        for &flag in bad {
            let set = match flag {
                '#' => self.flags.alternate,
                '+' => self.flags.plus,
                ' ' => self.flags.leading_space,
                '0' => self.flags.zero_pad,
                ',' => self.flags.group,
                // Only `(` remains: `-` is validated separately.
                _ => self.flags.parentheses,
            };
            if set {
                return Err(mismatch(flag, self.conversion_char()));
            }
        }
        Ok(())
    }

    /// The conversion as written (upper case when the `UPPERCASE` flag is set).
    fn conversion_char(&self) -> char {
        if self.flags.uppercase {
            self.conversion.to_ascii_uppercase()
        } else {
            self.conversion
        }
    }

    /// `checkNumeric`.
    fn check_numeric(&self) -> Result<()> {
        if self.width.is_none() && (self.flags.left_justify || self.flags.zero_pad) {
            return Err(Error::new(self.text.clone()));
        }
        if (self.flags.plus && self.flags.leading_space)
            || (self.flags.left_justify && self.flags.zero_pad)
        {
            return Err(Error::new(format!(
                "Flags = '{}'",
                self.flags.to_java_string()
            )));
        }
        Ok(())
    }

    /// The validity checks of `FormatSpecifier`'s constructor.
    fn check(&self) -> Result<()> {
        if self.date_time {
            // Date/time conversions are rejected later with a dedicated message.
            return Ok(());
        }
        match self.conversion {
            'b' | 'h' | 's' => {
                if self.conversion != 's' && self.flags.alternate {
                    return Err(mismatch('#', self.conversion_char()));
                }
                if self.width.is_none() && self.flags.left_justify {
                    return Err(Error::new(self.text.clone()));
                }
                self.check_bad_flags(&['+', ' ', '0', ',', '('])
            }
            'c' => {
                if let Some(precision) = self.precision {
                    return Err(Error::new(precision.to_string()));
                }
                self.check_bad_flags(&['#', '+', ' ', '0', ',', '('])?;
                if self.width.is_none() && self.flags.left_justify {
                    return Err(Error::new(self.text.clone()));
                }
                Ok(())
            }
            'd' | 'o' | 'x' => {
                self.check_numeric()?;
                if let Some(precision) = self.precision {
                    return Err(Error::new(precision.to_string()));
                }
                if self.conversion == 'd' {
                    self.check_bad_flags(&['#'])
                } else {
                    self.check_bad_flags(&[','])
                }
            }
            'e' | 'f' | 'g' | 'a' => {
                self.check_numeric()?;
                match self.conversion {
                    'a' => self.check_bad_flags(&['(', ',']),
                    'e' => self.check_bad_flags(&[',']),
                    'g' => self.check_bad_flags(&['#']),
                    _ => Ok(()),
                }
            }
            '%' => {
                if let Some(precision) = self.precision {
                    return Err(Error::new(precision.to_string()));
                }
                let plain = Flags {
                    left_justify: false,
                    ..self.flags
                };
                if plain != Flags::default() {
                    return Err(Error::new(format!(
                        "Flags = '{}'",
                        self.flags.to_java_string()
                    )));
                }
                if self.width.is_none() && self.flags.left_justify {
                    return Err(Error::new(self.text.clone()));
                }
                Ok(())
            }
            // 'n'
            _ => {
                if let Some(precision) = self.precision {
                    return Err(Error::new(precision.to_string()));
                }
                if let Some(width) = self.width {
                    return Err(Error::new(width.to_string()));
                }
                if self.flags != Flags::default() {
                    return Err(Error::new(format!(
                        "Flags = '{}'",
                        self.flags.to_java_string()
                    )));
                }
                Ok(())
            }
        }
    }
}

/// Parse the specifier starting after the `%` at `start`; returns the specifier and the
/// position after it.
fn parse_spec(format: &str, start: usize) -> Result<(Spec, usize)> {
    let bytes = format.as_bytes();
    let mut pos = start;
    let digits = |pos: &mut usize| -> Option<usize> {
        let begin = *pos;
        while matches!(bytes.get(*pos), Some(b) if b.is_ascii_digit()) {
            *pos += 1;
        }
        if *pos > begin {
            format[begin..*pos].parse().ok()
        } else {
            None
        }
    };
    // Argument index or width: digits followed by `$` are an index.
    let mut index = None;
    let mut width = None;
    let mut probe = pos;
    if let Some(number) = digits(&mut probe)
        && bytes.get(probe) == Some(&b'$')
    {
        index = Some(number);
        pos = probe + 1;
    }
    let mut flags = Flags::default();
    while let Some(&c) = bytes.get(pos) {
        if b"-#+ 0,(<".contains(&c) {
            flags.add(char::from(c))?;
            pos += 1;
        } else {
            break;
        }
    }
    if index.is_none() {
        if let Some(number) = digits(&mut pos) {
            width = Some(number);
        }
    } else if let Some(number) = digits(&mut pos) {
        width = Some(number);
    }
    let mut precision = None;
    if bytes.get(pos) == Some(&b'.') {
        let mut after = pos + 1;
        match digits(&mut after) {
            Some(number) => {
                precision = Some(number);
                pos = after;
            }
            None => {
                return Err(Error::new(format!(
                    "Conversion = '{}'",
                    format[start..].chars().next().unwrap_or('%')
                )));
            }
        }
    }
    // An incomplete specifier is reported with the character following the `%`.
    let incomplete = || {
        Error::new(format!(
            "Conversion = '{}'",
            format[start..].chars().next().unwrap_or('%')
        ))
    };
    let mut date_time = false;
    let Some(&c) = bytes.get(pos) else {
        return Err(incomplete());
    };
    let mut conversion = char::from(c);
    if conversion == 't' || conversion == 'T' {
        date_time = true;
        pos += 1;
        conversion = format[pos..].chars().next().unwrap_or('%');
    }
    if !conversion.is_ascii_alphabetic() && conversion != '%' {
        return Err(Error::new(format!("Conversion = '{conversion}'")));
    }
    pos += 1;
    if conversion.is_ascii_uppercase() {
        flags.uppercase = true;
    }
    let text = format!("%{}", &format[start..pos]);
    let spec = Spec {
        index,
        flags,
        width,
        precision,
        date_time,
        conversion: conversion.to_ascii_lowercase(),
        text,
    };
    if !date_time && !"bhscdoxefga%n".contains(spec.conversion) {
        return Err(Error::new(format!(
            "Conversion = '{}'",
            spec.conversion_char()
        )));
    }
    Ok((spec, pos))
}

/// `String.format(locale, format, args)`; the error is the message of the
/// `IllegalFormatException` Java would throw.
pub fn java_format(locale: Locale, format: &str, args: &[Value]) -> Result<String> {
    let mut out = String::new();
    let mut pos = 0;
    let mut last_ordinary: Option<usize> = None;
    let mut last_index: Option<usize> = None;
    while let Some(offset) = format[pos..].find('%') {
        out.push_str(&format[pos..pos + offset]);
        let (spec, next) = parse_spec(format, pos + offset + 1)?;
        spec.check()?;
        pos = next;
        if spec.date_time {
            return Err(Error::new("Date/time conversions (%t) are not supported."));
        }
        match spec.conversion {
            '%' => {
                out.push_str(&justify("%", spec.width, spec.flags.left_justify));
                continue;
            }
            'n' => {
                out.push('\n');
                continue;
            }
            _ => {}
        }
        // Argument selection.
        let index = if spec.flags.previous {
            last_index.ok_or_else(|| Error::new(format!("Format specifier '{}'", spec.text)))?
        } else if let Some(explicit) = spec.index {
            explicit
                .checked_sub(1)
                .ok_or_else(|| Error::new("Illegal format argument index = 0"))?
        } else {
            let next = last_ordinary.map_or(0, |i| i + 1);
            last_ordinary = Some(next);
            next
        };
        last_index = Some(index);
        let arg = args
            .get(index)
            .ok_or_else(|| Error::new(format!("Format specifier '{}'", spec.text)))?;
        out.push_str(&format_arg(locale, &spec, arg)?);
    }
    out.push_str(&format[pos..]);
    Ok(out)
}

/// Pad `text` with spaces to `width` (counted in UTF-16 units like Java).
fn justify(text: &str, width: Option<usize>, left: bool) -> String {
    let length = text.encode_utf16().count();
    match width {
        Some(width) if width > length => {
            let padding = " ".repeat(width - length);
            if left {
                format!("{text}{padding}")
            } else {
                format!("{padding}{text}")
            }
        }
        _ => text.to_owned(),
    }
}

/// Java `hashCode()` of a value, for `%h`.
fn java_hash(value: &Value) -> i32 {
    match value {
        Value::Boolean(true) => 1231,
        Value::Boolean(false) => 1237,
        Value::Long(l) => (l ^ (l >> 32)) as i32,
        Value::Counter(c) => (c.get() ^ (c.get() >> 32)) as i32,
        Value::Double(d) => {
            let bits = if d.is_nan() {
                f64::NAN.to_bits()
            } else {
                d.to_bits()
            } as i64;
            (bits ^ (bits >> 32)) as i32
        }
        Value::String(s) => s
            .encode_utf16()
            .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(c))),
        Value::List(list) => list
            .borrow()
            .iter()
            .fold(1i32, |h, v| h.wrapping_mul(31).wrapping_add(java_hash(v))),
        Value::Map(map) => map.borrow().iter().fold(0i32, |h, (k, v)| {
            h.wrapping_add(java_hash(k) ^ java_hash(v))
        }),
        Value::Set(set) => set
            .borrow()
            .iter()
            .fold(0i32, |h, v| h.wrapping_add(java_hash(v))),
        // `null` hashes to 0; identity hash codes are not reproducible (they depend on the
        // JVM) and are reported as 0 too.
        _ => 0,
    }
}

/// `IllegalFormatConversionException`: `d != java.lang.Double`.
fn conversion_error(spec: &Spec, value: &Value) -> Error {
    Error::new(format!(
        "{} != {}",
        spec.conversion_char(),
        value.java_class()
    ))
}

/// Format one argument with `spec`.
fn format_arg(locale: Locale, spec: &Spec, arg: &Value) -> Result<String> {
    let flags = spec.flags;
    let text = match spec.conversion {
        'b' => match arg {
            Value::Null => "false".to_owned(),
            Value::Boolean(b) => b.to_string(),
            _ => "true".to_owned(),
        },
        'h' => match arg {
            Value::Null => "null".to_owned(),
            other => format!("{:x}", java_hash(other) as u32),
        },
        's' => {
            if flags.alternate {
                return Err(mismatch('#', spec.conversion_char()));
            }
            let text = match arg {
                Value::Null => "null".to_owned(),
                other => other.to_string(),
            };
            match spec.precision {
                Some(precision) => {
                    let units: Vec<u16> = text.encode_utf16().take(precision).collect();
                    String::from_utf16_lossy(&units)
                }
                None => text,
            }
        }
        'c' => match arg {
            Value::Null => "null".to_owned(),
            other => return Err(conversion_error(spec, other)),
        },
        'd' | 'o' | 'x' => match arg {
            Value::Null => "null".to_owned(),
            Value::Long(value) => format_integer(locale, spec, *value),
            other => return Err(conversion_error(spec, other)),
        },
        _ => match arg {
            Value::Null => "null".to_owned(),
            Value::Double(value) => format_double(locale, spec, *value),
            other => return Err(conversion_error(spec, other)),
        },
    };
    let text = if flags.uppercase {
        text.to_uppercase()
    } else {
        text
    };
    // Numbers with the zero pad flag were padded already.
    Ok(justify(&text, spec.width, flags.left_justify))
}

/// The sign (or opening parenthesis) in front of a number (`leadingSign`).
fn leading_sign(flags: Flags, negative: bool) -> &'static str {
    if negative {
        if flags.parentheses { "(" } else { "-" }
    } else if flags.plus {
        "+"
    } else if flags.leading_space {
        " "
    } else {
        ""
    }
}

/// `localizedMagnitude`: group the integer part of `magnitude` (digits with an optional
/// decimal point), localise the decimal separator and zero pad to `width` (including `sign`).
fn localized_magnitude(
    locale: Locale,
    flags: Flags,
    width: Option<usize>,
    sign: &str,
    magnitude: &str,
) -> String {
    let (integer, fraction) = match magnitude.find('.') {
        Some(dot) => (&magnitude[..dot], Some(&magnitude[dot + 1..])),
        None => (magnitude, None),
    };
    let mut body = String::new();
    if flags.group {
        let digits: Vec<char> = integer.chars().collect();
        for (i, c) in digits.iter().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                body.push(locale.grouping);
            }
            body.push(*c);
        }
    } else {
        body.push_str(integer);
    }
    if let Some(fraction) = fraction {
        body.push(locale.decimal);
        body.push_str(fraction);
    }
    let mut out = String::from(sign);
    if flags.zero_pad
        && let Some(width) = width
    {
        let width = if flags.parentheses && sign == "(" {
            width - 1
        } else {
            width
        };
        let length = out.len() + body.chars().count();
        if width > length {
            out.push_str(&"0".repeat(width - length));
        }
    }
    out.push_str(&body);
    out
}

/// `%d`, `%o`, `%x` of a `LONG`.
fn format_integer(locale: Locale, spec: &Spec, value: i64) -> String {
    let flags = spec.flags;
    match spec.conversion {
        'd' => {
            let negative = value < 0;
            let magnitude = value.unsigned_abs().to_string();
            let sign = leading_sign(flags, negative);
            let mut out = localized_magnitude(locale, flags, spec.width, sign, &magnitude);
            if negative && flags.parentheses {
                out.push(')');
            }
            out
        }
        radix => {
            let mut out = String::new();
            if flags.alternate {
                out.push_str(if radix == 'o' { "0" } else { "0x" });
            }
            let digits = if radix == 'o' {
                format!("{:o}", value as u64)
            } else {
                format!("{:x}", value as u64)
            };
            if flags.zero_pad
                && let Some(width) = spec.width
            {
                let length = out.len() + digits.len();
                if width > length {
                    out.push_str(&"0".repeat(width - length));
                }
            }
            out.push_str(&digits);
            out
        }
    }
}

/// The shortest decimal digits of a positive finite double and its decimal exponent
/// (`FloatingDecimal`: value = 0.d1d2… × 10^exp).
fn shortest_digits(value: f64) -> (Vec<u8>, i32) {
    let text = format!("{value:e}");
    let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
    let digits: Vec<u8> = mantissa
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect();
    let exponent: i32 = exponent.parse().unwrap_or_default();
    (digits, exponent + 1)
}

/// `FormattedFloatingDecimal.applyPrecision`: round `digits` half-up to `prec` digits;
/// returns the (possibly incremented) decimal exponent.
fn apply_precision(dec_exp: i32, digits: &mut [u8], prec: i32) -> i32 {
    let n = digits.len() as i32;
    if prec >= n || prec < 0 {
        return dec_exp;
    }
    if prec == 0 {
        if digits[0] >= 5 {
            digits[0] = 1;
            digits[1..].fill(0);
            return dec_exp + 1;
        }
        digits.fill(0);
        return dec_exp;
    }
    let prec = prec as usize;
    if digits[prec] >= 5 {
        let mut i = prec;
        loop {
            i -= 1;
            if digits[i] != 9 {
                digits[i] += 1;
                digits[i + 1..].fill(0);
                return dec_exp;
            }
            if i == 0 {
                digits[0] = 1;
                digits[1..].fill(0);
                return dec_exp + 1;
            }
        }
    }
    digits[prec..].fill(0);
    dec_exp
}

fn digit_chars(digits: &[u8]) -> String {
    digits.iter().map(|d| char::from(b'0' + d)).collect()
}

/// `fillDecimal`: the plain decimal mantissa with at most `precision` fraction digits.
fn fill_decimal(precision: i32, digits: &[u8], exp: i32) -> String {
    let n = digits.len() as i32;
    if exp > 0 {
        let exp = exp as usize;
        if n < exp as i32 {
            let mut out = digit_chars(digits);
            out.push_str(&"0".repeat(exp - digits.len()));
            out
        } else {
            let t = (n - exp as i32).min(precision);
            let mut out = digit_chars(&digits[..exp]);
            if t > 0 {
                out.push('.');
                out.push_str(&digit_chars(&digits[exp..exp + t as usize]));
            }
            out
        }
    } else {
        let zeros = (-exp).min(precision).max(0);
        let t = n.min(precision + exp).max(0);
        if zeros > 0 || t > 0 {
            let mut out = String::from("0.");
            out.push_str(&"0".repeat(zeros as usize));
            out.push_str(&digit_chars(&digits[..t as usize]));
            out
        } else {
            "0".to_owned()
        }
    }
}

/// `fillScientific`: the mantissa `d.ddd` with at most `precision` fraction digits and the
/// exponent text (`+03`).
fn fill_scientific(precision: i32, digits: &[u8], exp: i32) -> (String, String) {
    let t = (digits.len() as i32 - 1).min(precision).max(0) as usize;
    let mut mantissa = digit_chars(&digits[..1]);
    if t > 0 {
        mantissa.push('.');
        mantissa.push_str(&digit_chars(&digits[1..=t]));
    }
    let (sign, e) = if exp <= 0 {
        ('-', -exp + 1)
    } else {
        ('+', exp - 1)
    };
    (mantissa, format!("{sign}{e:02}"))
}

/// `addZeros`: pad the fraction of `mantissa` with zeros up to `prec` digits.
fn add_zeros(mantissa: &mut String, prec: i32) {
    let prec = prec.max(0) as usize;
    let existing = match mantissa.find('.') {
        Some(dot) => mantissa.len() - dot - 1,
        None => {
            if prec > 0 {
                mantissa.push('.');
            }
            0
        }
    };
    if prec > existing {
        mantissa.push_str(&"0".repeat(prec - existing));
    }
}

/// `%e`, `%f`, `%g`, `%a` of a `DOUBLE`.
fn format_double(locale: Locale, spec: &Spec, value: f64) -> String {
    let flags = spec.flags;
    if value.is_nan() {
        return "NaN".to_owned();
    }
    let negative = value.is_sign_negative();
    let sign = leading_sign(flags, negative);
    let magnitude = value.abs();
    let mut out = String::new();
    if magnitude.is_infinite() {
        out.push_str(sign);
        out.push_str("Infinity");
    } else {
        let precision = spec.precision.map_or(6, |p| p as i32);
        let (mut digits, dec_exp) = shortest_digits(magnitude);
        let body = match spec.conversion {
            'e' => {
                let exp = apply_precision(dec_exp, &mut digits, precision + 1);
                let (mut mantissa, exponent) = fill_scientific(precision, &digits, exp);
                add_zeros(&mut mantissa, precision);
                if flags.alternate && precision == 0 {
                    mantissa.push('.');
                }
                let mut text = localized_magnitude(
                    locale,
                    flags,
                    spec.width.map(|w| w.saturating_sub(exponent.len() + 1)),
                    sign,
                    &mantissa,
                );
                text.push('e');
                text.push_str(&exponent);
                text
            }
            'f' => {
                let exp = apply_precision(dec_exp, &mut digits, dec_exp + precision);
                let mut mantissa = fill_decimal(precision, &digits, exp);
                add_zeros(&mut mantissa, precision);
                if flags.alternate && precision == 0 {
                    mantissa.push('.');
                }
                localized_magnitude(locale, flags, spec.width, sign, &mantissa)
            }
            'g' => {
                let prec = match spec.precision {
                    None => 6,
                    Some(0) => 1,
                    Some(p) => p as i32,
                };
                if magnitude == 0.0 {
                    let mut mantissa = "0".to_owned();
                    add_zeros(&mut mantissa, prec - 1);
                    localized_magnitude(locale, flags, spec.width, sign, &mantissa)
                } else {
                    let exp = apply_precision(dec_exp, &mut digits, prec);
                    // Java: `exp - 1 < -4 || exp - 1 >= prec`.
                    if exp - 1 < -4 || exp > prec {
                        let (mut mantissa, exponent) = fill_scientific(prec - 1, &digits, exp);
                        add_zeros(&mut mantissa, prec - 1);
                        let mut text = localized_magnitude(
                            locale,
                            flags,
                            spec.width.map(|w| w.saturating_sub(exponent.len() + 1)),
                            sign,
                            &mantissa,
                        );
                        text.push('e');
                        text.push_str(&exponent);
                        text
                    } else {
                        let mut mantissa = fill_decimal(prec - exp, &digits, exp);
                        add_zeros(&mut mantissa, prec - exp);
                        localized_magnitude(locale, flags, spec.width, sign, &mantissa)
                    }
                }
            }
            // 'a': `Double.toHexString` style, without zero padding or precision handling.
            _ => {
                let mut text = String::from(sign);
                text.push_str(&hex_float(magnitude));
                text
            }
        };
        out.push_str(&body);
    }
    if negative && flags.parentheses {
        out.push(')');
    }
    out
}

/// `Double.toHexString` for a positive finite double (`%a`).
fn hex_float(value: f64) -> String {
    if value == 0.0 {
        return "0x0.0p0".to_owned();
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = bits & 0x000f_ffff_ffff_ffff;
    let (lead, exp) = if exponent == 0 {
        ('0', -1022)
    } else {
        ('1', exponent - 1023)
    };
    let hex = format!("{mantissa:013x}");
    let hex = hex.trim_end_matches('0');
    let hex = if hex.is_empty() { "0" } else { hex };
    format!("0x{lead}.{hex}p{exp}")
}

/// `STRINGFORMAT`: `String.format` of a format string with a list of arguments, optionally
/// in a locale (`'fr'`).
fn stringformat(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "STRINGFORMAT";
    let mut locale = Locale::ROOT;
    let args = match stack.pop()? {
        Value::List(list) => list,
        Value::String(tag) => {
            locale = Locale::for_tag(&tag).ok_or_else(|| fail(NAME, "expects a valid locale."))?;
            match stack.pop()? {
                Value::List(list) => list,
                _ => return Err(fail(NAME, "expects list of Objects under the locale.")),
            }
        }
        _ => {
            return Err(fail(
                NAME,
                "expects a locale name or a list of Objects as the first parameter.",
            ));
        }
    };
    let Value::String(format) = stack.pop()? else {
        return Err(fail(NAME, "expects format under the list of Objects."));
    };
    let formatted = java_format(locale, &format, &args.borrow())
        .map_err(|err| fail_with_cause(NAME, "expects a valid format.", err))?;
    stack.push(Value::from(formatted));
    Ok(())
}
