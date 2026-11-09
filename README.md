# Selawi Island

A Dynamic Island-style activity center for the desktop: a black pill at the
top of the screen that shows calls, music, downloads, devices, notifications
and system events, and expands when you need more detail.

Private project. Runs entirely on your own computer: no accounts, no cloud,
no hosting.

## Status

| Part | Status |
|---|---|
| Shared Rust core (event model, lifecycle, priority, expiry, grouping) | Implemented, tested |
| GNOME Shell extension (GNOME 51): pill, compact/expanded views, animation, hover, click, keyboard | Implemented, checked on GNOME 51 |
| Daemon `selawi-islandd` on D-Bus + systemd user service | Implemented, tested on a private bus |
| CLI `selawi-island` (notify, progress, list, dismiss, send, status) | Implemented, tested |
| Demo events | Simulated, off by default |
| Automatic detection: music players, battery, Bluetooth | Implemented ([INTEGRATIONS](docs/INTEGRATIONS.md)) |
| Notifications, audio, network, calls | Not started (see [ROADMAP](docs/ROADMAP.md)) |

## Requirements (all free)

- Arch Linux with GNOME 51
- Rust to build the daemon and CLI: `sudo pacman -S --needed base-devel git rust`
- Node.js only to run the JavaScript unit tests: `sudo pacman -S --needed nodejs`

## Install or update

```bash
./scripts/install-local.sh
```

This builds and installs the daemon and CLI into `~/.local/bin`, starts the
`selawi-islandd` user service, and installs the GNOME extension. The first
time (and after extension updates), log out and back in: GNOME on Wayland
loads extension code only at login.

```bash
gnome-extensions enable selawi-island@selawi
```

Then try it:

```bash
selawi-island notify "Hello from the terminal"
```

More in [docs/CLI.md](docs/CLI.md).

**Preferences:** right-click the island, or run
`gnome-extensions prefs selawi-island@selawi`. There you can hide it when
idle, change sizes, turn dragging off, reset its position or turn on the demo.

Remove everything: `./scripts/uninstall.sh` (add `--purge` to also delete
settings and saved events).

What to check is in [docs/TESTING.md](docs/TESTING.md#manual-checks-on-gnome).

## Run the automated checks

```bash
./scripts/test.sh
```

## Repository layout

```text
core/               Rust: platform-independent event model and engine
daemon/             Rust: selawi-islandd, the D-Bus service
cli/                Rust: selawi-island command-line client
packaging/          systemd user service and D-Bus activation file
gnome-extension/    GNOME Shell extension (the island UI)
  ui/               pill, compact/expanded views, monitor placement
  services/         protocol validation, event store, simulator
  data/             demo scenario played by the simulator
tests/js/           JavaScript unit tests (run with Node)
tests/integration/  end-to-end test on a private D-Bus bus
tests/fixtures/     protocol cases shared by Rust and JavaScript tests
platform/           notes for future Windows/macOS ports
scripts/            install, dev, test, uninstall
docs/               architecture, event system, testing, limitations, roadmap
```
