// "Microphone / camera in use", from what GNOME itself watches: recording
// streams in GNOME's mixer and Shell.CameraMonitor (the same sources as
// GNOME's own top-bar privacy icons). Sent to the daemon as one event
// (`privacy:capture`) that lasts while the mic or camera is in use.
// It says which apps record; it never claims a call is happening.

import Shell from 'gi://Shell';

import * as Volume from 'resource:///org/gnome/shell/ui/status/volume.js';

import {METER_APP_ID, METER_APP_NAME} from './level-meter.js';

const EVENT_ID = 'privacy:capture';
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
        this._mic = false;

        const update = () => this._update();
        this._control.connectObject(
            'state-changed', update,
            'stream-added', update,
            'stream-removed', update,
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

    destroy() {
        this._control.disconnectObject(this);
        this._camera.disconnectObject(this);
        this._camera = null;
        if (this._shownKey)
            this._send('ended', this._lastInput);
    }

    _recordingApps() {
        const names = new Set();
        for (const output of this._control.get_source_outputs()) {
            const id = output.get_application_id() ?? '';
            const name = output.get_name() || id;
            if (SKIPPED_APPS.includes(id) || name === METER_APP_NAME)
                continue;
            names.add(String(name).slice(0, 60));
        }
        return [...names];
    }

    _update() {
        const apps = this._recordingApps();
        const mic = apps.length > 0;
        const camera = Boolean(this._camera?.cameras_in_use);
        if (mic !== this._mic) {
            this._mic = mic;
            this._onMicChanged(mic);
        }

        if (!mic && !camera) {
            if (this._shownKey) {
                this._send('ended', this._lastInput);
                this._shownKey = null;
            }
            return;
        }
        const title = camera ? (mic ? 'Camera and microphone' : 'Camera') : 'Microphone';
        const detail = apps.length ? apps.join(', ') : 'In use';
        const key = `${title}|${detail}`;
        if (key === this._shownKey)
            return;
        const input = {
            id: EVENT_ID,
            source: {app: apps[0] ?? 'Camera', integration: 'gnome-privacy'},
            category: 'call',
            priority: 'call',
            persistent: true,
            payload: {type: 'system', title, detail, severity: 'info'},
        };
        this._send(this._shownKey ? 'updated' : 'started', input);
        this._shownKey = key;
        this._lastInput = input;
    }

    _send(state, input) {
        this._submit({...input, state});
    }
}
