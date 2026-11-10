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
| Sources: Media players / Battery / Bluetooth / Notifications / Network / USB drives / System warnings | Turn each daemon integration on or off. Off removes its events right away. System warnings is off by default |
| Sources: Audio devices | Audio output, microphone and mute changes (watched by the extension) |
| Sources: Microphone and camera in use | Orange while an app uses the microphone, green when the camera is on |
| Sources: Voice waves | Bars that move with your voice while the microphone is in use |
| Sources: Mute button mutes | Only the apps using the microphone (default), or the whole microphone |
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

## Network — `network` (implemented)

Source: NetworkManager (`org.freedesktop.NetworkManager`, system bus).

| When | Shows | How long |
|---|---|---|
| The main connection changes | Name · Wi-Fi connected / Ethernet connected | 4 s |
| No connection left | Offline · No network connection | 6 s |
| The network needs a login page (hotel, café) | Sign in to network · name | Until it is no longer needed |
| A VPN (incl. WireGuard) connects / disconnects | Name · VPN connected / disconnected | 4 s |

Nothing at startup. When a VPN carries all traffic, the island still shows
the real Wi-Fi or Ethernet underneath. Signal-strength changes are ignored.

## USB drives — `storage` (implemented)

Source: UDisks2 (`org.freedesktop.UDisks2`, system bus).

| When | Shows | How long |
|---|---|---|
| A USB or removable drive is plugged in | Vendor Model · USB drive connected | 4 s |
| It is removed | Vendor Model · USB drive removed | 4 s |

Internal disks and drives present at startup are not announced. Other USB
devices (keyboards, phones, hubs) are not reported by UDisks2, so they are
not shown; wireless ones are covered by Bluetooth.

## System warnings — `health` (implemented, off by default)

Sources: `/proc/meminfo` and CPU temperature sensors in `/sys/class/hwmon`
(coretemp, k10temp, zenpower, cpu_thermal), or CPU thermal zones.

| When | Shows | How long |
|---|---|---|
| Available memory below 10 % | Memory low · 7% free · 1.1 GB available | 15 s |
| CPU at 90 °C or more | High temperature · CPU at 93 °C | 15 s |

Each warning is shown once and again only after the value has recovered
(memory above 15 %, CPU below 80 °C). There is no signal for these values,
so this is the only integration that checks on a timer (every 15 s, two
small file reads); that is why it is off by default.

## Microphone and camera in use — `gnome-privacy` (implemented, in the extension)

Sources: recording streams in GNOME's mixer and `Shell.CameraMonitor`, the
same ones behind GNOME's own top-bar privacy icons.

| When | Shows |
|---|---|
| An app uses the microphone | 🟠 Microphone · app name · running time · voice waves |
| The camera is on (with or without the microphone) | 🟢 Camera / Camera and microphone · app name · running time |

One event (`privacy:capture`, call priority) that stays while the
microphone or camera is in use. It names the apps that record; it never
claims you are in a call (a real call bubble needs the browser extension).

**Voice waves:** while the microphone event is on screen and another app
is using the microphone, the extension measures the microphone's loudness
(8 kHz mono, 12 readings a second) with GStreamer and moves the bars.
No sound is stored or sent. It stops the moment the microphone is released,
the island hides (fullscreen), or another event is shown instead. Its own
stream is named “Selawi Island” and is ignored when deciding whether the
microphone is in use; it appears next to the real app in tools that list
recording apps (e.g. pavucontrol). Needs `gst-plugins-good`; without it the
bars stay still.

**Mute / Unmute** (expanded island, while the microphone is in use): a
system-level backup for apps the island cannot control directly. It mutes
the recording apps' microphone streams (or the whole microphone, per the
setting), so nobody hears you, but the app itself still shows you as
unmuted. The island shows “Microphone muted” and stops the waves. Linux
remembers per-app mute, so whatever the island muted is unmuted again
after the call: the whole microphone as soon as no app records, and app
streams the next time that app starts recording.

Limits: the camera can only be shown, not turned off. Camera use is
reported without the app's name (GNOME does not say which app). Apps keep
their own mute buttons; only the browser extension (Phase 6) can press
those for web meetings.

## Not shown on purpose

| What | Why |
|---|---|
| Volume, brightness, keyboard backlight | GNOME already shows its own on-screen display |
| Low disk space | GNOME already sends a notification, which the island shows |
| GPU usage | Readings differ by vendor and are not reliable |

## Not yet implemented

| Integration | Plan |
|---|---|
| Google Meet calls, per-tab media (YouTube, YouTube Music, SoundCloud) with ads hidden, browser downloads | Implemented — see [BROWSER_INTEGRATIONS.md](BROWSER_INTEGRATIONS.md) |
| Calls on any website (Slack, Teams, Zoom, Discord…) | Implemented: site name, timer and voice waves, no buttons |
| Calls (Slack, Zoom, Teams, Discord…) | Phase 7 |
