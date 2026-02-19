#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOOKS_DIR="$ROOT/.githooks"

if ! git -C "$ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "Not a git repository: $ROOT"
    exit 1
fi

chmod +x "$HOOKS_DIR/pre-commit"
git -C "$ROOT" config core.hooksPath .githooks

echo "Git hooks installed. core.hooksPath=.githooks"
echo "pre-commit will run homie-core clippy when relevant Rust/core files are staged."

