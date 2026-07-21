# mrml — project context

Rust port of [MJML](https://github.com/mjmlio/mjml). Working name `mrml` (a published
`mrml` crate already exists — see Prior art; rename before publishing).

## Mental model

Treat MJML as a **language** (like HTML), and build a **compiler-style toolchain** for it,
following the architecture of [oxc](https://github.com/oxc-project/oxc) (the JS/TS Rust
toolchain). Scope is the *engine / cli / renderer* — NOT a clone of mjml's per-component
npm packages (those are consumer/template libraries, out of scope).

Pipeline: `source → lexer/parser → AST (arena, spanned) → semantic → codegen (renderer) → HTML`.
Components are AST node kinds with render impls **inside codegen** (like oxc_codegen's `Gen`
per node), never separate crates.

## Locked decisions

- **Goal:** full MJML feature parity.
- **Architecture:** oxc-style, many focused crates.
- **AST:** arena-allocated (bumpalo), nodes carry lifetime `'a`, strings interned as `Atom`. Every node spanned.
- **Diagnostics:** thin custom (span-labeled), no heavy dep. Not just thiserror strings.
- **Crate names:** hyphens (`mrml-parser`); underscores in `use`.
- **Scaffolding:** grow per-milestone — a crate is born when its milestone arrives, workspace always compiles.
- **Testing oracle:** golden snapshots from mjml's own `.mjml`→`.html` fixtures, compared **semantically** (HTML/CSS-aware), not byte-exact — steal this from jdrouet/mrml's `html-compare`/`css-compare`.
- **Binary layout:** `apps/mrml-cli` (oxc puts apps under `apps/`).

## Crate map (north star, ~22 crates)

Core pipeline: `mrml-allocator` · `mrml-span` · `mrml-diagnostics` · `mrml-syntax`
· `mrml-index` · `mrml-ast` (+ `mrml-ast-macros`) · `mrml-parser` · `mrml-semantic`
· `mrml-codegen` (renderer) · `mrml-linter` · `mrml` (umbrella) · `apps/mrml-cli`.

Tools: `mrml-traverse` · `mrml-transformer` (mj-include, v3→v4 migrate) · `mrml-minifier`
· `mrml-formatter` (`mrml fmt`).

Surfaces: `mrml-sourcemap` · `mrml-language-server` (LSP) · `mrml-wasm` + `mrml-napi`
(wasm = drop-in for JS mjml) · `mrml-dev-server` (`mrml dev`, hot-reload preview,
axum + tokio-tungstenite + notify).

CSS inlining (`mj-style inline`) lives in semantic/codegen. `mj-include` resolution via
`mrml-transformer` (+ a loader abstraction: local + http).

## Roadmap (milestones)

Phase 1 — core pipeline:
- M0 workspace + Makefile + `mrml-allocator`, `mrml-span`, `mrml-diagnostics`
- M1 `mrml-syntax`, `mrml-index`
- M2 `mrml-ast` (+ macros)
- M3 `mrml-parser`
- M4 `mrml-codegen` slice (`mj-body/section/column/text`) + umbrella + cli → `mrml file.mjml` runs
- M5 `mrml-semantic` (mj-attributes / mj-class / defaults / mj-head)
- M6 fan out all components
- M7 `mrml-linter` + golden fixtures (parity gate)

Phase 2 — toolchain: M8 traverse + transformer · M9 minifier + formatter · M10 sourcemap.
Phase 3 — surfaces: M11 wasm + napi · M12 language-server · M13 dev-server.

## Working agreement

- **Plan → approve → code.** Always show the plan and wait for explicit approval before
  writing code, creating files, `git init`, or committing. Do not assume the user's goal
  or expected output — ask.
- Advise on tradeoffs; the user decides.

## Prior art — jdrouet/mrml (the existing `mrml` crate, v6)

A mature Rust MJML renderer. **Different philosophy: renderer library, not a compiler
toolchain.** Key contrasts vs this project:

| Aspect | jdrouet/mrml | this project |
| --- | --- | --- |
| Crate split | one mega-crate `mrml-core` (components = modules) | many focused oxc-style crates |
| AST | owned + serde-serializable (`json` feature) | arena (bumpalo) + spans + `Atom` |
| Positions | ~none | first-class spans everywhere |
| Errors | thiserror strings | span-labeled diagnostics |
| Pipeline | feature flags: parse/print/render/json | separate crates per stage |
| Tooling | parse, print (AST→MJML), render, css-inline, http/local loader | + linter, formatter, transformer, sourcemap, LSP, dev-server |
| Bindings | wasm + python | wasm + napi (consider python later) |

**Adopt from them:** semantic HTML/CSS comparison for golden tests (`html-compare`,
`css-compare`); the `mj-include` loader abstraction (local + http); explicit `css-inline`
step; optional `async` for async loaders.

**Deliberate divergence:** arena + spans buys speed, precise diagnostics, and makes the LSP
/ formatter / sourcemap surfaces natural — at the cost of cheap serde JSON round-trip of the
AST. **Decided:** AST↔JSON (mjml→json) is out of scope — pure arena, no serde AST. Bindings
are **wasm + napi only** (no python).
