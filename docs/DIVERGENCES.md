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

## 5. Floating point mathematics

### 5.1 Transcendental functions may differ from the JDK by one ulp

`java.lang.Math` is allowed to deviate from the correctly rounded result by up to 1 ulp. On
x86-64, HotSpot uses its own intrinsics for `sin`, `cos`, `tan`, `exp`, `log`, `log10` and
`pow`, and delegates the other transcendental functions to `StrictMath` (fdlibm 5.3). The port
currently calls the host C library through Rust's `f64` methods, so results may differ from
the JDK by 1 ulp, and may differ between hosts. Measured against OpenJDK 27 on 100 000 random
arguments with glibc, the results differ in 0.02 % (`TANH`), 0.03 % (`LOG`), 0.1–0.4 % (`SIN`,
`COS`, `TAN`, `EXP`, `EXPM1`, `LOG1P`, `**`, `SINH`, `CBRT`), 2.4 % (`ATAN`), 8.6–8.8 % (`ATAN2`,
`LOG10`) and 11.6–13.1 % (`HYPOT`, `COSH`, `ASIN`, `ACOS`) of the cases. `SQRT`, `FLOOR`,
`CEIL`, `RINT`, `ROUND`, `ULP`, `NEXTUP`, `NEXTDOWN`, `NEXTAFTER`, `COPYSIGN`, `SCALB`,
`GETEXPONENT`, `IEEEREMAINDER`, `SIGNUM`, `ABS`, `MAX`, `MIN`, `TORADIANS` and `TODEGREES` are
exact and match the JDK bit for bit.

The Commons Math `erfInv` used by `NBOUNDS` calls `FastMath.log`, ported as `f64::ln`; the
bounds match the reference values we test bit for bit, but the same 1 ulp caveat applies.

Planned: a pure Rust port of fdlibm (the `StrictMath` specification) will make the
`StrictMath`-delegated functions bit-identical to the JDK and every result reproducible across
hosts; the seven intrinsified functions will then be equal to `StrictMath`, which HotSpot's
intrinsics match in about 90–97 % of the cases.

### 5.2 `RANGE` with a step overflowing a `LONG`

`[ 9223372036854775800 9223372036854775807 10 ] RANGE` wraps around in Java and keeps
appending elements until the JVM runs out of memory. The port stops the sequence when the
next element would overflow. Non-`LONG` list elements raise `RANGE expects a list of 1 to 3
longs on top of the stack.` (Java raises a bare `ClassCastException`).

### 5.3 Random numbers

`RAND` / `RANDOM` draw 53 bits of operating-system entropy (`getrandom`) instead of
`java.security.SecureRandom`; the distribution (uniform on the doubles of `[0, 1)` with a
2^-53 granularity) is the same. `PRNG` / `SRAND` replay `java.util.Random` bit for bit; the
generator state is stored in the `seeded.prng` stack attribute as a `LONG` rather than as a
`Random` object.

## 6. Concurrency of `TIMEBOX`

Warp 10 runs the boxed macro on a separate thread and interrupts it on timeout. WarpScript
values are not thread-safe in this port (they use `Rc`/`RefCell`), so `TIMEBOX` runs the macro
on the **calling thread** and enforces the deadline cooperatively: the interpreter checks the
clock between statements and aborts when the deadline has passed. Observable behaviour (a
`TIMEBOX reached the execution time limit (...)` error, `STOP` swallowed, the stack left as the
macro left it) matches; a single statement that blocks forever in native code cannot be
interrupted, whereas the JVM version could.

## 7. Strings, encodings and byte serialisations

### 7.1 Strings are UTF-16 in Java, UTF-8 here

Java indexes strings by UTF-16 code unit; Rust strings cannot hold lone surrogates. The index
based functions (`SUBSTRING`, the length-1 check of `SPLIT`'s delimiter, the odd-length check of
`HEX->` / `HEXTOB64`, the length check of `B64->` / `B64URL->` / `B64TOHEX`) count UTF-16 units
so that results and error messages match Warp 10, but a `SUBSTRING` that cuts a surrogate pair
in half yields U+FFFD where Java would carry a lone surrogate. `REPLACEALL` with a pattern that
matches the empty string advances by one character after an empty match (Java advances by one
UTF-16 unit, so `'😀' '' '-' REPLACEALL` gives `-😀-` here and three dashes in Java).
`REOPTALT` sorts and factors its alternatives on Unicode scalar values instead of UTF-16 units:
the generated expression is identical for text of the Basic Multilingual Plane and still matches
exactly the alternatives otherwise. `URLDECODE` reports an invalid hexadecimal digit outside the
BMP as the whole character (Java prints the lone high surrogate); the numeric value is the same.

