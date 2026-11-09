#!/usr/bin/env bash
# Builds and installs Selawi Island for the current user:
#   - selawi-islandd (daemon) and selawi-island (CLI) into ~/.local/bin
#   - the systemd user service and D-Bus activation file
#   - the GNOME Shell extension
#   - the browser helper (native messaging host) and, if npm is available,
#     the browser extension in ~/.local/share/selawi-island/browser/
# Run again to update. Use --extension-only to skip the Rust and browser parts.
set -euo pipefail
cd "$(dirname "$0")/.."

UUID="selawi-island@selawi"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
BIN="$HOME/.local/bin"
EXT_DEST="$DATA/gnome-shell/extensions/$UUID"

install_extension() {
    command -v glib-compile-schemas >/dev/null || { echo "glib-compile-schemas not found (install glib2)"; exit 1; }
    local src=gnome-extension
    rm -rf "$EXT_DEST"
    mkdir -p "$EXT_DEST"
    cp -r "$src"/metadata.json "$src"/extension.js "$src"/prefs.js "$src"/prefs-integrations.js "$src"/stylesheet.css \
          "$src"/ui "$src"/services "$src"/data "$src"/schemas "$EXT_DEST"/
    glib-compile-schemas "$EXT_DEST/schemas"
    echo "Extension installed to $EXT_DEST"
    if command -v gnome-extensions >/dev/null; then
        gnome-extensions enable "$UUID" 2>/dev/null \
            || echo "GNOME does not see the extension yet: log out and back in, then run: gnome-extensions enable $UUID"
    fi
}

install_daemon() {
    command -v cargo >/dev/null || { echo "cargo not found. Install Rust: sudo pacman -S --needed rust"; exit 1; }
    echo "Building (release)..."
    cargo build --release --locked -p selawi-islandd -p selawi-island -p selawi-island-browser-host
    install -Dm755 target/release/selawi-islandd "$BIN/selawi-islandd"
    install -Dm755 target/release/selawi-island "$BIN/selawi-island"
    install -Dm755 target/release/selawi-island-browser-host "$BIN/selawi-island-browser-host"
    install -Dm644 packaging/systemd/selawi-islandd.service "$CONFIG/systemd/user/selawi-islandd.service"
    install -Dm644 packaging/dbus/com.selawi.Island.service "$DATA/dbus-1/services/com.selawi.Island.service"
    install -Dm644 README.md "$DATA/doc/selawi-island/README.md"
    echo "Installed selawi-islandd and selawi-island to $BIN"

    if command -v systemctl >/dev/null && systemctl --user show-environment >/dev/null 2>&1; then
        systemctl --user daemon-reload
        systemctl --user enable selawi-islandd.service >/dev/null 2>&1
        systemctl --user restart selawi-islandd.service
        echo "Daemon running: systemctl --user status selawi-islandd"
    else
        echo "No systemd user session found; start the daemon with: $BIN/selawi-islandd"
    fi

    case ":$PATH:" in
        *":$BIN:"*) ;;
        *) echo "Note: $BIN is not in your PATH. Add this to ~/.bashrc to use 'selawi-island':"
           echo "  export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
    esac
}

CHROME_EXTENSION_ID="bjooidhahonkdcdpibfadoapilihpgig"
FIREFOX_EXTENSION_ID="island@selawi.local"

install_browser() {
    local host="$BIN/selawi-island-browser-host"
    # Tell each browser where the helper is and which extension may start it.
    for dir in google-chrome google-chrome-beta google-chrome-unstable chromium BraveSoftware/Brave-Browser; do
        install -d "$CONFIG/$dir/NativeMessagingHosts"
        cat > "$CONFIG/$dir/NativeMessagingHosts/io.selawi.island.json" <<JSON
{
  "name": "io.selawi.island",
  "description": "Selawi Island browser helper",
  "path": "$host",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$CHROME_EXTENSION_ID/"]
}
JSON
    done
    install -d "$HOME/.mozilla/native-messaging-hosts"
    cat > "$HOME/.mozilla/native-messaging-hosts/io.selawi.island.json" <<JSON
{
  "name": "io.selawi.island",
  "description": "Selawi Island browser helper",
  "path": "$host",
  "type": "stdio",
  "allowed_extensions": ["$FIREFOX_EXTENSION_ID"]
}
JSON
    if command -v npm >/dev/null; then
        ./scripts/build-browser.sh >/dev/null
        rm -rf "$DATA/selawi-island/browser"
        install -d "$DATA/selawi-island/browser"
        cp -r browser-extensions/build/chrome browser-extensions/build/firefox "$DATA/selawi-island/browser/"
        echo "Browser extension ready in $DATA/selawi-island/browser (see docs/BROWSER_INTEGRATIONS.md to load it)"
    else
        echo "npm not found: browser extension not built (sudo pacman -S --needed nodejs npm, then run this again)"
    fi
}

if [ "${1:-}" != "--extension-only" ]; then
    install_daemon
    install_browser
fi
install_extension
echo "On Wayland, GNOME loads extension code once per login: log out and back in after updating the extension."
