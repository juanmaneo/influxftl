#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::cmp::Ordering;

use super::functional::{natural_compare, natural_sort, shuffle_impl, shuffle_with, sort_with};
use crate::jrandom::JavaRandom;
use crate::stack::Stack;
use crate::tests::{assert_fails, assert_stack, assert_top, exec, fail_msg, run, top};
use crate::value::Value;

fn longs(items: &[i64]) -> Value {
    Value::list(items.iter().copied().map(Value::Long).collect())
}

/// Java `toString()` of the top of the stack.
fn show(script: &str) -> String {
    top(script).to_string()
}

fn empty_stack(name: &str, operands: &[&str]) {
    for prefix in operands {
        assert_fails(&format!("{prefix} {name}"), "Empty stack.");
    }
}

// -----------------------------------------------------------------------------------------
// LMAP, LFLATMAP, MMAP
// -----------------------------------------------------------------------------------------

#[test]
fn lmap_and_lflatmap() {
    assert_top("[ 1 2 3 ] <% DROP 2 * %> LMAP", longs(&[2, 4, 6]));
    // The index is pushed after the element.
    assert_top("[ 1 2 3 ] <% + %> LMAP", longs(&[1, 3, 5]));
    assert_top("[ 1 2 ] <% 10 * %> false LMAP", longs(&[10, 20]));
    assert_top("[ 1 2 ] <% 10 * %> true LMAP", longs(&[0, 10]));
    assert_top("[ ] <% %> LMAP", longs(&[]));
    // A new list is built.
    let mut stack = exec("[ 1 ] DUP <% DROP %> LMAP");
    let mapped = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert_eq!(mapped, original);
    assert!(!mapped.same_object(&original));
    assert_eq!(show("[ 1 [ 2 3 ] ] <% DROP %> LMAP"), "[1, [2, 3]]");
    assert_eq!(show("[ 1 [ 2 3 ] ] <% DROP %> LFLATMAP"), "[1, 2, 3]");
    assert_eq!(
        show("[ 1 2 ] <% DROP DUP 2 ->LIST %> LFLATMAP"),
        "[1, 1, 2, 2]"
    );
    assert_eq!(show("[ 1 2 ] <% DROP [ ] %> false LFLATMAP"), "[]");
    // Shrinking the list under the iteration fails like ArrayList.get.
    assert_fails(
        "[ 1 2 3 ] 'l' STORE $l <% DROP DROP $l 0 REMOVE DROP DROP 0 %> LMAP",
        "Index 2 out of bounds for length 1",
    );
    // Errors of the macro propagate.
    assert_fails("[ 1 ] <% 0 0 GET %> LMAP", "GET failed.");
    assert_fails("[ 1 ] <% DROP DROP %> LMAP", "Empty stack.");
    // BREAK is not intercepted: it breaks the enclosing loop (the element and index pushed
    // for the macro stay on the stack).
    assert_stack(
        "0 1 3 <% DROP [ 1 2 ] <% BREAK %> LMAP 1 + %> FOR",
        &[Value::Long(0), Value::Long(1), Value::Long(0)],
    );
    assert_fails("[ ] 1 LMAP", "LMAP expects a macro on top of the stack.");
    assert_fails(
        "[ ] <% %> 'x' LMAP",
        "LMAP expects a macro on top of the stack.",
    );
    assert_fails(
        "1 <% %> LMAP",
        "LMAP expects a list below the macro on top of the stack.",
    );
    assert_fails(
        "[ ] 1 LFLATMAP",
        "LFLATMAP expects a macro on top of the stack.",
    );
    assert_fails(
        "1 <% %> LFLATMAP",
        "LFLATMAP expects a list below the macro on top of the stack.",
    );
    empty_stack("LMAP", &["", "<% %>", "true"]);
    empty_stack("LFLATMAP", &["", "<% %>"]);
}