### 7.2 Case mapping and trimming

`TOUPPER` / `TOLOWER` use the Unicode default case mapping (the JDK root locale behaviour, e.g.
`ß` → `SS`, `İ` → `i̇`). Warp 10 calls `toUpperCase()` / `toLowerCase()` without a locale, which
depends on the JVM default locale: on a Turkish locale `i` maps to `İ` there, never here.

### 7.3 Charsets of `->BYTES` / `BYTES->`

Only `UTF-8`, `ISO-8859-1`, `US-ASCII`, `UTF-16`, `UTF-16BE` and `UTF-16LE` (with their JDK
aliases, case-insensitive) are supported; every other name raises
`->BYTES unsupported encoding 'NAME' (NAME)` like a JVM without that charset would. Encoding and
decoding, including the `?` substitution for unmappable characters, the `UTF-16` byte order mark
and the U+FFFD replacement of malformed input (including the JDK's grouping of malformed UTF-8
and UTF-16 sequences), match the JDK bit for bit.

### 7.4 Java runtime exceptions

Where Warp 10 lets a Java runtime exception escape, the port raises a descriptive WarpScript
error instead:

- `JOIN` with a `NULL` element in its `n` form raises `JOIN cannot join a NULL element.`
  (Java: `NullPointerException`); the list form renders `NULL` as `null` like `String.valueOf`.
- `REPLACE` / `REPLACEALL` with a `NULL` operand raise `REPLACE cannot operate on NULL.`
  (Java: `NullPointerException`); any other operand type is converted with `toString()` as in
  Java.
- `->LONGBYTES` with a non-`LONG` list element reports the Warp 10 message with the cause
  `Invalid element type STRING, only LONG elements are supported.` (Java: the
  `ClassCastException` text).
- `VARINT->` on a truncated varint raises `VARINT-> error while decoding values. (Buffer
  underflow.)` (Java: an unwrapped `BufferUnderflowException`).
- Guava's base64 decoding errors are reported with the message of the `DecodingException`
  (`Unrecognized character: -`, `Invalid input length 1`) without the
  `com.google.common.io.BaseEncoding$DecodingException:` class prefix Java adds.
- `SUBSTRING` with a negative length reports the JDK 27 messages (`Range [1, 0) out of bounds for
  length 5` for strings, `1 > 0` for byte arrays); older JVMs word them differently.

### 7.5 `RLP->` and oversized or truncated payloads

Java's `Arrays.copyOfRange` zero-pads a string payload that extends past the end of the input
(and could allocate up to 4 GiB for a forged length). The port raises
`RLP-> encountered an error while decoding RLP. (Index 3 out of bounds for length 2)` instead,
the same error Java raises when the bytes of a header or of a nested list are missing.

### 7.6 `BITCOUNT`, `BITSET` values and `MATCHER` rendering

`BITCOUNT` pushes two `LONG` values (Java pushes `Integer`s, which WarpScript treats the same
way). `BITSET` values are immutable and compare by content like `BitSet.equals`. `TOSTRING` /
`SNAPSHOT` of a `MATCHER` render it as `'pattern' MATCHER` (Java's `toString()` of a `Matcher`
is the unhelpful `java.util.regex.Matcher[pattern=… region=0,0 lastmatch=]`).

### 7.7 `UUID`

`UUID` draws its 128 bits from operating-system entropy (`getrandom`) instead of
`java.security.SecureRandom`; the result is a version 4, IETF variant UUID exactly like
`UUID.randomUUID()`.

## 8. Regular expressions

