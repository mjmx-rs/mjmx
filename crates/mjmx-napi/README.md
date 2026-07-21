# @mjmx/node

Node.js bindings for [mjmx](https://github.com/ryeasin03/mjmx) — an MJML-to-HTML
compiler written in Rust.

The npm package lives alongside the Rust crate in this directory: `package.json`,
`index.js` (loader + thin wrapper), `index.d.ts` (hand-written types), and
`test/smoke.mjs`. The compiled addon is staged here as
`mjmx.<platform>-<arch>[-gnu].node` by `make napi`.

## Build & test

```sh
make napi        # cargo build -p mjmx-napi --release + stage the .node
make napi-test   # the above, then node crates/mjmx-napi/test/smoke.mjs
```

## API

```js
const mjmx = require('@mjmx/node');

mjmx.render(source, { breakpoint: 480, beautify: false, minify: false }); // -> HTML
mjmx.renderFile('email.mjml', options);   // resolves <mj-include>
mjmx.lint(source);                        // -> Diagnostic[], never throws
mjmx.format(source, { indentWidth: 2 });  // -> canonical MJML
mjmx.version();                           // -> "0.1.0"
```

`render`, `renderFile` and `format` throw an `Error` whose message aggregates the
diagnostics (`mjmx: error: <first message> (+N more diagnostics)`). The wrapper in
`index.js` also attaches the structured list as `err.diagnostics` — the same shape
`lint()` returns.

Not published to npm.
