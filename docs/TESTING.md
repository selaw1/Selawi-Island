# Testing

## Automated (no GNOME session needed)

```bash
./scripts/test.sh
```

| Check | What it covers |
|---|---|
| `cargo fmt --check`, `cargo clippy -D warnings` | Style; bans `unwrap`/`expect` outside tests, forbids `unsafe` |
| `core` unit tests (14) | ID and text validation, ranges, payload/category rules, JSON shape, unknown fields, lifecycle transitions, priority order |
| `core/tests/engine.rs` (17) | Create, update, dedup, expiry, ttl refresh, linger, stale updates, priority (calls are never displaced), stable order, user overrides, dismissal, grouping, capacity, 10 000-event flood, snapshot/restore |
| `core/tests/protocol_fixtures.rs` | Rust accepts/rejects every case in `tests/fixtures/protocol-cases.json` |
| `core/tests/demo_scenario.rs` | Every simulator event is valid and the scenario plays twice through the engine |
| `tests/js/*.test.js` (20) | JS validator on the same shared cases, JS event store (same rules as Rust), view model (durations, progress, no invented battery, unconfirmed calls not shown as calls) |
| `node --check` | Every extension file parses |
| `glib-compile-schemas --strict` | Settings schema is valid |

Simulated events are the test source: `gnome-extension/data/demo-scenario.json`.

## Manual checks on GNOME

These need your GNOME 51 session; they cannot run in CI.

Install (`./scripts/install-local.sh`, then log out/in) or run
`./scripts/dev.sh`. Watch logs with:

```bash
journalctl -f -o cat /usr/bin/gnome-shell
```

The demo loops about every 55 seconds. Check each item:

1. **Position** — a small black pill centered just below the top bar on the primary monitor.
2. **Music** (~1.5 s) — pill widens smoothly, briefly expands showing “Midnight City”, collapses after ~3.5 s. Compact shows a pink play icon and a running time.
3. **Earbuds** (~5.5 s) — “Galaxy Buds2 Pro”, Connected, 82%.
4. **Download** — blue progress 0→88%, then green “Done”; disappears ~4 s later.
5. **Call** — “Design sync” takes over with a green running timer.
6. **Notification during call** — “Build passed” arrives but **does not** take over; compact shows “+N”.
7. **Call ends**, then **Battery low** (critical, orange) takes over; then earbuds “Disconnected” with **no battery shown**.
8. **Hover** — pill grows slightly; moving away shrinks it.
9. **Click** — expands; the other events are listed below; clicking one shows it; click again collapses. Leaving the expanded island collapses it after ~1.2 s.
10. **Actions** — “Pause” on music freezes the timer and changes to “Play”; “Cancel” on the download shows Failed in red; “Leave” ends the call; “Dismiss” removes an event. None of these should collapse the island.
11. **Keyboard** — Ctrl+Alt+Tab → “Selawi Island”; Enter toggles, Escape collapses, Delete dismisses.
12. **Interruption** — click rapidly during an animation: it reverses smoothly, no jumps.
13. **Fullscreen** — fullscreen a video: the island fades out; it returns when you leave fullscreen.
14. **Reduce animation** (Settings → Accessibility) — changes are instant.
15. **Preferences** — `gnome-extensions prefs selawi-island@selawi`: change widths, turn off “Below the top bar”, turn off demo events — the island updates immediately.
16. **Restart** — `gnome-extensions disable selawi-island@selawi` then `enable`: the island disappears cleanly and comes back with no errors in the log.
17. **Monitors** — with two monitors, set Monitor to 1; plug/unplug a monitor: the island repositions.

Report anything that looks wrong with the step number and any log lines.
