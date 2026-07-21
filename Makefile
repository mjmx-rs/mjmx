.DEFAULT_GOAL := help
CARGO ?= cargo
NODE ?= node

# Node addon naming: the cargo cdylib is copied next to the npm package as
# `mjmx.<platform>-<arch>[-gnu].node`, the filename `crates/mjmx-napi/index.js`
# looks for first.
NAPI_DIR := crates/mjmx-napi
NAPI_EXT := $(if $(filter Darwin,$(shell uname -s)),dylib,so)
NAPI_LIB := target/release/libmjmx_napi.$(NAPI_EXT)
NAPI_TRIPLE := $(shell $(NODE) -p "process.platform + '-' + process.arch + (process.platform === 'linux' ? '-gnu' : '')" 2>/dev/null)
NAPI_OUT := $(NAPI_DIR)/mjmx.$(if $(NAPI_TRIPLE),$(NAPI_TRIPLE),unknown).node

# WebAssembly package: cargo emits a bare module, `wasm-bindgen` generates the JS
# glue + .d.ts next to it. The CLI version must match the `wasm-bindgen` crate
# version pinned in the workspace Cargo.toml (see crates/mjmx-wasm/README.md).
WASM_DIR := crates/mjmx-wasm
WASM_TARGET := wasm32-unknown-unknown
WASM_LIB := target/$(WASM_TARGET)/release/mjmx_wasm.wasm
WASM_OUT := $(WASM_DIR)/pkg
WASM_BG := $(WASM_OUT)/mjmx_wasm_bg.wasm

# `wasm-opt` (binaryen) shrinks the wasm-bindgen output by roughly a third. It is
# an *optional* tool: `make wasm` runs it when it is on PATH and prints a hint
# (rather than failing) when it is not, so a plain build never depends on it.
# Install with `cargo install wasm-opt`, `npm i -g binaryen`, or a distro package.
# `-Oz` optimises for size. Always re-run `make wasm-test` after optimising.
WASM_OPT ?= wasm-opt
WASM_OPT_FLAGS ?= -Oz

.PHONY: help build release run test check fmt fmt-check clippy lint doc clean ci napi napi-test wasm wasm-opt wasm-release wasm-test lsp lsp-test cli-lsp-test

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

build: ## Build the workspace
	$(CARGO) build --workspace

release: ## Build the workspace in release mode
	$(CARGO) build --workspace --release

run: ## Run the mjmx CLI (ARGS=...)
	$(CARGO) run -p mjmx-cli -- $(ARGS)

test: ## Run all workspace tests
	$(CARGO) test --workspace

check: ## Type-check the workspace and all targets
	$(CARGO) check --workspace --all-targets

fmt: ## Format all code
	$(CARGO) fmt --all

fmt-check: ## Check formatting without writing changes
	$(CARGO) fmt --all --check

clippy: ## Run clippy with warnings denied
	$(CARGO) clippy --workspace --all-targets -- -D warnings

lint: fmt-check clippy ## Run fmt-check and clippy

napi: ## Build the Node addon and stage it as a loadable .node
	$(CARGO) build -p mjmx-napi --release
	cp $(NAPI_LIB) $(NAPI_OUT)
	@echo "staged $(NAPI_OUT)"

napi-test: napi ## Run the Node addon smoke test
	$(NODE) $(NAPI_DIR)/test/smoke.mjs

wasm: ## Build the WebAssembly package (wasm-opt applied when available)
	$(CARGO) build -p mjmx-wasm --target $(WASM_TARGET) --release
	wasm-bindgen --target nodejs --out-dir $(WASM_OUT) --out-name mjmx_wasm $(WASM_LIB)
	@$(MAKE) --no-print-directory wasm-opt
	@echo "staged $(WASM_OUT)"

wasm-opt: ## Shrink the staged wasm with binaryen (no-op if wasm-opt is missing)
	@if command -v $(WASM_OPT) >/dev/null 2>&1; then \
	  before=$$(wc -c < $(WASM_BG)); \
	  $(WASM_OPT) $(WASM_OPT_FLAGS) $(WASM_BG) -o $(WASM_BG).opt && mv $(WASM_BG).opt $(WASM_BG); \
	  after=$$(wc -c < $(WASM_BG)); \
	  awk -v b=$$before -v a=$$after 'BEGIN { \
	    printf "wasm-opt $(WASM_OPT_FLAGS): %.1f KiB -> %.1f KiB (-%.1f%%)\n", \
	      b/1024, a/1024, (b-a)*100/b }'; \
	else \
	  echo "wasm-opt not found — skipping size optimisation."; \
	  echo "  install binaryen to shrink the artifact: cargo install wasm-opt"; \
	fi

wasm-release: ## Build the wasm package, requiring wasm-opt, and smoke-test it
	@command -v $(WASM_OPT) >/dev/null 2>&1 || { \
	  echo "wasm-release requires wasm-opt (binaryen); install it with: cargo install wasm-opt" >&2; \
	  exit 1; }
	@$(MAKE) --no-print-directory wasm-test

wasm-test: wasm ## Run the wasm smoke test against the staged (optimised) artifact
	$(NODE) $(WASM_DIR)/test/smoke.mjs

lsp: ## Build the language server
	$(CARGO) build -p mjmx-language-server --release
	@echo "built target/release/mjmx-language-server"

lsp-test: ## Run the language server smoke test
	$(CARGO) test -p mjmx-language-server --test lsp_smoke -- --nocapture

cli-lsp-test: ## Run the same LSP session through the CLI's `mjmx lsp`
	$(CARGO) test -p mjmx-cli --test lsp_stdio -- --nocapture

doc: ## Build workspace documentation
	$(CARGO) doc --workspace --no-deps

clean: ## Remove build artifacts
	$(CARGO) clean

ci: lint test ## Run lint and test (CI entrypoint)
