# Known limitations

## Milestone 1

- **All events are simulated.** No real calls, music, Bluetooth, downloads
  or notifications are detected yet.
- **No daemon.** The Rust core is tested but not running. The extension
  uses its own JavaScript copy of the engine rules until Phase 3.
- **Not run on GNOME by the developer.** The extension was written and
  checked against the GNOME Shell and mutter 51.1 source and its pure logic
  is unit-tested, but the UI could only be verified on a real GNOME 51
  session (see `docs/TESTING.md`).
- **State is not kept across extension restarts.** Disabling/enabling
  restarts the demo from the beginning. Real recovery comes from the daemon.
- **Default position is below the top bar.** Placing it over the top bar
  covers the clock.
- **Code updates need a re-login** on Wayland (or a nested session).
- **Monitor choice is by index** (-1 = primary), not by name.
- **Only GNOME 51** is declared in `metadata.json`.
- **A 1-second timer runs while a call timer or playing media is visible.**
  Otherwise the island is fully event-driven.
- **Quick preferences only.** The full settings app is Phase 10.
