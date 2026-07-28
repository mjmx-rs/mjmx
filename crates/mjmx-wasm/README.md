# mjmx

**MJML → HTML email compiler for JavaScript.** Written in Rust, compiled to WebAssembly.

A faster, drop-in-shaped alternative to `mjml` / `mjml-browser`. Works in Node, browsers,
Deno and Bun — no native addon, no postinstall, no build step.

```sh
npm install mjmx
```

```js
import { render } from 'mjmx';

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

| | `mjmx` | `mjml` / `mjml-browser` |
| --- | --- | --- |
| Speed | **~7× faster** | baseline |
| API | `render()` → `string` | `mjml2html()` → `{ html, errors }` |
| Async | synchronous | **`async` since mjml 5** |
| Bundle impact | ~620 KB (one flavour) | ~1.1 MB (`mjml-browser`) |
| mjml versions | 5.4.0 **and** 4.18.0, selectable | whichever you installed |
| Errors | throws, or `lint()` for a list | `errors` array |

Speed measured in Node over 40 MJML fixtures, 800 renders each, against `mjml@4.18.0`:
**~55 µs vs ~377 µs per render**, stable across three runs. Single machine, warm process
— directional, not paper-grade. Reproduce it yourself; don't take the number on faith.

Need more? The native Node addon (`mjmx-node`) is ~19 µs — about 3× this package and
~20× mjml — at the cost of a per-platform binary. This package trades that for running
anywhere.

The package **unpacks to ~1.9 MB** because it ships three wasm flavours (Node, bundler,
browser). Your bundle only ever includes one of them.

## Migrating from `mjml` / `mjml-browser`

Two things change at the call site.

**1. The return shape.** `mjml2html` hands back an object; `render` hands back the string
and throws on failure. A mechanical find-and-replace will silently give you `undefined`:

```js
// before
const { html, errors } = mjml2html(source);

// after
let html;
try {
  html = render(source);
} catch (err) {
  console.error(err.message);
  // mjmx: error: unclosed element `<mj-section>` (help: …) (+2 more diagnostics)
}
```

Want the diagnostics as data instead of an exception? `lint(source)` returns an array and
never throws:

```js
const problems = lint(source);          // [{ severity, message, labels, help }]
const fatal = problems.filter((d) => d.severity === 'error');
```

**2. Pin the mjml version.** mjmx defaults to reproducing mjml **5.4.0**. If you are
coming from mjml 4.x and want your existing output back, ask for it:

```js
render(source, { mjmlVersion: 4 });   // reproduce mjml 4.18.0
render(source);                       // reproduce mjml 5.4.0 (default)
```

Both are kept because a renderer that silently changes its HTML on upgrade is unusable in
a pipeline that diffs output. V4 output is frozen and gated by golden fixtures generated
from real mjml; every V5 behaviour was verified byte-for-byte against real `mjml@5.4.0`.

**Diff before you trust it.** V4 mode reproduces 4.18.0 specifically — if you are on a
different 4.x, render a few real templates both ways and compare.

If you were already `await`ing mjml 5's `mjml2html`, `await render(...)` still works on a
non-promise, so that call site can stay as-is until you clean it up.

## Usage

### Node (CommonJS)

```js
const { render, lint, version } = require('mjmx');
console.log(version());
```

### Bundlers and browsers (ESM)

```js
import { render } from 'mjmx';
```

Three builds ship behind a conditional `exports` map, so one import resolves correctly
everywhere:

| condition | flavour | for |
| --- | --- | --- |
| `node` | `pkg/node/` (CommonJS) | Node.js |
| `browser` | `pkg/web/` (ESM, self-fetching) | browsers / Deno, no bundler |
| `import` / default | `pkg/bundler/` (ESM) | webpack / vite / rollup |

### Embedding the wasm yourself

For bundlers that cannot fetch a sibling `.wasm`, or sandboxed hosts with no network
access (a Figma plugin, for instance), import the `web` flavour directly and boot it from
bytes you inline at build time:

```js
import { initSync, render } from 'mjmx/pkg/web/mjmx_wasm.js';
import wasmBytes from 'mjmx/pkg/web/mjmx_wasm_bg.wasm';  // esbuild: loader { '.wasm': 'binary' }

