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
  // V5 (the default compat) ignores includes unless asked; opt in to exercise resolution.
  const out = mjmx.renderFile(join(fixtures, 'include', 'main.mjml'), { ignoreIncludes: false });
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
  // V5 (the default compat) ignores includes unless asked; opt in to exercise resolution.
  const out = mjmx.render(main, { filePath: join(fixtures, 'include'), ignoreIncludes: false });
  assert.ok(out.includes('Included section content.'), 'expected the partial to be inlined');
});

check('render({ minifyOptions }) matches mjml', () => {
  const two =
    '<mjml><mj-body><mj-section><mj-column><mj-text>hello    world</mj-text></mj-column>' +
    '<mj-column><mj-text>b</mj-text></mj-column></mj-section></mj-body></mjml>';
  // Pinned to V4: the empty `style=""` these switches observe is a V4-skeleton
  // artifact (V5 drops it from the wrapper div); the minify plumbing is unchanged.
  const opts = (minifyOptions) => mjmx.render(two, { minify: true, minifyOptions, mjmlVersion: 4 });

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
});

check('render({ ignoreIncludes }) drops mj-include instead of resolving it', () => {
  const main = readFileSync(join(fixtures, 'include', 'main.mjml'), 'utf8');
  const base = join(fixtures, 'include');
  // With a base the partial is inlined; ignoreIncludes wins over filePath.
  // (V5 default ignores includes, so opt in for the resolving arm of the contrast.)
  assert.ok(mjmx.render(main, { filePath: base, ignoreIncludes: false }).includes('Included section content.'));
  const out = mjmx.render(main, { filePath: base, ignoreIncludes: true });
  assert.ok(!out.includes('Included section content.'), 'expected the partial to be dropped');
  assert.ok(out.includes('Before include.') && out.includes('After include.'));
});

