VALE ?= vale

.PHONY: help all clean test build build-python release lint lint-python
.PHONY: lint-whitaker typecheck fmt check-fmt markdownlint spellcheck spelling
.PHONY: spelling-config spelling-config-write spelling-phrase-check
.PHONY: spelling-helper-test nixie publish-check
.PHONY: check-published-gpui stage-published-gpui-e2e e2e-published-gpui
.PHONY: forbid-async-trait vale update-ui-lints-lock update-published-gpui-0-2-2-lock update-published-gpui-e2e-lock
.PHONY: test-workflow-contracts

SHELL := bash
export PATH := $(HOME)/.cargo/bin:$(HOME)/.bun/bin:$(HOME)/.local/bin:$(PATH)
APP ?= cargo-bdd
CARGO ?= $(or $(shell command -v cargo 2>/dev/null),$(HOME)/.cargo/bin/cargo)
# `.rustfmt.toml` enables unstable rustfmt options, which only a nightly
# rustfmt understands. Formatting therefore runs on nightly while every other
# Cargo invocation stays on the pinned stable toolchain. The nightly is pinned
# to a date so that an upstream rustfmt change cannot reformat the tree out
# from under unrelated pull requests.
FMT_TOOLCHAIN ?= nightly-2026-08-07
CARGO_FMT ?= $(CARGO) +$(FMT_TOOLCHAIN) fmt
BUILD_JOBS ?=
RUST_FLAGS ?= -D warnings
RUSTDOC_FLAGS ?= --cfg docsrs -D warnings
CARGO_FLAGS ?= --workspace --all-targets --all-features
CLIPPY_FLAGS ?= $(CARGO_FLAGS) -- $(RUST_FLAGS)
MDLINT ?= $(or $(shell command -v markdownlint-cli2 2>/dev/null),$(HOME)/.bun/bin/markdownlint-cli2)
ACRONYM_SCRIPT ?= scripts/update_acronym_allowlist.py
UV ?= $(or $(shell command -v uv 2>/dev/null),$(HOME)/.local/bin/uv)
UVX ?= $(or $(shell command -v uvx 2>/dev/null),$(HOME)/.local/bin/uvx)
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
PROJECT_PYTHON = $(UV_ENV) $(UV) run --python 3.14 python
# Keep the Makefile and CI pins aligned; workflow-contract tests protect the
# shared value without making a specific release part of the test contract.
RUFF_VERSION ?= 0.16.4
RUFF = $(UV_ENV) $(UV) tool run --python 3.14 ruff@$(RUFF_VERSION) --config pyproject.toml
TY_VERSION ?= 0.0.74
TY = $(UV_ENV) $(UV) run --with ty==$(TY_VERSION) ty
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
MD_FILES_FIND = find . -type f -name '*.md' -not -path '*/target/*' -not -path '*/node_modules/*' -not -path './.vtcode/*' -print0
LADING_REF ?= e0a8d43fa3d6d7598cad0d4c25883e7ea625feb9
LADING_SPEC ?= lading @ git+https://github.com/leynos/lading@$(LADING_REF)
PYTHON_TARGETS ?= $(filter-out $(SPELLING_PY_SRCS),$(shell find scripts tests/workflow_contracts -type f -name "*.py" -print | sort))
PYLINT_PYTHON ?= pypy
PYLINT_TARGETS ?= scripts tests/workflow_contracts
PYLINT_PYPY_SHIM_REF ?= 726d09f968b4d729ee4b29c71fc732e744854f3b
PYLINT_PYPY_SHIM = git+https://github.com/leynos/pylint-pypy-shim.git@$(PYLINT_PYPY_SHIM_REF)
DF12_PYTHON_LINTS_REF ?= v0.3.0
DF12_PYTHON_LINTS = git+https://github.com/leynos/df12-python-lints.git@$(DF12_PYTHON_LINTS_REF)
DF12_PYTHON ?= 3.14
PYLINT = $(UV_ENV) $(UV) tool run --python $(PYLINT_PYTHON) \
	--from '$(PYLINT_PYPY_SHIM)' pylint-pypy
DF12_PYLINT_MESSAGES = R9101,C9102,R9103,R9104,C9105,C9106,C9107,R9108,R9109,R9110,R9111,R9112,C9112
DF12_PYLINT = $(UV_ENV) $(UV) run --python $(DF12_PYTHON) pylint \
	--py-version=$(DF12_PYTHON) --disable=all --load-plugins=df12_python_lints \
	--enable=$(DF12_PYLINT_MESSAGES)