#[test]
fn mmap() {
    assert_eq!(show("{ 'a' 1 'b' 2 } <% DROP 10 * %> MMAP"), "{a=10, b=20}");
    // The index is pushed after the key and the value.
    assert_eq!(show("{ 'a' 1 'b' 2 } <% + %> MMAP"), "{a=1, b=3}");
    assert_eq!(show("{ 'a' 1 } <% 2 * %> false MMAP"), "{a=2}");
    // The macro may change the key.
    assert_eq!(
        show("{ 'a' 1 } <% DROP SWAP DROP 'k' SWAP %> MMAP"),
        "{k=1}"
    );
    assert_eq!(show("{ } <% %> MMAP"), "{}");
    // The entries are iterated from a snapshot: the macro may touch the map.
    assert_stack(
        "{ 'a' 1 'b' 2 } 'm' STORE $m <% DROP $m 9 'c' PUT DROP %> MMAP $m SIZE",
        &[top("{ 'a' 1 'b' 2 }"), Value::Long(3)],
    );
    assert_fails("{ } 1 MMAP", "MMAP expects a macro on top of the stack.");
    assert_fails(
        "1 <% %> MMAP",
        "MMAP expects a MAP below the macro on top of the stack.",
    );
    assert_fails("{ 'a' 1 } <% DROP DROP DROP %> MMAP", "Empty stack.");
    assert_fails("{ 'a' 1 } <% DROP DROP %> MMAP", "Empty stack.");
    assert_fails("{ 'a' 1 } <% 0 0 GET %> MMAP", "GET failed.");
    empty_stack("MMAP", &["", "<% %>", "false"]);
}

// -----------------------------------------------------------------------------------------
// LFILTER, MFILTER, NONNULL, FILTERBY
// -----------------------------------------------------------------------------------------

#[test]
fn lfilter_and_mfilter() {
    assert_top("[ 1 2 3 4 ] <% DROP 2 % 0 == %> LFILTER", longs(&[2, 4]));
    assert_eq!(show("[ 'a' 'b' 'c' ] <% SWAP DROP 1 > %> LFILTER"), "[c]");
    assert_top("[ 1 2 3 ] <% 1 > %> false LFILTER", longs(&[2, 3]));
    assert_top("[ ] <% %> LFILTER", longs(&[]));
    assert_fails(
        "[ 1 ] <% DROP DROP 1 %> LFILTER",
        "LFILTER expects the filter macro to return a BOOLEAN for each element.",
    );
    assert_fails(
        "[ 1 2 ] 'l' STORE $l <% DROP DROP $l 0 REMOVE DROP DROP true %> LFILTER",
        "Index 1 out of bounds for length 1",
    );
    assert_fails("[ 1 ] <% DROP DROP %> LFILTER", "Empty stack.");
    assert_fails("[ 1 ] <% 0 0 GET %> LFILTER", "GET failed.");
    assert_fails("[ ] 1 LFILTER", "LFILTER expects a MACRO.");
    assert_fails("1 <% %> LFILTER", "LFILTER expects a LIST.");
    empty_stack("LFILTER", &["", "<% %>", "true"]);
    assert_eq!(
        show("{ 'a' 1 'b' 2 'c' 3 } <% DROP SWAP DROP 1 > %> MFILTER"),
        "{b=2, c=3}"
    );
    assert_eq!(
        show("{ 'a' 1 'b' 2 } <% 0 == SWAP DROP SWAP DROP %> MFILTER"),
        "{a=1}"
    );
    assert_eq!(show("{ 'a' 1 } <% 2 == SWAP DROP %> false MFILTER"), "{}");
    assert_eq!(show("{ } <% %> MFILTER"), "{}");
    assert_fails(
        "{ 'a' 1 } <% DROP DROP %> MFILTER",
        "MFILTER expects the filter macro to return a BOOLEAN for each element.",
    );
    assert_fails("{ 'a' 1 } <% 0 0 GET %> MFILTER", "GET failed.");
    assert_fails("{ } 1 MFILTER", "MFILTER expects a MACRO.");
    assert_fails("1 <% %> MFILTER", "MFILTER expects a MAP.");
    empty_stack("MFILTER", &["", "<% %>", "true"]);
}

#[test]
fn nonnull_and_filterby() {
    assert_eq!(show("[ 1 NULL 'a' NULL ] NONNULL"), "[1, a]");
    assert_eq!(show("[ ] NONNULL"), "[]");
    let mut stack = exec("[ 1 ] DUP NONNULL");
    let filtered = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert_eq!(filtered, original);
    assert!(!filtered.same_object(&original));
    assert_fails("1 NONNULL", "NONNULL operates on a list.");
    empty_stack("NONNULL", &[""]);
    assert_top("[ 1 2 3 4 ] <% 2 % 0 == %> FILTERBY", longs(&[2, 4]));
    assert_top("[ 1 2 ] <% DROP false %> FILTERBY", longs(&[]));
    assert_top("[ ] <% %> FILTERBY", longs(&[]));
    assert_fails("[ 1 ] <% 0 0 GET %> FILTERBY", "GET failed.");
    assert_fails(
        "[ 1 ] <% %> FILTERBY",
        "FILTERBY expects its macro to leave a BOOLEAN on the stack.",
    );
    assert_fails(
        "[ ] 1 FILTERBY",
        "FILTERBY expects a macro on top of the stack.",
    );
    assert_fails("1 <% %> FILTERBY", "FILTERBY operates on a list.");
    empty_stack("FILTERBY", &["", "<% %>"]);
}

