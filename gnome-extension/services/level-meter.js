// Measures microphone loudness for the wave bars. Runs only while
// `active` is true (another app is using the mic and the island is
// showing it). Reads loudness 12 times a second at 8 kHz mono; no sound
// is stored or sent anywhere.
//
// Uses GStreamer (pulsesrc + level from gst-plugins-good), loaded only on
// first use. If it is missing, the bars simply stay still.

import GLib from 'gi://GLib';

import {loudness, parseLevelMessage} from '../ui/level.js';

/** Name of our own recording stream, so the mic watcher can ignore it. */
export const METER_APP_ID = 'selawi-island-levels';
export const METER_APP_NAME = 'Selawi Island';
const INTERVAL_NS = 80_000_000;

export class LevelMeter {
    /**
     * @param {(level: number) => void} onLevel - 0…1, about 12 times a second
     * @param {object} logger
     */
    constructor(onLevel, logger) {
        this._onLevel = onLevel;
        this._log = logger;
        this._active = false;
        this._pipeline = null;
        this._busId = 0;
        this._Gst = null;
        this._unavailable = false;
        this._starting = false;
    }

    set active(active) {
        if (active === this._active)
            return;
        this._active = active;
        if (active)
            this._start().catch(e => this._log.warn(`Wave meter failed: ${e.message}`));
        else
            this._stop();
    }

    destroy() {
        this._active = false;
        this._stop();
    }

    async _loadGst() {
        if (this._Gst || this._unavailable)
            return this._Gst;
        try {
            const {default: Gst} = await import('gi://Gst?version=1.0');
            if (!Gst.is_initialized())
                Gst.init(null);
            if (!Gst.ElementFactory.find('pulsesrc') || !Gst.ElementFactory.find('level'))
                throw new Error('GStreamer elements pulsesrc/level not installed (gst-plugins-good)');
            this._Gst = Gst;
        } catch (e) {
            this._unavailable = true;
            this._log.warn(`Voice waves unavailable: ${e.message}`);
        }
        return this._Gst;
    }

    async _start() {
        if (this._pipeline || this._starting)
            return;
        this._starting = true;
        const Gst = await this._loadGst();
        this._starting = false;
        if (!Gst || !this._active || this._pipeline)
            return;

        const pipeline = Gst.parse_launch(
            'pulsesrc name=src ! audioconvert ! audioresample ! ' +
            'audio/x-raw,rate=8000,channels=1 ! ' +
            `level interval=${INTERVAL_NS} post-messages=true ! fakesink sync=false`);
        const src = pipeline.get_by_name('src');
        src.client_name = METER_APP_NAME;
        src.stream_properties = Gst.Structure.from_string(
            `props,application.id=(string)${METER_APP_ID},application.name=(string)"${METER_APP_NAME}",media.role=(string)production`)[0];

        const bus = pipeline.get_bus();
        bus.add_signal_watch();
        this._busId = bus.connect('message', (_bus, message) => {
            if (message.type === Gst.MessageType.ELEMENT) {
                const structure = message.get_structure();
                if (structure?.get_name() === 'level') {
                    const db = parseLevelMessage(structure.to_string());
                    if (db !== null)
                        this._onLevel(loudness(db));
                }
            } else if (message.type === Gst.MessageType.ERROR) {
                const [error] = message.parse_error();
                this._log.warn(`Voice waves stopped: ${error.message}`);
                this._stop();
            }
        });
        this._pipeline = pipeline;
        pipeline.set_state(Gst.State.PLAYING);
    }

    _stop() {
        if (!this._pipeline)
            return;
        const bus = this._pipeline.get_bus();
        if (this._busId)
            bus.disconnect(this._busId);
        this._busId = 0;
        bus.remove_signal_watch();
        this._pipeline.set_state(this._Gst.State.NULL);
        this._pipeline = null;
        // Let the bars settle.
        GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._onLevel(0);
            return GLib.SOURCE_REMOVE;
        });
    }
}
