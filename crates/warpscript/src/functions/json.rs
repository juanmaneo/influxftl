// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! JSON serialisation and parsing: `->JSON`, `JSON->` and the configuration functions
//! `JSONLOOSE`, `JSONPRETTY`, `JSONCOMPACT`, `MAXJSON` (`io.warp10.json.JsonUtils`).
//!
//! The output is what Warp 10 produces with Jackson 2.14: insertion-ordered maps,
//! `Double.toString` rendering of doubles, every non-ASCII character escaped as `\uXXXX`
//! (`ESCAPE_NON_ASCII`), the `DefaultPrettyPrinter` layout, bare `NaN` / `Infinity` tokens in
//! loose mode and `"NaN"` / `"Infinity"` strings in strict mode (`WRITE_NAN_AS_STRINGS`). The
//! custom serializers of `io.warp10.json` are honoured: `BYTES` become the ISO-8859-1 string of
//! the bytes, macros and named functions their `toString()`, and the objects Jackson cannot
//! serialise as beans (`BITSET`, `MARK`, contexts, `MATCHER`…) become `null`.
//!
//! Parsing follows Jackson's strict mapper configuration: non-numeric numbers (`NaN`,
//! `Infinity`, `-INF`…) and missing array values (`[1,,2]`) are accepted, trailing tokens are
//! rejected, integers become `LONG`s and other numbers `DOUBLE`s.
//!
//! Not ported yet (`io.warp10.json`): the serializers of Geo Time Series, GTS encoders,
//! `Metadata`, `RealVector` / `RealMatrix` and audit statements — none of those types exist in
//! the runtime so far.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use indexmap::IndexMap;

use super::Entry;
use super::args::{fail_with_cause, pop_string};
use super::misc::ATTRIBUTE_JSON_STRICT;
use crate::encoder::GtsEncoder;
use crate::error::fault::ResourceError;
use crate::error::text::TextError;
use crate::error::{Error, Result};
use crate::format::java_double_to_string;
use crate::geoxp;
use crate::gts::{self, GeoTimeSerie, Metadata};
use crate::stack::Stack;
use crate::value::Value;

/// Stack attribute set by `JSONPRETTY` / `JSONCOMPACT` (`ATTRIBUTE_JSON_PRETTY`).
pub const ATTRIBUTE_JSON_PRETTY: &str = "json.pretty";

/// Maximum nesting depth of arrays and objects, when serialising and when parsing (the
/// default of Jackson's `StreamWriteConstraints` / `StreamReadConstraints`).
pub const MAX_NESTING_DEPTH: usize = 1000;

/// Longest token quoted in an `Unrecognized token` error (`MAX_ERROR_TOKEN_LENGTH`).
const MAX_ERROR_TOKEN_LENGTH: usize = 256;

/// The list of valid values quoted by the parser errors.
const VALID_VALUES: &str = "(JSON String, Number (or 'NaN'/'INF'/'+INF'), Array, Object or token 'null', 'true' or 'false')";

/// Start location of the root context in the mismatched close marker errors.
const ROOT_START: &str = "[line: 1, column: 0]";

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("->JSON", Entry::Stack(tojson)),
    ("JSON->", Entry::Stack(jsonto)),
    ("JSONLOOSE", Entry::Stack(jsonloose)),
    ("JSONPRETTY", Entry::Stack(jsonpretty)),
    ("JSONCOMPACT", Entry::Stack(jsoncompact)),
    ("MAXJSON", Entry::Stack(maxjson)),
];

// -----------------------------------------------------------------------------------------
// Serialisation
// -----------------------------------------------------------------------------------------

/// How [`to_json`] renders a value.
#[derive(Debug, Clone, Copy)]
pub struct JsonOptions {
    /// Strict JSON: non-finite doubles are written as strings (`"NaN"`); loose JSON writes
    /// the bare tokens `NaN`, `Infinity` and `-Infinity`.
    pub strict: bool,
    /// Indented output (Jackson's `DefaultPrettyPrinter`).
    pub pretty: bool,
    /// Maximum number of characters of the output (`i64::MAX` = unbounded).
    pub max_size: i64,
}

impl Default for JsonOptions {
    /// Loose, compact, unbounded.
    fn default() -> Self {
        JsonOptions {
            strict: false,
            pretty: false,
            max_size: i64::MAX,
        }
    }
}

/// Why [`to_json`] failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonError {
    /// The output exceeds the maximum size (`BoundedWriter`).
    TooBig,
    /// Arrays or objects nested deeper than [`MAX_NESTING_DEPTH`] (which is also what a
    /// structure referencing itself looks like).
    TooDeep,
}

type SerializeResult = std::result::Result<(), JsonError>;

/// The value of the number `text`, as Jackson reads it: a `LONG`, or a `DOUBLE` for a float or
/// for an integer beyond the `long` range (Jackson yields a `BigInteger`, which has no closer
/// value here). The scanner has checked that `text` follows the JSON number grammar, which
/// `f64` parsing accepts whole.
pub(super) fn number_value(text: &str, is_float: bool) -> Value {
    let double = || Value::Double(text.parse().unwrap_or(f64::NAN));
    if is_float {
        return double();
    }
    text.parse()
        .map_or_else(|_beyond_long| double(), Value::Long)
}

