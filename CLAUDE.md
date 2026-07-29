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
migrate, `mjmx lsp`, incremental LSP sync, wasm-opt, CSS inlining, and
`<mj-html-attributes>` (see below). Rendering parity has **no known divergences** from
mjml 4.18.0 across 79 fixtures / 81 golden tests — a 2026-07-22 audit found three real
divergences (mj-raw dropped/mis-wrapped in scaffolding parents; `<mj-html-attributes>`
silently unimplemented; a suspected bare-attribute divergence), and all three are now
fixed or resolved with a real-mjml golden fixture each (`mj_raw_in_{column,group,section,
hero}`, `mj_html_attributes{,_edge}`, `section_full_width_bare`). The bare-attribute case
proved to be *agreement*, not a divergence: mjml's htmlparser2 gives a valueless attribute
the value `""` and `convertBooleans` only maps `"true"`/`"false"`, so `<mj-section
full-width>` fails the `=== 'full-width'` check exactly as mjmx's absent-valued attribute
does; the fixture locks that in. (One theoretical, non-real-world edge remains: mjml stores
a bare attribute as `""` where mjmx stores it as absent, which could only diverge for an
attribute whose *value* is echoed verbatim into output under a bare valueless spelling —
no such construct occurs in real MJML.) **This paragraph describes the V4 path**, which
is now frozen — see the mjml-version section directly below.

**Generated-MJML fixtures** (`generated_subset_{doc,blocks,raw_iconrow}`, added
2026-07-27). Every other fixture is hand-written MJML; these reproduce what a
**design-to-email generator** emits (the `figma-email` plugin's `docToMjml` is the model),
which leans on constructs a human rarely writes: explicit `padding` on *every* block (a
generator means "no padding", not "inherit mjml's non-zero default"), fractional %-width
columns from fill sizing, and `mj-raw` `<table>`s for icon rows since `mj-column` cannot
nest a column. One `.mjml` source each, two references — `<name>.html` from real
`mjml@4.18.0` (V4 gate, `golden.rs`) and `<name>_v5.html` from real `mjml@5.4.0` (V5 gate,
`golden_v5.rs`, via the three-arg `golden_v5!` arm so the source is never copied). They
paid for themselves on the first run by catching the ghost-width rounding divergence
recorded below. Regenerate with the pinned bench packages
(`cd benches/perf && npm install`, then render with `{ beautify: true }`) — **never**
hand-edit a golden.

## mjml version compat (`MjmlCompat`) — 5.4.0 default, 4.18.0 opt-in

**mjmx now reproduces mjml 5.4.0 by default and mjml 4.18.0 on request.** mjml has no
such option (an install *is* one version); mjmx carries both because a renderer that
silently changes its HTML on upgrade is unusable in a pipeline that diffs output.

- `MjmlCompat { V4, V5 }` in `mjmx-codegen/src/context.rs`, **default `V5`**, with an
  `is_v5()` helper that render sites branch on (so a future `V6` does not silently inherit
  V4 at every call site). Carried on `RenderOptions.compat`, threaded to CLI
  (`--mjml-version 4|5`, default 5), wasm (`mjmlVersion?: 4 | 5`) and napi (same).
- **V4 is frozen**: it is the 4.18.0 behaviour mjmx shipped before 5.x, and the 78 golden
  tests are its byte-identical regression gate. Every V5 change is gated on `is_v5()` so
  V4 output never moves. Every V5 behaviour was **verified byte-for-byte against real
  `mjml@5.4.0` output** (via `npm pack` / running the package), never the upgrade guide —
  the guide was wrong twice (see "verified non-changes" below).

What V5 changes, by area (each `is_v5()`-gated; V4 unchanged):

- **Skeleton** (`document.rs`, `components/body.rs`): `<body>` is generated under
  `mj-body`, not the global skeleton; the wrapper `<div>` drops its empty `style=""` and
  takes the whole-body style at the tag end; `css-class` moves from the div onto the
  `<body>`; `<mj-body id>` is emitted. Context carries `body_css_class` / `body_id`
  (V5-only).
- **Minifier** (`mjmx-minifier`): `MinifyOptions::for_mjml5()` (the V5 default) turns on
  CSS minification with `CssDialect::Cssnano` — cssnano leaves selector preludes
  byte-for-byte and closes the space-before-`!important` gap, vs cleancss (V4). Style
  *attributes* are minified too (`tag.rs` + `css::minify_declarations`). This **narrows**
  the minifier's semantic-invariance guarantee (`minify(x)` html-compare-equal to `x`) for
  V5, so the invariance tests are pinned to V4.
- **Includes** (`mjmx-transformer`, `mjmx/src/lib.rs`): mjml 5.4.0 flipped `ignoreIncludes`
  to **true by default** and added a **path sandbox**. `ignore_includes` is now
  `Option<bool>` — `None` follows compat (`resolve_ignore_includes()`: V5 ignores, V4
  resolves); new `include_path` roots. The sandbox (`IncludeOptions { sandbox, roots }`,
  V5-only) denies an include that escapes the base via `../`, an absolute/drive-letter/UNC
  path, a NUL byte, a percent-encoded `../`, a symlink whose real target is outside, or a
  missing file — replacing it with a non-blocking `<!-- mj-include denied -->` comment
  (mjml's `denyInclude`). Containment is checked on canonicalized paths, and the
  **canonical path is what gets loaded** — closing the check→read symlink window (mjml has
  that window; mjmx does not) and honouring percent-decoded paths. CLI gained
  `--allow-includes` (opt back in) and `--include-path`, plus mjml's two stderr warnings
  (tags-found-but-disabled; some-denied). wasm forwards the Option (no FS → no sandbox);
  napi adds `includePath`. See the include-sandbox tests in
  `crates/mjmx/tests/include_sandbox_v5.rs`.
- **Component attrs**: `mj-social-element` gains a `border` on the icon `<img>` (default
  `0`, forwardable from `<mj-social border>`); `mj-accordion` retargets its toggle
  selectors for the new Outlook (`input.mj-accordion-checkbox` →
  `.mj-accordion-checkbox[type="checkbox"]`, `:checked` rules drop `input`; `.moz-text-html`
  fallbacks unchanged), via a `HEAD_CSS_V5` const.
- **Outlook ghost width rounding** (`components/column.rs`, found 2026-07-27): mjml 5.4.0
  wraps `getWidthAsPixel` in `normalizePxValue` (`Math.round`), 4.18.0 does not — so
  `<mj-column width="58.33%">` in a 552px box emits `width:322px` in the ghost `<td>`
  under V5 and `width:321.9816px` under V4. `ColumnWidth::as_pixel_emitted` (compat-aware)
  is used at the three emission sites in `section.rs`/`group.rs`; plain `as_pixel` stays
  **unrounded** because 5.4.0's `getChildContext` recomputes from `parsedWidth` without
  the rounding — a child is laid out against the unrounded width in *both* releases
  (verified: an `mj-image` under those columns emits `width="271"`/`"180"` in 4.18.0 and
  5.4.0 alike). Folding the rounding into `as_pixel` would therefore be wrong. Found by
  the `generated_subset_*` fixtures below — no prior fixture had a fractional column
  width, so this diverged silently.
- **Section `gutter`** (`components/{column,section,group}.rs`): the mjml 5 column-layout
  rework, **shipped** (was deferred). `<mj-section gutter>` threads `gutter` + the
  section's `direction` to each column (mjml's `context.gutter`/`direction`); a column
  reduces its width by `gutter*(sibling-1)/sibling` (so the `mj-column-per-*` class keys
  off the *reduced* width), registers a per-position `mj-column-gutter-{sibling}-{index+1}-
  {unit}-{token}{-rtl}` padding media query (section columns only), and grows padding:
  vertical `%` on the stacking div, horizontal desktop padding on a group column's div,
  and px padding on its Outlook ghost `<td>`. RTL swaps the leading/trailing sides and
  adds the `-rtl` token. px gutters distribute the fractional remainder one pixel at a
  time (lowest index first). Group columns inherit the section gutter but the *group's*
  own `direction`, carry the padding inline (no media query), and take the reduced width
  as their inline mobile width. The whole feature is `is_v5()`-gated on a non-empty
  `gutter` — a V4 render (or any section without `gutter`) is byte-identical to before;
  `Column`/`Group` take a `Gutter` context that is inert (`Gutter::none()`) off the gutter
  path. mjml's dead `gap` context key (never in `allowedAttributes`) is skipped. Verified
  against real `mjml@5.4.0` output: golden fixtures `section_gutter_{px,3col,pct,rtl,
  px_widths}` + `group_gutter` in `tests/golden_v5.rs` (a new V5 golden harness, beautified
  goldens like the V4 set), plus targeted token asserts in `compat_v5.rs`.

**Verified non-changes / deliberate scope cuts** (checked 4.18.0 vs 5.4.0 component
source, not the guide):

- **button `border-radius`** was already `'string'` in 4.18.0 — *not* a 4→5 change, so
  nothing shipped. (The upgrade guide implied otherwise.)
- **`mj-section gutter`** — the whole mjml 5 **gap/gutter/direction + RTL** column-layout
  rework — is now **shipped**, no longer deferred. See the `Section gutter` bullet in the
  "What V5 changes" list above for the implementation. (It has no baseline impact: a plain
  multi-column doc still emits `mj-column-per-50` in 5.4, identical to mjmx.)
- The guide's claim that 5.4 "removes background-color from the body" is **false** against
  real output (5.4 keeps it on both `<body>` and the wrapper div); mjmx followed the
  output.

**Bindings stay synchronous — deliberate.** mjml 5's `mjml2html` became `async` (it
returns a Promise). mjmx's core render is synchronous and does no awaitable I/O in the
render path, so the napi and wasm bindings stay sync: there is nothing to await, and async
wrappers would only add ceremony. Revisit only if an async loader ever enters the render
path.

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

