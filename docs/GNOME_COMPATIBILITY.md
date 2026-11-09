# GNOME compatibility

Target: **GNOME Shell 51** (tested against the 51.1 source). `metadata.json`
declares only `"51"`; add versions after testing them.

## API notes checked against GNOME Shell / mutter 51.1 source

| Topic | What 51 needs | Used in |
|---|---|---|
| Extension format | ES modules, `export default class … extends Extension` | `extension.js`, `prefs.js` |
| Vertical boxes | `St.BoxLayout` has no `vertical` property any more; use `orientation: Clutter.Orientation.VERTICAL` | views |
| Clicks | `Clutter.ClickGesture` added with `add_action()`. The shell no longer uses `button-release-event` | `island.js` |
| Dragging | `Clutter.PanGesture` (`recognize`, `pan-update`, `end`, `cancel`, `get_centroid_abs()`); it starts after 16 px of movement and then cancels the click. `global.stage.grab(actor)` must be held during the drag (as `slider.js` does), otherwise the pointer is handed to the window underneath and the gesture is cancelled | `island.js` |
| Clock width | `Main.panel.statusArea.dateMenu` (`notify::width`) | `monitor-manager.js` |
| Keys | `Clutter.KeyController` (`key-press`, `get_key()`) | `island.js` |
| Nested buttons | Innermost gesture recognizes first and cancels outer ones; `island.js` also skips the toggle when the click lands on an `St.Button` | `island.js` |
| Chrome | `Main.layoutManager.addTopChrome(actor, {trackFullscreen})`. The only accepted options are `trackFullscreen` and `affectsStruts`; `affectsInputRegion` was removed (any other key throws). Visible reactive actors receive input automatically | `island.js` |
| Fullscreen rule | Hidden when `monitor.inFullscreen && !Main.overview.visible` (same rule GNOME uses) | `monitor-manager.js` |
| Reduced motion | `actor.ease()` returns 0 duration when “Reduce animation” is on | everywhere |
| HiDPI | Sizes set in JS are multiplied by `St.ThemeContext.scale_factor`; CSS sizes are scaled by St | `island.js` |
| Keyboard focus | `Main.ctrlAltTabManager.addGroup()` (removed when the actor is destroyed) | `island.js` |
| Icons | Only symbolic icons from the current Adwaita theme (not its `legacy/` folder) | `view-model.js` |
| Settings | `getSettings()` needs a compiled `schemas/gschemas.compiled` (the install script builds it) | `extension.js` |

## Wayland notes

- GNOME on Wayland cannot restart the shell. Extension code (ES modules) is
  loaded once per login, so code changes need a log out/in or a nested
  session (`scripts/dev.sh`, which runs `gnome-shell --wayland --devkit`).
- The island lives in the shell's own top chrome, so it is always on top and
  placed exactly; normal application windows cannot do this on Wayland.
- `--devkit` needs mutter's devkit viewer. If `dev.sh` reports it missing,
  check which Arch package provides it (`pacman -F mutter-devkit`).
