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

## T-006 — Microphone/camera in use and voice waves
- **Created:** 2026-11-09
- **Status:** Done (manual check pending)
- **Description:** Extension privacy source (mixer recording streams +
  Shell.CameraMonitor) → `privacy:capture` event; orange/green view with
  timer; GStreamer level meter gated on "waves on screen and microphone in
  use"; wave bars in compact and expanded views; two settings.
- **Files:** `gnome-extension/services/{privacy-source,level-meter,daemon-feed}.js`,
  `gnome-extension/ui/{level,waves,view-model,compact-view,expanded-view,island}.js`,
  `extension.js`, `prefs-integrations.js`, schema, stylesheet, tests, docs.
- **Dependencies added:** none in the repo; uses GStreamer's `pulsesrc`
  and `level` at runtime (gst-plugins-good), optional.
- **Tests:** 44 JS tests; meter checked under GJS (graceful failure with no
  sound server, 12 readings/s with a test tone, stops on demand); headless
  preferences test. Real microphone and camera untested.
- **Remaining:** manual checks; Phase 6.
- **Follow-up (same day):** Mute/Unmute backup on the microphone event
  (apps or whole microphone, undone after the call), with the button press
  routed daemon → extension via `ActionInvoked`; tested end to end.

## T-007 — Phase 6 (part 1): browser helper, Meet, downloads
- **Created:** 2026-11-10
- **Status:** Done (manual check pending)
- **Description:** Native messaging helper (Rust) with strict checks;
  Chrome/Firefox MV3 extension in strict TypeScript; Google Meet call with
  mute state, Mute/Unmute and Leave pressing Meet's buttons; downloads
  with progress and Cancel; `muted` added to call payloads; the mic
  bubble folds into a confirmed call; install/uninstall of the helper,
  browser registrations and the built extension.
- **Files:** `native-messaging/**`, `browser-extensions/**`,
  `core/src/events.rs`, `gnome-extension/services/protocol.js`,
  `gnome-extension/ui/{view-model,island}.js`, `scripts/*.sh`,
  `tests/browser/**`, `tests/integration/browser-*`, fixtures, docs.
- **Dependencies added:** dev-only `typescript` 7 and `@types/chrome`
  (pnpm, `pnpm-lock.yaml`).
- **Tests:** helper unit tests (6), extension model tests (4), helper
  against the daemon, and the built extension in real Chromium with a
  stand-in Meet page — all passing.
- **Remaining:** check on the real Meet; per-tab media and ads; Zoom/Teams web.

## T-008 — Phase 6 (part 2): media in each tab, ads
- **Created:** 2026-11-10
- **Status:** Done (manual check pending)
- **Description:** media content script (YouTube, YouTube Music,
  SoundCloud) reading the media keys data and the page's player; one
  event per tab with only the controls the page offers; last real title
  kept during YouTube ads; the browser's own MPRIS player hidden while the
  extension shows its tabs; browser family detected from the extension's
  own address (newer Chrome also has `browser`).
- **Files:** `browser-extensions/src/{media,background,model}.ts`,
  manifests, `scripts/build-browser.sh`, `gnome-extension/ui/{view-model,island}.js`,
  tests, docs.
- **Tests:** real Chromium with stand-in YouTube and SoundCloud pages
  (found and fixed: Chrome mistaken for Firefox; a paused clip at 0:00
  ending its tab); stable over repeated runs.
- **Remaining:** check on the real sites; Zoom/Teams/Slack web calls.

## T-009 — Calls on any website
- **Created:** 2026-11-10
- **Status:** Done (manual check pending)
- **Description:** generic call detection for every https site: a
  MAIN-world script wraps RTCPeerConnection and reports "live connection
  sending the microphone" yes/no; the background shows a green call with
  the site's name (known sites named, others by address), no buttons. The
  mic bubble folds into it, so it shows the voice waves.
- **Files:** `browser-extensions/src/{rtc-main,call,background,model}.ts`,
  manifests (https sites), build script, tests, docs.
- **Tests:** a real WebRTC call with Chromium's fake microphone on a
  stand-in Slack page; a microphone-less connection is not a call.

## T-010 — Fixes after first real tests
- **Created:** 2026-11-10
- **Status:** Done (manual check pending)
- **Description:** microphone Mute/Unmute switched to GNOME's system
  microphone mute (reflected in GNOME, state follows it, Unmute also clears
  app-level mutes; scope setting removed); compact text always left with
  time on the right; app names without "input"; no duplicate mute popup;
  camera shown in calls (optional `camera` on call payloads, Meet reads its
  camera button, the camera bubble's state carries into the call).
- **Files:** `gnome-extension/services/{privacy-source,privacy-model,audio-source}.js`,
  `gnome-extension/{extension.js,prefs-integrations.js,ui/compact-view.js}`,
  schema, tests, docs.

## T-011 — Logos
- **Created:** 2026-11-10
- **Status:** Done (manual check pending)
- **Description:** optional `source.icon` on events (`app:<desktop id>`
  or a PNG data URL up to 48 KiB, validated in Rust and JS); MPRIS and
  notifications set the app's icon from its desktop entry; the browser
  extension redraws each tab's favicon as a 64×64 PNG and attaches it to
  calls, Meet and media; the island shows the logo in place of the icon
  and moves muted / camera / paused to small icons beside the time.
- **Files:** `core/src/events.rs`, `daemon/src/integrations/{mod,mpris,notifications}.rs`,
  `gnome-extension/{services/protocol.js,ui/*.js,stylesheet.css}`,
  `browser-extensions/{src/background.ts,src/model.ts,chrome/manifest.json}`,
  `tests/fixtures/protocol-cases.json`, tests, docs.
- **Tests:** protocol cases (valid and invalid icons), fake MPRIS player
  with a desktop entry, view model logo and indicators, browser e2e (Meet
  and YouTube stand-ins carry their favicon as a PNG).

## T-012 — Playing media first
- **Created:** 2026-11-10
- **Status:** Done (manual check pending)
- **Description:** the island orders media so playing comes before
  paused (`playingFirst` in the view model); among playing ones the newest
  stays first. Other events keep their places.
- **Files:** `gnome-extension/ui/{view-model,island}.js`, `tests/js/view-model.test.js`, docs.
- **Tests:** view model test (paused/playing, both playing, mixed with a call).
