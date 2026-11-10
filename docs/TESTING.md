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
| `tests/browser/*.test.js` (6) | The browser extension's event building (mute state only when the site reports it, buttons, call controls from hostile input, media tabs with only the controls the page offers and no timer during ads, site names for calls on any site, download progress and names), every event validated by the island's validator. Runs after `scripts/build-browser.sh` (strict TypeScript) |
| `native-messaging` unit tests (6) | Message framing, size limit, refusing events that pose as other integrations or carry unknown fields, rate limit |
| `tests/integration/dbus-smoke.sh` (run and pacman part) | `selawi-island run` passes output and exit codes through and shows Running → Done / Failed (exit 3); a missing program exits 127; the pacman hook shows “Updating 2 packages” → “Updated 2 packages”, rejects odd package names, and nothing is shown while System updates is off. The root → user hand-off and the “Did not finish” watcher were checked by hand with a throwaway user (not in CI: needs root and a user session) |
| `daemon/tests/fake_services.rs` (notifications) | A `Notify` call to a fake server reaches the island without replacing it |
| `tests/js/shell-model.test.js` | GNOME Shell and GNOME-app notifications (text setting, urgency, empty titles, app logo and grouping), “Copied” kinds and that it never opens the island |
| `tests/integration/settings-app.gjs.js` (in `prefs-check.sh`) | The settings app's window headless against a real daemon: three pages, Status shows the running service, the missing GNOME Shell, each source's state; Copy report holds versions and states but not a notification that was on the island; without the extension's settings only Status is shown, with a banner |
| `tests/js/report.test.js` | Extension states as GNOME 51 reports them, the report's text, no network or device names |
| `tests/stress/stress-check.sh` | Load check on a private bus (debug build): the idle daemon uses no CPU (≤ 20 ms in 5 s), 5000 submits are handled with at most 100 `Changed` signals, at most 64 events are kept, memory stays under 40 MB. `tests/stress/measure.sh` prints the same numbers for a release build |
| `daemon/tests/plugins.rs` | A plugin's whole life over real D-Bus: refused before registering, asked about, nothing shown before Allow, sealed events (own namespace, manifest name and icon, cannot pose as Slack), undeclared category refused, `Events1` cannot use plugin ids, buttons sent only to the app, another app cannot take its id, 8-event cap, rate limit, events gone when it disconnects or is turned off |
| `sdk/rust/tests/helper.rs`, `tests/sdk/python-plugin.sh` | The Rust and Python helpers against the daemon: register, learn the answer, show, update, end, receive button presses |
| `tests/integration/browser-host.sh` | The real helper against the real daemon: a call arrives, spoofed and unknown messages are refused, Mute pressed in the island reaches the browser, closing the browser ends the call |
| `tests/integration/browser-e2e.sh` | The built extension in real Chromium (Playwright, Xvfb) with stand-in pages: Meet as a plain WebRTC call with standard call controls (“Google Meet”, logo, camera, Mute/Unmute and Leave through the page's handlers, the call goes on while muted with the microphone dropped from the connection), a download (appears, finishes), YouTube (song and artist, Pause/Play reach the page, a paused tab stays, the song title stays during an ad, Next presses the page's button), SoundCloud playing at the same time with its time read from the page, closing a tab ends its media, a real WebRTC call with the fake microphone on a stand-in Slack page shows as a green “Slack” call and ends on hang-up, and a connection without microphone is not a call. Stable over repeated runs |
| `node --check` | Every extension file parses |
| `glib-compile-schemas --strict` | Settings schema is valid |

The end-to-end test needs `dbus-run-session` (package `dbus`) and, for the
`DaemonFeed` part, `gjs` (installed with GNOME).

Simulated events are the test source: `gnome-extension/data/demo-scenario.json`.

The package recipe itself (`makepkg`) needs Arch and is not run here:
`scripts/test.sh` checks the shell scripts' syntax and that
`scripts/stage-package.sh` lays out every installed file (binaries,
systemd user service starting `/usr/bin/selawi-islandd`, D-Bus activation,
GNOME extension with its compiled schema, browser files and the helper's
registration for Chrome and Firefox).

## Manual checks on GNOME

These need your GNOME 51 session; they cannot run in CI.

Install (`./scripts/update.sh` or `selawi-island-update`, then log out/in) or run
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
11. Use the microphone on a website in Chrome (dismiss the Meet call to see the bubble, or record a voice message): the orange bubble shows the site's name and logo, never Chrome's logo.

### Browser (Google Meet, downloads)

