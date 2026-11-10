# The datastore functions

Warp 10 reads and writes through its own storage (Directory and Store clients, a token for each
request). `influxftl` has none of that: InfluxDB 3 is the store and authenticates the requests.
The datastore functions are therefore adapted rather than ported to the letter. This page says
what a script can rely on.

## The host contract

`warpscript::datastore::Datastore` is what a host plugs into a `Stack` (`Stack::set_datastore`):

| method            | used by                       | what it does                                                       |
|-------------------|-------------------------------|--------------------------------------------------------------------|
| `find`            | `FIND`, `FINDSETS`, `FINDSTATS`, `DELETE` | the metadata of the series a selector selects, by class then labels |
| `fetch`           | `FETCH`, `FETCHLONG`…         | the datapoints of a `FetchRequest`, most recent first               |
| `update`          | `UPDATE`                      | write datapoints (those at the same ticks are replaced)             |
| `update_metadata` | `META`, `METADIFF`            | replace or change attributes; creates unknown series                |
| `delete`          | `DELETE`                      | delete a range of ticks, or everything (series included)            |

`MemoryDatastore` implements it in memory: it is the reference of the semantics below, and what
`INFLUXFTL_DATASTORE=memory` connects to the `influxftl` binary (data lasts as long as the
process: a script, or a `serve`). `InfluxDatastore` (`crates/influxftl/src/influx`) implements it
on InfluxDB 3 (`INFLUXFTL_DATASTORE=influxdb3`, next section). A script without a datastore
fails with `<FUNCTION> needs a datastore, and none is connected to this runtime.`

## InfluxDB 3

### Connecting

| variable                  | meaning                                                              |
|---------------------------|----------------------------------------------------------------------|
| `INFLUXFTL_DATASTORE`     | `influxdb3` (or `memory`)                                            |
| `INFLUXDB3_DATABASE_NAME` | the database; required. InfluxDB 3 creates it on the first write: until then a read fails with its `database not found` |
| `INFLUXDB3_HOST_URL`      | `http://host[:port][/prefix]`, `http://127.0.0.1:8181` by default     |
| `INFLUXDB3_AUTH_TOKEN`    | sent as `Authorization: Bearer …`; none by default                    |

The client is a small HTTP/1.1 client on `std` (one request per connection, 120 s without an
answer is a failure). It does not speak TLS: `https://` is refused, not downgraded. Put a TLS
terminating proxy in front of InfluxDB 3 when the network is not one you trust. Nothing is
connected before the first function that needs it, so a wrong URL is reported by that function
(`find failed on the datastore: cannot connect to …`). InfluxDB 3 Core allows five databases.

### Layout

A class is a table, its labels are tags, and each datapoint is a row with these columns:
`time` (the tick in nanoseconds), `value_type` (`l`, `d`, `b` or `s`), the value in `value_l`,
`value_d`, `value_b` or `value_s`, `hhcode` and `elevation` (integers; `hhcode` is
`0x0145014501450145` and `elevation` is `i64::MIN` when the datapoint has none). Every column but
the value of the other types is written each time, so that a datapoint written again replaces the
one at its tick whatever its type was. A class has the columns its datapoints have used so far.
Several types in a series are several types in its table, and a fetch converts them to the type
of the first datapoint it reads, as `GTS` does.

The attributes of the series are rows of the table `_influxftl_series`: tags `class` and `labels`
(a JSON object, sorted by name), fields `attributes` (a JSON object) and `deleted`. The latest row
of a series is its current attributes; each row of a series is written after the one it replaces.
`META` can therefore create a series that only has attributes (`FIND` shows it, `FETCH` finds no
datapoint, `keepempty` gives it). `METADIFF` reads the current attributes and writes the changed
ones.

Refused, with the reason in the error: a label named `time`, `value_type`, `value_l`, `value_d`,
`value_b`, `value_s`, `hhcode` or `elevation` (they are columns); a label with an empty value, a
class or a name with a line break, a class that starts with `#` or is `_influxftl_series`
(line protocol cannot carry them); a `DOUBLE` that is not finite; a tick whose nanoseconds do not
fit 64 bits (InfluxDB 3 holds times up to the year 2262).

### Reads

