#!/usr/bin/env bash
# Real Chromium + built extension + native host + daemon, on a private
# D-Bus session and a throw-away browser profile. Needs xvfb-run and Playwright.
#   dbus-run-session -- tests/integration/browser-e2e.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
BIN="$PWD/target/debug"
"$BIN/selawi-islandd" --no-persist >/dev/null 2>&1 &
DAEMON=$!
trap 'kill $DAEMON 2>/dev/null || true' EXIT
for _ in $(seq 50); do "$BIN/selawi-island" status >/dev/null 2>&1 && break; sleep 0.1; done
timeout 120 xvfb-run -a node tests/integration/browser-e2e.mjs
