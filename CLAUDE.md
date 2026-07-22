# mjmx — project context

Rust port of [MJML](https://github.com/mjmlio/mjml). Project name **mjmx**. (Note: `mrml`,
a separate and unrelated published Rust MJML port, is discussed under Prior art — not us.)

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
- **Crate names:** hyphens (`mjmx-parser`); underscores in `use`.
- **Scaffolding:** grow per-milestone — a crate is born when its milestone arrives, workspace always compiles.
- **Testing oracle:** golden snapshots from mjml's own `.mjml`→`.html` fixtures, compared **semantically** (HTML/CSS-aware), not byte-exact — steal this from jdrouet/mrml's `html-compare`/`css-compare`.
- **Binary layout:** `apps/mjmx-cli` (oxc puts apps under `apps/`).

## Crate map (north star, ~22 crates)

Core pipeline: `mjmx-allocator` · `mjmx-span` · `mjmx-diagnostics` · `mjmx-syntax`
· `mjmx-index` · `mjmx-ast` (+ `mjmx-ast-macros`) · `mjmx-parser` · `mjmx-semantic`
· `mjmx-codegen` (renderer) · `mjmx-linter` · `mjmx` (umbrella) · `apps/mjmx-cli`.

Tools: `mjmx-traverse` · `mjmx-transformer` (mj-include, v3→v4 migrate) · `mjmx-minifier`
· `mjmx-formatter` (`mjmx fmt`) · `mjmx-css-inline` (`mj-style inline`).

HTML support: `mjmx-html-tokenizer` (one forgiving, span-carrying HTML tokenizer for the
whole workspace) · `mjmx-html-compare` (test-support only — production code must never
depend on it).

Surfaces: `mjmx-sourcemap` · `mjmx-language-server` (LSP) · `mjmx-wasm` + `mjmx-napi`
(wasm = drop-in for JS mjml) · `mjmx-dev-server` (`mjmx dev`, hot-reload preview,
axum + tokio-tungstenite + notify).

**Superseded:** this map used to say CSS inlining "lives in semantic/codegen". It does
not. Semantic partitions the two `<mj-style>` buckets and codegen hands the inline one
back as data; the inliner itself is `mjmx-css-inline`, a separate crate run by the
umbrella *after* codegen — because it is an HTML→HTML pass over the assembled document,
not a rendering decision. `mj-include` resolution via `mjmx-transformer` (+ a loader
abstraction: local + http).

## Roadmap (milestones)

Phase 1 — core pipeline:
- M0 workspace + Makefile + `mjmx-allocator`, `mjmx-span`, `mjmx-diagnostics`
- M1 `mjmx-syntax`, `mjmx-index`
- M2 `mjmx-ast` (+ macros)
- M3 `mjmx-parser`
- M4 `mjmx-codegen` slice (`mj-body/section/column/text`) + umbrella + cli → `mjmx file.mjml` runs
- M5 `mjmx-semantic` (mj-attributes / mj-class / defaults / mj-head)
- M6 fan out all components
- M7 `mjmx-linter` + golden fixtures (parity gate)

Phase 2 — toolchain: M8 traverse + transformer · M9 minifier + formatter · M10 sourcemap.
Phase 3 — surfaces: M11 wasm + napi · M12 language-server · M13 dev-server.

All of the above is **complete**, plus css-class, HTTP include loader (opt-in), v3→v4
migrate, `mjmx lsp`, incremental LSP sync, wasm-opt, and CSS inlining. Rendering parity has
**zero known divergences** from mjml 4.18.0 across 68 fixtures / 70 golden tests.

## CSS inlining (`mjmx-css-inline`, done)

Hand-rolled, no new third-party dep: `css-inline` costs +812 KB on the wasm artifact
(a 3.5x blow-up), so the project owns ~1000 LoC instead. Tokenizing uses `cssparser`
(already paid for by the minifier); HTML tokenizing uses `mjmx-html-tokenizer`.

Runs in `mjmx::render_source`, between `render_document_with_inline_css` and
`post_process` — after codegen, before minify/beautify — and only when the inline bucket
is non-empty. Behaviour was derived from mjml 4.18.0's *output*, not from juice's source,
and several points contradict what juice's source appears to say:

- only `<mj-style inline="inline">` is inlined, and it is **never** also a `<style>` tag;
  plain `<mj-style>` is a `<style>` tag and is never inlined (`applyStyleTags:false`);
- non-inlinable rules **vanish**: at-rules, `:hover/:active/:focus/:visited/:link`,
  `::before`/`::after` — not inlined, not preserved, not re-emitted;
