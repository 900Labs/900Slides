#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

# Install dependencies first with npm ci --prefix apps/desktop.
echo "==> cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "==> cargo clippy --workspace --all-targets -- -D warnings"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> cargo test --workspace"
cargo test --workspace

echo "==> npm run check (apps/desktop)"
npm run check --prefix apps/desktop

echo "==> Frontend unit tests and production build"
npm test --prefix apps/desktop
npm run build --prefix apps/desktop

echo "==> Standalone desktop formatting, clippy and tests"
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml

echo "==> Public-release gate"
./scripts/verify-public-release.sh

echo "==> Automated checks passed. Complete the native smoke test:"
echo "    npm run tauri:dev --prefix apps/desktop"
