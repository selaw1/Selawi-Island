# Browser integrations

A small extension for Chrome (and Chromium, Brave) and Firefox shows what
only the browser can see: **calls on any website**, **Google Meet**,
**media in each tab** (YouTube, YouTube Music, SoundCloud) and
**downloads**.

```text
 any site ─ rtc-main.js → call.js ┐
 Meet tab ─ meet.js ─┐
 media tabs ─ media.js ┤
                     ├─ background.js ─ native messaging ─ selawi-island-browser-host ─ D-Bus ─ selawi-islandd ─ island
 downloads API ──────┘                                     (checks every event)
```

## Install

`./scripts/install-local.sh` installs the helper (`selawi-island-browser-host`),
registers it with Chrome, Chromium, Brave and Firefox, and builds the
extension into `~/.local/share/selawi-island/browser/` (needs `pnpm`).

### Chrome / Chromium / Brave

1. Open `chrome://extensions`.
2. Turn on **Developer mode** (top right).
3. **Load unpacked** → choose `~/.local/share/selawi-island/browser/chrome`.

The extension id is always `bjooidhahonkdcdpibfadoapilihpgig`; the helper
only accepts that extension. It also works in installed web apps (PWAs) of
the same Chrome profile. After an update, press the reload button on the
extension's card.

### Firefox

Normal Firefox only keeps add-ons that Mozilla has signed. Until this one is
signed:

1. Open `about:debugging#/runtime/this-firefox`.
2. **Load Temporary Add-on…** → choose
   `~/.local/share/selawi-island/browser/firefox/manifest.json`.
3. In `about:addons` → Selawi Island → Permissions, allow access to
   websites (needed for calls on any site).

A temporary add-on is removed when Firefox restarts. Signing it for
personal use is free (an unlisted add-on on addons.mozilla.org); that is a
one-time manual step for you, not something the project does.

## What it does

| Site / feature | Shows | Buttons |
|---|---|---|
| A call on any website (Slack, Teams, Zoom, Discord, WhatsApp, Telegram, Webex, Jitsi…) | Green call with the site's name and a timer; voice waves when you talk; “Muted” and “Camera on” if the site reports them | **Mute / Unmute** and **Leave** only if the site offers them through Chrome's call controls (see below); otherwise none |
| Google Meet (`meet.google.com`) | Green call: meeting name, timer, “Muted” and “Camera on” (camera icon) when Meet says so | **Mute / Unmute** and **Leave** (Meet's own call controls; Leave falls back to Meet's leave button) |
| YouTube, YouTube Music, SoundCloud | Each tab on its own (several can play at once): title, artist, running time | **Previous** / **Play–Pause** / **Next**, only those the page offers |
| Downloads | File name, “512 MB of 1.2 GB”, progress bar; then Downloaded / Cancelled / Download failed | **Cancel** |

**Logos:** calls, Meet and media tabs show the site's own icon (its
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

During a Meet call, the separate “Microphone in use” bubble is folded
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
- On Meet it also reads whether its leave button exists and the page
  title. No chat, no names
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
in a call; when it closes or the microphone track stops, the call ends.
This works the same on every site, with no per-site code. The site name
comes from the tab's address (known sites get their proper names). Meet
keeps its own, richer support.

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

## How Meet is detected

Microphone, camera, Mute / Unmute and Leave come from Meet's call
controls above (checked on real Meet: it reports all of them). The Meet
content script only adds what those do not say: Meet's leave button
(Material icon `call_end`) means you are in a call, and the page title is
the meeting name. Leave presses that button if Meet ever stops offering
“hang up”. If Meet changes its page, the call stops showing (nothing
wrong is shown) until `browser-extensions/src/meet.ts` is updated.

## Build and test

```bash
./scripts/build-browser.sh
```

TypeScript (strict, no `any`) in `browser-extensions/src/`; the build goes
to `browser-extensions/build/{chrome,firefox}`. Tests: `tests/browser/`
(event building, validated by the island's own validator),
`tests/integration/browser-host.sh` (the helper against the daemon), and
`tests/integration/browser-e2e.sh` (the built extension in real Chromium
with stand-in Meet, YouTube and SoundCloud pages).
