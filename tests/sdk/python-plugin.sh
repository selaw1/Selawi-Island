#!/usr/bin/env bash
# The Python plugin helper against a real daemon on a private bus.
#   dbus-run-session -- tests/sdk/python-plugin.sh   (run by scripts/test.sh)
set -euo pipefail
cd "$(dirname "$0")/../.."
BIN="$PWD/target/debug"
WORK="$(mktemp -d)"
XDG_CONFIG_HOME="$WORK" "$BIN/selawi-islandd" --no-persist >/dev/null 2>&1 &
DAEMON=$!
trap 'kill $DAEMON 2>/dev/null || true; rm -rf "$WORK"' EXIT
for _ in $(seq 50); do "$BIN/selawi-island" status >/dev/null 2>&1 && break; sleep 0.1; done
PY="${PYTHON:-python3}"
"$PY" tests/sdk/python-plugin.py "$BIN"
