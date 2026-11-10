#!/usr/bin/env bash
# Runs the extension's preferences window headless (Xvfb) with an in-memory
# settings backend, against a real daemon on a private D-Bus session.
# Needs gjs, GTK 4, libadwaita, xvfb-run and dbus-run-session, and the
# daemon built (cargo build -p selawi-islandd).
set -euo pipefail
cd "$(dirname "$0")/../.."
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/schemas"
cp gnome-extension/schemas/*.xml "$WORK/schemas/"
glib-compile-schemas "$WORK/schemas"
sed "s#resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js#./shim.js#" gnome-extension/prefs.js > "$WORK/prefs.js"
cp -r gnome-extension/settings "$WORK/"
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
cat > "$WORK/run.sh" <<RUN
XDG_CONFIG_HOME="$WORK/config" ./target/debug/selawi-islandd --no-persist >/dev/null 2>&1 &
DAEMON=\$!
for _ in \$(seq 50); do ./target/debug/selawi-island status >/dev/null 2>&1 && break; sleep 0.1; done
status=0
GSETTINGS_BACKEND=memory timeout 60 xvfb-run -a gjs -m tests/integration/prefs.gjs.js "$WORK" || status=\$?
[ \$status = 0 ] && { GSETTINGS_BACKEND=memory SELAWI_ISLAND_SCHEMA_DIR="$WORK/schemas" timeout 60 xvfb-run -a gjs -m tests/integration/settings-app.gjs.js || status=\$?; }
kill \$DAEMON 2>/dev/null
exit \$status
RUN
out=$(dbus-run-session -- bash "$WORK/run.sh" 2>&1) || status=$?
echo "$out" | grep -v -e "Gtk-WARNING" -e "libEGL" -e "fd limit" -e "^$" || true
exit "$status"