// -----------------------------------------------------------------------------------------
// LSORT, MSORT
// -----------------------------------------------------------------------------------------

#[test]
fn natural_ordering() {
    assert_eq!(
        natural_compare(&Value::Long(1), &Value::Long(2)),
        Some(Ordering::Less)
    );
    assert_eq!(
        natural_compare(&Value::Boolean(true), &Value::Boolean(false)),
        Some(Ordering::Greater)
    );
    assert_eq!(
        natural_compare(&Value::Double(f64::NAN), &Value::Double(f64::NAN)),
        Some(Ordering::Equal)
    );
    assert_eq!(
        natural_compare(&Value::Double(-f64::NAN), &Value::Double(f64::INFINITY)),
        Some(Ordering::Greater)
    );
    assert_eq!(
        natural_compare(&Value::Double(-0.0), &Value::Double(0.0)),
        Some(Ordering::Less)
    );
    assert_eq!(
        natural_compare(&Value::from("\u{1f600}"), &Value::from("\u{ffff}")),
        Some(Ordering::Less)
    );
    assert_eq!(natural_compare(&Value::Long(1), &Value::Double(1.0)), None);
    assert_eq!(natural_compare(&Value::Null, &Value::Null), None);
    let mut items = vec![Value::Long(2), Value::Long(1)];
    natural_sort("X", &mut items).unwrap();
    assert_eq!(items, vec![Value::Long(1), Value::Long(2)]);
}

#[test]
fn lsort() {
    assert_top("[ 3 1 2 ] LSORT", longs(&[1, 2, 3]));
    assert_top(
        "[ MAXLONG MINLONG 0 ] LSORT",
        longs(&[i64::MIN, 0, i64::MAX]),
    );
    // In place, the list stays on the stack.
    let mut stack = exec("[ 3 1 2 ] DUP LSORT");
    let sorted = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert!(sorted.same_object(&original));
    assert_eq!(sorted, longs(&[1, 2, 3]));
    // Doubles: Double.compare order (reference from Collections.sort on JDK 27).
    let mut stack = Stack::new();
    stack.push(Value::list(
        [3.0, f64::NAN, -0.0, 0.0, f64::NEG_INFINITY, 1.0]
            .into_iter()
            .map(Value::Double)
            .collect(),
    ));
    crate::functions::call(&mut stack, "LSORT").unwrap();
    assert_eq!(
        stack.pop().unwrap(),
        Value::list(
            [f64::NEG_INFINITY, -0.0, 0.0, 1.0, 3.0, f64::NAN]
                .into_iter()
                .map(Value::Double)
                .collect()
        )
    );
    // Strings: UTF-16 code unit order (U+1F600 sorts before U+FFFF).
    assert_eq!(
        show("[ 'b' 'a' '\u{1f600}' '\u{ffff}' '' 'ab' 'B' ] LSORT"),
        "[, B, a, ab, b, \u{1f600}, \u{ffff}]"
    );
    assert_eq!(show("[ true false true ] LSORT"), "[false, true, true]");
    // Fewer than two elements are never compared.
    assert_eq!(show("[ [ ] ] LSORT"), "[[]]");
    assert_eq!(show("[ ] LSORT"), "[]");
    assert_fails("[ 1 'a' ] LSORT", "LSORT cannot compare LONG and STRING.");
    assert_fails("[ 1 1.0 ] LSORT", "LSORT cannot compare LONG and DOUBLE.");
    assert_fails("[ 1 2 NULL ] LSORT", "LSORT cannot compare LONG and NULL.");
    assert_fails(
        "[ [ ] [ ] ] LSORT",
        "LSORT cannot sort elements of type LIST.",
    );
    assert_fails("1 LSORT", "LSORT operates on a list.");
    empty_stack("LSORT", &[""]);
}

