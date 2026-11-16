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

## 6. How the errors are organised

`warpscript::Error` is one `thiserror` enum. Each failing domain has its own enum, wrapped by a
`#[from]` variant, so that `?` converts and Rust code matches on the variant:

| Enum (module)                        | Failures                                                           |
|--------------------------------------|--------------------------------------------------------------------|
| `StackError` (`error/stack.rs`)      | limits, malformed scripts, unknown functions/symbols/macros         |
| `Usage` variant (`Error::usage`)     | a function given arguments it refuses: function name + requirement |
| `ArithmeticError`, `NumberFormatError`, `BoundsError`, `IllegalArgumentError`, `ResourceError` (`error/fault.rs`) | the JDK faults WarpLib functions inherit |
| `MathError` (`error/math.rs`)        | Commons Math (interpolation, fitting, distributions, matrices)     |
| `TimeError`, `TextError`, `FormatError`, `SeriesError`, `AggregatorError`, `CollectionError`, `PlatformError`, `CryptoError`, `GeoError` | one per function family |
| `ThriftError`, `EncoderError`, `PickleError` | the binary formats, next to their code                      |

- **Display is Warp 10's wording**, because a script reads it (`TRY`, `ERROR`, the exec
  endpoint). Rust code never matches on message text.
- **Java class names live in `error/compat.rs` only** (the `type` field of `ERROR`, and the class
  names a message quotes through `compat::value_class` / `compat::py_class`). The sole exceptions
  are messages that quote a Java or Pyrolite *message* verbatim (`PickleError`), which are data.
- `Error::Usage` is the one place where a family of failures is a single variant: Warp 10 has
  about 1 300 `"<FUNCTION> expects …"` messages. The requirement is the Warp 10 sentence;
  callers tell the failures apart by function, never by text.
- The coverage gate skips a file that defines no function (the enum files whose `Display` is
  derived hold nothing the instrumentation can count).

## 7. Known gaps (to burn down)

The rules above hold for every `Result` of the crate, with these remaining exceptions, listed so
that they are not forgotten (`grep -rnF` the pattern to list them):

- **`Option` defaults on an index that was just proven valid**: `get(i).unwrap_or_default()`,
  `unwrap_or(Value::Null)` (about 280). They hide no `Err`; replacing them by direct indexing
  needs a panic-free access helper.
- **Conversions proven by a type check**: `as_f64()`, `as_i64()`, `as_bool()` after `is_number()`,
  a series type test or a `Packable` conversion (`numeric`, `number_at`, `double_of`, `long_of`,
  the filter thresholds). The check runs first, so the default is never taken. Moving the check
  into a typed variant is open work.
- **Parses of text this code just wrote**: the digits of `BigDecimal::double_value`,
  `message_decimal` and `vector_string` (a `{:.N}` rendering always parses back),
  `digits.parse()` of a sign-checked decimal (`arithmetic`). A parse that cannot fail has no
  error variant to name.
- **Warp 10 `int` casts**: a `LONG` used as an index, a key or a register is cut to 32 bits by
  `args::long_as_int`, as Warp 10's `(int)` cast does (oracle: `[ 10 20 30 ] 4294967297 GET`
  answers `20`).
- **Probes of a conversion**: `parse::<i64>().ok()`, `u32::try_from(x).ok()`,
  `usize::try_from(x) else { … }` (about 40). The error carries no information; `None` leads to a
  typed error or to the alternative path.
- **`map_err(|_| …)`** (36): the dropped error is a `TryFromIntError`/`ParseIntError`; the typed
  replacement names the input.
- **Infallible by construction, not yet expressed as such**: `->PICKLE` of an encoder
  (`togts(..).unwrap_or(())`), the series of the encoders `PARSE` fills (`decode().unwrap_or_default()`),
  the double of a JSON number the scanner validated (`parse().unwrap_or(NaN)`), the integer part of
  `message_decimal`. Each needs an API that cannot fail, because an unreachable `?` is an
  uncovered region.
- **Documented Warp 10 tolerances**, matched explicitly with a comment: a series of `LONG` that
  stores `0` for an unparsable string, `->JSON` of an encoder that stops at a corrupt datapoint,
  `CHECKMACRO` answering `false` for a macro that cannot be loaded.
