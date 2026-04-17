#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE/.."

echo "==> ensure cargo-component"
if ! command -v cargo-component &> /dev/null; then
  cargo install cargo-component --locked --version '^0.20'
fi

echo "==> ensure wasm32-wasip1 + wasm32-wasip2 targets"
rustup target add wasm32-wasip1 wasm32-wasip2 2>/dev/null || true

echo "==> build gtdx (needed by e2e)"
cargo build -p greentic-ext-cli

echo "==> AC reference extension build + install e2e"
./reference-extensions/adaptive-cards/tests/install_e2e.sh
