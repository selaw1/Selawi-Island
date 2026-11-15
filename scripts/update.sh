#!/usr/bin/env bash
# Installs or updates Selawi Island as an Arch package, from your copy of
# the repository. Installed as `selawi-island-update`.
#
#   First time, from the repository:  ./scripts/update.sh
#   After that, from anywhere:        selawi-island-update
#
# It pulls the latest committed code, builds the package with makepkg,
# installs it with pacman (asks for your password), then turns the
# extension and the daemon on for you.
#   --no-pull     build what you have, without git pull
#   --repo DIR    use the repository in DIR (remembered for next time)
#   --after-install   only the per-user steps (run by the newly installed
#                     copy of this script, so new steps apply at once)
set -euo pipefail

UUID="selawi-island@selawi"
CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
SAVED="$CONFIG/selawi-island/source-dir"

repo=""
pull=1
after_install=0
while [ $# -gt 0 ]; do
    case "$1" in
        --no-pull) pull=0 ;;
        --after-install) after_install=1 ;;
        --repo) repo="${2:?--repo needs a folder}"; shift ;;
        -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
    shift
done

[ "$(id -u)" -ne 0 ] || { echo "Run this as yourself, not root (it asks for your password when needed)."; exit 1; }

# Per-user parts a package can't do.
per_user_steps() {
    if systemctl --user show-environment >/dev/null 2>&1; then
        systemctl --user daemon-reload
        systemctl --user enable selawi-islandd.service >/dev/null 2>&1 || true
        systemctl --user restart selawi-islandd.service || true
    fi
    command -v gnome-extensions >/dev/null && gnome-extensions enable "$UUID" 2>/dev/null || true

    # Claude Code in the island: asked once, then remembered. When on, every
    # update refreshes the island's hooks in ~/.claude/settings.json (only its
    # own entries; a backup is kept). Change it any time:
    #   selawi-island agent-hooks on | off
    agents="$(selawi-island agent-hooks status --choice 2>/dev/null || echo unset)"
    if [ "$agents" = unset ]; then
        if [ -t 0 ]; then
            echo
            echo "Selawi Island can show what Claude Code is doing (working, waiting for"
            echo "your approval, done). It adds its own hooks to ~/.claude/settings.json"
            echo "and keeps a backup; nothing else in that file changes."
            read -r -p "Show Claude Code progress in the island? [Y/n] " answer || answer=n
            case "$answer" in
                [nN]*) selawi-island agent-hooks off >/dev/null || true ;;
                *) selawi-island agent-hooks on || echo "Claude Code progress was not set up (see above)." ;;
            esac
        else
            echo "Claude Code progress in the island: turn on with  selawi-island agent-hooks on"
        fi
    elif [ "$agents" = on ]; then
        selawi-island agent-hooks on >/dev/null || echo "Could not refresh Claude Code progress: run  selawi-island agent-hooks on"
    fi
    cat <<'DONE'

Selawi Island is installed (version: pacman -Q selawi-island).
  - Log out and back in so GNOME loads the new island.
  - Chrome: chrome://extensions → reload Selawi Island.
    First time: Developer mode → Load unpacked → /usr/share/selawi-island/browser/chrome
  - Remove it any time: sudo pacman -R selawi-island
DONE
}

if [ "$after_install" = 1 ]; then
    per_user_steps
    exit 0
fi
command -v makepkg >/dev/null || { echo "makepkg not found: this needs Arch Linux (pacman)."; exit 1; }

# Which repository: --repo, the one this script is in, or the saved one.
if [ -z "$repo" ]; then
    here="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
    if [ -f "$here/packaging/arch/PKGBUILD" ]; then
        repo="$here"
    elif [ -f "$SAVED" ]; then
        repo="$(cat "$SAVED")"
    fi
fi
if [ -z "$repo" ] || [ ! -f "$repo/packaging/arch/PKGBUILD" ]; then
    echo "Can't find your Selawi Island folder. Run: selawi-island-update --repo /path/to/Selawi-Island"
    exit 1
fi
repo="$(cd "$repo" && pwd)"
mkdir -p "$(dirname "$SAVED")"
printf '%s\n' "$repo" > "$SAVED"

# makepkg writes the version it works out back into the PKGBUILD: undo
# that, so it never looks like a local change or blocks git pull.
restore_pkgbuild() {
    git -C "$repo" checkout --quiet -- packaging/arch/PKGBUILD 2>/dev/null || true
}
restore_pkgbuild
trap restore_pkgbuild EXIT

if [ "$pull" = 1 ]; then
    echo "Getting the latest code..."
    git -C "$repo" pull --ff-only
fi
if [ -n "$(git -C "$repo" status --porcelain --untracked-files=no)" ]; then
    echo "Note: you have uncommitted changes; the package is built from the last commit only."
fi

# An older install from install-local.sh lives in your home folder and
# would hide the package: remove it (settings are kept).
if [ -e "$HOME/.local/bin/selawi-islandd" ] || [ -d "${XDG_DATA_HOME:-$HOME/.local/share}/gnome-shell/extensions/$UUID" ]; then
    echo "Removing the older home-folder install (settings are kept)..."
    "$repo/scripts/uninstall.sh" >/dev/null
fi

echo "Building and installing the package..."
(cd "$repo/packaging/arch" && makepkg -sicf --noconfirm --needed)
rm -rf "$repo/packaging/arch/src" "$repo/packaging/arch/pkg" \
       "$repo/packaging/arch/selawi-island" "$repo/packaging/arch/"*.pkg.tar.*

# The newly installed copy of this script does the per-user steps, so a
# step added in this update already runs now.
if [ -x /usr/bin/selawi-island-update ]; then
    restore_pkgbuild
    trap - EXIT
    exec /usr/bin/selawi-island-update --after-install
fi
per_user_steps
