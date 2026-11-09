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

## T-004 — Phase 4 (part 2): notifications, audio, settings
- **Created:** 2026-11-09
- **Status:** Done (manual check pending)
- **Description:** Live daemon configuration (`Configuration1`, saved to
  `daemon.json`), integrations stop/start live; media mode (all / one with
  preferred player); notifications integration (D-Bus monitor) with GNOME
  banner hiding; audio device and mic mute events from GNOME's mixer;
  Integrations preferences page; battery/audio icons.
- **Files:** `daemon/src/{config,hub,dbus,engine,main}.rs`,
  `daemon/src/integrations/{mod,mpris,notifications}.rs`,
  `daemon/tests/fake_services.rs`, `gnome-extension/{extension.js,prefs.js,prefs-integrations.js}`,
  `gnome-extension/services/{audio-source,daemon-feed}.js`,
  `gnome-extension/ui/{banner-blocker,view-model}.js`, schema, tests, docs.
- **Dependencies added:** none.
- **Tests:** 35 daemon unit tests, 7 fake-service integration tests
  (stable over 10 runs), headless preferences test against the real
  daemon, 38 JS tests, end-to-end D-Bus test — all passing. The headless
  preferences test found that GJS has no `structuredClone`; fixed.
- **Remaining:** manual checks on GNOME 51; Phase 5.

## T-005 — Phase 5: network, USB drives, system warnings
- **Created:** 2026-11-09
- **Status:** Done (manual check pending)
- **Description:** NetworkManager (connection changes, offline, captive
  portal, VPN), UDisks2 (USB drives in/out), opt-in health warnings (low
  memory, CPU temperature); per-integration defaults (health off);
  settings page rows; icons.
- **Files:** `daemon/src/integrations/{network,storage,health,mod}.rs`,
  `daemon/src/{config,dbus,main}.rs`, `daemon/tests/fake_services.rs`,
  `gnome-extension/{prefs-integrations.js,ui/view-model.js}`, tests, docs.
- **Dependencies added:** none.
- **Tests:** 47 daemon unit tests, 10 fake-service integration tests
  (stable over 10 runs), headless preferences test, 39 JS tests,
  end-to-end D-Bus test — all passing.
- **Remaining:** manual checks; Phase 6 (browser extensions).
