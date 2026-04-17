#!/usr/bin/env bash
set -euo pipefail

# greentic-adaptive-cards-extension is a wasm32 cdylib and cannot be
# compiled or tested for native targets — exclude it from host-side steps.
WASM_CRATE="greentic-adaptive-cards-extension"

echo "==> cargo fmt"
# Exclude the wasm32 cdylib — its auto-generated bindings.rs (wit-bindgen)
# has cfg(target_arch = "wasm32")-gated code that rustfmt cannot normalize
# on a host build and would otherwise fail the check.
cargo fmt -p greentic-ext-contract -p greentic-ext-runtime \
          -p greentic-ext-cli -p greentic-ext-testing \
          -p _wit-lint -p greentic-ext-registry -- --check

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
