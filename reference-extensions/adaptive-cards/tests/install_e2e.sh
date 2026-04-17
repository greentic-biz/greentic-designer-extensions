#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE/.."

# Build .gtxpack
./build.sh

# Fake home for isolation
TMP_HOME=$(mktemp -d)
trap "rm -rf $TMP_HOME" EXIT

GTDX="$(pwd)/../../target/debug/gtdx"
if [ ! -x "$GTDX" ]; then
  (cd "$(pwd)/../.." && cargo build -p greentic-ext-cli)
fi

# Install
"$GTDX" --home "$TMP_HOME" install \
  "$(pwd)/greentic.adaptive-cards-1.6.0.gtxpack" \
  -y --trust loose

# Verify directory
if [ ! -d "$TMP_HOME/extensions/design/greentic.adaptive-cards-1.6.0" ]; then
  echo "FAIL: extension directory not created"
  ls -R "$TMP_HOME"
  exit 1
fi

# Verify wasm
if [ ! -f "$TMP_HOME/extensions/design/greentic.adaptive-cards-1.6.0/extension.wasm" ]; then
  echo "FAIL: extension.wasm not installed"
  exit 1
fi

# Verify describe.json
if [ ! -f "$TMP_HOME/extensions/design/greentic.adaptive-cards-1.6.0/describe.json" ]; then
  echo "FAIL: describe.json not installed"
  exit 1
fi

# List
OUT="$("$GTDX" --home "$TMP_HOME" list)"
if ! echo "$OUT" | grep -q "greentic.adaptive-cards@1.6.0"; then
  echo "FAIL: list did not find extension"
  echo "$OUT"
  exit 1
fi

# Doctor
"$GTDX" --home "$TMP_HOME" doctor

echo ""
echo "✓ install e2e passed"
