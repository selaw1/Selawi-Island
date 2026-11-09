#!/usr/bin/env bash
# Runs every automated check that does not need a running GNOME session.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== Rust: format, lint, tests"
cargo fmt --check
cargo clippy --all-targets -q -- -D warnings
cargo test -q

echo "== JavaScript: unit tests"
node --test tests/js/*.test.js

echo "== JavaScript: syntax of every extension file"
find gnome-extension -name '*.js' -print0 | xargs -0 -n1 node --check

echo "== GSettings schema"
glib-compile-schemas --strict --dry-run gnome-extension/schemas

echo "All checks passed."
