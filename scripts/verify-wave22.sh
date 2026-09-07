#!/usr/bin/env bash
# Wave 22 release-size verification for the distributable Tauri feature set.
set -euo pipefail

cd "$(dirname "$0")/.."

max_bytes="${MAX_RELEASE_BINARY_BYTES:-16777216}"
default_binary="apps/desktop/src-tauri/target/release/slides-desktop"
if [[ "${OS:-}" == "Windows_NT" ]]; then
    default_binary="${default_binary}.exe"
fi
binary="${1:-$default_binary}"

if [[ $# -eq 0 ]]; then
    echo "Building fresh frontend assets and the distributable Tauri bundle..."
    echo "This requires npm dependencies and the Tauri system prerequisites."
    npm run tauri:build --prefix apps/desktop
fi

if [[ ! -f "$binary" ]]; then
    echo "FAIL: release binary not found: $binary" >&2
    echo "Build it with: npm run tauri:build --prefix apps/desktop" >&2
    exit 1
fi

bytes="$(wc -c < "$binary" | tr -d '[:space:]')"
echo "Release binary: $binary"
echo "Release bytes: $bytes"
echo "Budget bytes: $max_bytes"

if (( bytes > max_bytes )); then
    echo "FAIL: release binary exceeds the Wave 22 size budget." >&2
    exit 1
fi

echo "Wave 22 release-size gate PASSED."
