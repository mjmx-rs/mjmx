# @mjmx-rs/node

**MJML → HTML email compiler for Node.js.** Written in Rust, shipped as a native addon.

The fastest way to render MJML from JavaScript: **~20× faster than `mjml`**, with the
same option names, plus structured diagnostics and real filesystem support for
`<mj-include>`.

```sh
npm install @mjmx-rs/node
```

```js
const { render } = require('@mjmx-rs/node');

const html = render(`
  <mjml>
    <mj-body>
      <mj-section background-color="#f4f4f4">
        <mj-column>
          <mj-text font-size="20px">Hello</mj-text>
          <mj-button href="https://example.com">Click me</mj-button>
        </mj-column>
      </mj-section>
    </mj-body>
  </mjml>
`);
```

## Why not just use `mjml`?

| | `@mjmx-rs/node` | `mjml` |
| --- | --- | --- |
| Speed | **~20× faster** | baseline |
| API | `render()` → `string` | `mjml2html()` → `{ html, errors }` |
| Async | synchronous | **`async` since mjml 5** |
| Errors | throws `MjmxError` with `.diagnostics` | `errors` array |
| mjml versions | 5.4.0 **and** 4.18.0, selectable | whichever you installed |
| Install | prebuilt binary per platform | pure JS |

Measured in Node over 40 MJML fixtures, 800 renders each, against `mjml@4.18.0`:
**~19 µs vs ~377 µs per render**, stable across three runs. Single machine, warm process
— directional, not paper-grade. Reproduce it yourself rather than taking it on faith.

