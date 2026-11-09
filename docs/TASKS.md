# Task history

## T-001 — Milestone 1: foundation and island UI
- **Created:** 2026-11-09
- **Status:** Done (manual GNOME check pending)
- **Description:** Phases 1–2: repository, conventions, event protocol v1,
  Rust core, GNOME 51 extension with simulated events.
- **Files:** `Cargo.toml`, `core/**`, `gnome-extension/**`, `tests/**`,
  `scripts/*.sh`, `docs/*`, `README.md`, `CLAUDE.md`, `CHANGELOG.md`,
  `package.json`, `platform/*/README.md`.
- **Tests:** `./scripts/test.sh` — 33 Rust tests, 20 JS tests, syntax and
  schema checks, all passing. UI not run on GNOME (see KNOWN_LIMITATIONS).
- **Remaining:** manual checks in `docs/TESTING.md`; Phase 3.

## T-002 — Phase 3: daemon, D-Bus, CLI
- **Created:** 2026-11-09
- **Status:** Done (manual GNOME check pending)
- **Description:** Rust daemon `selawi-islandd` serving `com.selawi.Island`
  (`Core1`, `Events1`), persistence with restore grace period, systemd user
  service + D-Bus activation, CLI `selawi-island`, extension `DaemonFeed`
  (default; demo now opt-in).
- **Files:** `daemon/**`, `cli/**`, `core/src/{events,store}.rs`,
  `gnome-extension/{extension.js,prefs.js,services/daemon-feed.js,services/protocol.js,schemas/*}`,
  `packaging/**`, `scripts/*.sh`, `tests/integration/**`,
  `tests/js/stored-events.test.js`, docs.
- **Dependencies added:** `zbus` 6 (D-Bus), `tokio` 1 (async runtime),
  `clap` 4 (CLI arguments).
- **Tests:** `./scripts/test.sh` — 38 Rust tests, 23 JS tests, end-to-end
  D-Bus test (daemon + CLI + DaemonFeed under GJS), all passing.
- **Remaining:** manual checks in `docs/TESTING.md`; Phase 4 integrations.

## T-003 — Phase 4 (part 1): media, battery, Bluetooth
- **Created:** 2026-11-09
- **Status:** Done (manual check pending); notifications and audio remain
- **Description:** Daemon split into library + binary; `Hub` for engine,
  signals, action routing and integration status; `Integrations1`;
  `daemon.json` on/off config; MPRIS, UPower and BlueZ integrations;
  `selawi-island integrations`; centered compact text in the island.
- **Files:** `daemon/src/{lib,hub,config}.rs`, `daemon/src/integrations/*`,
  `daemon/tests/fake_services.rs`, `daemon/src/{dbus,engine,main}.rs`,
  `core/src/{events,store}.rs`, `cli/src/main.rs`,
  `gnome-extension/ui/compact-view.js`, `gnome-extension/stylesheet.css`,
  scripts and docs.
- **Dependencies added:** `futures-util` (stream helpers), `serde` derive
  in daemon and CLI.
- **Tests:** `./scripts/test.sh` — 22 daemon unit tests, 4 integration
  tests against fake MPRIS/UPower/BlueZ services (stable over 10 runs),
  plus all earlier checks.
- **Remaining:** notifications, audio; artwork; preferred player.
