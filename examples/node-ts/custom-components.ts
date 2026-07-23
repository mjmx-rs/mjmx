// Custom components with @mjmx/node, typed (TypeScript).
//
//   npm i -D typescript
//   npx tsc --project examples/node-ts        # type-check
//   npx ts-node examples/node-ts/custom-components.ts   # or run it
//
// With the package installed the import would be `from '@mjmx/node'`.
import { render, type ComponentCx, type ComponentResult } from '../../crates/mjmx-napi';

// The callback signature is fully typed: `cx` is a ComponentCx, the return is
// `string | ComponentResult`.
const badge = (cx: ComponentCx): string => {
  const label = cx.attr('label') ?? '';
  const color = cx.attr('color') ?? '#5b21b6';
  return (
    `<span style="display:inline-block;padding:2px 10px;border-radius:9999px;` +
    `background:${color};color:#fff;font:600 12px sans-serif">${label}</span>`
  );
};

const card = (cx: ComponentCx): ComponentResult => ({
  html: `<div class="mjx-card" style="font-family:Fraunces,Georgia,serif">${cx.rawInner}</div>`,
  styles: ['.mjx-card{border:1px solid #e5e7eb;border-radius:8px;padding:16px}'],
  fonts: [['Fraunces', 'https://fonts.googleapis.com/css?family=Fraunces']],
});

const source = `
<mjml>
  <mj-body>
    <mj-section>
      <mj-column>
        <mj-badge label="New" color="#dc2626" />
        <mj-card>Hand-rolled card body, rendered by a host callback.</mj-card>
      </mj-column>
    </mj-section>
  </mj-body>
</mjml>`;

const html: string = render(source, {
  components: { 'mj-badge': badge, 'mj-card': card },
});

console.log(html);
