# mjmx

MJML-to-HTML compiler toolchain in Rust — parse and render [MJML](https://mjml.io) into
responsive HTML email.

This is the umbrella crate: the one-call front door over the pipeline
(`parse → semantic → codegen`), re-exporting the option and diagnostic types you need.
The individual stages ship as their own crates (`mjmx-parser`, `mjmx-codegen`,
`mjmx-linter`, …) for tools that want to reach into them.

```rust
let html = mjmx::render_default(
    "<mjml><mj-body><mj-section><mj-column><mj-text>Hi</mj-text></mj-column></mj-section></mj-body></mjml>",
)
.unwrap();
assert!(html.starts_with("<!doctype html>"));
```

## mjml compatibility

mjmx reproduces **mjml 5.4.0 by default** and **mjml 4.18.0 on request** — a renderer that
silently changed its HTML on upgrade would be unusable in a pipeline that diffs output.
Select with `RenderOptions.compat` (`MjmlCompat::V5` default, `V4` opt-in). V4 output is
frozen and gated by golden fixtures generated from real mjml; every V5 behaviour is
verified byte-for-byte against real `mjml@5.4.0` output.

## Features

- **Render** — full component set, `mj-attributes` / `mj-class` cascade, `<mj-include>`,
  CSS inlining for `<mj-style inline="inline">`, `<mj-html-attributes>`.
- **Options parity** with mjml's own (`keepComments`, `validationLevel`, `fonts`,
  `juiceOptions`, `minifyOptions`, `sanitizeStyles`, …), each mapped deliberately rather
  than forwarded blindly.
- **Diagnostics** — span-labeled errors and warnings, not opaque strings.
- **Custom components** — implement `CustomComponent`, register it, and an otherwise
  unknown tag renders through your code.
- `http-loader` (off by default) opts `<mj-include>` into network access.

## Status

Pre-1.0: the rendering pipeline is complete and parity-tested, but the API may still
change.

See the [repository](https://github.com/dreygur/mjmx) for architecture, the crate map, and
the rest of the toolchain — linter, formatter, minifier, sourcemap, language server and
dev-server.

## License

MIT
