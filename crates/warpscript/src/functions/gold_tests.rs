// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of `GOLDWRAP`, `METAMATCH`, `ATTRDELTA` and `ATTRSKIP`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const G: &str = r"NEWGTS 'g' RENAME { 'b' '2' 'a' '1' 'é' 'x' 'Z' 'z' } RELABEL { 'y' 'v' 'x' 'w' } SETATTRIBUTES 3 NaN NaN NaN 1.5 ADDVALUE 1 48.0 2.0 5 2.5 ADDVALUE 3 NaN NaN NaN 0.5 ADDVALUE 1 NaN NaN NaN 2.5 ADDVALUE 1 10.0 2.0 NaN 2.5 ADDVALUE 2 NaN NaN NaN NaN ADDVALUE 2 NaN NaN NaN 1.0 ADDVALUE 3 NaN NaN 4 0.5 ADDVALUE";
const L: &str = r"NEWGTS 'l' RENAME 5 NaN NaN NaN 3 ADDVALUE 5 NaN NaN NaN 1 ADDVALUE 5 1.0 1.0 NaN 1 ADDVALUE 2 NaN NaN NaN 9 ADDVALUE 5 2.0 2.0 NaN 1 ADDVALUE 5 0.5 0.5 NaN 1 ADDVALUE 7 NaN NaN 3 1 ADDVALUE 7 NaN NaN 1 1 ADDVALUE 7 NaN NaN NaN 1 ADDVALUE 7 NaN NaN 2 1 ADDVALUE";
const S: &str = r"NEWGTS 's' RENAME 1 NaN NaN NaN 'b' ADDVALUE 1 1.0 1.0 NaN 'a' ADDVALUE 1 NaN NaN NaN 'a' ADDVALUE 0 NaN NaN NaN 'c' ADDVALUE";
const B: &str =
    r"NEWGTS 'b' RENAME 1 NaN NaN NaN T ADDVALUE 1 NaN NaN NaN F ADDVALUE 0 NaN NaN NaN T ADDVALUE";
