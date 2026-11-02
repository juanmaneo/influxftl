// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The 100 % coverage gate.
//!
//! Input: the `llvm-cov export --summary-only` JSON produced by `cargo llvm-cov report --json`.
//! Rule: every product source file must appear in the export and be fully covered on all
//! three axes LLVM reports on stable Rust — **lines**, **regions** and **functions**.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Files excluded from the measurement: test modules, integration tests, benches, examples
/// and the `xtask` tooling crate. Passed verbatim to `--ignore-filename-regex` (POSIX ERE).
pub const IGNORE_FILENAME_REGEX: &str =
    "(/tests\\.rs$|_tests\\.rs$|/proofs\\.rs$|/tests/|/benches/|/examples/|/xtask/|/target/)";

/// Directories (relative to the workspace root) holding product sources.
pub const SOURCE_ROOTS: &[&str] = &["crates"];

/// One coverage axis of one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Axis {
    /// Number of instrumented items.
    pub count: u64,
    /// Number of covered items.
    pub covered: u64,
}

impl Axis {
    /// `true` when every instrumented item was executed (an empty axis is complete).
    pub fn is_complete(self) -> bool {
        self.covered >= self.count
    }

    /// Percentage, 100 when there is nothing to cover.
    pub fn percent(self) -> f64 {
        if self.count == 0 {
            100.0
        } else {
            (self.covered as f64) * 100.0 / (self.count as f64)
        }
    }
}

impl fmt::Display for Axis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{} ({:.2}%)",
            self.covered,
            self.count,
            self.percent()
        )
    }
}

/// Coverage of one source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCoverage {
    /// Path relative to the workspace root.
    pub path: String,
    /// Line coverage.
    pub lines: Axis,
    /// Region coverage.
    pub regions: Axis,
    /// Function coverage.
    pub functions: Axis,
}

impl FileCoverage {
    /// `true` when lines, regions and functions are all fully covered.
    pub fn is_complete(&self) -> bool {
        self.lines.is_complete() && self.regions.is_complete() && self.functions.is_complete()
    }
}

/// Outcome of the gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Every measured file, sorted by path.
    pub files: Vec<FileCoverage>,
    /// Product files that were expected but absent from the export (never instrumented).
    pub missing: Vec<String>,
}

impl Report {
    /// `true` when nothing is missing and every file is complete.
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty() && self.files.iter().all(FileCoverage::is_complete)
    }

    /// Paths of the files that fail the gate (incomplete or missing).
    pub fn failures(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .files
            .iter()
            .filter(|f| !f.is_complete())
            .map(|f| f.path.clone())
            .collect();
        out.extend(self.missing.iter().cloned());
        out
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{:<60} {:>22} {:>22} {:>22}",
            "file", "lines", "regions", "functions"
        )?;
        for file in &self.files {
            let mark = if file.is_complete() { "ok  " } else { "FAIL" };
            writeln!(
                f,
                "{mark} {:<55} {:>22} {:>22} {:>22}",
                file.path,
                file.lines.to_string(),
                file.regions.to_string(),
                file.functions.to_string()
            )?;
        }
        for path in &self.missing {
            writeln!(f, "MISS {path:<55} (not present in the coverage export)")?;
        }
        Ok(())
    }
}

/// Evaluate the gate over an `llvm-cov` JSON export.
///
/// * `export` — the JSON text;
/// * `root` — the workspace root, used to relativise paths;
/// * `expected` — product source files (relative paths) that must all be present.
pub fn evaluate(
    export: &str,
    root: &Path,
    expected: &BTreeSet<String>,
) -> Result<Report, crate::Error> {
    let value: Value = serde_json::from_str(export)?;
    let mut files = Vec::new();
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for entry in &data {
        let entry_files = entry
            .get("files")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for file in &entry_files {
            let Some(filename) = file.get("filename").and_then(Value::as_str) else {
                continue;
            };
            let path = relativise(Path::new(filename), root);
            let Some(summary) = file.get("summary") else {
                continue;
            };
            files.push(FileCoverage {
                path,
                lines: axis(summary, "lines"),
                regions: axis(summary, "regions"),
                functions: axis(summary, "functions"),
            });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let present: BTreeSet<&str> = files.iter().map(|f| f.path.as_str()).collect();
    let missing = expected
        .iter()
        .filter(|p| !present.contains(p.as_str()))
        .cloned()
        .collect();
    Ok(Report { files, missing })
}

fn axis(summary: &Value, key: &str) -> Axis {
    let node = summary.get(key);
    let count = node
        .and_then(|n| n.get("count"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let covered = node
        .and_then(|n| n.get("covered"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Axis { count, covered }
}

/// Make `path` relative to `root` when possible, with `/` separators.
pub fn relativise(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => path.to_string_lossy().replace('\\', "/"),
    }
}

/// Collect the product source files: every `.rs` under `<root>/crates/*/src` that is not a
/// test module (same convention as [`IGNORE_FILENAME_REGEX`]).
pub fn expected_source_files(root: &Path) -> Result<BTreeSet<String>, crate::Error> {
    let mut out = BTreeSet::new();
    for source_root in SOURCE_ROOTS {
        let dir = root.join(source_root);
        if !dir.is_dir() {
            continue;
        }
        for crate_dir in std::fs::read_dir(&dir)? {
            let src = crate_dir?.path().join("src");
            if src.is_dir() {
                walk(&src, root, &mut out)?;
            }
        }
    }
    Ok(out)
}

fn walk(dir: &Path, root: &Path, out: &mut BTreeSet<String>) -> Result<(), crate::Error> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "tests") {
                continue;
            }
            walk(&path, root, out)?;
        } else if is_product_source(&path) {
            out.insert(relativise(&path, root));
        }
    }
    Ok(())
}

/// `true` for `.rs` files that are neither test modules nor Kani proof modules (`proofs.rs`,
/// compiled only under `cargo kani`).
pub fn is_product_source(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    path.extension().is_some_and(|ext| ext == "rs")
        && name != "tests.rs"
        && name != "proofs.rs"
        && !name.ends_with("_tests.rs")
}
