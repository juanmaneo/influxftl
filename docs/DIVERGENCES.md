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
- **`MAXJSON`** checks the `maxjson` / `limits` capabilities like Warp 10 (see §12); the
  ceiling is the `maxjson` capability value or the hard limit (`warpscript.maxjson.hard` in
  Warp 10, `Stack::hard_limits` here) with the Java message `MAXJSON cannot extend limit past
  <limit>`.
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

## 11. Cryptography and compression

The digests (`MD5`, `SHA256`, `KECCAK.*`, `SHA3.*`, `RIPEMD*`, `TIGER`, `WHIRLPOOL`,
`GOST`…), `SHA256HMAC` / `SHA1HMAC`, `HASH`, the Base58 conversions and `AESWRAP` /
`AESUNWRAP` are verified against the BouncyCastle version Warp 10 depends on (1.78.1) and
produce identical bytes. The differences:

- **Compressed bytes.** `GZIP` and `DEFLATE` are produced by the pure Rust `miniz_oxide`
  backend of `flate2`; they decompress to the same data as the JDK streams (and the JDK
  streams decompress here) but the compressed bytes are not identical to those of
  `java.util.zip` (zlib). The gzip header carries no modification time and the `OS` byte is
  `255` (unknown) like JDK 16 and later; earlier JDKs wrote `0`. Scripts must not compare
  compressed output byte for byte (they could not across JDK versions either).
