# Roadmap

Status key: **Done** · **In progress** · **Next** · Planned

| Phase | Scope | Status |
|---|---|---|
| 1 | Repository, conventions, architecture docs, event protocol, Rust core | **Done** |
| 2 | GNOME extension: pill, compact/expanded, animation, hover/click, simulated events | **Done** (checked on GNOME 51) |
| 3 | Rust daemon, D-Bus (`com.selawi.Island`), lifecycle/routing via `selawi-core`, systemd user service, `DaemonFeed` in the extension, CLI basics | **Done** (checked on GNOME 51) |
| 4 | Linux integrations: notifications, MPRIS, Bluetooth (BlueZ), power, audio | **Done** (checked on GNOME 51) |
| 5 | Network, VPN, USB, storage, CPU/RAM/GPU, temperature, brightness | **Done** (checked on GNOME 51); brightness and GPU left out on purpose |
| 6 | Browser extensions (Chromium, Firefox) + native messaging | **Done** — Meet, per-tab media with ads hidden, downloads; web Zoom/Teams/Slack left for when real pages can be checked |
| 7 | Communication apps (Slack, Meet, Zoom, Teams, Discord, …) by reliable detection | **Next** |
| 8 | Files, packages, Git, Docker/Podman, build/test progress, full CLI | Planned |
| 9 | Plugin manifests, versioned protocol, isolation | Planned |
| 10 | GTK4/libadwaita settings app, diagnostics | Planned |
| 11 | Profiling, stress and multi-monitor testing | Planned |
| 12 | Arch package (PKGBUILD), install/update/uninstall | Planned |
| 13 | Other Linux distributions | Planned |
| 14 | Windows and macOS | Planned, only after Linux is stable |

## Phase 3 acceptance criteria (met)

- `selawi-islandd` runs as a systemd user service and owns `com.selawi.Island`.
- `Events1`: submit (validated by `selawi-core`), list, dismiss, invoke
  action, `Changed` and `ActionInvoked` signals. `Core1`: versions.
- One expiry timer from `next_deadline()`; no polling.
- Persistent events survive a daemon restart (user-only file), with a 60 s
  grace period so stale ones clear themselves.
- The extension uses the daemon by default and shows the idle pill when it
  is not running; the demo is a preference.
- `selawi-island notify|progress|list|dismiss|send|status`.

## Phase 4 (done)

1. **MPRIS media** — per-player events, Previous/Play-Pause/Next, all
   players or one (preferred) player.
2. **Power** (UPower) — charger, full, low, critical.
3. **Bluetooth** (BlueZ) — connect/disconnect, reported battery, low battery.
4. **Notifications** — observed as a D-Bus monitor; GNOME's banner hidden
   while the island shows them.
5. **Audio** (extension, GNOME mixer) — output/microphone changes, mic mute.
6. Integration status (`Integrations1`), live configuration
   (`Configuration1`) and the Integrations preferences page.

## Phase 5 (done)

- **Network** (NetworkManager): main connection changes, offline, captive
  portal sign-in, VPN up/down.
- **USB drives** (UDisks2): plugged in / removed.
- **System warnings** (opt-in): low memory, high CPU temperature.
- Left out on purpose: brightness and keyboard backlight (GNOME's own
  display), low disk space (GNOME's notification), GPU usage (unreliable).

## Phase 6 (done)

Native messaging helper; Chrome/Firefox extension; Google Meet (call,
mute state, Mute/Unmute and Leave pressing Meet's buttons, mic bubble
folded into the call); per-tab media for YouTube, YouTube Music and
SoundCloud with Previous/Play-Pause/Next, ads hidden, the browser's own
duplicate player hidden; downloads with progress and Cancel.

Not done: Zoom, Teams and Slack calls in the browser. Their pages could not
be checked from the development machine; they show as “Microphone in use”
with the backup mute meanwhile.
