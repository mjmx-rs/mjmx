<p align="center">
  <img src="assets/mjmx_logo.png" alt="mjmx" width="200">
</p>

# mjmx

Rust port of [MJML](https://github.com/mjmlio/mjml), built as an oxc-style compiler toolchain
(parse → semantic → codegen → lint), rendering MJML to responsive HTML email.

See [CLAUDE.md](CLAUDE.md) for architecture and roadmap.

## Performance

Warm in-process render, 76 MJML fixtures, 200 iterations, release build (LTO). Times
exclude process startup — the fair library-level comparison.

| Engine | Version | Language | µs / render | Renders / sec | Relative |
| --- | --- | --- | --- | --- | --- |
| **mjmx** | — | Rust | **13.6** | **73,738** | 1.0× |
| [mrml](https://github.com/jdrouet/mrml) | 6.0.1 | Rust | 20.3 | 49,174 | 0.55× |
| [mjml](https://github.com/mjmlio/mjml) | 4.18.0 | JS (Node 24) | 547.2 | 1,827 | 0.025× |
| [mjml](https://github.com/mjmlio/mjml) | 5.4.0 | JS (Node 24) | 573.0 | 1,745 | 0.024× |

mjmx renders **~42× faster than mjml 5.4.0** and **~1.5× faster than mrml**. Rust engines
were timed on the 72 fixtures both render (mrml fails 4); both mjml lines render all 76.
mjml 5.x is measured through its `Promise`-returning API — 5.0 made `mjml2html` async.
Single machine, single run — directional, not paper-grade.

> Not affiliated with [jdrouet/mrml](https://github.com/jdrouet/mrml), a separate Rust MJML port.
