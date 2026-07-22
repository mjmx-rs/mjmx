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

// Render options use mjml's own `mjml2html` names and defaults; `breakpoint` is the
// one mjmx extension (mjml hardcodes 480 and only <mj-breakpoint> changes it).
mjmx.render(source, {
  breakpoint: 480,              // mjmx extension
  beautify: false,
  minify: false,
  keepComments: true,
  validationLevel: 'soft',      // 'soft' | 'strict' | 'skip'
  filePath: './partials',       // base dir for <mj-include>, as in mjml
  fonts: { Inter: 'https://fonts.example/inter.css' }, // replaces the built-in table
  minifyOptions: {},            // only consulted when `minify` is true
  ignoreIncludes: false,        // true drops <mj-include> instead of resolving it
  printerSupport: false,        // true adds an `@media only print` block
});                                       // -> HTML
mjmx.renderFile('email.mjml', options);   // resolves <mj-include> relative to the file
mjmx.lint(source);                        // -> Diagnostic[], never throws
mjmx.format(source, { indentWidth: 2 });  // -> canonical MJML
mjmx.version();                           // -> "0.1.0"
```

`minifyOptions` carries mjml's four html-minifier switches:

```js
mjmx.render(source, {
  minify: true,
  minifyOptions: {
    collapseWhitespace: true,     // default true (mjml's own)
    minifyCSS: false,             // default false; a conservative whitespace/comment
                                  //   pass, not html-minifier's clean-css optimizer
    caseSensitive: true,          // default true; false lower-cases attribute names
    removeEmptyAttributes: false, // default false — mjml passes true, see below
  },
});
```

`removeEmptyAttributes` is the one place mjmx's default differs from mjml's. mjml's own
output carries `<div … style="" role="article">`, so mjml's `true` *removes an
attribute* while minifying; mjmx defaults it off to keep `minify` a pure whitespace
transform. Pass `true` for byte parity with mjml.

`render`, `renderFile` and `format` throw an `Error` whose message aggregates the
diagnostics (`mjmx: error: <first message> (+N more diagnostics)`). The wrapper in
`index.js` also attaches the structured list as `err.diagnostics` — the same shape
`lint()` returns.

Not published to npm.