/// Serialise `value` like `JsonUtils.objectToJson`.
pub fn to_json(value: &Value, options: JsonOptions) -> std::result::Result<String, JsonError> {
    let max_size = if options.max_size == i64::MAX {
        None
    } else {
        Some(usize::try_from(options.max_size.max(0)).unwrap_or(usize::MAX))
    };
    let mut serializer = Serializer {
        out: String::new(),
        strict: options.strict,
        pretty: options.pretty,
        nesting: 0,
        max_size,
    };
    serializer.write_value(value, 0)?;
    Ok(serializer.out)
}

/// The JSON object of a Geo Time Series (`GeoTimeSerieSerializer`): `c` (class name), `l`
/// (labels), `a` (attributes), `la` (last activity) and `v`, the datapoints as
/// `[tick, lat, lon, elevation, value]` arrays with the absent location and elevation left
/// out.
fn gts_entries(gts: &GeoTimeSerie) -> IndexMap<Value, Value> {
    let datapoints = (0..gts.size()).map(|i| {
        (
            gts.tick_at(i),
            gts.location_at(i),
            gts.elevation_at(i),
            gts.value_at(i),
        )
    });
    series_entries(&gts.metadata, datapoints)
}

/// The JSON object of an encoder (`GTSEncoderSerializer`): the same fields as a series, the
/// values of every type mixed. The decoding stops at a corrupt datapoint.
fn encoder_entries(encoder: &GtsEncoder) -> IndexMap<Value, Value> {
    let mut decoder = encoder.decoder();
    let mut datapoints = Vec::new();
    loop {
        match decoder.next() {
            Ok(true) => datapoints.push((
                decoder.timestamp(),
                decoder.location(),
                decoder.elevation(),
                decoder.value().to_value(),
            )),
            Ok(false) => break,
            // Warp 10's serializer stops at a corrupt datapoint and keeps the ones before it.
            Err(_corrupt) => break,
        }
    }
    series_entries(&encoder.metadata, datapoints.into_iter())
}

fn series_entries(
    metadata: &Metadata,
    datapoints: impl Iterator<Item = (i64, i64, i64, Value)>,
) -> IndexMap<Value, Value> {
    let strings = |map: &IndexMap<String, String>| {
        Value::map(
            map.iter()
                .map(|(k, v)| (Value::from(k.as_str()), Value::from(v.as_str())))
                .collect(),
        )
    };
    let datapoints = datapoints
        .map(|(tick, location, elevation, value)| {
            let mut point = vec![Value::Long(tick)];
            if location != gts::NO_LOCATION {
                let (lat, lon) = geoxp::from_geoxp_point(location);
                point.push(Value::Double(lat));
                point.push(Value::Double(lon));
            }
            if elevation != gts::NO_ELEVATION {
                point.push(Value::Long(elevation));
            }
            point.push(value);
            Value::list(point)
        })
        .collect();
    IndexMap::from([
        (
            Value::from("c"),
            Value::from(metadata.name.as_deref().unwrap_or_default()),
        ),
        (Value::from("l"), strings(&metadata.labels)),
        (Value::from("a"), strings(&metadata.attributes)),
        (Value::from("la"), Value::Long(metadata.last_activity)),
        (Value::from("v"), Value::list(datapoints)),
    ])
}

/// A Jackson-compatible generator writing into a string.
struct Serializer {
    out: String,
    strict: bool,
    pretty: bool,
    /// Indentation level of the pretty printer: objects indent, arrays are inline.
    nesting: usize,
    /// Size bound, `None` when unbounded. The output is pure ASCII, so bytes are characters.
    max_size: Option<usize>,
}

impl Serializer {
    /// `BoundedWriter`: fail as soon as the output exceeds the bound.
    fn check_size(&self) -> SerializeResult {
        match self.max_size {
            Some(max) if self.out.len() > max => Err(JsonError::TooBig),
            _ => Ok(()),
        }
    }

