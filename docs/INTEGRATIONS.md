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

## Settings

Right-click the island → **Integrations** page. Changes apply immediately.

| Setting | What it does |
|---|---|
| Sources: Media players / Battery / Bluetooth / Notifications | Turn each daemon integration on or off. Off removes its events right away |
| Sources: Audio devices | Audio output, microphone and mute changes (watched by the extension) |
| Media: Players to show | **All players**, or **One player** |
| Media: Preferred player | In one-player mode: this player wins whenever it is playing |
| Notifications: Show notification text | Off shows only the title and app |
| Notifications: Hide GNOME's notification banners | Each notification appears once (only while the island shows notifications) |

The daemon settings are stored in `~/.config/selawi-island/daemon.json`
(written by the settings window; editing by hand also works, followed by
`systemctl --user restart selawi-islandd`):

```json
{
  "integrations": { "bluez": false },
  "media": { "mode": "one", "preferred_player": "spotify" },
  "notifications": { "show_body": true }
}
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
- **Several players:** “All players” shows each one. “One player” shows
  the preferred player while it plays; otherwise the player that started
  playing most recently (or, if none is playing, the preferred one). A
  hidden player disappears silently and comes back when it wins again.
  Player keys are stable across browser restarts (`firefox`, `chromium`,
  `spotify`, `vlc`…).

Limits: position is read when something changes and advanced by the
island while playing. No artwork (by choice: it looks cleaner without).

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

## Notifications — `notifications` (implemented)

Source: the daemon watches `org.freedesktop.Notifications.Notify` calls on
the session bus as a D-Bus **monitor**. GNOME stays the notification
server: notifications still go to GNOME's list, with their buttons.

- Shown for the time the app asks (2–30 s), 6 s by default; critical ones
  15 s with the highest priority; low-urgency ones at low priority.
- Grouped by app. Markup is removed. Text only if “Show notification text”.
- A media player's own “now playing” notification is skipped (the media
  event already shows it).
- GNOME's banner is hidden while this integration is active and “Hide
  GNOME's notification banners” is on. If the daemon stops, GNOME's
  banners come back automatically.

Limits: notification buttons (Reply, Mark as read…) are not shown in the
island, because only the notification server can deliver them to the app.
Replaced or withdrawn notifications are not tracked; each one simply
times out.

## Audio devices — `gnome-audio` (implemented, in the extension)

Source: GNOME Shell's own mixer (the one behind the volume menu), so this
part lives in the extension and sends its events to the daemon.

| When | Shows | How long |
|---|---|---|
| Default output device changes | Device name · Audio output | 4 s |
| Default microphone changes | Device name · Microphone | 4 s |
| Microphone muted / unmuted | Microphone muted / on | 3 s |

Volume changes are not shown: GNOME already shows its own volume display.
Nothing is shown for the devices in use at startup.

## Not yet implemented

| Integration | Plan |
|---|---|
| Network, VPN, USB, storage, CPU/RAM, brightness | Phase 5 |
| Calls (Slack, Meet, Zoom…) | Phases 6–7, through browser extensions and app integrations |
