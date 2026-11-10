# Architecture

Three layers. Each may depend only on the layers listed after it.

1. **Platform UI** — draws the island. Linux: the GNOME Shell extension.
2. **Platform adapters** — talk to the operating system (D-Bus, BlueZ, MPRIS,
   PipeWire, ...). Linux only for now; none exist yet.
3. **Shared core** — `core/` (`selawi-core` crate). Event model, validation,
   lifecycle, priority, expiry, grouping. No platform APIs.

## What exists today

```text
  scripts, terminal                       any D-Bus client
  selawi-island (CLI) ─┐                 ┌─┘
                       ▼                 ▼
             ┌──────────────────────────────────────────┐
             │ selawi-islandd (daemon, Rust)             │
             │  D-Bus: com.selawi.Island                 │
             │   Core1 Events1 Integrations1 Configuration1
             │  Hub ─ Engine ─ selawi-core EventStore    │
             │  config: ~/.config/selawi-island/daemon.json
             │  integrations/ mpris upower bluez network │
             │                storage notifications ◄────┼── session / system bus
             │                health (opt-in, /proc /sys)│
             │  snapshot: ~/.local/state/…/events.json   │
             └──────────────────────────────────────────┘
                       │ Changed(json) signal / List()
                       ▼
 gnome-extension/  DaemonFeed ──► Island (ui/)
                   SimulatedFeed ─┘  (demo, when enabled in preferences)
```

| Part | Where | Job |
|---|---|---|
| Shared core | `core/` | Event types, validation, lifecycle, priority, expiry, grouping, snapshot/restore. No platform code |
| Daemon | `daemon/` | Owns the one `EventStore` behind a `Hub`, serves D-Bus, arms one expiry timer, saves persistent events, runs the integrations. systemd user service `selawi-islandd` |
| Integrations | `daemon/src/integrations/` | MPRIS media, UPower battery, BlueZ Bluetooth, notifications (as a D-Bus monitor), NetworkManager network/VPN, UDisks2 USB drives, opt-in system warnings. Each turns D-Bus signals into events, owns the event ids `"<id>:…"`, reports its status, follows the live configuration (turned off → stopped and its events withdrawn), and is restarted after failures. See [INTEGRATIONS.md](INTEGRATIONS.md) |
| Extension sources | `gnome-extension/services/audio-source.js`, `privacy-source.js` (+ pure `privacy-model.js`) | What only GNOME Shell can see, sent to the daemon with `Submit`: audio device changes, and microphone/camera in use. Resent when the daemon (re)starts. Buttons on these events come back from the daemon as `ActionInvoked` and are handled in the extension (Mute/Unmute) |
| Voice waves | `services/level-meter.js`, `ui/level.js`, `ui/waves.js` | Microphone loudness → bars. The meter runs only while the island shows the microphone event and another app is recording |
| Banner blocker | `gnome-extension/ui/banner-blocker.js` | Hides GNOME's notification banners while the daemon's notifications integration is active |
| Browser extension | `browser-extensions/` (TypeScript) | Google Meet calls and downloads; talks to the helper by native messaging. See [BROWSER_INTEGRATIONS.md](BROWSER_INTEGRATIONS.md) |
| Browser helper | `native-messaging/` (Rust, `selawi-island-browser-host`) | Started by the browser; checks every event (`browser:` ids only) and submits it over D-Bus; sends button presses back |
| Settings pages | `gnome-extension/settings/` | Island page (GSettings), Integrations page (daemon `Configuration1` + GSettings), Status page (service, extension state via GNOME Shell's `org.gnome.Shell.Extensions`, sources, Restart via the systemd user manager, “Copy report” built by the pure `report.js`) |
| Settings app | `settings-app/` (`selawi-island-settings`) | GJS GTK 4 + libadwaita app showing the shared pages; opened by right-clicking the island; works when the extension is off or broken |
| Preferences window | `gnome-extension/prefs.js` | The same pages in GNOME's extension preferences window |
| Plugins | `core/src/plugins.rs` (manifest, sealing, rate limit), `daemon/src/plugins.rs` (`Plugins1`), `sdk/python`, `sdk/rust` | Apps showing their own events: ask-first approval, own id namespace, manifest's name and icon, buttons sent only to the app, events gone when it disconnects. See PLUGINS.md |
| CLI | `cli/` | `selawi-island notify/progress/list/dismiss/send/status` over D-Bus |
| Extension | `gnome-extension/` | Draws the island from a feed |

The D-Bus contract is in [DBUS.md](DBUS.md); the CLI in [CLI.md](CLI.md).

### The feed boundary

The island never creates or changes events. It talks to a **feed**:

| Member | Meaning |
|---|---|
| `events()` | All current events, most important first |
| `dismiss(id)` | The user dismissed an event |
| `invokeAction(id, actionId)` | The user pressed an action button |
| `'changed'` signal | `events()` changed |
| `destroy()` | Stop everything |

- `DaemonFeed` (default) mirrors the daemon: it watches the bus name, calls
  `List()` when the daemon appears, follows `Changed`, and shows an empty
  island when the daemon is gone. The daemon sends `Changed` at once after
  a quiet moment and then at most every 50 ms while changes keep coming,
  so bursts never make the island redraw more than ~20 times a second. Every event is validated again
  (`protocol.js` `parseEventList`) before the UI sees it.
- `SimulatedFeed` plays `data/demo-scenario.json` through the JavaScript
  copy of the engine (`services/event-store.js`). It is used only when
  “Demo events” is on in preferences.

### Rust core and JavaScript store

`core/src/store.rs` is authoritative and runs inside the daemon. The
JavaScript `event-store.js` follows the same rules for the demo only. Both
are checked against `tests/fixtures/protocol-cases.json` and the demo scenario.

### Island internals (`gnome-extension/ui/`)

| File | Job |
|---|---|
| `island.js` | Pill actor, idle/compact/expanded state, animation, hover, click, keys, auto-expand |
| `compact-view.js` | One-row content: icon, title, “+N”, trailing value |
| `expanded-view.js` | Detail: icon tile, title, subtitle, progress, actions, other events |
| `view-model.js` | Pure: event → icon, text, accent, progress (unit-tested) |
| `monitor-manager.js` | Which monitor, top offset, scale factor, fullscreen |
| `placement.js` | Pure: default anchors (top/center/bottom × left/center/right, custom), dragged positions, keeping the pill on screen, snap-back on drop (unit-tested) |
| `widgets.js` | Small St helpers |

Design rules: actors are built once and updated in place; size and position
are eased together with the same curve so the pill stays centered and new
targets interrupt smoothly; no polling (a 1-second timer runs only while a
call duration or playing media is on screen).

## Planned (not built yet)

- Interface `Diagnostics1` (the settings app's Status page covers it for now).
- When Windows/macOS work starts, the Linux integrations move behind
  platform traits; until then they live in the daemon (no premature
  abstraction).

## Other platforms

Windows and macOS will reuse `core/` and the event protocol with their own
native overlay and adapters. Notes in `platform/windows/README.md` and
`platform/macos/README.md`. Nothing is implemented for them.
