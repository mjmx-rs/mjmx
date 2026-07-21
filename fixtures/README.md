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

All six render byte-identical (under the whitespace-normalized comparator) to the
vendored mjml 4.18.0 output in `crates/mrml/tests/golden.rs`.
