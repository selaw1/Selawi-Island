# Known limitations

## Current (after Phase 3)

- **Automatic detection covers media (MPRIS), battery (UPower) and
  Bluetooth (BlueZ) only.** Notifications, audio, network and calls come
  later. Per-integration limits are in `docs/INTEGRATIONS.md`.
- **No album artwork yet** (the event protocol has no image field).
- **Several media players are all shown**; there is no preferred-player
  setting yet.
- **Integrations are configured by a JSON file**, not in preferences yet.
- **Actions reach only D-Bus sources.** `ActionInvoked` is a signal; the CLI
  exits after sending, so CLI events offer no action buttons (only Dismiss).
- **Any program of the same user can send events.** The session bus is
  per-user, so other users cannot, but there is no per-app permission yet
  (plugin permissions come in Phase 9).
- **The CLI is stateless.** `progress update/finish` read the current event
  from the daemon first; if it already expired, they fail with a message.
- **Demo uses the JavaScript engine copy**, not the daemon.
- **One position for all monitors.** The default position and the dragged
  position apply to the one monitor the island is on.
- **Dragging is mouse/touchpad only**; there is no keyboard way to move it.
- **Code updates need a re-login** on Wayland for the extension (the daemon
  is restarted by the install script).
- **Monitor choice is by index** (-1 = primary), not by name.
- **Only GNOME 51** is declared in `metadata.json`.
- **A 1-second timer runs in the extension while a call timer or playing
  media is visible.** Everything else is event-driven.
- **Quick preferences only.** The full settings app is Phase 10.
- **Integrations were tested against fake services**, not yet against
  your real players and devices (see `docs/TESTING.md`).
