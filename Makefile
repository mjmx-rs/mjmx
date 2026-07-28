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
# One cargo build feeds three `wasm-bindgen` runs — `nodejs` (CommonJS, `require`),
# `bundler` (ESM for webpack/vite/rollup) and `web` (ESM for browsers/Deno without a
# bundler) — each into its own `pkg/<flavor>` dir, wired up by the package.json
# `exports` map. The three `_bg.wasm` differ (per-target glue), so each is optimised.
WASM_DIR := crates/mjmx-wasm
WASM_TARGET := wasm32-unknown-unknown
WASM_LIB := target/$(WASM_TARGET)/release/mjmx_wasm.wasm
WASM_OUT := $(WASM_DIR)/pkg
WASM_BG := $(WASM_OUT)/node/mjmx_wasm_bg.wasm $(WASM_OUT)/bundler/mjmx_wasm_bg.wasm $(WASM_OUT)/web/mjmx_wasm_bg.wasm

# Read the pin out of the workspace manifest rather than duplicating it, so the
# `wasm-tools` check can never drift from the `wasm-bindgen = "=x.y.z"` dependency.
WASM_BINDGEN_VERSION := $(shell sed -n 's/^wasm-bindgen = "=\(.*\)"$$/\1/p' Cargo.toml)

# `wasm-opt` (binaryen) shrinks the wasm-bindgen output by roughly a third. It is
# an *optional* tool: `make wasm` runs it when it is on PATH and prints a hint
# (rather than failing) when it is not, so a plain build never depends on it.
# Install with `cargo install wasm-opt`, `npm i -g binaryen`, or a distro package.
# `-Oz` optimises for size. Always re-run `make wasm-test` after optimising.
WASM_OPT ?= wasm-opt
WASM_OPT_FLAGS ?= -Oz

BENCH_DIR := benches/perf

.PHONY: help build release run test check fmt fmt-check clippy lint doc clean ci preflight coverage coverage-html hooks napi napi-test wasm wasm-tools wasm-opt wasm-release wasm-test lsp lsp-test cli-lsp-test bench bench-write

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

wasm-tools: ## Verify the wasm toolchain (rustup target + pinned wasm-bindgen CLI)
	@if command -v rustup >/dev/null 2>&1 && \
	    ! rustup target list --installed 2>/dev/null | grep -qx '$(WASM_TARGET)'; then \
	  echo "missing rustup target $(WASM_TARGET) — install with:" >&2; \
	  echo "  rustup target add $(WASM_TARGET)" >&2; \
	  exit 1; \
	fi
	@command -v wasm-bindgen >/dev/null 2>&1 || { \
	  echo "wasm-bindgen CLI not found — install the pinned version with:" >&2; \
	  echo "  cargo install wasm-bindgen-cli --version $(WASM_BINDGEN_VERSION)" >&2; \
	  exit 1; }
	@have=$$(wasm-bindgen --version | awk '{ print $$2 }'); \
	if [ "$$have" != "$(WASM_BINDGEN_VERSION)" ]; then \
	  echo "wasm-bindgen CLI is $$have but the workspace pins =$(WASM_BINDGEN_VERSION)." >&2; \
	  echo "The generated glue must match the crate exactly; install with:" >&2; \
	  echo "  cargo install wasm-bindgen-cli --version $(WASM_BINDGEN_VERSION) --force" >&2; \
	  exit 1; \
	fi

wasm: wasm-tools ## Build the WebAssembly package (node + bundler + web; wasm-opt applied when available)
	$(CARGO) build -p mjmx-wasm --target $(WASM_TARGET) --release
	wasm-bindgen --target nodejs  --out-dir $(WASM_OUT)/node    --out-name mjmx_wasm $(WASM_LIB)
	wasm-bindgen --target bundler --out-dir $(WASM_OUT)/bundler --out-name mjmx_wasm $(WASM_LIB)
	wasm-bindgen --target web     --out-dir $(WASM_OUT)/web     --out-name mjmx_wasm $(WASM_LIB)
	@$(MAKE) --no-print-directory wasm-opt
	@echo "staged $(WASM_OUT) (node/ bundler/ web/)"

wasm-opt: ## Shrink the staged wasm with binaryen (no-op if wasm-opt is missing)
	@if command -v $(WASM_OPT) >/dev/null 2>&1; then \
	  for f in $(WASM_BG); do \
	    [ -f $$f ] || continue; \
	    before=$$(wc -c < $$f); \
	    $(WASM_OPT) $(WASM_OPT_FLAGS) $$f -o $$f.opt && mv $$f.opt $$f; \
	    after=$$(wc -c < $$f); \
	    awk -v b="$$before" -v a="$$after" -v n="$$f" 'BEGIN { \
	      printf "wasm-opt $(WASM_OPT_FLAGS): %s %.1f KiB -> %.1f KiB (-%.1f%%)\n", \
	        n, b/1024, a/1024, (b-a)*100/b }'; \
	  done; \
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

bench: ## Print the live performance table (mjmx, mrml, mjml 4.18 + 5.4)
	cd $(BENCH_DIR) && npm install --silent && $(CARGO) run --release -- $(ARGS)

bench-write: ## Run the benchmark and splice the table into README.md
	cd $(BENCH_DIR) && npm install --silent && $(CARGO) run --release -- --write $(ARGS)

ci: lint test ## Run lint and test (CI entrypoint)

# The project has no CI. `preflight` is the pre-push gate: everything `ci` runs,
# plus the two node smoke tests that live outside `cargo test --workspace`.
preflight: lint test napi-test wasm-test ## Full local gate — run before every push
	@echo "preflight: all checks passed"

coverage: ## Per-crate line coverage report (needs `cargo install cargo-llvm-cov`)
	@$(CARGO) llvm-cov --version >/dev/null 2>&1 || { \
		echo "cargo-llvm-cov not found — install with: cargo install cargo-llvm-cov"; \
		exit 1; \
	}
	$(CARGO) llvm-cov --workspace --summary-only

coverage-html: ## Same as `coverage`, as a browsable HTML report
	$(CARGO) llvm-cov --workspace --html
	@echo "open target/llvm-cov/html/index.html"

hooks: ## Install the git hooks (pre-push runs `make preflight`)
	@install -m 755 scripts/pre-push .git/hooks/pre-push
	@echo "installed .git/hooks/pre-push"
