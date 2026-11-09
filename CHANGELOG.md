# Changelog

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