    /// Write `value`, nested in `depth` arrays or objects.
    fn write_value(&mut self, value: &Value, depth: usize) -> SerializeResult {
        match value {
            // Beans and unknown objects are replaced by `null` (`CustomSerializer`).
            Value::Null | Value::BitSet(_) | Value::Mark | Value::Context(_) => {
                self.out.push_str("null");
            }
            Value::Boolean(b) => self.out.push_str(if *b { "true" } else { "false" }),
            Value::Long(l) => self.out.push_str(&l.to_string()),
            // Jackson writes an `AtomicLong` as its number.
            Value::Counter(c) => self.out.push_str(&c.get().to_string()),
            Value::Double(d) => self.write_double(*d),
            // Jackson writes a `BigDecimal` with its `toString()`.
            Value::BigDecimal(bd) => self.out.push_str(&bd.to_string()),
            // Jackson writes `getData()` / `toArray()`.
            Value::Matrix(m) => {
                let rows: Vec<Value> = (0..m.rows)
                    .map(|r| Value::list((0..m.cols).map(|c| Value::Double(m.get(r, c))).collect()))
                    .collect();
                self.write_array(rows.iter(), depth)?;
            }
            Value::Vector(v) => {
                let entries: Vec<Value> = v.iter().map(|&x| Value::Double(x)).collect();
                self.write_array(entries.iter(), depth)?;
            }
            Value::String(s) => self.write_string(s.chars()),
            // `BytesSerializer`: the ISO-8859-1 decoding of the bytes.
            Value::Bytes(b) => self.write_string(b.iter().map(|&byte| char::from(byte))),
            // `MacroSerializer`: `Macro.toString()`.
            Value::Macro(m) => self.write_string(m.to_string().chars()),
            // `NamedWarpScriptFunctionSerializer` writes `toString()`; the `Matcher` pushed by
            // `MATCHER` is not a named function and has no serializer, hence `null`.
            Value::Function(f) => {
                if f.type_name() == "MATCHER" {
                    self.out.push_str("null");
                } else {
                    self.write_string(f.snapshot().chars());
                }
            }
            Value::List(list) => {
                let items = list.borrow();
                self.write_array(items.iter(), depth)?;
            }
            Value::Set(set) => {
                let items = set.borrow();
                self.write_array(items.iter(), depth)?;
            }
            Value::Map(map) => {
                let entries = map.borrow();
                self.write_object(&entries, depth)?;
            }
            // `GeoTimeSerieSerializer`: the metadata fields then the datapoints.
            Value::Gts(gts) => {
                let entries = gts_entries(&gts.borrow());
                self.write_object(&entries, depth)?;
            }
            Value::Encoder(encoder) => {
                let entries = encoder_entries(&encoder.borrow());
                self.write_object(&entries, depth)?;
            }
        }
        self.check_size()
    }

    /// `Double.toString`, with non-finite values quoted in strict mode.
    fn write_double(&mut self, d: f64) {
        let text = java_double_to_string(d);
        if d.is_finite() || !self.strict {
            self.out.push_str(&text);
        } else {
            self.out.push('"');
            self.out.push_str(&text);
            self.out.push('"');
        }
    }

    /// A quoted string: `"`, `\` and the control characters are escaped the JSON way, every
    /// non-ASCII character as `\uXXXX` UTF-16 units (`ESCAPE_NON_ASCII`).
    fn write_string(&mut self, chars: impl Iterator<Item = char>) {
        self.out.push('"');
        for c in chars {
            match c {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\u{8}' => self.out.push_str("\\b"),
                '\t' => self.out.push_str("\\t"),
                '\n' => self.out.push_str("\\n"),
                '\u{c}' => self.out.push_str("\\f"),
                '\r' => self.out.push_str("\\r"),
                c if c.is_ascii() => {
                    if c < ' ' {
                        self.out.push_str(&format!("\\u{:04X}", u32::from(c)));
                    } else {
                        self.out.push(c);
                    }
                }
                c => {
                    let mut units = [0u16; 2];
                    for unit in c.encode_utf16(&mut units) {
                        self.out.push_str(&format!("\\u{unit:04X}"));
                    }
                }
            }
        }
        self.out.push('"');
    }

    /// A map key (`StdKeySerializers`): strings as is, `null` as `"null"`, byte arrays in
    /// base64, anything else through `toString()`.
    fn write_key(&mut self, key: &Value) {
        match key {
            Value::String(s) => self.write_string(s.chars()),
            Value::Bytes(b) => self.write_string(STANDARD.encode(b).chars()),
            other => self.write_string(other.to_string().chars()),
        }
    }

    /// A line break followed by the current indentation (`DefaultIndenter`).
    fn indent(&mut self) {
        self.out.push('\n');
        for _ in 0..self.nesting {
            self.out.push_str("  ");
        }
    }

    /// `[1,2]`, or `[ 1, 2 ]` / `[ ]` when pretty printing (`FixedSpaceIndenter`).
    fn write_array<'a>(
        &mut self,
        items: impl Iterator<Item = &'a Value>,
        depth: usize,
    ) -> SerializeResult {
        if depth >= MAX_NESTING_DEPTH {
            return Err(JsonError::TooDeep);
        }
        self.out.push('[');
        for (idx, item) in items.enumerate() {
            if idx > 0 {
                self.out.push(',');
            }
            if self.pretty {
                self.out.push(' ');
            }
            self.write_value(item, depth + 1)?;
        }
        if self.pretty {
            self.out.push(' ');
        }
        self.out.push(']');
        Ok(())
    }

    /// `{"a":1}`, or one `"a" : 1` entry per line / `{ }` when pretty printing.
    fn write_object(&mut self, entries: &IndexMap<Value, Value>, depth: usize) -> SerializeResult {
        if depth >= MAX_NESTING_DEPTH {
            return Err(JsonError::TooDeep);
        }
        self.out.push('{');
        self.nesting += 1;
        for (idx, (key, value)) in entries.iter().enumerate() {
            if idx > 0 {
                self.out.push(',');
            }
            if self.pretty {
                self.indent();
            }
            self.write_key(key);
            self.out.push_str(if self.pretty { " : " } else { ":" });
            self.write_value(value, depth + 1)?;
        }
        self.nesting -= 1;
        if self.pretty {
            if entries.is_empty() {
                self.out.push(' ');
            } else {
                self.indent();
            }
        }
        self.out.push('}');
        Ok(())
    }
}

