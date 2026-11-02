// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Developer automation for `influxftl`, invoked as `cargo xtask <command>`.
//!
//! The commands are deliberately thin wrappers around the standard cargo tooling so that
//! the exact same gates run on a laptop, inside the container (`Dockerfile`, stage `dev`)
//! and in CI. The one piece of real logic — the 100 % coverage gate — lives in
//! [`coverage`] and is unit tested.

pub mod coverage;
pub mod functions_doc;

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Packages that contain `#[kani::proof]` harnesses and are verified by `cargo xtask proof`.
pub const PROOF_PACKAGES: &[&str] = &["warpscript"];

/// Workspace packages excluded from the coverage measurement (tooling, not product code).
pub const COVERAGE_EXCLUDED_PACKAGES: &[&str] = &["xtask"];

/// Errors produced by the xtask commands.
#[derive(Debug)]
pub enum Error {
    /// The command line could not be understood.
    Usage(String),
    /// A spawned tool exited unsuccessfully.
    Tool {
        /// The program that was run.
        program: String,
        /// Its arguments.
        args: Vec<String>,
        /// Exit status or spawn error.
        status: String,
    },
    /// An I/O error while reading or writing files.
    Io(std::io::Error),
    /// The coverage gate failed.
    Coverage(String),
    /// The coverage JSON export could not be parsed.
    Json(serde_json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(msg) => write!(f, "{msg}\n\n{USAGE}"),
            Error::Tool {
                program,
                args,
                status,
            } => {
                write!(f, "`{program} {}` failed: {status}", args.join(" "))
            }
            Error::Io(err) => write!(f, "I/O error: {err}"),
            Error::Coverage(msg) => write!(f, "coverage gate failed:\n{msg}"),
            Error::Json(err) => write!(f, "invalid coverage export: {err}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err)
    }
}

/// Abstraction over process spawning so that the command plumbing can be tested without
/// actually running cargo.
pub trait Runner {
    /// Run `program args…` in the workspace root, streaming its output, failing on non-zero exit.
    fn run(&mut self, program: &str, args: &[&str]) -> Result<(), Error>;
    /// Root directory of the workspace.
    fn workspace_root(&self) -> PathBuf;
}

/// The real [`Runner`]: spawns processes with inherited stdio.
pub struct ProcessRunner;

impl Runner for ProcessRunner {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<(), Error> {
        println!("\n$ {program} {}", args.join(" "));
        let status = Command::new(program)
            .args(args)
            .current_dir(self.workspace_root())
            .status()
            .map_err(|err| Error::Tool {
                program: program.to_owned(),
                args: args.iter().map(|a| (*a).to_owned()).collect(),
                status: err.to_string(),
            })?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::Tool {
                program: program.to_owned(),
                args: args.iter().map(|a| (*a).to_owned()).collect(),
                status: status.to_string(),
            })
        }
    }

    fn workspace_root(&self) -> PathBuf {
        workspace_root()
    }
}

/// The workspace root, derived from this crate's manifest directory (`<root>/xtask`).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

const USAGE: &str = "\
cargo xtask <command>

Commands:
  security-audit   cargo deny check + cargo audit
  check            cargo check --workspace --all-targets --locked
  fmt              cargo fmt --all --check
  clippy           cargo clippy --workspace --all-targets --locked -- -D warnings
  test             cargo test --workspace --locked
  coverage         cargo llvm-cov; fails unless every file is 100% lines/regions/functions
  proof            cargo kani on the packages that carry proof harnesses
  dev-check        security-audit, check, fmt, clippy, test, coverage, proof (in that order)
  functions        regenerate docs/FUNCTIONS.md from the registry and docs/warplib-inventory.tsv
  help             print this message";

/// The xtask commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command_ {
    /// `cargo deny check` + `cargo audit`.
    SecurityAudit,
    /// `cargo check`.
    Check,
    /// `cargo fmt --check`.
    Fmt,
    /// `cargo clippy -D warnings`.
    Clippy,
    /// `cargo test`.
    Test,
    /// Coverage measurement and 100 % gate.
    Coverage,
    /// Kani proofs.
    Proof,
    /// Everything, in the pre-push order.
    DevCheck,
    /// Regenerate `docs/FUNCTIONS.md`.
    Functions,
    /// Print usage.
    Help,
}

impl Command_ {
    /// Parse a command name.
    pub fn parse(name: &str) -> Result<Self, Error> {
        Ok(match name {
            "security-audit" | "audit" => Command_::SecurityAudit,
            "check" => Command_::Check,
            "fmt" => Command_::Fmt,
            "clippy" => Command_::Clippy,
            "test" => Command_::Test,
            "coverage" | "cov" => Command_::Coverage,
            "proof" | "kani" => Command_::Proof,
            "dev-check" | "pre-push" => Command_::DevCheck,
            "functions" => Command_::Functions,
            "help" | "--help" | "-h" => Command_::Help,
            other => return Err(Error::Usage(format!("unknown command `{other}`"))),
        })
    }
}

/// Entry point: parse `args` and execute the corresponding command with `runner`.
pub fn run(args: &[String], runner: &mut dyn Runner) -> Result<(), Error> {
    let Some(name) = args.first() else {
        return Err(Error::Usage("missing command".to_owned()));
    };
    if args.len() > 1 {
        return Err(Error::Usage(format!("unexpected arguments after `{name}`")));
    }
    execute(Command_::parse(name)?, runner)
}

