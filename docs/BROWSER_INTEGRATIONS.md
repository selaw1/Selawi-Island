# Browser integrations

A small extension for Chrome (and Chromium, Brave) and Firefox shows what
only the browser can see: **calls on any website** (Google Meet included),
**media in each tab** (YouTube, YouTube Music, SoundCloud) and
**downloads**.

```text
 any site ─ rtc-main.js → call.js ─────────────┐
 any site ─ session-main.js, mic-main.js → session.js ┤
 media tabs ─ media.js ┤
                     ├─ background.js ─ native messaging ─ selawi-island-browser-host ─ D-Bus ─ selawi-islandd ─ island
 downloads API ──────┘                                     (checks every event)
```

## Install

The Arch package (`selawi-island-update`, see [INSTALL.md](INSTALL.md))
installs the helper (`selawi-island-browser-host`), registers it with
Google Chrome, Chromium and Firefox, and puts the extension in
`/usr/share/selawi-island/browser/`. (The development install,
`./scripts/install-local.sh`, uses `~/.local/share/selawi-island/browser/`
instead and also registers Brave.)

### Chrome / Chromium / Brave

1. Open `chrome://extensions`.
2. Turn on **Developer mode** (top right).
3. **Load unpacked** → choose `/usr/share/selawi-island/browser/chrome`.

The extension id is always `bjooidhahonkdcdpibfadoapilihpgig`; the helper
only accepts that extension. It also works in installed web apps (PWAs) of
the same Chrome profile. After an update, press the reload button on the
extension's card.

### Firefox

Normal Firefox only keeps add-ons that Mozilla has signed. Until this one is
signed:

1. Open `about:debugging#/runtime/this-firefox`.
2. **Load Temporary Add-on…** → choose
   `/usr/share/selawi-island/browser/firefox/manifest.json`.
3. In `about:addons` → Selawi Island → Permissions, allow access to
   websites (needed for calls on any site).

A temporary add-on is removed when Firefox restarts.

### Firefox: a signed copy (free, private)

Mozilla signs add-ons for free as **unlisted**: not shown in their store,
only for people you give the file to. Once signed, it installs
permanently.

1. `./scripts/firefox-xpi.sh` → `dist/selawi-island-firefox-<version>.xpi`.
2. With a free Mozilla account, upload it at addons.mozilla.org → Submit
   a New Add-on → **On your own**. Mozilla checks it automatically and
   gives back a signed `.xpi`.
3. Open the signed file in Firefox (or drag it onto `about:addons`) and
   allow website access in its Permissions.

Each new version needs a new upload; the script numbers versions by
commit, so each one is higher than the last. Signing is done by you, by
hand; the project never uploads anything.

## What it does

| Site / feature | Shows | Buttons |
|---|---|---|
| A call on any website (Google Meet, Slack, Teams, Zoom, Discord, WhatsApp, Telegram, Webex, Jitsi…) | Green call with the site's name and a timer; voice waves when you talk; “Muted” and “Camera on” if the site reports them | **Mute / Unmute** and **Leave** only if the site offers them through Chrome's call controls (see below); otherwise none |
| YouTube, YouTube Music, SoundCloud | Each tab on its own (several can play at once): title, artist, running time | **Previous** / **Play–Pause** / **Next**, only those the page offers |
| Downloads | File name, “512 MB of 1.2 GB”, progress bar; then Downloaded / Cancelled / Download failed | **Cancel** |

**Logos:** calls and media tabs show the site's own icon (its
favicon) instead of a generic one. The extension redraws the icon as a
small 64×64 PNG before sending it, so the island never loads anything
from a website. Microphone-off and camera-on then show as small icons
next to the time. Downloads keep the download icon.

**Ads (YouTube, YouTube Music):** while the player is marked as showing
an ad, the last real title stays and the ad's own timer is hidden.

**No doubles:** a browser also reports one media player to Linux (MPRIS).
While the extension shows that browser's tabs, the island hides the
browser's own player; other browsers and apps are not affected. A paused
tab disappears 5 minutes after its last change, as for other players.
Media on other sites (Netflix, Twitch…) still shows through the browser's
own player, one at a time.

During a call, the separate “Microphone in use” bubble is folded
into the call bubble, which then shows the voice waves.

## Privacy

- Permissions: `nativeMessaging`, `downloads`, and access to https
  websites (Chrome calls this “read and change your data on all
  websites”). It is needed to notice calls on any site; no history, no
  cookies, no page content is read.
- On every https site it only watches whether a live call connection
  (WebRTC) is sending your microphone; that yes/no and the site's name
  are all that is sent, plus the site's icon (see Logos above). Nothing
  about the call's people, chat or audio.
  A page could pretend to be in a call; the worst it can do is show a
  call bubble with its own name.
- On media sites it reads only what the site publishes for the media
  keys (title, artist, album), the player's play state and time, and
  whether YouTube's player is showing an ad. It presses only the page's
  own play, next and previous controls.
- On every https site it also notes whether the page holds the
  microphone (yes/no) and the Media Session call controls the
  page reports (microphone/camera on or off, mute/hang-up available).
 No chat, no names
  of participants, no audio or video.
- For downloads only the file name is sent, never the folder or the
  address it came from.
- The helper accepts only events whose id starts with `browser:` and that
  are marked `browser`, at most 20 a second, 64 KiB each; it never runs
  commands. When the browser closes, everything it showed is ended.

## How calls on any site are detected

Every web call uses the browser's WebRTC connection. A small script runs
in each https page before the page's own code and wraps it: when a
connection is connected and sending a live microphone track, the tab is
in a call. Some sites drop the microphone from the connection while you
are muted, so the call goes on as long as the page still holds the
microphone; when the connection closes and the microphone is released,
the call ends. This works the same on every site, with no per-site code,
Google Meet included. The site name comes from the tab's address (known
sites get their proper names).

## Which site uses the microphone

Linux only sees that the browser records. A small script in each https
page notes whether the page holds a live microphone track (from
`getUserMedia`; no audio is read) and the extension reports the site
(`browser:mic:<tab>`, with the site's name and logo). The island never
shows this on its own: it puts that site's name and logo on its orange
microphone bubble in place of “Google Chrome”.

## Call controls a site reports (Media Session)

Chrome lets call sites report “microphone on/off” and “camera on/off” and
offer “toggle microphone” and “hang up” controls (the Media Session API,
made for Chrome's floating call window). A small script in each https
page notes what the page reports there. When a site uses it:

- the call shows “Muted” / “Camera on” as the site reports them;
- **Mute / Unmute** and **Leave** appear and call the site's own handlers.

Sites that do not use it keep the plain call bubble. Which sites use it is
up to them (Google Meet does); Firefox does not have these controls. A page
could report anything here; the worst it can do is change its own bubble.

## Build and test

```bash
./scripts/build-browser.sh
```

TypeScript (strict, no `any`) in `browser-extensions/src/`; the build goes
to `browser-extensions/build/{chrome,firefox}`. Tests: `tests/browser/`
(event building, validated by the island's own validator),
`tests/integration/browser-host.sh` (the helper against the daemon), and
`tests/integration/browser-e2e.sh` (the built extension in real Chromium
with stand-in Meet, Slack, Teams, YouTube and SoundCloud pages).
