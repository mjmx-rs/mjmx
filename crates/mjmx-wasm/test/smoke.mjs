// Smoke test for the mjmx WebAssembly package: loads the built wasm module and
// exercises every exported function against the repo's fixtures.
//
//   make wasm && node crates/mjmx-wasm/test/smoke.mjs
//
// The `--target nodejs` glue is CommonJS, hence `createRequire`.

import assert from 'node:assert/strict';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..', '..');
const fixtures = join(repoRoot, 'fixtures');
const pkg = join(here, '..', 'pkg');

if (!existsSync(join(pkg, 'mjmx_wasm.js'))) {
  throw new Error(`mjmx: no wasm package at ${pkg}. Run \`make wasm\` first.`);
}

const mjmx = require(join(pkg, 'mjmx_wasm.js'));

let passed = 0;
function check(name, fn) {
  fn();
  passed += 1;
  console.log(`  ok  ${name}`);
}

/**
 * Collapse whitespace *between* tags so two renderings can be compared for
 * structural equality without tripping over pretty-printing differences.
 */
function normalize(html) {
  return html.replace(/>\s+</g, '><').replace(/\s+/g, ' ').trim();
}

console.log('mjmx-wasm smoke test');

check('the wasm module loads', () => {
  const bytes = statSync(join(pkg, 'mjmx_wasm_bg.wasm')).size;
  console.log(`      mjmx_wasm_bg.wasm = ${bytes} bytes (${(bytes / 1024).toFixed(1)} KiB)`);
  for (const fn of ['render', 'lint', 'format', 'version']) {
    assert.equal(typeof mjmx[fn], 'function', `expected an exported ${fn}()`);
  }
});

check('version()', () => {
  const v = mjmx.version();
  assert.match(v, /^\d+\.\d+\.\d+$/);
  console.log(`      version = ${v}`);
});

const slice = readFileSync(join(fixtures, 'slice.mjml'), 'utf8');
const html = mjmx.render(slice);

check('render() produces a full HTML document', () => {
  assert.ok(html.includes('Hello world'), 'expected "Hello world" in output');
  assert.ok(html.startsWith('<!doctype html>'), 'expected a <!doctype html> prologue');
  console.log(`      ${html.length} bytes of HTML`);
});

check('render() matches the vendored mjml golden output', () => {
  const golden = readFileSync(join(fixtures, 'slice.html'), 'utf8');
  assert.equal(
    normalize(html),
    normalize(golden),
    'wasm render diverged from fixtures/slice.html'
  );
  console.log(`      byte-identical to fixtures/slice.html (whitespace-normalized)`);
});

check('render({ minify: true }) is smaller than the default render', () => {
  const minified = mjmx.render(slice, { minify: true });
  assert.ok(minified.includes('Hello world'));
  assert.ok(
    minified.length < html.length,
    `expected minified (${minified.length}) < default (${html.length})`
  );
  console.log(`      ${html.length} -> ${minified.length} bytes`);
});

check('render({ breakpoint }) is honored', () => {
  const out = mjmx.render(slice, { breakpoint: 320 });
  assert.ok(out.includes('320px'), 'expected the custom breakpoint in the media query');
});

check('render({ keepComments }) matches mjml', () => {
  const source =
    '<mjml><mj-body><mj-section><mj-column>' +
    '<!-- kept --><mj-text>Hi</mj-text>' +
    '</mj-column></mj-section></mj-body></mjml>';
  // mjml defaults `keepComments` to true.
  assert.ok(mjmx.render(source).includes('kept'), 'expected comments kept by default');
  assert.ok(
    !mjmx.render(source, { keepComments: false }).includes('kept'),
    'expected --keepComments:false to strip the comment'
  );
});

check('render({ fonts }) replaces the built-in table, in key order', () => {
  const source =
    '<mjml><mj-body><mj-section><mj-column>' +
    '<mj-text font-family="Zeta">a</mj-text><mj-text font-family="Alpha">b</mj-text>' +
    '</mj-column></mj-section></mj-body></mjml>';
  const out = mjmx.render(source, {
    fonts: { Zeta: 'https://z.example/z.css', Alpha: 'https://a.example/a.css' },
  });
  assert.ok(!out.includes('fonts.googleapis.com'), 'expected the built-ins to be replaced');
  assert.ok(
    out.indexOf('z.example') < out.indexOf('a.example'),
    'expected the object key order to be preserved'
  );
});

check('render({ validationLevel }) matches mjml', () => {
  const bad = '<mjml><mj-body><mj-text>x</mj-text></mj-body></mjml>';
  // 'soft' (the default) reports but renders anyway; 'strict' throws; 'skip' is silent.
  assert.ok(mjmx.render(bad).includes('<!doctype html>'), 'expected soft to render');
  assert.throws(() => mjmx.render(bad, { validationLevel: 'strict' }), /mj-text/);
  assert.ok(mjmx.render(bad, { validationLevel: 'skip' }).includes('<!doctype html>'));
  assert.throws(() => mjmx.render(bad, { validationLevel: 'loud' }), /validationLevel|invalid options/i);
});

