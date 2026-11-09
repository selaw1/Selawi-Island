# Selawi Island

A Dynamic Island-style activity center for the desktop: a black pill at the
top of the screen that shows calls, music, downloads, devices, notifications
and system events, and expands when you need more detail.

Private project. Runs entirely on your own computer: no accounts, no cloud,
no hosting.

## Status

**Milestone 1 (Phases 1–2): foundation and island UI, using simulated events.**

| Part | Status |
|---|---|
| Shared Rust core (event model, lifecycle, priority, expiry, grouping) | Implemented, tested |
| GNOME Shell extension (GNOME 51): pill, compact/expanded views, animation, hover, click, keyboard | Implemented, needs manual check on your machine |
| Demo events (call, music, download, Bluetooth, notification, battery) | Simulated |
| Daemon, D-Bus, real integrations, CLI, settings app, browser extensions | Not started (see [ROADMAP](docs/ROADMAP.md)) |

## Requirements (all free)

- Arch Linux with GNOME 51
- For building and testing the Rust core: `sudo pacman -S --needed base-devel git rust`
- Node.js is only needed to run the JavaScript unit tests: `sudo pacman -S --needed nodejs`

## Try it

```bash
./scripts/install-local.sh
```

Then log out and back in (GNOME on Wayland only loads extension code at
login) and enable it if it is not already:

```bash
gnome-extensions enable selawi-island@selawi
```

To try it without logging out, use a nested GNOME Shell window:

```bash
./scripts/dev.sh
```

Preferences: `gnome-extensions prefs selawi-island@selawi`.
Remove: `./scripts/uninstall.sh`.

What you should see is described in [docs/TESTING.md](docs/TESTING.md#manual-checks-on-gnome).

## Run the automated checks

```bash
./scripts/test.sh
```

## Repository layout

```text
core/               Rust: platform-independent event model and engine
gnome-extension/    GNOME Shell extension (the island UI)
  ui/               pill, compact/expanded views, monitor placement
  services/         protocol validation, event store, simulator
  data/             demo scenario played by the simulator
tests/js/           JavaScript unit tests (run with Node)
tests/fixtures/     protocol cases shared by Rust and JavaScript tests
platform/           notes for future Windows/macOS ports
scripts/            install, dev, test, uninstall
docs/               architecture, event system, testing, limitations, roadmap
```
