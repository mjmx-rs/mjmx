<p align="center">
  <img src="assets/mjmx_logo.png" alt="mjmx" width="200">
</p>

# mjmx

Rust port of [MJML](https://github.com/mjmlio/mjml), built as an oxc-style compiler toolchain
(parse → semantic → codegen → lint), rendering MJML to responsive HTML email.

See [CLAUDE.md](CLAUDE.md) for architecture and roadmap.

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
| **mjmx** | 5.4.0 / 4.18.0 | Rust | **19.2** | **51,952** | 1.000× |
| [mrml](https://github.com/jdrouet/mrml) | 6.0.1 | Rust | 23.5 | 42,576 | 0.820× |
| [mjml](https://github.com/mjmlio/mjml) | 4.18.0 | JS (Node) | 529.5 | 1,888 | 0.036× |
| [mjml](https://github.com/mjmlio/mjml) | 5.4.0 | JS (Node) | 534.4 | 1,871 | 0.036× |

mjmx renders **~28× faster than mjml 5.4.0** and **~1.2× faster than mrml**. Rust engines
were timed on the 72 fixtures both render (mrml fails 4); both mjml lines render all 76.
mjml 5.x is measured through its `Promise`-returning API — 5.0 made `mjml2html` async.
Single machine, single run — directional, not paper-grade.
<!-- BENCH:END -->

> Not affiliated with [jdrouet/mrml](https://github.com/jdrouet/mrml), a separate Rust MJML port.