check('render({ filePath }) is rejected: wasm has no filesystem', () => {
  assert.throws(
    () => mjmx.render(slice, { filePath: './partials' }),
    /filePath/,
    'expected filePath to be refused rather than silently ignored'
  );
});

check('render({ minifyOptions }) matches mjml', () => {
  const two =
    '<mjml><mj-body><mj-section><mj-column><mj-text>hello    world</mj-text></mj-column>' +
    '<mj-column><mj-text>b</mj-text></mj-column></mj-section></mj-body></mjml>';
  const opts = (minifyOptions) => mjmx.render(two, { minify: true, minifyOptions });

  // Defaults collapse whitespace and keep `style=""` (mjmx's one divergence).
  assert.ok(!opts({}).includes('hello    world'));
  assert.ok(opts({}).includes('style=""'));

  // Each switch is observable at its non-default value.
  assert.ok(opts({ collapseWhitespace: false }).includes('hello    world'));
  assert.ok(!opts({ removeEmptyAttributes: true }).includes('style=""'));
  assert.ok(opts({ minifyCSS: true }).length < opts({}).length);

  // minifyOptions is ignored entirely without `minify`, as in mjml.
  assert.equal(
    mjmx.render(two, { minifyOptions: { collapseWhitespace: false } }),
    mjmx.render(two)
  );

  // Unknown html-minifier switches have no effect. (The Rust `deny_unknown_fields`
  // does reject them, but serde-wasm-bindgen resolves struct fields by lookup rather
  // than by iterating the JS object, so extra keys never reach the deserializer.)
  assert.equal(opts({ removeComments: true }), opts({}));
});

check('render({ ignoreIncludes }) drops mj-include', () => {
  const withInclude =
    '<mjml><mj-body><mj-include path="./_section.mjml" /><mj-section><mj-column>' +
    '<mj-text>after</mj-text></mj-column></mj-section></mj-body></mjml>';
  // Without it the unresolvable include is an error; with it the tag simply vanishes.
  assert.throws(() => mjmx.render(withInclude), /mj-include/);
  const out = mjmx.render(withInclude, { ignoreIncludes: true });
  assert.ok(out.includes('after'), 'expected the rest of the document to render');
  assert.ok(!out.includes('mj-include'));
});

check('render({ printerSupport }) adds an @media only print block', () => {
  const two =
    '<mjml><mj-body><mj-section><mj-column><mj-text>a</mj-text></mj-column>' +
    '<mj-column><mj-text>b</mj-text></mj-column></mj-section></mj-body></mjml>';
  assert.ok(!mjmx.render(two).includes('@media only print'));
  const out = mjmx.render(two, { printerSupport: true });
  assert.ok(out.includes('@media only print'));
  // It replays the base rules, after the screen and Thunderbird blocks.
  assert.ok(out.indexOf('.moz-text-html') < out.indexOf('@media only print'));
  assert.ok(out.slice(out.indexOf('@media only print')).includes('.mj-column-per-50'));
});

check('lint() flags a known-bad document', () => {
  const diagnostics = mjmx.lint('<mjml><mj-body><mj-column></mj-column></mj-body></mjml>');
  assert.ok(Array.isArray(diagnostics), 'expected a JS array');
  assert.ok(diagnostics.length > 0, 'expected at least one diagnostic');
  const errors = diagnostics.filter((d) => d.severity === 'error');
  assert.ok(errors.length > 0, 'expected at least one severity:"error" diagnostic');
  const first = errors[0];
  assert.equal(typeof first.message, 'string');
  assert.ok(Array.isArray(first.labels));
  assert.equal(typeof first.labels[0].start, 'number');
  console.log(`      ${diagnostics.length} diagnostic(s); first error: ${first.message}`);
});

check('lint() returns no errors for a valid document', () => {
  const diagnostics = mjmx.lint(slice);
  assert.equal(
    diagnostics.filter((d) => d.severity === 'error').length,
    0,
    'expected no errors for the slice fixture'
  );
});

check('format() indents messy MJML and is idempotent', () => {
  const messy =
    '<mjml><mj-body><mj-section><mj-column><mj-text>Hi</mj-text></mj-column></mj-section></mj-body></mjml>';
  const once = mjmx.format(messy);
  assert.ok(once.includes('\n'), 'expected multi-line output');
  assert.ok(once.includes('  <mj-body>'), 'expected two-space indentation');
  assert.equal(mjmx.format(once), once, 'format is not idempotent');
  assert.equal(mjmx.render(once).length, mjmx.render(messy).length, 'format changed the render');
});

check('format({ indentWidth: 4 })', () => {
  const out = mjmx.format('<mjml><mj-body></mj-body></mjml>', { indentWidth: 4 });
  assert.ok(out.includes('    <mj-body'), `expected four-space indentation, got:\n${out}`);
});

check('render() throws an aggregated Error', () => {
  let thrown;
  try {
    mjmx.render('<mjml><mj-body><mj-section>');
  } catch (err) {
    thrown = err;
  }
  assert.ok(thrown instanceof Error, 'expected a thrown Error');
  assert.match(thrown.message, /^mjmx: error: /);
  console.log(`      message: ${thrown.message}`);
});

console.log(`\n${passed} checks passed.`);
