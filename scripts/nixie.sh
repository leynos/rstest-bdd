#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

MERMAID_CLI_SPEC=${MERMAID_CLI_SPEC:-@mermaid-js/mermaid-cli}

if command -v npx >/dev/null 2>&1; then
    run=(npx --yes "$MERMAID_CLI_SPEC")
    if [ "${NIXIE_VERBOSE:-}" = "1" ]; then
        echo "Using npx to run $MERMAID_CLI_SPEC"
    fi
elif command -v bun >/dev/null 2>&1; then
    run=(bun x "$MERMAID_CLI_SPEC")
    if [ "${NIXIE_VERBOSE:-}" = "1" ]; then
        echo "Using bun x to run $MERMAID_CLI_SPEC"
    fi
else
    if [ "${NIXIE_VERBOSE:-}" = "1" ]; then
        echo "No npx or bun found for $MERMAID_CLI_SPEC"
    fi
    echo "nixie requires npx or bun. Install one to render Mermaid diagrams." >&2
    exit 1
fi

extra_flags=()
if [ -n "${MERMAID_EXTRA_FLAGS:-}" ]; then
    # Split MERMAID_EXTRA_FLAGS into an array for safe expansion.
    read -r -a extra_flags <<<"${MERMAID_EXTRA_FLAGS}"
fi

failed=0
while IFS= read -r -d '' f; do
    d="$(dirname "$f")"
    base="$(basename "$f")"
    out="$d/${base%.*}.svg"
    if ! "${run[@]}" "${extra_flags[@]}" -i "$f" -o "$out"; then
        echo "Mermaid render failed: $f" >&2
        failed=1
    fi
done < <(find . -type f -name '*.mmd' -not -path '*/target/*' -not -path '*/node_modules/*' -print0)

test "$failed" -eq 0