#[test]
fn msort() {
    assert_eq!(show("{ 'b' 1 'a' 2 } MSORT"), "{a=2, b=1}");
    assert_eq!(show("{ 2 'x' 1 'y' 0 'z' } MSORT"), "{0=z, 1=y, 2=x}");
    assert_eq!(show("{ } MSORT"), "{}");
    assert_eq!(show("{ [ ] 1 } MSORT"), "{[]=1}");
    let mut stack = exec("{ 'b' 1 'a' 2 } DUP MSORT");
    let sorted = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert!(sorted.same_object(&original));
    assert_fails(
        "{ 1 'x' 'a' 'y' } MSORT",
        "MSORT cannot compare LONG and STRING.",
    );
    assert_fails(
        "{ [ ] 1 [ 2 ] 2 } MSORT",
        "MSORT cannot sort elements of type LIST.",
    );
    assert_fails("1 MSORT", "MSORT operates on a map created by {} or ->MAP.");
    empty_stack("MSORT", &[""]);
}

// -----------------------------------------------------------------------------------------
// SORTBY
// -----------------------------------------------------------------------------------------

#[test]
fn sortby() {
    assert_top("[ 3 1 2 ] <% %> SORTBY", longs(&[1, 2, 3]));
    assert_eq!(show("[ 'bb' 'a' 'ccc' ] <% SIZE %> SORTBY"), "[a, bb, ccc]");
    assert_eq!(show("[ 'bb' 'a' 'ccc' ] <% %> SORTBY"), "[a, bb, ccc]");
    assert_top("[ 2 1 ] <% 1.0 * %> SORTBY", longs(&[1, 2]));
    // Stable.
    assert_top(
        "[ 21 11 22 12 23 ] <% 10 / %> SORTBY",
        longs(&[11, 12, 21, 22, 23]),
    );
    assert_top("[ ] <% %> SORTBY", longs(&[]));
    // In place, pushed back.
    let mut stack = exec("[ 2 1 ] DUP <% %> SORTBY");
    let sorted = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert!(sorted.same_object(&original));
    assert_eq!(sorted, longs(&[1, 2]));
    // Maps: the macro gets the key and the value.
    assert_eq!(show("{ 'b' 1 'a' 2 } <% DROP %> SORTBY"), "{a=2, b=1}");
    assert_eq!(show("{ 'a' 2 'b' 1 } <% SWAP DROP %> SORTBY"), "{b=1, a=2}");
    assert_eq!(show("{ } <% %> SORTBY"), "{}");
    let mut stack = exec("{ 'b' 1 'a' 2 } DUP <% DROP %> SORTBY");
    let sorted = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert!(sorted.same_object(&original));
    let message = "SORTBY expects its macro to return a non null double, long or string in a consistent manner.";
    assert_fails("[ 1 ] <% DROP true %> SORTBY", message);
    assert_fails("[ 1 2 ] <% DROP NULL %> SORTBY", message);
    assert_fails("[ 1 'a' ] <% %> SORTBY", message);
    assert_fails("[ 1 1.0 ] <% %> SORTBY", message);
    assert_fails("{ 'a' 1 'b' 'x' } <% SWAP DROP %> SORTBY", message);
    assert_fails("[ 1 ] <% 0 0 GET %> SORTBY", "GET failed.");
    assert_fails(
        "[ ] 1 SORTBY",
        "SORTBY expects a macro on top of the stack.",
    );
    assert_fails(
        "1 <% %> SORTBY",
        "SORTBY operates on a list or a map created by {} or ->MAP.",
    );
    empty_stack("SORTBY", &["", "<% %>"]);
}

// -----------------------------------------------------------------------------------------
// SORTWITH
// -----------------------------------------------------------------------------------------

/// Sort `list` with a logging comparator and return `(sorted list, comparisons)` as Java
/// strings.
fn sortwith_trace(list: &str) -> (String, String) {
    let mut stack = exec(&format!(
        "[ ] 'log' STORE {list} <% 2 DUPN 2 ->LIST 1 ->LIST $log SWAP APPEND DROP - %> SORTWITH $log"
    ));
    let log = stack.pop().unwrap().to_string();
    let sorted = stack.pop().unwrap().to_string();
    (sorted, log)
}

