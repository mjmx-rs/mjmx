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

.PHONY: help build release run test check fmt fmt-check clippy lint doc clean ci napi napi-test

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

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

doc: ## Build workspace documentation
	$(CARGO) doc --workspace --no-deps

clean: ## Remove build artifacts
	$(CARGO) clean

ci: lint test ## Run lint and test (CI entrypoint)
