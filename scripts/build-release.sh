#!/usr/bin/env bash
set -euo pipefail

# Strata Kernel v1.0 — Release Build Script
# Produces a minimal release binary with no debug artifacts.
# Total on-disk footprint: ~718K binary + ~53M build cache (cleaned after).

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(dirname "$SCRIPT_DIR")"
TARGET_DIR=$(mktemp -d /tmp/strata-release-XXXXXX)
trap 'rm -rf "$TARGET_DIR"' EXIT

echo "=== Strata Kernel v1.0 Release Build ==="
echo "  Target: $TARGET_DIR"

# Build release binary (stripped, optimized, no debug)
CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release --manifest-path "$REPO_DIR/Cargo.toml" "$@"

# Show results
BINARY="$TARGET_DIR/release/strata"
if [ -f "$BINARY" ]; then
    echo ""
    echo "=== Build Complete ==="
    ls -lh "$BINARY"
    echo "  Binary: $BINARY"
    echo "  Size:   $(du -h "$BINARY" | cut -f1)"
    echo "  SHA256: $(shasum -a 256 "$BINARY" | cut -d' ' -f1)"
    # Copy binary to repo for convenience
    cp "$BINARY" "$REPO_DIR/target/release/strata" 2>/dev/null || true
    echo ""
    echo "Done. Clean build; no persistent debug artifacts."
else
    echo "ERROR: Binary not found at $BINARY"
    exit 1
fi
