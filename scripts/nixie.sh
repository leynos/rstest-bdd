#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

MERMAID_CLI_SPEC=${MERMAID_CLI_SPEC:-@mermaid-js/mermaid-cli}

if command -v npx >/dev/null 2>&1; then
    run=(npx --yes "$MERMAID_CLI_SPEC")
elif command -v bun >/dev/null 2>&1; then
    run=(bun x "$MERMAID_CLI_SPEC")
else
    echo "nixie requires npx or bun. Install one to render Mermaid diagrams." >&2
    exit 1
fi

failed=0
while IFS= read -r -d '' f; do
    d="$(dirname "$f")"
    base="$(basename "$f")"
    out="$d/${base%.*}.svg"
    if ! "${run[@]}" -i "$f" -o "$out"; then
        echo "Mermaid render failed: $f" >&2
        failed=1
    fi
done < <(find . -type f -name '*.mmd' -not -path '*/target/*' -not -path '*/node_modules/*' -print0)

test "$failed" -eq 0
