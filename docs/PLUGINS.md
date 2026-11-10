# Plugins: show your own app's events on the island

Any program can put its own bubbles on the island — a timer, a build, a
call in your own app — with buttons that come back to it. It talks to the
Selawi Island service over D-Bus (`com.selawi.Island.Plugins1`), from any
language. Helpers: Python (`sdk/python/selawi_island.py`) and Rust
(`sdk/rust`, crate `selawi-island-plugin`).

**Web apps don't need this.** A site in Chrome already gets a call bubble
with Mute / Leave through the browser's standard call controls, and its
notifications through the browser (see
[BROWSER_INTEGRATIONS.md](BROWSER_INTEGRATIONS.md#call-controls-a-site-reports-media-session)).

## How it works

1. Your app **registers** with a small manifest (who it is, what it shows).
2. The first time, the island **asks you**: “Pomodoro wants to show things
   on the island” with **Allow** / **Don't allow**. Until you allow it,
   nothing it sends is shown. You can change your answer any time in
   Settings → Integrations → **Apps**.
3. It **shows events**: the same event format as everything else
   ([EVENT_SYSTEM.md](EVENT_SYSTEM.md)), with its own ids.
4. **Buttons** pressed on its events come back to it, and only to it.
5. When it **exits**, its events disappear.

## The manifest

```json
{
  "manifest_version": 1,
  "id": "pomodoro",
  "name": "Pomodoro",
  "description": "Focus timer",
  "icon": "app:org.example.Pomodoro",
  "categories": ["custom", "notification"]
}
```

| Field | Rule |
|---|---|
| `manifest_version` | `1` |
| `id` | Stable and unique: 1–40 lowercase letters, digits, `.` or `-` (`com.example.selawigram`) |
| `name` | Shown as the app name on every event and in Settings (1–40 characters) |
| `description` | Optional, up to 200 characters; shown when asking and in Settings |
| `icon` | Optional: `app:<desktop id>` (an installed app's icon) or a PNG data URL, like `source.icon` |
| `categories` | The kinds of events it may show: `call`, `media`, `transfer`, `device`, `notification`, `system`, `reminder`, `metric`, `custom` |

## What the island enforces

- **Its own events only.** Its event ids are stored as
  `plugin:<id>:<its id>`; it can only add, update or remove those. Its ids
  may not contain `:` (up to 64 characters).
- **No pretending.** `source` is always replaced: the app name and icon
  are the manifest's, the integration is `plugin`. It cannot show a
  category it did not declare, and cannot use `critical` priority (`call`
  priority only for call events). `Events1.Submit` refuses ids starting
  with `plugin:` or `plugins:`.
- **Limits.** At most 8 events at a time; about 10 submits a second
  (bursts of 30).
- **Private buttons.** A press is sent only to the app's own connection
  (`ActionInvoked`), not broadcast.
- **One app per id.** While an app is connected, another connection
  cannot register the same id.

## Python

```python
from gi.repository import GLib
import sys; sys.path.insert(0, "/path/to/Selawi-Island/sdk/python")   # or installed with the package
from selawi_island import Island

island = Island({"id": "pomodoro", "name": "Pomodoro", "categories": ["custom"]})
island.on_action(lambda event_id, action_id: print("pressed", action_id, "on", event_id))
island.show("timer", category="custom", persistent=True,
            payload={"type": "progress", "title": "Focus", "detail": "25:00 left", "percent": 0},
            actions=[{"id": "pause", "label": "Pause"}])
GLib.MainLoop().run()
```

`show` returns False while the app is not allowed; events shown before
the user allows it appear once they do. `end(id)` shows it as done,
`remove(id)` removes it at once. If the island restarts, the helper
registers again and shows its events again. Needs PyGObject
(`python-gobject`, installed with GNOME). The package installs the module,
so `import selawi_island` works without the path line.

## Rust

```toml
[dependencies]
selawi-island-plugin = { path = "/path/to/Selawi-Island/sdk/rust" }
```

```rust
let island = selawi_island_plugin::Island::register(MANIFEST_JSON).await?;
island.show(r#"{"id":"focus","category":"custom","state":"started","persistent":true,
    "payload":{"type":"progress","title":"Focus"}}"#).await?;
let mut pressed = island.actions().await?;          // (event id, action id)
let mut answers = island.allowed_changes().await?;   // true / false when the user answers
```

A full example: `cargo run -p selawi-island-plugin --example countdown`.

## Any language (D-Bus)

`com.selawi.Island.Plugins1` at `/com/selawi/Island` on the session bus
(details in [DBUS.md](DBUS.md#plugins1)):

```bash
gdbus call --session --dest com.selawi.Island --object-path /com/selawi/Island \
  --method com.selawi.Island.Plugins1.Register \
  '{"manifest_version":1,"id":"demo","name":"Demo","categories":["custom"]}'
```

Registration belongs to the connection that made it, so a plugin must keep
one connection open (a one-off `gdbus call` registers and disconnects at
once). Use a helper or your language's D-Bus library.
