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

# Content scripts run as plain page scripts: no import/export allowed.
for script in meet media call rtc-main session session-main; do
    sed -i '/^export {};$/d' "build/js/$script.js"
    if grep -qE '^(import|export)' "build/js/$script.js"; then
        echo "$script.js must not import or export anything"; exit 1
    fi
done

for browser in chrome firefox; do
    mkdir -p "build/$browser"
    cp "$browser/manifest.json" build/js/background.js build/js/model.js build/js/meet.js build/js/media.js build/js/call.js build/js/rtc-main.js build/js/session.js build/js/session-main.js "build/$browser/"
done
echo "Built browser-extensions/build/chrome and browser-extensions/build/firefox"
