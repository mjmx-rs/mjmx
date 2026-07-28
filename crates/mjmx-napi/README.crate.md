# mjmx-napi (crate)

N-API bindings for [mjmx](https://github.com/mjmx-rs/mjmx), an MJML-to-HTML compiler
written in Rust. Published to npm as [`mjmx-node`](https://www.npmjs.com/package/mjmx-node).

> **This is the crate-facing README.** The npm package renders `README.md`, which is
> written for JavaScript consumers. npm always reads `README.md` and cannot be pointed
> elsewhere, so Cargo's `readme` field points here instead.

The crate is a **thin translation layer**: it converts JS values into the umbrella
crate's Rust types, calls straight through, and converts the result back. No compiler
logic lives here.

## Not a normal Rust dependency

Nothing depends on this crate from Rust. It exists to be compiled into a `.node` addon
and consumed from JavaScript. To render MJML from Rust, depend on the `mjmx` umbrella
crate directly.

## Layout

The npm package lives alongside the crate in this directory:

| file | role |
| --- | --- |
| `package.json` | npm manifest, incl. the per-triple `optionalDependencies` |
| `index.js` | CommonJS loader + thin wrapper (adds `cx.attr()` to component callbacks) |
| `index.mjs` | ESM re-export |
| `index.d.ts` | hand-written types — the closed `RenderOptions` interface |
| `npm/<triple>/` | per-platform package scaffolding (`napi create-npm-dirs`) |
| `test/smoke.mjs` | the smoke test `make napi-test` runs |

## Build & test

```sh
make napi        # cargo build -p mjmx-napi --release, then stage the .node
make napi-test   # the above, then node crates/mjmx-napi/test/smoke.mjs
```

The staged filename is `mjmx.<platform>-<arch>[-gnu].node`, which is what `index.js`
looks for when no per-triple package is installed — so a bare `make napi` is enough for
local development.

`.node` binaries are gitignored; only `npm/<triple>/package.json` is committed as
scaffolding.

## The loader

`index.js` is hand-written rather than generated, because it enriches errors with the
diagnostics array. It resolves in two steps:

1. `require('mjmx-node-<triple>')` — the optional dependency npm installed for this host.
2. Fall back to a locally built `.node` in this directory.

If neither is present it throws naming the triple it looked for, rather than failing
deeper with a confusing message.

## Custom components across the boundary

A JS component callback is held as a `FunctionRef` and `borrow_back`ed with the `Env` at
call time, then invoked **synchronously on the JS thread** during the render — no
threadsafe-function, no async. Head contributions come back through the callback's
**return value** rather than by mutating a live `cx`, which keeps the JS call one-way
and avoids reentrancy.

`index.js` wraps each callback to add the `cx.attr(name)` method as a pure lookup over
the plain-data `attrs` snapshot the native side passes in, so the native code never
calls back into JS to resolve an attribute.

## Publishing

Not yet published. It needs a CI release workflow (`@napi-rs/cli`'s GitHub Actions flow)
to cross-build all five triples — `linux-x64-gnu`, `linux-arm64-gnu`, `darwin-x64`,
`darwin-arm64`, `win32-x64-msvc` — and publish each `mjmx-node-<triple>` package
alongside the main one.

**Publishing from one machine would break installs everywhere else**: npm resolves the
optional dependency for the host platform, and any triple whose package does not exist
leaves the loader with nothing to load. All five must go up together.

Bump the **workspace** version rather than only `package.json`, and keep the
`optionalDependencies` pins in step with it.

## Note on option parity

`index.d.ts` is hand-written and deliberately **closed**: `#[napi(object)]` decodes field
by field, so an unknown key is dropped before mjmx ever sees it. The TypeScript interface
is therefore the only place an unsupported option can be refused, and it does so at
compile time. The wasm binding, which decodes through `serde-wasm-bindgen`, throws at
runtime instead.

## License

MIT
