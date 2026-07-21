# AGENTS.md

Instructions for any AI agent working in this repo.

**Read [CLAUDE.md](./CLAUDE.md) first** — it holds the project context, mental model,
locked decisions, crate map, and milestone roadmap. This file is a short pointer so
non-Claude agents find the same context.

## Non-negotiables

- **Plan → approve → code.** Show a plan and get explicit user approval before writing
  code, creating files, or running git. Never assume the user's goal or expected output.
- **Architecture:** oxc-style toolchain for MJML-as-a-language. Arena AST (bumpalo) with
  spans and interned `Atom`s. Components are render impls inside `mjmx-codegen`, not crates.
- **Crates:** hyphen names, grow per-milestone, workspace must always compile.
- **Tests:** golden `.mjml`→`.html` snapshots compared semantically (HTML/CSS-aware).
- Build via the Makefile (`make build/test/lint/ci`).

See CLAUDE.md for the full crate map and roadmap.
