.PHONY: help all clean test build release lint fmt check-fmt markdownlint nixie

APP ?= rstest-bdd
CARGO ?= cargo
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings
MDLINT ?= markdownlint

build: target/debug/$(APP) ## Build debug binary
release: target/release/$(APP) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artifacts
	$(CARGO) clean

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="-D warnings" $(CARGO) test --all-targets --all-features $(BUILD_JOBS)

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
	@command -v npx >/dev/null 2>&1 || { echo "nixie requires npx (Node.js). Install Node.js or adjust the CI image."; exit 1; }
	find . -type f -name '*.mmd' -not -path './target/*' -not -path './node_modules/*' -print0 | \
                xargs -0 -I{} sh -c 'd=$$(dirname "$$1"); npx --yes @mermaid-js/mermaid-cli -i "$$1" -o "$$d"' _ {} || true

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
