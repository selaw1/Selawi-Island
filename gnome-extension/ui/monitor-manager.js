// Decides where the island lives: which monitor, how far from the top,
// the current scale factor, and whether a fullscreen window should hide it.
// Emits 'changed' whenever any of that changes.

import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {EventEmitter} from 'resource:///org/gnome/shell/misc/signals.js';

export class MonitorManager extends EventEmitter {
    constructor(settings) {
        super();
        this._settings = settings;
        this._themeContext = St.ThemeContext.get_for_stage(global.stage);

        const changed = () => this.emit('changed');
        Main.layoutManager.connectObject('monitors-changed', changed, this);
        Main.layoutManager.panelBox.connectObject('notify::height', changed, this);
        Main.overview.connectObject('showing', changed, 'hidden', changed, this);
        global.display.connectObject('in-fullscreen-changed', changed, this);
        this._themeContext.connectObject('notify::scale-factor', changed, this);
        this._settings.connectObject(
            'changed::monitor-index', changed,
            'changed::below-top-bar', changed,
            'changed::top-margin', changed, this);
    }

    get scale() {
        return this._themeContext.scale_factor;
    }

    get monitor() {
        const monitors = Main.layoutManager.monitors;
        const index = this._settings.get_int('monitor-index');
        return monitors[index] ?? Main.layoutManager.primaryMonitor ?? monitors[0] ?? null;
    }

    /**
     * The chosen monitor's rectangle and the y of the default spot.
     *
     * @returns {{x: number, y: number, width: number, height: number, defaultTop: number} | null}
     */
    area() {
        const monitor = this.monitor;
        if (!monitor)
            return null;
        let top = monitor.y + this._settings.get_int('top-margin') * this.scale;
        // The top bar only exists on the primary monitor.
        if (this._settings.get_boolean('below-top-bar') &&
            monitor.index === Main.layoutManager.primaryIndex)
            top += Main.layoutManager.panelBox.height;
        return {
            x: monitor.x,
            y: monitor.y,
            width: monitor.width,
            height: monitor.height,
            defaultTop: Math.round(top),
        };
    }

    /** Index of the monitor an actor is mostly on. */
    monitorIndexFor(actor) {
        return Main.layoutManager.findIndexForActor(actor);
    }

    /** Monitor object for an index (primary when out of range). */
    monitorAt(index) {
        return Main.layoutManager.monitors[index] ?? Main.layoutManager.primaryMonitor ?? null;
    }

    get primaryIndex() {
        return Main.layoutManager.primaryIndex;
    }

    /** Same rule GNOME uses for its own chrome. */
    get fullscreenBlocked() {
        const monitor = this.monitor;
        return Boolean(monitor?.inFullscreen) && !Main.overview.visible;
    }

    destroy() {
        Main.layoutManager.disconnectObject(this);
        Main.layoutManager.panelBox.disconnectObject(this);
        Main.overview.disconnectObject(this);
        global.display.disconnectObject(this);
        this._themeContext.disconnectObject(this);
        this._settings.disconnectObject(this);
    }
}
