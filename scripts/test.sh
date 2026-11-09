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
find gnome-extension tests/integration -name '*.js' -print0 | xargs -0 -n1 node --check

echo "== GSettings schema"
glib-compile-schemas --strict --dry-run gnome-extension/schemas

echo "== End-to-end over a private D-Bus session"
if command -v dbus-run-session >/dev/null; then
    cargo build -q -p selawi-islandd -p selawi-island
    if ! out=$(dbus-run-session -- tests/integration/dbus-smoke.sh 2>&1); then
        echo "$out"
        echo "End-to-end test failed"
        exit 1
    fi
    echo "$out" | grep -v "fd limit"
else
    echo "(dbus-run-session not installed: skipped)"
fi

echo "== Preferences window (headless GTK)"
if command -v xvfb-run >/dev/null && command -v gjs >/dev/null &&
   gjs -c "imports.gi.versions.Gtk = '4.0'; imports.gi.Gtk; imports.gi.Adw" >/dev/null 2>&1; then
    tests/integration/prefs-check.sh || { echo "Preferences check failed"; exit 1; }
else
    echo "(needs gjs, GTK 4, libadwaita and xvfb-run: skipped)"
fi

echo "All checks passed."
