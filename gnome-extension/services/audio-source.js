// Audio device changes from GNOME's own mixer (the same one the volume
// menu uses), sent to the daemon as events:
//   audio:output    the default output device changed ("Galaxy Buds")
//   audio:input     the default microphone changed
//   audio:mic-mute  the microphone was muted or unmuted
// Volume changes are left to GNOME's own on-screen display.
// Nothing is announced for the devices already in use at startup.

import Gvc from 'gi://Gvc';

import * as Volume from 'resource:///org/gnome/shell/ui/status/volume.js';

const SOURCE = {app: 'Sound', integration: 'gnome-audio'};

export class AudioSource {
    /**
     * @param {(input: object) => void} submit - sends one event input
     * @param {() => boolean} micBubbleShown - true while the microphone
     *   bubble already shows the mute state (then no extra popup)
     */
    constructor(submit, micBubbleShown = () => false) {
        this._submit = submit;
        this._micBubbleShown = micBubbleShown;
        this._control = Volume.getMixerControl();
        this._ready = false;
        this._sinkId = null;
        this._sourceId = null;
        this._mic = null;

        this._control.connectObject(
            'state-changed', () => this._onState(),
            'default-sink-changed', (_c, id) => this._onSink(id),
            'default-source-changed', (_c, id) => this._onSource(id),
            this);
        this._onState();
    }

    destroy() {
        this._control.disconnectObject(this);
        this._mic?.disconnectObject(this);
        this._mic = null;
    }

    _onState() {
        if (this._control.get_state() !== Gvc.MixerControlState.READY) {
            this._ready = false;
            return;
        }
        if (this._ready)
            return;
        // Remember what is in use now without announcing it.
        this._sinkId = this._control.get_default_sink()?.id ?? null;
        this._sourceId = this._control.get_default_source()?.id ?? null;
        this._watchMic(this._control.get_default_source());
        this._ready = true;
    }

    _describe(stream) {
        const device = this._control.lookup_device_from_stream(stream);
        const name = device?.description || stream.description || stream.name || 'Audio device';
        return String(name).slice(0, 200);
    }

    _onSink(id) {
        if (!this._ready || id === this._sinkId)
            return;
        this._sinkId = id;
        const stream = this._control.lookup_stream_id(id);
        if (stream)
            this._send('audio:output', this._describe(stream), 'Audio output', 4000);
    }

    _onSource(id) {
        if (!this._ready || id === this._sourceId)
            return;
        this._sourceId = id;
        const stream = this._control.lookup_stream_id(id);
        this._watchMic(stream);
        if (stream)
            this._send('audio:input', this._describe(stream), 'Microphone', 4000);
    }

    _watchMic(stream) {
        this._mic?.disconnectObject(this);
        this._mic = stream ?? null;
        this._mic?.connectObject('notify::is-muted', () => {
            if (this._micBubbleShown())
                return;
            const muted = this._mic.is_muted;
            this._send('audio:mic-mute', muted ? 'Microphone muted' : 'Microphone on',
                this._describe(this._mic), 3000);
        }, this);
    }

    _send(id, title, detail, ttl) {
        this._submit({
            id,
            source: SOURCE,
            category: 'device',
            state: 'started',
            ttl_ms: ttl,
            payload: {type: 'system', title, detail, severity: 'info'},
        });
    }
}
