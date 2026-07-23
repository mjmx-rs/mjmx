# @mjmx/wasm

WebAssembly bindings for **mjmx**, an MJML-to-HTML compiler written in Rust. This is
the "drop-in for JS mjml" surface: the same compiler the CLI uses, compiled to
`wasm32-unknown-unknown` and callable from browsers, Node, Deno, Bun or any
wasm-capable runtime.

The crate is a **thin translation layer** — it converts JS values to the umbrella
crate's Rust types, calls straight through, and converts back. No compiler logic
lives here.

## Install

```sh
npm i @mjmx/wasm
```

The package ships **three wasm-bindgen flavours** behind a conditional `exports` map, so
one import resolves correctly everywhere:

| condition | flavour (`pkg/…`) | for |
| --- | --- | --- |
| `node` | `node/` (CommonJS, `require`) | Node.js |
| `browser` | `web/` (ESM, self-fetching) | browsers / Deno, no bundler |
| `import` / default | `bundler/` (ESM) | webpack / vite / rollup |

```js
const { render } = require('@mjmx/wasm');   // Node (CommonJS)
import { render } from '@mjmx/wasm';         // bundler / browser (ESM)
```

## Build

```sh
make wasm         # cargo build --target wasm32-unknown-unknown --release + wasm-bindgen + wasm-opt
make wasm-test    # build, then run the Node smoke test against the built artifact
make wasm-release # the same, but fail up-front if wasm-opt is missing
```

Two toolchain pieces are required, and the CLI version **must** match the
`wasm-bindgen` crate version pinned in the workspace `Cargo.toml` (`=0.2.100`),
or code generation refuses to run:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.100 --locked
```

A third, `wasm-opt` from [binaryen](https://github.com/WebAssembly/binaryen), is
**optional**: `make wasm` runs it with `-Oz` when it is on `PATH` and prints a
hint instead of failing when it is not, so the build never depends on it. It is
worth having — it cuts each module by ~17%:

```sh
cargo install wasm-opt        # or: npm i -g binaryen, or a distro package
```

| | `mjmx_wasm_bg.wasm` (per flavour) |
| --- | --- |
| wasm-bindgen output | ~558 KiB |
| after `wasm-opt -Oz` | ~463 KiB (-17%) |

`make wasm-test` runs the smoke test against the **optimised** artifact, so the
size win is always verified to still render byte-identically to the golden output.

`make wasm` runs one cargo build and three `wasm-bindgen` passes, landing in
`crates/mjmx-wasm/pkg/` (gitignored) as `node/`, `bundler/` and `web/` subdirs. Each
holds `mjmx_wasm.js` (glue), `mjmx_wasm.d.ts` (types) and `mjmx_wasm_bg.wasm` (the
optimised module); the `bundler/` flavour additionally emits `mjmx_wasm_bg.js`. The
package.json `exports` map wires the three to the `node` / `browser` / `import`
conditions (see Install above).

## API

```ts
render(source: string, options?: RenderOptions): string   // throws on diagnostics
lint(source: string): Diagnostic[]                        // never throws
format(source: string, options?: FormatOptions): string   // throws on parse errors
version(): string

