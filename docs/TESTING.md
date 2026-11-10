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
| `daemon` unit tests (47) | Engine (submit/list/dismiss, limits, expiry, snapshot 0600, corrupt snapshot), config, safe reading of D-Bus values, MPRIS event building (controls only when supported, paused ttl, stopped, hostile metadata), battery rules (quiet at startup, plug/unplug, low once, critical persistent, cleared by charger), Bluetooth (names, kinds, battery only when reported, never on disconnect), configuration (defaults, round trip, typos rejected, saved 0600), player keys and which player is shown in each media mode, notifications (markup, hidden text, urgency, timeouts, app keys), network changes (switch, offline, portal, VPN), USB drive naming, memory/temperature parsing and warning margins against fake /proc and /sys files |
| `daemon/tests/fake_services.rs` (10) | Each integration against a fake service on a private bus: fake MPRIS players (appears, Pause reaches it, closes → ended; dismissed then resumed comes back; one-player mode follows the preferred player and switches back to all), fake UPower (quiet at start, Charging, Battery low), fake BlueZ (connect, battery added a moment later, disconnect without battery), a fake notification server (Notify observed while the real server still answers; markup cleaned), turning an integration off and on live, a fake NetworkManager (quiet start, VPN up, Wi-Fi → Ethernet, portal, offline with VPN down), a fake UDisks2 (internal disk ignored, USB drive in and out), and the warnings integration reading a fake /proc |
| `tests/js/*.test.js` (51) | JS validator on the shared cases, JS event store, view model (logos and small status icons, playing media before paused, mic bubble logo), recording app logo id, parsing the daemon's event list (real reply) and dropping bad entries, battery/audio/network icons, microphone (orange, waves) and camera (green) view, muted state, a confirmed call absorbing the microphone event, a browser's own MPRIS player hidden while the extension shows its tabs, microphone event model (titles, Mute/Unmute buttons), loudness parsing and wave bar math, placement (every default anchor, growth direction, top-bar inset, custom default, edges, second monitor, snap-back) |
| `tests/integration/dbus-smoke.sh` | On a private D-Bus bus: CLI without daemon fails cleanly; daemon starts; notify, progress start/update/finish, rejected updates and invalid events; expiry; persistent event restored after a daemon restart; and the extension's real `DaemonFeed` under GJS (connect, follow changes, dismiss, refused action, and a Mute press on an extension-submitted event coming back as `action-invoked`) |
| `tests/integration/prefs-check.sh` | Opens the real preferences window headless (Xvfb, GTK 4, libadwaita) against a real daemon on a private bus: Default position dropdown, Make this the default / Reset, and the Integrations page (service status, turning Bluetooth off, one-player mode, notification text, audio switch). Skipped if `xvfb-run` is missing (`sudo pacman -S xorg-server-xvfb`) |
| `tests/browser/*.test.js` (6) | The browser extension's event building (Meet titles, mute state only when known, buttons, media tabs with only the controls the page offers and no timer during ads, site names for calls on any site, download progress and names), every event validated by the island's validator. Runs after `scripts/build-browser.sh` (strict TypeScript) |
| `native-messaging` unit tests (6) | Message framing, size limit, refusing events that pose as other integrations or carry unknown fields, rate limit |
| `tests/integration/browser-host.sh` | The real helper against the real daemon: a Meet call arrives, spoofed and unknown messages are refused, Mute pressed in the island reaches the browser, closing the browser ends the call |
| `tests/integration/browser-e2e.sh` | The built extension in real Chromium (Playwright, Xvfb) with stand-in pages: Meet (call with its name, Mute/Unmute press the page's microphone button, Leave ends the call), a download (appears, finishes), YouTube (song and artist, Pause/Play reach the page, a paused tab stays, the song title stays during an ad, Next presses the page's button), SoundCloud playing at the same time with its time read from the page, closing a tab ends its media, a real WebRTC call with the fake microphone on a stand-in Slack page shows as a green “Slack” call and ends on hang-up, and a connection without microphone is not a call. Stable over repeated runs |
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

### Integrations (Phase 4)

1. `selawi-island integrations`: `mpris` active; `upower` active (laptop) or unavailable (desktop); `bluez` active.
2. Play music in Spotify, Firefox (YouTube) or any player: the island shows the track with a running time. **Pause / Next / Previous** in the expanded island control the player.
3. Pause the music: it stays (Play button); after 5 minutes paused it disappears. Close the player: it ends.
4. Laptop: unplug and plug in the charger: “On battery” / “Charging”.
5. Connect Bluetooth earbuds: “Connected”, with battery only if the earbuds report it. Disconnect: “Disconnected” without battery.
6. Right-click the island → Integrations: turn Bluetooth off; `selawi-island integrations` shows `disabled` right away, no restart.
7. Open two players (e.g. Spotify and a YouTube tab). Integrations → Players to show → **One player**, Preferred player → Spotify: only Spotify shows while it plays; pause it and the other one shows.
8. Send yourself a notification (e.g. `notify-send "Hello" "From the terminal"`): it appears in the island and **not** as a GNOME banner; it is still in GNOME's notification list. Turn off “Hide GNOME's notification banners”: both appear.
9. `systemctl --user stop selawi-islandd`, then `notify-send hi`: GNOME's banner appears again (nothing is lost while the daemon is off).
10. Switch audio output (e.g. connect earbuds, or pick another output in the volume menu): “<device> · Audio output”. Mute the microphone: “Microphone muted”.
11. `journalctl --user -u selawi-islandd` shows status changes but never track titles or device names.

