#!/usr/bin/env bash
# Installs the GNOME Shell extension for the current user.
set -euo pipefail

UUID="selawi-island@selawi"
SRC="$(cd "$(dirname "$0")/../gnome-extension" && pwd)"
DEST="${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/$UUID"

command -v glib-compile-schemas >/dev/null || { echo "glib-compile-schemas not found (install glib2)"; exit 1; }

rm -rf "$DEST"
mkdir -p "$DEST"
cp -r "$SRC"/metadata.json "$SRC"/extension.js "$SRC"/prefs.js "$SRC"/stylesheet.css \
      "$SRC"/ui "$SRC"/services "$SRC"/data "$SRC"/schemas "$DEST"/
glib-compile-schemas "$DEST/schemas"

echo "Installed to $DEST"
if command -v gnome-extensions >/dev/null; then
    gnome-extensions enable "$UUID" 2>/dev/null \
        && echo "Enabled $UUID" \
        || echo "GNOME does not see the extension yet: log out and back in, then run: gnome-extensions enable $UUID"
fi
echo "On Wayland, GNOME loads extension code once per login. After updating, log out and back in (or use scripts/dev.sh)."