**Second hard invariant:** the inliner must never descend into templating syntax
either. EJS `<% %>` and Handlebars `{{ }}` are masked before tokenizing, always — juice
does this and mjmx did not, which was a real bug (see the `juiceOptions` section).

**Hard invariant:** the inliner must never descend into conditional comments. mjmx merges
Outlook conditionals inside `render_document`, i.e. *before* the inliner, where upstream
runs juice first; the two orders agree only because juice never sees inside `<!--[if …]>`.
`fixtures/mj_style_conditional_order` is the regression test.

`render_with_sourcemap` **refuses** a document with an inline bucket (inlining shifts every
recorded offset). The refusal lives in codegen's `sourcemap_refusal`, after semantic
analysis, because it is a property of the document rather than of the options.

**Structural/functional pseudo-classes are supported** (2026-07-24): an inline `<mj-style>`
rule keyed on `:first-child`/`:last-child`/`:only-child`/`:nth-child`/`:nth-last-child`, the
four `-of-type` variants, `:empty`, `:root`, or `:not`/`:is`/`:where` inlines onto the same
elements real juice would (verified against juice output; `mjmx-css-inline/src/selector.rs`).
The `Tree` gained a `has_content` flag (for `:empty`, whitespace counts as content, per
Level 3) and `following_siblings` (for the `-last-` / `:last-child` family). **Still
dropped** (parse returns `None`, rule vanishes — never mis-applied): state-dependent pseudos
(`:hover`/`:active`/`:focus`/`:visited`/`:link`), any unknown pseudo, and `::before`/`::after`
(handled by the pseudo-element pass). **Deliberate divergence toward correctness:** the
*list* form `:is(.a, .b)` matches nothing in the old css-select juice ships (a fixed bug);
mjmx does the standard thing (`:is`/`:where` = any matches, `:not` = none). Single-argument
`:not`/`:is`/`:where` — the realistic case — matches juice exactly. **Known gap remaining:**
CSS Nesting (nested qualified rules), dropped at the stylesheet level (`css.rs`).

## `<mj-html-attributes>` (`mjmx-css-inline::apply_html_attributes`, done)

mjml's `<mj-head><mj-html-attributes><mj-selector path="…"><mj-html-attribute name="…">v`
collects `(selector, name, value)` triples (`globalData.htmlAttributes`) and, in a
cheerio pass over the assembled body, adds `name="value"` to every element the CSS `path`
matches. mjmx implements it as a **second post-codegen pass, a sibling of the inliner** in
`mjmx-css-inline` — same shape (match selectors, mutate matched elements) so it reuses the
inliner's `dom.rs` + `selector.rs` + `mjmx-html-tokenizer` rather than a third matcher; it
lives there, not in a new crate, for that reuse. `MjSelector`/`MjHtmlAttribute` were added
to the tag enum; semantic ingests the head structure into `HeadData::html_attributes`
(mjml's object-keyed merge: a repeated `path` merges, a repeated `name` keeps its last
value); codegen hands the rules back as data (like the inline bucket); the umbrella applies
them in `render_source`.

