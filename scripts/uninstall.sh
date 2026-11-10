#!/usr/bin/env bash
# Removes everything install-local.sh installed. Settings and the saved
# event file are kept unless --purge is given.
set -euo pipefail

UUID="selawi-island@selawi"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
STATE="${XDG_STATE_HOME:-$HOME/.local/state}"

command -v gnome-extensions >/dev/null && gnome-extensions disable "$UUID" 2>/dev/null || true
rm -rf "$DATA/gnome-shell/extensions/$UUID"

if command -v systemctl >/dev/null; then
    systemctl --user disable --now selawi-islandd.service 2>/dev/null || true
fi
rm -f "$HOME/.local/bin/selawi-islandd" "$HOME/.local/bin/selawi-island" \
      "$HOME/.local/bin/selawi-island-browser-host" \
      "$HOME/.mozilla/native-messaging-hosts/io.selawi.island.json" \
      "$CONFIG/systemd/user/selawi-islandd.service" \
      "$DATA/dbus-1/services/com.selawi.Island.service"
rm -rf "$DATA/doc/selawi-island" "$DATA/selawi-island/browser" "$DATA/selawi-island/settings"
rm -f "$HOME/.local/bin/selawi-island-settings" "$DATA/applications/com.selawi.Island.Settings.desktop" \
      "$DATA/icons/hicolor/scalable/apps/com.selawi.Island.Settings.svg"
for dir in google-chrome google-chrome-beta google-chrome-unstable chromium BraveSoftware/Brave-Browser; do
    rm -f "$CONFIG/$dir/NativeMessagingHosts/io.selawi.island.json"
done
command -v systemctl >/dev/null && systemctl --user daemon-reload 2>/dev/null || true

if [ "${1:-}" = "--purge" ]; then
    rm -rf "$STATE/selawi-island"
    command -v dconf >/dev/null && dconf reset -f /org/gnome/shell/extensions/selawi-island/ || true
    echo "Removed Selawi Island, its settings and saved events."
else
    echo "Removed Selawi Island. Settings and saved events kept (use --purge to remove them)."
fi
