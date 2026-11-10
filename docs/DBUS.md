# D-Bus interface

The daemon `selawi-islandd` owns **`com.selawi.Island`** on the **session
bus** and serves object **`/com/selawi/Island`**. Interfaces are versioned:
a breaking change gets a new name (`Events2`), never an edit of `Events1`.

The session bus is private to your user, so only your own programs can
reach the daemon. It is started at login by systemd, or on first use
through D-Bus activation.

## com.selawi.Island.Core1

| Member | Kind | Type | Meaning |
|---|---|---|---|
| `Version` | property (read) | `s` | Daemon version, e.g. `0.1.0` |
| `ProtocolVersion` | property (read) | `u` | Event protocol version (`1`) |

## com.selawi.Island.Events1

| Member | Kind | Signature | Meaning |
|---|---|---|---|
| `Submit(event_json)` | method | `s → ()` | Add or update an event. `event_json` is one protocol v1 **event input** ([EVENT_SYSTEM.md](EVENT_SYSTEM.md)), at most 64 KiB |
| `List()` | method | `→ s` | All current **stored events** as a JSON array, most important first |
| `Dismiss(id)` | method | `s → b` | Remove an event as if the user dismissed it. Returns whether it existed |
| `InvokeAction(id, action_id)` | method | `ss → ()` | The user pressed a button. Emits `ActionInvoked` only if the event declared that action |
| `Changed(events_json)` | signal | `s` | The list changed; same JSON as `List()`. Sent at once after a quiet moment, then at most every 50 ms while changes keep coming, always with the latest list (so a flood of submits causes a few signals, not thousands) |
| `ActionInvoked(id, action_id)` | signal | `ss` | Sent to the event's source so it can react (pause music, leave a call…) |

## com.selawi.Island.Integrations1

| Member | Kind | Signature | Meaning |
|---|---|---|---|
| `List()` | method | `→ s` | JSON array of `{"id", "name", "status", "detail"}`; status is `starting`, `active`, `unavailable`, `disabled` or `failed`. See [INTEGRATIONS.md](INTEGRATIONS.md) |
| `MediaPlayers()` | method | `→ s` | Running media players, JSON array of `{"id", "name"}`; `id` is the key for `media.preferred_player` |
| `Changed(integrations_json)` | signal | `s` | An integration's status changed; same JSON as `List()` |

## com.selawi.Island.Configuration1

| Member | Kind | Signature | Meaning |
|---|---|---|---|
| `Get()` | method | `→ s` | The daemon configuration as JSON (shape in [INTEGRATIONS.md](INTEGRATIONS.md#settings)) |
| `Set(config_json)` | method | `s → ()` | Replace it. Checked (unknown keys or integrations are rejected), saved to `~/.config/selawi-island/daemon.json` (0600) and applied immediately |
| `Changed(config_json)` | signal | `s` | The configuration changed |

Events created by built-in integrations have ids starting with the
integration id (`mpris:`, `upower:`, `bluez:`). Their action buttons are
handled inside the daemon; `ActionInvoked` is still emitted for them.

`pacman` is an integration id too, but its events come from outside (the
pacman hooks, through `Submit`). `Submit` quietly drops any event whose id
starts with a known integration id that is turned off (`"pacman:…"` while
`integrations.pacman` is false); the call still succeeds.

## com.selawi.Island.Plugins1

Apps showing their own events (guide: [PLUGINS.md](PLUGINS.md)). A
registration belongs to the D-Bus connection that made it.

| Member | Kind | Signature | Meaning |
|---|---|---|---|
| `Register(manifest_json)` | method | `s → (sb)` | Registers the calling connection as the plugin described by the manifest. Returns its id and whether the user has allowed it. The first time, the island asks the user (a question event `plugins:request:<id>` with Allow / Don't allow). Another connection cannot register an id that is in use |
| `Submit(event_json)` | method | `s → ()` | Adds or updates one of the plugin's events (protocol v1 event input; `source` may be left out). Stored as `plugin:<id>:<its id>` with `source` = the manifest's name and icon, integration `plugin`. Only declared categories, no `critical` priority, `call` priority only for call events, at most 8 events, about 10 per second (bursts of 30) |
| `Remove(id)` | method | `s → b` | Removes one of the plugin's events (its own id) at once |
| `List()` | method | `→ s` | Known plugins as JSON: `id`, `name`, `description`, `icon`, `categories`, `allowed`, `decided`, `running` (for Settings) |
| `ActionInvoked(id, action_id)` | signal | `ss` | A button on one of the plugin's events was pressed. Sent **only to that plugin's connection**, with its own id; plugin events' buttons are not broadcast on `Events1.ActionInvoked` |
| `Changed(plugins_json)` | signal | `s` | The list or an answer changed; same JSON as `List()` |

The user's answers are the configuration's `plugins` map (`{"pomodoro":
true}`); a plugin not in it has not been answered and shows nothing.
Turning one off removes its events. Its events also go when its
connection closes.

### Errors

| Name | When |
|---|---|
| `com.selawi.Island.Error.InvalidEvent` | Malformed JSON, failed validation, too large, a rejected state change (e.g. updating a finished event), an id reserved for plugins (`plugin:`, `plugins:`) on `Events1.Submit`, or (`Plugins1`) a category not in the manifest or more than 8 events |
| `com.selawi.Island.Error.UnknownAction` | No such event, or it has no such action |
| `com.selawi.Island.Error.InvalidConfig` | The configuration was malformed, invalid or could not be saved |
| `com.selawi.Island.Error.InvalidPlugin` | `Register`: malformed manifest, its id is in use by another connection, or this connection registered another plugin |
| `com.selawi.Island.Error.NotRegistered` | `Plugins1`: the connection has not registered |
| `com.selawi.Island.Error.NotAllowed` | `Plugins1.Submit`: the user has not allowed the plugin |
| `com.selawi.Island.Error.RateLimited` | `Plugins1.Submit`: too many events too fast |

## Try it with gdbus

```bash
gdbus introspect --session --dest com.selawi.Island --object-path /com/selawi/Island
```

```bash
gdbus call --session --dest com.selawi.Island --object-path /com/selawi/Island \
  --method com.selawi.Island.Events1.List
```

```bash
gdbus monitor --session --dest com.selawi.Island
```

## Behaviour

- One timer is armed for the next expiry; the daemon never polls.
- Active **persistent** events are saved to
  `$XDG_STATE_HOME/selawi-island/events.json` (default
  `~/.local/state/selawi-island/events.json`, mode 0600) and restored after
  a restart. A restored event disappears after 60 s unless its source
  updates it, so nothing stays stuck after a reboot. `--no-persist` turns
  saving off.
- The daemon logs only counts and errors, never event contents.
