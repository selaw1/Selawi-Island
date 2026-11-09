#!/usr/bin/env bash
# End-to-end: daemon + CLI (+ the extension's DaemonFeed under GJS when
# available) on a private D-Bus session bus. Run via scripts/test.sh, or:
#   dbus-run-session -- tests/integration/dbus-smoke.sh
set -euo pipefail
cd "$(dirname "$0")/../.."

BIN=./target/debug
CLI="$BIN/selawi-island"
WORK="$(mktemp -d)"
trap 'kill "$DAEMON_PID" 2>/dev/null || true; rm -rf "$WORK"' EXIT
fail() { echo "FAIL: $*"; exit 1; }

if "$CLI" status >/dev/null 2>&1; then fail "status should fail without a daemon"; fi

"$BIN/selawi-islandd" --state-file "$WORK/state/events.json" 2>"$WORK/daemon.log" &
DAEMON_PID=$!
for _ in $(seq 50); do "$CLI" status >/dev/null 2>&1 && break; sleep 0.1; done
"$CLI" status | grep -q "protocol 1" || fail "daemon did not start"
sleep 0.5
INTEGRATIONS=$("$CLI" integrations)
for id in mpris upower bluez notifications; do
    echo "$INTEGRATIONS" | grep -q "^$id " || fail "integration $id not listed"
done
echo "$INTEGRATIONS" | grep -q "^mpris .*active" || fail "mpris should be active on the session bus"

"$CLI" notify "Hello" --id n1 --ttl 1
ID=$("$CLI" progress start --title "Tests" --percent 10 --ttl 120)
"$CLI" progress update --id "$ID" --percent 65
"$CLI" list | grep -q '"percent":65.0' || fail "progress update not applied"
"$CLI" progress finish --id "$ID"
"$CLI" list | grep -q '"state":"ended"' || fail "finish not applied"
if "$CLI" progress update --id "$ID" --percent 70 2>/dev/null; then fail "update after finish accepted"; fi
if "$CLI" send '{"id":"x","exec":"rm"}' 2>/dev/null; then fail "invalid event accepted"; fi
sleep 1.2
"$CLI" list | grep -q '"id":"n1"' && fail "notification did not expire"

# Persistence: a persistent event survives a restart.
"$CLI" send '{"id":"call","source":{"app":"Slack","integration":"test"},"category":"call","state":"started","persistent":true,"payload":{"type":"call","title":"Standup","confidence":"confirmed"}}'
kill "$DAEMON_PID"; wait "$DAEMON_PID" 2>/dev/null || true
"$BIN/selawi-islandd" --state-file "$WORK/state/events.json" 2>>"$WORK/daemon.log" &
DAEMON_PID=$!
for _ in $(seq 50); do "$CLI" status >/dev/null 2>&1 && break; sleep 0.1; done
"$CLI" list | grep -q '"id":"call"' || fail "persistent event not restored"
"$CLI" dismiss call

if command -v gjs >/dev/null; then
    gjs -m tests/integration/daemon-feed.gjs.js "$CLI" || fail "DaemonFeed check failed"
else
    echo "(gjs not installed: skipped the DaemonFeed check)"
fi
echo "D-Bus smoke test passed."
