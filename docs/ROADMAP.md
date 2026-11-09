# Roadmap

Status key: **Done** · **In progress** · **Next** · Planned

| Phase | Scope | Status |
|---|---|---|
| 1 | Repository, conventions, architecture docs, event protocol, Rust core | **Done** |
| 2 | GNOME extension: pill, compact/expanded, animation, hover/click, simulated events | **Done** (checked on GNOME 51) |
| 3 | Rust daemon, D-Bus (`com.selawi.Island`), lifecycle/routing via `selawi-core`, systemd user service, `DaemonFeed` in the extension, CLI basics | **Done** (checked on GNOME 51) |
| 4 | Linux integrations: notifications, MPRIS, Bluetooth (BlueZ), power, audio | **In progress** — MPRIS, power, Bluetooth done; notifications and audio next |
| 5 | Network, VPN, USB, storage, CPU/RAM/GPU, temperature, brightness | Planned |
| 6 | Browser extensions (Chromium, Firefox) + native messaging | Planned |
| 7 | Communication apps (Slack, Meet, Zoom, Teams, Discord, …) by reliable detection | Planned |
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

## Phase 4 status

1. **MPRIS media** — done: per-player events, Previous/Play-Pause/Next.
2. **Power** (UPower) — done: charger, full, low, critical.
3. **Bluetooth** (BlueZ) — done: connect/disconnect, reported battery, low battery.
4. **Notifications** — next: mirror freedesktop notifications without replacing GNOME's.
5. **Audio** — output device changes, volume/mute where reliably exposed.
6. Integration status (`Integrations1`, `selawi-island integrations`) and
   on/off configuration — done.
