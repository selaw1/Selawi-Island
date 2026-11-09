# Known limitations

## Current (after Phase 3)

- **No automatic detection yet.** Events come from the CLI, from any
  program using the D-Bus interface, or from the demo. Music, Bluetooth,
  battery, notifications and calls are detected in Phase 4 and later.
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
- **Phase 3 was tested on a private D-Bus bus in CI, not yet in your
  GNOME session** (see `docs/TESTING.md`).