// -----------------------------------------------------------------------------------------
// Parsing
// -----------------------------------------------------------------------------------------

/// Parse `input` like `JsonUtils.jsonToObject` followed by `JSONTO.transform`: objects become
/// insertion-ordered `MAP`s, arrays `LIST`s, integers `LONG`s (`DOUBLE`s beyond the `long`
/// range), other numbers `DOUBLE`s. The error message is the parser's, with the location
/// where the problem was found.
pub fn from_json(input: &str) -> Result<Value> {
    let mut parser = Parser {
        input,
        bytes: input.as_bytes(),
        pos: 0,
    };
    parser.skip_ws()?;
    let Some(first) = parser.peek_char() else {
        return Err(TextError::JsonLocated {
            message: "No content to map due to end-of-input".to_owned(),
            location: ROOT_START.to_owned(),
        }
        .into());
    };
    let value = parser.parse_value(first, 0, true)?;
    parser.skip_ws()?;
    match parser.peek_char() {
        Some(next) => Err(parser.trailing_token(next)),
        None => Ok(value),
    }
}

/// Java `Character.isJavaIdentifierStart`, approximated: letters, `_` and currency symbols.
fn is_identifier_start(c: char) -> bool {
    c.is_alphabetic() || matches!(c, '_' | '$' | '\u{a2}'..='\u{a5}' | '\u{20ac}')
}

/// Java `Character.isJavaIdentifierPart`, approximated: identifier starts, digits and the
/// "identifier ignorable" control characters.
fn is_identifier_part(c: char) -> bool {
    is_identifier_start(c)
        || c.is_numeric()
        || matches!(c, '\0'..='\u{8}' | '\u{e}'..='\u{1b}' | '\u{7f}'..='\u{9f}')
}

/// `ParserMinimalBase._getCharDesc`: `'x' (code 120)`, `(CTRL-CHAR, code 9)`, and the hex
/// code too beyond ISO-8859-1.
fn char_desc(c: char) -> String {
    let code = u32::from(c);
    if c.is_control() {
        format!("(CTRL-CHAR, code {code})")
    } else if code > 255 {
        format!("'{c}' (code {code} / 0x{code:x})")
    } else {
        format!("'{c}' (code {code})")
    }
}

