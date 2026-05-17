#!/bin/bash
set -euo pipefail

echo "Checking formatting..."
cargo fmt --check || {
  echo "Error: formatting check failed. Run 'cargo fmt' to fix." >&2
  exit 1
}
echo "✓ Format check passed"

echo "Running linter..."
cargo clippy -- -D warnings || {
  echo "Error: clippy found issues" >&2
  exit 1
}
echo "✓ Clippy passed"

echo "Running tests..."
cargo test || {
  echo "Error: tests failed" >&2
  exit 1
}
echo "✓ Tests passed"

echo "All pre-tests passed! ✓"
