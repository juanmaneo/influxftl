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
process: a script, or a `serve`). The InfluxDB 3 datastore (SQL for reads, line protocol for
writes, `README.md` §2) is the next step; a script without a datastore fails with
`<FUNCTION> needs a datastore, and none is connected to this runtime.`

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
