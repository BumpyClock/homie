#!/usr/bin/env bash
set -euo pipefail

root_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root_dir"

if [[ -n "${HOMIE_GATEWAY_URL:-}" && -z "${VITE_GATEWAY_URL:-}" ]]; then
  export VITE_GATEWAY_URL="$HOMIE_GATEWAY_URL"
fi

cleanup() {
  jobs -p | xargs kill 2>/dev/null || true
}
trap cleanup EXIT INT TERM

cargo run -p homie-gateway &

pnpm -C src/web dev &

wait -n
