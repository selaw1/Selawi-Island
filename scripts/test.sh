#!/usr/bin/env bash
# Runs every automated check that does not need a running GNOME session.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== Rust: format, lint, tests"
cargo fmt --check
cargo clippy --all-targets -q -- -D warnings
cargo test -q

echo "== Browser extension: type check, build, unit tests"
./scripts/build-browser.sh >/dev/null
node --test tests/browser/*.test.js

echo "== JavaScript: unit tests"
node --test tests/js/*.test.js

echo "== JavaScript: syntax of every extension file"
find gnome-extension tests/integration -name '*.js' -print0 | xargs -0 -n1 node --check

echo "== GSettings schema"
glib-compile-schemas --strict --dry-run gnome-extension/schemas

echo "== Shell scripts and the Arch package layout"
for script in scripts/*.sh tests/integration/*.sh packaging/arch/PKGBUILD; do bash -n "$script"; done
cargo build -q -p selawi-islandd -p selawi-island -p selawi-island-browser-host
stage="$(mktemp -d)"
SELAWI_PROFILE=debug ./scripts/stage-package.sh "$stage"
for file in usr/bin/selawi-islandd usr/bin/selawi-island usr/bin/selawi-island-browser-host usr/bin/selawi-island-update \
    usr/lib/systemd/user/selawi-islandd.service usr/share/dbus-1/services/com.selawi.Island.service \
    usr/share/gnome-shell/extensions/selawi-island@selawi/extension.js \
    usr/share/gnome-shell/extensions/selawi-island@selawi/schemas/gschemas.compiled \
    usr/share/selawi-island/browser/chrome/manifest.json usr/share/selawi-island/browser/firefox/manifest.json \
    etc/opt/chrome/native-messaging-hosts/io.selawi.island.json usr/lib/mozilla/native-messaging-hosts/io.selawi.island.json \
    usr/share/libalpm/hooks/selawi-island-install-pre.hook usr/share/libalpm/hooks/selawi-island-remove-post.hook; do
    [ -f "$stage/$file" ] || { echo "package is missing $file"; exit 1; }
done
grep -q '^ExecStart=/usr/bin/selawi-islandd$' "$stage/usr/lib/systemd/user/selawi-islandd.service" ||
    { echo "packaged service does not start /usr/bin/selawi-islandd"; exit 1; }
rm -rf "$stage"

echo "== End-to-end over a private D-Bus session"
if command -v dbus-run-session >/dev/null; then
    cargo build -q --workspace
    if ! out=$(dbus-run-session -- cargo test -q -p selawi-islandd --test fake_services 2>&1); then
        echo "$out"
        echo "Integration tests against fake services failed"
        exit 1
    fi
    echo "$out" | grep -E "test result"
    if ! out=$(dbus-run-session -- tests/integration/browser-host.sh 2>&1); then
        echo "$out"
        echo "Browser helper test failed"
        exit 1
    fi
    echo "$out" | grep "^ok:"
    if command -v xvfb-run >/dev/null && node -e "require('playwright')" >/dev/null 2>&1; then
        if ! out=$(dbus-run-session -- tests/integration/browser-e2e.sh 2>&1); then
            echo "$out"
            echo "Browser end-to-end test failed"
            exit 1
        fi
        echo "$out" | grep "^ok:"
    else
        echo "(Chromium end-to-end test skipped: needs xvfb-run and Playwright)"
    fi
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
if command -v xvfb-run >/dev/null && command -v gjs >/dev/null && command -v dbus-run-session >/dev/null &&
   gjs -c "imports.gi.versions.Gtk = '4.0'; imports.gi.Gtk; imports.gi.Adw" >/dev/null 2>&1; then
    cargo build -q -p selawi-islandd -p selawi-island
    tests/integration/prefs-check.sh || { echo "Preferences check failed"; exit 1; }
else
    echo "(needs gjs, GTK 4, libadwaita and xvfb-run: skipped)"
fi

echo "All checks passed."
