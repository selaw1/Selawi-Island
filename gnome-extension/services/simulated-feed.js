// Plays data/demo-scenario.json into an EventStore so the island can be
// developed and tested without any real integration.
//
// Feed interface (the island depends only on this):
//   events()                    ordered list of stored events
//   dismiss(id)                 user dismissed an event
//   invokeAction(id, actionId)  user pressed an action button
//   'changed' signal            the list changed
//   destroy()
//
// A daemon-backed feed with the same interface replaces this in Phase 3.

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';

import {EventEmitter} from 'resource:///org/gnome/shell/misc/signals.js';

import {EventStore} from './event-store.js';
import {PROTOCOL_VERSION} from './protocol.js';
import {mediaPosition} from '../ui/view-model.js';

Gio._promisify(Gio.File.prototype, 'load_contents_async');

export class SimulatedFeed extends EventEmitter {
    constructor(scenarioFile, logger) {
        super();
        this._file = scenarioFile;
        this._log = logger;
        this._store = new EventStore();
        this._steps = [];
        this._looping = false;
        this._index = 0;
        this._stepTimer = 0;
        this._deadlineTimer = 0;
        this._destroyed = false;
        this._cancellable = new Gio.Cancellable();
    }

    async start() {
        try {
            const [bytes] = await this._file.load_contents_async(this._cancellable);
            const scenario = JSON.parse(new TextDecoder().decode(bytes));
            if (scenario.protocol_version !== PROTOCOL_VERSION)
                throw new Error(`unsupported protocol version ${scenario.protocol_version}`);
            if (!Array.isArray(scenario.steps))
                throw new Error('scenario has no steps');
            this._steps = scenario.steps;
            this._looping = scenario.loop === true;
        } catch (e) {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                this._log.error(`Could not load demo scenario: ${e.message}`);
            return;
        }
        if (!this._destroyed)
            this._scheduleStep();
    }

    events() {
        return this._store.ordered();
    }

    dismiss(id) {
        if (this._store.dismiss(id))
            this._changed();
    }

    invokeAction(id, actionId) {
        const event = this._store.get(id);
        if (!event)
            return;
        const now = Date.now();
        const next = {
            id: event.id,
            source: event.source,
            category: event.category,
            state: 'updated',
            persistent: event.persistent,
            payload: {...event.payload},
            actions: event.actions,
        };
        switch (actionId) {
        case 'play_pause': {
            const pos = mediaPosition(event, now);
            next.payload.playing = !event.payload.playing;
            if (pos !== null)
                next.payload.position_ms = Math.round(pos);
            next.actions = [{id: 'play_pause', label: next.payload.playing ? 'Pause' : 'Play'}];
            break;
        }
        case 'end':
            next.state = 'ended';
            next.actions = [];
            break;
        case 'cancel':
            next.state = 'failed';
            next.payload.detail = 'Cancelled';
            next.actions = [];
            break;
        default:
            this._log.warn(`Simulator has no handler for action "${actionId}"`);
            return;
        }
        this._apply(next, now);
    }

    destroy() {
        this._destroyed = true;
        this._cancellable.cancel();
        if (this._stepTimer)
            GLib.source_remove(this._stepTimer);
        if (this._deadlineTimer)
            GLib.source_remove(this._deadlineTimer);
        this._stepTimer = 0;
        this._deadlineTimer = 0;
    }

    _scheduleStep() {
        if (this._index >= this._steps.length) {
            if (!this._looping || this._steps.length === 0)
                return;
            this._index = 0;
        }
        const step = this._steps[this._index];
        const delay = Number.isInteger(step?.delay_ms) && step.delay_ms >= 0 ? step.delay_ms : 1000;
        this._stepTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, delay, () => {
            this._stepTimer = 0;
            this._index++;
            this._apply(step?.event, Date.now());
            this._scheduleStep();
            return GLib.SOURCE_REMOVE;
        });
    }

    _apply(raw, now) {
        this._store.tick(now);
        const result = this._store.apply(raw, now);
        // "Already finished" is expected after the user pressed Leave or
        // Cancel while the scenario keeps playing.
        if (!result.ok && result.error === 'event has already finished')
            this._log.debug(`Ignored simulated update: ${result.error}`);
        else if (!result.ok)
            this._log.warn(`Rejected simulated event: ${result.error}`);
        this._changed();
    }

    // Emits 'changed' and arms one timer for the next expiry. No polling.
    _changed() {
        if (this._deadlineTimer) {
            GLib.source_remove(this._deadlineTimer);
            this._deadlineTimer = 0;
        }
        const deadline = this._store.nextDeadline();
        if (deadline !== null) {
            const wait = Math.max(0, deadline - Date.now());
            this._deadlineTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, wait, () => {
                this._deadlineTimer = 0;
                this._store.tick(Date.now());
                this._changed();
                return GLib.SOURCE_REMOVE;
            });
        }
        this.emit('changed');
    }
}