#[test]
fn sortwith_reproduces_timsort_comparisons() {
    // Reference sequences from ArrayList.sort on JDK 27 (see /tmp/jref/coll/SortSeq.java).
    assert_eq!(
        sortwith_trace("[ 3 1 2 ]"),
        (
            "[1, 2, 3]".to_owned(),
            "[[1, 3], [2, 1], [2, 3], [2, 1]]".to_owned()
        )
    );
    assert_eq!(
        sortwith_trace("[ 1 2 3 ]"),
        ("[1, 2, 3]".to_owned(), "[[2, 1], [3, 2]]".to_owned())
    );
    assert_eq!(
        sortwith_trace("[ 2 1 ]"),
        ("[1, 2]".to_owned(), "[[1, 2]]".to_owned())
    );
    assert_eq!(
        sortwith_trace("[ 5 4 3 2 1 ]"),
        (
            "[1, 2, 3, 4, 5]".to_owned(),
            "[[4, 5], [3, 4], [2, 3], [1, 2]]".to_owned()
        )
    );
    assert_eq!(
        sortwith_trace("[ 2 2 1 3 1 ]"),
        (
            "[1, 1, 2, 2, 3]".to_owned(),
            "[[2, 2], [1, 2], [1, 2], [1, 2], [3, 2], [3, 2], [1, 2], [1, 2], [1, 1]]".to_owned()
        )
    );
    assert_eq!(
        sortwith_trace("[ 4 1 3 1 2 5 0 2 ]"),
        (
            "[0, 1, 1, 2, 2, 3, 4, 5]".to_owned(),
            "[[1, 4], [3, 1], [3, 4], [3, 1], [1, 3], [1, 1], [2, 3], [2, 1], [5, 2], [5, 4], [0, 3], [0, 1], [0, 1], [2, 2], [2, 4], [2, 3]]".to_owned()
        )
    );
    // Constant comparators.
    assert_eq!(
        run("[ ] 'log' STORE [ 3 1 2 ] <% 2 DUPN 2 ->LIST 1 ->LIST $log SWAP APPEND DROP 2 DROPN 0 %> SORTWITH $log")
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["[3, 1, 2]", "[[1, 3], [2, 1]]"]
    );
    assert_eq!(
        run("[ ] 'log' STORE [ 3 1 2 0 ] <% 2 DUPN 2 ->LIST 1 ->LIST $log SWAP APPEND DROP 2 DROPN 1 %> SORTWITH $log")
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["[3, 1, 2, 0]", "[[1, 3], [2, 1], [0, 2]]"]
    );
    assert_eq!(
        run("[ ] 'log' STORE [ 3 1 2 0 ] <% 2 DUPN 2 ->LIST 1 ->LIST $log SWAP APPEND DROP 2 DROPN -1 %> SORTWITH $log")
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["[0, 2, 1, 3]", "[[1, 3], [2, 1], [0, 2]]"]
    );
}

#[test]
fn sortwith_lists_and_maps() {
    assert_top("[ 3 1 2 ] <% SWAP - %> SORTWITH", longs(&[3, 2, 1]));
    assert_top("[ ] <% %> SORTWITH", longs(&[]));
    assert_top("[ 1 ] <% %> SORTWITH", longs(&[1]));
    // Stable.
    assert_top(
        "[ 21 11 22 12 23 ] <% 10 / SWAP 10 / SWAP - %> SORTWITH",
        longs(&[11, 12, 21, 22, 23]),
    );
    let mut stack = exec("[ 2 1 ] DUP <% - %> SORTWITH");
    let sorted = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert!(sorted.same_object(&original));
    assert_eq!(sorted, longs(&[1, 2]));
    // Maps: the comparator gets key1 value1 key2 value2.
    assert_eq!(
        show("{ 'a' 3 'b' 1 'c' 2 } <% SWAP DROP ROT DROP - %> SORTWITH"),
        "{b=1, c=2, a=3}"
    );
    assert_eq!(show("{ } <% %> SORTWITH"), "{}");
    let mut stack = exec("{ 'a' 3 'b' 1 } DUP <% SWAP DROP ROT DROP - %> SORTWITH");
    let sorted = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert!(sorted.same_object(&original));
    assert_fails(
        "[ ] 1 SORTWITH",
        "SORTWITH expects a macro on top of the stack.",
    );
    assert_fails(
        "1 <% %> SORTWITH",
        "SORTWITH operates on a list or a map created by {} or ->MAP.",
    );
    empty_stack("SORTWITH", &["", "<% %>"]);
}

