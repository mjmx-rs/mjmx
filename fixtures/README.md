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
| `body_attrs` | `<mj-body background-color width css-class>`: the colour on both `<body style>` and the ARIA wrapper `<div>`, the wrapper `class`, and the 500px container width threaded into the section/column. |
| `body_attrs_head` | The same `<mj-body>` attributes supplied via `<mj-attributes><mj-body …>` instead of inline, proving they resolve through the semantic layer. |
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
| `text_attrs` | `<mj-text>` with non-default `align`, `color`, `font-size/style/weight`, `line-height`, `letter-spacing`, `text-decoration/transform`, `height` (Outlook ghost cell), `padding`, `container-background-color`. Locks the align/text-align + font resolution and the conditional div declarations. |
| `button_attrs` | `<mj-button>` with non-default box/typography attrs incl. `width`/`inner-padding`/`border` driving the computed `<a>` width (`calculateAWidth`), `height`, `letter-spacing`, `font-style`, and `container-background-color`. |
| `image_attrs` | `<mj-image>` with `align`, `width`/`height`, `border`, `border-radius`, `padding`, `container-background-color`: themed `<img>` style + parsed `height` attribute. |
| `section_attrs` | `<mj-section>` with `background-color`, `border`, `border-radius`, `padding`, `direction`, `text-align`: bg on ghost/div/table, rounded-clip, and the content-box (`600 − padding − border`) handed to columns. |
| `column_attrs` | `<mj-column>` gutter mode (`background-color`, `border`, `border-radius`, `padding`, `vertical-align`): the extra gutter cell + the section ghost cell's mirrored vertical-align. |
| `divider_attrs` | `<mj-divider>` with `border-*`, `width` (px), `align` (margin per alignment), `padding`, `container-background-color`. |
| `spacer_attrs` | `<mj-spacer>` with `height`, `padding`, `container-background-color`. |
| `table_attrs` | `<mj-table>` with `align`, `color`, `font-size`, `line-height`, `width`, `border`, `cellpadding/cellspacing` (cellspacing>0 → `border-collapse:separate`), `table-layout`, `padding`, `container-background-color`. |
| `social_attrs` | `<mj-social>` forwarding `icon-size`, `border-radius`, `color`, `font-size`, `inner-padding`, plus `align`, `padding`, `container-background-color`. |
| `hero_attrs` | `<mj-hero>` with non-default `background-position`, `vertical-align`, `height`, `padding` (fixed-height cell math). |
| `navbar_attrs` | `<mj-navbar base-url align>` with links setting `color`, `font-size/weight/style`, `line-height`, `letter-spacing`, `text-decoration/transform`, `padding`. |
| `accordion_attrs` | `<mj-accordion>` with `border`, `font-family`, `icon-position/width/height`, `padding`, and title/text `font-size`, `line-height`, `padding`, `color`, `background-color` resolved per child. |
| `wrapper_attrs` | `<mj-wrapper>` with section-style `background-color`, `border`, `border-radius`, `padding`, `direction`, `text-align`; box width threaded to nested sections. |
| `group_attrs` | `<mj-group>` with `background-color` (div + ghost `bgcolor`), `vertical-align` (div + section ghost cell), `direction`. |
| `section_bg_url` | `<mj-section background-url background-color>`: default `repeat` / `auto` / `top center` — the `background` shorthand + longhands on div and table, the zero-line-height inner div, the `background=` attribute, and the VML `<v:rect>`/`<v:fill type="tile" origin="0.5, 0">` block. |
| `section_bg_full` | `<mj-section>` with `background-size="cover"`, `background-repeat="no-repeat"`, `background-position="bottom right"`: keyword→percentage VML math (`origin="0.5, 0.5"`), `type="frame"`, `size="1,1" aspect="atleast"`. |
| `section_bg_attrs` | Background attributes resolved through `<mj-attributes>` (a section default: `contain` + single-keyword `left`) and `<mj-class>` (`10.5% bottom`, two-length `background-size`), exercising `parseInt` percentage truncation. |
| `section_bg_full_width` | `<mj-section full-width="full-width">` with a background: declarations move to the outer 100% table, the rect stretches with `mso-width-percent:1000`, and `background-position-x`/`-y` override the shorthand. |
| `wrapper_bg_url` | `<mj-wrapper background-url background-color>`: the section background path applied to a wrapper (its ghost row still wraps each nested section). |
| `wrapper_bg_full_width` | `<mj-wrapper full-width="full-width">` with `background-position="25% 75%"`, `background-size="50% 30%"`, `no-repeat`: percentage VML math (`origin="-0.25, 0.25"`) and two-length `size="50%,30%"`. |

All fixtures render byte-identical (under the whitespace-normalized comparator) to the
vendored mjml 4.18.0 output in `crates/mjmx/tests/golden.rs`.

**`include/` (mj-include).** `include/main.mjml` uses `<mj-include path="./_section.mjml" />`
to splice a partial (`include/_section.mjml`, a bare `<mj-section>` fragment with no
`<mjml>` root) between two inline sections. The golden `include/main.html` was generated
the same way as every other fixture — `npx --yes mjml@4 fixtures/include/main.mjml -o
fixtures/include/main.html` — because mjml resolves the include from the input file's
directory. The `include_matches_mjml_reference` test drives it through
`mjmx::render_file` (not `render_default`), which resolves `<mj-include>` against the
file's parent directory via `mjmx-transformer`'s `LocalLoader`; a string passed to
`render`/`render_default` has no base directory and cannot resolve relative includes.

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
