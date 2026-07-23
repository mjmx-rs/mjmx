// Custom components with @mjmx/node (ES modules).
//
//   node examples/node-esm/custom-components.mjs
//
// Uses the locally built addon via a relative path; with the package installed the
// import would be `import mjmx from '@mjmx/node'`.

import mjmx from '../../crates/mjmx-napi/index.mjs';

const badge = (cx) => {
  const label = cx.attr('label') ?? '';
  const color = cx.attr('color') ?? '#5b21b6';
  return (
    `<span style="display:inline-block;padding:2px 10px;border-radius:9999px;` +
    `background:${color};color:#fff;font:600 12px sans-serif">${label}</span>`
  );
};

const card = (cx) => ({
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

const html = mjmx.render(source, {
  components: { 'mj-badge': badge, 'mj-card': card },
});

console.log(html);