`FIND` lists the tables with the `value_type` column and the series of each (`SELECT DISTINCT`
over its tags); the selectors are evaluated by `influxftl` on those series and their attributes,
because a label selector also selects on an attribute and the regular expressions are Java's. A
selector with a class that is not a regular expression reads only that table. The tables that
are not written by `influxftl` (no `value_type`) are ignored, and so are those InfluxDB 3 deleted
(`name-20261110T132233`), which it keeps listing.

`FETCH` reads each series with `ORDER BY time DESC` over `[start, end]` (a tick holds the 1000
nanoseconds from its microsecond), a `LIMIT` when only `count` and `skip` thin it, and the
boundaries with one more query each, then applies `skip`, `step`, `timestep` and `count` like the
datastore in memory. `type` is a condition on `value_type`.

The series of a table are found with `SELECT DISTINCT` over its tag columns (InfluxDB 3 Core
has no series index to ask), a scan of those columns each time a function needs them, and a
`FETCH` runs one query per series (plus one per boundary). Both are fine for the tables a script
selects by class and labels, and worth knowing for a table with very many rows or series.

### Writes and deletions

`UPDATE` is line protocol (batches of 10 000 lines, nanosecond precision). `DELETE` is limited by
what InfluxDB 3 Core does: it has no deletion of datapoints, only of tables, and it keeps a
deleted table under another name. `DELETE` with `NULL NULL` deletes the table of each class
selected, provided every series of the table is selected, and marks the series with attributes
as deleted. A range of ticks, or a selector that leaves a series of the table, fails with
`InfluxDB 3 Core only deletes whole tables`. The table can be written again afterwards.

## Tokens

Every function that takes a token still takes one (it must be a `STRING`, a script written for
Warp 10 keeps working), and ignores it. There is no read/write scoping by token, no
`.producer` / `.owner` / `.app` labels, no `TOKENGEN` here.

## `FETCH`

`[ token class labels end timespan ]`, `[ token class labels 'start' 'end' ]` (ISO 8601 or
numeric strings) or a map: `token`, one of `selector` / `selectors` / `class` and `labels`, and
the time range (`start`, `end`, `timespan`, `count`: `FETCH.computeTimeRange`, ported as is: `now`,
ISO 8601 dates and `P…` durations, a negative `timespan` as a `count`, swapped bounds put right).
Supported: `type` (and the typed `FETCHLONG`, `FETCHDOUBLE`, `FETCHSTRING`, `FETCHBOOLEAN`),
`boundary`, `boundary.pre`, `boundary.post`, `skip`, `step`, `timestep`, `gskip`, `gcount`,
`keepempty`, `merge` and `priority` (read, no effect). The result is a list of series, the most
recent datapoint of each first.

Not supported, and refused with `<FUNCTION> does not support the '<parameter>' parameter.`:
`metaset`, `gts`, `extra`, `typeattr`, `selpairs`, `active.after`, `quiet.after`, `sample`,
`wtimestamp`, `ttl`, `showuuid`, `encoders` (when `true`).

## `FIND`, `FINDSETS`, `FINDSTATS`

`[ token class labels ]` (a fourth element, the UUID flag, is read and ignored), the older
`token class labels`, or a map with `selector` or `class` and `labels` (`gskip` and `gcount`
supported; `selectors` with several selectors and `active.after` / `quiet.after` are refused).
`FIND` gives series without datapoints; `FINDSETS` the classes, the values of each label and of
each attribute; `FINDSTATS` the counts (exact, where Warp 10 estimates) under `gts.estimate`,
`classes.estimate`, `labelnames.estimate`, `labelvalues.estimate`, `per.label.value.estimate`
and `error.rate` (`0.0`). `METASET` needs Warp 10's keys and is not provided.

## `UPDATE`, `META`, `METADIFF`, `DELETE`

`UPDATE` (`gts-or-encoders token UPDATE`) requires a series with a name that was renamed (like
Warp 10); an encoder holding several types is written as one series per type. `META` replaces
the attributes of the series (created when unknown), `METADIFF` changes them (an empty value
removes one). `DELETE` (`token selector start end expected DELETE`) deletes the datapoints of
the range (everything, the series too, for `NULL` `NULL`), refuses to delete more series than
`expected`, and pushes the number of series; `NULL` for `expected` is a dry run that pushes the
series a deletion would touch.
