#!/usr/bin/env bash
# Packs the Firefox extension into dist/selawi-island-firefox-<version>.xpi,
# ready to upload to addons.mozilla.org as an *unlisted* add-on (free,
# private: not shown in the store). Mozilla signs it and gives back a file
# that installs permanently. Each upload needs a higher version; this uses
# the project version plus the number of commits (e.g. 0.1.0.412).
set -euo pipefail
cd "$(dirname "$0")/.."

command -v zip >/dev/null || { echo "zip not found: sudo pacman -S --needed zip"; exit 1; }
./scripts/build-browser.sh >/dev/null
base=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
version="$base.$(git rev-list --count HEAD)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp -r browser-extensions/build/firefox/. "$work/"
node -e '
const fs = require("fs");
const file = process.argv[1];
const manifest = JSON.parse(fs.readFileSync(file, "utf8"));
manifest.version = process.argv[2];
fs.writeFileSync(file, JSON.stringify(manifest, null, 2) + "\n");
' "$work/manifest.json" "$version"

mkdir -p dist
out="$PWD/dist/selawi-island-firefox-$version.xpi"
rm -f "$out"
(cd "$work" && zip -qr -X "$out" .)
echo "Ready: $out"
echo "Upload it at https://addons.mozilla.org/developers/addon/submit/distribution → “On your own”."
