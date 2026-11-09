#!/usr/bin/env bash
# Removes the GNOME Shell extension for the current user.
set -euo pipefail

UUID="selawi-island@selawi"
DEST="${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/$UUID"

command -v gnome-extensions >/dev/null && gnome-extensions disable "$UUID" 2>/dev/null || true
rm -rf "$DEST"
echo "Removed $DEST. Settings are kept; to reset them run:"
echo "  dconf reset -f /org/gnome/shell/extensions/selawi-island/"