#[test]
fn sortwith_merge_sort_beyond_31_elements() {
    // Reference result from ArrayList.sort on JDK 27 for i % 7, i = 40 down to 1.
    let items: Vec<i64> = (1..=40).rev().map(|i| i % 7).collect();
    let literal = items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    let mut expected = items.clone();
    expected.sort_unstable();
    assert_top(&format!("[ {literal} ] <% - %> SORTWITH"), longs(&expected));
    // Stability: compare on the hundreds only.
    let items: Vec<i64> = (1..=40).rev().map(|i| (i % 7) * 100 + i).collect();
    let literal = items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    let mut expected = items.clone();
    expected.sort_by_key(|v| v / 100);
    assert_top(
        &format!("[ {literal} ] <% 100 / SWAP 100 / SWAP - %> SORTWITH"),
        longs(&expected),
    );
    // A failing comparator stops the merge.
    assert_fails(
        &format!("[ {literal} ] <% 2 DROPN 'x' %> SORTWITH"),
        "SORTWITH encountered an error with comparator: SORTWITH was given a macro which doesn't return a LONG.",
    );
    // Direct check of the merge sort with an inconsistent comparator: it terminates.
    let mut values: Vec<i64> = (0..40).collect();
    sort_with(&mut values, |_, _| Ok(1)).unwrap();
    assert_eq!(values.len(), 40);
}

/// `SORTWITH` with a comparator that fails on its `n`-th call.
fn failing_on_call(list: &str, n: i64) -> String {
    fail_msg(&format!(
        "0 'n' STORE {list} <% $n 1 + 'n' STORE $n {n} == <% 2 DROPN 'x' %> <% - %> IFTE %> SORTWITH"
    ))
}

#[test]
fn sortwith_comparator_failures_at_every_stage() {
    let not_long = "SORTWITH encountered an error with comparator: SORTWITH was given a macro which doesn't return a LONG.";
    // Descending run detection, ascending run detection, binary insertion.
    assert!(failing_on_call("[ 3 2 1 ]", 2).contains(not_long));
    assert!(failing_on_call("[ 1 2 3 ]", 2).contains(not_long));
    assert!(failing_on_call("[ 3 1 2 ]", 3).contains(not_long));
    assert!(failing_on_call("[ 3 1 2 ]", 1).contains(not_long));
}

#[test]
fn sortwith_errors() {
    let not_long = "SORTWITH encountered an error with comparator: SORTWITH was given a macro which doesn't return a LONG. This LONG must be within the bounds of a 32-bit integer. (SORTWITH was given a macro which doesn't return a LONG. This LONG must be within the bounds of a 32-bit integer.)";
    assert!(fail_msg("[ 2 1 ] <% 2 DROPN 'x' %> SORTWITH").contains(not_long));
    assert_fails(
        "[ 2 1 ] <% 2 DROPN 4294967296 %> SORTWITH",
        "SORTWITH encountered an error with comparator: integer overflow (integer overflow)",
    );
    assert_fails(
        "[ 2 1 ] <% 2 DROPN -2147483649 %> SORTWITH",
        "SORTWITH encountered an error with comparator: integer overflow",
    );
    assert_top("[ 2 1 ] <% 2 DROPN 2147483647 %> SORTWITH", longs(&[2, 1]));
    assert_top("[ 2 1 ] <% 2 DROPN -2147483648 %> SORTWITH", longs(&[1, 2]));
    // An error raised inside the macro.
    let message = fail_msg("[ 2 1 ] <% 2 DROPN 0 0 GET %> SORTWITH");
    assert!(message.contains("SORTWITH encountered an error with comparator: Exception at"));
    assert!(message.contains("GET failed. (Invalid OBJECT to GET on.)"));
    // The comparator may consume the operands and leave nothing.
    assert_fails(
        "[ 2 1 ] <% 2 DROPN %> SORTWITH",
        "SORTWITH encountered an error with comparator: Empty stack.",
    );
    assert_fails(
        "{ 'a' 1 'b' 2 } <% 4 DROPN 'x' %> SORTWITH",
        "SORTWITH encountered an error with comparator: SORTWITH was given a macro",
    );
    // Signals are rethrown as is for lists (STOP keeps the stack as it is)…
    assert_stack(
        "[ 2 1 ] <% STOP %> SORTWITH",
        &[Value::Long(1), Value::Long(2)],
    );
    assert_stack(
        "0 1 2 <% DROP [ 2 1 ] <% BREAK %> SORTWITH %> FOR",
        &[Value::Long(0), Value::Long(1), Value::Long(2)],
    );
    // … but wrapped for maps, like in Java.
    assert_fails(
        "{ 'a' 1 'b' 2 } <% STOP %> SORTWITH",
        "SORTWITH encountered an error with comparator: STOP",
    );
}

