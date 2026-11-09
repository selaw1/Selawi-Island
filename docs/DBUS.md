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
| `Changed(events_json)` | signal | `s` | The list changed; same JSON as `List()` |
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

### Errors

| Name | When |
|---|---|
| `com.selawi.Island.Error.InvalidEvent` | Malformed JSON, failed validation, too large, or a rejected state change (e.g. updating a finished event) |
| `com.selawi.Island.Error.UnknownAction` | No such event, or it has no such action |
| `com.selawi.Island.Error.InvalidConfig` | The configuration was malformed, invalid or could not be saved |

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
