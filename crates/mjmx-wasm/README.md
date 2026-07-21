# mjmx-wasm

WebAssembly bindings for **mjmx**, an MJML-to-HTML compiler written in Rust. This is
the "drop-in for JS mjml" surface: the same compiler the CLI uses, compiled to
`wasm32-unknown-unknown` and callable from browsers, Node, Deno, Bun or any
wasm-capable runtime.

The crate is a **thin translation layer** — it converts JS values to the umbrella
crate's Rust types, calls straight through, and converts back. No compiler logic
lives here.

## Build

```sh
make wasm        # cargo build --target wasm32-unknown-unknown --release + wasm-bindgen
make wasm-test   # build, then run the Node smoke test
```

Two toolchain pieces are required, and the CLI version **must** match the
`wasm-bindgen` crate version pinned in the workspace `Cargo.toml` (`=0.2.100`),
or code generation refuses to run:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.100 --locked
```

Output lands in `crates/mjmx-wasm/pkg/` (gitignored):

| File | Purpose |
| --- | --- |
| `mjmx_wasm.js` | CommonJS glue (`--target nodejs`) |
| `mjmx_wasm.d.ts` | TypeScript declarations |
| `mjmx_wasm_bg.wasm` | the compiled module (~315 KiB release) |

For a browser bundle, re-run `wasm-bindgen` with `--target web` or `--target bundler`
against the same `target/wasm32-unknown-unknown/release/mjmx_wasm.wasm`.

## API

```ts
render(source: string, options?: RenderOptions): string   // throws on diagnostics
lint(source: string): Diagnostic[]                        // never throws
format(source: string, options?: FormatOptions): string   // throws on parse errors
version(): string

interface RenderOptions { breakpoint?: number; beautify?: boolean; minify?: boolean }
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
const { render } = require('./pkg/mjmx_wasm.js');
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

## Workspace note

`mjmx-wasm` is a normal workspace member and builds on the **host** target too
(`wasm-bindgen`'s non-wasm code paths compile fine; the exported functions simply are
not callable there). That keeps `cargo build --workspace`, `cargo test --workspace` and
`cargo clippy --workspace --all-targets` green without excluding the crate. The
`cdylib` + `rlib` crate types mean the host build produces an (unused) shared library
alongside the rlib.
