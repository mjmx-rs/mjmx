# @mjmx/node

Node.js bindings for [mjmx](https://github.com/dreygur/mjmx) — an MJML-to-HTML
compiler written in Rust.

The npm package lives alongside the Rust crate in this directory: `package.json`,
`index.js` (CommonJS loader + thin wrapper), `index.mjs` (ESM re-export),
`index.d.ts` (hand-written types), and `test/smoke.mjs`.

## Install

```sh
npm i @mjmx/node
```

The native binary ships as a per-platform **optional dependency**
(`@mjmx/node-<triple>`, e.g. `@mjmx/node-linux-x64-gnu`); npm installs only the one
matching the host. Prebuilt triples: `linux-x64-gnu`, `linux-arm64-gnu`, `darwin-x64`,
`darwin-arm64`, `win32-x64-msvc`. The loader (`index.js`) `require`s the matching
package first, then falls back to a locally built binary — so a bare
`cargo build -p mjmx-napi` or `make napi` is enough for development.

Both module systems resolve:

```js
const mjmx = require('@mjmx/node'); // CommonJS
import mjmx from '@mjmx/node';      // ESM (named exports too: import { render } …)
```

## Build & test

```sh
make napi        # cargo build -p mjmx-napi --release + stage the .node
make napi-test   # the above, then node crates/mjmx-napi/test/smoke.mjs
```

## API

```js
const mjmx = require('@mjmx/node');

// Render options use mjml's own `mjml2html` names and defaults; `breakpoint` is the
// one mjmx extension (mjml hardcodes 480 and only <mj-breakpoint> changes it).
mjmx.render(source, {
  breakpoint: 480,              // mjmx extension
  beautify: false,
  minify: false,
  keepComments: true,
  validationLevel: 'soft',      // 'soft' | 'strict' | 'skip'
  filePath: './partials',       // base dir for <mj-include>, as in mjml
  fonts: { Inter: 'https://fonts.example/inter.css' }, // replaces the built-in table
  minifyOptions: {},            // only consulted when `minify` is true
  ignoreIncludes: false,        // true drops <mj-include> instead of resolving it
  printerSupport: false,        // true adds an `@media only print` block
  juiceOptions: {},             // how to inline <mj-style inline="inline">
  juicePreserveTags: {},        // extra templating delimiters the inliner skips
});                                       // -> HTML
mjmx.renderFile('email.mjml', options);   // resolves <mj-include> relative to the file
mjmx.lint(source);                        // -> Diagnostic[], never throws
mjmx.format(source, { indentWidth: 2 });  // -> canonical MJML
mjmx.version();                           // -> "0.1.0"
```

`minifyOptions` carries mjml's four html-minifier switches:

```js
mjmx.render(source, {
  minify: true,
  minifyOptions: {
    collapseWhitespace: true,     // default true (mjml's own)
    minifyCSS: false,             // default false; a conservative whitespace/comment
                                  //   pass, not html-minifier's clean-css optimizer
    caseSensitive: true,          // default true; false lower-cases attribute names
    removeEmptyAttributes: false, // default false — mjml passes true, see below
  },
});
```

`removeEmptyAttributes` is the one place mjmx's default differs from mjml's. mjml's own
output carries `<div … style="" role="article">`, so mjml's `true` *removes an
attribute* while minifying; mjmx defaults it off to keep `minify` a pure whitespace
transform. Pass `true` for byte parity with mjml.

## Custom components

Register a renderer for a tag through `options.components`. When rendering reaches a tag
the parser does not recognize, its callback renders it; an unregistered unknown tag stays
the usual fatal "not a recognized MJML component" error. This is mjmx's own extension
point (a small, versioned contract), not mjml's `.mjmlconfig` JS packages.

```js
const html = mjmx.render(source, {
  components: {
    // Return a string for HTML only…
    'mj-badge': (cx) => `<span>${cx.attr('label') ?? ''}</span>`,
    // …or an object to also contribute to <head>.
    'mj-card': (cx) => ({
      html: `<div class="card">${cx.rawInner}</div>`,
      styles: ['.card{border:1px solid #ccc}'],
      fonts: [['Inter', 'https://fonts.example/inter.css']], // emitted if referenced
    }),
  },
});
```

The callback receives a read-only `cx`: `attr(name)` and `attrs` (cascade-resolved through
`mj-attributes` / `mj-class` / defaults), `containerWidth`, `breakpoint`, and `rawInner`
(the verbatim inner text). A component is a **body leaf** — it does not drive the responsive
column layout (that is Tier 2, not yet available).

**Return cell *content*, not a cell.** mjmx wraps your fragment in the standard leaf `<td>`
cell (the one mjml's column emits around every child), so `<mj-badge>` above becomes
`<tr><td style="font-size:0px;word-break:break-word;"><span>…</span></td></tr>` — valid
table markup. The cell reads `align`, `css-class`, `container-background-color` and the
`padding` family from the component's own (cascade-resolved) attributes; set none and the
cell is the bare default. Runnable examples are in the repo's `examples/` directory.

## CSS inlining: `juiceOptions` and `juicePreserveTags`

mjml hands `<mj-style inline="inline">` to [juice]. mjmx has no juice — the inliner is
hand-rolled — so `juiceOptions` is not a pass-through object. Each juice option was
classified, and `JuiceOptions` is exactly the set that means something here:

```js
mjmx.render(source, {
  juiceOptions: {
    applyWidthAttributes: true,          // width  -> width  attr on table/td/th/img
    applyHeightAttributes: true,         // height -> height attr on the same
    applyAttributesTableElements: true,  // background-color -> bgcolor, etc. on <table>
    preserveImportant: false,            // keep `!important` in the written value
    styleAttributeName: 'style',
  },
});
```

Everything else juice accepts is **refused** — the TypeScript interface is closed, so
passing one in an object literal is a compile error. Note the enforcement is
compile-time only: N-API decodes this object into a typed Rust struct field by field,
so from plain JS an unknown key is dropped before mjmx sees it rather than throwing
(the wasm build, which receives the raw object, throws with the reason). Same
trade-off as `minifyOptions` here. The reasons, in short: `extraCss`,
`applyStyleTags`, `removeStyleTags` and `insertPreservedExtraCss` describe an
arrangement mjmx fixes before rendering (it splits `<mj-style>` into an inline bucket
and a `<style>` bucket, and the inliner only ever sees the first);
`preserveMediaQueries` / `preserveFontFaces` / `preserveKeyFrames` / `preservePseudos`
are unreachable under mjml's own configuration; `xmlMode`, `url` and `webResources`
have no analogue and the inliner does no I/O; and `resolveCSSVariables` and
`inlinePseudoElements` are real gaps that cannot be faked (mjmx writes `var(--x)`
through verbatim and drops `::before`/`::after` rules).

`juicePreserveTags` registers templating delimiters the inliner must not look inside:

```js
mjmx.render(source, {
  juicePreserveTags: { myTag: { start: '<#', end: '</#' } },
});
```

These **add** to the built-ins, which are always active whether or not you pass
anything: EJS (`<% %>`) and Handlebars (`{{ }}`), exactly as in juice. That matters —
without it, `{{ "<div class='a'>" }}` parses as a real tag and the inliner splices a
`style` attribute into the middle of your template expression.

One deliberate divergence from mjml: upstream assigns your tags into juice's
module-global `codeBlocks` object, so they leak into every later render in the same
process. mjmx's are per-call.

[juice]: https://github.com/Automattic/juice

`render`, `renderFile` and `format` throw an `Error` whose message aggregates the
diagnostics (`mjmx: error: <first message> (+N more diagnostics)`). The wrapper in
`index.js` also attaches the structured list as `err.diagnostics` — the same shape
`lint()` returns.

## Publishing

The main package and the per-platform packages under `npm/` publish separately. Build
each platform's binary on that platform (`cargo build -p mjmx-napi --release`), copy the
cdylib into `npm/<triple>/mjmx.<triple>.node`, then `npm publish` each `npm/<triple>`
followed by the root package. `npm run napi:version` keeps the platform-package versions
in lockstep with the root. There is no CI release workflow yet; the `napi` config block
and the generated `npm/` dirs are ready for one (`@napi-rs/cli`'s GitHub Actions flow).
