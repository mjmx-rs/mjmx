// ESM entry point: a thin re-export of the CommonJS loader in `index.js`, so the
// package resolves under both `require('mjmx-node')` and `import … from 'mjmx-node'`.
// The loader, native binary resolution and diagnostics enrichment all live in the CJS
// module; this file only restates its surface as named ESM exports.

import mod from './index.js';

export const render = mod.render;
export const renderFile = mod.renderFile;
export const format = mod.format;
export const lint = mod.lint;
export const version = mod.version;

export default mod;