AMBRLEAKS = $(UV_ENV) $(UV) tool run --python $(DF12_PYTHON) \
	--from '$(DF12_PYTHON_LINTS)' ambrleaks
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
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) test --doc --workspace --all-features $(BUILD_JOBS)
	# Exercise the Python documentation helpers alongside the Rust suite.
	$(UV_ENV) $(UV) run pytest scripts/tests

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: ## Run Clippy and the Whitaker Dylint suite with warnings denied
	$(CARGO) clippy $(CLIPPY_FLAGS)
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --workspace --no-deps
	$(MAKE) lint-whitaker
	$(MAKE) lint-python
	$(PROJECT_PYTHON) scripts/check_rs_file_lengths.py
	$(PROJECT_PYTHON) scripts/check_users_guide_links.py
	$(PROJECT_PYTHON) scripts/check_gpui_mapping_table.py
	$(PROJECT_PYTHON) scripts/check_serial_nextest_matrix.py

lint-whitaker: ## Run the Whitaker Dylint suite with warnings denied
	RUSTFLAGS="$(RUST_FLAGS)" $(WHITAKER) --all -- $(CARGO_FLAGS)

lint-python: build-python ## Run Python linters
	$(RUFF) check $(PYTHON_TARGETS)
	$(PYLINT) $(PYLINT_TARGETS)
	$(DF12_PYLINT) $(PYLINT_TARGETS)
	$(AMBRLEAKS) tests

typecheck: build-python ## Run cargo and Python type checks with warnings denied
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) check $(CARGO_FLAGS) $(BUILD_JOBS)
	$(TY) check --python-version 3.14 $(PYTHON_TARGETS)

PUBLISHED_GPUI_MANIFEST := tests/fixtures/published-gpui-0-2-2/Cargo.toml
PUBLISHED_GPUI_E2E_DIR := tests/fixtures/published-gpui-e2e
PUBLISHED_GPUI_E2E_STAGE_DIR := target/published-gpui-e2e
PUBLISHED_GPUI_E2E_VERSION := 0.6.0-beta4
PUBLISHED_GPUI_E2E_PACKAGES := \
	rstest-bdd-patterns \
	rstest-bdd-policy \
	rstest-bdd-harness \
	rstest-bdd-macros \
	rstest-bdd \
	rstest-bdd-harness-gpui
PUBLISHED_GPUI_E2E_PACKAGE_PATCHES := $(foreach package,$(PUBLISHED_GPUI_E2E_PACKAGES),\
	--config 'patch.crates-io.$(package).path="$(CURDIR)/crates/$(package)"')

check-published-gpui: ## Compile the published gpui 0.2.2 documentation fixture
	# This nested workspace bypasses the root workspace's vendored gpui path.
	# CI exports RUSTFLAGS=-D warnings job-wide; set it here too so an unused
	# import or dead helper fails locally rather than only on CI. Note this
	# does not catch a write-only struct field: rustc's dead_code pass treats
	# `state.field = v` as a use, so such a field warns in neither place.
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) check --locked \
		--manifest-path $(PUBLISHED_GPUI_MANIFEST)

stage-published-gpui-e2e: ## Package first-party crates for the published GPUI E2E fixture
	# Cargo's package manifest removes workspace path overrides, including the
	# vendored GPUI shim. Extracting the packages makes that published surface
	# available to the standalone fixture without changing the root workspace.
	# Temporary patches merely let Cargo package unreleased beta dependencies;
	# they do not appear in the normalized package manifests being extracted.
	rm -rf $(PUBLISHED_GPUI_E2E_STAGE_DIR)
	mkdir -p $(PUBLISHED_GPUI_E2E_STAGE_DIR)
	set -e; \
	for package in $(PUBLISHED_GPUI_E2E_PACKAGES); do \
		$(CARGO) package --allow-dirty --no-verify --package "$$package" \
			$(PUBLISHED_GPUI_E2E_PACKAGE_PATCHES); \
		tar -xzf "target/package/$$package-$(PUBLISHED_GPUI_E2E_VERSION).crate" \
			-C $(PUBLISHED_GPUI_E2E_STAGE_DIR); \
	done
	@sed -n '/^\[dependencies\.gpui\]/,/^\[/p' \
		$(PUBLISHED_GPUI_E2E_STAGE_DIR)/rstest-bdd-harness-gpui-$(PUBLISHED_GPUI_E2E_VERSION)/Cargo.toml \
		| grep -q 'version = "0.2.2"'
	@! sed -n '/^\[dependencies\.gpui\]/,/^\[/p' \
		$(PUBLISHED_GPUI_E2E_STAGE_DIR)/rstest-bdd-harness-gpui-$(PUBLISHED_GPUI_E2E_VERSION)/Cargo.toml \
		| grep -q '^path = '

e2e-published-gpui: stage-published-gpui-e2e ## Run the nightly published-GPUI stateful scenario
	# `cd` lets rustup discover this fixture's pinned nightly override.
	cd $(PUBLISHED_GPUI_E2E_DIR) && RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) test --locked

forbid-async-trait: ## Ensure the async-trait crate and macro remain absent
	$(PROJECT_PYTHON) scripts/check_forbidden_async_trait.py

