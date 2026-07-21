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

All fixtures render byte-identical (under the whitespace-normalized comparator) to the
vendored mjml 4.18.0 output in `crates/mrml/tests/golden.rs`.
