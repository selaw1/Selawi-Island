# Architecture

Three layers. Each may depend only on the layers listed after it.

1. **Platform UI** — draws the island. Linux: the GNOME Shell extension.
2. **Platform adapters** — talk to the operating system (D-Bus, BlueZ, MPRIS,
   PipeWire, ...). Linux only for now; none exist yet.
3. **Shared core** — `core/` (`selawi-core` crate). Event model, validation,
   lifecycle, priority, expiry, grouping. No platform APIs.

## What exists today

```text
 gnome-extension/
 ┌──────────────────────────────────────────────────────────┐
 │ extension.js ── creates Island and a feed                │
 │                                                          │
 │ SimulatedFeed ──► EventStore ──► events() ──► Island     │
 │ (plays data/      (services/      ordered      (ui/)     │
 │  demo-scenario)    event-store)                │         │
 │        ▲                                       │         │
 │        └──── dismiss() / invokeAction() ◄──────┘         │
 └──────────────────────────────────────────────────────────┘

 core/  (Rust, not yet used at runtime)
 events.rs  lifecycle.rs  priority.rs  store.rs
```

### The feed boundary

The island never creates or changes events. It talks to a **feed** with
this interface:

| Member | Meaning |
|---|---|
| `events()` | All current events, most important first |
| `dismiss(id)` | The user dismissed an event |
| `invokeAction(id, actionId)` | The user pressed an action button |
| `'changed'` signal | `events()` changed |
| `destroy()` | Stop everything |

Today the only feed is `SimulatedFeed`. In Phase 3 a `DaemonFeed` with the
same interface will read the daemon over D-Bus, and the island will not
change.

### Rust core and JavaScript store

`core/src/store.rs` is the authoritative engine and will run inside the
daemon. Until the daemon exists, `gnome-extension/services/event-store.js`
applies the same rules inside the extension so the UI can be built against
realistic behaviour. Both sides are checked against the same test cases
(`tests/fixtures/protocol-cases.json`) and the same demo scenario.

### Island internals (`gnome-extension/ui/`)

| File | Job |
|---|---|
| `island.js` | Pill actor, idle/compact/expanded state, animation, hover, click, keys, auto-expand |
| `compact-view.js` | One-row content: icon, title, “+N”, trailing value |
| `expanded-view.js` | Detail: icon tile, title, subtitle, progress, actions, other events |
| `view-model.js` | Pure: event → icon, text, accent, progress (unit-tested) |
| `monitor-manager.js` | Which monitor, top offset, scale factor, fullscreen |
| `widgets.js` | Small St helpers |

Design rules: actors are built once and updated in place; size and position
are eased together with the same curve so the pill stays centered and new
targets interrupt smoothly; no polling (a 1-second timer runs only while a
call duration or playing media is on screen, and one timer is armed for the
next expiry).

## Planned (not built yet)

- **Daemon** (Rust, systemd user service) owning a `selawi-core` `EventStore`.
- **D-Bus** name `com.selawi.Island` with versioned interfaces `Core1`,
  `Events1`, `Integrations1`, `Plugins1`, `Configuration1`, `Diagnostics1`.
- **Adapters** under `platform/linux/` for notifications, MPRIS, BlueZ,
  power, audio.
- **CLI**, **settings app**, **browser extensions**, **plugins** — see the roadmap.

## Other platforms

Windows and macOS will reuse `core/` and the event protocol with their own
native overlay and adapters. Notes in `platform/windows/README.md` and
`platform/macos/README.md`. Nothing is implemented for them.
