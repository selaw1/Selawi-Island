# Changelog

## Unreleased — Phase 8 (part 1): commands and system updates

### Added
- `selawi-island run -- COMMAND`: shows any command running with a timer,
  then Done (with its time), Failed (exit N) or Stopped; keeps the
  command's output and exit code.
- System updates: installs, updates and removals with pacman or yay show
  in the island (“Updating 23 packages” → “Updated 23 packages”, Failed
  “Did not finish” if stopped), through pacman hooks that never block a
  transaction and send as the logged-in user, not root. On/off in
  Integrations → System updates.
- Progress without a percentage (commands, updates) shows a running timer.

## Unreleased — Arch package

### Added
- Arch package built from the repository (`packaging/arch/PKGBUILD`):
  install and update with `./scripts/update.sh` the first time, then
  `selawi-island-update`; remove with `sudo pacman -R selawi-island`. An
  older home-folder install is removed automatically (settings kept).
- `docs/INSTALL.md`: setup for you or a friend with access to the private
  repository.
- `scripts/firefox-xpi.sh`: the Firefox extension as a file to sign for free
  on addons.mozilla.org (unlisted, private).

### Fixed
- `selawi-island-update` no longer leaves the PKGBUILD looking edited
  (makepkg writes the version into it; the update now puts it back).
- Updates are much faster: the package build reuses your Cargo download
  cache and keeps compiled output in `~/.cache/selawi-island`, so only
  what changed is rebuilt.

## Unreleased — call controls and app logos

### Added
- Calls on any website show “Muted”, “Camera on”, **Mute / Unmute** and
  **Leave** when the site reports them through Chrome's Media Session call
  controls; the buttons use the site's own controls. Meet uses them first,
  its page buttons otherwise.
- The microphone bubble shows the recording app's logo (Slack, Zoom…),
  still orange; muted and camera show as small icons next to the time.

- When a website uses the microphone, the orange bubble shows that site's
  name and logo instead of “Google Chrome”.

### Changed
- No Meet-specific code any more: Meet is found like any other web call
  (shown as “Google Meet”, without the meeting name), with microphone,
  camera, Mute / Unmute and Leave from its standard call controls
  (confirmed on real Meet). A call now goes on while muted as long as the
  page holds the microphone.

### Fixed
- A logo in the expanded island no longer sits on a coloured square.
- Browsers no longer get their own logo on the microphone bubble.

## Unreleased — playing media first

### Changed
- With two media at once, the closed island shows the one that is playing,
  not a paused one. If both play, the one started last, as before.

## Unreleased — logos

### Added
- Logos instead of generic icons: the site's icon for calls, Meet and
  browser media tabs; the app's own icon for media players (Spotify…) and
  notifications. Muted microphone, camera on and paused show as small
  icons next to the time. New optional `source.icon` in the event protocol
  (`app:<desktop id>` or a small PNG).

### Changed
- Chrome: the extension asks for website access as a host permission too
  (it already had it through its call detection), so it can read each
  tab's icon.

## Unreleased — fixes after testing

### Changed
- The microphone bubble's Mute is now GNOME's own microphone mute: Linux
  shows the microphone as muted, and muting from GNOME shows in the
  bubble. The “Mute button mutes” setting is gone.
- Compact island: icon and title always on the left; “+N”, waves and the
  time on the right, for every kind of event.
- App names in the microphone bubble without “input” (e.g. “Google Chrome”).

### Fixed
- Unmute could fail when an app's own microphone connection had been left
  muted; Unmute now clears that too.
- No extra “Microphone muted” popup while the microphone bubble shows it.

## Unreleased — Phase 6 (part 2)

### Added
- Media in each browser tab (YouTube, YouTube Music, SoundCloud): several
  tabs at once, with Previous / Play–Pause / Next pressing the page's own
  controls.
- During YouTube ads the song title stays and the ad's timer is hidden.
- Calls on any website (Slack, Teams, Zoom, Discord, WhatsApp…): a green
  bubble with the site's name, a timer and voice waves; no buttons. The
  extension now needs access to https sites for this.
- While the extension shows a browser's tabs, that browser's own single
  media player is hidden (no doubles).

### Fixed
- Newer Chrome was detected as Firefox by the extension.

## Unreleased — Phase 6 (part 1)

### Added
- Browser extension for Chrome/Chromium/Brave and Firefox, with a native
  messaging helper (`selawi-island-browser-host`) that checks every event.
- Google Meet: green call with the meeting name and timer, “Muted” when
  Meet says so, Mute/Unmute and Leave that press Meet's own buttons.
- Browser downloads: name, progress, Done/Cancelled/Failed, Cancel button.
- Event protocol: optional `muted` on call payloads (only when known).
- The browser extension is built with pnpm (`pnpm-lock.yaml`).
- During a confirmed call, the microphone bubble is folded into the call,
  which shows the voice waves.

## Unreleased — microphone and camera

### Added
- “Microphone in use” (orange) and “Camera in use” (green) with the app
  name and a running timer, from GNOME's own mixer and camera monitor.
- Voice waves: bars that move with your voice, measured only while the
  microphone event is on screen and another app is using the microphone.
