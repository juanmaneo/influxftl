// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the platform functions.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const ONE: &str = r"1";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template.replace("@ONE", ONE)
}

/// The stack left by `script`, top first, as the JSON array of the Warp 10 exec endpoint,
/// or the error message.
fn stack_json(script: &str) -> Result<String, String> {
    let mut stack = test_stack();
    stack
        .exec_checked(&format!("{script} DEPTH ->LIST REVERSE ->JSON"))
        .map_err(|err| err.to_string())?;
    match stack.pop().expect("pop should succeed") {
        Value::String(json) => Ok(json.to_string()),
        other => panic!("not a string: {other:?}"),
    }
}

/// Check every case, reporting all the mismatches at once.
fn check(cases: &[(&str, Result<&str, &str>)]) {
    let mut failures = Vec::new();
    for (template, expected) in cases {
        let script = expand(template);
        match (expected, stack_json(&script)) {
            (Ok(json), Ok(actual)) if actual == *json => {}
            (Err(message), Err(actual)) if actual.contains(message) => {}
            (expected, actual) => failures.push(format!(
                "{template}\n  expected {expected:?}\n  actual   {actual:?}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn platform() {
    check(&[
        (r"UPDATEOFF", Err(r"UPDATEOFF missing manager capability.")),
        (r"UPDATEON", Err(r"UPDATEON missing manager capability.")),
        (r"METAOFF", Err(r"METAOFF missing manager capability.")),
        (r"METAON", Err(r"METAON missing manager capability.")),
        (r"DELETEOFF", Err(r"DELETEOFF missing manager capability.")),
        (r"DELETEON", Err(r"DELETEON missing manager capability.")),
        (r"[[]]", Ok(r"[[]]")),
        (r"WFON", Ok(r"[]")),
        (r"WFOFF", Ok(r"[]")),
        (r"WF.GETREPOS", Ok(r"[[]]")),
        (
            r"'x' WF.ADDREPO",
            Err(r"WF.ADDREPO expects the capability 'wfset' to be set."),
        ),
        (
            r"NULL WF.SETREPOS",
            Err(r"WF.SETREPOS expects the capability 'wfset' to be set."),
        ),
        (
            r"'x' TOKENINFO",
            Ok(
                r#"[{"ReadTokenDecodeError":"Invalid token.","WriteTokenDecodeError":"Invalid token."}]"#,
            ),
        ),
        (
            r"1 TOKENINFO",
            Err(r"TOKENINFO expects a string on top of the stack."),
        ),
        (r"TOKENINFO", Err(r"Empty stack.")),
        (
            r"'tok' 'hook' GETHOOK",
            Err(r"io.warp10.script.WarpScriptException: Invalid token. (Invalid token.)"),
        ),
        (
            r"1 GETHOOK",
            Err(r"GETHOOK expects a hook name on top of the stack."),
        ),
        (
            r"'x' 1 GETHOOK",
            Err(r"GETHOOK expects a hook name on top of the stack."),
        ),
        (r"GETHOOK", Err(r"Empty stack.")),
        (r"'x' GETHOOK", Err(r"Empty stack.")),
        (r"{ 'token' 'x' } KVLOAD", Err(r"Invalid token.")),
        (r"{ } KVLOAD", Err(r"KVLOAD expects a token under 'token'.")),
        (r"'x' KVLOAD", Err(r"KVLOAD operates on a map.")),
        (
            r"{ 'token' 1 } KVLOAD",
            Err(r"KVLOAD expects a token under 'token'."),
        ),
        (r"KVLOAD", Err(r"Empty stack.")),
        (r"{ 'k' 'v' } 'tok' KVSTORE", Err(r"Invalid token.")),
        (r"1 KVSTORE", Err(r"KVSTORE missing token.")),
        (r"KVSTORE", Err(r"Empty stack.")),
        (r"1 RUNNERNONCE", Err(r"RUNNERNONCE expects a String.")),
        (r"RUNNERNONCE", Err(r"Empty stack.")),
        (
            r"1 RUNNERAT",
            Err(r"RUNNERAT requires capability runner.reschedule.min.period."),
        ),
        (
            r"RUNNERAT",
            Err(r"RUNNERAT requires capability runner.reschedule.min.period."),
        ),
        (
            r"1 RUNNERIN",
            Err(r"RUNNERIN requires capability runner.reschedule.min.period."),
        ),
        (
            r"ACCEL.REPORT",
            Ok(
                r#"[{"status":false,"cache":false,"persist":true,"accelerated":false,"chunkcount":0,"chunkspan":0,"defaults.read":["cache","persist"],"defaults.write":["cache","persist"],"defaults.delete":["cache","persist"]}]"#,
            ),
        ),
        (r"ACCEL.CACHE", Ok(r"[]")),
        (r"ACCEL.NOCACHE", Ok(r"[]")),
        (r"ACCEL.PERSIST", Ok(r"[]")),
        (r"ACCEL.NOPERSIST", Ok(r"[]")),
        (r"'x' LOGMSG", Ok(r"[]")),
        (r"LOGMSG", Err(r"Empty stack.")),
        (r"NOLOG", Ok(r"[]")),
        (r"LOGINIT", Ok(r"[]")),
        (r"WSAUDIT", Ok(r"[[]]")),
        (r"WSAUDITMODE", Ok(r"[]")),
        (r"PIGSCHEMA", Ok(r#"["()"]"#)),
        (
            r"'x' 2 PIGSCHEMA",
            Ok(r#"["(top: long, l2: chararray)",2,"x"]"#),
        ),
        (
            r"1 'x' 'y' 2.0 T [ ] { } NULL PIGSCHEMA",
            Ok(
                r#"["(top: bytearray, l2: map:[], l3: tuple:(), l4: boolean, l5: double, l6: chararray, l7: chararray, l8: long)",null,{},[],true,2.0,"y","x",1]"#,
            ),
        ),
        (
            r"'1.5' ->BD [ 1 ] ->SET '00' HEX-> PIGSCHEMA",
            Ok(r#"["(top: bytearray, l2: bag{}, l3: bigdecimal)","\u0000",[1],1.5]"#),
        ),
        (
            r"1 'x' GETHOOK",
            Err(r"GETHOOK expects a read token below the hook name."),
        ),
        (
            r"{ } SETMACROCONFIG",
            Err(r"SETMACROCONFIG missing capability."),
        ),
        (
            r"SETMACROCONFIG",
            Err(r"SETMACROCONFIG missing capability."),
        ),
    ]);
}

/// The functions that depend on capabilities and on the runtime's absence of tokens.
#[test]
fn platform_with_capabilities() {
    let run = |caps: &[(&'static str, &'static str)], script: &str| {
        let mut stack = test_stack();
        for (name, value) in caps {
            stack
                .capabilities_mut()
                .insert((*name).to_owned(), (*value).to_owned());
        }
        stack
            .exec_checked(&format!("{script} DEPTH ->LIST REVERSE ->JSON"))
            .map(|()| stack.pop().expect("pop should succeed").to_string())
            .map_err(|err| err.to_string())
    };
    let manager = [("manager", "")];
    let wf = [("wfset", ""), ("wfget", "")];
    let config = [("setmacroconfig", "a@.*")];
    let runner = |value: &'static str| [("runner.reschedule.min.period", value)];
    assert!(
        run(&manager, "UPDATEON UPDATEOFF")
            .expect_err("the value should fail")
            .contains("Empty stack.")
    );
    assert!(
        run(&[], "1 DUMP")
            .expect_err("the value should fail")
            .contains("DUMP missing token.")
    );
    assert!(
        run(&[], "'x' DUMP")
            .expect_err("run should fail")
            .contains("Invalid token.")
    );
    assert!(
        run(&wf, "WF.ADDREPO")
            .expect_err("run should fail")
            .contains("Empty stack.")
    );
    assert!(
        run(&wf, "WF.SETREPOS")
            .expect_err("the value should fail")
            .contains("Empty stack.")
    );
    assert!(
        run(&runner("1000"), "RUNNERIN")
            .expect_err("the value should fail")
            .contains("Empty stack.")
    );
    assert!(
        run(&config, "SETMACROCONFIG")
            .expect_err("the value should fail")
            .contains("Empty stack.")
    );
    assert!(
        run(&config, "'a@m' SETMACROCONFIG")
            .expect_err("the value should fail")
            .contains("Empty stack.")
    );
    let long = format!("{}@x", "a".repeat(46));
    assert!(
        run(
            &[("setmacroconfig", "(a|a)*\\1b@x")],
            &format!("'v' '{long}' SETMACROCONFIG")
        )
        .expect_err("the value should fail")
        .contains("Max limit for backtracking")
    );
    assert_eq!(
        run(&manager, "'maintenance' UPDATEOFF METAON 'x' DELETEOFF").expect("run should succeed"),
        "[]"
    );
    assert_eq!(
        run(
            &wf,
            "'https://a' WF.ADDREPO 'https://b' WF.ADDREPO WF.GETREPOS"
        )
        .expect("the value should succeed"),
        r#"[["https://a","https://b"]]"#
    );
    assert_eq!(
        run(&wf, "[ 'u' ] WF.SETREPOS WF.GETREPOS").expect("run should succeed"),
        r#"[["u"]]"#
    );
    assert_eq!(
        run(&wf, "[ 'u' ] WF.SETREPOS NULL WF.SETREPOS WF.GETREPOS").expect("run should succeed"),
        "[[]]"
    );
    assert!(
        run(&wf, "[ 1 ] WF.SETREPOS")
            .expect_err("the value should fail")
            .contains("expects a list of repository URLs")
    );
    assert!(
        run(&wf, "1 WF.SETREPOS")
            .expect_err("the value should fail")
            .contains("expects a list of repository URLs")
    );
    assert!(
        run(&wf, "1 WF.ADDREPO")
            .expect_err("the value should fail")
            .contains("expects a repository URL")
    );
    assert_eq!(
        run(&[("wfset", "")], "'u' WF.ADDREPO WF.GETREPOS").expect("run should succeed"),
        "[[]]"
    );
    assert!(
        run(&runner("x"), "1 RUNNERIN")
            .expect_err("the value should fail")
            .contains("'x' is not a valid LONG")
    );
    assert!(
        run(&runner("0"), "1 RUNNERIN")
            .expect_err("the value should fail")
            .contains("strictly greater than 0 ms")
    );
    assert!(
        run(&runner("1000"), "'x' RUNNERIN")
            .expect_err("the value should fail")
            .contains("expects a LONG period")
    );
    assert!(
        run(&runner("1000"), "'x' RUNNERAT")
            .expect_err("the value should fail")
            .contains("expects a LONG timestamp")
    );
    assert!(
        run(&runner("1000"), "1 s RUNNERIN")
            .expect("run should succeed")
            .contains("[]")
    );
    assert!(
        run(&runner("1000"), "1 ms RUNNERIN")
            .expect_err("the value should fail")
            .contains("cannot set period below 1000 ms")
    );
    assert!(
        run(&runner("1000"), "NOW RUNNERAT")
            .expect_err("the value should fail")
            .contains("cannot schedule a task before now + 1000 ms")
    );
    assert_eq!(
        run(&runner("1000"), "NOW 1 h + RUNNERAT").expect("run should succeed"),
        "[]"
    );
    assert_eq!(
        run(&config, "'v' 'a@m' SETMACROCONFIG").expect("run should succeed"),
        "[]"
    );
    assert_eq!(
        run(
            &config,
            "'v' 'a@m' SETMACROCONFIG NULL 'a@m' SETMACROCONFIG"
        )
        .expect("the value should succeed"),
        "[]"
    );
    let mut stack = test_stack();
    stack
        .capabilities_mut()
        .insert("setmacroconfig".to_owned(), ".*".to_owned());
    stack
        .exec_checked("'v' 'a@m' SETMACROCONFIG 2 'b@m' SETMACROCONFIG")
        .expect("the value should succeed");
    assert_eq!(stack.config_mut().get("a@m").map(String::as_str), Some("v"));
    assert_eq!(stack.config_mut().get("b@m").map(String::as_str), Some("2"));
    stack
        .exec_checked("NULL 'a@m' SETMACROCONFIG")
        .expect("exec_checked should succeed");
    assert_eq!(stack.config_mut().get("a@m"), None);
    assert!(
        run(&config, "'v' 'b@m' SETMACROCONFIG")
            .expect_err("the value should fail")
            .contains("not modifiable")
    );
    assert!(
        run(&config, "'v' 'am' SETMACROCONFIG")
            .expect_err("the value should fail")
            .contains("name@scope")
    );
    assert!(
        run(&config, "'v' 1 SETMACROCONFIG")
            .expect_err("the value should fail")
            .contains("to be a STRING")
    );
    assert!(run(&[("setmacroconfig", "[")], "'v' 'a@m' SETMACROCONFIG").is_err());
    assert_eq!(
        run(&[], "'x' RUNNERNONCE 'y' RUNNERPATH").expect("run should succeed"),
        "[null,null]"
    );
}