`MATCH`, `MATCHER`, `REPLACE`, `REPLACEALL` (and the expressions generated by `REOPTALT`) use
the `fancy-regex` crate rather than `java.util.regex`. The port adds the Java `\Q…\E` quoting
(translated into escaped literals) and anchors `MATCH` patterns as `\A(?:…)\z` to obtain
`Matcher.matches()` semantics (so `'ab' 'a|ab' MATCH` matches, with backtracking into the
alternation; a verbose `(?x)` pattern ending with a `#` comment is handled). Replacement strings
follow `Matcher.appendReplacement` exactly (`$1`, `${name}`, `\$`, `\\`, the largest legal group
number for `$11`, and the Java error messages such as `No group 2` or `Illegal group reference`).

Supported Java syntax: lookahead and lookbehind, backreferences (`\1`, `\k<name>`), named groups
`(?<name>…)`, atomic groups `(?>…)`, possessive quantifiers (`a*+`), inline flags `(?i)`,
`(?m)`, `(?s)`, `(?x)`, `(?U)`, the anchors `\A`, `\z`, `\Z`, `\G`, `\b`, `\h`, `\R`, POSIX
(`\p{Alpha}`) and Unicode (`\p{L}`) classes, class intersections (`[a-z&&[^b]]`), `\x{…}`,
`\uXXXX`, `\N{…}`.

Known gaps and differences:

- Not supported: the `\p{javaLowerCase}`-style classes, control escapes `\cX`, octal escapes
  `\0nn`, `(?d)` (Unix lines), `\uXXXX` escapes denoting a lone surrogate.
- `\d`, `\w`, `\s`, `\b` and case-insensitive matching are Unicode-aware (`\d` matches `١`,
  `(?i)é` matches `É`); Java restricts them to ASCII unless `(?U)` is given.
- `.` excludes only `\n`; Java also excludes `\r`, `\u0085`, `\u2028` and `\u2029`.
- `$` (without `(?m)`) matches only at the very end of the input; Java also matches before a
  final line terminator. With `(?m)`, only `\n` is a line terminator here.
- Syntax errors carry the `fancy-regex` wording (`Parsing error at position 2: Opening
  parenthesis without closing parenthesis`) instead of the `PatternSyntaxException` one
  (`Unclosed group near index 2`).
- Patterns that need catastrophic backtracking fail with `Error executing regex: Max limit for
  backtracking count exceeded` after one million backtracking steps instead of running for an
  unbounded time.

## 9. JSON (`->JSON`, `JSON->`, `MAXJSON`)

The serialiser and the parser reproduce Jackson 2.14.2 (the version Warp 10 pins) configured
like `io.warp10.json.JsonUtils`: `->JSON` output is byte-for-byte what Warp 10 produces for
`LONG`, `DOUBLE`, `BOOLEAN`, `STRING`, `BYTES`, `LIST`, `SET`, `MAP`, macros, named function
objects and `NULL`, in strict and loose mode, compact and pretty. The differences:

- **Line separator.** Pretty printing always uses `\n`; Jackson uses `System.lineSeparator()`
  (`\r\n` on Windows hosts).
- **Nesting depth.** Arrays and objects may be nested 1000 deep, when serialising and when
  parsing (the defaults of Jackson 2.15+ `StreamReadConstraints` / 2.16+
  `StreamWriteConstraints`). Warp 10 with Jackson 2.14 has no explicit limit and fails with a
  `StackOverflowError` several thousand levels deeper, which `->JSON` reports as
  `->JSON failed with to convert to JSON, the structure is too deep or it references itself.
  (StackOverflowError)` — the port uses the same message for its depth limit (self-referencing
  lists and maps hit it too). `JSON->` reports ``Document nesting depth (1001) exceeds the
  maximum allowed (1000, from `StreamReadConstraints.getMaxNestingDepth()`)``. Both are
  recursive: a document nested 1000 deep needs about 1.6 KB of stack per level in debug builds
  (less than 0.5 KB in release builds), more than the 2 MB default stack of a non-main thread.
