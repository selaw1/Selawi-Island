# Working on Selawi Island

The full project specification is the master plan the owner provided
(phases 1–14). This file holds the conventions that apply to every change.

## Before changing anything
1. Read `docs/ROADMAP.md` to find the current milestone.
2. Read `docs/ARCHITECTURE.md` and `docs/EVENT_SYSTEM.md`.
3. Check GNOME APIs against the GNOME Shell / mutter **51** source before
   using them (see `docs/GNOME_COMPATIBILITY.md`). Do not assume an API exists.

## Rules
- Rust: no `unwrap()`/`expect()` outside tests (clippy denies them), no
  `unsafe` (forbidden), keep `core/` free of any platform API.
- zbus is version 6: check its API in the cargo registry source, not memory.
- D-Bus interfaces are versioned (`Events1`); never change one
  incompatibly. Update `docs/DBUS.md` with any change.
- The daemon must not log event contents.
- Integrations keep their decisions in pure functions (unit-tested) and are
  also tested against fake services in `daemon/tests/fake_services.rs`.
- JavaScript: GNOME 45+ ES modules only. Files in `gnome-extension/services/`
  `protocol.js`, `event-store.js`, `ui/view-model.js`, `ui/placement.js`, `ui/level.js` and `services/privacy-model.js` must stay free of
  `gi://` imports so Node can test them.
- Browser extension: strict TypeScript, no `any`. Use pnpm (not npm or
  yarn) for the development dependencies. Content scripts must not
  import or export at runtime (the build checks). The helper only accepts
  `browser:` events.
- No shell execution as a shortcut for integrations. No telemetry. No cloud.
- Show only what a source actually reports; never invent data (battery,
  mute state, call status).
- Every disable() must remove all signals, timers and actors it created.
- Event protocol changes must update Rust (`core/src/events.rs`), JavaScript
  (`gnome-extension/services/protocol.js`), `docs/EVENT_SYSTEM.md` and
  `tests/fixtures/protocol-cases.json` together.
- Create directories only when code needs them.
- No license file: this is a private project.

## Checks
Run `./scripts/test.sh` before every commit. GNOME UI behaviour cannot be
tested in CI; list what needs manual checking in the milestone report.

## Docs to keep current
`docs/ROADMAP.md`, `docs/ARCHITECTURE.md`, `docs/TESTING.md`,
`docs/KNOWN_LIMITATIONS.md`, `docs/TASKS.md`, `docs/DBUS.md`, `docs/CLI.md`,
`docs/INTEGRATIONS.md`, `docs/BROWSER_INTEGRATIONS.md`,
`CHANGELOG.md`.
Describe what exists, not what is planned, except in the roadmap.
