VALE ?= vale

.PHONY: help all clean test build build-python release lint lint-python typecheck fmt check-fmt markdownlint nixie publish-check forbid-async-trait vale update-ui-lints-lock

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
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
LADING_REF ?= d3217a599ea34adad6a6e3845845fff2fe923758
LADING_SPEC ?= lading @ git+https://github.com/leynos/lading@$(LADING_REF)
PYTHON_TARGETS ?= $(shell find scripts -maxdepth 1 -type f -name "*.py" -print | sort)
PYLINT_TARGETS ?= $(PYTHON_TARGETS)

build: target/debug/$(APP) ## Build debug binary
build-python: pyproject.toml ## Build Python tooling environment
	$(UV_ENV) $(UV) sync --group python-tools
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artifacts
	$(CARGO) clean

test: build-python ## Run tests with warnings treated as errors
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) build --bin $(APP) --bin todo-cli $(BUILD_JOBS)
	if command -v cargo-nextest >/dev/null 2>&1; then \
		RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) nextest run $(CARGO_FLAGS) $(BUILD_JOBS); \
	else \
		RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) test $(CARGO_FLAGS) $(BUILD_JOBS); \
	fi
	# Exercise the Python documentation helpers alongside the Rust suite.
	$(UV_ENV) $(UV) run pytest scripts/tests/test_check_users_guide_links.py \
		scripts/tests/test_check_gpui_mapping_table.py

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: ## Run Clippy with warnings denied
	$(CARGO) clippy $(CLIPPY_FLAGS)
	$(MAKE) lint-python
	python3 scripts/check_rs_file_lengths.py
	python3 scripts/check_users_guide_links.py
	python3 scripts/check_gpui_mapping_table.py

lint-python: build-python ## Run Python linters
	$(UV_ENV) $(UV) run ruff check $(PYTHON_TARGETS)
	$(UV_ENV) $(UV) run pylint $(PYLINT_TARGETS)

typecheck: build-python ## Run cargo and Python type checks with warnings denied
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) check $(CARGO_FLAGS) $(BUILD_JOBS)
	$(UV_ENV) $(UV) run ty check $(PYTHON_TARGETS)

forbid-async-trait: ## Ensure the async-trait crate and macro remain absent
	python3 scripts/check_forbidden_async_trait.py

fmt: build-python ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	$(UV_ENV) $(UV) run ruff format $(PYTHON_TARGETS)
	$(UV_ENV) $(UV) run ruff check --select I --fix $(PYTHON_TARGETS)
	mdformat-all

check-fmt: build-python ## Verify formatting
	$(CARGO) fmt --all -- --check
	$(UV_ENV) $(UV) run ruff format --check $(PYTHON_TARGETS)

markdownlint: ## Lint Markdown files
	find . -type f -name '*.md' -not -path '*/target/*' -not -path '*/node_modules/*' -print0 | xargs -0 $(MDLINT)

nixie:
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	nixie --no-sandbox

publish-check: build-python ## Package crates in release order to validate publish readiness
	$(UV_ENV) $(UV) run --with "$(LADING_SPEC)" lading publish --workspace-root . --allow-unpublished-workspace-deps

update-ui-lints-lock: ## Refresh ui_lints trybuild lockfile for `--locked` CI
	$(CARGO) generate-lockfile --manifest-path crates/rstest-bdd/tests/ui_lints/Cargo.toml

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'

vale: ## Check prose
	$(VALE) sync
	$(UV) run $(ACRONYM_SCRIPT)
	$(VALE) --no-global --output line .