### Network, drives and warnings (Phase 5)

1. Turn Wi-Fi off and on (or unplug Ethernet): “Offline”, then “<network> · Wi-Fi connected”.
2. Connect and disconnect a VPN: “<VPN> · VPN connected / disconnected”.
3. Plug in a USB stick: “<brand model> · USB drive connected”; remove it: “… removed”.
4. Integrations → turn on **System warnings**; `selawi-island integrations` shows `health active — checks memory and CPU temperature every 15 s` (or only what your machine has).
5. Optional stress check: run something heavy until the CPU passes 90 °C, or fill memory: one warning, not repeated while it stays high.

### Microphone, camera and voice waves

1. Use an app that records (e.g. the Slack desktop app in a huddle): an **orange** “Microphone · <app>” appears (app name without “input”), with a running timer and bars on the right.
2. Talk: the bars move with your voice; stay quiet: they sit flat.
3. Turn the camera on: it turns **green** (“Camera and microphone”). Camera only (mic muted in the app may still count as microphone in use; that is the app's choice).
4. Leave the meeting: it ends and disappears; `pactl list source-outputs short` no longer lists “Selawi Island”.
5. While it shows, go fullscreen or let another event take over the pill: the “Selawi Island” recording stream disappears (waves are only measured while visible).
6. Integrations → turn off **Voice waves**: bars stop and the extra stream never appears. Turn off **Microphone and camera in use**: the event disappears.
7. `systemctl --user restart selawi-islandd` during a call: the microphone event comes back by itself.
8. Expand the island during a call → **Mute**: it shows “Microphone muted”, the bars stop, GNOME's volume menu shows the microphone muted, and the other side can't hear you. **Unmute** brings your voice back.
9. Mute the microphone from GNOME's volume menu (or the mic key) during a call: the bubble shows “Microphone muted”; **Unmute** in the bubble turns it back on.
10. Join a call in a desktop app (Slack, Zoom, Discord): the orange bubble shows the app's logo instead of the mic icon; Mute adds a small crossed-out mic next to the time.

### Browser (Google Meet, downloads)

1. Load the extension (docs/BROWSER_INTEGRATIONS.md).
2. Join a Google Meet: a **green** call with the meeting name and a timer; the orange microphone bubble disappears into it and the call shows the voice waves. The call shows Meet's logo. Turn the camera on in Meet: a small camera icon appears next to the time and “Camera on” in the details; off: it goes away.
3. Mute in Meet: the island shows “Muted”. Press **Unmute** in the island: Meet unmutes (its own button changes).
4. Press **Leave** in the island: you leave the meeting and the call ends.
5. Play YouTube and SoundCloud tabs: each shows its site's logo; pausing adds a small pause icon next to the time. Spotify (if installed) shows Spotify's own icon.
6. Download a file: its name and progress, then “Downloaded”. Start another and press **Cancel**: “Cancelled”.
7. Close Chrome during a call: the call ends in the island.
8. If nothing shows on Meet, the page may have changed: open Meet, press F12 → Console, and report any “Selawi” messages.
9. Play a YouTube video and a SoundCloud track in two tabs: both show (no extra “Chrome” player). Previous/Play-Pause/Next in the island control each tab.
10. Let a YouTube ad play: the island keeps the song's title and hides the timer.
11. Pause a tab: it stays with a Play button for 5 minutes. Close it: it disappears. With two tabs open, pause the newer one: the closed island switches to the one still playing.
12. Join a Slack huddle, a Teams or Zoom web call, or a Discord voice channel in Chrome: a green bubble with the site's logo and name, a timer and voice waves. Leave: it disappears. If Mute / Leave buttons appear, the site reports Chrome's call controls: check they work and note which sites do.
13. On Meet, Mute / Unmute / Leave in the island still work (Meet's call controls first, its page buttons otherwise).
14. The expanded Meet call shows Meet's logo with no coloured square behind it.

### Idle and position

1. With no events the island is a thin bar as wide as the clock above it. Hovering makes it taller; moving away makes it thin again.
2. Click it: it expands and says **Nothing to show**. Click again (or Escape) to close.
3. Drag it somewhere else and let go: it stays there. Send `selawi-island notify hi`: it grows in place, never off the screen edge.
4. Log out and in: it is still where you left it.
5. Drag it back near the top center: it snaps into the default spot. Preferences → Position → **Reset** does the same.
6. With two monitors, drag it onto the other one: it stays on that monitor.
7. Turn off **Allow dragging**: dragging does nothing, clicking still works.
8. Turn off **Show when idle**: with no events the island disappears completely.
9. Drag fast, across windows and the top bar: the island stays under the pointer the whole way.
10. Right-click the island: its preferences open.
11. Preferences → Position → **Default position**: try each choice. The island moves there; at the bottom it grows upward when an event arrives; in the corners it stays the edge margin away from both edges.
12. Drag it somewhere, then **Make this the default**: the dropdown shows Custom, and Reset or a drop near that spot puts it back there.
13. With a bottom default, drag it away and drop it back near the bottom: it snaps into place.

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
