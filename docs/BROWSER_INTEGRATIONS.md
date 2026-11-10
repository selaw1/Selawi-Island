# Browser integrations

A small extension for Chrome (and Chromium, Brave) and Firefox shows what
only the browser can see: **Google Meet calls**, **media in each tab**
(YouTube, YouTube Music, SoundCloud) and **downloads**.

```text
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
3. In `about:addons` → Selawi Island → Permissions, allow **meet.google.com**,
   **youtube.com**, **music.youtube.com** and **soundcloud.com**.

A temporary add-on is removed when Firefox restarts. Signing it for
personal use is free (an unlisted add-on on addons.mozilla.org); that is a
one-time manual step for you, not something the project does.

## What it does

| Site / feature | Shows | Buttons |
|---|---|---|
| Google Meet (`meet.google.com`) | Green call: meeting name, timer, “Muted” when Meet says so | **Mute / Unmute** (presses Meet's own microphone button), **Leave** (presses Meet's leave button) |
| YouTube, YouTube Music, SoundCloud | Each tab on its own (several can play at once): title, artist, running time | **Previous** / **Play–Pause** / **Next**, only those the page offers |
| Downloads | File name, “512 MB of 1.2 GB”, progress bar; then Downloaded / Cancelled / Download failed | **Cancel** |

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

- Permissions: `nativeMessaging`, `downloads`, and access to
  `meet.google.com`, `youtube.com`, `music.youtube.com` and
  `soundcloud.com` only. No history, no other sites, no cookies.
- On media sites it reads only what the site publishes for the media
  keys (title, artist, album), the player's play state and time, and
  whether YouTube's player is showing an ad. It presses only the page's
  own play, next and previous controls.
- On Meet it reads only its call controls (is there a leave button, the
  microphone button's muted flag) and the page title. No chat, no names
  of participants, no audio or video.
- For downloads only the file name is sent, never the folder or the
  address it came from.
- The helper accepts only events whose id starts with `browser:` and that
  are marked `browser`, at most 20 a second, 64 KiB each; it never runs
  commands. When the browser closes, everything it showed is ended.

## How Meet is detected

Meet's page has no public API. The content script looks for Meet's own
buttons: the leave button (Material icon `call_end`) means you are in a
call, and the microphone toggle's `data-is-muted` attribute gives the mute
state. If Meet changes its page, detection stops (nothing wrong is shown)
until `browser-extensions/src/meet.ts` is updated.

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
