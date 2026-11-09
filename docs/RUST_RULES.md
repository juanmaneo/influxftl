# Rust coding rules

This is a Rust port, not a transliteration of Java. These rules apply to every crate of the
workspace. A change that breaks one is rejected, even if the tests pass.

## 1. No `unsafe` (target: zero)

- `unsafe_code = "forbid"` is set for the whole workspace (`[workspace.lints.rust]`). It is never lifted.
- `unsafe` is allowed only for vendor C SDK FFI or hardware registers, and for test shims that fake
  a C SDK/libc or need syscalls `std` lacks. This project has neither, so the count is zero.
  Should one become unavoidable, it gets:
  - its own module with a comment saying WHY,
  - `unsafe_code = "deny"` in that crate and `#![allow(unsafe_code)]` on that module only,
  - a `// SAFETY:` comment on every block or impl, and a safe wrapper that checks the invariant first,
  - a mention in the commit summary, with the alternatives tried.
- Prefer `std`, then crates already in the tree (`rustix`, `memmap2`); raw `libc` comes last.
- `std::env::set_var` / `remove_var` are forbidden everywhere. Use `Command::env`, a lookup at the
  sole entry point, or an explicit parameter.

## 2. Errors are values: propagate them

- Use `?`. Library code never calls `unwrap()`, `expect()` or `panic!` on a `Result`.
- Tests assert with `expect("what should have happened")`, never `unwrap()`.
- Never swallow an error: no `let _ =` on a fallible call, no `.ok()`, no `.unwrap_or_default()` or
  `.unwrap_or(..)` that hides an `Err`, no `.is_ok()`, no `if let Ok(..)` that drops the `Err`.
  Only best-effort cleanup may ignore a failure, with a comment saying WHY.
- Stay typed. Libraries define a `thiserror` enum with `#[from]` / `#[source]`; binaries use `anyhow`
  and add context with `.context(..)`. Text appears only at the edges (REST body, stderr, UI), never
  as a `String`, `bool` or `Option` standing for an error in the middle of the stack.
- Every detectable failure has its own variant, even if it is only logged. A variant is never
  removed, merged or skipped because no caller uses it. Tolerating a failure happens at the handling
  site, by matching and logging the variant, never by erasing it where it is detected.
- A function that must return `io::Error` builds it with
  `io::Error::new(err.kind(), format!("{context}: {err}"))`, never `io::Error::other(text)`.
- Every `?` is a coverage region, so each one has a test.

### Warp 10 wording lives at the boundary

A WarpScript sees exception classes and messages (`ERROR`, `TRY`, `CATCH`). They are Warp 10
compatibility facts, not the shape of the Rust API:

- Rust code matches on `Error` variants, never on message text.
- `Display` of a variant produces the Warp 10 wording, because the script reads it.
- Java class names (`java.lang.…`, `io.warp10.…`) appear **only** in
  `crates/warpscript/src/error/compat.rs`, which maps a variant to the class a script reads in the
  `type` field of `ERROR`. Nowhere else names a Java class of an error.

## 3. NaN and infinity

At an edge, NaN and ±Infinity are an untyped error, never a value: they are not stored, encoded,
displayed or acted on there. The edges are HTTP/JSON in, the CLI, configuration, the datastore and
every format that cannot represent them (JSON, line protocol, …).

- Check with `warpscript::finite::ensure_finite`, wrap the result as `Error::NotFinite(#[from])`
  and propagate it with `?`.
- Also check every result that can leave the finite range from finite inputs: division, `ln`,
  `log10`, `log2`, `sqrt`, `acos`, `asin`, `tan`, `powf`, `powi`, `exp`, long sums.
- Never hide one: no `unwrap_or(0.0)`, no silent `is_finite` filter, no `partial_cmp().unwrap()`, no
  `total_cmp` / `max` / `min` that orders it away, no `as` cast of an unchecked float.
- Each check is tested with `f64::NAN`, `f64::INFINITY` and `f64::NEG_INFINITY`.
- An edge that cannot refuse (an SSE stream, a line-protocol writer) omits the figure and reports a
  typed error. It never writes `null`, `0`, a clamped bound or the text `"NaN"`.

**Inside the WarpScript evaluator** NaN and ±Infinity remain legitimate `DOUBLE` values, exactly as
in Warp 10: the `NaN` literal, a NaN latitude/longitude meaning "no location", `1.0 0.0 /`. Scripts
depend on that, and the compatibility contract (README §2) requires it. The check applies where a
value enters or leaves the evaluator.

## 4. Write Rust, not Java

- Model closed sets as enums, absence as `Option`, ownership with borrows. Do not mirror a Java
  class hierarchy, a `getXxx`/`setXxx` pair, a `null` sentinel or an exception used for control flow.
- Avoid needless `.clone()`. Borrow, move, or share with `Rc`/`Arc` when the value really is shared.
- Prefer iterators and pattern matching to index loops and flag variables.
- Keep the Java names out of production code, comments excepted when they say which Warp 10 behavior
  is reproduced.
- Match the Warp 10 behavior that scripts can observe. The oracle tests (generated from a real
  Warp 10 3.5.0) decide what that behavior is.

## 5. Enforcement

- `cargo xtask dev-check` runs the security audit, `cargo check`, `fmt`, `clippy -D warnings`, the
  tests, the 100 % coverage gate and the Kani proofs. Each commit leaves it green.
- Clippy denies `unwrap_used`, `expect_used`, `panic` and the other lints of `[workspace.lints]`
  in non-test code. Test modules opt out explicitly with `#![allow(..)]` at the top of the file.
