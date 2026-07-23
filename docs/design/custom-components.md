# Design: custom components (mjmx extension points)

Status: **Phase 1 (native Rust) — implemented 2026-07-23.** Tier 2 (container
components) and Phases 2–3 (napi/wasm/python callbacks) remain proposal. Author-reviewed
2026-07-22. The shipped surface: `mjmx::{CustomComponent, ComponentRegistry, ComponentCx}`
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

## Tier 2 (deferred): container components

Letting a custom tag contain layout components (`mj-column`, …) requires exposing
the container-width computation, the ghost-row Outlook scaffolding, and a
child-render entry back into mjmx's dispatch. That ABI is large and couples to
mjmx's layout internals — the opposite of Tier 1's small stable surface. Out of
scope until Tier 1 ships and demand is demonstrated.

## Phasing

1. **Native Rust trait + `ComponentCx` + registry-aware dispatch/linter.** Proves
   the ABI with zero binding complexity. Small.
2. **napi + wasm callbacks** over the same `cx`. Medium — threadsafe-function /
   imported-function plumbing, done once against a shared shape.
3. **python (pyo3)** — same `cx`, at that binding's milestone.

Native-first locks the ABI before callback plumbing is paid for three times.

## Open questions — resolved for Phase 1

- **Head contributions.** *Decided: collected via `cx` mutation.* `add_style` /
  `add_font` push onto the `RenderContext` (styles reuse the existing dedup buffer; fonts
  land in an owned `custom_fonts: Vec<(String, String)>` side-channel, merged into the head
  font pass and emitted — `<link>` + `@import` — only when the family is referenced in the
  body, mjml's own rule). Owned rather than arena-bound so a future callback need not
  thread the allocator. The napi/wasm return-channel question (Phase 2) is still open and
  does not affect the native shape.
- **`attr()` — raw or resolved?** *Decided: resolved only*, through
  `Semantic::attribute` exactly as built-ins read attributes. Raw access is not offered
  (it would bypass `mj-attributes`).
- **Versioning.** *Decided: crate semver*, documented. No embedded ABI-version integer in
  `ComponentCx`.