- **Decompression error causes.** `UNGZIP` / `INFLATE` raise Java's `… encountered an error
  while decompressing.` with the decoder's own message as the cause (`corrupt deflate
  stream`, `incomplete deflate stream`…) instead of the `ZipException` texts (`Not in GZIP
  format`, `incorrect header check`…). Like `GZIPInputStream`, `UNGZIP` concatenates all gzip
  members and silently ignores trailing bytes that do not start another member; `INFLATE`
  ignores anything after the end of the zlib stream, like `InflaterInputStream`.
- **Base58 error rendering.** `B58->` reports an invalid character as Java does (`Invalid
  input 'c' at position i.`, positions in UTF-16 units; characters `{`, `|`, `}` and `~` raise
  Java's `Index N out of bounds for length 123`); a character outside the Basic Multilingual
  Plane is rendered as the character itself where Java prints a lone surrogate.
- **`AESWRAP` nonce.** The 8 random bytes come from the operating system (`getrandom`)
  instead of `java.security.SecureRandom`; a failure to obtain entropy is a script error
  (`Unable to obtain random data from the operating system (…)`) rather than a JVM error.
- **`AESUNWRAP` of data without a nonce** (wrapped by a plain `CryptoUtils.wrap`) raises
  `8 > N`, the text of the `IllegalArgumentException` of `Arrays.copyOfRange`.
- **`ARGON2`** is a line-by-line port of BouncyCastle's `Argon2BytesGenerator` (with its own
  BLAKE2b) and produces the same bytes for every type, version, lane count, memory size and
  output length, including the sizes other libraries refuse (salts shorter than 8 bytes,
  memory below 8 KiB). The engine's `IllegalStateException` texts are raised unchanged
  (`iterations is less than: 1`, `memory is less than: 2 expected 2`, `lanes must be greater
  than 1`, `output length less than 4`), as is the `-1` of the `NegativeArraySizeException`
  for a negative `size`. Memory that cannot be allocated raises `Java heap space` (Java: an
  `OutOfMemoryError`, which is not catchable by scripts).

## 12. Capabilities and tokens

Capabilities (`CAPGET`, `CAPCHECK`, `CAPDEL`, `CAPADD`, `CAPEXPORT`, and the `argon2.*`,
`limits`, `maxops`… checks of other functions) follow `io.warp10.warp.sdk.Capabilities`, with
these differences:

- **Source of the capabilities.** Warp 10 reads them from the `.cap:` attributes of the token
  of the request (and from the `warp.capabilities.default` configuration). Here the host
  installs them on the stack (`Stack::capabilities_mut`) before running the script. Default
  capabilities merged there by the host can be revoked by `CAPDEL`; in Warp 10 the defaults
  survive `CAPDEL`.
- **`CAPADD`** needs a `TokenResolver` installed by the host (`Stack::set_token_resolver`) to
  decode tokens; without one every token is `invalid token.`, like an undecodable token in
  Warp 10.
- **`CAPGET` with a `LIST`** on a stack that was granted no capability at all returns an empty
  map, like a Warp 10 stack without a `Capabilities` object; once at least one capability is
  granted, missing names map to `NULL` as in Warp 10.

## 13. Runtime introspection and control

- **Function objects render as `'name' FUNCREF`.** Like `NamedWarpScriptFunction.toString()`,
  a function object on the stack (`mapper.sum`, the result of `FUNCREF`…) is snapshot and
  JSON-serialised as `'name' FUNCREF`; inside a macro, stack functions keep their bare name.
- **`REV`** reports `3.5.0`, the Warp 10 revision this runtime reproduces (`Stack::REVISION`),
  rather than a build-specific string; `MINREV` / `MAXREV` compare against it.
- **`DEREF`** returns a modified copy of the macro; Warp 10 modifies the macro object in place,
  which was observable through other references to the same macro.
- **`STMTPOS`**, **`LINEON`** / **`LINEOFF`**, **`ECHOON`** / **`ECHOOFF`**, **`TIMEON`** /
  **`TIMEOFF`**, **`DEBUGON`** / **`NDEBUGON`** / **`DEBUGOFF`** and **`TIMINGS`** /
  **`NOTIMINGS`** record their flags as stack attributes for the host (interactive console,
  HTTP endpoint) to honour; statement positions (`last.stmtpos`, `last.errorpos`) and per-line
  timings (`ELAPSED`) are not produced by the interpreter itself.
- **`PEEK`, `PEEKN`, `NPEEK`, `PSTACK`** print to the writer installed by the host
  (`Stack::set_interactive_writer`) and do nothing without one, like Warp 10 without an
  interactive writer attribute.
- **`CALL`** (external subprocesses) is not supported and fails with `CALL Not supported.`;
  **`EXTLOADED`** is always `false` (no Java extension can be loaded).
- **`REPORT`** produces the revision and runtime sections only (no JVM memory, Sensision
  metrics, extensions or plugins); it still requires the `report` capability.
- **`FUNCTIONS`** lists the functions in alphabetical order (Warp 10: hash order).
- **`SYMBOLS`**, **`SNAPSHOTALL`**, **`CAPGET`** and the other functions enumerating a Java
  `HashMap` return insertion order instead of hash order.
- **`IMPORT`** reproduces the Warp 10 rule table keyed by alias length: two aliases of the
  same length share one rule (the later namespace replaces the earlier one under the first
  alias), and removing an alias removes the rule of that length.
- **`GUARD`** failures are `WarpScriptATCException`s wrapped as `Line #n (Exception in GUARDed
  macro.)` at the top level; the exception class is reported by `ERROR` as
  `class io.warp10.script.WarpScriptATCException`.

## 14. Dates, times and durations

`ISO8601`, `TSELEMENTS` / `->TSELEMENTS`, `TSELEMENTS->`, `ADDDAYS`, `ADDMONTHS`,
`ADDYEARS`, `ADDDURATION`, `DURATION`, `ISODURATION`, `HUMANDURATION`, `TOTIMESTAMP`,
`NOTBEFORE` and `NOTAFTER` are implemented on `chrono` and the IANA database embedded by
`chrono-tz`, and verified against Joda-Time 2.13.1 and `java.time` for instants from year 0
to 10000, fixed offsets, historical LMT offsets (`+00:09:21`) and every daylight saving case
(gap, overlap, forward and backward arithmetic, east and west of Greenwich). The differences:

- **Range of years.** Joda-Time handles years up to ±292 million and `java.time` up to
  ±999 999 999; `chrono` stops at ±262 143. Instants beyond that raise `The instant N is out
  of range.`, dates `Value N for year must be in the range [-262143,262142]`, and
  `TOTIMESTAMP` reports `Invalid value for Year (valid values -262143 - 262142): N`.
- **Time zone database.** The zone identifiers, offsets and transition rules come from the
  IANA database compiled into `chrono-tz` rather than from the JDK / Joda `tzdata` of the
  Warp 10 host; recent rule changes may differ between the two.
- **Arithmetic overflow** (`ADDDAYS` with billions of days, `ADDDURATION` with huge periods)
  raises `The calculation caused an overflow.`, where Joda would raise an `ArithmeticException`
  with a similar text or wrap silently.
- **`TOTIMESTAMP` error details.** The cause texts follow `java.time`'s
  `DateTimeParseException` wording (`Text '…' could not be parsed at index N`, `Invalid value
  for HourOfDay (valid values 0 - 23): 25`, `Invalid date 'FEBRUARY 30'`…) but a few rare
  cases (an out-of-range year, a zone id that is not followed by `]`) are worded differently.
- **Non-numeric tselements** raise `class java.lang.String cannot be cast to class
  java.lang.Number`, the first part of the JVM's `ClassCastException` message (the JVM adds the
  class loader details).

## 15. `STRINGFORMAT` and counters

`STRINGFORMAT` implements the `java.util.Formatter` specification and is verified against
OpenJDK's `String.format` (flags, widths, precisions, argument indexes, half-up rounding of
the shortest decimal representation, the `IllegalFormatException` messages and the way Java
rebuilds a specifier in them, e.g. `%-2$d`). The differences:

- **Locales.** The locale argument selects the decimal and grouping separators of the CLDR
  data for the common European and Asian languages (`fr` → `1 234,5` with a narrow no-break
  space, `de` → `1.234,5`, `ru` → `1 234,5`…); other languages are rejected with
  `STRINGFORMAT expects a valid locale.`, and languages with non-Latin digits (`ar`, `fa`…)
  are not supported. Without a locale the root symbols apply (Warp 10: the JVM default
  locale).
- **`%t` / `%T` date-time conversions** are not supported and fail with `Date/time conversions
  (%t) are not supported.`; use `ISO8601` / `TSELEMENTS` instead.
- **`%h`** reproduces Java's `hashCode()` for strings, numbers, booleans, lists, maps and sets;
  objects whose Java hash code is an identity hash (macros, functions, byte arrays…) are
  reported as `0`.
- **`%c`** fails for every WarpScript value (`c != java.lang.Long`), exactly like Warp 10
  where `LONG`s are not `Character`s.
- **`%.1s` cutting a surrogate pair** yields U+FFFD where the JVM writes `?` when encoding the
  lone surrogate.

- **Counters** (`COUNTER`, `COUNTERVALUE`, `COUNTERDELTA`, `COUNTERSET`) are accepted by
  `TOSTRING`, `STRINGFORMAT` and `->JSON` like Java `AtomicLong`s, but the arithmetic and
  comparison operators do not treat them as numbers (Java would silently convert them to
  doubles through `Number.doubleValue()`).

## 16. `TEMPLATE`

`TEMPLATE` is a port of the subset of mustache.java 0.9.6 that Warp 10 exposes (the
`DefaultMustacheFactory` with inclusion disabled): variables, escaped and unescaped (`{{{x}}}`
and `{{& x}}`), sections, inverted sections, comments, delimiter changes and dotted names,
with the HTML escaping and the `Iterable`/`Boolean`/empty-string truthiness rules of
`BaseObjectHandler.iterate`. The differences:

- **Reflection is emulated for a fixed set of members.** mustache.java resolves a name that
  is not a map key by reflecting on the Java object (`ReflectionObjectHandler`). The port
  recognises the members a template may reasonably use: on strings `length`, `isEmpty`
  / `empty`, `toUpperCase`, `toLowerCase`, `trim`, `toString`; on `LONG`s and counters
  `intValue`, `longValue`, `doubleValue`, `toString`; on `DOUBLE`s the same plus `isNaN` /
  `naN` and `isInfinite` / `infinite`; on booleans `booleanValue`, `toString`; on lists `size`,
  `isEmpty` / `empty`, `toString`. Any other member name resolves to nothing (rendered as the
  empty string, exactly like an unknown name in Java). Maps are only ever queried by string
  key, never reflected: `{{m.size}}` is empty unless the map has a `size` key.
- **Partials and parents** (`{{> name}}`, `{{< name}}`) fail with `TEMPLATE failed.
  (Template inclusion is disabled.)` like Warp 10, whose resolver rejects every inclusion.
- **Accepted scope types.** Only strings, numbers (`LONG`, `DOUBLE`, counters), booleans,
  lists and maps (keys and values, recursively) may appear in the scope; anything else fails
  with `TEMPLATE failed. (Invalid type for template substitution.)`.
- **Error messages** of the mustache.java parser are reproduced, including the JDK 27 texts
  of the exceptions it lets escape (`Index 1 out of bounds for length 1` for a one-character
  delimiter used with `{{{`, `Range [0, -1) out of bounds for length 0` for an empty
  unescaped variable name); the `@[:line]` position suffix of `MustacheException` messages
  (an empty template name, as Warp 10 compiles the template from a string) is reproduced
  with the line number Java computes.

## 17. `ASREGS`

`ASREGS` follows the three passes of Warp 10 (inspection of the registers in use, breadth
first through the nested macros; assignment of the lowest free registers; rewriting), with
the same rules on what aborts the conversion, Java's 32-bit truncation of register numbers
(`4294967296 LOAD` becomes `PUSHR0`) and its `bitIndex < 0: -1` error on negative ones. The
differences:

- **A new macro is returned** (as for `DEREF`); Warp 10 rewrites the macro and its nested
  macros in place, which was observable through other references to the same macro, and it
  also modified list values (`[ 'a' 'b' ] 'STORE' FUNCREF 2 ->LIST ->MACRO`) shared with the
  rest of the script. Macro names are preserved.
- **Register numbers out of range** (`300 LOAD`, `[ 300 ] STORE`) keep their `n LOAD` /
  `n STORE` form instead of becoming a `PUSHR300` / `POPR300` that does not exist as a
  function; both fail identically at run time (`Invalid register number…`).
- **`] STORE` without an opening mark** fails with `ASREGS was unable to convert variables to
  registers.`; Warp 10 either leaves it alone or dies with an internal
  `ArrayIndexOutOfBoundsException`, depending on the elements.
- **A `null` value pushed by a list literal** (only possible through `->MACRO`) is treated
  like the `NULL` function, i.e. the corresponding value is `DROP`ped; Warp 10 keeps such a
  list as a `[ … ] STORE`.

## 18. Geo Time Series

The `GeoTimeSerie` model (`crates/warpscript/src/gts.rs`) reproduces the Java one: typed value
arrays fixed by the first datapoint with the same conversions (`GTSHelper.setValue`), byte
arrays decoded as ISO-8859-1, locations as GeoXP HHCodes and elevations as `LONG`s appearing
on demand, the `sorted` / `reversed` flags maintained exactly as `GTSHelper` does, the
iterative quicksort of Warp 10 (so that datapoints sharing a tick come out in the same,
unstable, order), `GeoTimeSerie.toString` and `dump` (`TOSTRING`), and the JSON form of
`GeoTimeSerieSerializer`. The differences:

- **Labels and attributes keep their insertion order.** Warp 10 stores them in `HashMap`s,
  so `LABELS`, `->JSON`, `TOSTRING` and `SNAPSHOT` list them in Java's hash bucket order;
  here they come out in the order they were set. Scripts that depend on the Java order of
  more than one label see a different (but deterministic) order.
- **Counters** (`COUNTER`) are accepted as values by `ADDVALUE` / `SETVALUE` and ignored,
  fixing the type of an untyped series to `BOOLEAN`: this is what `GTSHelper.setValue` does
  with a Java `AtomicLong`. As elsewhere (§15) they are not numbers for the tick, latitude,
  longitude and elevation arguments.
- **GTS encoders** are accepted wherever Warp 10 accepts them (`ADDVALUE`, `SETVALUE`,
  `TICKLIST`, `VALUES`, `CLONEEMPTY`, `TICKINDEX`, `SIZE`, `CLONE`, `TOSELECTOR`, `TIMECLIP`,
  `CLIP`, `CHUNK`, `MERGE`, `FOREACH`, `VALUEHISTOGRAM`, `POLYFUNC`, `PIVOT`…). The keys of
  `VALUEHISTOGRAM` follow the Java `HashMap` order.
- **`PIVOT` / `PIVOTSTRICT`** reproduce the Java `HashMap` orders of the result list and of
  the labels of each result (colliding hash buckets aside).
- **`[ value ] ADDVALUE`** (a one-element datapoint list) adds the value at tick `0` as
  `MACROMAPPER.listToObjects` intends; Warp 10 dies with a `ClassCastException` on the
  `Integer` tick that method builds.
- **`MERGE`** is a port of `mergeViaEncoders` (every datapoint appended to a new series
  carrying the metadata of the first one); with a boolean (the sorted merge) it emulates the
  Java `PriorityQueue` so that datapoints sharing a tick come out in the Warp 10 order.
- **`CHUNK` with a chunk width of `0`** fails with `/ by zero` on every path; Warp 10 only
  divides by the width when it has to align the last chunk or skip empty chunks, and loops
  over up to 2³¹ empty chunks otherwise. A non bucketized series **without datapoint**, with
  `keepempty` false and an explicit last chunk, yields an empty list; Warp 10 overflows on
  `Long.MIN_VALUE` and loops almost forever. A chunk of an encoder starting at
  `Long.MIN_VALUE` ends the chunk walk, where Warp 10 wraps around and loops forever.
- **`CROP` of a bucketized series without datapoint on a bucket boundary** fails with the
  JDK message of the `NullPointerException` Warp 10 dies with (`Cannot load from long array
  because "cropped.ticks" is null`).
- **Operators on series** (`+ - * /`, the comparisons, `&& || !`, the bitwise operators,
  `->DOUBLEBITS` and friends, and the numerical functions such as `ABS`, `**`, `MAX` or
  `ATAN2`; `GTSOpsHelper`) follow Warp 10, quirks included: two series are combined on their
  common ticks after being sorted *in place*, the result of two bucketized series is
  bucketized only on aligned grids, `GTS GTS ==` with a series of another type fails with
  the message Warp 10 prints without a space after the operator name, an empty result of a
  bitwise operator or of a `LONG` series compared to `NaN` keeps its type. The only
  difference: the bit conversions of a series build the result directly where Warp 10
  round-trips through a `GTSEncoder` (same datapoints, metadata and order).
- **Selectors** (`PARSESELECTOR`, `TOSELECTOR`, `filter.byselector`) behave as with
  `warp10.absent.label.support` enabled (the oracle configuration): `label=` selects the
  series without that label. `filter.bymetadata` compares the name, labels, attributes and
  ids of the metadata like Thrift's `Metadata.equals`, except that an unset map and an empty
  one are equal; while `FILTER` runs, the series carry the class ids Warp 10 numbers them with
  (so, as in Warp 10, only the first series of the first list can equal a reference series
  with a class id of `0`). Its `SNAPSHOT` lists the reference metadata in insertion order
  where Warp 10 follows the order of a `HashSet`.
- **`CLONE` and the functions returning a copy** (`TIMESHIFT`, `TIMESCALE`, `CROP`…) reset the
  `renamed` flag like `GeoTimeSerie.clone`; the flag is only read by `UPDATE`.
- **`VALUESORT` / `RVALUESORT` / `VALUEDEDUP` on a `BOOLEAN` series** whose `false` or
  `true` datapoints are not in tick order: Warp 10 sorts each half with an off-by-one upper
  bound and reads (and swaps in) the array slot past the last datapoint, producing a bogus
  tick (`0` or a stale value). The port sorts within bounds and otherwise follows Warp 10,
  including the stale `reversed` flag it consults to decide the reading direction of each
  half (`[ 3 1 2 ] true` values come out as ticks `3 2 1`).
- **`DEDUP` with a macro or mapper** runs `GTSHelper.map` in its deduplicating mode (one
  application per distinct tick, on the window of that tick) like Warp 10, with the same
  differences as `MAP` (§19).
- **`LTTB` / `TLTTB`** reproduce Warp 10, quirks included: the reference vertex of every
  triangle stays the first datapoint (Warp 10 never moves it), `TLTTB` divides by a time
  bucket of `0` (`/ by zero`) on series spanning two time units, and small, `STRING` and
  `BOOLEAN` series are returned as they are (the same object). A bucket whose triangles
  all have a `NaN` area is skipped, as in the reference revision; Warp 10 3.5.0 fails with
  an `ArrayIndexOutOfBoundsException` (`-1`) there. `SINGLEEXPONENTIALSMOOTHING` and
  `DOUBLEEXPONENTIALSMOOTHING` sort their input in place and accept a `NaN` factor, as in
  Warp 10.
- **`DTW` / `ZDTW` / `RAWDTW` on `'locations'` or `'elevations'`** compare the locations or
  elevations of the datapoints. Warp 10 reads the whole backing arrays of the series
  (`getOriginalLocations`), whose unused capacity (at least 64 slots for a series built by
  `ADDVALUE`) holds zeros, i.e. extra datapoints at latitude -90, longitude -180 or at
  elevation 0, which skew the min-max normalization of `DTW`; it also fills the elevations of
  the second series using the length of the first one. Otherwise the family follows Warp 10,
  quirks included: `ZDTW` divides the second series by its variance, `DoubleUtils.muvar`
  shifts by the first zero value (and adds back a `n`-th of it), the first row of the warping
  matrix starts from zeros, `OPTDTW` bounds the search with the distance found at position
  `count - 1` of its heap array (not the `count`-th best one), the timestamps of a series
  without datapoint fail with `NullPointerException`. The geographic distances use the JDK
  9+ `Math.toRadians` (a multiplication), as Warp 10 does on a current JDK; on JDK 8 the last
  digits differ.
- **`PATTERNS` / `PATTERNDETECTION` / `DISCORDS`** (and their `Z` forms) produce the bSAX
  words of Warp 10 (`SAXUtils`: Acklam's inverse normal CDF refined with Cody's `erfc`; a
  breakpoint computed by the transcendental functions may differ by an ulp, which only
  matters for an aggregate exactly on it) and keep its quirks: `DISCORDS` only looks for
  nearest neighbours among the windows of the same word and of the words before it in
  lexicographic order, skips the second of two overlapping discords twice, and fails with
  `Index -1 out of bounds for length 0` for a count of `0`. The differences: **a
  non positive window** fails with `Index n out of bounds for length n` (the number of
  datapoints): Warp 10 reads past the datapoints into the tick array, which is exactly sized
  for a series built by `BUCKETIZE` (the usual input) but holds stale slots otherwise; **a
  `DISCORDS` distance ratio** that keeps the last candidate (its distance times the ratio is
  not below the best one) ends the pruning, where Warp 10 loops forever.
- **`POLYFUNC` / `RANDPDF` / `SRANDPDF`** follow Warp 10: `POLYFUNC` drops the trailing
  zero coefficients like `PolynomialFunction`, builds a series without metadata (or
  bucketization) and evaluates a bucketized series on every bucket (or bucket index), most
  recent first; it is a mapper too (`TYPEOF` is `AGGREGATOR`). `RANDPDF` sorts the values
  of the histogram in their natural order (`TreeMap`: they must be of one comparable class)
  and leaves the stack below untouched. The unseeded `RANDPDF` draws from the system entropy
  (§5.3), `SRANDPDF` from the `PRNG` sequence exactly like Warp 10.
- **`PROB` / `CPROB` / `PROBABILITY`** follow Warp 10: `CPROB` with a separator sorts its
  input by value in place and fails with `NullPointerException` when an event has leading or
  trailing blanks (it counts the trimmed events but looks the untrimmed ones up). The
  `PROBABILITY` function lists its values in insertion order in its snapshot (Warp 10:
  `HashMap` order).
- **`LOWESS` / `RLOWESS` / `POLYFIT`** compute bit-identical results (the polynomial fits
  of degree two and more go through a port of Commons Math's Levenberg-Marquardt optimizer,
  `curvefit.rs`) and keep Warp 10's quirks: a negative `RLOWESS` radius is truncated to `0`
  (`(long) Math.pow(10, d)` is `0`), a first tick of `-1` is reported as a duplicate tick, a
  tick skipped without radius (a series starting before `-1`) and a bucketized series with
  missing values and locations or elevations fail with `NullPointerException`, and `POLYFIT`
  never limits its iterations (Warp 10 drops the fitter `withMaxIterations` returns). The
  Commons Math exceptions escape unwrapped with their own texts (`illegal state: maximal
  count (2,147,483,647) exceeded: iterations`, `empty polynomials coefficients array`…).
  The difference: **on non finite values** the optimizer cycles through the same state and
  Warp 10 spins through 2³¹ iterations (minutes to hours) before raising
  `TooManyIterationsException`; the port detects the cycle and raises it at once. A degree
  below `-1` fails with the JDK 11+ `NegativeArraySizeException` text (`-1`; JDK 8: none),
  robustness iterations on a series without datapoint with `Index 0 out of bounds for
  length 0` (JDK 8: `0`).
- **`LASTSORT` with series of mixed value types** compares the values as strings (`"10"`
  before `"2"`), an inconsistent order for which Java's TimSort and Rust's merge sort may
  produce different permutations; consistent comparisons (same type, or `LONG` / `DOUBLE`)
  sort identically.
- **JDK exception texts** are those of JDK 11 and later (`Index -1 out of bounds for length
  0` for `[ ] COMMONTICKS`, `Index 0 out of bounds for length 0` for `RESETS` on a numeric
  series without datapoint, `class java.lang.String cannot be cast to class java.lang.Number`
  for `INTEGRATE` on a `STRING` series); a Warp 10 running on JDK 8 prints `-1`, `0` and
  `java.lang.String cannot be cast to java.lang.Number`.
- **`MODE` compares values by value.** Warp 10 compares the boxed `Object`s by reference
  (`lastValue != value`), so it only groups `LONG`s in the `Integer` cache (-128 to 127),
  `BOOLEAN`s and `STRING`s that happen to be the same instance; every other value counts as
  its own mode (a `DOUBLE` series reports all its values). The port groups equal values.
- **`MUSIGMA`, `NSUMSUMSQ` and `LR`** compute with `BigDecimal` like Warp 10 (§22);
  `STANDARDIZE`, `ZSCORE`, `SKEWNESS` and `KURTOSIS` use the plain double arithmetic of their
  Java counterparts.

## 19. `BUCKETIZE`, `MAP` and the aggregators

`BUCKETIZE.CALENDAR` / `UNBUCKETIZE.CALENDAR` are ports of `BUCKETIZECALENDAR` (the search
of the index and offset of the last bucket, `durationBucketize`, `addNonNegativePeriod` and
its error) on the `ADDDURATION` arithmetic of §14. Like Warp 10, `BUCKETIZE.CALENDAR`
removes the time zone from the parameter list it is given, sorts its input series in place,
hands the bucketizers the datapoints of each bucket most recent first and reuses a single
sub-series for the macro bucketizers. The differences:

- **The three calendar attributes are inserted in the order Warp 10's `HashMap` lists
  them** (`.buckettimezone`, `.bucketduration`, `.bucketoffset`), after the attributes of the
  input series (§18: insertion order elsewhere).
- **Periods overflowing the date arithmetic** fail with `The calculation caused an
  overflow.`; Joda reports its own limits (`Value cannot fit in an int: …`, `Value … for
  year must be in the range [-292275054,292278993]`), and `chrono` stops at year ±262143
  where Joda goes on to ±292 million. Periods with a field beyond a Java `int` parse
  differently (Warp 10 ends with `/ by zero`).
- **An empty duration** fails with the JDK 17+ text `Index 0 out of bounds for length 0`
  (JDK 8: `String index out of range: 0`).

`BUCKETIZE` is a port of `GTSHelper.bucketize` (parameter derivation included: a `0` last
bucket aligns the last tick on the span, a `0` span derives from the count and a `-1` span
makes the first tick start the first bucket) with the aggregator and macro bucketizers.
`MAP` is a port of `MAP` / `GTSHelper.map` / `MACROMAPPER` (both parameter forms, pre and
post windows in ticks or in time, occurrences and their backwards form, output ticks, step,
the tick override, the duplicate-tick handling, the Java `int` / `Integer.MAX_VALUE`
cappings and the multi-output maps). The
aggregators (`bucketizer.*`, `mapper.*`, `reducer.*`) are the shared `WarpScriptAggregatorFunction`
classes of Warp 10 (`Sum`, `Mean`, `Min`, `Max`, `First`, `Last`, `Count`, `And`, `Or`, `RMS`,
`Variance` / `VarianceWelford`, `StandardDeviation`, `CircularMean`, `Median`, `MAD`,
`Percentile`, `Join` so far), each registered name keeping its `ignoreNulls` / `forbidNulls`
flag; the parameterized ones (`true bucketizer.var`, `50 'type7' mapper.percentile`,
`'-' reducer.join`, `360 bucketizer.mean.circular`) are built by the same stack functions
as in Warp 10 and snapshot as that builder call. The differences:

- **A macro bucketizer or mapper receives a copy of the result's metadata** with each
  bucket's or window's datapoints; Warp 10 shares the metadata object, so a macro renaming
  or relabelling the sub-series would alter the bucketized or mapped series.
- **Java runtime exceptions escape the frameworks unwrapped.** `MAP` wraps the
  `WarpScriptException`s of a mapper (`MAP was given invalid parameters. (…)`) but lets the
  Java runtime exceptions through, as Warp 10 does: a macro mapper leaving an empty stack
  (`Empty stack.`, an `InformativeEmptyStackException`), a map whose values are not lists
  (`class java.lang.Long cannot be cast to class java.util.List`), a non numeric tick in a
  result list (`class java.lang.String cannot be cast to class java.lang.Number`), or the
  tick override on a single-element list result, whose `0` tick is a Java `Integer` (`class
  java.lang.Integer cannot be cast to class java.lang.Long`). `ERROR` reports the class of
  these as Warp 10 does (`class java.lang.ClassCastException`,
  `class io.warp10.script.InformativeEmptyStackException`,
  `class java.lang.ArrayIndexOutOfBoundsException`, `class java.lang.RuntimeException`),
  with an empty `stacktrace`.
- **A map returned by a macro mapper is read as it is**; Warp 10 replaces each of its list
  values by the array it builds from them (`MACROMAPPER.listToObjects`) — the mutated map is
  unreachable afterwards, so nothing observable differs. The series of the named outputs are
  created in the mapper's order and returned sorted by name (`TreeMap`), as in Warp 10.
- **A `null` result** (alone or as the last element of a list) is left on the stack, like
  Warp 10 does — scripts relying on that garbage behave identically.
- **Java runtime exceptions are reproduced by message**: a non numeric value after a numeric
  first one raises the `ClassCastException` text (`class java.lang.String cannot be cast to
  class java.lang.Number`), `MAD` on a window of booleans or strings fails like Java's
  `Arrays.sort` / `(double)` casts (`class java.lang.Boolean cannot be cast to class
  java.lang.Double`, `Unable to compute median on an even number of non numeric values.`),
  and `median` of no value at all (`Index -1 out of bounds for length 0`) or `mad` of an
  empty window (`Index 0 out of bounds for length 0`) report the JDK 11+
  `ArrayIndexOutOfBoundsException` texts. The `RuntimeException` of a non numeric
  `median` window propagates bare (`bucketizer.median can only operate on numeric Geo Time
  Series.`) while `percentile` wraps it (`java.lang.RuntimeException: … (…)`), as in Warp 10.
- **`median` and `percentile` on mixed `LONG` / `DOUBLE` windows** (only reducers produce
  them) compare the values exactly like `EQ.compare`; Warp 10 fails with `Infinite or NaN`
  when a `LONG` meets a `NaN` or infinite `DOUBLE` in its `BigDecimal` comparison, the port
  orders infinities naturally and puts `NaN` last. `MAD` on such a window fails like Java's
  `Arrays.sort` (`class java.lang.Long cannot be cast to class java.lang.Double`, naming the
  class of the first value and the first other class).
- **Warp 10 quirks reproduced**: the Welford variance divides by the index in the window
  (nulls included), so a skipped `null` skews `reducer.var.welford`; `MAD` keeps the
  `NO_LOCATION` / `NO_ELEVATION` markers that `Arrays.binarySearch` does not land on in its
  location and elevation medians (the median elevation of two elevated datapoints among six
  is `(100 + Long.MIN_VALUE) / 2`); `join` counts a skipped `null` as a first element
  (`-1-2` for `null 1 2`); a `NaN` percentile rank is accepted (and selects the lowest
  value). `MAD` sorts copies of the window's locations and elevations where Warp 10 sorts
  the framework's arrays in place, with no observable difference.
- **The mappers reading the index of the current tick in the window** (`mapper.product`,
  `mapper.dotproduct`…) read `Window::tick_index`; an output tick holding no datapoint
  (index `-1`) raises the `ArrayIndexOutOfBoundsException` Warp 10 raises, with the JDK 11+
  text (`Index -1 out of bounds for length 1`, Warp 10 on JDK 8: `-1`).
- **The change and motion mappers** (`mapper.delta`, `rate`, `highest`, `lowest`, `hdist`,
  `hspeed`, `vdist`, `vspeed`, `truecourse`, which the three frameworks accept) keep the Java
  quirks: `delta` and `rate` are at the tick delta and follow the type of the last value
  (wrapping `long` arithmetic, no result for non numeric values), `hspeed` sums the
  distances in the window's order but divides by its span, `truecourse` uses the first and
  last datapoints of the window and Williams' west-positive formula with the cosine of the
  destination's longitude (heading east gives 270°), and a lone datapoint at
  `Long.MAX_VALUE` / `Long.MIN_VALUE` makes `GTSHelper.getFirstLastTicks` return `-1`
  (`Index -1 out of bounds for length 1`). The degree / radian conversions are those of
  JDK 9+ (`x * (PI / 180)`); Warp 10 3.5 on JDK 8 computes `x / 180 * PI`, so its
  `truecourse` may differ in the last digit (`315.08876647084264` there, `315.0887664708421`
  here).
- **The kernel smoothers** (`step width mapper.kernel.*`) use the Java weights (a width of 1
  divides 0 by 0, giving `NaN` weights for the kernels of `u`) and the Java failures: a
  `0` step raises `/ by zero`, a negative one an `ArrayIndexOutOfBoundsException`, the
  `int` width `-1` a `NegativeArraySizeException` (message `-2147483648`, JDK 11+) and
  `-3` / `-5` the `OutOfMemoryError` `Requested array size exceeds VM limit`. The port
  refuses kernels of more than 2^24 weights (a width beyond 2^25) with the
  `OutOfMemoryError` `Java heap space`, where Java allocates as much as its heap allows.
- **The regular expression mappers** use the `MATCH` / `REPLACE` engine (§8): a non
  `STRING` value raises the `ClassCastException` of `Pattern.matcher((String) value)`, and
  the replacement errors (`No group 2`, `Illegal group reference`) are the runtime
  exceptions `MAP` lets through. Both snapshot as a `FUNCREF`, as in Warp 10.
- **`mapper.product` / `reducer.product`** compute the exact `BigDecimal` product: the
  wrapping product when all the values are `LONG`s, the correctly rounded product otherwise
  (`0.1 0.2 0.3` give `0.006`, `1e300 1e300 1e-300` give `1.0000000000000002E300`, a `-0.0`
  factor gives `0.0`), and the `NumberFormatException` `Infinite or NaN` on a non-finite
  value.
- **The single datapoint mappers** (`mapper.abs`, `ceil`, `floor`, `round`, `tanh`,
  `sigmoid`, `finite`, `toboolean`, `tolong`, `todouble`, `tostring`, `lat`, `lon`, `elev`,
  `geo.clear`, `tick`, and the `year`, `month`, `day`, `weekday`, `hour`, `minute`,
  `second` builders) reproduce the Java quirks: `mapper.round` of a `LONG` goes through
  `Math.round(float)` (`123456789` gives `123456792`, `Long.MAX_VALUE` gives
  `2147483647`); `mapper.abs` keeps `Long.MIN_VALUE`; a `LONG` millisecond offset given to
  the date-time builders is truncated to an `int` (`9999999999` fails with `Millis out of
  range: 1410065407`). The differences:
  - The date-time builders keep the zone identifier as given: Joda resolves the IANA links
    (`'US/Pacific' mapper.hour` snapshots as `'America%2FLos_Angeles' mapper.hour` in
    Warp 10, `'US%2FPacific' mapper.hour` here; `EST` and `GMT` likewise stay themselves).
    The computed fields are identical.
  - Ticks whose instant lies beyond the years `chrono` represents (±262143, i.e. ticks
    beyond about ±8.2 × 10¹⁸ microseconds) make the date-time mappers fail with
    `The instant … is out of range.`; Joda computes them (year `294247` for
    `Long.MAX_VALUE`).
  - `mapper.tolong` parses with `Long.valueOf` restricted to ASCII digits; Java also
    accepts the other Unicode decimal digits (`'٥'` gives 5 in Warp 10, no value here).
- **The mappers configured with constants** (`mapper.add`, `mul`, `mod`, `min.x`, `max.x`,
  `pow`, `sqrt`, `exp`, `log`, `replace`, `npdf`, `parsedouble`) reproduce the Java
  objects, quirks included: a `LONG` operand of `mapper.add` / `mapper.mul` truncates a
  `DOUBLE` value and gives a `LONG` (`3 mapper.add` maps `2.7` to `5`), the integer
  operations wrap, `0 mapper.mod` on a `LONG` raises the `ArithmeticException` `/ by zero`
  that `MAP` lets through, `mapper.log` and `mapper.sqrt` snapshot as the stored operand
  (`10 mapper.log` as `2.302585092994046 mapper.log`, `mapper.sqrt` as `0.5 mapper.sqrt`),
  `mapper.parsedouble` as `'mapper.parsedouble' FUNCREF`, and `mapper.replace` takes no
  single value check. `mapper.parsedouble` is `DecimalFormat.parse`: the longest numeric
  prefix, grouping separators ignored, an `E` exponent (no `+`; an exponent beyond a
  `long` is ignored, one within is truncated to an `int`), `∞` and `NaN`. The tag is read
  like `Locale.forLanguageTag` (`fr_FR` is the root locale) and the separators are those of
  the `STRINGFORMAT` locales (§15, JDK 11+ CLDR data: the French grouping separator is
  U+202F and `NaN` is spelled `NaN`; Warp 10 3.5 on its JDK 8 image uses U+00A0 and
  U+FFFD). Only ASCII digits are recognised (Java also reads the other Unicode decimal
  digits).
- **`CompareTo`** (`mapper.gt` … `mapper.lt.elev`, accepted by the three frameworks)
  keeps the earliest passing datapoint of the window like Warp 10: a `LONG` reference
  truncates `DOUBLE` values, a `DOUBLE` one uses `Double.compare` (`NaN` greatest), a
  `STRING` one compares the `toString()` of any value by UTF-16 code unit, a `BOOLEAN` one
  only matches booleans, and the latitude / longitude of a datapoint without location are
  those of the `NO_LOCATION` HHCode. A `STRING` reference meeting a `null` value (a
  `REDUCE` window) raises the `NullPointerException` Warp 10 raises; Java's has no message
  (`ERROR` reports `null`), the port's message is `NullPointerException`, which is how Warp
  10 renders it in the error message.
- **`Double.valueOf` is reproduced exactly** wherever Warp 10 uses it (`TODOUBLE`,
  `mapper.todouble`, string series converted to `DOUBLE`): surrounding control characters
  and spaces are trimmed, one `f` / `F` / `d` / `D` suffix is accepted, hexadecimal
  literals (`0x1.8p3`) are parsed and correctly rounded, and the Rust-only forms (`inf`,
  `nan`, `infinity`) are rejected (`For input string: "nan"`, `empty String`).
- **`REDUCE` / `PREDUCE` and `PARTITION` / `STRICTPARTITION`** port `GTSHelper.partition`
  and `GTSHelper.reduceUnflattened`: the classes, their common labels and attributes, the
  bucketization of the results (same span, congruent last buckets), the `Long.MIN_VALUE`
  ticks and `null` values of the series without a datapoint at a tick, the extra label map
  of the class handed to the reducer, repeated ticks reduced once each, the named outputs of
  macro reducers (sorted like a `TreeMap`), the tick override and the in-place sort of the
  input series. Unlike the other `HashMap`s of Warp 10, the order of the classes is
  reproduced: it is computed from the Java hash codes of the label maps and the table sizes
  Java uses (only buckets of eight colliding keys or more, which Java turns into trees,
  could be ordered differently). The reducer reads copies of the series, so a macro reducer
  modifying them does not affect the ticks being reduced; the name of a series that was never
  named is the empty string in the arguments of a macro reducer (Java: `null`).
- **`MACROMAPPER`, `MACROREDUCER` and `MACROBUCKETIZER`** wrap a macro into an aggregator of
  the three roles that receives its arguments as lists (`[ tick [ names ] [ labels ] [ ticks ]
  [ lats ] [ lons ] [ elevations ] [ values ] ]`, one name and label map for `BUCKETIZE`, one
  per datapoint for `MAP`, plus the class labels for `REDUCE`) and snapshots as `<% <% … %>
  MACROMAPPER %> EVAL`. A map result under `BUCKETIZE` raises the `ClassCastException` of
  Java's cast to `Object[]`. `BUCKETIZE` works on a copy of the sorted series so that a
  macro reaching the series cannot interfere with the bucketization.
- **`STRICTMAPPER` / `STRICTREDUCER`, `reducer.argmin` / `argmax` and
  `reducer.shannonentropy.*`** follow Warp 10, snapshots included (the macro form of
  `STRICTMAPPER` is a macro holding the run-time function, `<% <% <% … %> 1 2 STRICTMAPPER
  %> EVAL EVAL %>`). A strict wrapper around a `MACROMAPPER` / `MACROREDUCER` runs the macro
  on the framework's stack. `argmin` / `argmax` under `BUCKETIZE` raise the
  `ArrayIndexOutOfBoundsException` of the single label map Java hands bucketizers
  (`Index 1 out of bounds for length 1`), and the entropy of no value is `-0.0` (`0 /
  log(0)`).
- **`APPLY` / `PAPPLY`** port `GTSHelper.partitionAndApplyUnflattened` and
  `applyNAryFunction`: only the collections of more than one series are partitioned (each
  series once; the first collection when there are none), a collection of a single series
  takes part in every class, the validator macro receives the per-collection lists and the
  class labels, the result is named after the last series of the class (`op.*` receive the
  index of the collection of each series in place of its tick), the class order is Java's
  hash order, and the class and labels ids of the input series are unset afterwards. The
  `op.*` operations reproduce the Java type rules (`true + 1.5` is `2.5`, a first string
  makes `op.add` and `op.mul` concatenate, `NaN` never breaks the order of `op.lt` …
  `op.ge`).
- **`FILTER` / `PFILTER`** partition like `APPLY` and hand each class to the filter:
  `filter.byclass`, `filter.bylabels` / `byattr` / `bylabelsattr`, `filter.bysize`,
  `filter.last.*`, `filter.any.*` / `filter.all.*` and `MACROFILTER` (whose list result is
  taken as is). The selectors `''` and `'='` of `filter.bylabels` require the label to be
  absent, as with `warp10.absent.label.support = true` (the setting of the Warp 10 3.5
  image); Warp 10 configured without it compiles them as exact matches of the empty string.
  The snapshot of `filter.bylabels` reproduces Java's missing spaces (`{ 'a' 'b''c' 'd'}`).
  `filter.bymetadata`, `filter.byselector` (they need `PARSESELECTOR`) and
  `filter.latencies` are not ported yet.
- **`FILL` and the fillers** port `GTSHelper.fill` in its four forms (cross fill and
  univariate fill, each with a windowed or a single value filler), `FILL.fillerFromMacro`,
  `MACROFILLER` and the `filler.*` objects, with `FILLVALUE`, `FILLPREVIOUS`, `FILLNEXT`,
  `FILLTICKS` and `INTERPOLATE`. The Commons Math 3.6.1 interpolators the fillers build on
  (linear, spline, Akima, divided differences, LOESS) are ported operation for operation
  (`interpolation.rs`), so the filled values are bit-identical. The Warp 10 quirks are
  reproduced: `FILL` sorts its input series in place; the univariate fills return a list
  even for a single series; `FILL.sortDedupTicks` sorts the user's tick list in place and
  drops the first occurrence of the first duplicated tick too (`[ 0 15 15 20 ]` fills `0`
  and `20`); the verification of a single value fill walks the series along the ticks
  (a tick equal to the next tick of the series is filled again, one following an existing
  tick may be skipped); a macro filler adds its datapoints to the original series, which
  the bucket and cross fills keep reading until a limit of the stack stops them;
  `filler.trend` throws the `NullPointerException` of Java whenever a projected window
  slot lacks a location or an elevation, and stores the projected elevation, rounded, as
  the location; `filler.interpolate` rounds elevations where `INTERPOLATE` truncates them,
  and rejects a single located (or elevated) datapoint (`number of points (1)`);
  `FILLVALUE` skips the bucket following a tick that is not on a bucket; `FILLTICKS` adds
  the datapoint at existing ticks too; a single value fill whose ticks deduplicate to none
  falls back to the empty buckets (`/ by zero` on a series that is not bucketized). The
  differences:
  - The messages follow JDK 11+: `class java.lang.Long cannot be cast to class
    java.lang.Double` (JDK 8: `java.lang.Long cannot be cast to java.lang.Double`), and the
    `NotFiniteNumberException` of LOESS reads `NaN is not a finite number` (JDK 8 renders
    the `NaN` of `MessageFormat` as U+FFFD) or `∞ is not a finite number`.
  - A windowed fill of a bucketized series holding a tick on the bucket grid before its
    first bucket enumerates the grid up to that tick around the whole `long` range, like
    Java, until memory runs out (Java: `OutOfMemoryError`).

## 20. Encoders and wrapped series (`GTSEncoder`, `GTSWrapper`)

`encoder.rs` and `thrift.rs` port `GTSEncoder`, `GTSDecoder`, `GTSWrapperHelper` and the
libthrift compact protocol byte for byte, including the errors of malformed payloads (the
Java runtime exceptions — `BufferUnderflowException`, `IllegalArgumentException`… — carry
their JDK 11+ messages). The differences:

- **Gzipped content differs.** The deflate implementation (miniz) does not produce the
  same bytes as the JDK's zlib, so compressed wraps (and the `SNAPSHOT` of large series and
  encoders) differ from Warp 10's, and in borderline
  cases the number of compression passes (or the choice not to compress) may too. Both
  sides read each other's output.
- **Label and attribute order** is the insertion order (§18), where Java serializes its
  `HashMap` order.
- **Nesting depth**: skipping unknown Thrift fields stops at 512 nested levels with a
  `StackOverflowError`, where Java overflows its thread stack after thousands.
- **Huge lengths** in a payload are rejected once they exceed the data, without emulating
  the `OutOfMemoryError` Java may raise first.
- **Metadata are copied, not shared**: after `merge` / `reset` Java encoders share the
  metadata object of their source, so that later renames affect both; here each encoder
  owns its copy.
- **Not ported**: the AES wrapping keys of encoders (encrypted blocks are skipped like a
  decoder without key does) and read-only encoders, neither of which WarpScript can create.
- **`OPTIMIZE` only checks its arguments**: Rust vectors are not over-allocated the way the
  Java arrays of series and encoder buffers are, so there is no spare capacity to release.
- **`ENCODER->` pushes copies** of the labels and attributes maps; Java pushes the maps of
  the decoder metadata themselves, which a later `PUT` on them would modify.
- **Error causes of `PARSEVALUE`** carry the JDK 27 texts (`Index 0 out of bounds for length
  0`, `No digits found.`…) where the oracle prints JDK 8's.

## 21. Outlier tests and the t distribution

`THRESHOLDTEST`, `ZSCORETEST`, `GRUBBSTEST` and `ESDTEST` follow `GTSOutliersHelper`. The
critical values come from a port of Commons Math's `TDistribution` (`distribution.rs`), which
evaluates with `FastMath` where the port uses the standard library: the quantiles may differ
in their last bits (they match Commons Math exactly on the tested values), which can only flip
a test whose statistic equals its critical value to the last bit. Messages of Commons Math
render `NaN` as `NaN` (JDK 11+) where the oracle's JDK 8 prints `\uFFFD`.

## 22. `BIGDECIMAL`

`bigdecimal.rs` ports the `BigDecimal` / `BigInteger` operations WarpScript uses (on
`num-bigint`): scales of the results, rounding, `toString` / `toPlainString`, the exceptions
and their JDK 27 messages (the oracle's JDK 8 throws message-less `NumberFormatException`s
for malformed strings). `->BD`, `BD->`, every `BD*` function, the operators (`+ - * / %`,
the comparisons, `& | ^ << >> >>>`), `STRINGFORMAT` (`%e %f %g` on the exact digits),
`->HEX` / `->BIN`, `PARSEVALUE`, encoders and series accept or produce `BIGDECIMAL`s like
Warp 10. The differences:

- **Probable primes.** `BDISPROBABLEPRIME` and `BDNEXTPROBABLEPRIME` use deterministic
  Miller-Rabin rounds (exact below 3.3 × 10^24) where Java draws random bases; the answers
  only differ for composites Java would wrongly accept with negligible probability.
  `BDPROBABLEPRIME` / `SBDPROBABLEPRIME` draw odd candidates of the requested length until one
  passes, where Java sieves: the primes drawn from a given seed differ from Warp 10's.
- **`BDRAND`** draws from a SHA-256 counter mode generator keyed by the operating system
  (Java: `SecureRandom`); `SBDRAND` reproduces Warp 10's draws from the seeded PRNG.

## 23. Matrices and vectors

`MATRIX` and `VECTOR` values follow Commons Math's `Array2DRowRealMatrix` and
`ArrayRealVector` (arithmetic order, `LUDecomposition` with its `1e-11` singularity threshold,
exception messages, `toString`). Commons Math switches to a `BlockRealMatrix` above 4096
entries, whose products sum in another order (and whose `toString` differs): the port always
computes like `Array2DRowRealMatrix`.

## 24. Platform functions

The runtime runs inside InfluxDB, which authenticates the requests: it knows no Warp 10
token, so `TOKENINFO`, `GETHOOK`, `KVSTORE`, `KVLOAD` and `DUMP` behave as on a Warp 10
instance that did not issue the token (`Invalid token.`), and `RUNNERNONCE` / `RUNNERPATH`,
which need the runner key of a Warp 10 instance, push `NULL`. The manager switches
(`UPDATEON`…), the WarpFleet repositories, the accelerator flags and `SETMACROCONFIG` act on
the stack (attributes and configuration) rather than on a JVM-wide state; `ACCEL.REPORT`
reports no accelerator; `LOGMSG`, `NOLOG` and `LOGINIT` log nothing; `WSAUDITMODE` does not
change how scripts are parsed, so `WSAUDIT` returns an empty list. `[[]]` pushes a `LIST`
where Warp 10 pushes a `java.util.Vector` (`VLIST`).

## 25. Immutable collections

`IMMUTABLE` pushes a frozen **copy** of the list, set or map, where Warp 10 pushes an
unmodifiable *view*: later changes to the original collection are not seen through the
immutable one. Every mutating function (`SET`, `PUT`, `REMOVE`, `REVERSE`, `+!`, `LSORT`,
`SHUFFLE`, `SORTBY`, `SORTWITH`, `MSORT`) fails on a frozen collection with the same
`UnsupportedOperationException` as Warp 10, in the same check order; `APPEND` copies a
frozen list or set like Warp 10 does.

## 26. Python pickles

`->PICKLE` reproduces the protocol 2 output of Pyrolite 4.30 byte for byte, including its
memo (by Java `hashCode` / `equals`) and the Java `HashMap` / `HashSet` iteration order of
sets and series maps. Labels and attributes follow the Java `HashMap` order of their keys,
which may differ from Warp 10 when two keys collide in a bucket (§18).

`PICKLE->` decodes protocols 0 to 5 like Pyrolite, but pushes WarpScript values where
Warp 10 pushes opaque Java objects:

- tuples and `array.array`s are `LIST`s (Warp 10: `Object[]` and primitive arrays), and
  `array('u')` / `array('c')` a `STRING` (`char[]`); `float` arrays hold the `DOUBLE` of
  the shortest decimal form of each `float`;
- integers beyond the `LONG` range are `BIGDECIMAL`s (Warp 10: `BigInteger`);
- `datetime` and `date` are their instant in milliseconds, as a `LONG` (Warp 10:
  `GregorianCalendar`); dates before 1582 use the proleptic Gregorian calendar;
- `complex`, `time` and `timedelta` are `MAP`s of their fields; time zones are their id as a
  `STRING`, `tzinfo` objects and constructors `NULL`; Python exceptions are their message;
- byte arrays, tuples, arrays and exceptions, which Java compares by identity, become
  values: equal ones merge as map keys or set members;
- `bytearray(…, encoding)` supports ISO-8859-1, UTF-8 and US-ASCII only;
- a lone UTF-16 surrogate becomes U+FFFD;
- structures nested more than 2000 levels deep fail with `StackOverflowError`.

Error messages follow JDK 11+ (`Index 1 out of bounds for length 1`, helpful
`NullPointerException` messages) where the Warp 10 3.5 oracle runs on JDK 8.