Behaviour is read off mjml 4.18.0 output, not guessed:

- runs **after codegen, before the CSS inliner** — mjml runs `htmlAttributes` before
  `juice`, so an added `class` can be a selector target for the inliner that follows
  (verified byte-identical to mjml, incl. the case where the div's pre-existing `color`
  wins the cascade over the inline rule);
- an existing attribute of the same name is **overwritten in place**; a new one is
  appended after the last attribute; a selector matching nothing is a silent no-op;
- it **never descends into Outlook conditional comments** (the ghost tables inside
  `<!--[if mso | IE]>` are comment text to the tokenizer, so a `table` selector reaches
  only the real tables — exactly what mjml's cheerio, fed the commented markup, does);
- it runs on body content only (mjml's cheerio `content` is body-inner), so mjmx gates on
  the inliner's `inlinable` flag plus the structural `html`/`body` tags;
- **not** masked for templating, unlike the inliner — mjml's cheerio pass does not mask
  either (only juice's `codeBlocks` do);
- a selector mjmx cannot parse is dropped, the same known gap the inliner's selector
  engine has.

Like the inliner, this pass shifts byte offsets, so `render_with_sourcemap` **refuses** a
document carrying `<mj-html-attributes>` (codegen's `sourcemap_refusal`, alongside the
inline-bucket and minify/beautify refusals). No new third-party dep; wasm artifact grew
+6,498 B (461,001 B at the time — see the performance section for the current figure).

## Options parity (audit done 2026-07-22 against mjml 4.18.0 source)

The defaults below are the **V4** values unless a cell says otherwise; where 5.4.0
differs, `compat` decides (see the mjml-version section above). `RenderOptions`
(`mjmx-codegen/src/context.rs`) is the single option carrier, threaded to CLI, wasm and
napi. Tier 1 is **shipped**:

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
| `minify_options: MinifyOptions` | `minifyOptions` | `{}` (V5: `for_mjml5()`) | only read when `minify`; `MinifyOptions` carries a `CssDialect` (V4 cleancss, V5 cssnano) — see the mjml-version section |
| `ignore_includes: Option<bool>` | `ignoreIncludes` | `None` → compat (V5 true, V4 false) | `None` follows compat via `resolve_ignore_includes()`; `Some(_)` pins. Still **drops** `<mj-include>`, wins over `file_path`. mjml 5.4.0 flipped the default — see the mjml-version section |
| `include_path: Vec<PathBuf>` | `includePath` | `[]` | **V5 only.** Extra allowed roots for the include sandbox; relative entries resolve against the base. Ignored under V4 (no sandbox) |
| `printer_support: bool` | `printerSupport` | false | third `<style>` replaying the base column rules under `@media only print`, after the screen + Thunderbird blocks; suppressed when there are no column rules, as upstream |
| `inline_options: InlineOptions` | `juiceOptions` **+** `juicePreserveTags` | juice's, as mjml sets them | one carrier for both, since there is no juice to hand two objects to; `preserve_tags` is `juicePreserveTags`. See "juiceOptions" below |

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
`--ignore-includes`, `--printer-support`, `--juice-options <JSON>`,
`--juice-preserve-tags <JSON>`.

### `juiceOptions` / `juicePreserveTags` (shipped)

mjml forwards these to juice (`mjml-core/src/index.js:382-396`, with `...juiceOptions`
spread **last** — so upstream lets a caller override even mjml's own `extraCss`). mjmx
has no juice, so every option was classified rather than forwarded:

| juice option | mjmx | why |
| --- | --- | --- |
| `applyWidthAttributes` (true) | **implemented** | was unconditional; now switchable |
| `applyHeightAttributes` (true) | **implemented** | same |
| `applyAttributesTableElements` (true) | **implemented** | same |
| `preserveImportant` (false) | **implemented** | default unchanged; `true` writes `x: y !important`. Mirrors never carry the keyword, as in juice |
| `styleAttributeName` ("style") | **implemented** | juice reads *and* writes the same name; two literals in `apply.rs` |
| `codeBlocks` | **implemented**, as `juicePreserveTags` | see below |
| `extraCss`, `applyStyleTags`, `removeStyleTags`, `insertPreservedExtraCss` | **refused** | not settings in mjmx but the shape of the pass: semantic splits `<mj-style>` into an inline bucket and a `<style>` bucket, and the inliner only ever sees the first. `extraCss` *is* the `css` argument |
| `preserveMediaQueries`, `preserveFontFaces`, `preserveKeyFrames`, `preservePseudos` | **refused** | unreachable — juice reads them only from `getPreservedText`, gated on `insertPreservedExtraCss` / `removeStyleTags`, and mjml sets both false |
| `xmlMode`, `url`, `webResources` | **refused** | no analogue; `url`/`webResources` are `juiceResources`-only and mjml calls plain `juice()`. The inliner does no I/O |
| `resolveCSSVariables` (**true**) | **implemented** | `apply.rs`: per-element custom-property maps (matched-rule + own `style`, filtered to `--*`); `var(--x)`/`var(--x, fallback)` resolved up the ancestor chain (nearest wins, element first), nested refs re-scanned, `--x` declarations dropped; resolution runs before attribute mirroring. Default `true` (juice's), **both compat modes** — it is plain-juice behaviour mjml always had, not a 5.x change. Only matched elements are rewritten (the crate's existing invariant); every real `var()` reaches output through a matched inline `<mj-style>` rule, so that covers the realistic cases. Verified against real mjml output |
| `inlinePseudoElements` (falsy) | **implemented** | `pseudo.rs`: `::before`/`::after` synthesized from `content` — quoted strings, `url()`→`<img>`, `attr()`, `var()`, `counter()` with a `counter-reset`/`counter-increment` subsystem (decimal + roman/latin styles, all a faithful port of juice 10.0.1). The box is a `<span>` (or `<img>`) carrying the rule's other declarations, prepended/appended by splicing at new DOM content-start/content-end offsets. Default **false** (juice's; mjml never enables it), so a normal render is byte-identical. Verified against juice's own output |

Refusals are **errors with the reason**, never silent ignores — `mjmx_css_inline::
unsupported_option` is the single source of those strings, shared by CLI and wasm.
**Exception, deliberate:** napi's `#[napi(object)]` decodes field by field, so an
unsupported key is dropped before mjmx sees it; there the refusal is the closed
TypeScript interface in the hand-written `index.d.ts` (compile-time only). Same
limitation `minifyOptions` already had in that binding; wasm throws properly.

**Latent bug found and fixed by this pass.** juice masks its `codeBlocks` — EJS
`<% %>` and Handlebars `{{ }}` — **on every call**, before cheerio parses, whether or
not `juicePreserveTags` was passed. mjmx's inliner did not, so a document with an
inline `<mj-style>` mangled templating syntax: `{{ "<div class='a'>" }}` tokenized as
a real tag, and the inliner spliced `style="…"` into the middle of the Handlebars
string literal (verified against mjml 4.18.0). `mjmx-css-inline`'s `mask.rs` now does
it. Regression tests: `fixtures/mj_style_templating` + crate unit tests.

The mask is **byte-length preserving** (a run of inert ASCII), unlike juice's
`JUICE_CODE_BLOCK_<n>_`: the inliner addresses the document by byte offset, so a
length-changing mask would invalidate every offset. The tree is built from the masked
copy and the edits splice into the *original*, which restores the masked bytes for
free — no decode pass. The one leak path, an existing `style` attribute whose value
holds a code block, is closed by reading that value back through its span in the
original (`apply::attr_value`).

**Divergence from upstream, deliberate:** `juicePreserveTags` mutates the module-global
`juice.codeBlocks` object upstream, so registered tags leak into every later render in
the process. mjmx's are per-render data with no global state. (Also: juice interpolates
`start`/`end` into a `RegExp`, so they are regex source; mjmx treats them as literals,
which is what the documented `"<#"` / `"</#"` examples mean.)

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

### `sanitizeStyles` / `templateSyntax` / `allowMixedSyntax` (shipped, mjml 5.4.0)

mjml 5.4.0's protection for **template syntax embedded in CSS** — `color: {{brand}}`,
`{{prop}}: red`, `{{#if}}…{{/if}}` — from the CSS-minify pass, which would otherwise
corrupt it (mjmx's `cssparser` mangles `{{brand}}`; upstream's `cssnano`/`postcss` throws
outright). A **real bug** in mjmx before this: `--minify` on such a document silently
truncated the CSS (`.x{color:{{brandColor}` — *worse* than mjml, which at least errored).
Three `RenderOptions` fields (`mjmx-codegen/src/context.rs`), **available in both compat
modes** (the corruption exists in both), threaded to CLI/wasm/napi:

- `sanitize_styles: bool` (default false) — mjml's `sanitizeStyles`. CLI `--sanitize-styles`.
- `template_syntax: Option<Vec<(String,String)>>` (default `None` → `{{ }}` + `[[ ]]`) —
  mjml's `templateSyntax`, literal delimiter pairs (mjml regex-interpolates; mjmx does not,
  matching the documented `"<#"`/`"#>"` examples). CLI `--template-syntax <JSON array>`.
- `allow_mixed_syntax: bool` (default false) — mjml's `allowMixedSyntax`. CLI `--allow-mixed-syntax`.

The pass is a **faithful port of mjml 5.4.0's `sanitizeTemplateVariablesInHtml`/`restore`**
(`mjmx-minifier/src/sanitize.rs`, ~450 LoC, **no new dep** — literal `prefix`/`suffix`
scanning over the two CSS scopes mjml's regexes target: `style="…"` values and
`<style>…</style>` bodies). Three placement contexts, masked before the CSS pass and
restored after, exactly as mjml: **value** (`{{x}}` → bare ident `variable_temp_N`),
**property** (`{{x}}:` → custom prop `--mj-prop-temp_N:`), **block** (else → fake decl
`sanitizedI:inner;`). Two guards, also mjml's: **unbalanced** delimiters → error;
**mixed** block + value/property syntax → error unless `allow_mixed_syntax`.

Orchestration is in the umbrella's `post_process` (now fallible, `Result<String,
Diagnostic>`), which gates the whole thing on `minify && minify_options.minify_css` — the
**only** transform that parses CSS. Deliberate mjmx divergence, faithful to *mjmx's* actual
behaviour rather than mjml's: **beautify and non-CSS minify never corrupt** (mjmx's
tokenizer emits `<style>` bodies and `style` attrs verbatim), so they need no sanitize and
raise no error, whereas mjml sanitizes its beautify path too (js-beautify reformats CSS).

**Default behaviour when off + template syntax present + CSS minify (chosen, not mjml's):**
mjmx **errors** with an actionable message pointing at `--sanitize-styles`, rather than
mjml's effective `cssnano` throw or the old silent corruption. A no-template-variable
document is byte-identical to a plain minify (the 78 V4 goldens + V5 baseline are unmoved;
sanitize only runs when `detect().any()`). Every masked→minify→restore round-trip was
**verified byte-for-byte against real `mjml@5.4.0`** (`{ minify:true, sanitizeStyles:true,
allowMixedSyntax:true }`), including the `{{pad}} px` space artifact masking introduces.
Tests: `mjmx-minifier/src/sanitize.rs` unit tests + `crates/mjmx/tests/sanitize_styles.rs`.
**Not** ported from 5.x still: nothing in this family — `sanitizeStyles`/`templateSyntax`/
`allowMixedSyntax` are done.

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
- **Both former juice gaps are now implemented:** `resolveCSSVariables` (juice's default
  `true`, both compat modes) and `inlinePseudoElements` (juice's default `false`, opt-in) —
  see their `juiceOptions` table rows. No juice option is refused as a "real gap" any more;
  the remaining refusals are structural (`extraCss`/`applyStyleTags`/… — the shape of
  mjmx's split-bucket pass, not settings) or unreachable-in-mjml (`preserveMediaQueries`/…),
  each still erroring with its reason rather than silently ignored.
- **Inliner table-element mirror divergence — FIXED (2026-07-24).** mjmx used to mirror
  the `TABLE_ATTRIBUTES` set (`background-color`→`bgcolor`, etc.) onto `<table>` **only**;
  real juice (`applyAttributesTableElements`) mirrors onto its whole `tableElements` set.
  `apply.rs` now gates on `is_table_element` (`table`/`th`/`tr`/`td`/`caption`/`colgroup`/
  `col`/`thead`/`tbody`/`tfoot` — juice's list verbatim) and skips a gradient
  `background-image` (juice's `/(linear|radial)-gradient\(/i` guard: no attribute spelling,
  declaration kept in `style`). Verified against real juice output; unit tests
  `table_attributes_mirror_onto_cells_like_juice` + `gradient_background_is_not_mirrored`.
  V4 goldens unmoved (no fixture sets one of these on a cell). Dimension mirror
  (`width`/`height` → `table`/`td`/`th`/`img`) already matched juice's `widthElements`/
  `heightElements` — no gap there.
- **Won't port:** `skeleton`, `preprocessors`, `presets` — JS function / `require()` semantics.
  Also `mjmlConfigPath` / `useMjmlConfigOptions` (was Tier 3): their *primary* purpose is
  loading **custom components** from a `.mjmlconfig` via JS `require()`, which a Rust
  compiler with no JS runtime cannot execute — the same boundary as `preprocessors` /
  `skeleton`. The `options`-merge half alone (reading an `options` block out of
  `.mjmlconfig` and folding it into `RenderOptions`) is not worth a bespoke config reader,
  so the pair is dropped rather than half-implemented.
- Upstream oddities deliberately not copied: `--config.*` booleans don't coerce `"false"`;
  `juicePreserveTags` mutates the global juice module (mjmx's is per-render);
  `...juiceOptions` is spread last, letting a caller delete a document's own inline CSS
  via `extraCss`; mjml-cli's README documents a `-l` flag that does not exist in 4.18.0.

## Custom / unknown components

**mjmx is strict on unknown elements — mjml's model, not mrml's.** An unrecognized tag
(`<mj-foo>`, or a bare `<div>` in the body) is a **fatal error at both lint and render**,
matching mjml ("Element X doesn't exist or is not registered"), which rejects it even under
soft validation. jdrouet/mrml is the lenient outlier: its generic `node` component passes
unknown tags through as literal HTML. For a compiler toolchain with an LSP, strict-with-a-
good-diagnostic is the right call.

A 2026-07-22 diagnostics fix (`render::unsupported`, `mjmx-linter::check_unknown`) made this
consistent: the render error used to read "rendering is not implemented for `<div>` yet …
unsupported component in this build", framing a nonexistent tag as a pending mjmx feature —
false, since every real `MjmlTag` has a renderer, so that branch only ever fired for tags
that don't exist. It now distinguishes `MjmlTag::Unknown` ("not a recognized MJML component")
from a recognized head-only tag misplaced in the body ("not valid inside the body"). The
linter was a **warning** whose help falsely claimed unknown tags "are rendered as-is"; it is
now an **error** with accurate help, so lint and render tell the same story.

**Custom components proper are won't-port via `.mjmlconfig`** (see "Won't port" above) but
mjmx ships an mjmx-owned alternative: **`docs/design/custom-components.md`** — a small,
versioned `ComponentCx` contract (leaf-only first), a native Rust trait plus (future)
napi/wasm/python callbacks over one shared shape, with the cross-binding rule
"registered → renders, else the clean unknown-component error".

**Phases 1–2 are implemented (2026-07-23): native Rust + the napi/wasm JS callbacks.**
Public API on the umbrella: `mjmx::{CustomComponent, ComponentRegistry, ComponentCx,
ComponentLookup}` + `render_with_components` / `render_file_with_components`. A host `impl`s
`CustomComponent` (`tag()` + `render(&mut ComponentCx)`), registers it on a
`ComponentRegistry`, and passes that in; a tag the parser left as `MjmlTag::Unknown` renders
through its component, and only an *unregistered* unknown tag is the usual fatal "not a
recognized MJML component" error (the linter agrees via `mjmx_linter::lint_with_components`,
so a registered tag is not flagged under `strict`; a registered Tier 1 tag *outside*
`<mj-body>` is flagged instead — a body-only leaf). Trait/registry/`ComponentCx` live in
`crates/mjmx-codegen/src/custom.rs`; registry-aware dispatch is the `MjmlTag::Unknown` arm in
`render.rs`; the registry is threaded as `RenderContext.registry: Option<&'a dyn
ComponentLookup>` (no new lifetime — a `&'r` param coerces since `'r: 'a`), and codegen's
`render_document_with_inline_css_and_components` carries it. **ABI = `ComponentCx`**:
`attr`/`attr_or` (cascade-resolved only), `resolved_attrs` (the whole cascade-resolved map,
for a binding snapshot), `container_width`, `breakpoint`, `raw_inner` (verbatim inner text),
`write`, `add_style`, `add_font`. Head contributions are owned side-channels on
`RenderContext` (`add_style` reuses the style buffer; `add_font` → `custom_fonts:
Vec<(String,String)>`, emitted `<link>`+`@import` only when the family is referenced in the
body, mjml's rule) — so a component needs no arena access. Byte-identical to before for any
document with no custom fonts (the 78 golden tests are unmoved). Tests:
`crates/mjmx/tests/custom_components.rs`.

**The author returns cell *content*; mjmx supplies the `<td>` cell** (`render::
render_custom_in_cell`). mjml's `mjml-column`/`mjml-hero` wrap every non-raw child in a
`<td>` in `renderChildren`, reading the cell from that child: `align` + `css-class`
attributes, then a style block of `background` (from `container-background-color`),
`font-size:0px`, the `padding` family, `word-break:break-word` — verified against real
`mjml-column@5.4.0` source (`lib/index.js`, the child cell). A custom component gets the
same cell, so `<tr>` (from the column) + `<td>` (from mjmx) is valid `<tr><td>…</td></tr>`;
without this a bare `<span>` return produced invalid `<tr><span>`. A custom component has
no mjml attribute defaults, so each cell attribute appears **only when the cascade resolves
one** (mjml's `htmlAttributes` drops undefined); with none set the cell is
`<td style="font-size:0px;word-break:break-word;">`. Native and both JS bindings share this
one path (the wrapping is in codegen, before the binding's `cx.write`).

**Phase 2 — JS bindings.** Both `@mjmx-rs/node` and `mjmx-wasm` take a `components:
Record<string, ComponentFn>` render option, `ComponentFn = (cx) => string |
{ html, styles?, fonts? }`. `cx` is a **read-only snapshot** — `attr(name)`, `attrs`,
`containerWidth`, `breakpoint`, `rawInner` — and head contributions come back through the
**return channel** (resolving the Phase-2 half of the head-contributions open question:
mutation for native, return-value for callbacks — a callback holds no live `cx`, and a
return value keeps the JS call one-way, no reentrancy). The registry was generalized to a
`ComponentLookup` trait so a binding supplies a JS-backed registry — napi holds each
callback as a `FunctionRef` and `borrow_back`s it with the `Env` at call time; wasm holds a
`js_sys::Function`. Both call the callback **synchronously on the JS thread** during render
(no threadsafe-function / async — the bindings stay sync, per the mjml-version section's
bindings note). napi's `index.js` wraps each callback to add the `cx.attr(name)` method (a
pure lookup over the plain-data `attrs` it hands the native side, so the native never calls
back into JS); wasm builds `attr` as a `Closure` over the snapshot. The `attrs` snapshot is
built from `Semantic::resolved_attributes` (union of inline attrs + every applicable
`mj-attributes`/`mj-class`/`mj-all` name, each cascade-resolved). Examples in `examples/`
(node-cjs, node-esm, node-ts, browser); napi + wasm smoke tests each gained a custom-
component check.

**Tier 2 container components — native, shipped (2026-07-23).** A custom tag can hold
layout children (`mj-column`, …) and drive them itself: opt in with
`CustomComponent::is_container() -> true`, and mjmx skips the leaf `<td>` cell and calls
`render()`, where `ComponentCx` gains `render_children()` (recurse into mjmx dispatch for
the tag's MJML children at the current width) and `set_container_width(u32)`. The ABI is
the **`render_children` callback** shape (chosen over a themed-section alias or a
pre-rendered-children Vec): the component owns its outer markup and calls back for the
inner children — mjmx supplies **no** section ghost-row / column-share layout, so a bare
`mj-column` under a container renders standalone (full width unless it has an explicit
`width`); the container builds whatever grid it wants. `render_custom_container` (`render.rs`)
save/restores `container_width` so a child width never leaks to a sibling; a registered
container is exempt from the linter's parent-child constraints (passes strict as a body
child). Tests: `tier2_*` in `crates/mjmx/tests/custom_components.rs`. See
`docs/design/custom-components.md`.

**Still unimplemented (deliberately):** the **JS-callback** container (a JS `render_children`
across the FFI boundary — bindings stay Tier 1 leaf-only); the CLI (no JS-callback surface —
a CLI user renders built-ins + any Rust-registered components only, the documented
consequence of the same rule); and Phase 3 python (pyo3), which will reuse the same `cx`
shape + return channel.

## Performance (profiled + optimized 2026-07-29)

**Render throughput improved ~39%, the wasm artifact shrank ~25%.** Measured on an M1,
warm, over the 85 `fixtures/*.mjml`; every change is byte-identical output (the 81 V4 +
9 V5 goldens are the gate) and added no dependency.

| | before | after |
| --- | --- | --- |
| native render (mean over fixtures) | 14.8 µs | **9.0 µs** |
| `make bench` (81-fixture mrml intersection) | 16.0 µs | **10.6 µs** |
| …relative to mrml 6.0.1 | 0.96× | **1.6×** |
| …relative to mjml 5.4.0 | ~20× | **~32×** |
| wasm artifact (post `wasm-opt -Oz`) | 610,524 B | **459,101 B** |
| wasm render (node) | 27.8 µs | **27.5 µs** (at `opt-level="z"`) |
| clean `cargo build --workspace --release` | 70 s / 269 s CPU | **62 s / 233 s CPU** |

Method: `sample(1)` on a render loop over the fixtures, attributing samples to the
nearest mjmx frame. Read the profile before changing anything — two of the seven planned
items were dropped *because* it said they were not worth doing.

What actually paid, in order:

- **`merge_outlook_conditionals` was 13.2% of total runtime** — a single `str::find`.
  `find` with a multi-byte needle builds a fresh two-way searcher **per call**, and
  mjml-shaped output has one `<![endif]-->` per section/column/ghost table, so the setup
  was paid dozens of times per document. Now scans for the marker's `[` (a rare byte, so
  `core`'s SIMD `memchr` path) and confirms the marker at candidate positions —
  `find_endif` in `document.rs`. It also copies in chunks between deletions instead of
  once per marker. **−12.7%.**
- **`push_str(&format!(…))`, 191 sites** — each allocated a temporary `String`, formatted
  into it, copied it into the buffer and freed it. All 191 became `write!` straight into
  the buffer (**−16.4%**), and the hot ones then became plain `push_str` chains
  (**−4.1%**). Two shared helpers came out of it: `push_decl` (`property:value;`, 52
  sites) and `inner_text`/`inner_text_trimmed`, which replaced **seven** copies of the
  same "concatenate the text children" loop — three of which allocated twice.
- **Buffer growth was 23% inclusive** (`RawVecInner::finish_grow`): 44 style accumulators
  growing from zero, plus `body_buf`/`out` with no capacity hint, in a crate with exactly
  one `with_capacity`. Now `body_capacity()` (source length × 8, clamped), `HEAD_CAPACITY`
  and `STYLE_CAPACITY` — each sized from a measurement, not a guess. **−6.5%.**
- **The CSS inliner ran the whole cascade twice per element.** `custom_properties` calls
  `winners` + `merge` for every element just to collect `--x` declarations, and
  `resolveCSSVariables` is on by default — so every inlining document paid double. Now
  gated on `uses_var()`: with no `var(` in the CSS or the HTML, `substitute_vars` is
  provably the identity, so the map cannot be read. The `--x` declarations are still
  dropped, which is a separate juice behaviour. Also, `dom::build` lower-cased every tag
  into a fresh `String`; `Element::tag` is a `Cow` now and borrows when the tag is already
  lower-case (it always is, in generated markup). **−9.4%** overall, ~4.6 µs per document
  that actually inlines.

**Deliberately not done, with the measurement that says why:**

- **The head's per-font body scan** (`head.rs`, 3.6% before the other work). Stubbing the
  check out entirely — while making the render do *more* emission — bought only 1.8%. A
  byte-wise multi-pattern scan would very likely be **slower**: `str::contains` skips
  roughly a needle-length per step, where a scan-every-byte loop cannot.
- **`[profile.dev] debug = "line-tables-only"`.** Clean `cargo test --no-run` went 26.5 s
  → 25.5 s (inside the noise) for 1.3 GB → 1.1 GB of `target`. Not worth losing a
  debugger's variable inspection; the first measurement of this even came out *slower*,
  which is why it is quoted from the second pair of runs.

**`[profile.release-wasm]`** (`opt-level = "z"`, inherited from `release`) exists because
native and wasm want opposite things. On the current code it is 460,277 B vs `3`'s
609,711 B — 24.5% smaller — for a 24% slower wasm render. Net against where this started,
the artifact is 25% smaller *and* marginally faster, because the codegen work more than
paid for the size mode. `panic = "abort"` + fat LTO were measured too: **724 bytes**
together, so they are not set. `make wasm` builds `--profile release-wasm`.

**Build time**: `axum` lost its default features (the dev-server uses only `http1`,
`json`, `query`, `tokio`, `ws` — `form`, `matched-path`, `original-uri` and the `tracing`
integration were dead weight). The remaining clean-build cost is dominated by
`mjmx-language-server` (~40 s) and its `tower-lsp → lsp-types → url → idna → icu` chain,
which cannot shrink without dropping `tower-lsp`.

## Planned next

**Distribution decision (2026-07-28): git-first for Rust, npm for the wasm binding.**
A first `cargo publish --workspace` was attempted and **rolled back**. crates.io throttles
*new* crates to a burst of 5 then roughly one per 10 minutes, so a 24-crate workspace takes
~3 hours of retries; 5 leaf crates (`mjmx-allocator`, `mjmx-html-tokenizer`, `mjmx-index`,
`mjmx-span`, `mjmx-syntax`) went up before the 429 and were **deleted** via crates.io's
self-serve delete (eligible: 0 downloads, no reverse dependencies, minutes old — note
`cargo yank` would *not* have worked, it neither deletes nor frees the name). The Rust side
is therefore consumed by **git dependency**; the known cost is that no crate published on
crates.io can depend on mjmx, and there is no docs.rs page. Revisit by asking
`help@crates.io` to raise the new-crate limit for the workspace, which is the normal remedy.
Publish-readiness work already landed and still stands: correct `repository` URL, keywords,
categories, umbrella README, docs.rs metadata, and a green `cargo publish --workspace
--dry-run`.

1. **Ship as three packages: JS, Rust, Python.**
   - JS: **npm names are `@mjmx-rs/mjmx` (wasm) and `@mjmx-rs/node` (native, +
     `@mjmx-rs/node-<triple>`)**, under the npm org matching the GitHub org. The wasm
     build **is published**; the native one is **not** — it still needs the CI
     cross-build, since only the local triple can be built here and an incomplete set
     breaks installs on every platform but this one. Both dropped `"private": true` and
     gained keywords, `homepage`/`bugs`, and a dual `exports` map.

     **How the name landed there — three failed attempts, each with a lesson:**
     1. `@mjmx/wasm` → `404 Not Found` on `PUT`. **The `@mjmx` scope is someone else's**
        (`@mjmx/core`, a *different* MJML tool — "Write mjml using JSX syntax", mjmx.dev).
        Do not read `npm access list packages @mjmx` as proof of ownership: it reports
        `@mjmx/core: read-write`, which is the *package's* access setting, not this
        account's permission. `npm access list packages` with **no scope** is the
        authoritative list; no `@mjmx/*` appears in it.
     2. Unscoped `mjmx-wasm` → published fine as `0.1.0` (still on npm; npm forbids
        unpublishing after 72h, so that name simply falls behind).
     3. Unscoped `mjmx` → `403 Forbidden`, "**Package name too similar to existing
        packages mjml, mem, mime, mobx**". **`npm view <name>` returning 404 does not
        mean a name is claimable** — it means nobody published it. npm's typosquatting
        filter is a separate server-side gate that only fires on the real `PUT`, and
        there is no way to test it short of publishing. It applies to **unscoped names
        only**, which is the deciding reason for a scope: every future
        `@mjmx-rs/*` package is guaranteed publishable, whereas unscoped `mjmx-node`
        would have been an untested gamble discovered *after* building CI for five
        platform triples.

     Note the **crates** keep their own names (`mjmx-wasm`, `mjmx-napi`) — only the npm
     packages are scoped. Scoped packages need `--access public` on first publish, and
     this account enforces 2FA, so publishing needs `--otp=<code>`.

     The wasm `exports` map also exposes `"./pkg/*"`, without which the documented deep
     import (`@mjmx-rs/mjmx/pkg/web/mjmx_wasm.js` — the only way to embed the wasm bytes
     in a host with no network access, e.g. a Figma plugin) fails with
     `ERR_PACKAGE_PATH_NOT_EXPORTED`: an `exports` field blocks every subpath it does not
     list.
     - **napi** adopts `@napi-rs/cli`: the binary ships as a per-platform **optional
       dependency** `@mjmx-rs/node-<triple>` (5 triples: `linux-x64-gnu`, `linux-arm64-gnu`,
       `darwin-x64`, `darwin-arm64`, `win32-x64-msvc`), scaffolded under
       `crates/mjmx-napi/npm/<triple>/` by `napi create-npm-dirs`. The hand-written
       `index.js` loader (kept for its diagnostics enrichment) now `require`s the matching
       optional dep first, then falls back to a locally built binary; `index.mjs` adds the
       ESM surface. `npm/<triple>/package.json` is committed as scaffolding; the `.node`
       binaries stay gitignored (`*.node`), only the local `linux-x64-gnu` one is staged.
       Other triples build on their own platform (no cross-compile / CI yet).
     - **wasm** is now multi-target: one cargo build feeds three `wasm-bindgen` passes —
       `nodejs` (CJS) → `pkg/node/`, `bundler` (ESM) → `pkg/bundler/`, `web` (ESM) →
       `pkg/web/` — wired to the `node` / `browser` / `import` `exports` conditions.
       `"type": "module"` was deliberately **not** set (it would reinterpret the CJS node
       glue as ESM and break `require`).
     - The `crates/mjmx-{napi,wasm}/test/smoke.mjs` tests were stale against the V5-default
       flip (a stale staged `.node` had hidden it): they assumed V4 include-resolution and
       the V4 `style=""` skeleton. Fixed by pinning the affected checks to `mjmlVersion: 4`
       or opting in with `ignoreIncludes: false`; `make napi-test` / `make wasm-test` green.
     - **Still open:** a CI release workflow (`@napi-rs/cli`'s GitHub Actions flow) to
       cross-build every triple and `npm publish` `@mjmx-rs/node`. The wasm package is published and
       needs no cross-build (one portable artifact), which is exactly why it shipped first.
   - Rust: the umbrella's published-quality metadata is **done** (docs.rs metadata,
     crate-level docs, keywords, categories, crate README) but the crate itself is **not on
     crates.io** — see the distribution decision above; consumed by git dependency.
   - Python: **new binding** (pyo3/maturin). This reverses the earlier "wasm + napi only, no
     python" decision recorded under Prior art.
2. **CLI parity audit — done for 4.18.0.** Walked mjml 4.18.0's `mjml-cli/src/client.js`
   (NOT master — see below) and closed the render-path gaps:
   - `-w`/`--watch` (`apps/mjmx-cli/src/watch.rs`): recompiles the file on every change
     until Ctrl-C. Matches mjml: watch output is **always a file, never stdout** — `-o` if
     given, else `<stem>.html` (basename, `.mjml`→`.html`) in the **cwd**, not beside the
     source. Requires a file input (stdin unwatchable; clap rejects `--watch --stdin` and
     `--watch --stdout`). Per-rebuild status to **stderr**. Reuses the `notify` crate with
     a minimal copy of the dev-server watcher's three anti-self-feed guards (event kind /
     path / re-arm); the `.html` output can't match the `.mjml` path filter, so no output
     fingerprint is needed. Folder-watch is **not** ported (mjmx's model is single-file;
     the initial build is an intentional, friendlier divergence — mjml compiles on change
     only).
   - Stdout file comment (`RenderArgs::stdout_file_comment`): mjml prepends
     `<!-- FILE: {path} -->\n` before the HTML when writing to **stdout** for a **file**
     input — the path *as given* (not basename), verified byte-for-byte against
     `npx mjml@4.18.0`. `--no-stdout-file-comment` (alias `--noStdoutFileComment`)
     suppresses it. Never present with `-o` (file write); suppressed for stdin (mjml prints
     a nonsensical `<!-- FILE: undefined -->` there — mjmx omits it, a deliberate fix).
   - Compat alias flags `-r`/`--read`, `-i`/`--stdin`, `-s`/`--stdout`: name the input/output
     modes that are otherwise mjmx's defaults, for drop-in familiarity. Idiomatic clap
     conflicts mirror mjml's "too many input/output arguments" errors: `-r`/`-i`/positional
     are mutually exclusive; `-s`/`-o` are mutually exclusive.

   Earlier passes added `dev --breakpoint` and `fmt`/`migrate --indent-width`. The
   `--config.*` options stay exposed as flat idiomatic flags (a locked decision, not a gap).

   **Now retargeted to 5.4.0 (default), 4.18.0 opt-in** — see the mjml-version section. The
   render-path CLI audit above targeted 4.18.0 (`-r`/`-i`/`-s`, watch, the FILE comment) and
   still holds for both compat modes. The 5.4.0-specific CLI additions shipped since:
   `--mjml-version 4|5`, and the include changes `--allow-includes` / `--include-path` with
   mjml 5's two stderr warnings, plus the `sanitizeStyles` family (`--sanitize-styles` /
   `--template-syntax` / `--allow-mixed-syntax`) — see the `sanitizeStyles` section below.

Method reminder: read mjml's actual README/**output** rather than guessing — that has caught
real bugs every single time, including twice during the 5.x retarget (the upgrade guide was
wrong about `border-radius` and about body `background-color`).

## Project rules

Non-negotiable. These are enforced locally, not by a service — the repo has **no CI**.

- **No AI co-authorship on commits.** A commit's author and its message are the user's.
  Never add a `Co-Authored-By: Claude …` trailer, a `🤖 Generated with Claude Code` line,
  or any other agent attribution — not in commits, not in PR descriptions, not in tags.
  Write the message as a plain conventional commit describing the change.
- **Full test suite before every push.** `make preflight` (= `lint test napi-test
  wasm-test`) must pass. It is the pre-push gate precisely *because* there is no CI: a
  green `cargo test --workspace` alone is not enough, since the napi and wasm smoke tests
  live outside it and have gone stale before (see the V5-default flip under "Planned
  next"). Install the hook once per clone with `make hooks`
  (`scripts/pre-push` → `.git/hooks/pre-push`; git does not track `.git/hooks`).
  `git push --no-verify` bypasses it — emergencies only, and say so.
  The wasm leg is a **hard requirement, never skipped**: `make wasm-tools` checks the
  rustup target (declared in `rust-toolchain.toml`, so rustup installs it automatically)
  and that the `wasm-bindgen` CLI matches the workspace's `=0.2.100` pin exactly — the
  pin is read out of `Cargo.toml` by the Makefile, never duplicated — each failure
  carrying its install command. One-time per machine:
  `cargo install wasm-bindgen-cli --version 0.2.100`. (`wasm-opt`/binaryen stays
  optional: it only shrinks the artifact and already no-ops when absent — but note
  `make wasm-release` *does* require it, and the size report in `make wasm-opt` had a
  macOS-only bug until 2026-07-28: BSD `wc -c < file` pads its output with leading
  spaces, so the unquoted `awk -v b=$$before` word-split and awk read `-v` as a filename.
  It works on Linux, where GNU `wc` does not pad. Quote shell values in recipes.)
- **Cover all the codebase in tests.** Every crate carries tests; every new code path and
  every fixed bug lands with a test in the same change. `make coverage` (needs
  `cargo install cargo-llvm-cov`) prints per-crate line coverage, `make coverage-html`
  the browsable report. There is deliberately **no failing threshold** — the number is
  there to find gaps, not to gate work — so uncovered code is a review finding, not a
  build error. Prefer the existing shapes: golden `.mjml`→`.html` fixtures compared
  semantically for render behaviour, crate-level unit tests for passes.

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
were originally **wasm + napi only**; that "no python" decision is **reversed** — a pyo3
python binding is now planned (see "Planned next"). Adopting mrml's python binding as prior
art there.
