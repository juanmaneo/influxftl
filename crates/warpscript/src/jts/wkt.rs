// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Well-Known Text: the reader behind `GEO.WKT` and the writer behind `->WKT`.
//!
//! Both do what the JTS 1.13 `WKTReader` and `WKTWriter` of Warp 10 do, quirks included: the
//! type names and `EMPTY` are case insensitive, an elevation is read and dropped, the old
//! `MULTIPOINT (1 2, 3 4)` syntax is accepted, whatever follows the geometry is ignored, and
//! a text that does not start with a word is no geometry at all (not an error). Numbers are
//! written with at most 16 fraction digits and never in scientific notation.

use std::iter::Peekable;
use std::str::Chars;

use super::geometry::{Coord, Geometry, Line, Polygon, Ring};
use crate::error::fault::ResourceError;
use crate::error::geometry::{GeometryError, NumberError};
use crate::error::{Error, Result};

/// The deepest nesting of geometry collections the reader follows (a JVM stack overflows
/// somewhere past that, at a depth that depends on its settings).
const MAX_DEPTH: usize = 512;

/// The most fraction digits the writer prints (`WKTWriter`'s `0.################` pattern).
const FRACTION_DIGITS: usize = 16;

/// What the tokenizer (`java.io.StreamTokenizer` set up by `WKTReader`) returns.
#[derive(Clone, Debug, PartialEq)]
enum Token {
    /// A run of letters, digits, `-`, `+`, `.` and the characters from `U+00A0`.
    Word(String),
    /// Any other character that is not blank: `(`, `)`, `,`, `;`…
    Symbol(char),
    /// The end of the text.
    End,
}

/// A word of the grammar (`getNextWord`).
#[derive(Debug, PartialEq)]
enum Word {
    /// `EMPTY`, whatever its case.
    Empty,
    /// `(`.
    Open,
    /// `)`.
    Close,
    /// `,`.
    Comma,
    /// Any other word.
    Other(String),
}

impl Word {
    /// The word as `getNextWord` returns it (`EMPTY` upper case, the others as written).
    fn into_text(self) -> String {
        match self {
            Word::Empty => "EMPTY".to_owned(),
            Word::Open => "(".to_owned(),
            Word::Close => ")".to_owned(),
            Word::Comma => ",".to_owned(),
            Word::Other(word) => word,
        }
    }
}

/// What a text that has no word where one is needed holds there.
struct NotAWord {
    /// The token found, as an error message shows it.
    found: String,
    /// The line of the text.
    line: usize,
}

impl From<NotAWord> for GeometryError {
    fn from(not_a_word: NotAWord) -> Self {
        GeometryError::Expected {
            expected: "word",
            found: not_a_word.found,
            line: not_a_word.line,
        }
    }
}

impl From<NotAWord> for Error {
    fn from(not_a_word: NotAWord) -> Self {
        GeometryError::from(not_a_word).into()
    }
}

/// Whether the tokenizer takes `c` for a blank.
fn is_blank(c: char) -> bool {
    c <= ' '
}

/// Whether the tokenizer takes `c` for a character of a word: the letters and digits of
/// ASCII, `-`, `+`, `.`, the characters of Latin-1 from `U+00A0` and everything above
/// (`StreamTokenizer` takes the characters past `U+00FF` for letters).
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '+' | '.') || c >= '\u{a0}'
}

/// The reader: a tokenizer and the recursive descent over its tokens.
struct Reader<'a> {
    chars: Peekable<Chars<'a>>,
    /// The line of the text the tokenizer is at, from 1.
    line: usize,
    /// The token most recently read (`ttype` and `sval`).
    last: Token,
    /// Whether the next read returns `last` again (`pushBack`).
    pushed_back: bool,
}

