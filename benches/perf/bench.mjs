// Times the two pinned mjml releases over the repo's `fixtures/*.mjml` and prints one
// JSON object per engine to stdout (consumed by the Rust harness in `src/main.rs`).
//
// mjml 5.x's `mjml2html` returns a Promise (5.0 went async), so every render is awaited;
// 4.18.0 is synchronous but awaiting its plain return is harmless. Timing excludes the
// module load and a warmup pass, matching the Rust side. Iteration count is argv[2]
// (default 200). All human-readable output goes to stderr so stdout stays pure JSONL.

import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const ITERS = Number.parseInt(process.argv[2] ?? '200', 10) || 200;
const here = dirname(fileURLToPath(import.meta.url));
const fixturesDir = join(here, '..', '..', 'fixtures');

const sources = readdirSync(fixturesDir)
  .filter((f) => f.endsWith('.mjml'))
  .sort()
  .map((f) => readFileSync(join(fixturesDir, f), 'utf8'));

/** Time one mjml entry point over every fixture, `ITERS` passes after a warmup. */
async function bench(name, version, mjml2html) {
  // Warmup + renderability count (both mjml versions render all fixtures, but count
  // defensively so the caveat stays honest if that ever changes).
  let renderable = 0;
  for (const src of sources) {
    try {
      await mjml2html(src);
      renderable += 1;
    } catch {
      /* skip a fixture this engine cannot render */
    }
  }

  const start = process.hrtime.bigint();
  for (let i = 0; i < ITERS; i += 1) {
    for (const src of sources) {
      try {
        await mjml2html(src);
      } catch {
        /* already counted above */
      }
    }
  }
  const elapsedNs = Number(process.hrtime.bigint() - start);
  const renders = renderable * ITERS;
  const usPerRender = elapsedNs / renders / 1000;

  process.stderr.write(`${name}: ${renderable}/${sources.length} fixtures, ${usPerRender.toFixed(1)} us/render\n`);
  process.stdout.write(
    `${JSON.stringify({ version, us_per_render: usPerRender, fixtures: renderable })}\n`,
  );
}

const { default: mjml418 } = await import('mjml418');
const { default: mjml540 } = await import('mjml540');

await bench('mjml 4.18.0', '4.18.0', mjml418);
await bench('mjml 5.4.0', '5.4.0', mjml540);