initSync({ module: wasmBytes });   // once, before the first render
```

## API

```ts
render(source: string, options?: RenderOptions): string   // throws on diagnostics
lint(source: string): Diagnostic[]                        // never throws
format(source: string, options?: FormatOptions): string   // pretty-print MJML source
version(): string
```

`RenderOptions` mirrors mjml's own `mjml2html` option names and defaults. Every field is
documented in the bundled `.d.ts`, so your editor will explain them inline.

| option | default | notes |
| --- | --- | --- |
| `mjmlVersion` | `5` | `4` or `5` — which mjml release to reproduce |
| `breakpoint` | `480` | mjmx extension; upstream only allows `<mj-breakpoint>` |
| `beautify` / `minify` | `false` | post-processing |
| `keepComments` | `true` | matches upstream's parser default |
| `validationLevel` | `'soft'` | `'soft'` \| `'strict'` \| `'skip'` |
| `fonts` | built-ins | **replaces** the built-in table; key order decides `<link>` order |
| `minifyOptions` | `{}` | only read when `minify` |
| `ignoreIncludes` | follows `mjmlVersion` | mjml 5 disables includes by default |
| `printerSupport` | `false` | adds an `@media only print` block |
| `juiceOptions` | juice's | how `<mj-style inline="inline">` is inlined |
| `juicePreserveTags` | — | extra templating delimiters the inliner must not enter |
| `sanitizeStyles` | `false` | protects `{{template}}` syntax in CSS from the minifier |
| `templateSyntax` | `{{ }}`, `[[ ]]` | delimiter pairs for `sanitizeStyles` |
| `allowMixedSyntax` | `false` | allow mixed block and value template syntax |
| `components` | — | custom component renderers, see below |
| `filePath` | — | **unsupported**, throws — see "No filesystem" |

An unsupported `juiceOptions` key is **refused with the reason**, never silently ignored.
EJS (`<% %>`) and Handlebars (`{{ }}`) are always masked before inlining, so templating
syntax is never mistaken for markup.

### Templating in your MJML

Template placeholders survive rendering, including inside `<mj-style>` when you ask for
it:

```js
render(source, { minify: true, sanitizeStyles: true });
```

Without `sanitizeStyles`, a `{{brand}}` inside CSS would be fed to the CSS minifier, which
cannot parse it. mjmx raises an actionable error instead of silently corrupting your
stylesheet.

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
also contribute to `<head>`. An unregistered unknown tag stays a fatal error, as in mjml.

You return the cell **content**; mjmx wraps it in the standard leaf `<td>` — reading
`align`, `css-class`, `container-background-color` and `padding` from the component's own
attributes — so the table markup stays valid. Components are body leaves; container
components that lay out their own children are not available from JavaScript yet.

## No filesystem

WebAssembly has no ambient filesystem, so there is no `renderFile`, and
`<mj-include path="…">` cannot be resolved — there is no base directory and no loader.
Passing `filePath` **throws** rather than quietly returning HTML with the includes
unresolved. Either inline partials into the source string before calling `render`, or use
the native Node addon (`mjmx-node`) where a filesystem exists.

## TypeScript

Types ship with the package — no `@types/…` needed.

```ts
import { render, type RenderOptions } from 'mjmx';

const opts: RenderOptions = { mjmlVersion: 4, beautify: true };
const html: string = render(source, opts);
```

## Links

- [Repository](https://github.com/mjmx-rs/mjmx) — architecture, and the rest of the
  toolchain: CLI, linter, formatter, minifier, sourcemap, language server, dev-server
- [MJML language reference](https://documentation.mjml.io) — mjmx implements the same
  components and attributes

## License

MIT
