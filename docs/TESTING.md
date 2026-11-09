# Testing

## Automated (no GNOME session needed)

```bash
./scripts/test.sh
```

| Check | What it covers |
|---|---|
| `cargo fmt --check`, `cargo clippy -D warnings` | Style; bans `unwrap`/`expect` outside tests, forbids `unsafe` |
| `core` unit tests (14) | ID and text validation, ranges, payload/category rules, JSON shape, unknown fields, lifecycle transitions, priority order |
| `core/tests/engine.rs` (18) | Create, update, dedup, expiry, ttl refresh, linger, stale updates, priority (calls are never displaced), stable order, user overrides, dismissal, grouping, capacity, 10 000-event flood, snapshot/restore, restore grace period |
| `core/tests/protocol_fixtures.rs` | Rust accepts/rejects every case in `tests/fixtures/protocol-cases.json` |
| `core/tests/demo_scenario.rs` | Every simulator event is valid and the scenario plays twice through the engine |
| `daemon` unit tests (5) | Submit/list/dismiss, size limit, invalid input, expiry, snapshot file (mode 0600) survives restart, corrupt snapshot ignored |
| `tests/js/*.test.js` (23) | JS validator on the shared cases, JS event store, view model, parsing the daemon's event list (real reply) and dropping bad entries |
| `tests/integration/dbus-smoke.sh` | On a private D-Bus bus: CLI without daemon fails cleanly; daemon starts; notify, progress start/update/finish, rejected updates and invalid events; expiry; persistent event restored after a daemon restart; and the extension's real `DaemonFeed` under GJS (connect, follow changes, dismiss, refused action) |
| `node --check` | Every extension file parses |
| `glib-compile-schemas --strict` | Settings schema is valid |

The end-to-end test needs `dbus-run-session` (package `dbus`) and, for the
`DaemonFeed` part, `gjs` (installed with GNOME).

Simulated events are the test source: `gnome-extension/data/demo-scenario.json`.

## Manual checks on GNOME

These need your GNOME 51 session; they cannot run in CI.

Install (`./scripts/install-local.sh`, then log out/in) or run
`./scripts/dev.sh`. Watch logs with:

```bash
journalctl -f -o cat /usr/bin/gnome-shell
```

### Real events from the daemon (Phase 3)

1. `systemctl --user status selawi-islandd` shows **active (running)**.
2. `selawi-island status` prints the version and event count.
3. `selawi-island notify "Hello" --body "From the terminal"` — the island expands with it, then it disappears after ~8 s.
4. Progress: run the three `progress` commands from `docs/CLI.md`; the bar moves, then shows Done and goes away.
5. Click **Dismiss** on an event in the island; `selawi-island list` no longer shows it.
6. `systemctl --user stop selawi-islandd` — the island goes back to the empty pill. `selawi-island notify hi` starts the daemon again by itself (D-Bus activation) and shows it.
7. Restart survival: send a persistent event, then restart the daemon. It is still there, and disappears about 60 s later because nothing updates it:
   ```bash
   selawi-island send '{"id":"t","source":{"app":"Test","integration":"cli"},"category":"call","state":"started","persistent":true,"payload":{"type":"call","title":"Test call","confidence":"confirmed"}}'
   ```
   ```bash
   systemctl --user restart selawi-islandd
   ```

### Demo (turn on “Demo events” in preferences)

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
