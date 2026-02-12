#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"

echo "=== cargo check ==="
cargo check 2>&1

echo ""
echo "=== cargo test ==="
cargo test 2>&1

echo ""
echo "=== cargo clippy ==="
cargo clippy -- -D warnings 2>&1

echo ""
echo "All checks passed."
