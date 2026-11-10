# Task history

## Waiting for the owner to test

Remind the owner about these when they ask what to test or what is next:

- **Slack in Chrome** (huddle or call): do Mute / Leave buttons appear in
  the island? (Yes means Slack uses Chrome's standard call controls.)
- **Slack desktop app** call: the orange mic bubble shows Slack's logo.
- **System updates:** `yay -S cowsay`, then `yay -R cowsay`; and the next
  `yay -Syu` (“Updating N packages” → “Updated”).
- **`selawi-island run -- sleep 5`**, then `selawi-island run -- false`.
- **GNOME notifications:** a screenshot (Print Screen) and a GNOME Clocks
  timer show in the island.
- **Copied:** Ctrl+C text, an image, files in Files.
- **Charging:** plug in and unplug; icon, green and percentage.
- **Settings app:** right-click the island; Status page, Restart, Copy report.
- **Plugins:** `cargo run -p selawi-island-plugin --example countdown`, Allow, run again; Settings → Apps.

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

## T-013 — Call controls and app logos (Phase 7 start)
- **Created:** 2026-11-10
- **Status:** Done (manual check pending)
- **Description:** browser: `session-main.ts` (MAIN world) notes the Media
  Session call controls a page reports (`setMicrophoneActive`,
  `setCameraActive`, `togglemicrophone` / `togglecamera` / `hangup`
  handlers), `session.ts` forwards them; web calls and Meet show muted /
  camera and get Mute / Leave that call the page's handlers, Meet falling
  back to its page buttons. GNOME: the mic bubble's app logo (matched via
  Shell.AppSystem from the stream's app id, icon name or a running app's
  exact name); small status icons with a logo; logo tile without the
  accent colour.
- **Files:** `browser-extensions/src/{session-main,session,model,background}.ts`,
  manifests, `scripts/build-browser.sh`,
  `gnome-extension/services/{privacy-source,privacy-model}.js`,
  `gnome-extension/{ui/view-model.js,stylesheet.css}`, tests, docs.