/// A recursive descent parser mirroring `ReaderBasedJsonParser` (Jackson 2.14) with
/// `ALLOW_NON_NUMERIC_NUMBERS`, `ALLOW_MISSING_VALUES` and `FAIL_ON_TRAILING_TOKENS`.
struct Parser<'a> {
    input: &'a str,
    bytes: &'a [u8],
    /// Byte offset of the next character, always on a character boundary.
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek_char(&self) -> Option<char> {
        self.input
            .get(self.pos..)
            .and_then(|rest| rest.chars().next())
    }

    fn next_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    /// Line and column of the byte offset `pos`, 1-based like `JsonLocation` (the column
    /// counts the characters since the last line break; `\r\n` is a single break).
    fn line_column(&self, pos: usize) -> (usize, usize) {
        let (mut line, mut column) = (1usize, 1usize);
        let mut after_cr = false;
        for c in self.input.get(..pos).unwrap_or("").chars() {
            match c {
                '\n' if after_cr => after_cr = false,
                '\n' | '\r' => {
                    line += 1;
                    column = 1;
                    after_cr = c == '\r';
                }
                _ => {
                    column += 1;
                    after_cr = false;
                }
            }
        }
        (line, column)
    }

    /// `[line: L, column: C]` of the byte offset `pos`.
    fn location(&self, pos: usize) -> String {
        let (line, column) = self.line_column(pos);
        format!("[line: {line}, column: {column}]")
    }

    /// An error located at the byte offset `pos`.
    fn error(&self, pos: usize, message: impl AsRef<str>) -> Error {
        TextError::JsonLocated {
            message: message.as_ref().to_owned(),
            location: self.location(pos),
        }
        .into()
    }

    /// `_reportUnexpectedChar`: consume `c` (the character at the current position) and
    /// report it.
    fn unexpected(&mut self, c: char, comment: &str) -> Error {
        self.pos += c.len_utf8();
        self.error(
            self.pos,
            format!("Unexpected character ({}){comment}", char_desc(c)),
        )
    }

    /// End of input inside an array or object (`_handleEOF`).
    fn eof_close(&self, kind: &str, start: usize) -> Error {
        self.error(
            self.pos,
            format!(
                "Unexpected end-of-input: expected close marker for {kind} (start marker at {})",
                self.location(start)
            ),
        )
    }

    /// `_reportMismatchedEndMarker`: `]` closing an object, `}` closing an array, either of
    /// them at the root (`start` is `None`); Jackson names the other marker as expected.
    fn mismatched_close(&mut self, actual: char, kind: &str, start: Option<usize>) -> Error {
        self.pos += 1;
        let expected = if actual == ']' { '}' } else { ']' };
        let start = match start {
            Some(start) => self.location(start),
            None => ROOT_START.to_owned(),
        };
        self.error(
            self.pos,
            format!(
                "Unexpected close marker '{actual}': expected '{expected}' (for {kind} starting at {start})"
            ),
        )
    }

    /// `StreamReadConstraints`: the nesting depth limit.
    fn too_deep() -> Error {
        TextError::JsonNestingDepth {
            depth: MAX_NESTING_DEPTH + 1,
            max: MAX_NESTING_DEPTH,
        }
        .into()
    }

    /// `_reportInvalidToken`: the token starts at `start`, `matched_end` is where it stopped
    /// matching; it is extended with the following identifier characters.
    fn invalid_token(&mut self, start: usize, matched_end: usize) -> Error {
        let mut token = self.input.get(start..matched_end).unwrap_or("").to_owned();
        let mut length = token.chars().count();
        self.pos = matched_end;
        while let Some(c) = self.peek_char() {
            if !is_identifier_part(c) {
                break;
            }
            self.pos += c.len_utf8();
            token.push(c);
            length += 1;
            if length >= MAX_ERROR_TOKEN_LENGTH {
                token.push_str("...");
                break;
            }
        }
        self.error(
            self.pos,
            format!("Unrecognized token '{token}': was expecting {VALID_VALUES}"),
        )
    }

    /// Skip the JSON white space; other control characters are illegal between tokens, and
    /// a `/` is reported as a would-be comment (`ALLOW_COMMENTS` is disabled).
    fn skip_ws(&mut self) -> Result<()> {
        while let Some(b) = self.peek() {
            match b {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                b'/' => {
                    self.pos += 1;
                    return Err(self.error(
                        self.pos,
                        "Unexpected character ('/' (code 47)): maybe a (non-standard) comment? (not recognized as one since Feature 'ALLOW_COMMENTS' not enabled for parser)",
                    ));
                }
                0..=0x1f => {
                    self.pos += 1;
                    return Err(self.error(
                        self.pos,
                        format!(
                            "Illegal character ({}): only regular white space (\\r, \\n, \\t) is allowed between tokens",
                            char_desc(char::from(b))
                        ),
                    ));
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// A value starting with `first` (the character at the current position), nested in
    /// `depth` containers. At the root, numbers must be followed by white space and close
    /// markers are reported as mismatched; elsewhere the callers handle the missing values.
    fn parse_value(&mut self, first: char, depth: usize, root: bool) -> Result<Value> {
        match first {
            '"' => {
                self.pos += 1;
                self.parse_string(false).map(Value::from)
            }
            '[' => self.parse_array(depth),
            '{' => self.parse_object(depth),
            '-' | '0'..='9' => self.parse_number(root),
            '+' => self.parse_plus(),
            't' => self.parse_literal("true", Value::Boolean(true)),
            'f' => self.parse_literal("false", Value::Boolean(false)),
            'n' => self.parse_literal("null", Value::Null),
            'N' => self.parse_literal("NaN", Value::Double(f64::NAN)),
            'I' => self.parse_literal("Infinity", Value::Double(f64::INFINITY)),
            ']' | '}' if root => Err(self.mismatched_close(first, "root", None)),
            _ => Err(self.odd_value(first)),
        }
    }

    /// `_handleOddValue`: something that looks like a token is an unrecognized token, the
    /// rest an unexpected character.
    fn odd_value(&mut self, c: char) -> Error {
        let start = self.pos;
        if is_identifier_start(c) {
            self.pos += c.len_utf8();
            self.invalid_token(start, self.pos)
        } else {
            self.unexpected(c, &format!(": expected a valid value {VALID_VALUES}"))
        }
    }

    /// `_matchToken`: the first `matched` characters of `expected` (which starts at `start`)
    /// have been read; check the rest, and that no identifier character follows.
    fn match_token(&mut self, start: usize, expected: &str, matched: usize) -> Result<()> {
        for (idx, b) in expected.bytes().enumerate().skip(matched) {
            if self.bytes.get(start + idx) != Some(&b) {
                return Err(self.invalid_token(start, start + idx));
            }
        }
        self.pos = start + expected.len();
        if let Some(c) = self.peek_char()
            && c >= '0'
            && c != ']'
            && c != '}'
            && is_identifier_part(c)
        {
            return Err(self.invalid_token(start, self.pos));
        }
        Ok(())
    }

    /// `true`, `false`, `null`, `NaN`, `Infinity`: the first character has been recognised.
    fn parse_literal(&mut self, expected: &str, value: Value) -> Result<Value> {
        let start = self.pos;
        self.match_token(start, expected, 1)?;
        Ok(value)
    }

    /// `+INF` / `+Infinity`; a `+` sign is not allowed in front of a number.
    fn parse_plus(&mut self) -> Result<Value> {
        let start = self.pos;
        self.pos += 1;
        match self.peek_char() {
            None => Err(self.error(self.pos, "Unexpected end-of-input in a Number value")),
            Some('I') => self.parse_signed_infinity(start, false),
            Some(c) => Err(self.plus_sign(c)),
        }
    }

    /// `ALLOW_LEADING_PLUS_SIGN_FOR_NUMBERS` is disabled: consume `c` and report the sign.
    fn plus_sign(&mut self, c: char) -> Error {
        self.pos += c.len_utf8();
        self.error(
            self.pos,
            format!(
                "Unexpected character ({}) in numeric value: JSON spec does not allow numbers to have plus signs: enable `JsonReadFeature.ALLOW_LEADING_PLUS_SIGN_FOR_NUMBERS` to allow",
                char_desc('+')
            ),
        )
    }

    /// `_handleInvalidNumberStart` for a sign followed by `I`: `-INF`, `-Infinity`, `+INF`,
    /// `+Infinity`. `start` is the offset of the sign, the current position that of the `I`.
    fn parse_signed_infinity(&mut self, start: usize, negative: bool) -> Result<Value> {
        self.pos += 1;
        let Some(c) = self.peek_char() else {
            return Err(self.error(self.pos, "Unexpected end-of-input in a Number value"));
        };
        let sign = if negative { "-" } else { "+" };
        let token = match c {
            'N' => format!("{sign}INF"),
            'n' => format!("{sign}Infinity"),
            _ if negative => {
                return Err(self.unexpected(
                    c,
                    " in numeric value: expected digit (0-9) to follow minus sign, for valid numeric value",
                ));
            }
            _ => return Err(self.plus_sign(c)),
        };
        self.pos += 1;
        self.match_token(start, &token, 3)?;
        let infinity = if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
        Ok(Value::Double(infinity))
    }

    /// A number: `-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?`, plus `-INF` / `-Infinity`.
    fn parse_number(&mut self, root: bool) -> Result<Value> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
            match self.peek_char() {
                None => {
                    return Err(self.error(
                        self.pos,
                        "Unexpected end-of-inputNo digit following minus sign",
                    ));
                }
                Some('I') => return self.parse_signed_infinity(start, true),
                Some('0'..='9') => {}
                Some(c) => {
                    return Err(self.unexpected(
                        c,
                        " in numeric value: expected digit (0-9) to follow minus sign, for valid numeric value",
                    ));
                }
            }
        }
        let int_start = self.pos;
        self.skip_digits();
        if self.peek_at(int_start) == Some(b'0') && self.pos - int_start > 1 {
            return Err(self.error(
                int_start + 1,
                "Invalid numeric value: Leading zeroes not allowed",
            ));
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            self.pos += 1;
            is_float = true;
            let fraction_start = self.pos;
            self.skip_digits();
            if self.pos == fraction_start {
                let c = self.peek_char().unwrap_or('.');
                return Err(self.error(
                    self.pos,
                    format!(
                        "Unexpected character ({}) in numeric value: Decimal point not followed by a digit",
                        char_desc(c)
                    ),
                ));
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            is_float = true;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            let exponent_start = self.pos;
            self.skip_digits();
            if self.pos == exponent_start {
                let message = match self.peek_char() {
                    None => {
                        "Unexpected end-of-inputexpected a digit for number exponent".to_owned()
                    }
                    Some(c) => format!(
                        "Unexpected character ({}) in numeric value: Exponent indicator not followed by a digit",
                        char_desc(c)
                    ),
                };
                return Err(self.error(self.pos, message));
            }
        }
        let text = self.input.get(start..self.pos).unwrap_or("");
        let value = number_value(text, is_float);
        // `_verifyRootSpace`: a root-level number must be followed by white space.
        if root
            && let Some(c) = self.peek_char()
            && !matches!(c, ' ' | '\t' | '\r' | '\n')
        {
            return Err(self.unexpected(c, ": Expected space separating root-level values"));
        }
        Ok(value)
    }

    fn peek_at(&self, pos: usize) -> Option<u8> {
        self.bytes.get(pos).copied()
    }

    fn skip_digits(&mut self) {
        while let Some(b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
    }

    /// The content of a string whose opening quote has been consumed; `name` selects the
    /// field name wording of the errors.
    fn parse_string(&mut self, name: bool) -> Result<String> {
        let mut out = String::new();
        loop {
            let Some(b) = self.peek() else {
                let message = if name {
                    "Unexpected end-of-input in field name"
                } else {
                    "Unexpected end-of-input: was expecting closing quote for a string value"
                };
                return Err(self.error(self.pos, message));
            };
            match b {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    out.push(self.parse_escape()?);
                }
                0..=0x1f => {
                    // Jackson only counts the line breaks it skips between tokens: a raw one
                    // inside a string is just the character after which the error is found.
                    let (line, column) = self.line_column(self.pos);
                    self.pos += 1;
                    return Err(TextError::JsonLocated {
 message: format!(
 "Illegal unquoted character ({}): has to be escaped using backslash to be included in {}",
 char_desc(char::from(b)),
 if name { "name" } else { "string value" }
 ),
 location: format!("[line: {line}, column: {}]", column + 1),
 }
 .into());
                }
                _ => {
                    let run_start = self.pos;
                    while let Some(c) = self.peek() {
                        if matches!(c, b'"' | b'\\' | 0..=0x1f) {
                            break;
                        }
                        self.pos += 1;
                    }
                    out.push_str(self.input.get(run_start..self.pos).unwrap_or(""));
                }
            }
        }
    }

    /// `_decodeEscaped`: the character after a backslash. Surrogate pairs written as two
    /// `\uXXXX` escapes are combined, a lone surrogate becomes U+FFFD.
    fn parse_escape(&mut self) -> Result<char> {
        let Some(c) = self.next_char() else {
            return Err(self.error(
                self.pos,
                "Unexpected end-of-input in character escape sequence",
            ));
        };
        Ok(match c {
            'b' => '\u{8}',
            't' => '\t',
            'n' => '\n',
            'f' => '\u{c}',
            'r' => '\r',
            '"' | '/' | '\\' => c,
            'u' => {
                let unit = self.hex4()?;
                if (0xD800..=0xDBFF).contains(&unit) {
                    self.low_surrogate(unit)?
                } else {
                    char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)
                }
            }
            other => {
                return Err(self.error(
                    self.pos,
                    format!("Unrecognized character escape {}", char_desc(other)),
                ));
            }
        })
    }

    /// Complete the high surrogate `high` with a following `\uDC00`–`\uDFFF` escape, if any.
    fn low_surrogate(&mut self, high: u32) -> Result<char> {
        let save = self.pos;
        if self.peek() == Some(b'\\') && self.peek_at(self.pos + 1) == Some(b'u') {
            self.pos += 2;
            let low = self.hex4()?;
            if (0xDC00..=0xDFFF).contains(&low) {
                let code = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                return Ok(char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER));
            }
            // Not a low surrogate: the escape is a character of its own, read it again.
            self.pos = save;
        }
        Ok(char::REPLACEMENT_CHARACTER)
    }

    /// The four hex digits of a `\u` escape.
    fn hex4(&mut self) -> Result<u32> {
        let mut value = 0;
        for _ in 0..4 {
            let Some(c) = self.next_char() else {
                return Err(self.error(
                    self.pos,
                    "Unexpected end-of-input in character escape sequence",
                ));
            };
            let Some(digit) = c.to_digit(16) else {
                return Err(self.error(
                    self.pos,
                    format!(
                        "Unexpected character ({}): expected a hex-digit for character escape sequence",
                        char_desc(c)
                    ),
                ));
            };
            value = (value << 4) | digit;
        }
        Ok(value)
    }

    /// An array; the current position is that of the `[`. A value missing before a `,` or a
    /// closing `]` is `null` (`ALLOW_MISSING_VALUES`).
    fn parse_array(&mut self, depth: usize) -> Result<Value> {
        let start = self.pos;
        self.pos += 1;
        if depth >= MAX_NESTING_DEPTH {
            return Err(Self::too_deep());
        }
        let mut items = Vec::new();
        self.skip_ws()?;
        match self.peek_char() {
            Some(']') => {
                self.pos += 1;
                return Ok(Value::list(items));
            }
            Some('}') => return Err(self.mismatched_close('}', "Array", Some(start))),
            None => return Err(self.eof_close("Array", start)),
            Some(_) => {}
        }
        loop {
            let item = match self.peek_char() {
                Some(',' | ']') => Value::Null,
                Some(first) => self.parse_value(first, depth + 1, false)?,
                None => {
                    return Err(self.error(
                        self.pos,
                        "Unexpected end-of-input within/between Array entries",
                    ));
                }
            };
            items.push(item);
            self.skip_ws()?;
            match self.peek_char() {
                Some(',') => {
                    self.pos += 1;
                    self.skip_ws()?;
                }
                Some(']') => {
                    self.pos += 1;
                    return Ok(Value::list(items));
                }
                Some('}') => return Err(self.mismatched_close('}', "Array", Some(start))),
                Some(c) => {
                    return Err(
                        self.unexpected(c, ": was expecting comma to separate Array entries")
                    );
                }
                None => return Err(self.eof_close("Array", start)),
            }
        }
    }

    /// An object; the current position is that of the `{`. Keys must be quoted strings, a
    /// value missing before a `,` is `null`, a duplicate key keeps its first position and the
    /// last value (`LinkedHashMap.put`).
    fn parse_object(&mut self, depth: usize) -> Result<Value> {
        let start = self.pos;
        self.pos += 1;
        if depth >= MAX_NESTING_DEPTH {
            return Err(Self::too_deep());
        }
        let mut entries = IndexMap::new();
        self.skip_ws()?;
        match self.peek_char() {
            Some('}') => {
                self.pos += 1;
                return Ok(Value::map(entries));
            }
            Some(']') => return Err(self.mismatched_close(']', "Object", Some(start))),
            None => return Err(self.eof_close("Object", start)),
            Some(_) => {}
        }
        loop {
            let key = match self.peek_char() {
                Some('"') => {
                    self.pos += 1;
                    self.parse_string(true)?
                }
                Some(c) => {
                    return Err(
                        self.unexpected(c, ": was expecting double-quote to start field name")
                    );
                }
                None => return Err(self.eof_between_entries()),
            };
            self.skip_ws()?;
            match self.peek_char() {
                Some(':') => self.pos += 1,
                Some(c) => {
                    return Err(self.unexpected(
                        c,
                        ": was expecting a colon to separate field name and value",
                    ));
                }
                None => return Err(self.eof_between_entries()),
            }
            self.skip_ws()?;
            let value = match self.peek_char() {
                Some(',') => Value::Null,
                Some(first) => self.parse_value(first, depth + 1, false)?,
                None => return Err(self.eof_between_entries()),
            };
            entries.insert(Value::from(key), value);
            self.skip_ws()?;
            match self.peek_char() {
                Some(',') => {
                    self.pos += 1;
                    self.skip_ws()?;
                }
                Some('}') => {
                    self.pos += 1;
                    return Ok(Value::map(entries));
                }
                Some(']') => return Err(self.mismatched_close(']', "Object", Some(start))),
                Some(c) => {
                    return Err(
                        self.unexpected(c, ": was expecting comma to separate Object entries")
                    );
                }
                None => return Err(self.eof_close("Object", start)),
            }
        }
    }

    fn eof_between_entries(&self) -> Error {
        self.error(
            self.pos,
            "Unexpected end-of-input within/between Object entries",
        )
    }

    /// `FAIL_ON_TRAILING_TOKENS`: something follows the root value. Jackson reads the token
    /// before complaining — eagerly for numbers and literals (which may fail on their own),
    /// lazily for strings and containers — and reports its type.
    fn trailing_token(&mut self, first: char) -> Error {
        let start = self.pos;
        let token_type = match first {
            '[' => "START_ARRAY",
            '{' => "START_OBJECT",
            '"' => "VALUE_STRING",
            _ => match self.parse_value(first, 0, true) {
                Err(err) => return err,
                Ok(Value::Long(_)) => "VALUE_NUMBER_INT",
                Ok(Value::Double(_)) => "VALUE_NUMBER_FLOAT",
                Ok(Value::Boolean(true)) => "VALUE_TRUE",
                Ok(Value::Boolean(false)) => "VALUE_FALSE",
                Ok(_) => "VALUE_NULL",
            },
        };
        self.error(
            start,
            format!(
                "Trailing token (of type {token_type}) found after value (bound as `java.lang.Object`): not allowed as per `DeserializationFeature.FAIL_ON_TRAILING_TOKENS`"
            ),
        )
    }
}