fmt: build-python ## Format Rust and Markdown sources
	$(CARGO_FMT) --all
	# Published GPUI fixtures are their own workspaces, so `--all` misses them.
	$(CARGO_FMT) --manifest-path $(PUBLISHED_GPUI_MANIFEST)
	$(CARGO_FMT) --manifest-path $(PUBLISHED_GPUI_E2E_DIR)/Cargo.toml
	$(RUFF) format $(PYTHON_TARGETS)
	$(RUFF) check --select I --fix $(PYTHON_TARGETS)
	mdformat-all

check-fmt: build-python ## Verify formatting
	$(CARGO_FMT) --all -- --check
	$(CARGO_FMT) --manifest-path $(PUBLISHED_GPUI_MANIFEST) -- --check
	$(CARGO_FMT) --manifest-path $(PUBLISHED_GPUI_E2E_DIR)/Cargo.toml -- --check
	$(RUFF) format --check $(PYTHON_TARGETS)

markdownlint: spelling ## Lint Markdown files and enforce en-GB-oxendict spelling
	$(MD_FILES_FIND) | xargs -0 $(MDLINT)

spellcheck: spelling ## Compatibility alias for the repository spelling gate

spelling: spelling-phrase-check ## Enforce en-GB-oxendict in tracked text
	@files=(); \
	while IFS= read -r -d '' path; do \
		[[ ! -e "$$path" ]] || files+=("$$path"); \
	done < <(git ls-files -z); \
	if (( $${#files[@]} > 0 )); then \
		printf '%s\0' "$${files[@]}" | \
			xargs -0 env $(UV_ENV) $(UV) tool run typos@$(TYPOS_VERSION) \
				--config typos.toml --force-exclude --hidden; \
	fi

spelling-phrase-check: spelling-config ## Reject prohibited spelling phrases
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python 3.14 scripts/typos_rollout_check.py --repository .

spelling-config: spelling-helper-test ## Verify generated spelling configuration
	@git ls-files --error-unmatch typos.toml >/dev/null
	@$(TYPOS_CONFIG_BUILDER) --repository . --check

spelling-config-write: spelling-helper-test ## Generate spelling configuration
	@$(TYPOS_CONFIG_BUILDER) --repository .

spelling-helper-test: ## Validate the shared spelling-policy integration
	@$(UV_ENV) $(UV) tool run --python 3.14 ruff@$(RUFF_VERSION) format --isolated --target-version py314 --check $(SPELLING_PY_SRCS)
	@$(UV_ENV) $(UV) tool run --python 3.14 ruff@$(RUFF_VERSION) check --isolated --target-version py314 $(SPELLING_PY_SRCS)
	@$(SPELLING_HELPER_PYTEST) $(SPELLING_PY_TESTS) -c /dev/null --rootdir=. -p no:cacheprovider $(SPELLING_COVERAGE_ARGS)

nixie:
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	nixie --no-sandbox

# Lading validates the standalone fixture manifest, whose patch paths require
# the staged package artefacts during the publish dry run.
publish-check: build-python stage-published-gpui-e2e ## Package crates in release order to validate publish readiness
	$(UV_ENV) $(UV) run --with "$(LADING_SPEC)" lading publish --workspace-root . --allow-unpublished-workspace-deps

test-workflow-contracts: ## Validate the mutation-testing caller contract
	$(UV_ENV) $(UV) run --with 'pytest>=8' --with 'pyyaml>=6' pytest tests/workflow_contracts -q

update-ui-lints-lock: ## Refresh ui_lints trybuild lockfile for `--locked` CI
	$(CARGO) generate-lockfile --manifest-path crates/rstest-bdd/tests/ui_lints/Cargo.toml

update-published-gpui-0-2-2-lock: ## Refresh the published GPUI 0.2.2 fixture lockfile
	$(CARGO) generate-lockfile --manifest-path $(PUBLISHED_GPUI_MANIFEST)

update-published-gpui-e2e-lock: stage-published-gpui-e2e ## Refresh the published GPUI E2E fixture lockfile
	cd $(PUBLISHED_GPUI_E2E_DIR) && $(CARGO) generate-lockfile

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'

vale: ## Check prose
	$(VALE) sync
	$(PROJECT_PYTHON) $(ACRONYM_SCRIPT)
	$(VALE) --no-global --output line .

# Opt-in accelerated debug builds (Cranelift + mold); requires a nightly
# toolchain. See AGENTS.md and tools/dev-fast/config.toml.
DEV_FAST_TOOLCHAIN ?= nightly-2026-08-16
DEV_FAST_CONFIG ?= tools/dev-fast/config.toml

.PHONY: dev-build dev-test
dev-build: ## Build debug binaries with Cranelift and mold
	$(CARGO) +$(DEV_FAST_TOOLCHAIN) --config "$(DEV_FAST_CONFIG)" build

dev-test: ## Run tests with Cranelift and mold
	$(CARGO) +$(DEV_FAST_TOOLCHAIN) --config "$(DEV_FAST_CONFIG)" test