impl<'a> Reader<'a> {
    fn new(text: &'a str) -> Reader<'a> {
        Reader {
            chars: text.chars().peekable(),
            line: 1,
            last: Token::End,
            pushed_back: false,
        }
    }

    /// `StreamTokenizer.nextToken`.
    fn next_token(&mut self) -> Token {
        if std::mem::take(&mut self.pushed_back) {
            return self.last.clone();
        }
        let token = self.scan();
        self.last = token.clone();
        token
    }

    fn scan(&mut self) -> Token {
        loop {
            let Some(c) = self.chars.next() else {
                return Token::End;
            };
            if is_blank(c) {
                if c == '\r' {
                    self.line += 1;
                    self.chars.next_if_eq(&'\n');
                } else if c == '\n' {
                    self.line += 1;
                }
            } else if is_word_char(c) {
                let mut word = String::from(c);
                while let Some(next) = self.chars.next_if(|next| is_word_char(*next)) {
                    word.push(next);
                }
                return Token::Word(word);
            } else if c == '#' {
                while self
                    .chars
                    .next_if(|next| !matches!(next, '\n' | '\r'))
                    .is_some()
                {}
            } else {
                return Token::Symbol(c);
            }
        }
    }

    /// `StreamTokenizer.pushBack`.
    fn push_back(&mut self) {
        self.pushed_back = true;
    }

    /// `tokenString`: the last token, as an error message shows it.
    fn token_string(&self) -> String {
        match &self.last {
            Token::End => "End-of-Stream".to_owned(),
            Token::Word(word) => format!("'{word}'"),
            Token::Symbol(symbol) => format!("'{symbol}'"),
        }
    }

    /// `parseErrorExpected`.
    fn expected(&self, expected: &'static str) -> Error {
        GeometryError::Expected {
            expected,
            found: self.token_string(),
            line: self.line,
        }
        .into()
    }

    /// `getNextWord`.
    fn next_word(&mut self) -> std::result::Result<Word, NotAWord> {
        match self.next_token() {
            Token::Word(word) if word.eq_ignore_ascii_case("EMPTY") => Ok(Word::Empty),
            Token::Word(word) => Ok(Word::Other(word)),
            Token::Symbol('(') => Ok(Word::Open),
            Token::Symbol(')') => Ok(Word::Close),
            Token::Symbol(',') => Ok(Word::Comma),
            Token::Symbol(_) | Token::End => Err(NotAWord {
                found: self.token_string(),
                line: self.line,
            }),
        }
    }

    /// `getNextNumber`.
    fn next_number(&mut self) -> Result<f64> {
        match self.next_token() {
            Token::Word(word) => parse_number(&word).map_err(|source| {
                GeometryError::InvalidNumber {
                    text: word,
                    line: self.line,
                    source,
                }
                .into()
            }),
            Token::Symbol(_) | Token::End => Err(self.expected("number")),
        }
    }

    /// `isNumberNext`: whether the next token is a word.
    fn is_number_next(&mut self) -> bool {
        let word = matches!(self.next_token(), Token::Word(_));
        self.push_back();
        word
    }

    /// `getNextEmptyOrOpener`: `true` for `EMPTY`, `false` for `(`.
    fn empty_or_opener(&mut self) -> Result<bool> {
        match self.next_word()? {
            Word::Empty => Ok(true),
            Word::Open => Ok(false),
            Word::Close | Word::Comma | Word::Other(_) => Err(self.expected("EMPTY or (")),
        }
    }

    /// `getNextCloserOrComma`: `true` for `,`, `false` for `)`.
    fn closer_or_comma(&mut self) -> Result<bool> {
        match self.next_word()? {
            Word::Comma => Ok(true),
            Word::Close => Ok(false),
            Word::Empty | Word::Open | Word::Other(_) => Err(self.expected(", or )")),
        }
    }

    /// `getNextCloser`.
    fn closer(&mut self) -> Result<()> {
        match self.next_word()? {
            Word::Close => Ok(()),
            Word::Empty | Word::Open | Word::Comma | Word::Other(_) => Err(self.expected(")")),
        }
    }

    /// `getPreciseCoordinate`: `x y` and an elevation if there is one.
    fn coordinate(&mut self) -> Result<Coord> {
        let x = self.next_number()?;
        let y = self.next_number()?;
        let z = if self.is_number_next() {
            Some(self.next_number()?)
        } else {
            None
        };
        Coord::new(x, y, z)
    }

    /// `getCoordinatesNoLeftParen`: the coordinates up to the closing parenthesis.
    fn coordinates_after_opener(&mut self) -> Result<Vec<Coord>> {
        let mut coords = vec![self.coordinate()?];
        while self.closer_or_comma()? {
            coords.push(self.coordinate()?);
        }
        Ok(coords)
    }

    /// `getCoordinates`: `EMPTY` or a parenthesized list.
    fn coordinates(&mut self) -> Result<Vec<Coord>> {
        if self.empty_or_opener()? {
            Ok(Vec::new())
        } else {
            self.coordinates_after_opener()
        }
    }

    /// `readPointText`.
    fn point_text(&mut self) -> Result<Option<Coord>> {
        if self.empty_or_opener()? {
            return Ok(None);
        }
        let point = self.coordinate()?;
        self.closer()?;
        Ok(Some(point))
    }

    fn line_text(&mut self) -> Result<Line> {
        Line::new(self.coordinates()?)
    }

    fn ring_text(&mut self) -> Result<Ring> {
        Ring::new(self.coordinates()?)
    }

    /// `readPolygonText`.
    fn polygon_text(&mut self) -> Result<Polygon> {
        if self.empty_or_opener()? {
            return Polygon::new(Ring::closed(Vec::new()), Vec::new());
        }
        let shell = self.ring_text()?;
        let mut holes = Vec::new();
        while self.closer_or_comma()? {
            holes.push(self.ring_text()?);
        }
        Polygon::new(shell, holes)
    }

    /// `readMultiPointText`: the points are parenthesized or, in the old syntax, not.
    fn multi_point_text(&mut self) -> Result<Geometry> {
        if self.empty_or_opener()? {
            return Ok(Geometry::MultiPoint(Vec::new()));
        }
        let old_syntax = self.next_word()? != Word::Open;
        self.push_back();
        if old_syntax {
            let points = self.coordinates_after_opener()?;
            return Ok(Geometry::MultiPoint(points.into_iter().map(Some).collect()));
        }
        let mut points = vec![self.point_text()?];
        while self.closer_or_comma()? {
            points.push(self.point_text()?);
        }
        Ok(Geometry::MultiPoint(points))
    }

    /// The elements of a multi-geometry, read by `element` between parentheses.
    fn elements<T>(&mut self, mut element: impl FnMut(&mut Self) -> Result<T>) -> Result<Vec<T>> {
        if self.empty_or_opener()? {
            return Ok(Vec::new());
        }
        let mut elements = vec![element(self)?];
        while self.closer_or_comma()? {
            elements.push(element(self)?);
        }
        Ok(elements)
    }

    /// `readGeometryTaggedText`: `None` when the text does not start with a word.
    fn tagged_text(&mut self, depth: usize) -> Result<Option<Geometry>> {
        let name = match self.next_word() {
            Ok(word) => word.into_text(),
            // JTS answers a text that starts with something else than a word with no geometry
            // at all, so that a collection of such texts is the failure of its constructor.
            Err(NotAWord { .. }) => return Ok(None),
        };
        let geometry = match name.to_ascii_uppercase().as_str() {
            "POINT" => Geometry::Point(self.point_text()?),
            "LINESTRING" => Geometry::LineString(self.line_text()?),
            "LINEARRING" => Geometry::LinearRing(self.ring_text()?),
            "POLYGON" => Geometry::Polygon(self.polygon_text()?),
            "MULTIPOINT" => self.multi_point_text()?,
            "MULTILINESTRING" => Geometry::MultiLineString(self.elements(Self::line_text)?),
            "MULTIPOLYGON" => Geometry::MultiPolygon(self.elements(Self::polygon_text)?),
            "GEOMETRYCOLLECTION" => {
                if depth >= MAX_DEPTH {
                    return Err(ResourceError::StackOverflow.into());
                }
                Geometry::collection(self.elements(|reader| reader.tagged_text(depth + 1))?)?
            }
            _ => {
                return Err(GeometryError::UnknownType {
                    name,
                    line: self.line,
                }
                .into());
            }
        };
        Ok(Some(geometry))
    }
}

/// `Double.parseDouble` of a word, as the callers that then refuse `NaN` and `Infinity` need it.
///
/// The decimal forms Java accepts (an optional sign, digits with an optional point, an optional
/// exponent, an optional `f` or `d` suffix) and the `NaN` and `Infinity` of the callers that
/// then refuse them; the hexadecimal floating point literals Java also accepts are not read.
pub(super) fn parse_number(word: &str) -> std::result::Result<f64, NumberError> {
    if word.eq_ignore_ascii_case("NaN") {
        return Ok(f64::NAN);
    }
    let unsigned = word.strip_prefix(['+', '-']).unwrap_or(word);
    let magnitude = match unsigned {
        "NaN" => f64::NAN,
        "Infinity" => f64::INFINITY,
        decimal => parse_decimal(decimal)?,
    };
    Ok(if word.starts_with('-') {
        -magnitude
    } else {
        magnitude
    })
}

/// A decimal number without its sign: `digits[.digits][e[sign]digits][f|d]`, at least one digit
/// around the point.
fn parse_decimal(text: &str) -> std::result::Result<f64, NumberError> {
    let body = text.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(text);
    // The float parser of Rust reads `inf` and `nan` too, which Java refuses: a decimal form
    // starts with a digit or a point and has nothing else but digits, the point, the exponent
    // and its sign.
    let starts_as_decimal = body.starts_with(|c: char| c.is_ascii_digit() || c == '.');
    let only_decimal_characters = body
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'));
    if !starts_as_decimal || !only_decimal_characters {
        return Err(NumberError::NotDecimal);
    }
    Ok(body.parse()?)
}

/// Read a geometry from WKT: `None` when the text does not start with a word.
///
/// # Errors
///
/// The `ParseException` of the grammar ([`GeometryError::Expected`],
/// [`GeometryError::InvalidNumber`], [`GeometryError::UnknownType`]), the `IllegalArgumentException`
/// of a ring or line string that is not valid, [`NotFinite`](crate::finite::NotFinite) for a
/// coordinate that is not a finite number and [`ResourceError::StackOverflow`] for collections
/// nested too deep.
pub fn read(text: &str) -> Result<Option<Geometry>> {
    Reader::new(text).tagged_text(0)
}

/// `Geometry.toText`: the geometry as WKT.
///
/// # Errors
///
/// [`Error::NullPointer`] for a multi-point of which some points are empty and others are not
/// (JTS writes the coordinate of each point).
pub fn write(geometry: &Geometry) -> Result<String> {
    let mut out = String::new();
    write_tagged(geometry, &mut out)?;
    Ok(out)
}

fn write_tagged(geometry: &Geometry, out: &mut String) -> Result<()> {
    let (tag, empty) = match geometry {
        Geometry::Point(_) => ("POINT ", false),
        Geometry::LineString(_) => ("LINESTRING ", false),
        Geometry::LinearRing(_) => ("LINEARRING ", false),
        Geometry::Polygon(_) => ("POLYGON ", false),
        Geometry::MultiPoint(_) => ("MULTIPOINT ", geometry.is_empty()),
        Geometry::MultiLineString(_) => ("MULTILINESTRING ", geometry.is_empty()),
        Geometry::MultiPolygon(_) => ("MULTIPOLYGON ", geometry.is_empty()),
        Geometry::Collection(_) => ("GEOMETRYCOLLECTION ", geometry.is_empty()),
    };
    out.push_str(tag);
    if empty {
        out.push_str("EMPTY");
        return Ok(());
    }
    match geometry {
        Geometry::Point(None) => out.push_str("EMPTY"),
        Geometry::Point(Some(point)) => {
            out.push('(');
            write_coord(point, out);
            out.push(')');
        }
        Geometry::LineString(line) => write_coords(line.coords(), out),
        Geometry::LinearRing(ring) => write_coords(ring.coords(), out),
        Geometry::Polygon(polygon) => write_polygon(polygon, out),
        Geometry::MultiPoint(points) => write_list(points, out, |point, out| {
            let point = point.as_ref().ok_or(Error::NullPointer)?;
            out.push('(');
            write_coord(point, out);
            out.push(')');
            Ok(())
        })?,
        Geometry::MultiLineString(lines) => {
            write_each(lines, out, |line, out| write_coords(line.coords(), out));
        }
        Geometry::MultiPolygon(polygons) => write_each(polygons, out, write_polygon),
        Geometry::Collection(elements) => write_list(elements, out, write_tagged)?,
    }
    Ok(())
}

/// A parenthesized list of `items`, written by `write_item`, separated by commas.
fn write_list<T>(
    items: &[T],
    out: &mut String,
    mut write_item: impl FnMut(&T, &mut String) -> Result<()>,
) -> Result<()> {
    out.push('(');
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        write_item(item, out)?;
    }
    out.push(')');
    Ok(())
}