- **Tests:** model tests (parsing hostile input, which control to press,
  events with and without controls, Meet merge); browser e2e with a
  stand-in Teams page using the real Media Session API (Mute, Unmute,
  Leave through the page's handlers); privacy model and view model tests.
- **Follow-up:** browsers get no app logo (`isBrowserApp`); `mic-main.ts`
  notes whether a page holds a live microphone track and the extension
  sends `browser:mic:<tab>` (never shown itself); `foldBrowserMic` puts
  that site's name and logo on the microphone bubble. Tested in the
  browser e2e (Slack stand-in) and the view model tests.
- **Follow-up 2:** real Meet confirmed to report microphone, camera and
  all three controls through Media Session, so `meet.ts` no longer reads
  Meet's mute/camera buttons; it only reports “in a call” (leave button)
  and the title, and presses leave as a fallback.
- **Follow-up 3:** `meet.ts` removed: Meet goes through the general call
  detection (`rtc-main.ts` now runs on Meet too) and the standard call
  controls; no meeting name. A call survives a site dropping the
  microphone from its connection while muted, as long as the tab still
  holds the microphone (`mic-main.ts`). E2e: the Meet stand-in is a plain
  WebRTC call with Media Session controls that drops the track on mute.

## T-014 — Phase 12: Arch package
- **Created:** 2026-11-10
- **Status:** Done (manual check on Arch pending)
- **Description:** `packaging/arch/PKGBUILD` builds from the local
  repository (git+file, committed code only), version from Cargo + commit
  count; `scripts/stage-package.sh` lays out the system install (binaries,
  `selawi-island-update`, user service with `/usr/bin`, D-Bus activation,
  extension with compiled schema, browser files, helper registration for
  Chrome, Chromium, Firefox); `scripts/update.sh` (installed as
  `selawi-island-update`) pulls, removes an old home-folder install,
  runs `makepkg -si`, enables the service and extension;
  `scripts/firefox-xpi.sh` packs the Firefox add-on for unlisted signing;
  `docs/INSTALL.md`.
- **Files:** `packaging/arch/PKGBUILD`, `scripts/{stage-package,update,firefox-xpi,test}.sh`,
  `docs/INSTALL.md`, `README.md`, docs, `.gitignore`.
- **Tests:** `scripts/test.sh` checks script syntax and the staged file
  layout; the PKGBUILD's pkgver/prepare/build/package ran end to end on a
  fresh clone here (makepkg itself needs Arch: not available in this
  sandbox).

## T-015 — Phase 8 (part 1): commands and system updates
- **Created:** 2026-11-10
- **Status:** Done (manual check on Arch pending)
- **Description:** `selawi-island run` (spawns the command, Ctrl+C goes to
  it, Running → Done / Failed / Stopped with duration, exit code passed
  through; title from program + subcommand only). pacman hooks
  (`packaging/pacman/*.hook`, pre/post for install/upgrade and remove,
  never abort): `selawi-island pacman-hook` as root reads targets and the
  local database, then starts `selawi-island pacman-user` as each
  logged-in user (uid ≥ 1000 with a session bus; `Command::uid/gid`,
  cleared environment) to send `pacman:transaction`; a watcher as the
  user marks it “Did not finish” if pacman's lock goes away first.
  Daemon: `pacman` integration id, `Submit` drops events of turned-off
  integrations. Island: icons for updates and commands, running timer.
- **Files:** `cli/src/{main,run,pacman}.rs`, `cli/Cargo.toml` (tokio
  `signal`), `daemon/src/{config,hub}.rs`, `packaging/pacman/*.hook`,
  `scripts/{stage-package,test}.sh`, `gnome-extension/{ui/view-model.js,prefs-integrations.js}`,
  tests, docs.
- **Tests:** unit tests (titles, outcomes, package names, summaries,
  owning integration), D-Bus smoke test (run and hooks end to end), view
  model test; root → user hand-off and the watcher checked by hand with
  a throwaway user.

## T-016 — Notifications from GNOME, Copied, battery icons
- **Created:** 2026-11-10
- **Status:** Done (manual check pending)
- **Description:** daemon monitor also follows
  `org.gtk.Notifications.AddNotification` (app name from the desktop
  entry, priority mapped); the extension mirrors GNOME Shell's own
  notifications (sources that are neither `FdoNotificationDaemonSource`
  nor `GtkNotificationDaemonAppSource`) while banners are hidden;
  `ClipboardSource` (Meta.Selection `owner-changed`, memory sources
  ignored) sends a quiet “Copied” event that never auto-expands;
  battery icons follow GNOME's `status/system.js` (floor, charged), green
  accent and percentage on the right while charging. Checked: GNOME 51
  has no paste signal (only `owner-changed`) and Files exposes no
  progress (only GNotifications).
- **Files:** `daemon/src/integrations/notifications.rs`,
  `daemon/tests/fake_services.rs`, `gnome-extension/services/{shell-model,shell-notifications,clipboard-source,daemon-feed}.js`,
  `gnome-extension/{extension.js,prefs-integrations.js,ui/island.js,ui/view-model.js}`, schema,
  tests, docs.
- **Tests:** notification unit tests and fake GTK notification server;
  `tests/js/shell-model.test.js`; battery view-model tests.

## T-017 — Phase 10: settings app
- **Created:** 2026-11-10
- **Status:** Done (manual check pending)
- **Description:** settings pages moved to `gnome-extension/settings/`
  (Island, Integrations, new Status, pure `report.js`), shared by the
  extension's preferences window and a new GJS GTK 4 + libadwaita app
  (`settings-app/`: view switcher, narrow layout, banner when the
  extension's settings are missing). Status uses D-Bus only:
  `com.selawi.Island` (versions, events count, integrations),
  `org.gnome.Shell.Extensions` on `org.gnome.Shell` (GetExtensionInfo,
  GetExtensionErrors, EnableExtension, ExtensionStateChanged), systemd user
  manager `RestartUnit`. Copy report leaves out events and source details.
  Right-click opens the app when installed. Packaged with launcher,
  desktop entry and icon; also in install-local.sh / uninstall.sh.
- **Files:** `gnome-extension/{prefs.js,settings/*,extension.js}`,
  `settings-app/{main,window}.js`, `packaging/app/*`,
  `scripts/{stage-package,install-local,uninstall,test}.sh`, PKGBUILD
  depends (gjs, gtk4, libadwaita), tests, docs.
- **Tests:** settings app headless against a real daemon
  (`tests/integration/settings-app.gjs.js`), report unit tests, package
  layout check, install-local/uninstall run in a throwaway home.

## T-018 — Phase 11: performance and stability
- **Created:** 2026-11-10
- **Status:** Done
- **Description:** measured the daemon (idle 0 CPU, ~5.5 MB; memory
  levels off over 50 000 events; the engine keeps at most 64 events).
  Found every change sent a full `Changed` signal (5000 submits → 5000
  signals): the hub now sends at once after a quiet moment and then at
  most every 50 ms with the latest list (→ 10 signals). Audited timers:
  the island ticks only while something shows a running time; the voice
  meter only while waves show; browser timers only while a call,
  microphone or download is active; media tabs send only on real changes.
  Island logic for 64 events < 0.1 ms per redraw.
- **Files:** `daemon/src/{hub,main}.rs`, `tests/stress/{flood.gjs.js,measure.sh,stress-check.sh}`,
  `scripts/test.sh`, `tests/js/placement.test.js`, docs.
- **Tests:** `tests/stress/stress-check.sh` in the suite; placement cases
  for unplugged, smaller, rotated and negative-coordinate monitors.

## T-019 — Phase 9: plugins
- **Created:** 2026-11-10
- **Status:** Done (manual check pending)
- **Description:** `core::plugins` (manifest v1 parsing, `seal`: namespace
  `plugin:<id>:`, manifest name/icon, integration `plugin`, declared
  categories only, no critical priority; token-bucket rate limit);
  daemon `Plugins1` (Register/Submit/Remove/List, Changed, unicast
  ActionInvoked; ask-first question `plugins:request:<id>` with
  Allow/Don't allow routed to `config.plugins`; 8-event cap; events
  withdrawn on disconnect and when turned off; `Events1.Submit` refuses
  reserved ids and plugin buttons are not broadcast); settings Apps
  group; Python (PyGObject) and Rust helpers; docs/PLUGINS.md.
- **Files:** `core/src/{plugins,events,lib}.rs`, `daemon/src/{plugins,dbus,hub,engine,config,main,lib}.rs`,
  `daemon/tests/plugins.rs`, `sdk/python/selawi_island.py`, `sdk/rust/**`,
  `tests/sdk/*`, `gnome-extension/settings/integrations-page.js`,
  `tests/integration/settings-app.gjs.js`, packaging, docs.
- **Tests:** core unit tests (manifests, sealing, declared kinds, rate),
  daemon end to end over D-Bus, both helpers end to end, settings Apps
  group headless.
