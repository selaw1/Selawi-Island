# Selawi Island

A Dynamic Island-style activity center for the desktop: a black pill at the
top of the screen that shows calls, music, downloads, devices, notifications
and system events, and expands when you need more detail.

Open source (MIT). Runs entirely on your own computer: no accounts, no
cloud, no hosting.

- Case study: <https://yousef.selawii.com/work/selawi-island>
- What it cannot do (yet): [docs/KNOWN_LIMITATIONS.md](docs/KNOWN_LIMITATIONS.md)

## Status

| Part | Status |
|---|---|
| Shared Rust core (event model, lifecycle, priority, expiry, grouping) | Implemented, tested |
| GNOME Shell extension (GNOME 51): pill, compact/expanded views, animation, hover, click, keyboard, dragging | Implemented, checked on GNOME 51 |
| Daemon `selawi-islandd` on D-Bus + systemd user service | Implemented, tested on a private bus |
| CLI `selawi-island` (notify, progress, list, dismiss, send, status, integrations, run) | Implemented, tested |
| Demo events | Simulated, off by default |
| Automatic detection: music players, battery, Bluetooth, notifications (including GNOME's own), audio devices, microphone and camera in use, network/VPN, USB drives, “Copied”, opt-in system warnings | Implemented ([INTEGRATIONS](docs/INTEGRATIONS.md)) |
| Browser extension: calls on any website (Mute / Leave where the site offers standard call controls), media in each tab (YouTube, YouTube Music, SoundCloud), downloads | Implemented ([BROWSER_INTEGRATIONS](docs/BROWSER_INTEGRATIONS.md)) |
| Developer workflows: `selawi-island run`, system updates with pacman and yay | Implemented ([CLI](docs/CLI.md)) |
| Plugins: your own apps show their own bubbles (`Plugins1`, Python and Rust helpers) | Implemented ([PLUGINS](docs/PLUGINS.md)) |
| Settings app (GTK 4 / libadwaita): Island, Integrations and Status pages | Implemented, manual check pending |
| Performance: load check in the test suite, multi-monitor cases | Implemented, tested |
| Arch package with one-command install and update | Implemented, manual check on Arch pending |
| Other Linux distributions, Windows, macOS | Not planned ([ROADMAP](docs/ROADMAP.md)) |

## Requirements (all free)

- Arch Linux with GNOME 51
- Rust to build the daemon and CLI: `sudo pacman -S --needed base-devel git rust`
- Node.js only to run the JavaScript unit tests: `sudo pacman -S --needed nodejs`
- For voice waves: `gst-plugins-good` (normally already installed with GNOME)
- To build the browser extension: `sudo pacman -S --needed nodejs pnpm`

## Install or update

Selawi Island installs as an Arch package built from this repository
(see [docs/INSTALL.md](docs/INSTALL.md)):

```bash
git clone https://github.com/selaw1/Selawi-Island.git
cd Selawi-Island
./scripts/update.sh          # first time, from this folder
selawi-island-update         # every update after that
```

It builds the package, installs it with pacman, starts the
`selawi-islandd` user service and turns on the GNOME extension. Log out
and back in afterwards: GNOME on Wayland loads extension code only at
login.

For development, `./scripts/install-local.sh` still installs into your
home folder instead (remove that with `./scripts/uninstall.sh` before using
the package).

Then try it:

```bash
selawi-island notify "Hello from the terminal"
```

More in [docs/CLI.md](docs/CLI.md). Your own apps can show their own
bubbles with buttons: [docs/PLUGINS.md](docs/PLUGINS.md).

**Settings:** right-click the island, or open **Selawi Island** from the
app grid (`selawi-island-settings`). Island page: hide it when idle,
sizes, dragging, position, demo. Integrations page: sources, preferred
media player, notification options. Status page: is the service running
(Restart), is the extension on or in error (Turn on), what each source is
doing, and **Copy report** for asking for help. The same pages are in
`gnome-extensions prefs selawi-island@selawi`.

Remove: `sudo pacman -R selawi-island` (settings are kept; see
[docs/INSTALL.md](docs/INSTALL.md#remove)).

What to check is in [docs/TESTING.md](docs/TESTING.md#manual-checks-on-gnome).

## Run the automated checks

```bash
./scripts/test.sh
```

## Repository layout

```text
core/               Rust: platform-independent event model and engine
daemon/             Rust: selawi-islandd, the D-Bus service and integrations
cli/                Rust: selawi-island command-line client
native-messaging/   Rust: helper the browser extension talks to
browser-extensions/ TypeScript: Chrome and Firefox extension
gnome-extension/    GNOME Shell extension (the island UI)
  ui/               pill, compact/expanded views, monitor placement
  services/         protocol validation, event store, GNOME sources
  settings/         settings pages shared with the settings app
  data/             demo scenario played by the simulator
settings-app/       GTK 4 / libadwaita settings app
sdk/                plugin helpers for Python and Rust
packaging/          Arch PKGBUILD, systemd and D-Bus files, pacman hooks, app entry
tests/              JavaScript, browser, integration, plugin and load tests
platform/           notes for possible Windows/macOS ports
scripts/            install, update, package, dev, test, uninstall
docs/               architecture, event system, testing, limitations, roadmap
```

## How it was built

Built with Claude Code. I plan each project in a long conversation with an AI assistant, which writes up what we settle as a brief. Claude Code then builds from that brief while I test it on real hardware, change scope and steer. The rules in CLAUDE.md and the notes in docs/ are the working record. More about the method: https://yousef.selawii.com/notes/building-with-agents

## License

MIT. See [LICENSE](LICENSE).
