# Installing Selawi Island

Selawi Island installs as a normal Arch package, built on your own
computer from this repository. Nothing is downloaded from a server other
than the usual Rust and Node build tools, and there is no hosting.

## What you need

- Arch Linux (or an Arch-based distribution) with **GNOME 51**
- `sudo pacman -S --needed base-devel git`

## First install

```bash
git clone https://github.com/selaw1/Selawi-Island.git
cd Selawi-Island
./scripts/update.sh
```

This builds the package with `makepkg` (installing Rust, Node.js and pnpm
for the build if needed), installs it with pacman (it asks for your
password), and turns the daemon and the GNOME extension on.

Then **log out and back in**. The island appears at the top of the screen.
Settings: right-click the island, or open **Selawi Island** from the app
grid. If something does not work, its **Status** page says what is
running; **Copy report** copies versions and states (never your events) to
paste when asking for help.

## Update

```bash
selawi-island-update
```

It pulls the latest code, rebuilds and reinstalls. The first build takes a few minutes; later
ones reuse the downloads and compiled files (kept in
`~/.cache/selawi-island`) and only rebuild what changed. Log out and back in
afterwards, and reload the browser extension (below).

`yay -Syu` does not update Selawi Island: it is not on the AUR. You can also build by hand in `packaging/arch` with
`makepkg -si` or `yay -Bi .`.

## Browser extension (optional)

Calls on websites (Google Meet, Slack, Teams…), media in each tab and
downloads need the browser extension. The package puts its files in
`/usr/share/selawi-island/browser/` and registers the helper with Google
Chrome, Chromium and Firefox.

**Chrome / Chromium** (once):

1. Open `chrome://extensions` and turn on **Developer mode**.
2. **Load unpacked** → `/usr/share/selawi-island/browser/chrome`.

After each update press **reload** on Selawi Island in `chrome://extensions`.

**Firefox:** `about:debugging` → This Firefox → **Load Temporary Add-on**
→ `/usr/share/selawi-island/browser/firefox/manifest.json` (removed when
Firefox restarts). For a permanent install, see “Firefox: a signed copy”
in [BROWSER_INTEGRATIONS.md](BROWSER_INTEGRATIONS.md).

## Remove

```bash
sudo pacman -R selawi-island
```

Your settings stay. To remove them too:
`dconf reset -f /org/gnome/shell/extensions/selawi-island/` and
`rm -r ~/.local/state/selawi-island ~/.config/selawi-island`.

## Moving from the old install script

If you installed with `./scripts/install-local.sh` before, `update.sh`
removes that home-folder copy first (settings are kept), so the package
is the only one.
