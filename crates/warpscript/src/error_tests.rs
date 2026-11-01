#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt::{self, Write};

use super::error::Error;

/// A writer that fails after `n` successful writes, to exercise the error paths of `Display`.
pub(crate) struct FailAfter(pub usize);

impl Write for FailAfter {
    fn write_str(&mut self, _s: &str) -> fmt::Result {
        if self.0 == 0 {
            return Err(fmt::Error);
        }
        self.0 -= 1;
        Ok(())
    }
}

#[test]
fn display_propagates_writer_failures() {
    let err = Error::with_cause("outer", Error::with_cause("middle", Error::new("root")));
    let outcomes: Vec<bool> = (0..8)
        .map(|n| write!(FailAfter(n), "{err}").is_ok())
        .collect();
    assert_eq!(
        outcomes,
        [false, false, false, false, false, false, false, true]
    );
}

#[test]
fn messages_and_causes() {
    let root = Error::new("root");
    let middle = Error::with_cause("middle", root.clone());
    let outer = Error::with_cause("outer", middle);
    assert_eq!(outer.message(), "outer");
    assert_eq!(outer.root_cause(), &root);
    assert_eq!(outer.messages(), vec!["outer", "middle", "root"]);
    assert_eq!(outer.to_string(), "outer (middle (root))");
    assert_eq!(root.to_string(), "root");
    assert!(!outer.is_signal());
}

#[test]
fn signals() {
    assert!(Error::Return.is_signal());
    assert!(Error::Break.is_signal());
    assert!(Error::Continue.is_signal());
    assert!(Error::Stop("STOP".to_owned()).is_signal());
    assert_eq!(Error::Return.message(), "RETURN");
    assert_eq!(Error::Stop("x".to_owned()).message(), "x");
    assert_eq!(
        Error::Break.message(),
        "Exception at BREAK: must be used in a one of FOR, FOREACH, FORSTEP, WHILE, UNTIL loop."
    );
    assert!(
        Error::Continue
            .message()
            .starts_with("Exception at CONTINUE:")
    );
    assert_eq!(Error::Break.root_cause(), &Error::Break);
    assert_eq!(Error::Break.messages().len(), 1);
    let wrapped = Error::with_cause("Line #1", Error::Break);
    assert_eq!(wrapped.messages().len(), 2);
    let as_std: &dyn std::error::Error = &wrapped;
    assert!(as_std.to_string().starts_with("Line #1 ("));
}