- a pre-existing `style` attribute **beats** a plain inlined rule (and mjml puts one on
  nearly every element, so plain rules usually lose);
- `!important` deletes the original declaration and re-appends it *unmarked* at the end;
  non-conflicting rule declarations are **prepended**;
- only elements a rule matched are rewritten, in `prop: value; ` form — untouched
  elements keep mjml's compact `font-size:0px;`, so both spellings coexist in one document;
- `width`/`height` become **unitless** attributes on `table`/`td`/`th`/`img`, and only
  from an integral px value (`100%`, `auto` are left alone); on `<table>`,
  `background-color`→`bgcolor`, `background-image`→`background`, `text-align`→`align`,
  `vertical-align`→`valign`, **additively** (declaration kept too).

**Hard invariant:** the inliner must never descend into conditional comments. mjmx merges
Outlook conditionals inside `render_document`, i.e. *before* the inliner, where upstream
runs juice first; the two orders agree only because juice never sees inside `<!--[if …]>`.
`fixtures/mj_style_conditional_order` is the regression test.

`render_with_sourcemap` **refuses** a document with an inline bucket (inlining shifts every
recorded offset). The refusal lives in codegen's `sourcemap_refusal`, after semantic
analysis, because it is a property of the document rather than of the options.

Known gap: functional/structural pseudo-classes (`:nth-child`, `:not`, …) and CSS Nesting
are dropped rather than mis-applied.

## Options parity (audit done 2026-07-22 against mjml 4.18.0 source)

`RenderOptions` (`mjmx-codegen/src/context.rs`) is the single option carrier, threaded to CLI,
wasm and napi. Tier 1 is **shipped**:

| field | mjml option | default | note |
| --- | --- | --- | --- |
| `breakpoint: u32` | — | 480 | **mjmx extension.** Upstream hardcodes 480px; only `<mj-breakpoint>` changes it. |
| `beautify` / `minify` | same | false | post-process only; deprecated in mjml-core but still functional |
| `keep_comments: bool` | `keepComments` | **true** | matches upstream's parser default |
| `validation_level` | `validationLevel` | `Soft` | `Soft` renders + reports, `Strict` fails, `Skip` never lints |
| `file_path: Option<PathBuf>` | `filePath` | None | include base for **string** input; dir → itself, file → parent |
| `fonts: Option<Vec<FontFace>>` | `fonts` | None | **replaces** the built-in map, never merges; key order decides `<link>` order (hence `indexmap`) |

Tier 2 is **shipped** (3 of 5; two were dead ends, see below):

| field | mjml option | default | note |
| --- | --- | --- | --- |
| `minify_options: MinifyOptions` | `minifyOptions` | `{}` | only read when `minify`; new `MinifyOptions` struct + `mjmx_minifier::minify_with` |
| `ignore_includes: bool` | `ignoreIncludes` | false | **drops** `<mj-include>`, does not merely skip resolution; wins over `file_path` and a file render's own base |
| `printer_support: bool` | `printerSupport` | false | third `<style>` replaying the base column rules under `@media only print`, after the screen + Thunderbird blocks; suppressed when there are no column rules, as upstream |

`MinifyOptions` = the four html-minifier switches mjml hardcodes
(`mjml-core/src/index.js:419-425`): `collapse_whitespace` (true), `minify_css` (false),
`case_sensitive` (true), `remove_empty_attributes` (**false — mjml passes true**).
`minify_css` is a conservative whitespace/comment pass, *not* clean-css.