**Prefer portability?** [`@mjmx-rs/mjmx`](https://www.npmjs.com/package/@mjmx-rs/mjmx) is the same
compiler as WebAssembly — ~7× mjml instead of ~20×, but one artifact that runs in Node,
browsers, Deno and Bun with no platform binary. Use this package when you're on Node and
want the speed; use that one when you need to run anywhere.

## Install

The native binary ships as a per-platform **optional dependency**, so npm downloads only
the one matching your host:

| platform | package |
| --- | --- |
| Linux x64 (glibc) | `@mjmx-rs/node-linux-x64-gnu` |
| Linux arm64 (glibc) | `@mjmx-rs/node-linux-arm64-gnu` |
| macOS Intel | `@mjmx-rs/node-darwin-x64` |
| macOS Apple Silicon | `@mjmx-rs/node-darwin-arm64` |
| Windows x64 | `@mjmx-rs/node-win32-x64-msvc` |

No postinstall script, no compiler needed on the install machine. Both module systems
resolve:

```js
const mjmx = require('@mjmx-rs/node');   // CommonJS
import mjmx from '@mjmx-rs/node';        // ESM — named exports work too
import { render, renderFile } from '@mjmx-rs/node';
```

If no matching binary is found, the loader raises an error naming the triple it looked
for, rather than failing somewhere deeper.

## Migrating from `mjml`

Two things change at the call site.

**1. The return shape.** `mjml2html` returns an object; `render` returns the string and
throws on failure. A mechanical find-and-replace silently yields `undefined`:

```js
// before
const { html, errors } = mjml2html(source);

// after
let html;
try {
  html = render(source);
} catch (err) {
  console.error(err.message);
  for (const d of err.diagnostics ?? []) {
    console.error(d.severity, d.message, d.labels);
  }
}
```

The thrown error is an `MjmxError` — a normal `Error` whose `.diagnostics` array carries
the structured list, each with `severity`, `message`, `labels` (byte offsets) and
optional `help`. To inspect problems without an exception, call `lint(source)`, which
never throws.

**2. Pin the mjml version.** mjmx reproduces mjml **5.4.0** by default:

```js
render(source, { mjmlVersion: 4 });   // reproduce mjml 4.18.0
render(source);                       // reproduce mjml 5.4.0 (default)
```

Both are kept because a renderer that silently changes its HTML on upgrade is unusable in
a pipeline that diffs output. V4 output is frozen and gated by golden fixtures generated
from real mjml; every V5 behaviour was verified byte-for-byte against real `mjml@5.4.0`.

**Diff before trusting it.** V4 mode reproduces 4.18.0 specifically — if you're on a
different 4.x, render real templates both ways and compare.

## Rendering a file

Unlike the wasm build, this package has a filesystem, so `<mj-include>` resolves:

```js
const { renderFile } = require('@mjmx-rs/node');

const html = renderFile('./emails/welcome.mjml');
```

Includes resolve relative to the file's own directory. For a string source, pass
`filePath` to give includes a base to resolve against:

```js
render(source, { filePath: './emails/welcome.mjml' });
```

mjml 5 **disables includes by default** and adds a path sandbox. Under the default
`mjmlVersion: 5`, `<mj-include>` is ignored unless you opt in:

```js
render(source, { filePath: 'emails/welcome.mjml', ignoreIncludes: false });
```

The sandbox denies any include that escapes the base directory — via `../`, an absolute
or UNC path, a percent-encoded traversal, or a symlink pointing outside — replacing it
with a non-blocking `<!-- mj-include denied -->` comment. Grant extra roots with
`includePath: ['./partials', './shared']`.

## API

```ts
render(source: string, options?: RenderOptions): string     // throws MjmxError
renderFile(path: string, options?: RenderOptions): string   // throws MjmxError
lint(source: string): Diagnostic[]                          // never throws
format(source: string, options?: FormatOptions): string     // pretty-print MJML source
version(): string
```

`RenderOptions` mirrors mjml's own `mjml2html` option names and defaults. Every field is
documented in the bundled `index.d.ts`, so your editor explains them inline.

| option | default | notes |
| --- | --- | --- |
| `mjmlVersion` | `5` | `4` or `5` — which mjml release to reproduce |
| `breakpoint` | `480` | mjmx extension; upstream only allows `<mj-breakpoint>` |
| `beautify` / `minify` | `false` | post-processing |
| `keepComments` | `true` | matches upstream's parser default |
| `validationLevel` | `'soft'` | `'soft'` \| `'strict'` \| `'skip'` |
| `filePath` | — | include base for a string source |
| `fonts` | built-ins | **replaces** the built-in table; key order decides `<link>` order |
| `minifyOptions` | `{}` | only read when `minify` |
| `ignoreIncludes` | follows `mjmlVersion` | mjml 5 disables includes by default |
| `includePath` | `[]` | extra roots the include sandbox will allow |
| `printerSupport` | `false` | adds an `@media only print` block |
| `juiceOptions` | juice's | how `<mj-style inline="inline">` is inlined |
| `juicePreserveTags` | — | extra templating delimiters the inliner must not enter |
| `sanitizeStyles` | `false` | protects `{{template}}` syntax in CSS from the minifier |
| `templateSyntax` | `{{ }}`, `[[ ]]` | delimiter pairs for `sanitizeStyles` |
| `allowMixedSyntax` | `false` | allow mixed block and value template syntax |
| `components` | — | custom component renderers, see below |

An unsupported `juiceOptions` key is **refused with the reason**, never silently ignored.
EJS (`<% %>`) and Handlebars (`{{ }}`) are always masked before inlining, so templating
syntax is never mistaken for markup.

### Templating in your MJML

Template placeholders survive rendering, including inside `<mj-style>`:

```js
render(source, { minify: true, sanitizeStyles: true });
```

Without `sanitizeStyles`, a `{{brand}}` inside CSS reaches the CSS minifier, which cannot
parse it. mjmx raises an actionable error rather than silently corrupting the stylesheet.

### Custom components

Register a renderer for a tag MJML doesn't know:

```js
const html = render(source, {
  components: {
    'mj-badge': (cx) => `<span>${cx.attr('label') ?? ''}</span>`,
    'mj-card': (cx) => ({
      html: `<div class="card">${cx.rawInner}</div>`,
      styles: ['.card{border:1px solid #ccc}'],
      fonts: [['Inter', 'https://fonts.example/inter.css']],
    }),
  },
});
```

The callback receives a read-only `cx` (`attr(name)`, `attrs`, `containerWidth`,
`breakpoint`, `rawInner`) and returns a string of HTML, or `{ html, styles?, fonts? }` to
also contribute to `<head>`. It is called synchronously during the render. An
unregistered unknown tag stays a fatal error, as in mjml.

You return the cell **content**; mjmx wraps it in the standard leaf `<td>` — reading
`align`, `css-class`, `container-background-color` and `padding` from the component's own
attributes — so the table markup stays valid. Components are body leaves; container
components that lay out their own children are native-Rust only for now.

## TypeScript

Types ship with the package — no `@types/…` needed.

```ts
import { render, type RenderOptions, type MjmxError } from '@mjmx-rs/node';

const opts: RenderOptions = { mjmlVersion: 4, beautify: true };
const html: string = render(source, opts);
```

The `RenderOptions` interface is closed: an unknown key is a compile-time error rather
than being silently dropped.

## Links

- [Repository](https://github.com/mjmx-rs/mjmx) — architecture, and the rest of the
  toolchain: CLI, linter, formatter, minifier, sourcemap, language server, dev-server
- [`@mjmx-rs/mjmx`](https://www.npmjs.com/package/@mjmx-rs/mjmx) — the portable WebAssembly build
- [MJML language reference](https://documentation.mjml.io) — mjmx implements the same
  components and attributes

## License

MIT
