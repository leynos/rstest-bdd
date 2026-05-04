VALE ?= vale

.PHONY: help all clean test build release lint typecheck fmt check-fmt markdownlint nixie publish-check forbid-async-trait vale

SHELL := bash
export PATH := $(HOME)/.cargo/bin:$(HOME)/.bun/bin:$(HOME)/.local/bin:$(PATH)
APP ?= cargo-bdd
CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(HOME)/.cargo/bin/cargo)
BUILD_JOBS ?=
RUST_FLAGS ?= -D warnings
CARGO_FLAGS ?= --workspace --all-targets --all-features
CLIPPY_FLAGS ?= $(CARGO_FLAGS) -- $(RUST_FLAGS)
MDLINT ?= $(or $(shell command -v markdownlint-cli2 2>/dev/null),$(HOME)/.bun/bin/markdownlint-cli2)
ACRONYM_SCRIPT ?= scripts/update_acronym_allowlist.py
UV ?= $(or $(shell command -v uv 2>/dev/null),$(HOME)/.local/bin/uv)
UVX ?= $(or $(shell command -v uvx 2>/dev/null),$(HOME)/.local/bin/uvx)

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artifacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) build --bin $(APP) --bin todo-cli $(BUILD_JOBS)
	if command -v cargo-nextest >/dev/null 2>&1; then \
		RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) nextest run $(CARGO_FLAGS) $(BUILD_JOBS); \
	else \
		RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) test $(CARGO_FLAGS) $(BUILD_JOBS); \
	fi
	# Exercise the Python release automation alongside the Rust suite.
	$(UV) run --with pytest --with cyclopts --with plumbum --with tomlkit \
		python -m pytest scripts/tests/publish_check

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: ## Run Clippy with warnings denied
	$(CARGO) clippy $(CLIPPY_FLAGS)
	find scripts -type f -name "*.py" -print0 | xargs -r -0 $(UVX) ruff check
	python3 scripts/check_rs_file_lengths.py

typecheck: ## Run cargo check with warnings denied
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) check $(CARGO_FLAGS) $(BUILD_JOBS)

forbid-async-trait: ## Ensure the async-trait crate and macro remain absent
	python3 scripts/check_forbidden_async_trait.py

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check
	find scripts -type f -name "*.py" -print0 | xargs -r -0 $(UVX) ruff format --check

markdownlint: ## Lint Markdown files
	find . -type f -name '*.md' -not -path '*/target/*' -not -path '*/node_modules/*' -print0 | xargs -0 $(MDLINT)

nixie:
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	nixie --no-sandbox

publish-check: ## Package crates in release order to validate publish readiness
	$(UV) run scripts/run_publish_check.py


help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'

vale: ## Check prose
	$(VALE) sync
	$(UV) run $(ACRONYM_SCRIPT)
	$(VALE) --no-global --output line .
