// The "Status" settings page: is the Selawi Island service running (with a
// Restart button), is the GNOME extension on or in error (with a Turn on
// button), what each source is doing, and "Copy report" for asking for
// help. Uses only D-Bus: the service itself, GNOME Shell's extension
// interface, and the systemd user manager to restart the service.

import Adw from 'gi://Adw';
import Gdk from 'gi://Gdk';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk';

import {buildReport, extensionStatus, integrationText, integrationTitle, osName} from './report.js';

const UUID = 'selawi-island@selawi';
const ISLAND = ['com.selawi.Island', '/com/selawi/Island'];
const SHELL = ['org.gnome.Shell', '/org/gnome/Shell'];
const SYSTEMD = ['org.freedesktop.systemd1', '/org/freedesktop/systemd1'];
const SERVICE_UNIT = 'selawi-islandd.service';

function call([name, path], iface, method, params = null, autoStart = false) {
    return new Promise((resolve, reject) => {
        Gio.DBus.session.call(name, path, iface, method, params, null,
            autoStart ? Gio.DBusCallFlags.NONE : Gio.DBusCallFlags.NO_AUTO_START, 3000, null,
            (bus, result) => {
                try {
                    resolve(bus.call_finish(result).deepUnpack());
                } catch (e) {
                    reject(e);
                }
            });
    });
}

async function property(target, iface, name) {
    const [value] = await call(target, 'org.freedesktop.DBus.Properties', 'Get',
        new GLib.Variant('(ss)', [iface, name]));
    return value.deepUnpack();
}

/** Everything the page shows, read fresh. Nothing here fails. */
async function readState() {
    const state = {daemon: null, integrations: [], extension: null, extensionErrors: [], shellVersion: null};
    try {
        const version = await property(ISLAND, 'com.selawi.Island.Core1', 'Version');
        const protocol = await property(ISLAND, 'com.selawi.Island.Core1', 'ProtocolVersion');
        const [list] = await call(ISLAND, 'com.selawi.Island.Events1', 'List');
        state.daemon = {version, protocol, events: JSON.parse(list).length};
        const [integrations] = await call(ISLAND, 'com.selawi.Island.Integrations1', 'List');
        state.integrations = JSON.parse(integrations);
    } catch {
        // Not running.
    }
    try {
        const [info] = await call(SHELL, 'org.gnome.Shell.Extensions', 'GetExtensionInfo',
            new GLib.Variant('(s)', [UUID]));
        state.extension = Object.fromEntries(Object.entries(info).map(([k, v]) => [k, v?.deepUnpack?.() ?? v]));
        if (Number(state.extension.state) === 3) {
            [state.extensionErrors] = await call(SHELL, 'org.gnome.Shell.Extensions', 'GetExtensionErrors',
                new GLib.Variant('(s)', [UUID]));
        }
        state.shellVersion = await property(SHELL, 'org.gnome.Shell', 'ShellVersion');
    } catch {
        // GNOME Shell is not running (e.g. another desktop).
    }
    return state;
}

function readOsName() {
    try {
        const [, bytes] = GLib.file_get_contents('/etc/os-release');
        return osName(new TextDecoder().decode(bytes));
    } catch {
        return 'Unknown';
    }
}

