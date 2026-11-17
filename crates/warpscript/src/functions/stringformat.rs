// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Behaviour of `STRINGFORMAT` derived from Warp 10 (Copyright 2018-2025 SenX S.A.S., Apache-2.0).

//! `STRINGFORMAT`: `String.format(locale, format, args…)` for the conversions WarpScript
//! values support (`%b %h %s %c %d %o %x %e %f %g %a %% %n` and their upper case forms), with
//! Java's flags, width, precision, argument indexes, rounding (half-up on the shortest decimal
//! representation) and error messages.
//!
//! The implementation follows the published specification of `java.util.Formatter` (the
//! syntax of a specifier, the flags each conversion accepts, the number localisation
//! algorithm, the rounding rules) and was validated black-box against OpenJDK's
//! `String.format`; it does not reuse the JDK's code.

use std::iter::repeat_n;

use super::Entry;
use super::args::{fail, fail_with_cause};
use crate::error::format::FormatError;
use crate::error::{Error, Result};
use crate::format::decimal_value;
use crate::format::double_to_text;
use crate::position::position;
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

/// The flags of a format specifier. `^` is not written in a format string: it is the internal
/// flag of an upper case conversion (`%S`, `%X`…), which Java lists with the others.
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
    /// Every flag with its character, in the order Java prints them (`-^#+ 0,(<`).
    fn entries(self) -> [(bool, char); 9] {
        [
            (self.left_justify, '-'),
            (self.uppercase, '^'),
            (self.alternate, '#'),
            (self.plus, '+'),
            (self.leading_space, ' '),
            (self.zero_pad, '0'),
            (self.group, ','),
            (self.parentheses, '('),
            (self.previous, '<'),
        ]
    }

    /// The characters of the set flags, as Java prints them in error messages.
    fn flags_text(self) -> String {
        self.entries()
            .into_iter()
            .filter(|entry| entry.0)
            .map(|entry| entry.1)
            .collect()
    }

    /// Whether the flag written `c` is set.
    fn has(self, c: char) -> bool {
        self.entries().contains(&(true, c))
    }

    /// Set the flag written `c` (one of `-#+ 0,(<`); a flag given twice is an error
    /// (`DuplicateFormatFlagsException`).
    fn set(&mut self, c: char) -> Result<()> {
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
        if std::mem::replace(slot, true) {
            return Err(FormatError::DuplicateFlag(c).into());
        }
        Ok(())
    }
}

/// One `%…` specifier of a format string.
#[derive(Clone, Debug)]
struct Spec {
    /// Explicit argument index (`2$`), 1-based.
    index: Option<usize>,
    flags: Flags,
    width: Option<usize>,
    precision: Option<usize>,
    /// The conversion character, lower cased; `t` for a date/time conversion.
    conversion: char,
    /// The character following a date/time `t` / `T`, as written.
    suffix: Option<char>,
}

