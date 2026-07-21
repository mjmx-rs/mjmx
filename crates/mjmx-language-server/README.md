# mjmx-language-server

A [Language Server Protocol](https://microsoft.github.io/language-server-protocol/)
server for **MJML**, built on the rest of the `mjmx` toolchain: it parses with
`mjmx-parser`, validates with `mjmx-linter`, pretty-prints with `mjmx-formatter`,
and walks the `mjmx-ast` tree for the outline.

## Features

| Request | Behaviour |
| --- | --- |
| `textDocument/publishDiagnostics` | Parse errors and linter findings, republished on open, change, save and cleared on close. |
| `textDocument/completion` | Context-aware. After `<`, offers only the tags legal as children of the enclosing element (driven by the linter's own `allowed_children` table); inside a tag, offers that component's attributes, required ones first. Items carry Markdown documentation. |
| `textDocument/hover` | Component summary + attribute list on a tag name; meaning and default on an attribute name. |
| `textDocument/formatting` | A single whole-document edit from `mjmx-formatter`, honouring the editor's `tabSize`. Skipped while the document has syntax errors. |
| `textDocument/documentSymbol` | The nested MJML element tree, so editors show structure and breadcrumbs. |

Document sync is **incremental** (`TextDocumentSyncKind.Incremental`): ranged
changes are spliced into the in-memory document, and the full-replacement event
shape (`range: null`) is handled too, since clients send both. Positions use the
LSP default UTF-16 encoding; `mjmx` byte spans are converted through a line index
that is unit-tested against multi-byte and CRLF input, and the incremental apply
is checked against a plain-`String` oracle over randomised edit sequences.

The hover/completion prose comes from a built-in table in `src/docs.rs`. It is
**non-exhaustive** by design — undocumented attributes still complete and hover,
just without prose.

## Two ways to run it

The same server ships behind two commands, and editors can use either:

| Command | Comes from | Use when |
| --- | --- | --- |
| `mjmx lsp` | `apps/mjmx-cli` (the `mjmx` binary) | You already have `mjmx` on `PATH` and want a single binary. |
| `mjmx-language-server` | this crate | You want the server on its own, without the CLI. |

Both take no further arguments and speak LSP over **stdin/stdout**; they are the
same code path (`mjmx_language_server::run()`), so the choice is purely about
which binary you want to install.

## Build

```sh
make lsp          # → target/release/mjmx-language-server
make lsp-test     # end-to-end JSON-RPC smoke test against the real binary
make cli-lsp-test # the same session driven through `mjmx lsp`
```

## Editor setup

Every example below shows `mjmx lsp` first; swap the command for
`mjmx-language-server` (with no arguments) to use the standalone binary instead.

### Neovim (0.11+, built-in `vim.lsp.config`)

```lua
vim.filetype.add({ extension = { mjml = "mjml" } })

vim.lsp.config("mjmx", {
  -- or: cmd = { "mjmx-language-server" }
  cmd = { "mjmx", "lsp" },
  filetypes = { "mjml" },
  root_markers = { ".git", "package.json" },
})
vim.lsp.enable("mjmx")
```

### Neovim (older, `nvim-lspconfig` style)

```lua
vim.filetype.add({ extension = { mjml = "mjml" } })

local configs = require("lspconfig.configs")
local util = require("lspconfig.util")

configs.mjmx = {
  default_config = {
    -- or: cmd = { "mjmx-language-server" }
    cmd = { "mjmx", "lsp" },
    filetypes = { "mjml" },
    root_dir = util.root_pattern(".git", "package.json"),
  },
}
require("lspconfig").mjmx.setup({})
```

### VS Code

There is no published extension; wire it up from a minimal one. In
`package.json`:

```json
{
  "contributes": {
    "languages": [
      { "id": "mjml", "extensions": [".mjml"], "configuration": "./language-configuration.json" }
    ]
  },
  "activationEvents": ["onLanguage:mjml"]
}
```

and in the extension entry point:

```js
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;
function activate(context) {
  client = new LanguageClient(
    "mjmx",
    "mjmx language server",
    {
      // or: { command: "mjmx-language-server", transport: TransportKind.stdio }
      command: "mjmx",
      args: ["lsp"],
      transport: TransportKind.stdio,
    },
    { documentSelector: [{ scheme: "file", language: "mjml" }] },
  );
  context.subscriptions.push(client.start());
}
function deactivate() {
  return client && client.stop();
}
module.exports = { activate, deactivate };
```

### Helix

In `languages.toml`:

```toml
[language-server.mjmx]
command = "mjmx"
args = ["lsp"]
# or, for the standalone binary:
#   command = "mjmx-language-server"

[[language]]
name = "mjml"
scope = "source.mjml"
file-types = ["mjml"]
roots = [".git"]
language-servers = ["mjmx"]
```

### Zed / any other client

Anything that can launch a command and speak LSP over stdio works: run either
`mjmx lsp` or `mjmx-language-server` (both with no further arguments) and
associate it with `*.mjml`.

## Layout

- `src/line_index.rs` — byte offset ↔ UTF-16 `Position`, the conversion everything
  else depends on.
- `src/context.rs` — what the caret is sitting on, tolerant of half-typed markup
  (the AST cannot answer this mid-keystroke).
- `src/docs.rs` — the built-in component/attribute documentation table.
- `src/analysis.rs` — the pure feature implementations, all synchronous.
- `src/server.rs` — the `tower-lsp` adapter; the only async code in the crate.

Async and tokio stay confined to this crate: `mjmx_language_server::run()` is a
synchronous front door that builds its own runtime, mirroring `mjmx-dev-server`.
Both `src/main.rs` here and `mjmx lsp` in `apps/mjmx-cli` are one call into it.
