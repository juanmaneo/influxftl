// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Geo Time Series selectors (`class{label=value,label~regexp}`): `GTSHelper.parseLabelsSelectors`,
//! `GTSHelper.buildSelector` and `MetadataSelectorMatcher`, as used by `PARSESELECTOR`,
//! `TOSELECTOR` and `filter.byselector`.
//!
//! Warp 10 is assumed to run with `warp10.absent.label.support` (as the reference oracle does):
//! the selectors `label=` require the label to be absent.

use indexmap::IndexMap;

use crate::error::{Error, Result};
use crate::format::{url_decode, url_encode};
use crate::functions::strings::CompiledPattern;
use crate::gts::Metadata;

/// A class, label or attribute value selector: exact or regular expression.
pub(crate) enum Selector {
    /// `=value` or `value`.
    Exact(String),
    /// `~regexp`.
    Regex(CompiledPattern),
}

impl Selector {
    /// Parse `=value`, `~regexp` or `value`.
    pub(crate) fn parse(selector: &str) -> Result<Selector> {
        Ok(if let Some(exact) = selector.strip_prefix('=') {
            Selector::Exact(exact.to_owned())
        } else if let Some(regex) = selector.strip_prefix('~') {
            Selector::Regex(CompiledPattern::compile(regex)?)
        } else {
            Selector::Exact(selector.to_owned())
        })
    }

    /// Whether `value` matches entirely (`Matcher.matches()`).
    pub(crate) fn matches(&self, value: &str) -> Result<bool> {
        Ok(match self {
            Selector::Exact(exact) => exact == value,
            Selector::Regex(regex) => !regex.full_match(value)?.is_empty(),
        })
    }
}

/// `UnsafeString.split`: every token between the separators, empty ones included.
fn split(s: &str, separator: char) -> Vec<&str> {
    s.split(separator).collect()
}

/// The `ParseException` of a selector token, wrapped like `new WarpScriptException(pe)`.
fn parse_exception(token: &str) -> Error {
    Error::with_cause(
        format!("java.text.ParseException: {token}"),
        Error::runtime("java.text.ParseException", token),
    )
}

/// `GTSHelper.parseLabelsSelectors`: `name=value` / `name~regexp` tokens separated by commas
/// (names and values percent-encoded) to an ordered map from the name to `=value` or
/// `~regexp`. Blank tokens are skipped; a token with neither `=` nor `~` is an error.
pub fn parse_labels_selectors(selectors: &str) -> Result<IndexMap<String, String>> {
    let mut result = IndexMap::new();
    for token in split(selectors, ',') {
        let token = token.trim_matches(|c: char| c <= ' ');
        if token.is_empty() {
            continue;
        }
        let separator = if token.contains('=') {
            '='
        } else if token.contains('~') {
            '~'
        } else {
            return Err(parse_exception(token));
        };
        // Sic: only the first two subtokens are used (`a=b=c` selects `a=b`).
        let subtokens = split(token, separator);
        let name = url_decode(subtokens[0])?;
        let value = url_decode(subtokens.get(1).copied().unwrap_or_default())?;
        result.insert(name, format!("{separator}{value}"));
    }
    Ok(result)
}

/// The line terminators that `.` does not match in a Java regular expression.
fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// `PARSESELECTOR.parse`: `class{labels}` to the decoded class selector and the labels
/// selectors. `None` when the syntax (`^([^{]+)\{(.*)\}$`) does not match.
pub fn parse_selector(selector: &str) -> Option<Result<(String, IndexMap<String, String>)>> {
    let (class, rest) = selector.split_once('{')?;
    let labels = rest.strip_suffix('}')?;
    if class.is_empty() || labels.contains(is_line_terminator) {
        return None;
    }
    Some(url_decode(class).and_then(|class| Ok((class, parse_labels_selectors(labels)?))))
}

/// `GTSHelper.buildSelector(metadata, true)`: the selector of a series, labels sorted by name.
/// A class name starting with `=` or `~` is prefixed with `=`.
pub fn build_selector(metadata: &Metadata) -> String {
    build_selector_ordered(metadata, true)
}

