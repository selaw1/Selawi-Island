# Integrations

Built-in integrations run inside the daemon. Each one listens to an
existing Linux service over D-Bus and turns its signals into events. None
of them poll, run shell commands, or need extra permissions.

See what each is doing:

```bash
selawi-island integrations
```

| Status | Meaning |
|---|---|
| `active` | Working (the detail says what it is watching) |
| `unavailable` | The source is not on this system (no battery, Bluetooth service not running…). Checked again every 30 s |
| `failed` | Stopped by an error. Retried every 30 s |
| `disabled` | Turned off in the configuration |

## Turning integrations off

Create `~/.config/selawi-island/daemon.json`:

```json
{ "integrations": { "bluez": false } }
```

Any integration not listed stays on. Restart the daemon after editing:

```bash
systemctl --user restart selawi-islandd
```

## Media players — `mpris` (implemented)

Source: MPRIS (`org.mpris.MediaPlayer2.*`, session bus). Works with any
player that supports it: Spotify, Firefox, Chromium/Chrome, VLC, mpv,
Rhythmbox, and others.

- One event per player: title, artist, album, position, length.
- **Playing** stays as long as it plays. **Paused** disappears 5 minutes
  after the last change. **Stopped** or a closed player ends it.
- **Previous / Pause–Play / Next** buttons, only when the player says it
  supports them. They control the player directly.
- A dismissed player comes back on a new track or when playback resumes.

Limits: no artwork yet; several players at once are each shown (no
"preferred player" choice yet); position is read when something changes
and advanced by the island while playing.

## Battery — `upower` (implemented)

Source: UPower display device (`org.freedesktop.UPower`, system bus).

| When | Shows | How long |
|---|---|---|
| Charger plugged in / unplugged | Charging / Plugged in / On battery, with % | 4 s |
| Reaches full while plugged in | Fully charged | 6 s |
| UPower's low-battery warning | Battery low, % remaining | 15 s |
| UPower's critical warning | Battery critical (highest priority) | Until plugged in |

Nothing is shown at startup except an existing warning. Desktops without
a battery report `unavailable`.

## Bluetooth — `bluez` (implemented)

Source: BlueZ (`org.bluez`, system bus).

| When | Shows | How long |
|---|---|---|
| A device connects | Name, type icon, battery **only if the device reports it** | 5 s |
| A device disconnects | Name, Disconnected | 4 s |
| A connected device's battery reaches 15 % | “<name> battery low” | 10 s (once, until it is above 20 % again) |

Devices already connected when the daemon starts are not announced. Many
earbuds report their battery a moment after connecting; it is added to
the “Connected” event if it arrives within 10 seconds. Individual earbud
and case levels are not exposed by BlueZ, so they are not shown.

## Not yet implemented

| Integration | Plan |
|---|---|
| Desktop notifications | Next: mirror notifications without replacing GNOME's |
| Audio output and volume | After notifications |
| Network, VPN, USB, storage, CPU/RAM | Phase 5 |
| Calls (Slack, Meet, Zoom…) | Phases 6–7, through browser extensions and app integrations |
