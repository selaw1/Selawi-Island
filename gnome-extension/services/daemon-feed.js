// Feed backed by the Selawi Island daemon (com.selawi.Island on the
// session bus). Same interface as SimulatedFeed; see docs/ARCHITECTURE.md.
//
// The daemon owns ordering, expiry and validation. This feed only mirrors
// the list it sends, validating it again before the UI sees it.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {EventEmitter} from 'resource:///org/gnome/shell/misc/signals.js';

import {parseEventList} from './protocol.js';

const BUS_NAME = 'com.selawi.Island';
const OBJECT_PATH = '/com/selawi/Island';
const EVENTS_IFACE = 'com.selawi.Island.Events1';
const INTEGRATIONS_IFACE = 'com.selawi.Island.Integrations1';
const CALL_TIMEOUT_MS = 5000;

export class DaemonFeed extends EventEmitter {
    constructor(logger) {
        super();
        this._log = logger;
        this._events = [];
        this._integrations = [];
        this._connected = false;
        this._cancellable = new Gio.Cancellable();
        this._bus = Gio.DBus.session;

        this._signalId = this._bus.signal_subscribe(
            BUS_NAME, EVENTS_IFACE, 'Changed', OBJECT_PATH, null,
            Gio.DBusSignalFlags.NONE,
            (_conn, _sender, _path, _iface, _signal, params) => {
                const [json] = params.deepUnpack();
                this._setList(json);
            });

        // Buttons pressed on events that the extension itself submitted
        // (e.g. Mute on the microphone event) come back through this.
        this._actionSignalId = this._bus.signal_subscribe(
            BUS_NAME, EVENTS_IFACE, 'ActionInvoked', OBJECT_PATH, null,
            Gio.DBusSignalFlags.NONE,
            (_conn, _sender, _path, _iface, _signal, params) => {
                const [id, actionId] = params.deepUnpack();
                if (typeof id === 'string' && typeof actionId === 'string')
                    this.emit('action-invoked', id, actionId);
            });

        this._integrationsSignalId = this._bus.signal_subscribe(
            BUS_NAME, INTEGRATIONS_IFACE, 'Changed', OBJECT_PATH, null,
            Gio.DBusSignalFlags.NONE,
            (_conn, _sender, _path, _iface, _signal, params) => {
                const [json] = params.deepUnpack();
                this._setIntegrations(json);
            });

        // Follows the daemon starting, stopping and restarting.
        this._watchId = Gio.bus_watch_name(
            Gio.BusType.SESSION, BUS_NAME, Gio.BusNameWatcherFlags.NONE,
            () => this._onAppeared(),
            () => this._onVanished());
    }

    /** True while the daemon is on the bus. */
    get connected() {
        return this._connected;
    }

    events() {
        return this._events;
    }

    /** Whether the daemon's integration `id` is currently active. */
    integrationActive(id) {
        return this._connected && this._integrations.some(i => i.id === id && i.status === 'active');
    }

    /** Sends one event input (an object) to the daemon. */
    submit(input) {
        if (this._connected)
            this._call('Submit', new GLib.Variant('(s)', [JSON.stringify(input)]));
    }

    dismiss(id) {
        this._call('Dismiss', new GLib.Variant('(s)', [id]));
    }

    invokeAction(id, actionId) {
        this._call('InvokeAction', new GLib.Variant('(ss)', [id, actionId]));
    }

    destroy() {
        this._cancellable.cancel();
        for (const key of ['_signalId', '_integrationsSignalId', '_actionSignalId']) {
            if (this[key]) {
                this._bus.signal_unsubscribe(this[key]);
                this[key] = 0;
            }
        }
        if (this._watchId) {
            Gio.bus_unwatch_name(this._watchId);
            this._watchId = 0;
        }
    }

    _onAppeared() {
        this._connected = true;
        // Lets extension-side sources send their current state again.
        this.emit('connected');
        this._call('List', null, reply => {
            const [json] = reply.deepUnpack();
            this._setList(json);
        });
        this._call('List', null, reply => {
            const [json] = reply.deepUnpack();
            this._setIntegrations(json);
        }, INTEGRATIONS_IFACE);
    }

    _onVanished() {
        if (this._connected)
            this._log.debug('Selawi Island daemon left the bus');
        this._connected = false;
        this._integrations = [];
        this.emit('integrations-changed');
        this._setEvents([]);
    }

    _setIntegrations(json) {
        try {
            const list = JSON.parse(json);
            if (!Array.isArray(list))
                return;
            this._integrations = list.filter(i =>
                i && typeof i.id === 'string' && typeof i.status === 'string');
        } catch {
            return;
        }
        this.emit('integrations-changed');
    }

    _setList(json) {
        if (typeof json !== 'string')
            return;
        const {events, errors} = parseEventList(json);
        for (const error of errors)
            this._log.warn(`Ignored invalid event from daemon: ${error}`);
        this._setEvents(events);
    }

    _setEvents(events) {
        this._events = events;
        this.emit('changed');
    }

    _call(method, params, onReply = null, iface = EVENTS_IFACE) {
        this._bus.call(BUS_NAME, OBJECT_PATH, iface, method, params, null,
            Gio.DBusCallFlags.NONE, CALL_TIMEOUT_MS, this._cancellable,
            (bus, res) => {
                try {
                    const reply = bus.call_finish(res);
                    onReply?.(reply);
                } catch (e) {
                    if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        this._log.warn(`Daemon call ${method} failed: ${e.message}`);
                }
            });
    }
}
