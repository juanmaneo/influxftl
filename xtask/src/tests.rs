// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::coverage::{self, Axis};
use super::functions_doc;
use super::{Command_, Error, Runner};

struct FakeRunner {
    calls: Vec<String>,
    fail_on: Option<String>,
}

impl Runner for FakeRunner {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<(), Error> {
        let call = format!("{program} {}", args.join(" "));
        self.calls.push(call.clone());
        match &self.fail_on {
            Some(needle) if call.contains(needle.as_str()) => Err(Error::Tool {
                program: program.to_owned(),
                args: args.iter().map(|a| (*a).to_owned()).collect(),
                status: "exit status: 1".to_owned(),
            }),
            _ => Ok(()),
        }
    }

    fn workspace_root(&self) -> PathBuf {
        super::workspace_root()
    }
}

fn fake() -> FakeRunner {
    FakeRunner {
        calls: Vec::new(),
        fail_on: None,
    }
}

#[test]
fn parses_every_command_name() {
    for (name, expected) in [
        ("security-audit", Command_::SecurityAudit),
        ("audit", Command_::SecurityAudit),
        ("check", Command_::Check),
        ("fmt", Command_::Fmt),
        ("clippy", Command_::Clippy),
        ("test", Command_::Test),
        ("coverage", Command_::Coverage),
        ("cov", Command_::Coverage),
        ("proof", Command_::Proof),
        ("kani", Command_::Proof),
        ("dev-check", Command_::DevCheck),
        ("pre-push", Command_::DevCheck),
        ("functions", Command_::Functions),
        ("help", Command_::Help),
        ("-h", Command_::Help),
    ] {
        assert_eq!(Command_::parse(name).unwrap(), expected, "{name}");
    }
    assert!(matches!(Command_::parse("bogus"), Err(Error::Usage(_))));
}

#[test]
fn run_rejects_missing_or_extra_arguments() {
    let mut runner = fake();
    let err = super::run(&[], &mut runner).unwrap_err();
    assert!(err.to_string().contains("missing command"));
    let err = super::run(&["check".into(), "extra".into()], &mut runner).unwrap_err();
    assert!(err.to_string().contains("unexpected arguments"));
    assert_eq!(runner.calls, Vec::<String>::new());
}

#[test]
fn simple_commands_spawn_the_expected_tools() {
    let mut runner = fake();
    super::run(&["check".into()], &mut runner).unwrap();
    super::run(&["fmt".into()], &mut runner).unwrap();
    super::run(&["clippy".into()], &mut runner).unwrap();
    super::run(&["test".into()], &mut runner).unwrap();
    super::run(&["audit".into()], &mut runner).unwrap();
    super::run(&["proof".into()], &mut runner).unwrap();
    super::run(&["help".into()], &mut runner).unwrap();
    assert_eq!(
        runner.calls,
        vec![
            "cargo check --workspace --all-targets --all-features --locked",
            "cargo fmt --all --check",
            "cargo clippy --workspace --all-targets --all-features --locked -- -D warnings",
            "cargo test --workspace --all-features --locked",
            "cargo deny --all-features check",
            "cargo audit --deny warnings",
            "cargo kani -p warpscript",
        ]
    );
}

#[test]
fn dev_check_stops_at_the_first_failing_gate() {
    let mut runner = FakeRunner {
        calls: Vec::new(),
        fail_on: Some("fmt".to_owned()),
    };
    let err = super::run(&["dev-check".into()], &mut runner).unwrap_err();
    assert!(err.to_string().contains("cargo fmt --all --check"), "{err}");
    // security-audit (2 calls) + check + fmt
    assert_eq!(runner.calls.len(), 4);
    assert!(!runner.calls.iter().any(|c| c.contains("clippy")));
}

#[test]
fn error_display_covers_every_variant() {
    let io = Error::from(std::io::Error::other("disk"));
    assert!(io.to_string().contains("disk"));
    let json = Error::from(serde_json::from_str::<serde_json::Value>("{").unwrap_err());
    assert!(json.to_string().contains("invalid coverage export"));
    assert!(
        Error::Coverage("x".into())
            .to_string()
            .contains("coverage gate failed")
    );
    assert!(
        Error::Usage("u".into())
            .to_string()
            .contains("cargo xtask <command>")
    );
    let tool = Error::Tool {
        program: "p".into(),
        args: vec!["a".into()],
        status: "s".into(),
    };
    assert_eq!(tool.to_string(), "`p a` failed: s");
    assert!(format!("{tool:?}").contains("Tool"));
}

#[test]
fn axis_arithmetic() {
    assert!(
        Axis {
            count: 0,
            covered: 0
        }
        .is_complete()
    );
    assert!(
        Axis {
            count: 3,
            covered: 3
        }
        .is_complete()
    );
    assert!(
        !Axis {
            count: 4,
            covered: 3
        }
        .is_complete()
    );
    assert_eq!(
        Axis {
            count: 0,
            covered: 0
        }
        .percent(),
        100.0
    );
    assert_eq!(
        Axis {
            count: 4,
            covered: 1
        }
        .percent(),
        25.0
    );
    assert_eq!(
        Axis {
            count: 4,
            covered: 1
        }
        .to_string(),
        "1/4 (25.00%)"
    );
}

