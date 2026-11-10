#!/usr/bin/env bash
# Fails if the daemon gets heavy: CPU while idle, memory, how it handles a
# flood of events, and how often it makes the island redraw.
#   tests/stress/stress-check.sh   (uses the debug build; run by test.sh)
set -euo pipefail
cd "$(dirname "$0")/../.."
out=$(SELAWI_BIN=./target/debug SELAWI_IDLE_SECONDS=5 dbus-run-session -- tests/stress/measure.sh 5000 2>/dev/null)
get() { echo "$out" | sed -n "s/^$1=//p"; }
fail() { echo "$out"; echo "FAIL: $*"; exit 1; }
idle=$(get idle_cpu_ms); rss=$(get rss_after_kb); events=$(get events_after)
changed=$(get flood | sed -n 's/.*"changed":\([0-9]*\).*/\1/p')
seconds=$(get flood | sed -n 's/.*"seconds":\([0-9.]*\).*/\1/p')
[ "$idle" -le 20 ] || fail "idle daemon used ${idle} ms CPU in 5 s (it should not wake up)"
[ "$rss" -le 40000 ] || fail "daemon uses ${rss} KB after a flood"
[ "$events" -le 64 ] || fail "more than 64 events kept ($events)"
[ -n "$changed" ] && [ "$changed" -le 100 ] || fail "5000 changes caused $changed Changed signals (island redraws)"
echo "ok: idle CPU ${idle} ms / 5 s, ${rss} KB after 5000 events in ${seconds} s, ${changed} redraw signals, ${events} events kept"
