#!/usr/bin/env bash
# Builds the browser extensions into browser-extensions/build/{chrome,firefox}.
# Needs Node.js and pnpm (sudo pacman -S --needed nodejs pnpm).
set -euo pipefail
cd "$(dirname "$0")/.."

command -v pnpm >/dev/null || { echo "pnpm not found. Install it: sudo pacman -S --needed nodejs pnpm"; exit 1; }
if [ ! -x node_modules/.bin/tsc ]; then
    echo "Installing the TypeScript compiler (development only)..."
    pnpm install --frozen-lockfile --reporter=silent
fi

cd browser-extensions
rm -rf build
../node_modules/.bin/tsc -p tsconfig.json

# meet.js runs as a plain page script: no import/export allowed there.
sed -i '/^export {};$/d' build/js/meet.js
if grep -qE '^(import|export)' build/js/meet.js; then
    echo "meet.js must not import or export anything"; exit 1
fi

for browser in chrome firefox; do
    mkdir -p "build/$browser"
    cp "$browser/manifest.json" build/js/background.js build/js/model.js build/js/meet.js "build/$browser/"
done
echo "Built browser-extensions/build/chrome and browser-extensions/build/firefox"