/// `GTSHelper.buildSelector(metadata, sort)`: the labels sorted by name when `sort` is set,
/// in their map order otherwise.
pub fn build_selector_ordered(metadata: &Metadata, sort: bool) -> String {
    let mut out = String::new();
    if let Some(name) = &metadata.name {
        if name.starts_with(['=', '~']) {
            out.push('=');
        }
        out.push_str(&url_encode(name));
    }
    out.push('{');
    let mut labels: Vec<(&String, &String)> = metadata.labels.iter().collect();
    if sort {
        labels.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    }
    let labels: Vec<String> = labels
        .into_iter()
        .map(|(name, value)| format!("{}={}", url_encode(name), url_encode(value)))
        .collect();
    out.push_str(&labels.join(","));
    out.push('}');
    out
}

/// The selectors of a labels or attributes part: the names that must be absent (`name=`) and
/// the patterns of the others.
type Part = (Vec<String>, Vec<(String, Selector)>);

fn part(selectors: &str) -> Result<Part> {
    let mut missing = Vec::new();
    let mut patterns = Vec::new();
    for (name, selector) in parse_labels_selectors(selectors)? {
        if selector == "=" {
            missing.push(name);
        } else {
            patterns.push((name, Selector::parse(&selector)?));
        }
    }
    Ok((missing, patterns))
}

/// `MetadataSelectorMatcher`: a compiled `class{labels}` or `class{labels}{attributes}`
/// selector.
pub struct MetadataSelectorMatcher {
    /// `None` for `~.*`, which matches every named series.
    class: Option<Selector>,
    labels: Part,
    /// The attributes part of an extended selector.
    attributes: Option<Part>,
}

impl MetadataSelectorMatcher {
    /// Compile a selector (`^([^{]+)\{([^}]*)\}(\{([^}]*)\})?$`).
    pub fn new(selector: &str) -> Result<MetadataSelectorMatcher> {
        let invalid = || Error::new("invalid syntax for selector.");
        let (class, rest) = selector.split_once('{').ok_or_else(invalid)?;
        let (labels, rest) = rest.split_once('}').ok_or_else(invalid)?;
        let attributes = match rest {
            "" => None,
            _ => {
                let attributes = rest
                    .strip_prefix('{')
                    .and_then(|r| r.strip_suffix('}'))
                    .filter(|r| !r.contains('}'))
                    .ok_or_else(invalid)?;
                Some(attributes)
            }
        };
        if class.is_empty() {
            return Err(invalid());
        }
        // `=` and `~` (an empty regular expression) both select the empty class name.
        let class = url_decode(class)?;
        let class = match class.as_str() {
            "~.*" => None,
            _ => Some(Selector::parse(&class)?),
        };
        let labels = part(labels)?;
        let attributes = attributes.map(part).transpose()?;
        Ok(MetadataSelectorMatcher {
            class,
            labels,
            attributes,
        })
    }

    /// Whether the metadata of a series matches. A series that was never named never matches.
    /// Without an attributes part, a label selector is checked against the attribute of that
    /// name when the label is missing (and an absent label must be absent from both).
    pub fn matches(&self, metadata: &Metadata) -> Result<bool> {
        let Some(name) = &metadata.name else {
            return Ok(false);
        };
        if let Some(class) = &self.class
            && !class.matches(name)?
        {
            return Ok(false);
        }
        let (labels, attributes) = (&metadata.labels, &metadata.attributes);
        let (missing, patterns) = &self.labels;
        let Some((missing_attributes, attribute_patterns)) = &self.attributes else {
            // Standard selector: a label selector also applies to an attribute.
            if missing
                .iter()
                .any(|k| labels.contains_key(k) || attributes.contains_key(k))
            {
                return Ok(false);
            }
            return all_match(patterns, |k| labels.get(k).or_else(|| attributes.get(k)));
        };
        if missing.iter().any(|k| labels.contains_key(k))
            || missing_attributes
                .iter()
                .any(|k| attributes.contains_key(k))
        {
            return Ok(false);
        }
        Ok(all_match(patterns, |k| labels.get(k))?
            && all_match(attribute_patterns, |k| attributes.get(k))?)
    }
}

/// Whether every pattern matches the value `lookup` finds for its name (a missing value does
/// not match).
fn all_match<'a>(
    patterns: &[(String, Selector)],
    lookup: impl Fn(&str) -> Option<&'a String>,
) -> Result<bool> {
    for (name, selector) in patterns {
        let Some(value) = lookup(name) else {
            return Ok(false);
        };
        if !selector.matches(value)? {
            return Ok(false);
        }
    }
    Ok(true)
}
