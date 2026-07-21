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
