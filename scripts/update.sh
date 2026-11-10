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
set -euo pipefail

UUID="selawi-island@selawi"
CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}"
SAVED="$CONFIG/selawi-island/source-dir"

repo=""
pull=1
while [ $# -gt 0 ]; do
    case "$1" in
        --no-pull) pull=0 ;;
        --repo) repo="${2:?--repo needs a folder}"; shift ;;
        -h|--help) sed -n '2,15p' "$0"; exit 0 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
    shift
done

[ "$(id -u)" -ne 0 ] || { echo "Run this as yourself, not root (it asks for your password when needed)."; exit 1; }
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

# Per-user parts a package can't do.
if systemctl --user show-environment >/dev/null 2>&1; then
    systemctl --user daemon-reload
    systemctl --user enable selawi-islandd.service >/dev/null 2>&1 || true
    systemctl --user restart selawi-islandd.service || true
fi
command -v gnome-extensions >/dev/null && gnome-extensions enable "$UUID" 2>/dev/null || true

cat <<'DONE'

Selawi Island is installed (version: pacman -Q selawi-island).
  - Log out and back in so GNOME loads the new island.
  - Chrome: chrome://extensions → reload Selawi Island.
    First time: Developer mode → Load unpacked → /usr/share/selawi-island/browser/chrome
  - Remove it any time: sudo pacman -R selawi-island
DONE
