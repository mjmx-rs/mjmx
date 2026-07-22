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

| Engine | Language | µs / render | Renders / sec | Relative |
| --- | --- | --- | --- | --- |
| **mjmx** | Rust | **13.7** | **73,036** | 1.0× |
| [mrml](https://github.com/jdrouet/mrml) | Rust | 20.6 | 48,620 | 0.67× |
| [mjml](https://github.com/mjmlio/mjml) | JS (Node 24) | 550.1 | 1,818 | 0.025× |

mjmx renders **~40× faster than mjml** and **~1.5× faster than mrml**. Rust engines were
timed on the 72 fixtures both render (mrml fails 4); mjml and mjmx render all 76.
Single machine, single run — directional, not paper-grade.

> Not affiliated with [jdrouet/mrml](https://github.com/jdrouet/mrml), a separate Rust MJML port.
