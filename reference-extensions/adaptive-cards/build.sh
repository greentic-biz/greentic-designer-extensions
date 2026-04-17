#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"

echo "==> cargo component build"
cargo component build --release

# cargo-component wraps via wasm32-wasip1 internally; the result is still a
# valid WASM Component Model binary.
WASM_PATH="../../target/wasm32-wasip1/release/greentic_adaptive_cards_extension.wasm"
if [ ! -f "$WASM_PATH" ]; then
  echo "ERROR: wasm not found at $WASM_PATH" >&2
  exit 1
fi

STAGE="$(mktemp -d)"
trap "rm -rf $STAGE" EXIT

cp describe.json "$STAGE/"
cp "$WASM_PATH" "$STAGE/extension.wasm"
cp -r schemas prompts i18n "$STAGE/"
mkdir -p "$STAGE/knowledge"

OUT="$HERE/greentic.adaptive-cards-1.6.0.gtxpack"
# zip appends .zip when the destination has no .zip extension; use a temp name.
TMP_ZIP="$STAGE/../greentic_ac_ext_$$.zip"
(cd "$STAGE" && zip -r "$TMP_ZIP" .) > /dev/null
mv "$TMP_ZIP" "$OUT"

echo "==> built $OUT"
echo "==> size: $(du -h "$OUT" | cut -f1)"
