# Golden fixtures

`.mjml` inputs paired with `.html` reference output, used as the parity oracle.

Reference `.html` files are generated from the **official JS mjml**, version-pinned,
and vendored (not regenerated at test time):

```sh
npx --yes mjml@4 fixtures/<name>.mjml -o fixtures/<name>.html
```

- Generator: `mjml` **4.18.0** (mjml-core + mjml-cli 4.18.0).
- Tests compare rendered output to these goldens **semantically** (whitespace between
  tags normalized), not byte-exact.

To add a fixture: write `<name>.mjml`, run the command above, commit both files.

## Fixtures

| Fixture | Exercises |
| --- | --- |
| `slice` | The M4 slice: `mj-body` / `mj-section` / `mj-column` / `mj-text`, no head. |
| `attrs_all` | `<mj-attributes>` with `<mj-all font-family>` + per-tag `<mj-text color>`. |
| `attrs_class` | `<mj-class name="blue" color>` applied via `mj-class="blue"`. |
| `head_breakpoint` | `<mj-breakpoint width="320px">` driving the responsive media queries. |
| `head_title_preview` | `<mj-title>` (`<title>` + `aria-label`) and `<mj-preview>` preheader. |
| `head_font` | Custom `<mj-font>` imported because `<mj-text font-family>` references it. |
| `img` | `<mj-image>` with a fixed `width`: aligned `<td>` + inner table + `<img>`, plus the `mj-full-width-mobile` head style. |
| `img_link` | `<mj-image href>` wrapping the `<img>` in an `<a target="_blank">`. |
| `divider` | `<mj-divider>` `<p>` rule + the `<!--[if mso | IE]>` ghost table sized to the content box. |
| `spacer` | `<mj-spacer height>` div/table vertical whitespace. |
| `button` | `<mj-button href>`: table + `<a>`, registering the default Ubuntu font in the head. |
| `twocol` | Two-column `<mj-section>`: each column stacks on mobile (inline `width:100%`), the `mj-column-per-50` class + media query carrying the desktop share. |
| `raw` | `<mj-raw>` body passthrough: inner HTML emitted verbatim with no wrapping. |
| `group` | `<mj-section>` holding a 2-column `<mj-group>` — side-by-side columns, group-managed Outlook ghost table, `mj-column-per-100`/`-50` media queries. |
| `wrapper` | `<mj-wrapper>` > `mj-section` > `mj-column` > `mj-text`: section-shaped container wrapping a whole section (fused Outlook conditionals). |
| `wrapper_full_width` | `<mj-wrapper full-width="full-width">`: adds the outer 100%-width table around the same inner markup. |
| `table` | `<mj-table>` wrapping raw `<tr>`/`<th>`/`<td>` rows verbatim inside a styled `<table>`, registering the default Ubuntu font. |
| `social` | `<mj-social>` (horizontal) with two `<mj-social-element>`s (facebook, twitter): per-element inline tables + merged mso ghost cells, baked network icons/colors/share hrefs. |
| `social_vertical` | `<mj-social mode="vertical">`: the same two elements stacked as rows in one `margin:0px` table. |
| `navbar` | `<mj-navbar>` with two `<mj-navbar-link>`s (no hamburger): `mj-inline-links` div + merged mso ghost table; still registers the `.mj-menu-*` head stylesheet. |
| `navbar_hamburger` | `<mj-navbar hamburger="hamburger">`: adds the CSS-checkbox mobile-menu toggle (input/label/trigger) plus the `.mj-menu-*` head stylesheet. |
| `hero` | `<mj-hero mode="fixed-height">` at body level: centered container + the `<v:image>`/mso ghost background, the `background` shorthand cell, and a nested `mj-hero-content` table holding an `mj-text` and `mj-button`. |
| `carousel` | `<mj-carousel>` with three `<mj-carousel-image>`s: the radio-button markup (hidden inputs, thumbnails, prev/next controls, per-slide divs), the mso first-image fallback, and the large `.mj-carousel-*` `:checked` head stylesheet. |
| `accordion` | `<mj-accordion>` with two `<mj-accordion-element>`s (title + text each): the `<label>`/checkbox collapsible markup, the `+`/`-` icon cells, and the static `.mj-accordion*` head stylesheet. |

All fixtures render byte-identical (under the whitespace-normalized comparator) to the
vendored mjml 4.18.0 output in `crates/mrml/tests/golden.rs`.

**Note on `navbar_hamburger`:** mjml stamps the checkbox/label pair with a per-navbar
*random* 16-hex-digit id (`genRandomHexString(16)`), which is non-reproducible. The
vendored golden's id is pinned to `0000000000000000` — the deterministic value our
renderer emits (`RenderContext::next_hamburger_id`); every other byte is mjml's own
output. The id is a purely internal anchor (`<input id>` must equal `<label for>`), so
pinning it changes nothing observable.

**Note on `carousel`:** mjml stamps each `<mj-carousel>` with a per-carousel *random*
16-hex-digit token (`genRandomHexString(16)`) woven through both the radio/label markup
and the `.mj-carousel-*` head stylesheet. The vendored golden's token is pinned to
`0000000000000000` — the deterministic value our renderer emits
(`RenderContext::next_carousel_id`); every other byte is mjml's own output. The token is
a purely internal anchor (radio `id`/`name` must agree with the `<label for>` and the
stylesheet selectors), so pinning it changes nothing observable.
