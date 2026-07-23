# mjmx perf bench

Generates the performance table in the top-level [`README.md`](../../README.md) **live** —
every number is measured on the machine that runs it, nothing is hard-coded.

## Run

From the repo root:

```sh
make bench          # print the table to stdout
make bench-write    # splice it into README.md (between the BENCH markers)
```

Or directly in this directory:

```sh
npm install
cargo run --release              # print
cargo run --release -- --write   # update README.md
cargo run --release -- --iters 500   # more iterations (default 200)
```

## What it measures

- **mjmx** (this repo, `MjmlCompat::V5` default) and **mrml** (crates.io `6.0.1`) —
  timed in-process, warm, over `fixtures/*.mjml`.
- **mjml 4.18.0** and **mjml 5.4.0** — timed by [`bench.mjs`](bench.mjs), which pins both
  releases as npm aliases (`mjml418`, `mjml540`) and renders through mjml 5's
  `Promise`-returning API.

Times exclude process startup. The two Rust engines are timed on the fixtures **both**
render (mrml fails a few), so they compare on one set; the mjml lines render all fixtures.
Single machine, single run — directional, not paper-grade.

## Why it is a separate project

It has its own `[workspace]` and a heavy `mrml` dependency, and the repo root's
`Cargo.toml` lists it under `exclude`, so it never builds under `cargo build --workspace`
or `make ci`. If `node` or the npm packages are unavailable, the Rust rows still print and
the mjml rows are skipped with a warning.
