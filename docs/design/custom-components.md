# Design: custom components (mjmx extension points)

Status: **Phases 1–2 + native Tier 2 implemented 2026-07-23** — native Rust (Tier 1 leaf
*and* Tier 2 container) *and* the napi + wasm JS callbacks (Tier 1 leaf). The JS-callback
container and Phase 3 (python) remain proposal.
Author-reviewed 2026-07-22.

The JS surface (`mjmx-node`, `mjmx-wasm`): a `components: Record<string, ComponentFn>`
render option, where `ComponentFn = (cx: ComponentCx) => string | ComponentResult`. `cx`
is a **read-only snapshot** — `attr(name)` / `attrs` (cascade-resolved), `containerWidth`,
`breakpoint`, `rawInner` — and head contributions come back through the **return channel**
(`ComponentResult { html, styles?, fonts? }`), *not* by mutating `cx`. That answers the
Phase-2 half of the "head contributions" open question below (mutation for native,
return-channel for the callbacks — a callback holds no live `cx` to mutate, and a return
value avoids any reentrant call back into the renderer). Both bindings build the snapshot
from `ComponentCx::resolved_attrs()` and route through
`render_with_components`; the registry generalized to a `ComponentLookup` trait so a
binding supplies a JS-backed registry (napi via `FunctionRef` + `borrow_back`, wasm via a
held `js_sys::Function`) without re-implementing it. Both call the JS synchronously on the
JS thread during render — no threadsafe-function machinery — which is sound because the
callback only reads its snapshot (no reentrancy).

The Phase-1 native surface: `mjmx::{CustomComponent, ComponentRegistry, ComponentCx}`
and `mjmx::render_with_components` / `render_file_with_components` (the trait, registry and
`ComponentCx` live in `crates/mjmx-codegen/src/custom.rs`; registry-aware dispatch in
`render.rs`, linter in `mjmx-linter` via `lint_with_components`). See
`crates/mjmx/tests/custom_components.rs`.

## Problem

mjml lets a document use third-party components declared in `.mjmlconfig`:

```json
{ "packages": ["./my-badge-component.js"], "options": { "keepComments": false } }
```

Each `packages` entry is a JS module `require()`d at render time, exporting a
class that `extends BodyComponent from 'mjml-core'` with a `render()` method.
mjmx cannot load these — a Rust compiler has no JS runtime, and even with one, a
package is glued to `mjml-core`'s base classes, context object, component
registry and head-registration hooks. Honoring one unmodified means reproducing
`mjml-core`'s internal plugin ABI bug-for-bug — which is exactly what mjmx exists
*not* to be (a clean-room compiler, not a `mjml-core` reimplementation). And once
`mjml-core` is loaded to drive those components, mjmx *is* mjml for those
subtrees, losing its speed, spans, diagnostics and LSP mapping there.

Prior art confirms the wall: **jdrouet/mrml also won't-ports `.mjmlconfig`** — no
config reader, no custom-component loading, a fixed Rust component set. Its only
adjacent feature is a generic `node` passthrough that renders unknown tags as
literal HTML (`<mj-foo>` stays `<mj-foo>`), not a component system. mjml itself is
strict: an unknown element is a fatal error. mjmx matches mjml's strictness (see
`crates/mjmx-linter` + `render::unsupported`).

So `.mjmlconfig` packages are **won't-port** (recorded in CLAUDE.md alongside
`preprocessors` / `skeleton`). This document proposes the honest alternative:
**mjmx-owned extension points** — custom rendering with a contract mjmx defines,
versions and documents, instead of chasing mjml's internals.

## Non-goals

- Loading or executing `.mjmlconfig` `packages`. A component written against
  `mjml-core` will **not** run here; it must be rewritten against mjmx's context.
- Bug-for-bug compatibility with `mjml-core`'s component base classes.
- Custom components that participate in the responsive column / ghost-table
  **layout** machinery (custom tag containing `mj-column`s). Deferred — see
  "Tier 2" — because the layout contract is the expensive, unstable surface.

## Scope: leaf-only (Tier 1)

A custom component is an **ending / raw tag**: it receives its resolved
attributes and its inner source text, and returns an HTML fragment. It does not
recurse into mjmx's dispatch and does not drive layout scaffolding. This covers
the overwhelming majority of real custom components (branded button, badge,
social widget, divider variant) while keeping the ABI small enough to keep
stable across versions.