check('renderFile({ ignoreIncludes }) drops mj-include too', () => {
  const main = join(fixtures, 'include', 'main.mjml');
  // V5 default ignores includes, so opt in for the resolving arm of the contrast.
  assert.ok(mjmx.renderFile(main, { ignoreIncludes: false }).includes('Included section content.'));
  assert.ok(!mjmx.renderFile(main, { ignoreIncludes: true }).includes('Included section content.'));
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

const templated =
  '<mjml><mj-head><mj-style inline="inline">.probe{color:#ff00ff}</mj-style></mj-head>' +
  '<mj-body><mj-raw><span class="probe">{{ "<div class=\'probe\'>" }}</span></mj-raw>' +
  '<mj-section><mj-column><mj-text css-class="probe">t</mj-text></mj-column></mj-section>' +
  '</mj-body></mjml>';

check('inlining masks EJS and Handlebars by default, as juice does', () => {
  const out = mjmx.render(templated);
  // The markup inside the Handlebars literal is not markup: it must come back byte
  // for byte, and must not have collected a style attribute of its own.
  assert.ok(out.includes('{{ "<div class=\'probe\'>" }}'), 'template expression mangled');
  assert.equal(out.split('color: #ff00ff').length - 1, 2, 'inlined an element that does not exist');
});

check('render({ juicePreserveTags }) adds to the built-ins', () => {
  const custom =
    '<mjml><mj-head><mj-style inline="inline">.probe{color:#ff00ff}</mj-style></mj-head>' +
    '<mj-body><mj-raw><span class="probe"><# <i class="probe"> </#</span></mj-raw>' +
    '</mj-body></mjml>';
  // Unregistered, the <i> inside the custom delimiters is real markup and gets inlined.
  assert.equal(mjmx.render(custom).split('color: #ff00ff').length - 1, 2);
  const out = mjmx.render(custom, { juicePreserveTags: { myTag: { start: '<#', end: '</#' } } });
  assert.equal(out.split('color: #ff00ff').length - 1, 1);
  assert.ok(out.includes('<# <i class="probe"> </#'));
  // Per-call, not global: the next render must not still know about myTag.
  assert.equal(mjmx.render(custom).split('color: #ff00ff').length - 1, 2);
});

check('render({ juiceOptions }) switches the attribute mirrors and !important', () => {
  const src =
    '<mjml><mj-head><mj-style inline="inline">' +
    'td.probe{width:123px !important;background-color:#010203}' +
    '</mj-style></mj-head><mj-body><mj-section><mj-column>' +
    '<mj-text css-class="probe">t</mj-text></mj-column></mj-section></mj-body></mjml>';
  const on = mjmx.render(src);
  assert.ok(on.includes('width="123"'), 'expected the width attribute mirror');
  // The document's own media queries use `!important`; check the inlined value.
  assert.ok(on.includes('width: 123px;'), 'the keyword is dropped by default, as in mjml');

  const off = mjmx.render(src, {
    juiceOptions: { applyWidthAttributes: false, preserveImportant: true },
  });
  assert.ok(!off.includes('width="123"'), 'applyWidthAttributes: false still mirrored');
  assert.ok(off.includes('width: 123px !important'), 'preserveImportant not honoured');
});

// N-API decodes `juiceOptions` field by field into a typed struct, so — unlike the
// wasm build, which sees the raw object and throws — an unsupported juice option is
// caught by the *TypeScript* interface (which is closed) rather than at runtime.
// Pinned here so the limitation is a decision and not a surprise. Same shape as this
// binding's long-standing `minifyOptions` behaviour.
check('an unsupported juiceOption is a type error, not a runtime one', () => {
  const out = mjmx.render(templated, { juiceOptions: { preserveMediaQueries: true } });
  assert.equal(out, mjmx.render(templated), 'an ignored key must not change the output');
});

// `resolveCSSVariables` (juice's default) resolves `var()` and drops `--x`.
const cssVarsDoc =
  '<mjml><mj-head><mj-style inline="inline">.v{--c:#ff0000;color:var(--c)}</mj-style>' +
  '</mj-head><mj-body><mj-section><mj-column><mj-text css-class="v">x</mj-text>' +
  '</mj-column></mj-section></mj-body></mjml>';
check('resolveCSSVariables resolves var() by default', () => {
  const on = mjmx.render(cssVarsDoc);
  assert.ok(on.includes('color: #ff0000'), 'var(--c) should resolve');
  assert.ok(!on.includes('--c'), 'the custom property should be dropped');
});
check('resolveCSSVariables:false leaves var() verbatim', () => {
  const off = mjmx.render(cssVarsDoc, { juiceOptions: { resolveCSSVariables: false } });
  assert.ok(off.includes('var(--c)'), 'var() should be left as written');
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

check('render({ components }) registers a custom component (Tier 1)', () => {
  const src =
    '<mjml><mj-body><mj-section><mj-column>' +
    '<mj-badge label="Hi">deep</mj-badge></mj-column></mj-section></mj-body></mjml>';
  const html = mjmx.render(src, {
    components: {
      'mj-badge': (cx) => ({
        html: `<span class="badge">${cx.attr('label')}:${cx.rawInner}</span>`,
        styles: ['.badge{color:#f0f}'],
      }),
    },
  });
  assert.ok(html.includes('<span class="badge">Hi:deep</span>'), 'expected the component fragment');
  // mjmx wraps the returned fragment in the standard leaf <td> cell, so the table is valid.
  assert.ok(
    html.includes('<td style="font-size:0px;word-break:break-word;"><span class="badge">Hi:deep</span></td>'),
    'expected the fragment wrapped in the standard cell',
  );
  assert.ok(html.includes('.badge{color:#f0f}'), 'expected the returned style in <head>');
  // A bare string return works too, and cx exposes the layout snapshot.
  const bare = mjmx.render(src, { components: { 'mj-badge': (cx) => `<i>${cx.containerWidth}</i>` } });
  assert.ok(/<i>\d+<\/i>/.test(bare), 'expected the string-return fragment with a width');
  // Unregistered, the same tag is the usual fatal error.
  assert.throws(() => mjmx.render(src), /not a recognized MJML component|unknown mjml component/);
});

console.log(`\n${passed} checks passed.`);
