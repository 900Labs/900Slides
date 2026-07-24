#!/usr/bin/env bash
#
# Public-release privacy gate for 900Slides.
#
# Verifies that no local paths, hostnames, secrets, telemetry, or
# machine-specific artifacts appear in application source. Run this before
# changing repository visibility or publishing a release.
#
set -euo pipefail

cd "$(dirname "$0")/.."

status=0

# Directories that contain application source (not infrastructure scripts).
SRC_DIRS="crates/ apps/desktop/src/ apps/desktop/src-tauri/src/"

echo "==> Checking for absolute local paths in application source"
if grep -rnE '(/Users/[^/]+|/home/[^/]+|[A-Z]:\\)' \
    --include='*.rs' --include='*.ts' --include='*.svelte' \
    --include='*.json' --include='*.toml' \
    $SRC_DIRS 2>/dev/null \
    | grep -v '/target/' \
    | grep -v 'node_modules'; then
    echo "FAIL: absolute local paths found in application source."
    status=1
fi

echo "==> Checking for hardcoded hostnames in application source"
if grep -rnE '\b(localhost|127\.0\.0\.1|0\.0\.0\.0)\b' \
    --include='*.rs' --include='*.ts' --include='*.svelte' \
    --include='*.toml' \
    $SRC_DIRS 2>/dev/null \
    | grep -v 'node_modules'; then
    echo "FAIL: hardcoded hostnames found in application source."
    status=1
fi

echo "==> Checking for secrets and credentials"
# Match common secret-like identifiers as whole words only.
if grep -rniE '\b(api_key|apikey|access_key|secret_key|private_key|password|passwd|client_secret|aws_secret)\b' \
    --include='*.rs' --include='*.ts' --include='*.svelte' \
    --include='*.json' --include='*.toml' \
    --exclude-dir=target --exclude-dir=node_modules \
    crates/ apps/desktop/ 2>/dev/null; then
    echo "FAIL: potential secrets found in source."
    status=1
fi

echo "==> Checking for telemetry or analytics in application source"
if grep -rniE '\b(telemetry|analytics|posthog|amplitude|mixpanel|segment\.io|sentry|datadog)\b' \
    --include='*.rs' --include='*.ts' --include='*.svelte' \
    --include='*.json' --include='*.toml' \
    --exclude-dir=target --exclude-dir=node_modules \
    $SRC_DIRS 2>/dev/null; then
    echo "FAIL: telemetry or analytics references found in application source."
    status=1
fi

echo "==> Checking for network HTTP dependencies"
if grep -rnE '(reqwest|hyper::client|ureq|isahc|attohttpc|minreq)' \
    --include='Cargo.toml' \
    crates/ apps/desktop/src-tauri/ 2>/dev/null; then
    echo "FAIL: network HTTP client dependency found."
    status=1
fi

if [ "$status" -eq 0 ]; then
    echo ""
    echo "Privacy gate PASSED. No local paths, hostnames, secrets, telemetry,"
    echo "or network dependencies detected in application source."
else
    echo ""
    echo "Privacy gate FAILED. Fix the issues above before publishing."
    exit 1
fi
