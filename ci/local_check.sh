#!/usr/bin/env bash
set -euo pipefail

# greentic-adaptive-cards-extension is a wasm32 cdylib and cannot be
# compiled or tested for native targets — exclude it from host-side steps.
WASM_CRATE="greentic-adaptive-cards-extension"

echo "==> cargo fmt"
cargo fmt --all -- --check

echo "==> cargo clippy"
cargo clippy --workspace --all-targets --all-features --locked \
  --exclude "$WASM_CRATE" -- -D warnings

echo "==> cargo test"
cargo test --workspace --all-features --locked \
  --exclude "$WASM_CRATE"

echo "==> cargo build (release)"
cargo build --workspace --locked --release \
  --exclude "$WASM_CRATE"

echo "All checks passed."
