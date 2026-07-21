// Loader + thin JS wrapper for the mjmx native addon.
//
// Two jobs:
//   1. Locate the compiled `.node` binary. `make napi` copies the cargo cdylib
//      output next to this file as `mjmx.<platform>.node`; a plain `index.node`
//      and the raw cargo target dirs are accepted as fallbacks so a bare
//      `cargo build -p mjmx-napi` is enough for local development.
//   2. Attach structured diagnostics to thrown errors. N-API's `Error` is a
//      status + reason pair with no room for a payload, so the native side
//      throws a well-worded aggregate message and we re-run `lint()` on the
//      offending source here to populate `err.diagnostics`.

'use strict';

const { existsSync, readFileSync } = require('node:fs');
const { join } = require('node:path');

function loadNative() {
  const platform = `${process.platform}-${process.arch}`;
  const libc = process.platform === 'linux' ? '-gnu' : '';
  const candidates = [
    join(__dirname, `mjmx.${platform}${libc}.node`),
    join(__dirname, `mjmx.${platform}.node`),
    join(__dirname, 'index.node'),
    join(__dirname, '..', '..', 'target', 'release', 'libmjmx_napi.so'),
    join(__dirname, '..', '..', 'target', 'debug', 'libmjmx_napi.so'),
  ];
  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      // `process.dlopen` rather than `require`, so the raw cargo `.so` fallback
      // (which lacks the `.node` extension) loads too.
      const handle = { exports: {} };
      process.dlopen(handle, candidate);
      return handle.exports;
    }
  }
  throw new Error(
    `mjmx: no native addon found. Run \`make napi\`. Looked in:\n  ${candidates.join('\n  ')}`
  );
}

const native = loadNative();

/** Re-run the linter on `source` and attach the result to `err.diagnostics`. */
function withDiagnostics(err, source) {
  if (typeof source === 'string') {
    try {
      err.diagnostics = native.lint(source);
    } catch {
      // Diagnostics are best-effort enrichment; never mask the original error.
    }
  }
  return err;
}

function render(source, options) {
  try {
    return native.render(source, options);
  } catch (err) {
    throw withDiagnostics(err, source);
  }
}

function renderFile(path, options) {
  try {
    return native.renderFile(path, options);
  } catch (err) {
    let source;
    try {
      source = readFileSync(path, 'utf8');
    } catch {
      source = undefined;
    }
    throw withDiagnostics(err, source);
  }
}

function format(source, options) {
  try {
    return native.format(source, options);
  } catch (err) {
    throw withDiagnostics(err, source);
  }
}

module.exports = {
  render,
  renderFile,
  format,
  lint: native.lint,
  version: native.version,
};