const EXPORT: &str = r#"{"data":[{"files":[
  {"filename":"/ws/crates/a/src/lib.rs","summary":{"lines":{"count":10,"covered":10,"percent":100},"regions":{"count":5,"covered":5,"percent":100},"functions":{"count":2,"covered":2,"percent":100}}},
  {"filename":"/ws/crates/a/src/partial.rs","summary":{"lines":{"count":10,"covered":9,"percent":90},"regions":{"count":5,"covered":5,"percent":100},"functions":{"count":2,"covered":2,"percent":100}}},
  {"filename":"/ws/crates/a/src/nosummary.rs"},
  {"nofilename":true}
],"totals":{}}],"type":"llvm.coverage.json.export","version":"2.0.1"}"#;

#[test]
fn coverage_gate_detects_partial_and_missing_files() {
    let root = Path::new("/ws");
    let expected: BTreeSet<String> = [
        "crates/a/src/lib.rs",
        "crates/a/src/partial.rs",
        "crates/a/src/never.rs",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let report = coverage::evaluate(EXPORT, root, &expected).unwrap();
    assert_eq!(report.files.len(), 2);
    assert_eq!(report.missing, vec!["crates/a/src/never.rs".to_owned()]);
    assert!(!report.is_complete());
    assert_eq!(
        report.failures(),
        vec![
            "crates/a/src/partial.rs".to_owned(),
            "crates/a/src/never.rs".to_owned()
        ]
    );
    let text = report.to_string();
    assert!(text.contains("ok   crates/a/src/lib.rs"));
    assert!(text.contains("FAIL crates/a/src/partial.rs"));
    assert!(text.contains("MISS crates/a/src/never.rs"));
    assert!(format!("{report:?}").contains("FileCoverage"));
}

#[test]
fn coverage_gate_passes_when_everything_is_complete() {
    let export = r#"{"data":[{"files":[{"filename":"/ws/crates/a/src/lib.rs","summary":{"lines":{"count":1,"covered":1},"regions":{"count":1,"covered":1},"functions":{"count":1,"covered":1}}}]}]}"#;
    let expected: BTreeSet<String> = ["crates/a/src/lib.rs".to_owned()].into_iter().collect();
    let report = coverage::evaluate(export, Path::new("/ws"), &expected).unwrap();
    assert!(report.is_complete());
    assert_eq!(report.failures().len(), 0);
    assert!(coverage::evaluate("{", Path::new("/ws"), &expected).is_err());
    let empty = coverage::evaluate("{}", Path::new("/ws"), &BTreeSet::new()).unwrap();
    assert!(empty.is_complete());
}

#[test]
fn relativise_handles_paths_outside_the_root() {
    assert_eq!(
        coverage::relativise(Path::new("/ws/a/b.rs"), Path::new("/ws")),
        "a/b.rs"
    );
    assert_eq!(
        coverage::relativise(Path::new("/other/b.rs"), Path::new("/ws")),
        "/other/b.rs"
    );
}

#[test]
fn product_source_detection_follows_the_convention() {
    assert!(coverage::is_product_source(Path::new("src/lib.rs")));
    assert!(!coverage::is_product_source(Path::new("src/tests.rs")));
    assert!(!coverage::is_product_source(Path::new(
        "src/lexer_tests.rs"
    )));
    assert!(!coverage::is_product_source(Path::new("src/data.json")));
    assert!(!coverage::is_product_source(Path::new("src/proofs.rs")));
    assert!(!coverage::is_product_source(Path::new("/")));
}

#[test]
fn expected_source_files_walks_the_real_workspace() {
    let root = super::workspace_root();
    let files = coverage::expected_source_files(&root).unwrap();
    assert!(files.contains("crates/warpscript/src/lib.rs"), "{files:?}");
    assert!(
        files
            .iter()
            .all(|f| !f.ends_with("/tests.rs") && !f.contains("/tests/"))
    );
    let none = coverage::expected_source_files(Path::new("/nonexistent-root")).unwrap();
    assert!(none.is_empty());
}

#[test]
fn functions_doc_renders_groups_and_extras() {
    let inventory = "# comment\nDUP\tDUP\nmapper.sum\tMapperSum\nreducer.max\tReducerMax\nbucketizer.mean\tBucketizerMean\nfilter.bylabels\tFilterByLabels\nop.add\tOpAdd\nfiller.next\tFillerNext\nnoclass\n\n";
    let doc = functions_doc::render(inventory, &["DUP", "EXTRA", "op.add"]);
    assert!(doc.contains("**2 / 8 functions ported (25.0 %).**"));
    assert!(doc.contains("## Functions (1 / 2)"));
    assert!(doc.contains("| `DUP` | `DUP` | ✅ ported |"));
    assert!(doc.contains("| `mapper.sum` | `MapperSum` | ❌ todo |"));
    assert!(doc.contains("| `noclass` | `` | ❌ todo |"));
    assert!(doc.contains("## N-ary operators (1 / 1)"));
    assert!(doc.contains("- `EXTRA`"));
    assert!(functions_doc::render("", &[]).contains("**0 / 0 functions ported (100.0 %).**"));
    let entries = functions_doc::parse_inventory("A|B\tC");
    assert_eq!(entries[0].name, "A|B");
    assert!(functions_doc::render("A|B\tC", &[]).contains("`A\\|B`"));
    assert_eq!(functions_doc::group_of("filler.next"), "Fillers");
    assert_eq!(functions_doc::group_of("filter.x"), "Filters");
    assert_eq!(functions_doc::group_of("bucketizer.x"), "Bucketizers");
    assert_eq!(functions_doc::group_of("reducer.x"), "Reducers");
    assert_eq!(functions_doc::group_of("a.b"), "Functions");
    assert!(format!("{:?}", entries[0]).contains("InventoryEntry"));
}