/// A parenthesized list of `items` that cannot fail to be written.
fn write_each<T>(items: &[T], out: &mut String, mut write_item: impl FnMut(&T, &mut String)) {
    out.push('(');
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        write_item(item, out);
    }
    out.push(')');
}

/// The coordinates of a line or a ring, `EMPTY` for none.
fn write_coords(coords: &[Coord], out: &mut String) {
    if coords.is_empty() {
        out.push_str("EMPTY");
        return;
    }
    out.push('(');
    for (index, coord) in coords.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        write_coord(coord, out);
    }
    out.push(')');
}

/// The rings of a polygon, `EMPTY` for no shell.
fn write_polygon(polygon: &Polygon, out: &mut String) {
    if polygon.shell().coords().is_empty() {
        out.push_str("EMPTY");
        return;
    }
    out.push('(');
    write_coords(polygon.shell().coords(), out);
    for hole in polygon.holes() {
        out.push_str(", ");
        write_coords(hole.coords(), out);
    }
    out.push(')');
}

/// `x y`: the elevation is not written.
fn write_coord(coord: &Coord, out: &mut String) {
    out.push_str(&format_number(coord.x));
    out.push(' ');
    out.push_str(&format_number(coord.y));
}

/// The digits of `value` (finite, not negative) as `(digits, exponent)`: the shortest digits
/// that read back as `value` and the power of ten of the first one.
fn shortest_digits(value: f64) -> (Vec<u8>, i32) {
    let mut digits = Vec::new();
    let mut exponent = 0;
    let mut negative_exponent = false;
    let mut in_exponent = false;
    // `{:e}` is `d[.ddd]e[-]x`.
    for c in format!("{value:e}").bytes() {
        match (in_exponent, c) {
            (false, b'e') => in_exponent = true,
            (false, b'.') => {}
            (false, digit) => digits.push(digit),
            (true, b'-') => negative_exponent = true,
            (true, digit) => exponent = exponent * 10 + i32::from(digit - b'0'),
        }
    }
    (
        digits,
        if negative_exponent {
            -exponent
        } else {
            exponent
        },
    )
}

