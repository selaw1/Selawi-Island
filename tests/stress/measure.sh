#!/usr/bin/env bash
# Measures the daemon on a private D-Bus session: idle CPU, memory, and
# a flood of events. Prints numbers; used by stress-check.sh.
#   dbus-run-session -- tests/stress/measure.sh [COUNT]
set -euo pipefail
cd "$(dirname "$0")/../.."
COUNT="${1:-5000}"
BIN="${SELAWI_BIN:-./target/release}"
IDLE="${SELAWI_IDLE_SECONDS:-10}"
"$BIN/selawi-islandd" --no-persist >/dev/null 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null || true' EXIT
for _ in $(seq 50); do "$BIN/selawi-island" status >/dev/null 2>&1 && break; sleep 0.1; done
ticks() { awk '{print $14 + $15}' "/proc/$PID/stat"; }
rss() { awk '/VmRSS/ {print $2}' "/proc/$PID/status"; }
HZ=$(getconf CLK_TCK)
sleep 2
t0=$(ticks); sleep "$IDLE"; t1=$(ticks)
echo "idle_cpu_ms=$(( (t1 - t0) * 1000 / HZ ))"
echo "idle_seconds=$IDLE"
echo "rss_idle_kb=$(rss)"
t0=$(ticks)
echo "flood=$(gjs -m tests/stress/flood.gjs.js "$COUNT")"
t1=$(ticks)
echo "flood_cpu_ms=$(( (t1 - t0) * 1000 / HZ ))"
echo "rss_after_kb=$(rss)"
echo "events_after=$("$BIN/selawi-island" list | grep -o '"id"' | wc -l)"
