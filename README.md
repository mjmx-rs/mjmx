<p align="center">
  <img src="assets/mjmx_banner.png" alt="mjmx — the modern MJML toolkit, reimagined in Rust">
</p>

# mjmx

Rust port of [MJML](https://github.com/mjmlio/mjml), built as an oxc-style compiler toolchain
(parse → semantic → codegen → lint), rendering MJML to responsive HTML email.

> ⚠️ **Work in progress.** mjmx is under active development. The rendering pipeline is
> complete and parity-tested, but APIs may still change. Not yet recommended for
> production use.

See [CLAUDE.md](CLAUDE.md) for architecture and roadmap.

## Install

**Rust** — mjmx is distributed from this repository rather than crates.io for now, so
depend on it by git:

```toml
[dependencies]
mjmx = { git = "https://github.com/mjmx-rs/mjmx" }
```

Pin a commit for reproducible builds (`rev = "…"`), since there are no released versions
to resolve against. Note that a crate published *to* crates.io cannot depend on a git
dependency — if you need that, open an issue and we'll prioritise the registry release.

The CLI installs from the same source:

```sh
cargo install --git https://github.com/mjmx-rs/mjmx mjmx-cli
```

**JavaScript** — `mjmx` (WebAssembly; runs in Node, bundlers and browsers):

```sh
npm install mjmx
```

```js
import { render } from 'mjmx';

const html = render('<mjml><mj-body><mj-section><mj-column><mj-text>Hi</mj-text></mj-column></mj-section></mj-body></mjml>');
```

`mjmx-node` (native, faster) is not published yet: it needs a CI cross-build for all
five platform triples, so publishing it today would break installs everywhere except
macOS arm64.

**Python** — planned (pyo3/maturin), not started.

## mjml compatibility

mjmx reproduces **mjml 5.4.0 by default** and **mjml 4.18.0 on request** — a renderer that
silently changed its HTML on upgrade would be unusable in a pipeline that diffs output.
Select with `RenderOptions.compat` (`MjmlCompat::V5` default, `V4` opt-in), the CLI's
`--mjml-version 4|5`, or `mjmlVersion: 4 | 5` in the Node/wasm bindings. V4 output is frozen
and gated by the golden fixtures; every V5 behaviour is verified byte-for-byte against real
`mjml@5.4.0` output.

## Performance

Warm in-process render, MJML fixtures, 200 iterations, release build (LTO). Times
exclude process startup — the fair library-level comparison. The table below is
**generated live** by `make bench` (`benches/perf`); run it to reproduce these numbers on
your own machine.

<!-- BENCH:START -->
| Engine | Version | Language | µs / render | Renders / sec | Relative |
| --- | --- | --- | --- | --- | --- |
| **mjmx** | 5.4.0 / 4.18.0 | Rust | **14.7** | **68,176** | 1.000× |
| [mrml](https://github.com/jdrouet/mrml) | 6.0.1 | Rust | 20.6 | 48,428 | 0.710× |
| [mjml](https://github.com/mjmlio/mjml) | 4.18.0 | JS (Node) | 531.5 | 1,881 | 0.028× |
| [mjml](https://github.com/mjmlio/mjml) | 5.4.0 | JS (Node) | 561.5 | 1,781 | 0.026× |

mjmx renders **~38× faster than mjml 5.4.0** and **~1.4× faster than mrml**. Rust engines
were timed on the 72 fixtures both render (mrml fails 4); both mjml lines render all 76.
mjml 5.x is measured through its `Promise`-returning API — 5.0 made `mjml2html` async.
Single machine, single run — directional, not paper-grade.
<!-- BENCH:END -->

> Not affiliated with [jdrouet/mrml](https://github.com/jdrouet/mrml), a separate Rust MJML port.
