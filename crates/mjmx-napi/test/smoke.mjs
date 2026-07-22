// Smoke test for the mjmx native addon: loads the built binary and exercises
// every exported function against the repo's fixtures.
//
//   node crates/mjmx-napi/test/smoke.mjs

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..', '..');
const fixtures = join(repoRoot, 'fixtures');

const mjmx = require(join(here, '..', 'index.js'));

let passed = 0;
function check(name, fn) {
  fn();
  passed += 1;
  console.log(`  ok  ${name}`);
}

console.log('mjmx-napi smoke test');

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

check('renderFile() resolves mj-include', () => {
  const out = mjmx.renderFile(join(fixtures, 'include', 'main.mjml'));
  assert.ok(out.includes('Before include.'));
  assert.ok(out.includes('Included section content.'), 'expected the partial to be inlined');
  assert.ok(out.includes('After include.'));
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

check('render({ filePath }) resolves mj-include from a string', () => {
  const main = readFileSync(join(fixtures, 'include', 'main.mjml'), 'utf8');
  const out = mjmx.render(main, { filePath: join(fixtures, 'include') });
  assert.ok(out.includes('Included section content.'), 'expected the partial to be inlined');
});

check('lint() flags a known-bad document', () => {
  const diagnostics = mjmx.lint('<mjml><mj-body><mj-column></mj-column></mj-body></mjml>');
  assert.ok(Array.isArray(diagnostics));
  assert.ok(diagnostics.length > 0, 'expected at least one diagnostic');
  const errors = diagnostics.filter((d) => d.severity === 'error');
  assert.ok(errors.length > 0, 'expected at least one severity:"error" diagnostic');
  const first = errors[0];
  assert.equal(typeof first.message, 'string');
  assert.ok(Array.isArray(first.labels));
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

check('render() throws an Error carrying diagnostics', () => {
  let thrown;
  try {
    mjmx.render('<mjml><mj-body><mj-section>');
  } catch (err) {
    thrown = err;
  }
  assert.ok(thrown instanceof Error, 'expected a thrown Error');
  assert.match(thrown.message, /^mjmx: error: /);
  assert.ok(Array.isArray(thrown.diagnostics), 'expected err.diagnostics');
  assert.ok(thrown.diagnostics.length > 0);
  console.log(`      message: ${thrown.message}`);
});

check('renderFile() throws on a missing file', () => {
  assert.throws(() => mjmx.renderFile(join(fixtures, 'does-not-exist.mjml')), /could not read/);
});

console.log(`\n${passed} checks passed.`);
