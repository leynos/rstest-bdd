#!/usr/bin/env bash
set -euo pipefail

if command -v npx >/dev/null 2>&1; then
    run='npx --yes @mermaid-js/mermaid-cli'
elif command -v bun >/dev/null 2>&1; then
    run='bun x @mermaid-js/mermaid-cli'
else
    echo "nixie requires npx or bun. Install one to render Mermaid diagrams."
    exit 1
fi

failed=0
while IFS= read -r -d '' f; do
    d="$(dirname "$f")"
    if ! $run -i "$f" -o "$d"; then
        echo "Mermaid render failed: $f"
        failed=1
    fi
done < <(find . -type f -name '*.mmd' -not -path './target/*' -not -path './node_modules/*' -print0)

test "$failed" -eq 0
