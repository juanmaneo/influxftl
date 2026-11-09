// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of `->MVSTRING`, `MVTICKSPLIT`, `MVINDEXSPLIT` and `MVEXTRACT`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const I1: &str = r"[ [ 0 1 ] [ 1 48.0 2.0 2.5 ] [ 2 NaN NaN 7 'x y' ] [ 5 T ] ] ->ENCODER";
const I2: &str = r"[ [ 3 10 ] [ 4 F ] [ 7 1.0 1.0 3 'z' ] ] ->ENCODER";
const MVE: &str = r"NEWENCODER 'mv' RENAME { 'l' 'v' } RELABEL 10 NaN NaN NaN @I1 ADDVALUE 20 10.0 20.0 5 @I2 ADDVALUE 30 NaN NaN NaN 'abc' ADDVALUE 40 NaN NaN NaN 'hex:ff00' PARSEVALUE ADDVALUE 50 NaN NaN NaN 42 ADDVALUE 60 NaN NaN NaN 1.5 ADDVALUE";
const MVG: &str = r"NEWGTS 'g' RENAME 1 NaN NaN NaN @I1 WRAPRAW 'ISO-8859-1' BYTES-> ADDVALUE 2 48.0 2.0 4 @I2 WRAPRAW 'ISO-8859-1' BYTES-> ADDVALUE 3 NaN NaN NaN 'abc' ADDVALUE";
const WIDE: &str = r"NEWENCODER 'w' RENAME 1 NaN NaN NaN [ 1 2 3 4 5 6 7 8 9 10 11 12 13 -4 4294967296 ] <% DUP 2 ->LIST %> F LMAP ->ENCODER ADDVALUE";
const CORRUPT: &str = r"[ '78044c02044c00' HEX-> ] ->ENCODER";
const DEC: &str = r"'781830011e5002011d1002011e1002038080808080401002000d00' HEX-> UNWRAPENCODER";
const RT: &str = r"'hex:a5ffffffffff' PARSEVALUE";
const NUM: &str = r"NEWGTS 'n' RENAME 1 NaN NaN NaN 1 ADDVALUE 2 48.0 2.0 3 2 ADDVALUE";
const NEST: &str = r"NEWENCODER 'nest' RENAME 1 NaN NaN NaN NEWENCODER 0 NaN NaN NaN @I2 ADDVALUE 9 NaN NaN NaN 'q' ADDVALUE ADDVALUE";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@CORRUPT", CORRUPT)
        .replace("@WIDE", WIDE)
        .replace("@NEST", NEST)
        .replace("@MVE", MVE)
        .replace("@MVG", MVG)
        .replace("@DEC", DEC)
        .replace("@NUM", NUM)
        .replace("@I1", I1)
        .replace("@I2", I2)
        .replace("@RT", RT)
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
fn mvextract() {
    check(&[
        (
            r"@MVE MVVALUES",
            Ok(r#"[[[1,2.5,"x y",true],[10,false,"z"],"abc","\u00FF\u0000",42,1.5]]"#),
        ),
        (
            r"@MVG MVVALUES",
            Ok(r#"[[[1,2.5,"x y",true],[10,false,"z"],"abc"]]"#),
        ),
        (r"@NUM MVVALUES", Ok(r"[[1,2]]")),
        (r"@NEST MVVALUES", Ok(r#"[[[[10,false,"z"],"q"]]]"#)),
        (
            r"[ @MVE @NUM ] MVVALUES",
            Ok(r#"[[[[1,2.5,"x y",true],[10,false,"z"],"abc","\u00FF\u0000",42,1.5],[1,2]]]"#),
        ),
        (r"[ ] MVVALUES", Ok(r"[[]]")),
        (
            r"'x' MVVALUES",
            Err(r"MVVALUES can only be applied on Geo Time Series or GTS Encoders."),
        ),
        (
            r"[ @NUM 1 ] MVVALUES",
            Err(r"MVVALUES can only be applied on Geo Time Series or GTS Encoders."),
        ),
        (r"MVVALUES", Err(r"Empty stack.")),
        (
            r"@MVE MVLOCATIONS",
            Ok(
                r"[[[[NaN,NaN],[[NaN,NaN],[47.99999998882413,1.9999999646097422],[NaN,NaN],[NaN,NaN]]],[[9.999999990686774,19.99999998137355],[[NaN,NaN],[NaN,NaN],[0.9999999823048711,0.9999999403953552]]],[NaN,NaN],[NaN,NaN],[NaN,NaN],[NaN,NaN]]]",
            ),
        ),
        (
            r"@MVG MVLOCATIONS",
            Ok(
                r"[[[[NaN,NaN],[[NaN,NaN],[47.99999998882413,1.9999999646097422],[NaN,NaN],[NaN,NaN]]],[[47.99999998882413,1.9999999646097422],[[NaN,NaN],[NaN,NaN],[0.9999999823048711,0.9999999403953552]]],[NaN,NaN]]]",
            ),
        ),
        (
            r"@NUM MVLOCATIONS",
            Ok(r"[[[NaN,NaN],[47.99999998882413,1.9999999646097422]]]"),
        ),
        (
            r"@NEST MVLOCATIONS",
            Ok(
                r"[[[[NaN,NaN],[[[NaN,NaN],[[NaN,NaN],[NaN,NaN],[0.9999999823048711,0.9999999403953552]]],[NaN,NaN]]]]]",
            ),
        ),
        (
            r"[ @MVE @NUM ] MVLOCATIONS",
            Ok(
                r"[[[[[NaN,NaN],[[NaN,NaN],[47.99999998882413,1.9999999646097422],[NaN,NaN],[NaN,NaN]]],[[9.999999990686774,19.99999998137355],[[NaN,NaN],[NaN,NaN],[0.9999999823048711,0.9999999403953552]]],[NaN,NaN],[NaN,NaN],[NaN,NaN],[NaN,NaN]],[[NaN,NaN],[47.99999998882413,1.9999999646097422]]]]",
            ),
        ),
        (r"[ ] MVLOCATIONS", Ok(r"[[]]")),
        (
            r"'x' MVLOCATIONS",
            Err(r"MVLOCATIONS can only be applied on Geo Time Series or GTS Encoders."),
        ),
        (
            r"[ @NUM 1 ] MVLOCATIONS",
            Err(r"MVLOCATIONS can only be applied on Geo Time Series or GTS Encoders."),
        ),
        (r"MVLOCATIONS", Err(r"Empty stack.")),
        (
            r"@MVE MVELEVATIONS",
            Ok(
                r"[[[-9223372036854775808,[-9223372036854775808,-9223372036854775808,7,-9223372036854775808]],[5,[-9223372036854775808,-9223372036854775808,3]],-9223372036854775808,-9223372036854775808,-9223372036854775808,-9223372036854775808]]",
            ),
        ),
        (
            r"@MVG MVELEVATIONS",
            Ok(
                r"[[[-9223372036854775808,[-9223372036854775808,-9223372036854775808,7,-9223372036854775808]],[4,[-9223372036854775808,-9223372036854775808,3]],-9223372036854775808]]",
            ),
        ),
        (r"@NUM MVELEVATIONS", Ok(r"[[-9223372036854775808,3]]")),
        (
            r"@NEST MVELEVATIONS",
            Ok(
                r"[[[-9223372036854775808,[[-9223372036854775808,[-9223372036854775808,-9223372036854775808,3]],-9223372036854775808]]]]",
            ),
        ),
        (
            r"[ @MVE @NUM ] MVELEVATIONS",
            Ok(
                r"[[[[-9223372036854775808,[-9223372036854775808,-9223372036854775808,7,-9223372036854775808]],[5,[-9223372036854775808,-9223372036854775808,3]],-9223372036854775808,-9223372036854775808,-9223372036854775808,-9223372036854775808],[-9223372036854775808,3]]]",
            ),
        ),
        (r"[ ] MVELEVATIONS", Ok(r"[[]]")),
        (
            r"'x' MVELEVATIONS",
            Err(r"MVELEVATIONS can only be applied on Geo Time Series or GTS Encoders."),
        ),
        (
            r"[ @NUM 1 ] MVELEVATIONS",
            Err(r"MVELEVATIONS can only be applied on Geo Time Series or GTS Encoders."),
        ),
        (r"MVELEVATIONS", Err(r"Empty stack.")),
        (
            r"@MVE MVTICKS",
            Ok(r"[[[10,[0,1,2,5]],[20,[3,4,7]],30,40,50,60]]"),
        ),
        (r"@MVG MVTICKS", Ok(r"[[[1,[0,1,2,5]],[2,[3,4,7]],3]]")),
        (r"@NUM MVTICKS", Ok(r"[[1,2]]")),
        (r"@NEST MVTICKS", Ok(r"[[[1,[[0,[3,4,7]],9]]]]")),
        (
            r"[ @MVE @NUM ] MVTICKS",
            Ok(r"[[[[10,[0,1,2,5]],[20,[3,4,7]],30,40,50,60],[1,2]]]"),
        ),
        (r"[ ] MVTICKS", Ok(r"[[]]")),
        (
            r"'x' MVTICKS",
            Err(r"MVTICKS can only be applied on Geo Time Series or GTS Encoders."),
        ),
        (
            r"[ @NUM 1 ] MVTICKS",
            Err(r"MVTICKS can only be applied on Geo Time Series or GTS Encoders."),
        ),
        (r"MVTICKS", Err(r"Empty stack.")),
        (
            r"@MVE MVHHCODES",
            Ok(
                r"[[[91480763316633925,[91480763316633925,-2296496678784176095,91480763316633925,91480763316633925]],[-4540737002759274244,[91480763316633925,91480763316633925,-4611002448571681790]],91480763316633925,91480763316633925,91480763316633925,91480763316633925]]",
            ),
        ),
        (
            r"@MVG MVHHCODES",
            Ok(
                r"[[[91480763316633925,[91480763316633925,-2296496678784176095,91480763316633925,91480763316633925]],[-2296496678784176095,[91480763316633925,91480763316633925,-4611002448571681790]],91480763316633925]]",
            ),
        ),
        (
            r"@NUM MVHHCODES",
            Ok(r"[[91480763316633925,-2296496678784176095]]"),
        ),
        (
            r"@NEST MVHHCODES",
            Ok(
                r"[[[91480763316633925,[[91480763316633925,[91480763316633925,91480763316633925,-4611002448571681790]],91480763316633925]]]]",
            ),
        ),
        (
            r"[ @MVE @NUM ] MVHHCODES",
            Ok(
                r"[[[[91480763316633925,[91480763316633925,-2296496678784176095,91480763316633925,91480763316633925]],[-4540737002759274244,[91480763316633925,91480763316633925,-4611002448571681790]],91480763316633925,91480763316633925,91480763316633925,91480763316633925],[91480763316633925,-2296496678784176095]]]",
            ),
        ),
        (r"[ ] MVHHCODES", Ok(r"[[]]")),
        (
            r"'x' MVHHCODES",
            Err(r"MVHHCODES can only be applied on Geo Time Series or GTS Encoders."),
        ),
        (
            r"[ @NUM 1 ] MVHHCODES",
            Err(r"MVHHCODES can only be applied on Geo Time Series or GTS Encoders."),
        ),
        (r"MVHHCODES", Err(r"Empty stack.")),
        (
            r"[ '78044c02044c00' HEX-> ] ->ENCODER MVVALUES",
            Err(r"BufferUnderflowException"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 'plain' ADDVALUE MVVALUES",
            Err(r"NullPointerException"),
        ),
        (
            r"NEWENCODER 1 NaN NaN NaN 'plain' ADDVALUE MVVALUES",
            Ok(r#"[["plain"]]"#),
        ),
        (
            r"NEWENCODER 1 NaN NaN NaN 'hex:00ff' PARSEVALUE ADDVALUE MVVALUES",
            Err(r"NullPointerException"),
        ),
        (
            r"NEWENCODER 1 NaN NaN NaN 'hex:7f' PARSEVALUE ADDVALUE MVVALUES",
            Ok(r#"[[""]]"#),
        ),
        (
            r"@WIDE MVTICKS",
            Ok(r"[[[1,[1,2,3,4,5,6,7,8,9,10,11,12,13,-4,4294967296]]]]"),
        ),
    ]);
}

#[test]
fn tomvstring() {
    check(&[
        (
            r"@MVE ->MVSTRING",
            Ok(
                r#"["[! 10/[! 1 1/47.99999998882413:1.9999999646097422/2.5 2//7/'x%20y' 5/T ] 20/9.999999990686774:19.99999998137355/5/[! 3/10 4/F 7/0.9999999823048711:0.9999999403953552/3/'z' ] 30/'abc' 40/b64:_wA 50/42 60/1.5 ]"]"#,
            ),
        ),
        (
            r"@MVG ->MVSTRING",
            Ok(
                r#"["[! 1/'x%1F%2C%02%C3%94%40%02%C3%A0%214p%214p%21%40%04%00%00%00%00%00%00%C2%98%0C%02%0E%03x%20y%04%06%16%08%00' 2/47.99999998882413:1.9999999646097422/4/'x%13L%06%14%02%02%C2%98L%06%C3%80%02m%C2%B4%02m%C2%B4%02%06%01z%16%06%00' 3/'abc' ]"]"#,
            ),
        ),
        (
            r"@NUM ->MVSTRING",
            Ok(r#"["[! 1/1 2/47.99999998882413:1.9999999646097422/3/2 ]"]"#),
        ),
        (
            r"@NEST ->MVSTRING",
            Ok(
                r#"["[! 1/[! [! 3/10 4/F 7/0.9999999823048711:0.9999999403953552/3/'z' ] 9/'q' ] ]"]"#,
            ),
        ),
        (
            r"@I1 ->MVSTRING",
            Ok(r#"["[! 1 1/47.99999998882413:1.9999999646097422/2.5 2//7/'x%20y' 5/T ]"]"#),
        ),
        (
            r"@I1 WRAP ->MVSTRING",
            Ok(r#"["[! 1 1/47.99999998882413:1.9999999646097422/2.5 2//7/'x%20y' 5/T ]"]"#),
        ),
        (
            r"@I1 WRAPRAW ->MVSTRING",
            Ok(r#"["[! 1 1/47.99999998882413:1.9999999646097422/2.5 2//7/'x%20y' 5/T ]"]"#),
        ),
        (
            r"@I1 WRAPRAW 'x' + ->MVSTRING",
            Err(
                r"+ can only operate on numeric, string, lists, matrices, vectors, Geo Time Series, byte array and macro values.",
            ),
        ),
        (r"'00ff' HEX-> ->MVSTRING", Err(r"NullPointerException")),
        (
            r"@I1 WRAPRAW ->OPB64 ->MVSTRING",
            Ok(r#"["[! 1 1/47.99999998882413:1.9999999646097422/2.5 2//7/'x%20y' 5/T ]"]"#),
        ),
        (r"NEWGTS ->MVSTRING", Ok(r#"["[! ]"]"#)),
        (r"NEWENCODER ->MVSTRING", Ok(r#"["[! ]"]"#)),
        (
            r"1 ->MVSTRING",
            Err(r"->MVSTRING operates on a Geo Time Series, ENCODER, STRING or byte array."),
        ),
        (r"->MVSTRING", Err(r"Empty stack.")),
        (
            r"NEWGTS 0 NaN NaN 3 1 ADDVALUE 0 NaN NaN NaN -1 ADDVALUE ->MVSTRING",
            Ok(r#"["[! 0//3/1 -1 ]"]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 'a%25b cé' ADDVALUE ->MVSTRING",
            Ok(r#"["[! 1/'a%25b%20c%C3%A9' ]"]"#),
        ),
        (r"'' ->MVSTRING", Ok(r#"["b64:"]"#)),
    ]);
}

#[test]
fn mvsplit() {
    check(&[
        (
            r"@MVE MVTICKSPLIT",
            Ok(
                r#"[[{"c":"mv:0","l":{"l":"v"},"a":{},"la":0,"v":[[10,1],[30,"abc"],[40,"\u00FF\u0000"],[50,42],[60,1.5]]},{"c":"mv:1","l":{"l":"v"},"a":{},"la":0,"v":[[10,47.99999998882413,1.9999999646097422,2.5]]},{"c":"mv:2","l":{"l":"v"},"a":{},"la":0,"v":[[10,7,"x y"]]},{"c":"mv:3","l":{"l":"v"},"a":{},"la":0,"v":[[20,9.999999990686774,19.99999998137355,5,10]]},{"c":"mv:4","l":{"l":"v"},"a":{},"la":0,"v":[[20,9.999999990686774,19.99999998137355,5,false]]},{"c":"mv:5","l":{"l":"v"},"a":{},"la":0,"v":[[10,true]]},{"c":"mv:7","l":{"l":"v"},"a":{},"la":0,"v":[[20,0.9999999823048711,0.9999999403953552,3,"z"]]}]]"#,
            ),
        ),
        (
            r"@MVG MVTICKSPLIT",
            Ok(
                r#"[[{"c":"g:0","l":{},"a":{},"la":0,"v":[[1,1],[3,"abc"]]},{"c":"g:1","l":{},"a":{},"la":0,"v":[[1,47.99999998882413,1.9999999646097422,2.5]]},{"c":"g:2","l":{},"a":{},"la":0,"v":[[1,7,"x y"]]},{"c":"g:3","l":{},"a":{},"la":0,"v":[[2,47.99999998882413,1.9999999646097422,4,10]]},{"c":"g:4","l":{},"a":{},"la":0,"v":[[2,47.99999998882413,1.9999999646097422,4,false]]},{"c":"g:5","l":{},"a":{},"la":0,"v":[[1,true]]},{"c":"g:7","l":{},"a":{},"la":0,"v":[[2,0.9999999823048711,0.9999999403953552,3,"z"]]}]]"#,
            ),
        ),
        (
            r"@NUM MVTICKSPLIT",
            Ok(
                r#"[[{"c":"n:0","l":{},"a":{},"la":0,"v":[[1,1],[2,47.99999998882413,1.9999999646097422,3,2]]}]]"#,
            ),
        ),
        (
            r"[ @MVE @MVG ] MVTICKSPLIT",
            Ok(
                r#"[[[{"c":"mv:0","l":{"l":"v"},"a":{},"la":0,"v":[[10,1]]},{"c":"mv:1","l":{"l":"v"},"a":{},"la":0,"v":[[10,47.99999998882413,1.9999999646097422,2.5]]},{"c":"mv:2","l":{"l":"v"},"a":{},"la":0,"v":[[10,7,"x y"]]},{"c":"mv:3","l":{"l":"v"},"a":{},"la":0,"v":[[20,9.999999990686774,19.99999998137355,5,10]]},{"c":"mv:4","l":{"l":"v"},"a":{},"la":0,"v":[[20,9.999999990686774,19.99999998137355,5,false]]},{"c":"mv:5","l":{"l":"v"},"a":{},"la":0,"v":[[10,true]]},{"c":"mv:7","l":{"l":"v"},"a":{},"la":0,"v":[[20,0.9999999823048711,0.9999999403953552,3,"z"]]}],[{"c":"g:0","l":{},"a":{},"la":0,"v":[[1,1]]},{"c":"g:1","l":{},"a":{},"la":0,"v":[[1,47.99999998882413,1.9999999646097422,2.5]]},{"c":"g:2","l":{},"a":{},"la":0,"v":[[1,7,"x y"]]},{"c":"g:3","l":{},"a":{},"la":0,"v":[[2,47.99999998882413,1.9999999646097422,4,10]]},{"c":"g:4","l":{},"a":{},"la":0,"v":[[2,47.99999998882413,1.9999999646097422,4,false]]},{"c":"g:5","l":{},"a":{},"la":0,"v":[[1,true]]},{"c":"g:7","l":{},"a":{},"la":0,"v":[[2,0.9999999823048711,0.9999999403953552,3,"z"]]}]]]"#,
            ),
        ),
        (r"[ @NUM ] MVTICKSPLIT", Ok(r"[[[]]]")),
        (
            r"@NEST MVTICKSPLIT",
            Ok(
                r#"[[{"c":"nest:0","l":{},"a":{},"la":0,"v":[[1,"x\u0013L\u0006\u0014\u0002\u0002\u0098L\u0006\u00C0\u0002m\u00B4\u0002m\u00B4\u0002\u0006\u0001z\u0000"]]},{"c":"nest:9","l":{},"a":{},"la":0,"v":[[1,"q"]]}]]"#,
            ),
        ),
        (
            r"@MVE [ 1 ] MVTICKSPLIT",
            Ok(
                r#"[[{"c":"mv:1","l":{"l":"v"},"a":{},"la":0,"v":[[10,47.99999998882413,1.9999999646097422,2.5]]}]]"#,
            ),
        ),
        (
            r"@MVE [ 0 [ 5 3 ] ] MVTICKSPLIT",
            Ok(
                r#"[[{"c":"mv:0","l":{"l":"v"},"a":{},"la":0,"v":[[10,1],[30,"abc"],[40,"\u00FF\u0000"],[50,42],[60,1.5]]},{"c":"mv:3","l":{"l":"v"},"a":{},"la":0,"v":[[20,9.999999990686774,19.99999998137355,5,10]]},{"c":"mv:4","l":{"l":"v"},"a":{},"la":0,"v":[[20,9.999999990686774,19.99999998137355,5,false]]},{"c":"mv:5","l":{"l":"v"},"a":{},"la":0,"v":[[10,true]]}]]"#,
            ),
        ),
        (
            r"[ @MVE ] [ [ 1 2 ] ] MVTICKSPLIT",
            Ok(
                r#"[[[{"c":"mv:1","l":{"l":"v"},"a":{},"la":0,"v":[[10,47.99999998882413,1.9999999646097422,2.5]]},{"c":"mv:2","l":{"l":"v"},"a":{},"la":0,"v":[[10,7,"x y"]]}]]]"#,
            ),
        ),
        (
            r"@MVE [ 7 ] MVTICKSPLIT",
            Ok(
                r#"[[{"c":"mv:7","l":{"l":"v"},"a":{},"la":0,"v":[[20,0.9999999823048711,0.9999999403953552,3,"z"]]}]]"#,
            ),
        ),
        (
            r"@MVE { 1 'one' '2' 'two' 3 '+suffix' 0 NULL } MVTICKSPLIT",
            Ok(
                r#"[[{"c":"mv:0","l":{"l":"v"},"a":{},"la":0,"v":[[10,1],[30,"abc"],[40,"\u00FF\u0000"],[50,42],[60,1.5]]},{"c":"one","l":{"l":"v"},"a":{},"la":0,"v":[[10,47.99999998882413,1.9999999646097422,2.5]]},{"c":"two","l":{"l":"v"},"a":{},"la":0,"v":[[10,7,"x y"]]},{"c":"mvsuffix","l":{"l":"v"},"a":{},"la":0,"v":[[20,9.999999990686774,19.99999998137355,5,10]]},{"c":"mv:4","l":{"l":"v"},"a":{},"la":0,"v":[[20,9.999999990686774,19.99999998137355,5,false]]},{"c":"mv:5","l":{"l":"v"},"a":{},"la":0,"v":[[10,true]]},{"c":"mv:7","l":{"l":"v"},"a":{},"la":0,"v":[[20,0.9999999823048711,0.9999999403953552,3,"z"]]}]]"#,
            ),
        ),
        (
            r"@MVE [ 1 ] { 1 'one' } MVTICKSPLIT",
            Ok(
                r#"[[{"c":"one","l":{"l":"v"},"a":{},"la":0,"v":[[10,47.99999998882413,1.9999999646097422,2.5]]}]]"#,
            ),
        ),
        (
            r"@MVE [ 'x' ] MVTICKSPLIT",
            Err(r"MVTICKSPLIT expects ticks or ranges of ticks (LONGs)."),
        ),
        (
            r"@MVE [ [ 1 ] ] MVTICKSPLIT",
            Err(r"MVTICKSPLIT expects ticks or ranges of ticks (LONGs)."),
        ),
        (
            r"@MVE [ [ 1 'x' ] ] MVTICKSPLIT",
            Err(r"MVTICKSPLIT expects ticks or ranges of ticks (LONGs)."),
        ),
        (
            r"'x' MVTICKSPLIT",
            Err(r"MVTICKSPLIT operates on Geo Time Series or ENCODER or a list thereof."),
        ),
        (r"[ 1 ] MVTICKSPLIT", Err(r"Empty stack.")),
        (
            r"[ @MVE 1 ] [ 1 ] MVTICKSPLIT",
            Err(r"MVTICKSPLIT operates on Geo Time Series or ENCODER or a list thereof."),
        ),
        (r"[ ] MVTICKSPLIT", Ok(r"[[]]")),
        (r"MVTICKSPLIT", Err(r"Empty stack.")),
        (r"NEWENCODER MVTICKSPLIT", Ok(r"[[]]")),
        (
            r"@WIDE MVTICKSPLIT",
            Ok(
                r#"[[{"c":"w:1","l":{},"a":{},"la":0,"v":[[1,1]]},{"c":"w:4294967296","l":{},"a":{},"la":0,"v":[[1,4294967296]]},{"c":"w:2","l":{},"a":{},"la":0,"v":[[1,2]]},{"c":"w:3","l":{},"a":{},"la":0,"v":[[1,3]]},{"c":"w:-4","l":{},"a":{},"la":0,"v":[[1,-4]]},{"c":"w:4","l":{},"a":{},"la":0,"v":[[1,4]]},{"c":"w:5","l":{},"a":{},"la":0,"v":[[1,5]]},{"c":"w:6","l":{},"a":{},"la":0,"v":[[1,6]]},{"c":"w:7","l":{},"a":{},"la":0,"v":[[1,7]]},{"c":"w:8","l":{},"a":{},"la":0,"v":[[1,8]]},{"c":"w:9","l":{},"a":{},"la":0,"v":[[1,9]]},{"c":"w:10","l":{},"a":{},"la":0,"v":[[1,10]]},{"c":"w:11","l":{},"a":{},"la":0,"v":[[1,11]]},{"c":"w:12","l":{},"a":{},"la":0,"v":[[1,12]]},{"c":"w:13","l":{},"a":{},"la":0,"v":[[1,13]]}]]"#,
            ),
        ),
        (
            r"[ @WIDE @I1 @WIDE ] MVTICKSPLIT",
            Ok(
                r#"[[[{"c":"w:1","l":{},"a":{},"la":0,"v":[[1,1]]},{"c":"w:4294967296","l":{},"a":{},"la":0,"v":[[1,4294967296]]},{"c":"w:2","l":{},"a":{},"la":0,"v":[[1,2]]},{"c":"w:3","l":{},"a":{},"la":0,"v":[[1,3]]},{"c":"w:-4","l":{},"a":{},"la":0,"v":[[1,-4]]},{"c":"w:4","l":{},"a":{},"la":0,"v":[[1,4]]},{"c":"w:5","l":{},"a":{},"la":0,"v":[[1,5]]},{"c":"w:6","l":{},"a":{},"la":0,"v":[[1,6]]},{"c":"w:7","l":{},"a":{},"la":0,"v":[[1,7]]},{"c":"w:8","l":{},"a":{},"la":0,"v":[[1,8]]},{"c":"w:9","l":{},"a":{},"la":0,"v":[[1,9]]},{"c":"w:10","l":{},"a":{},"la":0,"v":[[1,10]]},{"c":"w:11","l":{},"a":{},"la":0,"v":[[1,11]]},{"c":"w:12","l":{},"a":{},"la":0,"v":[[1,12]]},{"c":"w:13","l":{},"a":{},"la":0,"v":[[1,13]]}],[],[{"c":"w:1","l":{},"a":{},"la":0,"v":[[1,1]]},{"c":"w:4294967296","l":{},"a":{},"la":0,"v":[[1,4294967296]]},{"c":"w:2","l":{},"a":{},"la":0,"v":[[1,2]]},{"c":"w:3","l":{},"a":{},"la":0,"v":[[1,3]]},{"c":"w:-4","l":{},"a":{},"la":0,"v":[[1,-4]]},{"c":"w:4","l":{},"a":{},"la":0,"v":[[1,4]]},{"c":"w:5","l":{},"a":{},"la":0,"v":[[1,5]]},{"c":"w:6","l":{},"a":{},"la":0,"v":[[1,6]]},{"c":"w:7","l":{},"a":{},"la":0,"v":[[1,7]]},{"c":"w:8","l":{},"a":{},"la":0,"v":[[1,8]]},{"c":"w:9","l":{},"a":{},"la":0,"v":[[1,9]]},{"c":"w:10","l":{},"a":{},"la":0,"v":[[1,10]]},{"c":"w:11","l":{},"a":{},"la":0,"v":[[1,11]]},{"c":"w:12","l":{},"a":{},"la":0,"v":[[1,12]]},{"c":"w:13","l":{},"a":{},"la":0,"v":[[1,13]]}]]]"#,
            ),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 'plain' ADDVALUE MVTICKSPLIT",
            Err(r"NullPointerException"),
        ),
        (
            r"@MVE MVINDEXSPLIT",
            Ok(
                r#"[[{"c":"mv:0","l":{"l":"v"},"a":{},"la":0,"v":[[10,1],[20,9.999999990686774,19.99999998137355,5,10],[30,"abc"],[40,"\u00FF\u0000"],[50,42],[60,1.5]]},{"c":"mv:1","l":{"l":"v"},"a":{},"la":0,"v":[[10,47.99999998882413,1.9999999646097422,2.5],[20,9.999999990686774,19.99999998137355,5,false]]},{"c":"mv:2","l":{"l":"v"},"a":{},"la":0,"v":[[10,7,"x y"],[20,0.9999999823048711,0.9999999403953552,3,"z"]]},{"c":"mv:3","l":{"l":"v"},"a":{},"la":0,"v":[[10,true]]}]]"#,
            ),
        ),
        (
            r"@MVG MVINDEXSPLIT",
            Ok(
                r#"[[{"c":"g:0","l":{},"a":{},"la":0,"v":[[1,1],[2,47.99999998882413,1.9999999646097422,4,10],[3,"abc"]]},{"c":"g:1","l":{},"a":{},"la":0,"v":[[1,47.99999998882413,1.9999999646097422,2.5],[2,47.99999998882413,1.9999999646097422,4,false]]},{"c":"g:2","l":{},"a":{},"la":0,"v":[[1,7,"x y"],[2,0.9999999823048711,0.9999999403953552,3,"z"]]},{"c":"g:3","l":{},"a":{},"la":0,"v":[[1,true]]}]]"#,
            ),
        ),
        (
            r"@NUM MVINDEXSPLIT",
            Ok(
                r#"[[{"c":"n:0","l":{},"a":{},"la":0,"v":[[1,1],[2,47.99999998882413,1.9999999646097422,3,2]]}]]"#,
            ),
        ),
        (
            r"[ @MVE @MVG ] MVINDEXSPLIT",
            Ok(
                r#"[[[{"c":"mv:0","l":{"l":"v"},"a":{},"la":0,"v":[[10,1],[20,9.999999990686774,19.99999998137355,5,10]]},{"c":"mv:1","l":{"l":"v"},"a":{},"la":0,"v":[[10,47.99999998882413,1.9999999646097422,2.5],[20,9.999999990686774,19.99999998137355,5,false]]},{"c":"mv:2","l":{"l":"v"},"a":{},"la":0,"v":[[10,7,"x y"],[20,0.9999999823048711,0.9999999403953552,3,"z"]]},{"c":"mv:3","l":{"l":"v"},"a":{},"la":0,"v":[[10,true]]}],[{"c":"g:0","l":{},"a":{},"la":0,"v":[[1,1],[2,47.99999998882413,1.9999999646097422,4,10]]},{"c":"g:1","l":{},"a":{},"la":0,"v":[[1,47.99999998882413,1.9999999646097422,2.5],[2,47.99999998882413,1.9999999646097422,4,false]]},{"c":"g:2","l":{},"a":{},"la":0,"v":[[1,7,"x y"],[2,0.9999999823048711,0.9999999403953552,3,"z"]]},{"c":"g:3","l":{},"a":{},"la":0,"v":[[1,true]]}]]]"#,
            ),
        ),
        (r"[ @NUM ] MVINDEXSPLIT", Ok(r"[[[]]]")),
        (
            r"@NEST MVINDEXSPLIT",
            Ok(
                r#"[[{"c":"nest:0","l":{},"a":{},"la":0,"v":[[1,"x\u0013L\u0006\u0014\u0002\u0002\u0098L\u0006\u00C0\u0002m\u00B4\u0002m\u00B4\u0002\u0006\u0001z\u0000"]]},{"c":"nest:1","l":{},"a":{},"la":0,"v":[[1,"q"]]}]]"#,
            ),
        ),
        (
            r"@MVE [ 1 ] MVINDEXSPLIT",
            Ok(
                r#"[[{"c":"mv:1","l":{"l":"v"},"a":{},"la":0,"v":[[10,47.99999998882413,1.9999999646097422,2.5],[20,9.999999990686774,19.99999998137355,5,false]]}]]"#,
            ),
        ),
        (
            r"@MVE [ 0 [ 5 3 ] ] MVINDEXSPLIT",
            Ok(
                r#"[[{"c":"mv:0","l":{"l":"v"},"a":{},"la":0,"v":[[10,1],[20,9.999999990686774,19.99999998137355,5,10],[30,"abc"],[40,"\u00FF\u0000"],[50,42],[60,1.5]]},{"c":"mv:3","l":{"l":"v"},"a":{},"la":0,"v":[[10,true]]}]]"#,
            ),
        ),
        (
            r"[ @MVE ] [ [ 1 2 ] ] MVINDEXSPLIT",
            Ok(
                r#"[[[{"c":"mv:1","l":{"l":"v"},"a":{},"la":0,"v":[[10,47.99999998882413,1.9999999646097422,2.5],[20,9.999999990686774,19.99999998137355,5,false]]},{"c":"mv:2","l":{"l":"v"},"a":{},"la":0,"v":[[10,7,"x y"],[20,0.9999999823048711,0.9999999403953552,3,"z"]]}]]]"#,
            ),
        ),
        (r"@MVE [ 7 ] MVINDEXSPLIT", Ok(r"[[]]")),
        (
            r"@MVE { 1 'one' '2' 'two' 3 '+suffix' 0 NULL } MVINDEXSPLIT",
            Ok(
                r#"[[{"c":"mv:0","l":{"l":"v"},"a":{},"la":0,"v":[[10,1],[20,9.999999990686774,19.99999998137355,5,10],[30,"abc"],[40,"\u00FF\u0000"],[50,42],[60,1.5]]},{"c":"one","l":{"l":"v"},"a":{},"la":0,"v":[[10,47.99999998882413,1.9999999646097422,2.5],[20,9.999999990686774,19.99999998137355,5,false]]},{"c":"two","l":{"l":"v"},"a":{},"la":0,"v":[[10,7,"x y"],[20,0.9999999823048711,0.9999999403953552,3,"z"]]},{"c":"mvsuffix","l":{"l":"v"},"a":{},"la":0,"v":[[10,true]]}]]"#,
            ),
        ),
        (
            r"@MVE [ 1 ] { 1 'one' } MVINDEXSPLIT",
            Ok(
                r#"[[{"c":"one","l":{"l":"v"},"a":{},"la":0,"v":[[10,47.99999998882413,1.9999999646097422,2.5],[20,9.999999990686774,19.99999998137355,5,false]]}]]"#,
            ),
        ),
        (
            r"@MVE [ 'x' ] MVINDEXSPLIT",
            Err(r"MVINDEXSPLIT expects ticks or ranges of ticks (LONGs)."),
        ),
        (
            r"@MVE [ [ 1 ] ] MVINDEXSPLIT",
            Err(r"MVINDEXSPLIT expects ticks or ranges of ticks (LONGs)."),
        ),
        (
            r"@MVE [ [ 1 'x' ] ] MVINDEXSPLIT",
            Err(r"MVINDEXSPLIT expects ticks or ranges of ticks (LONGs)."),
        ),
        (
            r"'x' MVINDEXSPLIT",
            Err(r"MVINDEXSPLIT operates on Geo Time Series or ENCODER or a list thereof."),
        ),
        (r"[ 1 ] MVINDEXSPLIT", Err(r"Empty stack.")),
        (
            r"[ @MVE 1 ] [ 1 ] MVINDEXSPLIT",
            Err(r"MVINDEXSPLIT operates on Geo Time Series or ENCODER or a list thereof."),
        ),
        (r"[ ] MVINDEXSPLIT", Ok(r"[[]]")),
        (r"MVINDEXSPLIT", Err(r"Empty stack.")),
        (r"NEWENCODER MVINDEXSPLIT", Ok(r"[[]]")),
        (
            r"@WIDE MVINDEXSPLIT",
            Ok(
                r#"[[{"c":"w:0","l":{},"a":{},"la":0,"v":[[1,1]]},{"c":"w:1","l":{},"a":{},"la":0,"v":[[1,2]]},{"c":"w:2","l":{},"a":{},"la":0,"v":[[1,3]]},{"c":"w:3","l":{},"a":{},"la":0,"v":[[1,4]]},{"c":"w:4","l":{},"a":{},"la":0,"v":[[1,5]]},{"c":"w:5","l":{},"a":{},"la":0,"v":[[1,6]]},{"c":"w:6","l":{},"a":{},"la":0,"v":[[1,7]]},{"c":"w:7","l":{},"a":{},"la":0,"v":[[1,8]]},{"c":"w:8","l":{},"a":{},"la":0,"v":[[1,9]]},{"c":"w:9","l":{},"a":{},"la":0,"v":[[1,10]]},{"c":"w:10","l":{},"a":{},"la":0,"v":[[1,11]]},{"c":"w:11","l":{},"a":{},"la":0,"v":[[1,12]]},{"c":"w:12","l":{},"a":{},"la":0,"v":[[1,13]]},{"c":"w:13","l":{},"a":{},"la":0,"v":[[1,-4]]},{"c":"w:14","l":{},"a":{},"la":0,"v":[[1,4294967296]]}]]"#,
            ),
        ),
        (
            r"[ @WIDE @I1 @WIDE ] MVINDEXSPLIT",
            Ok(
                r#"[[[{"c":"w:0","l":{},"a":{},"la":0,"v":[[1,1]]},{"c":"w:1","l":{},"a":{},"la":0,"v":[[1,2]]},{"c":"w:2","l":{},"a":{},"la":0,"v":[[1,3]]},{"c":"w:3","l":{},"a":{},"la":0,"v":[[1,4]]},{"c":"w:4","l":{},"a":{},"la":0,"v":[[1,5]]},{"c":"w:5","l":{},"a":{},"la":0,"v":[[1,6]]},{"c":"w:6","l":{},"a":{},"la":0,"v":[[1,7]]},{"c":"w:7","l":{},"a":{},"la":0,"v":[[1,8]]},{"c":"w:8","l":{},"a":{},"la":0,"v":[[1,9]]},{"c":"w:9","l":{},"a":{},"la":0,"v":[[1,10]]},{"c":"w:10","l":{},"a":{},"la":0,"v":[[1,11]]},{"c":"w:11","l":{},"a":{},"la":0,"v":[[1,12]]},{"c":"w:12","l":{},"a":{},"la":0,"v":[[1,13]]},{"c":"w:13","l":{},"a":{},"la":0,"v":[[1,-4]]},{"c":"w:14","l":{},"a":{},"la":0,"v":[[1,4294967296]]}],[],[{"c":"w:0","l":{},"a":{},"la":0,"v":[[1,1]]},{"c":"w:1","l":{},"a":{},"la":0,"v":[[1,2]]},{"c":"w:2","l":{},"a":{},"la":0,"v":[[1,3]]},{"c":"w:3","l":{},"a":{},"la":0,"v":[[1,4]]},{"c":"w:4","l":{},"a":{},"la":0,"v":[[1,5]]},{"c":"w:5","l":{},"a":{},"la":0,"v":[[1,6]]},{"c":"w:6","l":{},"a":{},"la":0,"v":[[1,7]]},{"c":"w:7","l":{},"a":{},"la":0,"v":[[1,8]]},{"c":"w:8","l":{},"a":{},"la":0,"v":[[1,9]]},{"c":"w:9","l":{},"a":{},"la":0,"v":[[1,10]]},{"c":"w:10","l":{},"a":{},"la":0,"v":[[1,11]]},{"c":"w:11","l":{},"a":{},"la":0,"v":[[1,12]]},{"c":"w:12","l":{},"a":{},"la":0,"v":[[1,13]]},{"c":"w:13","l":{},"a":{},"la":0,"v":[[1,-4]]},{"c":"w:14","l":{},"a":{},"la":0,"v":[[1,4294967296]]}]]]"#,
            ),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 'plain' ADDVALUE MVINDEXSPLIT",
            Err(r"NullPointerException"),
        ),
    ]);
}

#[test]
fn mv_errors() {
    check(&[
        (
            r"NEWENCODER 1 NaN NaN NaN NEWENCODER 1 NaN NaN NaN @RT ADDVALUE ADDVALUE MVVALUES",
            Err(r"Index 6 out of bounds for length 6"),
        ),
        (
            r"NEWENCODER 1 NaN NaN NaN @RT ADDVALUE MVVALUES",
            Err(r"Index 6 out of bounds for length 6"),
        ),
        (
            r"NEWENCODER 1 NaN NaN NaN @RT ADDVALUE ->MVSTRING",
            Err(r"Index 6 out of bounds for length 6"),
        ),
        (
            r"NEWENCODER 1 NaN NaN NaN @RT ADDVALUE MVTICKSPLIT",
            Err(r"Index 6 out of bounds for length 6"),
        ),
        (
            r"'a5ffffffffff' HEX-> ->MVSTRING",
            Err(r"Index 6 out of bounds for length 6"),
        ),
        (r"@CORRUPT ->MVSTRING", Err(r"BufferUnderflowException")),
        (
            r"NEWENCODER 1 NaN NaN NaN 'hex:00ff' PARSEVALUE ADDVALUE ->MVSTRING",
            Err(r"NullPointerException"),
        ),
        (
            r"@DEC ->MVSTRING",
            Ok(r#"["[! 1.5 1/-1.5 2/1.5 3/1099511627.776 4/-7 ]"]"#),
        ),
        (
            r"@DEC MVVALUES",
            Ok(r"[[1.5,-1.5,1.5,1.099511627776E9,-7.0]]"),
        ),
        (
            r"'é' ->MVSTRING",
            Err(r"Invalid OPB64 character '?' at offset 0."),
        ),
        (r"'a' ->MVSTRING", Ok(r#"["b64:"]"#)),
        (
            r"NEWENCODER 1 NaN NaN NaN @CORRUPT ADDVALUE MVVALUES",
            Err(r"BufferUnderflowException"),
        ),
        (
            r"NEWENCODER 1 NaN NaN NaN @CORRUPT ADDVALUE MVTICKSPLIT",
            Err(r"BufferUnderflowException"),
        ),
        (r"{ } MVTICKSPLIT", Err(r"Empty stack.")),
        (r"@CORRUPT MVTICKSPLIT", Err(r"BufferUnderflowException")),
        (
            r"[ [ 1 [ [ 3 10 ] ] ->ENCODER WRAPRAW ] ] ->ENCODER MVTICKSPLIT",
            Ok(r#"[[{"c":"null:3","l":{},"a":{},"la":0,"v":[[1,10]]}]]"#),
        ),
        (
            r"NEWGTS 0 999 <% NaN NaN NaN 'aaaaaaaaaa' ADDVALUE %> FOR WRAP ->MVSTRING 0 20 SUBSTRING",
            Ok(r#"["[ 'aaaaaaaaaa' 1/'aa"]"#),
        ),
    ]);
}