1. Load the extension (docs/BROWSER_INTEGRATIONS.md).
2. Join a Google Meet: a **green** “Google Meet” call and a timer; the orange microphone bubble disappears into it and the call shows the voice waves. The call shows Meet's logo. Turn the camera on in Meet: a small camera icon appears next to the time and “Camera on” in the details; off: it goes away.
3. Mute in Meet: the island shows “Muted” and the call stays. Press **Unmute** in the island: Meet unmutes (its own button changes).
4. Press **Leave** in the island: you leave the meeting and the call ends.
5. Play YouTube and SoundCloud tabs: each shows its site's logo; pausing adds a small pause icon next to the time. Spotify (if installed) shows Spotify's own icon.
6. Download a file: its name and progress, then “Downloaded”. Start another and press **Cancel**: “Cancelled”.
7. Close Chrome during a call: the call ends in the island.
8. If Mute / Leave are missing on a site, check what it reports: F12 → Console, `document.addEventListener('selawi-island-session', e => console.log('SELAWI', e.detail))`, then mute and unmute.
9. Play a YouTube video and a SoundCloud track in two tabs: both show (no extra “Chrome” player). Previous/Play-Pause/Next in the island control each tab.
10. Let a YouTube ad play: the island keeps the song's title and hides the timer.
11. Pause a tab: it stays with a Play button for 5 minutes. Close it: it disappears. With two tabs open, pause the newer one: the closed island switches to the one still playing.
12. Join a Slack huddle, a Teams or Zoom web call, or a Discord voice channel in Chrome: a green bubble with the site's logo and name, a timer and voice waves. Leave: it disappears. If Mute / Leave buttons appear, the site reports Chrome's call controls: check they work and note which sites do.
13. On Meet, Mute / Unmute / Leave in the island work through Meet's standard call controls.
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

### Notifications from GNOME and copying

1. With “Hide GNOME's notification banners” on: take a screenshot (Print Screen) → the island shows “Screenshot captured”, no GNOME banner.
2. Set a 1-minute timer in GNOME Clocks: when it rings, the island shows it (with the Clocks logo).
3. Copy a big folder in Files and switch to another window: when it finishes, “Files · File Operations”.
4. Ctrl+C some text → “Copied” for a moment, the island stays closed; copy an image → “Copied image”; copy files in Files → “Copied files”. Close the app you copied from: no new “Copied”.
5. Integrations → turn off Copied: no bubble.

### Performance (Phase 11)

Measured here with a release build (see `tests/stress/measure.sh`):
idle daemon 0 ms CPU in 10 s and about 5.5 MB of memory; 5000 events in
0.4 s with 10 `Changed` signals; memory stays about 5.6 MB after floods
(it levels off, no growth over 50 000 events). The island's logic for a
full list of 64 events takes under 0.1 ms per redraw (Node).

On your machine, with a call or music playing for a while:

1. `top -p $(pgrep selawi-islandd)`: near 0 % CPU.
2. GNOME's own CPU (`gnome-shell` in `top`) does not rise noticeably while the island shows a timer; with voice waves it rises a little, only while the microphone is in use.

### Plugins

1. `cargo run -p selawi-island-plugin --example countdown`: the island asks “Focus timer wants to show things on the island”. Press **Allow**, run it again: a “Focus” bubble with “25:00 left” and **Stop**.
2. Settings → Integrations → Apps: “Focus timer”, on. Turn it off: the bubble goes.
3. Python: `python -c 'from selawi_island import Island'` works after installing the package.

### Settings app

1. Right-click the island: the **Selawi Island** settings app opens (also in the app grid, with its icon).
2. Status page: “Running · version …”, Island “On”, the sources with their state. Press **Restart**: the island blinks off and comes back; the page shows the service running again.
3. `gnome-extensions disable selawi-island@selawi`: the Status page shows “Off” with **Turn on**; pressing it brings the island back.
4. **Copy** report and paste it somewhere: versions and states only, no network or device names, no notifications.
5. Make the window narrow: the page tabs move to the bottom.

### Battery

1. Plug in the charger: green “Charging” with the battery percentage on the right, also when the island is closed; the icon matches GNOME's own battery icon in the top bar.
2. Unplug: “On battery”, white, with the percentage. When full: green “Fully charged” with the full-battery icon.

### Commands and system updates

1. `selawi-island run -- sleep 5`: “sleep” with a timer, then Done · 0:05. `selawi-island run -- false`: Failed (exit 1). Ctrl+C during `selawi-island run -- sleep 60`: Stopped.
2. `yay -S` a small package you don't have: “Installing name” with a timer, then “Installed name”. `yay -R` it: “Removing name”, then “Removed”.
3. `yay -Syu` with updates: “Updating N packages” and a few names, then “Updated N packages”. Pacman's output shows one extra line, “Telling Selawi Island...”.
4. Start an update and answer “n” at pacman's prompt, or press Ctrl+C while it installs: nothing shown, or Failed “Did not finish”.
5. Integrations → turn off System updates: installs no longer show.

### Arch package

1. From the repository: `./scripts/update.sh`. It removes an older home-folder install (settings kept), builds with makepkg, asks for your password, installs. `pacman -Q selawi-island` shows a version like `0.1.0.r412.gabc1234-1`.
2. Log out and in: the island works as before and keeps your settings.
3. `selawi-island-update` from any folder: pulls, rebuilds, reinstalls.
4. Chrome: Load unpacked from `/usr/share/selawi-island/browser/chrome` (remove the old one first); a Meet call still shows.
5. `sudo pacman -R selawi-island`, log out and in: the island is gone; reinstall brings it back with the same settings.