- **Parser messages.** `JSON->` wraps the parser's message like Java (`JSON-> failed to parse
  JSON (…)`), and the inner text follows Jackson's wording (`Unrecognized token 'x': was
  expecting (JSON String, Number (or 'NaN'/'INF'/'+INF'), Array, Object or token 'null',
  'true' or 'false')`, `Unexpected character ('}' (code 125)): was expecting double-quote to
  start field name`, `Illegal unquoted character ((CTRL-CHAR, code 9)): has to be escaped using
  backslash to be included in string value`…) followed by `\n at [line: L, column: C]`. Jackson
  additionally quotes the document in the location (`[Source: (String)"…"; line: 1, column:
  6]`), which the port leaves out. Columns follow Jackson's conventions (1-based, counted after
  the offending character, after an unrecognized token, at the start of a trailing token) but
  may differ by one in some number-literal errors; a character outside the BMP is described by
  its code point (`'😀' (code 128512 / 0x1f600)`) where Java shows its first UTF-16 unit; the
  extent of an unrecognized token uses an approximation of `Character.isJavaIdentifierPart`
  (letters, digits, `_`, `$`, the common currency symbols and the "ignorable" control
  characters).
- **Integers beyond the `LONG` range.** `JSON->` turns `9223372036854775808` into the `DOUBLE`
  `9.223372036854776E18`; Jackson yields a `BigInteger`, which WarpScript functions generally
  cannot use.
- **Lone surrogates.** A `\uD800` escape that is not part of a surrogate pair becomes U+FFFD;
  Java keeps the unpaired UTF-16 unit in the string.
- **Map keys** that are neither strings, `BYTES` (base64), `NULL` (`"null"`), numbers,
  booleans, lists, maps, sets, bit sets, macros nor named functions (`MARK`, saved contexts,
  `MATCHER` objects) are rendered with the runtime's `TOSTRING` text (`MARK`, `CONTEXT`,
  `'re' MATCHER`) instead of Java's default `Object.toString()`.
- **`MAXJSON`** does not check the `maxjson` / `limits` capabilities (there is no capability
  system yet) and therefore never raises `MAXJSON missing capability 'maxjson' or 'limits'.`;
  the ceiling is the hard limit (`warpscript.maxjson.hard` in Warp 10, `Stack::hard_limits` here)
  with the Java message `MAXJSON cannot extend limit past <limit>`.
- **Not ported yet.** The serializers of Geo Time Series, GTS encoders, `Metadata`,
  `RealVector` / `RealMatrix` and audit statements (`io.warp10.json`), and the
  `JsonUtils.addTransformer` extension point: objects without a serializer are written as
  `null`, like Warp 10 does when no transformer handles them.

## 10. Collections (lists, maps, sets, `PACK` / `UNPACK`)

The collection functions reproduce the Java implementations, including their edge cases
(`SUBLIST` bound arithmetic, `int` truncation of `LONG` indices, Java `int` wrap-around of
`PACK` field lengths). The differences:

- **`GROUPBY` order.** The groups come out in first-seen order of their keys. Java collects
  them in a `HashMap`, so the order depends on the hash codes of the keys (and on the
  table size); scripts that relied on that order were already relying on an unspecified
  behaviour.
- **Byte arrays are immutable.** `SET` and `REVERSE` on `BYTES` push a modified copy; Java
  modifies the `byte[]` in place, which is observable when the same array is also referenced
  from a variable or another collection. Lists, maps and sets are shared handles exactly like
  in Java.
- **Sorting mixed types.** `LSORT`, `MSORT` and `SORTBY` on elements (or keys) that are not
  mutually comparable raise `LSORT cannot compare LONG and STRING.` / `LSORT cannot sort
  elements of type LIST.`; Java lets the `ClassCastException` (or `NullPointerException`)
  escape with the JVM text. Lists of fewer than two elements are never compared, as in Java.
- **`SORTWITH` with 32 elements or more.** Java's `TimSort` comparison sequence is reproduced
  exactly for fewer than 32 elements (binary insertion sort); beyond that a stable bottom-up
  merge sort is used, so a comparator macro with side effects is called in a different order.
  Inconsistent comparators never raise Java's `Comparison method violates its general
  contract!`.
- **`SET` with a path on `BYTES`** raises `SET expects a LONG key when operating on a byte
  array.` (Java: a `ClassCastException` text).
- **Vectors** are modelled as lists: `->V` (see §3) builds a list, `V->` copies it.
- **Geo Time Series operands** are not modelled yet: `SIZE`, `REMOVE`, `INDEXOF`, `CLONE`,
  `APPEND`, `SHRINK`, `PIVOT` and `PIVOTSTRICT` on a GTS (or a GTS encoder, matrix…) raise
  the error Java raises for an unsupported operand type. `SORT` and `RSORT` (GTS-only
  functions) and `IMMUTABLE` (containers carry no immutability flag yet) are not ported.
