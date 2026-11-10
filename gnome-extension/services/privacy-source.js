// "Microphone / camera in use", from what GNOME itself watches: recording
// streams in GNOME's mixer and Shell.CameraMonitor (the same sources as
// GNOME's own top-bar privacy icons). Sent to the daemon as one event
// (`privacy:capture`) that lasts while the mic or camera is in use.
// It says which apps record; it never claims a call is happening.
//
// Mute / Unmute is GNOME's own microphone mute: the same switch as the
// volume menu and the microphone key, so Linux shows the microphone as
// muted too, and muting from GNOME shows here.

import Shell from 'gi://Shell';

import * as Volume from 'resource:///org/gnome/shell/ui/status/volume.js';

import {METER_APP_ID, METER_APP_NAME} from './level-meter.js';
import {appIcon, appName, isBrowserApp, privacyInput} from './privacy-model.js';

// These show an input level meter and would look like recording apps.
const SKIPPED_APPS = ['org.gnome.VolumeControl', 'org.PulseAudio.pavucontrol', METER_APP_ID];

export class PrivacySource {
    /**
     * @param {(input: object) => void} submit - sends one event input
     * @param {(micInUse: boolean) => void} onMicChanged
     */
    constructor(submit, onMicChanged) {
        this._submit = submit;
        this._onMicChanged = onMicChanged;
        this._control = Volume.getMixerControl();
        this._camera = new Shell.CameraMonitor();
        this._shownKey = null;
        this._lastInput = null;
        this._mic = false;

        const update = () => this._update();
        this._control.connectObject(
            'state-changed', update,
            'stream-added', update,
            'stream-removed', update,
            'stream-changed', update,
            'default-source-changed', update,
            this);
        this._camera.connectObject('notify::cameras-in-use', update, this);
        this._update();
    }

    get micInUse() {
        return this._mic;
    }

    /** Sends the current state again (the daemon just (re)started). */
    resync() {
        this._shownKey = null;
        this._update();
    }

    /** A button on the event was pressed. */
    handleAction(actionId) {
        if (actionId !== 'mute' && actionId !== 'unmute')
            return;
        const mute = actionId === 'mute';
        this._control.get_default_source()?.change_is_muted(mute);
        if (!mute) {
            // Also clear app-level mutes (e.g. left by an older version),
            // so "Unmute" always means they can hear you again.
            for (const output of this._outputs()) {
                if (output.is_muted)
                    output.change_is_muted(false);
            }
        }
    }

    destroy() {
        this._control.disconnectObject(this);
        this._camera.disconnectObject(this);
        this._camera = null;
        if (this._shownKey)
            this._send('ended', this._lastInput);
    }

    _outputs() {
        return this._control.get_source_outputs().filter(output => {
            const id = output.get_application_id() ?? '';
            return !SKIPPED_APPS.includes(id) && output.get_name() !== METER_APP_NAME;
        });
    }

    /**
     * The recording app's logo: the installed app it names (its app id or
     * icon name) or, failing that, a running app with exactly that name.
     * Null for a browser, or when it cannot be matched; the island then shows
     * the mic icon (or the site, from the browser extension).
     */
    _iconFor(output) {
        if (isBrowserApp(output.get_name(), output.get_application_id(), output.get_icon_name()))
            return null;
        const apps = Shell.AppSystem.get_default();
        const byId = id => id ? apps.lookup_app(`${id}.desktop`) ??
            apps.lookup_heuristic_basename(`${id}.desktop`) ??
            apps.lookup_desktop_wmclass(id) ?? apps.lookup_startup_wmclass(id) : null;
        const name = appName(output.get_name()).toLowerCase();
        const app = byId(output.get_application_id()) ?? byId(output.get_icon_name()) ??
            apps.get_running().find(a => a.get_name()?.toLowerCase() === name) ?? null;
        return appIcon(app?.get_id());
    }

    _update() {
        const outputs = this._outputs();
        const mic = outputs.length > 0;
        const apps = [...new Set(outputs.map(o => appName(o.get_name() || o.get_application_id())))];
        const sourceMuted = Boolean(this._control.get_default_source()?.is_muted);
        const muted = mic && (sourceMuted || outputs.every(o => o.is_muted));
        const camera = Boolean(this._camera?.cameras_in_use);
        const icon = outputs.length > 0 ? this._iconFor(outputs[0]) : null;

        if (mic !== this._mic) {
            this._mic = mic;
            this._onMicChanged(mic);
        }

        const input = privacyInput({apps, camera, muted, icon});
        if (!input) {
            if (this._shownKey) {
                this._send('ended', this._lastInput);
                this._shownKey = null;
            }
            return;
        }
        const key = JSON.stringify([input.source, input.payload, input.actions]);
        if (key === this._shownKey)
            return;
        this._send(this._shownKey ? 'updated' : 'started', input);
        this._shownKey = key;
        this._lastInput = input;
    }

    _send(state, input) {
        this._submit({...input, state});
    }
}
