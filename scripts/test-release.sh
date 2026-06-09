#!/usr/bin/env bash
set -euo pipefail

# Strata Kernel v1.0 — Release Test Script
# Runs the full test suite in an isolated temp directory.
# No persistent debug artifacts remain after completion.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"
TARGET_DIR=$(mktemp -d /tmp/strata-test-XXXXXX)
trap 'rm -rf "$TARGET_DIR"' EXIT

echo "=== Strata Kernel v1.0 Full Test Suite ==="
echo "  Target: $TARGET_DIR"

# Use dev profile (stripped debuginfo, no debug — ~53 MB build cache)
# or --release for release-profile tests (slower compile, stripped binary)
CARGO_TARGET_DIR="$TARGET_DIR" cargo test --manifest-path "$REPO_DIR/Cargo.toml" "$@"

echo ""
echo "=== All tests passed. Cleaned up build artifacts. ==="
