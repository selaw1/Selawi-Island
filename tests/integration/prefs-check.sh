#!/usr/bin/env bash
# Runs the extension's preferences window headless (Xvfb) with an in-memory
# settings backend. Needs gjs, GTK 4, libadwaita and xvfb-run.
set -euo pipefail
cd "$(dirname "$0")/../.."
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/schemas"
cp gnome-extension/schemas/*.xml "$WORK/schemas/"
glib-compile-schemas "$WORK/schemas"
sed "s#resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js#./shim.js#" gnome-extension/prefs.js > "$WORK/prefs.js"
cat > "$WORK/shim.js" <<SHIM
import Gio from 'gi://Gio';
export class ExtensionPreferences {
    getSettings() {
        const src = Gio.SettingsSchemaSource.new_from_directory('$WORK/schemas', Gio.SettingsSchemaSource.get_default(), false);
        return new Gio.Settings({settings_schema: src.lookup('org.gnome.shell.extensions.selawi-island', true)});
    }
}
SHIM
status=0
out=$(GSETTINGS_BACKEND=memory xvfb-run -a gjs -m tests/integration/prefs.gjs.js "$WORK" 2>&1) || status=$?
echo "$out" | grep -v -e "Gtk-WARNING" -e "libEGL" -e "^$" || true
exit "$status"
