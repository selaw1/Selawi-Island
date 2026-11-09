// "Microphone / camera in use", from what GNOME itself watches: recording
// streams in GNOME's mixer and Shell.CameraMonitor (the same sources as
// GNOME's own top-bar privacy icons). Sent to the daemon as one event
// (`privacy:capture`) that lasts while the mic or camera is in use.
// It says which apps record; it never claims a call is happening.
//
// Mute / Unmute (a backup for apps the island cannot control directly):
// mutes the recording apps' microphone streams, or the whole microphone,
// at the system level. The app itself still shows "unmuted". Linux
// remembers per-app mute, so whatever the island muted is unmuted again
// once that app stops recording, and the next call starts unmuted.

import Shell from 'gi://Shell';

import * as Volume from 'resource:///org/gnome/shell/ui/status/volume.js';

import {METER_APP_ID, METER_APP_NAME} from './level-meter.js';
import {privacyInput} from './privacy-model.js';

// These show an input level meter and would look like recording apps.
const SKIPPED_APPS = ['org.gnome.VolumeControl', 'org.PulseAudio.pavucontrol', METER_APP_ID];

export class PrivacySource {
    /**
     * @param {(input: object) => void} submit - sends one event input
     * @param {(micInUse: boolean) => void} onMicChanged
     * @param {() => string} muteScope - 'apps' or 'microphone'
     */
    constructor(submit, onMicChanged, muteScope) {
        this._submit = submit;
        this._onMicChanged = onMicChanged;
        this._muteScope = muteScope;
        this._control = Volume.getMixerControl();
        this._camera = new Shell.CameraMonitor();
        this._shownKey = null;
        this._lastInput = null;
        this._mic = false;
        // Apps (by key) whose streams the island muted, and whether it
        // muted the whole microphone; undone after the call.
        this._mutedApps = new Set();
        this._mutedMicrophone = false;

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
        const outputs = this._outputs();
        if (this._muteScope() === 'microphone') {
            const source = this._control.get_default_source();
            if (source) {
                source.change_is_muted(mute);
                this._mutedMicrophone = mute;
            }
        } else {
            for (const output of outputs) {
                output.change_is_muted(mute);
                if (mute)
                    this._mutedApps.add(this._key(output));
                else
                    this._mutedApps.delete(this._key(output));
            }
            // Unmute means "let them hear me": also undo a whole-mic mute.
            const source = this._control.get_default_source();
            if (!mute && source?.is_muted) {
                source.change_is_muted(false);
                this._mutedMicrophone = false;
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

    _key(output) {
        return output.get_application_id() || output.get_name() || '';
    }

    _outputs() {
        return this._control.get_source_outputs().filter(output => {
            const id = output.get_application_id() ?? '';
            return !SKIPPED_APPS.includes(id) && output.get_name() !== METER_APP_NAME;
        });
    }

    _update() {
        const outputs = this._outputs();
        const mic = outputs.length > 0;

        if (!mic)
            this._undoMuteAfterCall();
        else
            this._unmuteRememberedApps(outputs);

        const apps = [...new Set(outputs.map(o => String(o.get_name() || this._key(o)).slice(0, 60)))];
        const sourceMuted = Boolean(this._control.get_default_source()?.is_muted);
        const muted = mic && (sourceMuted || outputs.every(o => o.is_muted));
        const camera = Boolean(this._camera?.cameras_in_use);

        if (mic !== this._mic) {
            this._mic = mic;
            this._onMicChanged(mic);
        }

        const input = privacyInput({apps, camera, muted});
        if (!input) {
            if (this._shownKey) {
                this._send('ended', this._lastInput);
                this._shownKey = null;
            }
            return;
        }
        const key = JSON.stringify([input.payload, input.actions]);
        if (key === this._shownKey)
            return;
        this._send(this._shownKey ? 'updated' : 'started', input);
        this._shownKey = key;
        this._lastInput = input;
    }

    // The call is over: give the microphone back.
    _undoMuteAfterCall() {
        if (this._mutedMicrophone) {
            this._control.get_default_source()?.change_is_muted(false);
            this._mutedMicrophone = false;
        }
        // App mutes are undone when those apps record again (below),
        // because their streams no longer exist now.
        this._pendingUnmute = new Set([...this._pendingUnmute ?? [], ...this._mutedApps]);
        this._mutedApps.clear();
    }

    // An app the island muted during an earlier call starts recording
    // again, restored as muted by Linux: unmute it.
    _unmuteRememberedApps(outputs) {
        if (!this._pendingUnmute?.size)
            return;
        for (const output of outputs) {
            const key = this._key(output);
            if (this._pendingUnmute.has(key) && !this._mutedApps.has(key)) {
                this._pendingUnmute.delete(key);
                if (output.is_muted)
                    output.change_is_muted(false);
            }
        }
    }

    _send(state, input) {
        this._submit({...input, state});
    }
}