**The author returns cell *content*, not a cell.** mjmx wraps the returned
fragment in the standard leaf `<td>` cell — exactly the cell mjml's
`mjml-column`/`mjml-hero` emit around every non-raw child (`renderChildren`),
read from *this* component's attributes: `align` and `css-class` as attributes,
then a style block of `background` (from `container-background-color`),
`font-size:0px`, the `padding` family, and `word-break:break-word`. So the column's
`<tr>` and mjmx's `<td>` yield valid `<tr><td>…</td></tr>` markup. Unlike a
built-in, a custom component has no mjml-declared attribute defaults, so every cell
attribute is emitted **only when the cascade resolves one** (mjml's `htmlAttributes`
drops `null`/`undefined`); a component with no cell attributes gets
`<td style="font-size:0px;word-break:break-word;">`. Implemented in
`render::render_custom_in_cell`.

## The ABI: `ComponentCx`

One stable, versioned, documented slice of the render context. This is the entire
surface a component author sees.

```
attr(name) -> Option<str>      // resolved through mj-attributes / mj-class / defaults cascade
attr_or(name, default) -> str
container_width() -> u32       // px available for this element, set by the parent
breakpoint() -> u32
raw_inner() -> str             // verbatim source text between the tag's open/close
write(html)                    // append to the output buffer
add_font(name, href)           // head contribution: <link>/@import if referenced
add_style(css)                 // head contribution: a <style> block
```

Deliberately absent: any `mjml-core` base class, `renderChildren`, the component
registry, the global context. A component gets attributes in, writes HTML out, and
may contribute to `<head>`. Nothing else.

### Native (Rust, compiled-in)

```rust
pub trait CustomComponent {
    fn tag(&self) -> &str;                                   // "mj-badge"
    fn render(&self, cx: &mut ComponentCx<'_>) -> Result<(), Diagnostic>;
}

let reg = ComponentRegistry::new().with(MyBadge);
mjmx::render_with_components(src, &opts, &reg)?;
```

`ComponentCx` is a public wrapper over the existing `RenderContext`
(`crates/mjmx-codegen/src/context.rs`), exposing only the ABI above.

### napi / wasm (JS host)

```ts
render(source, {
  components: {
    "mj-badge": (cx) => `<span style="...">${cx.attr("label") ?? ""}</span>`,
  },
});

interface ComponentCx {
  attr(name: string): string | undefined;
  containerWidth: number;
  breakpoint: number;
  rawInner: string;
  addFont(name: string, href: string): void;
  addStyle(css: string): void;
}
```

When codegen reaches a registered tag it calls back into the host — napi via a
threadsafe function, wasm via an imported function — passes a `cx`, receives the
HTML string, and splices it. The `cx` shape is **identical** across napi, wasm and
(future) python, so the bindings never diverge.

### python (pyo3)

Same `cx` shape as a Python object, when the python binding lands. Deferred to
that milestone; listed here so the ABI is designed for it from the start.

## Why this beats the `.mjmlconfig` route

| aspect | `.mjmlconfig` packages | mjmx extension points |
| --- | --- | --- |
| ABI | `mjml-core`'s internal plugin contract | mjmx's own, versioned |
| runtime needed | a JS engine on every surface | native = Rust; host bindings = a callback |
| mjmx's advantages | lost — JS renders the subtree | kept — mjmx renders, calls out for one fragment |
| cross-binding behavior | napi-only → silent divergence | one rule everywhere (below) |

**The cross-binding rule is the payoff.** A document containing `<mj-badge>`
renders if the host registered a `mj-badge` component, and otherwise produces the
existing clean "`<mj-badge>` is not a recognized MJML component" error. Divergence
is explained by *"did you register the component"* — not by *"which binding"*. Same
input, same rule, every surface. This preserves the same-input/same-output
property defended throughout the options work (cf. the wasm `filePath` refusal).

A plain CLI user cannot inject a JS callback; that surface renders built-ins plus
any Rust-registered components only. That is a documented, consistent consequence
of the same rule, not a special case.

## Interactions this forces

1. **Linter must consult the registry.** The "unknown component" diagnostic
   (`crates/mjmx-linter/src/lib.rs`, `check_unknown`) currently errors on any
   `MjmlTag::Unknown`. It must become "unknown **and unregistered**": `lint` and
   `render` take the set of registered component names so a registered custom tag
   is recognized at both stages.
