#!/usr/bin/env bash
# Builds the frontend, builds the backend release binary, and places the
# frontend build where the backend looks for it by default (next to the
# binary) - so `./core/target/release/duplicast-core` works from any
# directory with no env vars needed.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLIENT_DIR="$ROOT_DIR/client"
CORE_DIR="$ROOT_DIR/core"
RELEASE_DIR="$CORE_DIR/target/release"

echo "==> Building frontend"
(cd "$CLIENT_DIR" && pnpm install --frozen-lockfile && pnpm run build)

echo "==> Building backend (release)"
(cd "$CORE_DIR" && cargo build --release)

echo "==> Placing frontend build next to the binary"
rm -rf "$RELEASE_DIR/dist"
cp -r "$CLIENT_DIR/dist" "$RELEASE_DIR/dist"

echo "==> Done"
echo "Binary:  $RELEASE_DIR/duplicast-core"
echo "Frontend: $RELEASE_DIR/dist"
echo "Run:     $RELEASE_DIR/duplicast-core"
