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
- With several players (or tabs), the island shows the one that is
  playing, not a paused one; if several play, the one started last.
- **Several players:** “All players” shows each one. “One player” shows
  the preferred player while it plays; otherwise the player that started
  playing most recently (or, if none is playing, the preferred one). A
  hidden player disappears silently and comes back when it wins again.
  Player keys are stable across browser restarts (`firefox`, `chromium`,
  `spotify`, `vlc`…).

- The player app's own logo (e.g. Spotify's) when it names its desktop
  entry (`DesktopEntry`); otherwise the usual play icon.

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

Sources, so that every notification GNOME would show reaches the island:

- `org.freedesktop.Notifications.Notify` calls (most apps: Slack,
  Discord, Chrome…), watched by the daemon as a D-Bus **monitor**.
- Everything else GNOME shows, passed on by the extension from GNOME's
  message tray while GNOME's banners are hidden: apps using GNOME's own
  notification route (GNotification — Clocks, Files, Software, Calendar,
  Flatpak apps — often through the notification portal), with the app's
  name and logo, and GNOME Shell's own notifications (“Screenshot
  captured”, network sign-in, drives…). Ids `notifications:gnome-shell:…`,
  same text setting, dropped when this integration is off.

GNOME stays the notification server: notifications still go to GNOME's
list, with their buttons.

- Shown for the time the app asks (2–30 s), 6 s by default; critical ones
  15 s with the highest priority; low-urgency ones at low priority.
- Grouped by app, with the app's logo when it names its desktop entry.
  Markup is removed. Text only if “Show notification text”.
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
| Microphone muted / unmuted (while no app uses it; otherwise the microphone bubble shows it) | Microphone muted / on | 3 s |

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

## Copied — clipboard (implemented, in the extension)

A short “Copied”, “Copied image” or “Copied files” bubble (about 1.5 s)
when something is copied, from GNOME's clipboard. Only the kind of data is
used, never the content. It shows in the closed island only (it never
opens it) and does not interrupt calls. Turn it off in Integrations →
Copied.

Limits: **pasting cannot be shown** — GNOME 51 announces a new clipboard
owner but not when an app reads it. Copies made inside GNOME Shell itself
(e.g. its search field) are not shown, because GNOME keeps the clipboard
of a closed app the same way and that must not look like a new copy.
**File copy progress** from GNOME Files is not available to other apps
(Files keeps it in its own window); Files' “All file operations have been
completed” notification does show. `selawi-island run -- cp -r …` shows a
copy started from a terminal.

## System updates — `pacman` (implemented, pacman hooks)

Source: pacman hooks (installed by the package in
`/usr/share/libalpm/hooks/`), so it works for `pacman` and for `yay` (which
hands installing to pacman), AUR packages included.

| When | Shows |
|---|---|
| Install / update starts | “Installing spotify” or “Updating 23 packages” (a few names below), a running timer |
| It finishes | “Installed spotify” / “Updated 23 packages”, Done |
| Remove | “Removing …”, then “Removed …” |
| It stops halfway (error, Ctrl+C) | Failed, “Did not finish” |

How it stays harmless: the hook always succeeds and gives up after about a
second, so it never stops or slows a transaction. It runs as root only to
read the package list it is given and the local package database (to tell
installs from updates); it then starts a copy of itself **as each
logged-in user** (no root, no environment but the path to that user's
session bus) to send one event, and does nothing if the island is not
running. A small helper, also as the user, waits for pacman to release
its lock (checking once a second while an update runs) to notice a
transaction that stopped without finishing. Only package names are sent.

Limits: no percentage (pacman does not report one to hooks). For AUR
packages, the time yay spends compiling comes before pacman starts and is
not shown; `selawi-island run -- yay -S name` shows the whole thing.
Removing Selawi Island itself is not shown. Turn it off in Integrations →
System updates.

## Microphone and camera in use — `gnome-privacy` (implemented, in the extension)

Sources: recording streams in GNOME's mixer and `Shell.CameraMonitor`, the
same ones behind GNOME's own top-bar privacy icons.

| When | Shows |
|---|---|
| An app uses the microphone | 🟠 Microphone · app name · running time · voice waves; the app's logo when it can be matched |
| The camera is on (with or without the microphone) | 🟢 Camera / Camera and microphone · app name · running time |

One event (`privacy:capture`, call priority) that stays while the
microphone or camera is in use. It names the apps that record; it never
claims you are in a call (a real call bubble needs the browser extension).
During a call shown by the browser extension, this event folds into the
call bubble: the call carries the voice waves and, if the camera is on,
a camera icon and “Camera on”.

Chrome often opens the camera directly instead of through PipeWire; then
GNOME (and this event) cannot see it. Sites that report their camera
through Chrome's call controls (Google Meet does) still show it. To let GNOME see Chrome's camera everywhere,
turn on `chrome://flags/#enable-webrtc-pipewire-camera`.

**Voice waves:** while the microphone event is on screen and another app
is using the microphone, the extension measures the microphone's loudness
(8 kHz mono, 12 readings a second) with GStreamer and moves the bars.
No sound is stored or sent. It stops the moment the microphone is released,
the island hides (fullscreen), or another event is shown instead. Its own
stream is named “Selawi Island” and is ignored when deciding whether the
microphone is in use; it appears next to the real app in tools that list
recording apps (e.g. pavucontrol). Needs `gst-plugins-good`; without it the
bars stay still.

**Mute / Unmute** (expanded island, while the microphone is in use) is
GNOME's own microphone mute, the same switch as the volume menu and the
microphone key. Linux shows the microphone as muted, nobody hears you, and
the bubble says “Microphone muted” (crossed-out icon, no waves). Muting
from GNOME or with the microphone key shows in the bubble too. Unmute turns
the microphone back on and also clears any mute an app's own audio
connection was left with, so it always means “they can hear you again”.
The app itself (Slack, Discord…) does not know and keeps its own mute
button as it was.

**App logo:** the recording app's logo (Slack, Zoom, Discord…) replaces
the mic icon when the island can match it to an installed app: by the
app id or icon name the app gives its audio connection, or else a running
app with exactly that name. Otherwise the mic icon stays. Browsers get
no logo of their own: with the browser extension, the bubble shows the
website that holds the microphone instead (its name and logo, e.g.
“Google Meet”); without it, the plain mic icon and the browser's name. With a logo,
“muted” and “camera on” show as small icons next to the time. The bubble
stays orange: it shows the microphone is in use, not that a call is on.

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
| Calls on any website (Google Meet included), per-tab media (YouTube, YouTube Music, SoundCloud) with ads hidden, browser downloads | Implemented — see [BROWSER_INTEGRATIONS.md](BROWSER_INTEGRATIONS.md) |
