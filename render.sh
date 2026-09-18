#!/usr/bin/env bash
# Run the DAG builder, then render every .dot it emitted to SVG.
set -euo pipefail

cd "$(dirname "$0")"
OUT_DIR="core/out"
FORMAT="${FORMAT:-svg}"

if ! command -v dot >/dev/null 2>&1; then
    echo "graphviz not found: install it (pacman -S graphviz)" >&2
    exit 1
fi

if [[ "${1:-}" != "--render-only" ]]; then
    cargo run --quiet
fi

shopt -s nullglob
dots=("$OUT_DIR"/*.dot)

if (( ${#dots[@]} == 0 )); then
    echo "no .dot files in $OUT_DIR" >&2
    echo "(a cycle may have stopped the run before output was written)" >&2
    exit 1
fi

for f in "${dots[@]}"; do
    out="${f%.dot}.$FORMAT"
    dot -T"$FORMAT" "$f" -o "$out"
    echo "rendered $out"
done
