# Documented divergences from Warp 10

`influxftl` aims for behavioural compatibility with Warp 10 (reference revision
`b5764364679f6ccc40a23632e352d134fab1c2eb`). This document lists the places where the Rust
port deliberately or unavoidably differs, so that users porting WarpScript programs know what
to expect. Anything not listed here is intended to match Warp 10 exactly; please report
mismatches that are not documented.

## 1. Platform and runtime

### 1.1 `DOUBLE` text formatting

Doubles are rendered with Rust's shortest round-tripping algorithm wrapped to reproduce
`java.lang.Double.toString` (plain notation in `[1e-3, 1e7)`, computerized scientific notation
otherwise, always at least one fractional digit). This matches the JDK for every value we
test. Should a value ever differ from a specific legacy JVM, the digits are still the shortest
decimal that round-trips to the same `f64`, so numeric equality is preserved.

### 1.2 `BYTES` `TOSTRING`

Calling Java `toString()` on a `byte[]` yields something like `[B@1b6d3586`, i.e. the JVM
identity hash code of the array, which is non-deterministic. We render `BYTES` as `[B@<len>`
(the length in hexadecimal) so the output is deterministic and testable. Programs should never
have relied on the Java form; use `->HEX`, `->B64` or `->OPB64` for a stable representation.

### 1.3 Time unit

The platform time unit is **microseconds**, as in a default Warp 10 (`time.units = us`). This
is a configuration choice in Warp 10; here it is the fixed default. A deployment configured for
`ms` or `ns` upstream will see scaled tick values.

## 2. Security scripts (`SECURE` / `UNSECURE` / `EVALSECURE`)

Warp 10 encrypts secure scripts with an AES key held in the instance key store, serialising
them with Thrift and compressing with GZIP. The runtime does not ship a key store, so by
default `SECURE` / `UNSECURE` / `EVALSECURE` fail with `Missing secure script encryption key.`
— exactly like a Warp 10 instance without the `AES_SECURESCRIPTS` key configured.

A host embedding the `warpscript` crate can install a `SecureCodec` to provide the encryption,
in which case the three functions work. The on-the-wire format produced by a custom codec is
**not** interoperable with Warp 10's Thrift/AES format; secured scripts do not transfer between
the two platforms.

## 3. Not-supported Java features

The following families exist in Warp 10 because of its Java runtime and have no equivalent
here. The functions are either absent or raise a clear "not supported" error rather than
silently misbehaving:

- `JAVA`, the Py4J bridge, and class-loader based extension/plugin loading (`EXT`…);
- `COMPILE` / `COMPILED` (JVM bytecode compilation of macros) — raise `Not supported.`;
- Processing/graphics functions that depend on the Java `Processing` library (`P*`);
- JVM-specific introspection whose output is an implementation detail (stack traces in
  `ERROR` carry the WarpScript message but an empty Java stack trace list).

## 4. Macro repositories and WarpFleet

Warp 10 resolves `@name` macros through, in order: the local symbol table, the on-disk macro
repository, the macro library, WarpFleet (remote), and the macro resolver. The runtime only
resolves macros stored in the **symbol table** (as `MACRO` values). On-disk repositories and
remote WarpFleet fetching are a storage/deployment concern and are out of scope; `@name` for an
unknown macro fails with `Unknown macro 'name'` as it would on an instance with no repositories
configured.

## 5. Concurrency of `TIMEBOX`

Warp 10 runs the boxed macro on a separate thread and interrupts it on timeout. WarpScript
values are not thread-safe in this port (they use `Rc`/`RefCell`), so `TIMEBOX` runs the macro
on the **calling thread** and enforces the deadline cooperatively: the interpreter checks the
clock between statements and aborts when the deadline has passed. Observable behaviour (a
`TIMEBOX reached the execution time limit (...)` error, `STOP` swallowed, the stack left as the
macro left it) matches; a single statement that blocks forever in native code cannot be
interrupted, whereas the JVM version could.
