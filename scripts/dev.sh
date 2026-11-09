#!/usr/bin/env bash
# Installs the extension and starts a nested GNOME Shell window to try it
# without logging out. Close the window to stop it.
set -euo pipefail

UUID="selawi-island@selawi"
"$(dirname "$0")/install-local.sh"
gnome-extensions enable "$UUID" 2>/dev/null || true

echo "Starting nested GNOME Shell (logs below; Ctrl+C to stop)..."
export MUTTER_DEBUG_DUMMY_MODE_SPECS="${MUTTER_DEBUG_DUMMY_MODE_SPECS:-1600x900}"
exec dbus-run-session -- gnome-shell --wayland --devkit
