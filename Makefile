.PHONY: help all clean test build release lint fmt check-fmt markdownlint nixie

SHELL := bash
APP ?= rstest-bdd
CARGO ?= cargo
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --workspace --all-targets --all-features -- -D warnings
MDLINT ?= markdownlint

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artifacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="-D warnings" $(CARGO) test --workspace --all-targets --all-features $(BUILD_JOBS)

target/%/$(APP): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(APP)

lint: ## Run Clippy with warnings denied
	$(CARGO) clippy $(CLIPPY_FLAGS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

markdownlint: ## Lint Markdown files
	find . -type f -name '*.md' -not -path './target/*' -print0 | xargs -0 $(MDLINT)

nixie: ## Render Mermaid diagrams from .mmd files (writes .svg next to sources)
	@if command -v npx >/dev/null 2>&1; then \
		run='npx --yes @mermaid-js/mermaid-cli'; \
	elif command -v bun >/dev/null 2>&1; then \
		run='bun x @mermaid-js/mermaid-cli'; \
	else \
		echo "nixie requires npx or bun. Install one to render Mermaid diagrams."; \
		exit 1; \
	fi; \
	failed=0; \
	find . -type f -name '*.mmd' -not -path './target/*' -not -path './node_modules/*' -print0 | \
	while IFS= read -r -d '' f; do \
		d="$$(dirname "$$f")"; \
		if ! $$run -i "$$f" -o "$$d"; then \
		echo "Mermaid render failed: $$f"; \
		failed=1; \
		fi; \
	done; \
	test "$$failed" -eq 0

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
