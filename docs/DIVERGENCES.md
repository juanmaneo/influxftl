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
- **`SNAPSHOT` of a series is a readable reconstruction** (`NEWGTS 'name' RENAME { … }
  RELABEL { … } SETATTRIBUTES tick lat lon elev value ADDVALUE …`) until `WRAP` /
  `UNWRAP` are ported; Warp 10 emits `'<opb64 wrapper>' UNWRAP`. Both evaluate back to an
  equal series; the bucketization parameters are not carried by the reconstruction.
- **A Geo Time Series or encoder used as a value** (`gts 1 NaN NaN NaN gts2 ADDVALUE`) fails
  with `ADDVALUE encountered an error while serializing the Geo Time Series.`; Warp 10 would
  store the Thrift-serialized wrapper as a byte array (`STRING` series), which needs `WRAP`.
- **Counters** (`COUNTER`) are accepted as values by `ADDVALUE` / `SETVALUE` and ignored,
  fixing the type of an untyped series to `BOOLEAN`: this is what `GTSHelper.setValue` does
  with a Java `AtomicLong`. As elsewhere (§15) they are not numbers for the tick, latitude,
  longitude and elevation arguments.
- **GTS encoders are not modelled yet**: the functions that accept "a Geo Time Series or
  encoder" only accept series, with the messages Warp 10 prints in that case.
- **`[ value ] ADDVALUE`** (a one-element datapoint list) adds the value at tick `0` as
  `MACROMAPPER.listToObjects` intends; Warp 10 dies with a `ClassCastException` on the
  `Integer` tick that method builds.
- **`MERGE` with a boolean** (the sorted merge) produces a GTS Encoder in Warp 10; it fails
  with `MERGE encountered an error while merging. (GTS Encoders are not supported.)` until
  encoders are modelled. The plain `MERGE` is a port of `mergeViaEncoders` (every datapoint
  appended to a new series carrying the metadata of the first one).
- **`CHUNK` with a chunk width of `0`** fails with `/ by zero` on every path; Warp 10 only
  divides by the width when it has to align the last chunk or skip empty chunks, and loops
  over up to 2³¹ empty chunks otherwise. A non bucketized series **without datapoint**, with
  `keepempty` false and an explicit last chunk, yields an empty list; Warp 10 overflows on
  `Long.MIN_VALUE` and loops almost forever.
- **`CROP` of a bucketized series without datapoint on a bucket boundary** fails with the
  JDK message of the `NullPointerException` Warp 10 dies with (`Cannot load from long array
  because "cropped.ticks" is null`).
- **Operators on series** (`==`, `!=`, `+`, `*`… with Geo Time Series operands, the `GTSHelper`
  *ops* family) are not ported yet: `==` / `!=` compare series by identity and the arithmetic
  operators reject them, where Warp 10 computes a new series tick by tick.
- **`CLONE` and the functions returning a copy** (`TIMESHIFT`, `TIMESCALE`, `CROP`…) reset the
  `renamed` flag like `GeoTimeSerie.clone`; the flag is only read by `UPDATE`.
- **`VALUESORT` / `RVALUESORT` / `VALUEDEDUP` on a `BOOLEAN` series** whose `false` or
  `true` datapoints are not in tick order: Warp 10 sorts each half with an off-by-one upper
  bound and reads (and swaps in) the array slot past the last datapoint, producing a bogus
  tick (`0` or a stale value). The port sorts within bounds and otherwise follows Warp 10,
  including the stale `reversed` flag it consults to decide the reading direction of each
  half (`[ 3 1 2 ] true` values come out as ticks `3 2 1`).
- **`DEDUP` with a macro or mapper** on top of the stack (`GTSHelper.map`) fails with `DEDUP
  with a macro or mapper requires the MAP framework, which is not supported yet.` instead
  of applying the mapper.
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
- **`MUSIGMA`, `NSUMSUMSQ` and `LR`** compute with `BigDecimal` in Warp 10 and are not ported
  yet (they will come with the `BD*` functions). `STANDARDIZE`, `ZSCORE`, `SKEWNESS` and
  `KURTOSIS` use the plain double arithmetic of their Java counterparts.

## 19. `BUCKETIZE` and the aggregators

`BUCKETIZE` is a port of `GTSHelper.bucketize` (parameter derivation included: a `0` last
bucket aligns the last tick on the span, a `0` span derives from the count and a `-1` span
makes the first tick start the first bucket) with the aggregator and macro bucketizers. The
aggregators (`bucketizer.*`, `mapper.*`, `reducer.*`) are the shared `WarpScriptAggregatorFunction`
classes of Warp 10 (`Sum`, `Mean`, `Min`, `Max`, `First`, `Last`, `Count`, `And`, `Or` so far),
each registered name keeping its `ignoreNulls` flag. The differences:

- **A macro bucketizer receives a copy of the result's metadata** with each bucket's
  datapoints; Warp 10 shares the metadata object, so a macro renaming or relabelling the
  sub-series would alter the bucketized series.
- **A `null` result** (alone or as the last element of a list) is left on the stack, like
  Warp 10 does — scripts relying on that garbage behave identically.
- **`MAP`, `REDUCE`, `APPLY`, `FILTER`** and the other aggregators (`median`, `join`,
  `percentile`, `delta`, `rate`…) are not ported yet; `bucketizer.*` names that are not
  listed in `docs/FUNCTIONS.md` are unknown functions.