/// Execute one command.
pub fn execute(command: Command_, runner: &mut dyn Runner) -> Result<(), Error> {
    match command {
        Command_::SecurityAudit => security_audit(runner),
        Command_::Check => check(runner),
        Command_::Fmt => fmt(runner),
        Command_::Clippy => clippy(runner),
        Command_::Test => test(runner),
        Command_::Coverage => coverage(runner),
        Command_::Proof => proof(runner),
        Command_::DevCheck => dev_check(runner),
        Command_::Functions => functions(runner),
        Command_::Help => {
            println!("{USAGE}");
            Ok(())
        }
    }
}

fn security_audit(runner: &mut dyn Runner) -> Result<(), Error> {
    runner.run("cargo", &["deny", "--all-features", "check"])?;
    runner.run("cargo", &["audit", "--deny", "warnings"])
}

fn check(runner: &mut dyn Runner) -> Result<(), Error> {
    runner.run(
        "cargo",
        &[
            "check",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
        ],
    )
}

fn fmt(runner: &mut dyn Runner) -> Result<(), Error> {
    runner.run("cargo", &["fmt", "--all", "--check"])
}

fn clippy(runner: &mut dyn Runner) -> Result<(), Error> {
    runner.run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn test(runner: &mut dyn Runner) -> Result<(), Error> {
    runner.run(
        "cargo",
        &["test", "--workspace", "--all-features", "--locked"],
    )
}

fn coverage(runner: &mut dyn Runner) -> Result<(), Error> {
    let root = runner.workspace_root();
    let out_dir = root.join("coverage");
    std::fs::create_dir_all(&out_dir)?;
    let json_path = out_dir.join("coverage.json");
    // `llvm-cov report --html` creates an `html/` directory inside `--output-dir`.
    let html_dir = out_dir.join("html");
    let lcov_path = out_dir.join("lcov.info");

    // 1. Run the instrumented test suite once, keep the raw profiles.
    let mut args = vec![
        "llvm-cov",
        "--workspace",
        "--all-features",
        "--locked",
        "--no-report",
    ];
    for pkg in COVERAGE_EXCLUDED_PACKAGES {
        args.push("--exclude");
        args.push(pkg);
    }
    runner.run("cargo", &args)?;

    // 2. Produce the reports from the collected profiles.
    let json_path_s = path_str(&json_path);
    let out_dir_s = path_str(&out_dir);
    let lcov_path_s = path_str(&lcov_path);
    let ignore = coverage::IGNORE_FILENAME_REGEX;
    runner.run(
        "cargo",
        &[
            "llvm-cov",
            "report",
            "--ignore-filename-regex",
            ignore,
            "--show-missing-lines",
        ],
    )?;
    runner.run(
        "cargo",
        &[
            "llvm-cov",
            "report",
            "--ignore-filename-regex",
            ignore,
            "--json",
            "--summary-only",
            "--output-path",
            &json_path_s,
        ],
    )?;
    runner.run(
        "cargo",
        &[
            "llvm-cov",
            "report",
            "--ignore-filename-regex",
            ignore,
            "--lcov",
            "--output-path",
            &lcov_path_s,
        ],
    )?;
    runner.run(
        "cargo",
        &[
            "llvm-cov",
            "report",
            "--ignore-filename-regex",
            ignore,
            "--html",
            "--output-dir",
            &out_dir_s,
        ],
    )?;

    // 3. Gate: every product source file must be present and at 100 %.
    let export = std::fs::read_to_string(&json_path)?;
    let expected = coverage::expected_source_files(&root)?;
    let report = coverage::evaluate(&export, &root, &expected)?;
    println!("{report}");
    if report.is_complete() {
        println!("coverage gate: OK (100% files, lines, regions, functions)");
        Ok(())
    } else {
        Err(Error::Coverage(format!(
            "{} file(s) below 100% — see {} for details",
            report.failures().len(),
            html_dir.join("index.html").display()
        )))
    }
}

fn proof(runner: &mut dyn Runner) -> Result<(), Error> {
    for pkg in PROOF_PACKAGES {
        runner.run("cargo", &["kani", "-p", pkg])?;
    }
    Ok(())
}

fn dev_check(runner: &mut dyn Runner) -> Result<(), Error> {
    const CHAIN: [Command_; 7] = [
        Command_::SecurityAudit,
        Command_::Check,
        Command_::Fmt,
        Command_::Clippy,
        Command_::Test,
        Command_::Coverage,
        Command_::Proof,
    ];
    for (idx, step) in CHAIN.iter().enumerate() {
        println!("\n==> dev-check step {}/{}: {step:?}", idx + 1, CHAIN.len());
        execute(*step, runner)?;
    }
    println!("\ndev-check: all gates passed — safe to push.");
    Ok(())
}

fn functions(runner: &mut dyn Runner) -> Result<(), Error> {
    let root = runner.workspace_root();
    let inventory = std::fs::read_to_string(root.join("docs/warplib-inventory.tsv"))?;
    let registered = warpscript::functions::registered_names();
    let doc = functions_doc::render(&inventory, &registered);
    std::fs::write(root.join("docs/FUNCTIONS.md"), doc)?;
    println!("docs/FUNCTIONS.md regenerated");
    Ok(())
}

fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests;
