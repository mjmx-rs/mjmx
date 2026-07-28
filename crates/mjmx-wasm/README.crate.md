# mjmx-wasm (crate)

WebAssembly bindings for [mjmx](https://github.com/mjmx-rs/mjmx), an MJML-to-HTML
compiler written in Rust. Compiled to `wasm32-unknown-unknown` and shipped to npm as
[`@mjmx-rs/mjmx`](https://www.npmjs.com/package/@mjmx-rs/mjmx).

> **This is the crate-facing README.** The npm package renders `README.md`, which is
> written for JavaScript consumers. npm always reads `README.md` and cannot be pointed
> elsewhere, so Cargo's `readme` field points here instead.

The crate is a **thin translation layer**: it converts JS values into the umbrella
crate's Rust types, calls straight through, and converts the result back. No compiler
logic lives here — that is `mjmx` and the crates beneath it.

## Not a normal Rust dependency

Nothing depends on this crate from Rust. It exists to be compiled to wasm and consumed
from JavaScript; the exported functions are `#[wasm_bindgen]` entry points, not a Rust
API. If you want to render MJML from Rust, depend on the `mjmx` umbrella crate directly.

## Building

```sh
make wasm         # cargo build --target wasm32-unknown-unknown --release
                  #   + three wasm-bindgen passes + wasm-opt
make wasm-test    # build, then run the Node smoke test against the built artifact
make wasm-release # the same, but fail up front if wasm-opt is missing
```

Two toolchain pieces are required. The `wasm-bindgen` **CLI version must exactly match**
the crate version pinned in the workspace `Cargo.toml` (`=0.2.100`), or code generation
refuses to run:

```sh
rustup target add wasm32-unknown-unknown       # declared in rust-toolchain.toml
cargo install wasm-bindgen-cli --version 0.2.100 --locked
```

`make wasm-tools` checks both and prints the exact install command when either is
missing or mismatched. It reads the pin out of `Cargo.toml` rather than duplicating it,
so the check cannot drift from the dependency.

A third piece, `wasm-opt` from [binaryen](https://github.com/WebAssembly/binaryen), is
optional for `make wasm` (which skips it with a hint) but **required** by
`make wasm-release`. It is worth having:

| | `mjmx_wasm_bg.wasm`, per flavour |
| --- | --- |
| wasm-bindgen output | ~740 KiB |
| after `wasm-opt -Oz` | ~597 KiB (−19%) |

```sh
cargo install wasm-opt        # or: npm i -g binaryen, or a distro package
```

`make wasm-test` runs the smoke test against the **optimised** artifact, so the size win
is always verified to still render correctly.

## Output layout

One cargo build feeds three `wasm-bindgen` passes, landing in `crates/mjmx-wasm/pkg/`
(gitignored) as `node/`, `bundler/` and `web/`. Each holds `mjmx_wasm.js` (glue),
`mjmx_wasm.d.ts` (types) and `mjmx_wasm_bg.wasm`; the `bundler/` flavour additionally
emits `mjmx_wasm_bg.js`. The `package.json` `exports` map wires the three to the `node`
/ `browser` / `import` conditions.

`"type": "module"` is deliberately **not** set: it would reinterpret the CommonJS Node
glue as ESM and break `require`.

## Publishing

`npm run build` (i.e. `make wasm`) produces all three flavours into `pkg/`. The
`package.json` `files` whitelist ships `pkg/node`, `pkg/bundler` and `pkg/web` — that
entry overrides the gitignored `pkg/`, so `npm pack` includes them. Then `npm publish`.

The npm account enforces 2FA, so publishing needs `--otp=<code>` or a granular access
token with 2FA bypass. There is no CI release workflow yet.

Bump the **workspace** version rather than only `package.json`: `version()` returns
`CARGO_PKG_VERSION`, so bumping one without the other makes the package report a
version different from its own.

## Workspace note

`mjmx-wasm` is a normal workspace member and builds on the **host** target too —
`wasm-bindgen`'s non-wasm code paths compile fine, the exported functions simply are not
callable there. That keeps `cargo build --workspace`, `cargo test --workspace` and
`cargo clippy --workspace --all-targets` green without excluding the crate. The
`cdylib` + `rlib` crate types mean the host build also produces an unused shared library
alongside the rlib.

## License

MIT