// -----------------------------------------------------------------------------------------
// SHUFFLE, GROUPBY
// -----------------------------------------------------------------------------------------

fn shuffled(seed: i64, n: i64) -> Vec<i64> {
    let mut items: Vec<Value> = (0..n).map(Value::Long).collect();
    let mut rnd = JavaRandom::new(seed);
    shuffle_with(&mut items, &mut rnd);
    items.iter().map(|v| v.as_i64().unwrap()).collect()
}

#[test]
fn shuffle_matches_java_random() {
    // Reference permutations from Collections.shuffle(list, new Random(seed)) on JDK 27.
    assert_eq!(shuffled(42, 0), Vec::<i64>::new());
    assert_eq!(shuffled(42, 1), vec![0]);
    assert_eq!(shuffled(42, 3), vec![1, 0, 2]);
    assert_eq!(shuffled(42, 5), vec![1, 2, 3, 4, 0]);
    assert_eq!(shuffled(42, 10), vec![4, 6, 2, 1, 7, 9, 8, 5, 3, 0]);
    assert_eq!(
        shuffled(42, 33),
        vec![
            8, 17, 15, 6, 2, 4, 20, 11, 23, 10, 27, 21, 31, 9, 28, 24, 30, 26, 7, 12, 22, 18, 0, 5,
            19, 16, 32, 25, 13, 14, 3, 1, 29
        ]
    );
    assert_eq!(shuffled(-123_456_789, 7), vec![4, 0, 3, 2, 1, 5, 6]);
}

#[test]
fn shuffle_function() {
    // In place, the list stays on the stack, with the same elements.
    let mut stack = exec("[ 1 2 3 4 5 6 7 8 ] DUP SHUFFLE");
    let shuffled = stack.pop().unwrap();
    let original = stack.pop().unwrap();
    assert!(shuffled.same_object(&original));
    let Value::List(list) = shuffled else {
        panic!("not a list");
    };
    let mut items: Vec<i64> = list.borrow().iter().map(|v| v.as_i64().unwrap()).collect();
    items.sort_unstable();
    assert_eq!(items, (1..=8).collect::<Vec<_>>());
    assert_top("[ ] SHUFFLE", longs(&[]));
    assert_top("[ 1 ] SHUFFLE", longs(&[1]));
    assert_fails("1 SHUFFLE", "SHUFFLE operates on a list.");
    empty_stack("SHUFFLE", &[""]);
    let mut stack = Stack::new();
    stack.push(longs(&[1, 2]));
    assert_eq!(
        shuffle_impl(&mut stack, Err(getrandom::Error::UNSUPPORTED))
            .unwrap_err()
            .to_string(),
        format!(
            "Unable to obtain random data from the operating system ({}).",
            getrandom::Error::UNSUPPORTED
        )
    );
    stack.push(Value::Long(1));
    assert_eq!(
        shuffle_impl(&mut stack, Ok(42)).unwrap_err().to_string(),
        "SHUFFLE operates on a list."
    );
}

#[test]
fn groupby() {
    assert_eq!(
        show("[ 1 2 3 4 5 ] <% 2 % %> GROUPBY"),
        "[[1, [1, 3, 5]], [0, [2, 4]]]"
    );
    assert_eq!(
        show("[ 'a' 'bb' 'c' ] <% SIZE %> GROUPBY"),
        "[[1, [a, c]], [2, [bb]]]"
    );
    assert_eq!(
        show("[ 1 NULL ] <% DROP NULL %> GROUPBY"),
        "[[null, [1, null]]]"
    );
    assert_eq!(show("[ ] <% %> GROUPBY"), "[]");
    assert_fails("[ 1 ] <% 0 0 GET %> GROUPBY", "GET failed.");
    assert_fails(
        "[ ] 1 GROUPBY",
        "GROUPBY expects a macro on top of the stack.",
    );
    assert_fails("1 <% %> GROUPBY", "GROUPBY operates on a list.");
    empty_stack("GROUPBY", &["", "<% %>"]);
}
