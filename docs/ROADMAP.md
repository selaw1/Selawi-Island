# Roadmap

Status key: **Done** · **In progress** · **Next** · Planned

| Phase | Scope | Status |
|---|---|---|
| 1 | Repository, conventions, architecture docs, event protocol, Rust core | **Done** |
| 2 | GNOME extension: pill, compact/expanded, animation, hover/click, simulated events | **Done** — awaiting manual check on GNOME 51 |
| 3 | Rust daemon, D-Bus (`com.selawi.Island`), lifecycle/routing via `selawi-core`, systemd user service, `DaemonFeed` in the extension, CLI basics | **Next** |
| 4 | Linux integrations: notifications, MPRIS, Bluetooth (BlueZ), power, audio | Planned |
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

## Phase 3 acceptance criteria (proposed)

- `selawi-islandd` runs as a systemd user service and owns `com.selawi.Island`.
- `Events1` exposes: submit an event (JSON, validated by `selawi-core`),
  dismiss, invoke action, list current events, and a `Changed` signal.
- One expiry timer from `next_deadline()`; no polling.
- Persistent events survive a daemon restart (snapshot on disk, user only).
- The extension uses the daemon when it is running and shows the idle pill
  when it is not; the simulator stays available as a setting.
- `selawi-island notify|progress` CLI talks to the daemon over D-Bus.