const E: &str = r"NEWENCODER 'e' RENAME { 'k' 'v' } RELABEL 5 NaN NaN NaN 1 ADDVALUE 3 NaN NaN NaN 2.5 ADDVALUE 5 NaN NaN NaN T ADDVALUE 3 NaN NaN NaN 'x' ADDVALUE 5 NaN NaN NaN 'hex:00ff' PARSEVALUE ADDVALUE 1 NaN NaN NaN 7 ADDVALUE 5 NaN NaN NaN 0 ADDVALUE 3 NaN NaN NaN NaN ADDVALUE 5 NaN NaN NaN F ADDVALUE";
const DEC: &str = r"'781830011e5002011d1002011e1002038080808080401002000d00' HEX-> UNWRAPENCODER";
const SORTED: &str = r"NEWGTS 'o' RENAME 1 NaN NaN NaN 5 ADDVALUE 1 NaN NaN NaN 2 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 0 ADDVALUE 3 NaN NaN NaN 1 ADDVALUE SORT";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@SORTED", SORTED)
        .replace("@DEC", DEC)
        .replace("@G", G)
        .replace("@L", L)
        .replace("@S", S)
        .replace("@B", B)
        .replace("@E", E)
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
fn goldwrap() {
    check(&[
        (
            r"@G GOLDWRAP ->HEX",
            Ok(
                r#"["1c1801671b0488015a017a016101310162013202c3a90178160016001b0288017801770179017600683550020132914000c0a91ed0a91ed0a9914c00e0213470213470210a14027ff8000000000000100000021002010a910c00081000011e161000"]"#,
            ),
        ),
        (
            r"@G T GOLDWRAP ->HEX",
            Ok(
                r#"["1c1801671b0488015a017a016101310162013202c3a90178160016001b028801780177017901760068355006011e900c0008010a11001001000214007ff8000000000000904c01e0213470213470210a0132914000c0a91ed0a91ed0a91100161000"]"#,
            ),
        ),
        (
            r"@G F GOLDWRAP ->HEX",
            Ok(
                r#"["1c1801671b0488015a017a016101310162013202c3a90178160016001b0288017801770179017600683550020132914000c0a91ed0a91ed0a9914c00e0213470213470210a14027ff8000000000000100000021002010a910c00081000011e161000"]"#,
            ),
        ),
        (
            r"@L GOLDWRAP ->HEX",
            Ok(
                r#"["1c18016c1b00160016001b000068394c04120c0602894000c0009b6d009b6d00894000c0026db4026db402894000c009b6d009b6d0090e00040c0402890c0002890e0002890e0002161400"]"#,
            ),
        ),
        (
            r"@L T GOLDWRAP ->HEX",
            Ok(
                r#"["1c18016c1b00160016001b00006839cc0c0e0602890e0001890e000109000e03048c4000c009b6d009b6d00902894000c0026db4026db402894000c0009b6d009b6d0009004e0410161400"]"#,
            ),
        ),
        (
            r"@L F GOLDWRAP ->HEX",
            Ok(
                r#"["1c18016c1b00160016001b000068394c04120c0602894000c0009b6d009b6d00894000c0026db4026db402894000c009b6d009b6d0090e00040c0402890c0002890e0002890e0002161400"]"#,
            ),
        ),
        (
            r"@S GOLDWRAP ->HEX",
            Ok(
                r#"["1c1801731b00160016001b0000681638016358020161994000c0026db4026db40218000162160800"]"#,
            ),
        ),
        (
            r"@S T GOLDWRAP ->HEX",
            Ok(
                r#"["1c1801731b00160016001b000068165802016218000161994000c0026db4026db402380163160800"]"#,
            ),
        ),
        (
            r"@S F GOLDWRAP ->HEX",
            Ok(
                r#"["1c1801731b00160016001b0000681638016358020161994000c0026db4026db40218000162160800"]"#,
            ),
        ),
        (
            r"@B GOLDWRAP ->HEX",
            Ok(r#"["1c1801621b00160016001b000068052442020400160600"]"#),
        ),
        (
            r"@B T GOLDWRAP ->HEX",
            Ok(r#"["1c1801621b00160016001b000068054402020024160600"]"#),
        ),
        (
            r"@B F GOLDWRAP ->HEX",
            Ok(r#"["1c1801621b00160016001b000068052442020400160600"]"#),
        ),
        (
            r"@E GOLDWRAP ->HEX",
            Ok(
                r#"["1c1801651b0188016b0176160016001b000068244c020e14047ff800000000000010000132180001780c04000c0002020004001a000200ff161200"]"#,
            ),
        ),
        (
            r"@E T GOLDWRAP ->HEX",
            Ok(
                r#"["1c1801651b0188016b0176160016001b000068245006013214007ff8000000000000180001780c04020c00004c020e040802001a000200ff161200"]"#,
            ),
        ),
        (
            r"@E F GOLDWRAP ->HEX",
            Ok(
                r#"["1c1801651b0188016b0176160016001b000068244c020e14047ff800000000000010000132180001780c04000c0002020004001a000200ff161200"]"#,
            ),
        ),
        (
            r"@SORTED GOLDWRAP ->HEX",
            Ok(r#"["1c18016f1b00160016001b0000680e4c02040e00060c02000c00020902160a00"]"#),
        ),
        (
            r"@SORTED T GOLDWRAP ->HEX",
            Ok(r#"["1c18016f1b00160016001b0000680e4c060209010c00000c010a0c0004160a00"]"#),
        ),
        (
            r"@SORTED F GOLDWRAP ->HEX",
            Ok(r#"["1c18016f1b00160016001b0000680e4c02040e00060c02000c00020902160a00"]"#),
        ),
        (
            r"@G 'v' STORE $v GOLDWRAP DROP $v TICKLIST $v VALUES",
            Ok(r"[[2.5,2.5,2.5,NaN,1.0,0.5,0.5,1.5],[1,1,1,2,2,3,3,3]]"),
        ),
        (
            r"@G 'v' STORE $v T GOLDWRAP DROP $v TICKLIST $v VALUES",
            Ok(r"[[1.5,0.5,0.5,1.0,NaN,2.5,2.5,2.5],[3,3,3,2,2,1,1,1]]"),
        ),
        (
            r"@SORTED 'v' STORE $v T GOLDWRAP DROP $v TICKLIST $v VALUES",
            Ok(r"[[1,1,0,5,2],[3,2,2,1,1]]"),
        ),
        (
            r"@SORTED 'v' STORE $v GOLDWRAP DROP $v TICKLIST $v VALUES",
            Ok(r"[[2,5,0,1,1],[1,1,2,2,3]]"),
        ),
        (
            r"@L 'v' STORE $v GOLDWRAP DROP $v LOCATIONS $v VALUES",
            Ok(
                r"[[9,1,1,1,1,3,1,1,1,1],[NaN,NaN,0.4999999701976776,0.9999999403953552,1.9999999646097422,NaN,NaN,NaN,NaN,NaN],[NaN,NaN,0.4999999701976776,0.9999999823048711,1.9999999646097422,NaN,NaN,NaN,NaN,NaN]]",
            ),
        ),
        (
            r"[ @G [ @E @L ] ] GOLDWRAP <% <% ->HEX %> LMAP %> LMAP",
            Err(r"LMAP expects a list below the macro on top of the stack."),
        ),
        (
            r"@G WRAP GOLDWRAP ->HEX",
            Ok(
                r#"["1c1801671b0488015a017a016101310162013202c3a90178160016001b0288017801770179017600683550020132914000c0a91ed0a91ed0a9914c00e0213470213470210a14027ff8000000000000100000021002010a910c00081000011e161000"]"#,
            ),
        ),
        (
            r"@E WRAPRAW GOLDWRAP ->HEX",
            Ok(
                r#"["1c1801651b0188016b0176160016001b000068244c020e14047ff800000000000010000132180001780c04000c0002020004001a000200ff161200"]"#,
            ),
        ),
        (r"@G WRAP GOLDWRAP @G GOLDWRAP ==", Ok(r"[true]")),
        (r"NEWGTS GOLDWRAP ->HEX", Ok(r#"["7800160000"]"#)),
        (r"NEWENCODER GOLDWRAP ->HEX", Ok(r#"["7800160000"]"#)),
        (
            r"1 GOLDWRAP",
            Err(r"GOLDWRAP can only handle GTS, ENCODERs and list thereof."),
        ),
        (
            r"[ 1 ] GOLDWRAP",
            Err(r"GOLDWRAP can only handle GTS, ENCODERs and list thereof."),
        ),
        (r"T GOLDWRAP", Err(r"Empty stack.")),
        (r"GOLDWRAP", Err(r"Empty stack.")),
        (r"'00' HEX-> GOLDWRAP", Err(r"NullPointerException")),
        (
            r"'a5ff' HEX-> GOLDWRAP",
            Err(
                r"GOLDWRAP encountered an error while manipulating GTS Wrapper. (Cannot read. Remote side has closed. Tried to read 1 bytes, but only got 0 bytes. (This is often indicative of an internal error on the server side. Please check your server logs.))",
            ),
        ),
        (
            r"'%25%25' GOLDWRAP",
            Err(r"Invalid OPB64 character '%' at offset 0."),
        ),
        (
            r"'a5ff' HEX-> ->OPB64 GOLDWRAP",
            Err(
                r"GOLDWRAP encountered an error while manipulating GTS Wrapper. (Cannot read. Remote side has closed. Tried to read 1 bytes, but only got 0 bytes. (This is often indicative of an internal error on the server side. Please check your server logs.))",
            ),
        ),
        (
            r"'é' GOLDWRAP",
            Err(r"Invalid OPB64 character '?' at offset 0."),
        ),
        (
            r"@DEC GOLDWRAP ->HEX",
            Ok(r#"["781830011e5002011d1002011e1002038080808080401002000d160a00"]"#),
        ),
        (
            r"@DEC T GOLDWRAP ->HEX",
            Ok(r#"["78185008000d1001038080808080401001011e1001011d30011e160a00"]"#),
        ),
        (
            r"[ [ 1 2 ] ] ->ENCODER GOLDWRAP ->HEX",
            Ok(r#"["78034c0204160200"]"#),
        ),
        (
            r"[ '78044c02044c00' HEX-> ] ->ENCODER GOLDWRAP",
            Err(r"BufferUnderflowException"),
        ),
        (
            r"NEWGTS 'x' RENAME 1 NaN NaN NaN 1e300 ADDVALUE 2 NaN NaN NaN 1e-300 ADDVALUE 3 NaN NaN NaN -0.0 ADDVALUE GOLDWRAP ->HEX",
            Ok(
                r#"["1c1801781b00160016001b0000681854027e37e43c8800759c140201a56e1fc2f8f35910020000160600"]"#,
            ),
        ),
    ]);
}

#[test]
fn metamatch() {
    check(&[
        (r"@G 'g{}' METAMATCH", Ok(r"[true]")),
        (r"@G 'h{}' METAMATCH", Ok(r"[false]")),
        (r"@G '~.*{}' METAMATCH", Ok(r"[true]")),
        (r"@G '~g.*{a=1}' METAMATCH", Ok(r"[true]")),
        (r"@G 'g{a=2}' METAMATCH", Ok(r"[false]")),
        (r"@G 'g{x=w}' METAMATCH", Ok(r"[true]")),
        (r"@G 'g{x=w}{}' METAMATCH", Ok(r"[false]")),
        (r"@G 'g{a=1}{x~w.*}' METAMATCH", Ok(r"[true]")),
        (r"@G 'g{c=}' METAMATCH", Ok(r"[true]")),
        (r"@G 'g{a=}' METAMATCH", Ok(r"[false]")),
        (r"@G 'g{}{x=}' METAMATCH", Ok(r"[false]")),
        (r"@G 'g{}{q=}' METAMATCH", Ok(r"[true]")),
        (r"@E 'e{k=v}' METAMATCH", Ok(r"[true]")),
        (r"[ @G @E @L ] 'g{}' METAMATCH", Ok(r"[[true,false,false]]")),
        (r"[ ] 'g{}' METAMATCH", Ok(r"[[]]")),
        (
            r"[ @G 1 ] 'g{}' METAMATCH",
            Err(r"METAMATCH can only be applied to a LIST containing GTS or ENCODER elements."),
        ),
        (
            r"1 'g{}' METAMATCH",
            Err(r"METAMATCH operates on a GTS or ENCODER or a list thereof."),
        ),
        (
            r"@G 1 METAMATCH",
            Err(r"METAMATCH expects a STRING selector."),
        ),
        (r"@G 'g' METAMATCH", Err(r"invalid syntax for selector.")),
        (
            r"@G 'g{a~[}' METAMATCH",
            Err(r"Parsing error at position 1: Invalid character class"),
        ),
        (r"@G 'g{a}' METAMATCH", Err(r"a")),
        (r"[ [ 1 2 ] ] ->ENCODER '~.*{}' METAMATCH", Ok(r"[false]")),
        (r"NEWGTS '~.*{}' METAMATCH", Ok(r"[true]")),
        (r"@G '=g{}' METAMATCH", Ok(r"[true]")),
        (r"@G '~{}' METAMATCH", Ok(r"[false]")),
        (r"NEWENCODER '={}' METAMATCH", Ok(r"[true]")),
        (r"@G 'g%7B{}' METAMATCH", Err(r"{")),
        (r"'g{}' METAMATCH", Err(r"Empty stack.")),
        (r"METAMATCH", Err(r"Empty stack.")),
    ]);
}

#[test]
fn attrflags() {
    check(&[
        ("NULL ATTRDELTA", Ok("[false]")),
        ("T ATTRDELTA NULL ATTRDELTA", Ok("[true]")),
        ("T ATTRDELTA F ATTRDELTA NULL ATTRDELTA", Ok("[false]")),
        (
            "1 ATTRDELTA",
            Err("ATTRDELTA invalid parameter, expected a BOOLEAN or NULL."),
        ),
        ("ATTRDELTA", Err("Empty stack.")),
        ("NULL ATTRSKIP", Ok("[false]")),
        (
            "T ATTRSKIP NULL ATTRSKIP NULL ATTRDELTA",
            Ok("[false,true]"),
        ),
        (
            "'x' ATTRSKIP",
            Err("ATTRSKIP invalid parameter, expected a BOOLEAN or NULL."),
        ),
    ]);
}

/// A regular expression failing while matching (DIVERGENCES: the backtracking limit of
/// `fancy-regex`).
#[test]
fn metamatch_regex_failure() {
    let long = "a".repeat(46);
    let series = format!("NEWGTS 'g' RENAME {{ 'a' '{long}' }} RELABEL");
    for script in [
        format!("{series} 'g{{a~(a|a)*\\1b}}' METAMATCH"),
        format!("[ {series} ] 'g{{a~(a|a)*\\1b}}' METAMATCH"),
    ] {
        let message = stack_json(&script).expect_err("stack_json should fail");
        assert!(message.contains("Max limit for backtracking"), "{message}");
    }
}