// -----------------------------------------------------------------------------------------
// Stack functions
// -----------------------------------------------------------------------------------------

/// `->JSON`: serialise the top of the stack according to the `json.strict` / `json.pretty`
/// attributes and the JSON size limit.
fn tojson(stack: &mut Stack) -> Result<()> {
    let value = stack.pop()?;
    let options = JsonOptions {
        strict: matches!(
            stack.attribute(ATTRIBUTE_JSON_STRICT),
            Some(Value::Boolean(true))
        ),
        pretty: matches!(
            stack.attribute(ATTRIBUTE_JSON_PRETTY),
            Some(Value::Boolean(true))
        ),
        max_size: stack.limits.max_json,
    };
    match to_json(&value, options) {
        Ok(json) => {
            stack.push(json);
            Ok(())
        }
        Err(JsonError::TooBig) => Err(fail_with_cause(
            "->JSON",
            "failed with to convert to JSON.",
            Error::with_cause(
                "Resulting JSON is too big.",
                TextError::JsonWriteLimit(options.max_size).into(),
            ),
        )),
        Err(JsonError::TooDeep) => Err(fail_with_cause(
            "->JSON",
            "failed with to convert to JSON, the structure is too deep or it references itself.",
            ResourceError::StackOverflow.into(),
        )),
    }
}