/// A validation step of a specifier. The steps of a conversion run in order and the first
/// failure is reported, which decides which of Java's `IllegalFormatException`s wins when
/// several apply.
#[derive(Debug, Clone, Copy)]
enum Rule {
    /// These flags do not apply to the conversion (`FormatFlagsConversionMismatchException`).
    Reject(&'static [char]),
    /// These flags need a width (`MissingFormatWidthException`).
    NeedWidth(&'static [char]),
    /// These pairs of flags cannot be combined (`IllegalFormatFlagsException`).
    Incompatible(&'static [[char; 2]]),
    /// No flag outside this list is allowed (`IllegalFormatFlagsException`).
    Only(&'static [char]),
    /// A precision is not allowed (`IllegalFormatPrecisionException`).
    NoPrecision,
    /// A width is not allowed (`IllegalFormatWidthException`).
    NoWidth,
    /// The suffix of a date/time conversion must be one Java knows
    /// (`UnknownFormatConversionException`).
    DateTime,
}

/// The width requirement shared by the numeric conversions.
const NUMERIC_WIDTH: Rule = Rule::NeedWidth(&['-', '0']);
/// The flag combinations the numeric conversions forbid.
const NUMERIC_FLAGS: Rule = Rule::Incompatible(&[['+', ' '], ['-', '0']]);

/// The validation steps of each conversion, in Java's order.
fn rules(conversion: char) -> &'static [Rule] {
    use Rule::{DateTime, NeedWidth, NoPrecision, NoWidth, Only, Reject};
    match conversion {
        'b' | 'h' => &[
            Reject(&['#']),
            NeedWidth(&['-']),
            Reject(&['+', ' ', '0', ',', '(']),
        ],
        's' => &[NeedWidth(&['-']), Reject(&['+', ' ', '0', ',', '('])],
        'c' => &[
            NoPrecision,
            Reject(&['#', '+', ' ', '0', ',', '(']),
            NeedWidth(&['-']),
        ],
        'd' => &[NUMERIC_WIDTH, NUMERIC_FLAGS, NoPrecision, Reject(&['#'])],
        'o' | 'x' => &[NUMERIC_WIDTH, NUMERIC_FLAGS, NoPrecision, Reject(&[','])],
        'e' => &[NUMERIC_WIDTH, NUMERIC_FLAGS, Reject(&[','])],
        'f' => &[NUMERIC_WIDTH, NUMERIC_FLAGS],
        'g' => &[NUMERIC_WIDTH, NUMERIC_FLAGS, Reject(&['#'])],
        'a' => &[NUMERIC_WIDTH, NUMERIC_FLAGS, Reject(&['(', ','])],
        '%' => &[NoPrecision, Only(&['-']), NeedWidth(&['-'])],
        'n' => &[NoPrecision, NoWidth, Only(&[])],
        // 't'
        _ => &[
            NoPrecision,
            DateTime,
            Reject(&['#', '+', ' ', '0', ',', '(']),
            NeedWidth(&['-']),
        ],
    }
}

/// `FormatFlagsConversionMismatchException`: `Conversion = s, Flags = #`.
fn mismatch(flag: char, conversion: char) -> Error {
    FormatError::FlagMismatch { conversion, flag }.into()
}

/// `IllegalFormatFlagsException`: `Flags = '+ '`.
fn illegal_flags(flags: Flags) -> Error {
    FormatError::IllegalFlags(flags.flags_text()).into()
}

impl Spec {
    /// The conversion named in error messages: the suffix of a date/time conversion.
    fn shown_conversion(&self) -> char {
        self.suffix.unwrap_or(self.conversion)
    }

    /// The specifier as Java prints it in error messages: `%`, the flags (without `^`), the
    /// argument index (unless `<` is set), the width, `.precision` and the conversion in the
    /// case it was written.
    fn text(&self) -> String {
        let mut text = String::from("%");
        text.extend(
            self.flags
                .entries()
                .into_iter()
                .filter(|&(set, c)| set && c != '^')
                .map(|(_, c)| c),
        );
        if let Some(index) = self.index.filter(|_| !self.flags.previous) {
            text.push_str(&format!("{index}$"));
        }
        if let Some(width) = self.width {
            text.push_str(&width.to_string());
        }
        if let Some(precision) = self.precision {
            text.push_str(&format!(".{precision}"));
        }
        for c in [Some(self.conversion), self.suffix].into_iter().flatten() {
            text.push(if self.flags.uppercase {
                c.to_ascii_uppercase()
            } else {
                c
            });
        }
        text
    }

    /// Run the validation steps of the conversion.
    fn check(&self) -> Result<()> {
        for rule in rules(self.conversion) {
            self.apply(*rule)?;
        }
        Ok(())
    }

    /// Apply one validation step.
    fn apply(&self, rule: Rule) -> Result<()> {
        let flags = self.flags;
        match rule {
            Rule::Reject(list) => match list.iter().find(|&&c| flags.has(c)) {
                Some(&flag) => Err(mismatch(flag, self.shown_conversion())),
                None => Ok(()),
            },
            Rule::NeedWidth(list) => {
                if self.width.is_none() && list.iter().any(|&c| flags.has(c)) {
                    Err(FormatError::MissingWidth(self.text()).into())
                } else {
                    Ok(())
                }
            }
            Rule::Incompatible(pairs) => {
                if pairs.iter().any(|[a, b]| flags.has(*a) && flags.has(*b)) {
                    Err(illegal_flags(flags))
                } else {
                    Ok(())
                }
            }
            Rule::Only(list) => {
                if flags
                    .entries()
                    .iter()
                    .any(|&(set, c)| set && !list.contains(&c))
                {
                    Err(illegal_flags(flags))
                } else {
                    Ok(())
                }
            }
            Rule::NoPrecision => match self.precision {
                Some(precision) => Err(FormatError::IllegalPrecision(precision).into()),
                None => Ok(()),
            },
            Rule::NoWidth => match self.width {
                Some(width) => Err(FormatError::IllegalWidth(width).into()),
                None => Ok(()),
            },
            Rule::DateTime => {
                let suffix = self.shown_conversion();
                if "HIklMSLNpzZsQBbhAaCYyjmdeRTrDFc".contains(suffix) {
                    Ok(())
                } else {
                    Err(FormatError::UnknownDateTimeConversion(suffix).into())
                }
            }
        }
    }
}

/// Parse the specifier that follows the `%` at `start`; returns it with the position after it.
///
/// A malformed specifier is an `UnknownFormatConversionException` naming either the
/// conversion character (when there is one) or the first character after the `%`.
fn parse_spec(format: &str, start: usize) -> Result<(Spec, usize)> {
    let bytes = format.as_bytes();
    let unknown = |c: char| FormatError::UnknownConversion(c).into();
    let first = format[start..].chars().next().unwrap_or('%');
    let number = |pos: &mut usize| -> Option<usize> {
        let begin = *pos;
        while bytes.get(*pos).is_some_and(u8::is_ascii_digit) {
            *pos += 1;
        }
        format[begin..*pos].parse().ok()
    };
    let mut pos = start;
    // An argument index is a number followed by `$`.
    let mut index = None;
    let mut probe = pos;
    if let Some(n) = number(&mut probe)
        && bytes.get(probe) == Some(&b'$')
    {
        if n == 0 {
            return Err(FormatError::ZeroArgumentIndex.into());
        }
        index = Some(n);
        pos = probe + 1;
    }
    let mut flags = Flags::default();
    while let Some(&c) = bytes.get(pos)
        && b"-#+ 0,(<".contains(&c)
    {
        flags.set(char::from(c))?;
        pos += 1;
    }
    let width = number(&mut pos);
    let mut precision = None;
    if bytes.get(pos) == Some(&b'.') {
        pos += 1;
        precision = Some(number(&mut pos).ok_or_else(|| unknown(first))?);
    }
    let Some(c) = format[pos..].chars().next() else {
        return Err(unknown(first));
    };
    pos += c.len_utf8();
    let (conversion, suffix) = match c {
        't' | 'T' => {
            // A date/time conversion has a second, alphabetic, character.
            let suffix = format[pos..]
                .chars()
                .next()
                .filter(char::is_ascii_alphabetic)
                .ok_or_else(|| unknown(c))?;
            pos += suffix.len_utf8();
            ('t', Some(suffix))
        }
        c if "bBhHsScCdoxXeEfgGaA%n".contains(c) => (c.to_ascii_lowercase(), None),
        c if c.is_ascii_alphabetic() => return Err(unknown(c)),
        _ => return Err(unknown(first)),
    };
    flags.uppercase = c.is_ascii_uppercase();
    let spec = Spec {
        index,
        flags,
        width,
        precision,
        conversion,
        suffix,
    };
    Ok((spec, pos))
}

/// `MissingFormatArgumentException`: `Format specifier '%s'`.
fn missing_argument(spec: &Spec) -> Error {
    FormatError::MissingArgument(spec.text()).into()
}

/// `String.format(locale, format, args)`; the error is the message of the
/// `IllegalFormatException` Java would throw.
pub fn printf_format(locale: Locale, format: &str, args: &[Value]) -> Result<String> {
    let mut out = String::new();
    let mut pos = 0;
    // The index of the last argument taken in sequence, and of the last argument used.
    let mut ordinary: Option<usize> = None;
    let mut last: Option<usize> = None;
    while let Some(offset) = format[pos..].find('%') {
        out.push_str(&format[pos..pos + offset]);
        let (spec, next) = parse_spec(format, pos + offset + 1)?;
        spec.check()?;
        pos = next;
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
        // Argument selection: `<` reuses the previous argument, `n$` picks one, otherwise the
        // next argument in sequence is taken.
        let index = if spec.flags.previous {
            last.ok_or_else(|| missing_argument(&spec))?
        } else if let Some(explicit) = spec.index {
            explicit - 1
        } else {
            let next = ordinary.map_or(0, |i| i + 1);
            ordinary = Some(next);
            next
        };
        last = Some(index);
        let arg = args.get(index).ok_or_else(|| missing_argument(&spec))?;
        if spec.conversion == 't' {
            return Err(FormatError::DateTimeUnsupported.into());
        }
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

/// The first `precision` UTF-16 units of `text` (the precision of a general conversion).
fn cut(text: &str, precision: Option<usize>) -> String {
    match precision {
        Some(precision) => {
            let units: Vec<u16> = text.encode_utf16().take(precision).collect();
            String::from_utf16_lossy(&units)
        }
        None => text.to_owned(),
    }
}

/// Java `hashCode()` of a value, for `%h`.
pub(super) fn hash_code(value: &Value) -> i32 {
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
            .fold(1i32, |h, v| h.wrapping_mul(31).wrapping_add(hash_code(v))),
        Value::Map(map) => map.borrow().iter().fold(0i32, |h, (k, v)| {
            h.wrapping_add(hash_code(k) ^ hash_code(v))
        }),
        Value::Set(set) => set
            .borrow()
            .iter()
            .fold(0i32, |h, v| h.wrapping_add(hash_code(v))),
        // `null` hashes to 0; identity hash codes are not reproducible (they depend on the
        // JVM) and are reported as 0 too.
        _ => 0,
    }
}

/// `IllegalFormatConversionException`: `d != java.lang.Double`.
fn conversion_error(spec: &Spec, value: &Value) -> Error {
    FormatError::ConversionMismatch {
        conversion: spec.conversion,
        found: value.kind(),
    }
    .into()
}

/// Format one argument with `spec`.
fn format_arg(locale: Locale, spec: &Spec, arg: &Value) -> Result<String> {
    let flags = spec.flags;
    // Only a `Formattable` argument accepts `#` with `%s`, and no WarpScript value is one.
    if spec.conversion == 's' && flags.alternate {
        return Err(mismatch('#', 's'));
    }
    // The general conversions (and `null` for any conversion) print a string cut to the
    // precision; the numeric conversions need an argument of the matching type.
    let text = match (spec.conversion, arg) {
        ('d' | 'o' | 'x', Value::Long(value)) => format_integer(locale, spec, *value)?,
        ('e' | 'f' | 'g' | 'a', Value::Double(value)) => format_double(locale, spec, *value),
        ('e' | 'f' | 'g', Value::BigDecimal(value)) => format_big_decimal(locale, spec, value),
        ('b', Value::Null | Value::Boolean(false)) => cut("false", spec.precision),
        ('b', _) => cut("true", spec.precision),
        (_, Value::Null) => cut("null", spec.precision),
        ('h', _) => cut(&format!("{:x}", hash_code(arg) as u32), spec.precision),
        ('s', _) => cut(&arg.to_string(), spec.precision),
        (_, value) => return Err(conversion_error(spec, value)),
    };
    let text = if flags.uppercase {
        text.to_uppercase()
    } else {
        text
    };
    Ok(justify(&text, spec.width, flags.left_justify))
}

/// The sign written in front of a number: `-` or `(` when negative, else `+` or a space when
/// the corresponding flag is set.
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

/// The `)` closing a negative number written between parentheses.
fn closing_paren(flags: Flags, negative: bool) -> &'static str {
    if negative && flags.parentheses {
        ")"
    } else {
        ""
    }
}

/// The number localisation algorithm of the `Formatter` specification, applied to a magnitude
/// written `ddd[.ddd]`: grouping separators every three digits of the integer part when the
/// `,` flag is set, the locale's decimal separator, the sign (see [`leading_sign`]) and, when
/// the `0` flag is set, zeros between the sign and the digits up to `width`. The closing
/// parenthesis of a negative number, which the caller appends, counts towards the width.
fn localize(
    locale: Locale,
    flags: Flags,
    width: Option<usize>,
    negative: bool,
    magnitude: &str,
) -> String {
    let (integer, fraction) = magnitude
        .split_once('.')
        .map_or((magnitude, None), |(integer, fraction)| {
            (integer, Some(fraction))
        });
    let mut number = String::new();
    if flags.group {
        for (n, group) in integer.as_bytes().rchunks(3).rev().enumerate() {
            if n > 0 {
                number.push(locale.grouping);
            }
            number.push_str(std::str::from_utf8(group).unwrap_or_default());
        }
    } else {
        number.push_str(integer);
    }
    if let Some(fraction) = fraction {
        number.push(locale.decimal);
        number.push_str(fraction);
    }
    let sign = leading_sign(flags, negative);
    let mut out = String::from(sign);
    if flags.zero_pad
        && let Some(width) = width
    {
        let used = sign.len() + number.chars().count() + closing_paren(flags, negative).len();
        out.extend(repeat_n('0', width.saturating_sub(used)));
    }
    out.push_str(&number);
    out
}

/// `%d`, `%o`, `%x` of a `LONG`.
fn format_integer(locale: Locale, spec: &Spec, value: i64) -> Result<String> {
    let flags = spec.flags;
    if spec.conversion == 'd' {
        let negative = value < 0;
        let magnitude = value.unsigned_abs().to_string();
        let mut out = localize(locale, flags, spec.width, negative, &magnitude);
        out.push_str(closing_paren(flags, negative));
        return Ok(out);
    }
    // Octal and hexadecimal print the two's complement of a negative value and take no sign
    // flag; the radix indicator of `#` is part of the zero padded width.
    spec.apply(Rule::Reject(&['(', ' ', '+']))?;
    let (radix, digits) = if spec.conversion == 'o' {
        ("0", format!("{:o}", value as u64))
    } else {
        ("0x", format!("{:x}", value as u64))
    };
    let mut out = String::from(if flags.alternate { radix } else { "" });
    if flags.zero_pad
        && let Some(width) = spec.width
    {
        out.extend(repeat_n(
            '0',
            width.saturating_sub(out.len() + digits.len()),
        ));
    }
    out.push_str(&digits);
    Ok(out)
}

/// A finite, non-negative double as decimal digits: `d₁.d₂…dₙ × 10^exponent`.
///
/// `digits` is empty when rounding left nothing (the value is then zero).
#[derive(Debug)]
struct Decimal {
    digits: Vec<u8>,
    exponent: i32,
}

impl Decimal {
    /// The digits of `Double.toString(value)`: the shortest decimal that identifies the double
    /// (with Java's two-digit minimum for the tiniest subnormals), which is what Java rounds
    /// when a precision is given.
    fn shortest(value: f64) -> Decimal {
        let text = double_to_text(value);
        let (mantissa, exponent) = text.split_once('E').unwrap_or((&text, "0"));
        let exponent = decimal_value(exponent) as i32;
        let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let mut digits: Vec<u8> = integer
            .bytes()
            .chain(fraction.bytes())
            .map(|b| b - b'0')
            .collect();
        // The decimal point sits after the digits of `integer`: dropping the leading zeros
        // shifts it, dropping the trailing zeros changes nothing.
        let leading = digits.iter().take_while(|&&d| d == 0).count();
        if leading == digits.len() {
            return Decimal {
                digits: vec![0],
                exponent: 0,
            };
        }
        digits.drain(..leading);
        while digits.len() > 1 && digits.last() == Some(&0) {
            digits.pop();
        }
        Decimal {
            digits,
            exponent: integer.len() as i32 - leading as i32 - 1 + exponent,
        }
    }

    /// The decimal of the digits of an unscaled value (`d₁` at `10^exponent`).
    fn from_digits(text: &str, exponent: i32) -> Decimal {
        let mut digits: Vec<u8> = text.bytes().map(|b| b - b'0').collect();
        while digits.len() > 1 && digits.last() == Some(&0) {
            digits.pop();
        }
        // A zero keeps the exponent of its scale (`0.00` is `0e-02`).
        Decimal { digits, exponent }
    }

    /// Round half-up to `keep` significant digits; with `keep < 0` the value becomes zero.
    fn round(&mut self, keep: i32) {
        if keep >= self.digits.len() as i32 {
            return;
        }
        let round_up = usize::try_from(keep).is_ok_and(|k| self.digits[k] >= 5);
        self.digits.truncate(keep.max(0) as usize);
        if round_up {
            // Add one unit in the last kept place, carrying through the nines.
            let mut carry = true;
            for digit in self.digits.iter_mut().rev() {
                if *digit == 9 {
                    *digit = 0;
                } else {
                    *digit += 1;
                    carry = false;
                    break;
                }
            }
            if carry {
                self.digits.insert(0, 1);
                self.exponent += 1;
            }
        }
    }

    /// The digit at decimal position `p` (`0` is the units digit, `-1` the tenths), zero
    /// beyond the stored digits.
    fn digit_at(&self, p: i32) -> char {
        let digit = position(i64::from(self.exponent - p), self.digits.len())
            .and_then(|i| self.digits.get(i))
            .copied()
            .unwrap_or(0);
        char::from(b'0' + digit)
    }

    /// Plain notation with exactly `places` fraction digits.
    fn fixed(&self, places: usize) -> String {
        let mut text: String = (0..=self.exponent.max(0))
            .rev()
            .map(|p| self.digit_at(p))
            .collect();
        if places > 0 {
            text.push('.');
            text.extend((1..=places as i32).map(|k| self.digit_at(-k)));
        }
        text
    }

    /// Scientific notation: the mantissa `d.ddd` with exactly `places` fraction digits and the
    /// exponent as Java prints it (`+03`, `-100`).
    fn scientific(&self, places: usize) -> (String, String) {
        let mut mantissa = String::from(self.digit_at(self.exponent));
        if places > 0 {
            mantissa.push('.');
            mantissa.extend((1..=places as i32).map(|k| self.digit_at(self.exponent - k)));
        }
        let sign = if self.exponent < 0 { '-' } else { '+' };
        (mantissa, format!("{sign}{:02}", self.exponent.abs()))
    }
}

/// `%e`, `%f`, `%g`, `%a` of a `DOUBLE`.
fn format_double(locale: Locale, spec: &Spec, value: f64) -> String {
    let flags = spec.flags;
    if value.is_nan() {
        return "NaN".to_owned();
    }
    let negative = value.is_sign_negative();
    let magnitude = value.abs();
    let closing = closing_paren(flags, negative);
    if magnitude.is_infinite() {
        return format!("{}Infinity{closing}", leading_sign(flags, negative));
    }
    if spec.conversion == 'a' {
        // Zeros go between the radix indicator and the digits; `(` is not allowed here.
        let body = hex_float(magnitude, spec.precision);
        let mut out = format!("{}0x", leading_sign(flags, negative));
        if flags.zero_pad
            && let Some(width) = spec.width
        {
            out.extend(repeat_n('0', width.saturating_sub(out.len() + body.len())));
        }
        out.push_str(&body);
        return out;
    }
    format_decimal(locale, spec, negative, Decimal::shortest(magnitude))
}

/// `%e`, `%f`, `%g` of a `BIGDECIMAL`: its exact digits. `%g` chooses with the unrounded
/// magnitude (`[10^-4, 10^precision)`, or a zero of scale 0) and the exponent of its scale.
fn format_big_decimal(
    locale: Locale,
    spec: &Spec,
    value: &crate::bigdecimal::BigDecimal,
) -> String {
    use crate::bigdecimal::BigDecimal;
    let digits = num_traits::Signed::abs(&value.unscaled).to_string();
    let exponent = (digits.len() as i64 - 1 - i64::from(value.scale)) as i32;
    let decimal = Decimal::from_digits(&digits, exponent);
    let negative = value.signum() < 0;
    if spec.conversion != 'g' {
        return format_decimal(locale, spec, negative, decimal);
    }
    let precision = match spec.precision {
        None => 6,
        Some(0) => 1,
        Some(p) => p as i32,
    };
    let magnitude = value.abs();
    let zero = value.signum() == 0 && value.scale == 0;
    let lower = BigDecimal::new(1.into(), 4);
    let upper = BigDecimal::new(1.into(), precision.wrapping_neg());
    let plain = zero
        || (magnitude.compare(&lower) != std::cmp::Ordering::Less
            && magnitude.compare(&upper) == std::cmp::Ordering::Less);
    let mut adjusted = spec.clone();
    if plain {
        adjusted.conversion = 'f';
        adjusted.precision = Some((precision - exponent - 1).max(0) as usize);
    } else {
        adjusted.conversion = 'e';
        adjusted.precision = Some((precision - 1) as usize);
    }
    format_decimal(locale, &adjusted, negative, decimal)
}

/// `%e`, `%f`, `%g` of the decimal digits of a finite magnitude (a `DOUBLE`'s shortest
/// digits, a `BIGDECIMAL`'s exact ones), rounded half-up.
fn format_decimal(locale: Locale, spec: &Spec, negative: bool, mut decimal: Decimal) -> String {
    let flags = spec.flags;
    let closing = closing_paren(flags, negative);
    let precision = spec.precision.map_or(6, |p| p as i32);
    let (mut mantissa, places, exponent) = match spec.conversion {
        'e' => {
            decimal.round(precision + 1);
            let (mantissa, exponent) = decimal.scientific(precision as usize);
            (mantissa, precision, Some(exponent))
        }
        'f' => {
            decimal.round(decimal.exponent + 1 + precision);
            (decimal.fixed(precision as usize), precision, None)
        }
        _ => {
            // `%g`: `precision` significant digits (at least one), in plain notation when the
            // rounded magnitude lies in `[10^-4, 10^precision)`.
            let significant = precision.max(1);
            decimal.round(significant);
            if (-4..significant).contains(&decimal.exponent) {
                let places = significant - 1 - decimal.exponent;
                (decimal.fixed(places as usize), places, None)
            } else {
                let (mantissa, exponent) = decimal.scientific((significant - 1) as usize);
                (mantissa, significant - 1, Some(exponent))
            }
        }
    };
    if flags.alternate && places == 0 {
        mantissa.push('.');
    }
    // The exponent and its `e` take room from the width.
    let room = spec.width.map(|width| {
        exponent
            .as_ref()
            .map_or(width, |e| width.saturating_sub(e.len() + 1))
    });
    let mut out = localize(locale, flags, room, negative, &mantissa);
    if let Some(exponent) = exponent {
        out.push('e');
        out.push_str(&exponent);
    }
    out.push_str(closing);
    out
}

/// The hexadecimal notation of a finite non-negative double for `%a`, without the `0x`:
/// `d.hhhp±e`. Without a precision, or with one beyond the 13 digits of a significand, these
/// are the shortest digits (`Double.toHexString`), zero padded to the precision; otherwise the
/// significand is rounded to the precision (at least one digit, ties to even), subnormals being
/// normalised first.
fn hex_float(value: f64, precision: Option<usize>) -> String {
    let (mut mantissa, exponent) = match precision {
        Some(digits) if digits <= 12 && value != 0.0 => rounded_hex(value, digits.max(1)),
        _ => shortest_hex(value),
    };
    if let Some(digits) = precision {
        let fraction = mantissa.len() - 2;
        mantissa.extend(repeat_n('0', digits.saturating_sub(fraction)));
    }
    format!("{mantissa}p{exponent}")
}

/// `Double.toHexString` without the `0x`: `1.h…` for a normal number, `0.h…` with the
/// exponent `-1022` for a subnormal one, with the trailing zeros of the fraction removed
/// (keeping at least one digit).
fn shortest_hex(value: f64) -> (String, i32) {
    let bits = value.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let fraction = format!("{:013x}", bits & 0x000f_ffff_ffff_ffff);
    let fraction = fraction.trim_end_matches('0');
    let fraction = if fraction.is_empty() { "0" } else { fraction };
    if biased == 0 {
        let exponent = if value == 0.0 { 0 } else { -1022 };
        (format!("0.{fraction}"), exponent)
    } else {
        (format!("1.{fraction}"), biased - 1023)
    }
}

/// The significand of a finite positive double rounded to `digits` (1 to 12) hexadecimal
/// digits, ties to even: `1.h…` and the exponent. A subnormal is scaled up by `2^54` first (an
/// exact operation), the exponent being scaled back; a carry out of the significand gives
/// `1.0` with the next exponent (`1.0p1024` beyond `Double.MAX_VALUE`).
fn rounded_hex(value: f64, digits: usize) -> (String, i32) {
    let (value, scale) = if value < f64::MIN_POSITIVE {
        (value * 2f64.powi(54), 54)
    } else {
        (value, 0)
    };
    let bits = value.to_bits();
    let dropped = 52 - 4 * digits as u32;
    let half = 1u64 << (dropped - 1);
    let rest = bits & ((1u64 << dropped) - 1);
    let mut kept = bits >> dropped;
    if rest > half || (rest == half && kept & 1 == 1) {
        kept += 1;
    }
    let rounded = kept << dropped;
    let biased = ((rounded >> 52) & 0x7ff) as i32;
    let fraction = format!("{:013x}", rounded & 0x000f_ffff_ffff_ffff);
    (format!("1.{}", &fraction[..digits]), biased - 1023 - scale)
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
    let formatted = printf_format(locale, &format, &args.borrow())
        .map_err(|err| fail_with_cause(NAME, "expects a valid format.", err))?;
    stack.push(Value::from(formatted));
    Ok(())
}