/** @param {Gtk.Window} window - timers and signals are removed when it closes */
export function statusPage(window) {
    const page = new Adw.PreferencesPage({title: 'Status', icon_name: 'utilities-system-monitor-symbolic'});
    let latest = null;

    // ---- service
    const serviceGroup = new Adw.PreferencesGroup({title: 'Selawi Island service'});
    const serviceRow = new Adw.ActionRow({title: 'Service'});
    const restart = new Gtk.Button({label: 'Restart', valign: Gtk.Align.CENTER});
    serviceRow.add_suffix(restart);
    serviceGroup.add(serviceRow);
    page.add(serviceGroup);

    // ---- extension
    const extensionGroup = new Adw.PreferencesGroup({title: 'GNOME extension'});
    const extensionRow = new Adw.ActionRow({title: 'Island'});
    const turnOn = new Gtk.Button({label: 'Turn on', valign: Gtk.Align.CENTER, visible: false});
    extensionRow.add_suffix(turnOn);
    extensionGroup.add(extensionRow);
    page.add(extensionGroup);

    // ---- sources
    const sourcesGroup = new Adw.PreferencesGroup({
        title: 'Sources',
        description: 'What each source of the service is doing right now',
    });
    let sourceRows = [];
    page.add(sourcesGroup);

    // ---- report
    const reportGroup = new Adw.PreferencesGroup({title: 'Help'});
    const reportRow = new Adw.ActionRow({
        title: 'Copy report',
        subtitle: 'Versions and states, to paste when asking for help. Never your events, notifications or network names.',
    });
    const copy = new Gtk.Button({label: 'Copy', valign: Gtk.Align.CENTER});
    reportRow.add_suffix(copy);
    reportGroup.add(reportRow);
    page.add(reportGroup);

    function render(state) {
        latest = state;
        serviceRow.subtitle = state.daemon
            ? `Running · version ${state.daemon.version} · ${state.daemon.events} event(s)`
            : 'Not running';
        restart.label = state.daemon ? 'Restart' : 'Start';
        restart.sensitive = true;

        const ext = extensionStatus(state.extension);
        extensionRow.subtitle = ext.text;
        turnOn.visible = ext.canTurnOn;

        for (const row of sourceRows)
            sourcesGroup.remove(row);
        sourceRows = state.integrations.map(info => {
            const row = new Adw.ActionRow({title: integrationTitle(info), subtitle: integrationText(info)});
            sourcesGroup.add(row);
            return row;
        });
        sourcesGroup.visible = sourceRows.length > 0;
    }

    let refreshing = false;
    async function refresh() {
        if (refreshing)
            return;
        refreshing = true;
        try {
            render(await readState());
        } finally {
            refreshing = false;
        }
    }

    restart.connect('clicked', async () => {
        restart.sensitive = false;
        serviceRow.subtitle = 'Restarting…';
        try {
            await call(SYSTEMD, 'org.freedesktop.systemd1.Manager', 'RestartUnit',
                new GLib.Variant('(ss)', [SERVICE_UNIT, 'replace']), true);
        } catch (e) {
            serviceRow.subtitle = `Could not restart: ${e.message}`;
            restart.sensitive = true;
            return;
        }
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 1500, () => {
            refresh();
            return GLib.SOURCE_REMOVE;
        });
    });

    turnOn.connect('clicked', async () => {
        try {
            await call(SHELL, 'org.gnome.Shell.Extensions', 'EnableExtension', new GLib.Variant('(s)', [UUID]));
        } catch (e) {
            extensionRow.subtitle = `Could not turn on: ${e.message}`;
        }
        refresh();
    });

    copy.connect('clicked', async () => {
        const state = latest ?? await readState();
        const report = buildReport({...state, os: readOsName()});
        Gdk.Display.get_default()?.get_clipboard().set(report);
        copy.label = 'Copied';
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 2000, () => {
            copy.label = 'Copy';
            return GLib.SOURCE_REMOVE;
        });
    });

    // Follow changes as they happen: the service appearing or going, its
    // sources changing, the extension changing state.
    const bus = Gio.DBus.session;
    const subscriptions = [
        bus.signal_subscribe(ISLAND[0], 'com.selawi.Island.Integrations1', 'Changed', ISLAND[1], null,
            Gio.DBusSignalFlags.NONE, () => refresh()),
        bus.signal_subscribe(SHELL[0], 'org.gnome.Shell.Extensions', 'ExtensionStateChanged', SHELL[1], null,
            Gio.DBusSignalFlags.NONE, () => refresh()),
    ];
    const watchId = Gio.bus_watch_name(Gio.BusType.SESSION, ISLAND[0], Gio.BusNameWatcherFlags.NONE,
        () => refresh(), () => refresh());
    window.connect('close-request', () => {
        subscriptions.forEach(id => bus.signal_unsubscribe(id));
        Gio.bus_unwatch_name(watchId);
    });

    render({daemon: null, integrations: [], extension: null, extensionErrors: [], shellVersion: null});
    refresh();
    return page;
}
