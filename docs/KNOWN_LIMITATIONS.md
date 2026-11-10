# Known limitations

## Current (after Phase 3)

- **Automatic detection covers media, battery, Bluetooth, notifications,
  audio devices, network/VPN, USB drives and (opt-in) memory and CPU
  temperature.** Calls and per-tab browser media come later.
- **YouTube and SoundCloud support depends on their pages**
  (YouTube's ad marker and next/previous buttons,
  SoundCloud's player controls). Tested with stand-in pages built like
  them, not yet on the real sites. If a site changes, that part simply
  stops showing; media titles come from the standard media keys data and
  are the most robust part.
- **Calls on other sites have no buttons** (by choice) and show the site's
  name, not the meeting's. The extension needs access to all https sites
  for this, which Chrome warns about.
- **Per-tab media covers YouTube, YouTube Music and SoundCloud.** Other
  sites still show through the browser's single player.
- **Firefox needs a signed add-on** to keep it after a restart; until then
  it is a temporary add-on (docs/BROWSER_INTEGRATIONS.md).
- **Chrome shows a “developer mode” extension** because it is loaded
  unpacked.
- **Microphone/camera in use is not a call.** The island shows which app
  records, not whether you are in a meeting; camera use has no app name.
- **The microphone bubble's Mute is GNOME's microphone mute**: nobody
  hears you and GNOME shows it, but the app keeps its own mute button as
  it was.
- **Voice waves add a second recording stream** (“Selawi Island”) while
  they are on screen. They were tested with a test tone, not yet with a
  real microphone.
- **Only USB drives are shown**, not other USB devices (no desktop signal
  for those).
- **Warning limits are fixed** (memory 10 %, CPU 90 °C) for now.
  Per-integration limits are in `docs/INTEGRATIONS.md`.
- **No album artwork**, by choice.
- **Logos** come only from what a source names: an app without a desktop
  entry, or a site without a favicon, keeps the generic icon. Apps run as
  PWAs report the browser, so they show the browser's icon.
- **Notification buttons are not shown** in the island (only GNOME, as the
  notification server, can deliver them); they stay in GNOME's list.
- **Notifications rely on D-Bus monitoring**, which the session bus allows
  for the same user. If a bus ever refuses it, the integration reports
  `unavailable` and GNOME's banners stay visible.
- **Actions reach only D-Bus sources.** `ActionInvoked` is a signal; the CLI
  exits after sending, so CLI events offer no action buttons (only Dismiss).
- **Any program of the same user can send events.** The session bus is
  per-user, so other users cannot, but there is no per-app permission yet
  (plugin permissions come in Phase 9).
- **The CLI is stateless.** `progress update/finish` read the current event
  from the daemon first; if it already expired, they fail with a message.
- **Demo uses the JavaScript engine copy**, not the daemon.
- **One position for all monitors.** The default position and the dragged
  position apply to the one monitor the island is on.
- **Dragging is mouse/touchpad only**; there is no keyboard way to move it.
- **Code updates need a re-login** on Wayland for the extension (the daemon
  is restarted by the install script).
- **Monitor choice is by index** (-1 = primary), not by name.
- **Only GNOME 51** is declared in `metadata.json`.
- **A 1-second timer runs in the extension while a call timer or playing
  media is visible.** Everything else is event-driven.
- **Quick preferences only.** The full settings app is Phase 10.
- **Integrations were tested against fake services**, not yet against
  your real players and devices (see `docs/TESTING.md`).
- **Calls show the site's name, not the meeting's name** (e.g. “Google
  Meet”): there is no per-site code.
- **Mute / Leave on web calls** only for sites that use Chrome's Media
  Session call controls; other sites get the plain call bubble. Firefox has
  no such controls.
- **Desktop app logos on the mic bubble** need the app to name itself on
  its audio connection (or a running app with exactly that name);
  otherwise the mic icon stays.
- **Updates are not automatic:** Selawi Island is not on the AUR (the
  project is private), so `yay -Syu` does not update it; run
  `selawi-island-update`.
- **The package registers the browser helper for Google Chrome, Chromium
  and Firefox only**, not Brave (Brave works with the development
  install, `./scripts/install-local.sh`).
- **The package build (makepkg) is not run in the automated checks**
  (no Arch system there); its steps and the installed file layout are.
- **System updates show no percentage** (pacman does not report one to
  hooks), and yay's compile time for AUR packages is not shown (use
  `selawi-island run -- yay -S name`).
- **`selawi-island run` shows the program and subcommand as the title**;
  a plain-word second argument is treated as a subcommand and shown (use
  `--title` for commands whose arguments are private).
- **Pasting is not shown** (GNOME does not announce it), and **file copy
  progress from GNOME Files is not available** to other apps; only its
  “completed” notification shows.