- Integrations page: “Microphone and camera in use” and “Voice waves”.
- Mute / Unmute on the microphone event (system-level backup): mutes the
  recording apps, or the whole microphone; undone automatically after the
  call so the next call starts unmuted.

## Unreleased — Phase 5

### Added
- Network (NetworkManager): connection changes, offline, “Sign in to
  network” for captive portals, VPN connected/disconnected.
- USB drives (UDisks2): plugged in and removed.
- System warnings (opt-in): low memory and high CPU temperature.
- Network, drive and warning icons; three new switches on the
  Integrations page.

### Changed
- Integrations can have a default: System warnings is off unless turned
  on. `Configuration1.Get` reports every integration's on/off state.

## Unreleased — Phase 4 (part 2)

### Added
- Notifications in the island (watched as a D-Bus monitor, GNOME stays
  the notification server). GNOME's own banners are hidden while the
  island shows notifications, and come back if the daemon stops.
- Audio device events from GNOME's mixer: output and microphone changes,
  microphone muted/on.
- Media: “All players” or “One player” with a preferred player.
- Preferences → Integrations page: turn sources on/off, media mode and
  preferred player, notification text, GNOME banners, audio devices.
- `com.selawi.Island.Configuration1` (Get/Set/Changed), and
  `Integrations1.MediaPlayers()` / `Integrations1.Changed`.
- Battery events show battery-level icons; audio events show speaker or
  microphone icons.

### Changed
- Turning an integration off or on applies immediately (no restart) and
  removes its events.

## Unreleased — Phase 4 (part 1)

### Added
- Media players (MPRIS): Spotify, browsers, VLC, mpv and others, with
  Previous / Pause–Play / Next buttons that control the player.
- Battery (UPower): charging, on battery, fully charged, low, critical.
- Bluetooth (BlueZ): connect/disconnect with reported battery; low battery.
- `com.selawi.Island.Integrations1` and `selawi-island integrations`.
- `~/.config/selawi-island/daemon.json` to turn integrations off.

### Changed
- Compact island: icon and title are centered when they fit; a long
  title starts at the left instead, using the full width up to the
  time/percent on the right. “+N” sits at the left edge.
- Paused media with no known length no longer shows a frozen “0:00”.
- The daemon is a library plus a small binary, so integrations can be
  tested against fake services.

## Unreleased — Phase 3

### Added
- `selawi-islandd` daemon: event engine on the session bus as
  `com.selawi.Island` with `Core1` (versions) and `Events1` (Submit, List,
  Dismiss, InvokeAction, Changed, ActionInvoked). One expiry timer, no
  polling. Persistent events saved to `~/.local/state/selawi-island/` and
  restored with a 60 s grace period.
- systemd user service `selawi-islandd` and D-Bus activation.
- `selawi-island` CLI: notify, progress start/update/finish, list,
  dismiss, send, status.
- Extension `DaemonFeed`: the island now shows the daemon's events.
- End-to-end test on a private D-Bus bus, including the extension's
  daemon feed under GJS.

- Drag the island anywhere (any monitor); the position is remembered.
  Dropping it near the top center snaps it back. Preferences: “Allow
  dragging” and “Reset” for the dragged position.
- Clicking the idle island expands it to “Nothing to show”.
- Right-click the island to open its preferences.
- Choose the default position: top center/left/right, center, bottom
  center/left/right, or a custom spot saved from a drag (“Make this the
  default”). The island grows away from the edge it is attached to.
  Reset and snap-back use the chosen default.
- The idle island is as wide as the top bar clock (Preferences → “Idle
  width matches the clock”).

### Fixed
- Dragging stopped halfway and snapped back when the pointer left the
  island; the island now holds the pointer for the whole drag.

### Changed
- The idle island is a slim 12 px bar that grows under the pointer
  (Preferences → Idle height). Turn off “Show when idle” to hide it.
- Demo events are now off by default (Preferences → Demo events).
- `EventStore::restore` takes the current time and a grace period.
- `install-local.sh` builds and installs the daemon, CLI and service too;
  `uninstall.sh` removes them (`--purge` also removes settings and data).

## Unreleased — Milestone 1 (Phases 1–2)

### Fixed
- Extension failed to load on GNOME 51: `addTopChrome()` no longer accepts
  `affectsInputRegion`.
- Replaced icons missing from current Adwaita (notification, finished
  download, generic progress).

### Added
- Rust workspace with `selawi-core`: event protocol v1 types, input
  validation, lifecycle rules, eight priority tiers, and an event store with
  deduplication, grouping, expiry, linger after finishing, dismissal,
  capacity limits and snapshot/restore.
- GNOME Shell 51 extension: black pill in the top chrome with idle, compact
  and expanded states, interruptible morph animation, hover growth, click to
  expand, Ctrl+Alt+Tab keyboard access, action buttons, dismissal, an
  event list for multiple events, fullscreen hiding, multi-monitor and HiDPI
  placement, and quick preferences.
- Simulator that plays `gnome-extension/data/demo-scenario.json`.
- Shared protocol test cases used by both Rust and JavaScript tests.
- Scripts: `install-local.sh`, `uninstall.sh`, `dev.sh`, `test.sh`.
