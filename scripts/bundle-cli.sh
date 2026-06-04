#!/usr/bin/env bash
set -euo pipefail

TARGET_TRIPLE=$(rustc -vV | grep '^host:' | cut -d' ' -f2)
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$REPO_ROOT/src-tauri/binaries"

echo "Building mango-cli for $TARGET_TRIPLE ..."
cargo build --release -p mango-cli

mkdir -p "$OUT_DIR"
cp "$REPO_ROOT/target/release/mango" "$OUT_DIR/mango-cli-$TARGET_TRIPLE"
echo "Copied to $OUT_DIR/mango-cli-$TARGET_TRIPLE"
