# Vendored rstar 0.12.2

This directory is a copy of `rstar` 0.12.2 from crates.io, used through `[patch.crates-io]` in
the root `Cargo.toml`. The crate is dual licensed `MIT OR Apache-2.0`; the copy is used under
Apache-2.0 (`LICENSE-APACHE`, the text of the repository's `LICENSE`).

## The one change

`Cargo.toml`: `heapless` `0.8` becomes `0.9`. Nothing else in the crate is modified.

Why: `geo` 0.33.1, the newest release, requires `rstar` `^0.12`, and `rstar` 0.12.2 requires
`heapless` `^0.8`. `heapless` 0.8.0 has RUSTSEC-2026-0574 to 0582, which are patched in 0.9.4
(see the local advisory database). `rstar` uses `heapless` only for `heapless::binary_heap`, in
`src/algorithm/nearest_neighbor.rs`.

## Removing the patch

When a published `geo` depends on an `rstar` release that uses a patched `heapless`: delete
`vendor/rstar`, delete the `[patch.crates-io]` entry in the root `Cargo.toml`, then run
`cargo update -p geo -p rstar`.