/// `JSON->`: parse the string on top of the stack.
fn jsonto(stack: &mut Stack) -> Result<()> {
    let json = pop_string(stack, "JSON->", "expects a string on top of the stack.")?;
    let value =
        from_json(&json).map_err(|err| fail_with_cause("JSON->", "failed to parse JSON", err))?;
    stack.push(value);
    Ok(())
}

fn jsonloose(stack: &mut Stack) -> Result<()> {
    stack.set_attribute(ATTRIBUTE_JSON_STRICT, Some(Value::Boolean(false)));
    Ok(())
}

fn jsonpretty(stack: &mut Stack) -> Result<()> {
    stack.set_attribute(ATTRIBUTE_JSON_PRETTY, Some(Value::Boolean(true)));
    Ok(())
}

fn jsoncompact(stack: &mut Stack) -> Result<()> {
    stack.set_attribute(ATTRIBUTE_JSON_PRETTY, Some(Value::Boolean(false)));
    Ok(())
}

/// `MAXJSON`: raise (or lower) the JSON size limit, up to the `maxjson` capability value or
/// the hard limit (requires the `maxjson` or `limits` capability).
fn maxjson(stack: &mut Stack) -> Result<()> {
    let hard = stack.hard_limits.max_json;
    let limit = super::limits::pop_limit(
        stack,
        "MAXJSON",
        "maxjson",
        hard,
        "expects a numeric (long) limit.",
    )?;
    stack.limits.max_json = limit;
    Ok(())
}
