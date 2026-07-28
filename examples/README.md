# mjmx examples — custom components

Runnable examples of **Tier 1 native custom components** through the JS bindings. A host
registers a renderer for a tag (`mj-badge`, `mj-card`); when rendering reaches that tag its
callback returns the HTML. An unregistered unknown tag stays the usual fatal
"not a recognized MJML component" error.

The callback contract is identical across every binding:

```ts
type ComponentFn = (cx: ComponentCx) => string | ComponentResult;

interface ComponentCx {
  attr(name: string): string | undefined; // cascade-resolved (mj-attributes / mj-class / defaults)
  attrs: Record<string, string>;           // all resolved attributes
  containerWidth: number;                   // px available to this element
  breakpoint: number;                       // responsive breakpoint
  rawInner: string;                         // verbatim inner source text
}

interface ComponentResult {
  html: string;
  styles?: string[];                        // <style> blocks added to <head>
  fonts?: [name: string, href: string][];   // web fonts, emitted if referenced
}
```

Return a bare `string` for HTML only, or a `ComponentResult` to also contribute to `<head>`.

## Build the packages first

The examples import the locally built packages by relative path (no install needed):

```sh
make napi    # builds crates/mjmx-napi (the mjmx-node addon)
make wasm    # builds crates/mjmx-wasm/pkg/{node,bundler,web} (the `mjmx` npm package)
```

With the packages published, swap the relative import for `mjmx-node` / `mjmx-wasm`.

## Run

| Example | Command |
| --- | --- |
| `node-cjs/custom-components.js` | `node examples/node-cjs/custom-components.js` |
| `node-esm/custom-components.mjs` | `node examples/node-esm/custom-components.mjs` |
| `node-ts/custom-components.ts` | `cd examples/node-ts && npm install`, then `npm run check` (type-check) and `npm start` (run via ts-node) |
| `browser/custom-components.html` | serve the repo root (`python3 -m http.server`) and open `http://localhost:8000/examples/browser/custom-components.html` |

The Node examples print the rendered HTML to stdout. The browser example renders in-page via
WebAssembly (it `await init()`s the web build first).
