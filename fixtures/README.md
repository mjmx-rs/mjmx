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