**The one deliberate default divergence:** mjml passes `removeEmptyAttributes: true`, and
mjml's own output carries `<div … style="" role="article">` — so mjml's default *removes
an attribute* while minifying. Honoring it by default broke `mjmx-minifier`'s semantic
invariance guarantee (`minify(x)` is html-compare-equal to `x`) and shifted 54/56
fixtures' `--minify` bytes. Defaulted off; pass `--minify-options
'{"removeEmptyAttributes": true}'` for byte parity with mjml. Reconsider if parity is
judged to outweigh the invariant.

Real bug caught by reading html-minifier's source: `reEmptyAttribute` is
`^(?:class|id|style|title|lang|dir|on(focus|blur|change|click|dblclick|mouse…|key…))$`
plus `<input value>` — *not* "every empty attribute" and *not* "every `on*` handler".

CLI: `--minify-options <JSON>` (mjml's own camelCase keys, unknown keys rejected),
`--ignore-includes`, `--printer-support`.

`render_with_diagnostics` / `render_file_with_diagnostics` mirror mjml's `{ html, errors }`.

Decisions taken: CLI uses idiomatic flags (`--keep-comments/--no-keep-comments`,
`--validation-level`, `--file-path`, `--fonts`) rather than mjml's `--config.*`, since the CLI
already had flat `--beautify`/`--minify`. `--strict` is now an alias for
`--validation-level strict` and no longer `requires` `--validate`; `--validate` is pure
reporting. **wasm rejects `filePath`** (typed `never`, throws) — no FS there, and silently
ignoring it would return HTML with `<mj-include>` unresolved; napi supports it fully.

Gotcha worth remembering: a comment child of `mj-section` used to land inside the *open* ghost-row
Outlook conditional, and `-->` does not nest — it truncated the conditional. Fixed by matching
upstream's shape (each conditional opened *and* closed); `merge_outlook_conditionals` in
`document.rs` re-fuses them, so byte output is unchanged for comment-free documents.

### Still open

- **Tier 2 dead ends — deliberately *not* implemented as fields:**
  - `noMigrateWarn` is a **no-op for mjmx**. Upstream calls `handleMjml3(mjml, …)`
    unconditionally inside `mjml2html` (`mjml-core/src/index.js:151`) and logs "MJML v3
    syntax detected, migrating…" unless suppressed. mjmx's render path never migrates —
    `mjmx::migrate` is an explicit, separate entry point that returns *source*, and it
    reports via `Diagnostic`s, not a console warning. There is nothing to suppress, so a
    field would be dead. Revisit only if render ever auto-migrates.
  - `actualPath` has **nowhere to land**. `mjmx_diagnostics::Diagnostic` is
    `{severity, message, labels, help}` — no file provenance anywhere, and neither do AST
    nodes. Upstream sets `node.file` / `node.absoluteFilePath` *per node at parse time*,
    and crucially passes `actualPath: partialPath` for each included partial
    (`mjml-parser-xml/src/index.js:163-164`), so nodes from a partial report the
    partial's path. Bolting a single flat `file` onto `Diagnostic` would report the
    top-level path for include-sourced diagnostics — exactly the lie `actualPath` exists
    to prevent. Per-node file provenance (which would also fix the existing gap that
    spans from an included partial are meaningless in the main source) is a
    parser/AST milestone, not an option.
- **Tier 3:** `mjmlConfigPath` / `useMjmlConfigOptions` — needs a `.mjmlconfig` reader.
- **Unblocked, not implemented:** `juiceOptions` / `juicePreserveTags`. These were blocked
  on CSS inlining, which now exists (`mjmx-css-inline`), so they are ordinary options
  again — deliberately deferred to a later pass, not part of the inlining milestone.
  `juicePreserveTags` in particular mutates the global juice module upstream, so mjmx will
  need its own shape for it rather than a port.
- **Won't port:** `skeleton`, `preprocessors`, `presets` — JS function / `require()` semantics.
- Upstream oddities deliberately not copied: `--config.*` booleans don't coerce `"false"`;
  `juicePreserveTags` mutates the global juice module; mjml-cli's README documents a `-l` flag
  that does not exist in 4.18.0.

## Planned next

1. **Ship as three packages: JS, Rust, Python.**
   - JS: `@mjmx/node` (napi) and `mjmx-wasm` exist but are unpublished/`private`. Needs real
     packaging — naming, README, install story, ESM+CJS, per-platform prebuilds.
   - Rust: `mjmx` umbrella as a published-quality crate (docs.rs metadata, crate-level docs,
     examples, keywords).
   - Python: **new binding** (pyo3/maturin). This reverses the earlier "wasm + napi only, no
     python" decision recorded under Prior art.
2. **CLI parity audit** — walk the mjml README's CLI section and verify every argument has an
   `mjmx` equivalent (`-r`, `-w`/`--watch`, `-s`, `-m`, …); fill the gaps. Partially done:
   `dev --breakpoint` and `fmt`/`migrate --indent-width` were added during the options pass.

Method reminder: read mjml's actual README/source rather than guessing — that has caught real
bugs every single time.

## Working agreement

- **Plan → approve → code.** Always show the plan and wait for explicit approval before
  writing code, creating files, `git init`, or committing. Do not assume the user's goal
  or expected output — ask.
- Advise on tradeoffs; the user decides.

## Prior art — jdrouet/mrml (the existing `mrml` crate, v6)

A mature Rust MJML renderer (published as `mrml` — the name this project deliberately moved
off of). **Different philosophy: renderer library, not a compiler toolchain.** Key contrasts
vs this project:

| Aspect | jdrouet/mrml | this project (mjmx) |
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