// mjml's own `mjml2html` option names, with mjml's defaults. `breakpoint` is the one
// mjmx extension; `filePath` is the one mjml option wasm cannot support (no
// filesystem, so there is nothing to resolve `<mj-include>` against) and passing it
// throws rather than silently returning HTML with the includes unresolved.
interface RenderOptions {
  breakpoint?: number;                              // default 480 (mjmx extension)
  beautify?: boolean;                               // default false
  minify?: boolean;                                 // default false
  keepComments?: boolean;                           // default true
  validationLevel?: 'soft' | 'strict' | 'skip';     // default 'soft'
  filePath?: never;                                 // unsupported here — throws
  fonts?: Record<string, string>;                   // replaces the built-in table
  minifyOptions?: MinifyOptions;                    // only read when `minify` is true
  ignoreIncludes?: boolean;                         // default false; true drops them
  printerSupport?: boolean;                         // default false; @media only print
  juiceOptions?: JuiceOptions;                      // how to inline <mj-style inline>
  juicePreserveTags?: Record<string, PreserveTag>;  // extra delimiters to skip
}
// mjml's `juiceOptions`, restricted to what mjmx's hand-rolled inliner can honour —
// there is no juice here to forward an object to. Everything else juice accepts is
// *refused with the reason*, never accepted and ignored: `extraCss`, `applyStyleTags`,
// `removeStyleTags` and `insertPreservedExtraCss` describe an arrangement mjmx fixes
// before rendering; `preserveMediaQueries`/`preserveFontFaces`/`preserveKeyFrames`/
// `preservePseudos` are unreachable under mjml's own configuration; `xmlMode`, `url`
// and `webResources` have no analogue; `resolveCSSVariables` and
// `inlinePseudoElements` are real gaps that cannot be faked.
interface JuiceOptions {
  applyWidthAttributes?: boolean;                   // default true
  applyHeightAttributes?: boolean;                  // default true
  applyAttributesTableElements?: boolean;           // default true
  preserveImportant?: boolean;                      // default false (juice's own)
  styleAttributeName?: string;                      // default "style"
}
// Templating delimiters the inliner must not look inside. These *add* to the
// always-active built-ins — EJS (`<% %>`) and Handlebars (`{{ }}`) — so
// `{{ "<div class='a'>" }}` is never read as a tag. Per-call, unlike mjml, which
// writes them into juice's module-global `codeBlocks` and leaks them across renders.
interface PreserveTag { start: string; end: string }
// mjml's four html-minifier switches. `removeEmptyAttributes` is the one default that
// differs from mjml's `true`: mjml's own output carries `<div … style="">`, so `true`
// removes an attribute while minifying. mjmx keeps `minify` a pure whitespace
// transform by default; pass `true` for byte parity with mjml.
interface MinifyOptions {
  collapseWhitespace?: boolean;                     // default true
  minifyCSS?: boolean;                              // default false (conservative)
  caseSensitive?: boolean;                          // default true
  removeEmptyAttributes?: boolean;                  // default false (mjml: true)
}
interface FormatOptions { indentWidth?: number }
interface DiagnosticLabel { start: number; end: number; message: string }
interface Diagnostic {
  severity: 'error' | 'warning' | 'advice';
  message: string;
  labels: DiagnosticLabel[];
  help?: string;
}
```

```js
const { render } = require('@mjmx/wasm'); // or './pkg/node/mjmx_wasm.js' from a local build
console.log(render('<mjml><mj-body><mj-section><mj-column><mj-text>Hi</mj-text></mj-column></mj-section></mj-body></mjml>'));
```

`render` and `format` throw a JS `Error` whose message aggregates the diagnostics,
e.g. `mjmx: error: unclosed element `<mj-section>` (help: …) (+2 more diagnostics)`.
Call `lint()` on the same source for the structured list.

## No filesystem: `renderFile` / `mj-include`

The Node addon (`crates/mjmx-napi`) exports `renderFile(path)`, which resolves
`<mj-include path="./partial.mjml">` against the file's directory. **That function is
deliberately absent here.** WebAssembly has no ambient filesystem, so `mjmx::render_file`
and include resolution — which need a base directory and a loader — cannot work in this
target. Options:

- inline the partials into the source string before calling `render()`, or
- use `@mjmx/node` (the N-API addon) when a host filesystem is available.

A future revision could accept a caller-supplied `resolve(path) => string` callback to
run include resolution through JS; that is not implemented today.

## Publishing

`npm run build` (i.e. `make wasm`) produces all three flavours into `pkg/`; the
package.json `files` whitelist ships `pkg/node`, `pkg/bundler` and `pkg/web` (the `files`
entry overrides the gitignored `pkg/`, so `npm pack` includes them). Then `npm publish`.
There is no CI release workflow yet.

## Workspace note

`mjmx-wasm` is a normal workspace member and builds on the **host** target too
(`wasm-bindgen`'s non-wasm code paths compile fine; the exported functions simply are
not callable there). That keeps `cargo build --workspace`, `cargo test --workspace` and
`cargo clippy --workspace --all-targets` green without excluding the crate. The
`cdylib` + `rlib` crate types mean the host build produces an (unused) shared library
alongside the rlib.
