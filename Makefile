VALE ?= vale

.PHONY: help all clean test build build-python release lint lint-python
.PHONY: lint-whitaker typecheck fmt check-fmt markdownlint spellcheck spelling
.PHONY: spelling-config spelling-config-write spelling-phrase-check
.PHONY: spelling-helper-test nixie publish-check
.PHONY: forbid-async-trait vale update-ui-lints-lock test-workflow-contracts

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
RUFF_VERSION ?= 0.15.12
PATHSPEC_VERSION ?= 1.1.1
TYPOS_VERSION ?= 1.48.0
TYPOS_CONFIG_BUILDER_COMMIT := d6da92f02240a79a945c835f69bdd08a888da1d0
TYPOS_CONFIG_BUILDER_SOURCE := git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_COMMIT)
TYPOS_CONFIG_BUILDER := $(UV_ENV) $(UV) tool run --python 3.14 \
	--from "$(TYPOS_CONFIG_BUILDER_SOURCE)" typos-config-builder
SPELLING_PY_SRCS := \
	scripts/typos_rollout_check.py scripts/tests/test_typos_rollout_check.py
SPELLING_PY_TESTS := scripts/tests/test_typos_rollout_check.py
SPELLING_COVERAGE_ARGS := --cov=typos_rollout_check --cov-fail-under=90
SPELLING_HELPER_PYTEST = PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project \
	--python 3.14 --with pathspec==$(PATHSPEC_VERSION) --with pytest==9.0.2 \
	--with pytest-cov==7.0.0 python -m pytest
# Shared Markdown file list used by markdownlint and the spelling gate.
MD_FILES_FIND = find . -type f -name '*.md' -not -path '*/target/*' -not -path '*/node_modules/*' -print0
LADING_REF ?= d3217a599ea34adad6a6e3845845fff2fe923758
LADING_SPEC ?= lading @ git+https://github.com/leynos/lading@$(LADING_REF)
PYTHON_TARGETS ?= $(filter-out $(SPELLING_PY_SRCS),$(shell find scripts -type f -name "*.py" -print | sort))
PYLINT_TARGETS ?= $(PYTHON_TARGETS)
WHITAKER ?= whitaker

build: target/debug/$(APP) ## Build debug binary
build-python: pyproject.toml ## Build Python tooling environment
	$(UV_ENV) $(UV) sync --group python-tools
release: target/release/$(APP) ## Build release binary

all: release spelling ## Build the release binary and enforce spelling

clean: ## Remove build artefacts
	$(CARGO) clean
	rm -rf .uv-cache .uv-tools

test: build-python ## Run tests with warnings treated as errors
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) build --bin $(APP) --bin todo-cli $(BUILD_JOBS)
	if command -v cargo-nextest >/dev/null 2>&1; then \
		RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) nextest run $(CARGO_FLAGS) $(BUILD_JOBS); \
	else \
		RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) test $(CARGO_FLAGS) $(BUILD_JOBS); \
	fi
	# Exercise the Python documentation helpers alongside the Rust suite.
	$(UV_ENV) $(UV) run pytest scripts/tests

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: ## Run Clippy and the Whitaker Dylint suite with warnings denied
	$(CARGO) clippy $(CLIPPY_FLAGS)
	$(MAKE) lint-whitaker
	$(MAKE) lint-python
	python3 scripts/check_rs_file_lengths.py
	python3 scripts/check_users_guide_links.py
	python3 scripts/check_gpui_mapping_table.py
	python3 scripts/check_serial_nextest_matrix.py

lint-whitaker: ## Run the Whitaker Dylint suite with warnings denied
	RUSTFLAGS="$(RUST_FLAGS)" $(WHITAKER) --all -- $(CARGO_FLAGS)

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

markdownlint: spelling ## Lint Markdown files and enforce en-GB-oxendict spelling
	$(MD_FILES_FIND) | xargs -0 $(MDLINT)

spellcheck: spelling ## Compatibility alias for the repository spelling gate

spelling: spelling-phrase-check ## Enforce en-GB-oxendict in tracked text
	@git ls-files -z | xargs -0 -r env $(UV_ENV) \
		$(UV) tool run typos@$(TYPOS_VERSION) --config typos.toml --force-exclude --hidden

spelling-phrase-check: spelling-config ## Reject prohibited spelling phrases
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python 3.14 scripts/typos_rollout_check.py --repository .

spelling-config: spelling-helper-test ## Verify generated spelling configuration
	@git ls-files --error-unmatch typos.toml >/dev/null
	@$(TYPOS_CONFIG_BUILDER) --repository . --check

spelling-config-write: spelling-helper-test ## Generate spelling configuration
	@$(TYPOS_CONFIG_BUILDER) --repository .

spelling-helper-test: ## Validate the shared spelling-policy integration
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) format --isolated --target-version py313 --check $(SPELLING_PY_SRCS)
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) check --isolated --target-version py313 $(SPELLING_PY_SRCS)
	@$(SPELLING_HELPER_PYTEST) $(SPELLING_PY_TESTS) -c /dev/null --rootdir=. -p no:cacheprovider $(SPELLING_COVERAGE_ARGS)

nixie:
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	nixie --no-sandbox

publish-check: build-python ## Package crates in release order to validate publish readiness
	$(UV_ENV) $(UV) run --with "$(LADING_SPEC)" lading publish --workspace-root . --allow-unpublished-workspace-deps

test-workflow-contracts: ## Validate the mutation-testing caller contract
	$(UV_ENV) $(UV) run --with 'pytest>=8' --with 'pyyaml>=6' pytest tests/workflow_contracts -q

update-ui-lints-lock: ## Refresh ui_lints trybuild lockfile for `--locked` CI
	$(CARGO) generate-lockfile --manifest-path crates/rstest-bdd/tests/ui_lints/Cargo.toml

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'

vale: ## Check prose
	$(VALE) sync
	$(UV) run $(ACRONYM_SCRIPT)
	$(VALE) --no-global --output line .
