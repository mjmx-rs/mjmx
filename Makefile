.DEFAULT_GOAL := help
CARGO ?= cargo

.PHONY: help build release run test check fmt fmt-check clippy lint doc clean ci

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

build: ## Build the workspace
	$(CARGO) build --workspace

release: ## Build the workspace in release mode
	$(CARGO) build --workspace --release

run: ## Run the mrml CLI (ARGS=...)
	$(CARGO) run -p mrml-cli -- $(ARGS)

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

doc: ## Build workspace documentation
	$(CARGO) doc --workspace --no-deps

clean: ## Remove build artifacts
	$(CARGO) clean

ci: lint test ## Run lint and test (CI entrypoint)