2. **Parser / tag handling.** A registered `<mj-badge>` still parses to
   `MjmlTag::Unknown` with `raw_tag = "mj-badge"`; the registry lookup happens at
   dispatch (`render::dispatch`) and lint time, keyed on `raw_tag`. No new tag
   enum variants — the enum stays the fixed known-MJML set.
3. **Sourcemap.** Custom output is opaque HTML: the map records the tag's span but
   nothing inside it, exactly as for any leaf. No new refusal needed (unlike CSS
   inlining, this does not rewrite existing offsets — it only fills a tag's own
   span).
4. **CSS inlining / html-attributes.** Both run after codegen over the assembled
   document, so custom output participates normally with no special-casing.
5. **Trust boundary.** A JS/python callback is arbitrary host code — same trust as
   any plugin; document it. The native trait is safe Rust. Neither reaches the
   filesystem or network through `cx`.

## Honest cost

Porting a real mjml custom component means **rewriting its `render()` against
`ComponentCx`** — it will not drop in. That is the intended cost: a small stable
ABI mjmx owns, rather than tracking `mjml-core`'s evolving internals.

## Tier 2 (native, implemented 2026-07-23): container components

A custom tag may **contain** layout children (`mj-column`, `mj-section`, another
custom component, …) and drive their placement itself. A component opts in with
`is_container() -> true`; mjmx then emits **no** wrapping `<td>` cell (unlike a
Tier 1 leaf) and calls its `render()`, where it uses two new `ComponentCx`
methods:

```
render_children() -> Result<(), Diagnostic>   // render this tag's MJML children through
                                              // mjmx's normal dispatch, at the current width
set_container_width(width: u32)               // width the children size against; mjmx
                                              // restores the parent's width afterwards
```

The chosen ABI is the **`render_children` callback** (over a "themed section
alias" or a "pre-rendered children Vec"): the component controls its own outer
markup and calls back into dispatch for the inner children. Deliberately, mjmx
supplies **no** section ghost-row / column-share layout around them — a bare
`mj-column` renders standalone (full width unless it carries an explicit `width`),
so the container owns whatever grid it builds (its own ghost tables, widths, etc.).
That keeps mjmx's expensive layout internals *out* of the ABI while still letting a
container recurse. `render_custom_container` in `render.rs` brackets the call to
save/restore `container_width`, so a width a container sets for its children never
leaks to a sibling. A registered container is exempt from the linter's
parent-child structural constraints (it is a known, free-form tag), so it passes
strict validation as a direct `mj-body` child. Tests: the `tier2_*` cases in
`crates/mjmx/tests/custom_components.rs`.

**Still native-only.** The JS-callback container (a JS `render_children` reaching
back across the FFI boundary) is a larger surface; the JS bindings remain Tier 1
leaf-only for now, the same native-first ordering Tier 1 followed.

## Phasing

1. **Native Rust trait + `ComponentCx` + registry-aware dispatch/linter.** Proves
   the ABI with zero binding complexity. Small.
2. **napi + wasm callbacks** over the same `cx`. Medium — threadsafe-function /
   imported-function plumbing, done once against a shared shape.
3. **python (pyo3)** — same `cx`, at that binding's milestone.

Native-first locks the ABI before callback plumbing is paid for three times. Phases 1 and
2 are now done; Phase 3 (python) reuses the same `cx` shape and return channel.

## Open questions — resolved

- **Head contributions.** *Decided:* native via `cx` mutation, JS callbacks via a **return
  channel**. `add_style` / `add_font` push onto the `RenderContext` (styles reuse the
  existing dedup buffer; fonts land in an owned `custom_fonts: Vec<(String, String)>`
  side-channel, merged into the head font pass and emitted — `<link>` + `@import` — only
  when the family is referenced in the body, mjml's own rule). Owned rather than
  arena-bound so a callback need not thread the allocator. For napi/wasm the callback
  returns `{ html, styles?, fonts? }` rather than mutating a live `cx`: a callback holds no
  Rust `cx` to mutate, and a plain return value keeps the JS call one-way (no reentrancy
  back into the renderer). The binding component applies the returned styles/fonts through
  the native `cx` after the call.
- **`attr()` — raw or resolved?** *Decided: resolved only*, through
  `Semantic::attribute` exactly as built-ins read attributes. Raw access is not offered
  (it would bypass `mj-attributes`).
- **Versioning.** *Decided: crate semver*, documented. No embedded ABI-version integer in
  `ComponentCx`.