/// `DecimalFormat("0.################")` of a finite number, rounding half even.
///
/// Like Java, it rounds the shortest digits of the number, and tells the ties from the digits
/// that were rounded by the exact value of the number.
pub fn format_number(value: f64) -> String {
    let (digits, exponent) = shortest_digits(value.abs());
    let (mut whole, mut fraction): (Vec<u8>, Vec<u8>) = if exponent >= 0 {
        let whole_len = exponent as usize + 1;
        let mut padded = digits;
        if padded.len() < whole_len {
            padded.resize(whole_len, b'0');
        }
        let fraction = padded.split_off(whole_len);
        (padded, fraction)
    } else {
        let zeros = vec![b'0'; (-exponent - 1) as usize];
        (vec![b'0'], [zeros, digits].concat())
    };
    if fraction.len() > FRACTION_DIGITS {
        let tail = fraction.split_off(FRACTION_DIGITS);
        if rounds_up(&fraction, &tail, value.abs()) {
            increment(&mut whole, &mut fraction);
        }
    }
    while fraction.last() == Some(&b'0') {
        fraction.pop();
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let text = |digits: &[u8]| {
        digits
            .iter()
            .map(|digit| char::from(*digit))
            .collect::<String>()
    };
    if fraction.is_empty() {
        format!("{sign}{}", text(&whole))
    } else {
        format!("{sign}{}.{}", text(&whole), text(&fraction))
    }
}

/// Whether the digits `tail` that follow the 16 `fraction` digits kept round them up (half
/// even). Rounding only happens to numbers below 1, of which the integer part is `0`.
fn rounds_up(fraction: &[u8], tail: &[u8], value: f64) -> bool {
    if tail == b"5" {
        // Exactly half of the shortest digits: the exact value tells whether they were
        // rounded. Both texts are `0.` and fraction digits, padded to the same length. The
        // binary expansion of a number that has 17 fraction digits ends before the 120th.
        let exact = format!("{value:.120}").into_bytes();
        let mut shortest = [b"0.", fraction, b"5"].concat();
        shortest.resize(exact.len(), b'0');
        match exact.cmp(&shortest) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            // ASCII digits are odd when the digits are.
            std::cmp::Ordering::Equal => fraction.last().is_some_and(|digit| digit % 2 == 1),
        }
    } else {
        tail.first()
            .is_some_and(|first| *first > b'5' || (*first == b'5' && tail.len() > 1))
    }
}

/// Add one unit in the last place of `whole.fraction`.
pub(super) fn increment(whole: &mut Vec<u8>, fraction: &mut [u8]) {
    for digit in fraction.iter_mut().rev().chain(whole.iter_mut().rev()) {
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            return;
        }
    }
    whole.insert(0, b'1');
}
