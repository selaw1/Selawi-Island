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
const CALL_TIMEOUT_MS = 5000;

export class DaemonFeed extends EventEmitter {
    constructor(logger) {
        super();
        this._log = logger;
        this._events = [];
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

    dismiss(id) {
        this._call('Dismiss', new GLib.Variant('(s)', [id]));
    }

    invokeAction(id, actionId) {
        this._call('InvokeAction', new GLib.Variant('(ss)', [id, actionId]));
    }

    destroy() {
        this._cancellable.cancel();
        if (this._signalId) {
            this._bus.signal_unsubscribe(this._signalId);
            this._signalId = 0;
        }
        if (this._watchId) {
            Gio.bus_unwatch_name(this._watchId);
            this._watchId = 0;
        }
    }

    _onAppeared() {
        this._connected = true;
        this._call('List', null, reply => {
            const [json] = reply.deepUnpack();
            this._setList(json);
        });
    }

    _onVanished() {
        if (this._connected)
            this._log.debug('Selawi Island daemon left the bus');
        this._connected = false;
        this._setEvents([]);
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

    _call(method, params, onReply = null) {
        this._bus.call(BUS_NAME, OBJECT_PATH, EVENTS_IFACE, method, params, null,
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
