#!/usr/bin/env bash
# Lays out an installed Selawi Island under DESTDIR (system paths, /usr).
# Used by packaging/arch/PKGBUILD; expects a release build
# (cargo build --release) and a browser build (scripts/build-browser.sh).
#   scripts/stage-package.sh DESTDIR
set -euo pipefail
cd "$(dirname "$0")/.."

DEST="${1:?usage: stage-package.sh DESTDIR}"
UUID="selawi-island@selawi"
TARGET="${CARGO_TARGET_DIR:-target}/${SELAWI_PROFILE:-release}"
CHROME_EXTENSION_ID="bjooidhahonkdcdpibfadoapilihpgig"
FIREFOX_EXTENSION_ID="island@selawi.local"

# Programs.
for bin in selawi-islandd selawi-island selawi-island-browser-host; do
    install -Dm755 "$TARGET/$bin" "$DEST/usr/bin/$bin"
done
install -Dm755 scripts/update.sh "$DEST/usr/bin/selawi-island-update"

# Daemon: systemd user service and D-Bus activation.
SERVICE="$(mktemp)"
trap 'rm -f "$SERVICE"' EXIT
sed -e 's|%h/.local/bin/selawi-islandd|/usr/bin/selawi-islandd|' \
    -e 's|file://%h/.local/share/doc/selawi-island/README.md|file:///usr/share/doc/selawi-island/README.md|' \
    packaging/systemd/selawi-islandd.service > "$SERVICE"
install -Dm644 "$SERVICE" "$DEST/usr/lib/systemd/user/selawi-islandd.service"
install -Dm644 packaging/dbus/com.selawi.Island.service "$DEST/usr/share/dbus-1/services/com.selawi.Island.service"

# GNOME Shell extension, with its settings schema compiled next to it.
EXT="$DEST/usr/share/gnome-shell/extensions/$UUID"
install -d "$EXT"
cp -r gnome-extension/metadata.json gnome-extension/extension.js gnome-extension/prefs.js \
      gnome-extension/stylesheet.css gnome-extension/settings \
      gnome-extension/ui gnome-extension/services gnome-extension/data gnome-extension/schemas "$EXT/"
glib-compile-schemas "$EXT/schemas"
chmod -R u=rwX,go=rX "$EXT"

# Settings app: its code, the shared settings pages, launcher, app grid
# entry and icon.
APP="$DEST/usr/share/selawi-island/settings"
install -d "$APP/pages"
install -m644 settings-app/main.js settings-app/window.js "$APP/"
install -m644 gnome-extension/settings/*.js "$APP/pages/"
install -Dm755 packaging/app/selawi-island-settings "$DEST/usr/bin/selawi-island-settings"
install -Dm644 packaging/app/com.selawi.Island.Settings.desktop \
    "$DEST/usr/share/applications/com.selawi.Island.Settings.desktop"
install -Dm644 packaging/app/com.selawi.Island.Settings.svg \
    "$DEST/usr/share/icons/hicolor/scalable/apps/com.selawi.Island.Settings.svg"

# Browser extension files (loaded by hand) and the helper's registration
# with each browser.
install -d "$DEST/usr/share/selawi-island/browser"
cp -r browser-extensions/build/chrome browser-extensions/build/firefox "$DEST/usr/share/selawi-island/browser/"
chmod -R u=rwX,go=rX "$DEST/usr/share/selawi-island"

chrome_host() {
    install -d "$1"
    cat > "$1/io.selawi.island.json" <<JSON
{
  "name": "io.selawi.island",
  "description": "Selawi Island browser helper",
  "path": "/usr/bin/selawi-island-browser-host",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$CHROME_EXTENSION_ID/"]
}
JSON
    chmod 644 "$1/io.selawi.island.json"
}
chrome_host "$DEST/etc/opt/chrome/native-messaging-hosts"
chrome_host "$DEST/etc/chromium/native-messaging-hosts"
install -d "$DEST/usr/lib/mozilla/native-messaging-hosts"
cat > "$DEST/usr/lib/mozilla/native-messaging-hosts/io.selawi.island.json" <<JSON
{
  "name": "io.selawi.island",
  "description": "Selawi Island browser helper",
  "path": "/usr/bin/selawi-island-browser-host",
  "type": "stdio",
  "allowed_extensions": ["$FIREFOX_EXTENSION_ID"]
}
JSON
chmod 644 "$DEST/usr/lib/mozilla/native-messaging-hosts/io.selawi.island.json"

# pacman hooks: installs, updates and removals in the island.
for hook in packaging/pacman/*.hook; do
    install -Dm644 "$hook" "$DEST/usr/share/libalpm/hooks/$(basename "$hook")"
done

# Plugin helper for Python apps (import selawi_island).
if command -v python3 >/dev/null; then
    site=$(python3 -c "import sysconfig; print(sysconfig.get_path('purelib', vars={'base': '/usr'}))")
    install -Dm644 sdk/python/selawi_island.py "$DEST$site/selawi_island.py"
fi
install -Dm644 docs/PLUGINS.md "$DEST/usr/share/doc/selawi-island/PLUGINS.md"

install -Dm644 README.md "$DEST/usr/share/doc/selawi-island/README.md"
install -Dm644 LICENSE "$DEST/usr/share/licenses/selawi-island/LICENSE"
install -Dm644 docs/INSTALL.md "$DEST/usr/share/doc/selawi-island/INSTALL.md"
