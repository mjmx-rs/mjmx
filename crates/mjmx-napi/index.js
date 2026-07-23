// Loader + thin JS wrapper for the mjmx native addon.
//
// Two jobs:
//   1. Locate the compiled `.node` binary. In a published install the binary
//      arrives as a per-platform optional dependency (`@mjmx/node-<triple>`,
//      e.g. `@mjmx/node-linux-x64-gnu`), which npm installs only on a matching
//      host; we `require` that first. For local development a bare
//      `cargo build -p mjmx-napi` (or `make napi`, which stages the cdylib next
//      to this file as `mjmx.<triple>.node`) is enough — the raw cargo target
//      dirs and the staged name are accepted as fallbacks.
//   2. Attach structured diagnostics to thrown errors. N-API's `Error` is a
//      status + reason pair with no room for a payload, so the native side
//      throws a well-worded aggregate message and we re-run `lint()` on the
//      offending source here to populate `err.diagnostics`.

'use strict';

const { existsSync } = require('node:fs');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');

/** The napi-rs triple for the current host, e.g. `linux-x64-gnu`, `darwin-arm64`. */
function hostTriple() {
  const { platform, arch } = process;
  if (platform === 'linux') {
    // musl vs glibc: `process.report` exposes the libc on modern Node; default gnu.
    const libc =
      typeof process.report?.getReport === 'function' &&
      process.report.getReport().header?.glibcVersionRuntime === undefined
        ? 'musl'
        : 'gnu';
    return `linux-${arch}-${libc}`;
  }
  if (platform === 'darwin') return `darwin-${arch}`;
  if (platform === 'win32') return `win32-${arch}-msvc`;
  return `${platform}-${arch}`;
}

function loadNative() {
  const triple = hostTriple();

  // 1. Published install: the per-platform optional dependency. Its `main` is the
  //    `.node`, so `require` returns the native exports directly.
  try {
    return require(`@mjmx/node-${triple}`);
  } catch {
    // fall through to the local development fallbacks
  }

  // 2. Local development: a staged or raw-cargo binary next to / above this file.
  const candidates = [
    join(__dirname, `mjmx.${triple}.node`),
    join(__dirname, `mjmx.${process.platform}-${process.arch}.node`),
    join(__dirname, 'index.node'),
    join(__dirname, '..', '..', 'target', 'release', 'libmjmx_napi.so'),
    join(__dirname, '..', '..', 'target', 'debug', 'libmjmx_napi.so'),
    join(__dirname, '..', '..', 'target', 'release', 'libmjmx_napi.dylib'),
    join(__dirname, '..', '..', 'target', 'debug', 'libmjmx_napi.dylib'),
  ];
  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      // `process.dlopen` rather than `require`, so the raw cargo `.so`/`.dylib`
      // fallback (which lacks the `.node` extension) loads too.
      const handle = { exports: {} };
      process.dlopen(handle, candidate);
      return handle.exports;
    }
  }
  throw new Error(
    `mjmx: no native addon found for ${triple}. Install @mjmx/node-${triple}, ` +
      `or build from source with \`make napi\`. Looked in:\n  ${candidates.join('\n  ')}`
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
