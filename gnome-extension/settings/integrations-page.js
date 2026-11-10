// The "Integrations" settings page (shared by the settings app and the
// extension's preferences window). Daemon settings are read and
// changed over D-Bus (com.selawi.Island.Configuration1); extension-only
// settings (audio, GNOME banners) are GSettings.

import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk';

const BUS_NAME = 'com.selawi.Island';
const PATH = '/com/selawi/Island';
const CONFIG_IFACE = 'com.selawi.Island.Configuration1';
const INTEGRATIONS_IFACE = 'com.selawi.Island.Integrations1';
const PLUGINS_IFACE = 'com.selawi.Island.Plugins1';

const SOURCES = [
    ['mpris', 'Media players', 'Spotify, browsers, VLC and other players'],
    ['upower', 'Battery', 'Charger, full, low and critical battery'],
    ['bluez', 'Bluetooth', 'Devices connecting and disconnecting'],
    ['notifications', 'Notifications', 'App notifications'],
    ['network', 'Network', 'Wi-Fi, Ethernet, VPN and network sign-in'],
    ['storage', 'USB drives', 'Drives plugged in and removed'],
    ['health', 'System warnings', 'Low memory and high CPU temperature (checks every 15 s)'],
    ['pacman', 'System updates', 'Installs, updates and removals with pacman or yay'],
];

function call(iface, method, args = null) {
    return new Promise((resolve, reject) => {
        Gio.DBus.session.call(BUS_NAME, PATH, iface, method, args, null,
            Gio.DBusCallFlags.NO_AUTO_START, 3000, null, (bus, result) => {
                try {
                    resolve(bus.call_finish(result).deepUnpack());
                } catch (e) {
                    reject(e);
                }
            });
    });
}

function statusText(info) {
    if (!info)
        return '';
    const status = {active: 'Active', unavailable: 'Unavailable', failed: 'Failed',
        disabled: 'Off', starting: 'Starting'}[info.status] ?? info.status;
    return info.detail ? `${status} — ${info.detail}` : status;
}

export function integrationsPage(settings, window) {
    const page = new Adw.PreferencesPage({title: 'Integrations', icon_name: 'application-x-addon-symbolic'});
    const cleanups = [];
    window.connect('close-request', () => cleanups.forEach(fn => fn()));

    let config = null;
    let integrations = [];
    let players = [];
    let syncing = false;

    // ---- service status
    const serviceGroup = new Adw.PreferencesGroup();
    const serviceRow = new Adw.ActionRow({title: 'Selawi Island service'});
    serviceGroup.add(serviceRow);
    page.add(serviceGroup);

    // ---- sources
    const sources = new Adw.PreferencesGroup({
        title: 'Sources',
        description: 'What the island watches. Turning one off removes its events right away.',
    });
    const switches = new Map();
    for (const [id, title, subtitle] of SOURCES) {
        const row = new Adw.SwitchRow({title, subtitle});
        row.connect('notify::active', () => {
            if (syncing || !config)
                return;
            const next = JSON.parse(JSON.stringify(config));
            next.integrations = {...next.integrations, [id]: row.active};
            setConfig(next);
        });
        switches.set(id, row);
        sources.add(row);
    }
    const audioRow = new Adw.SwitchRow({
        title: 'Audio devices',
        subtitle: 'Output device, microphone and mute changes',
    });
    settings.bind('audio-events', audioRow, 'active', Gio.SettingsBindFlags.DEFAULT);
    sources.add(audioRow);
    const privacyRow = new Adw.SwitchRow({
        title: 'Microphone and camera in use',
        subtitle: 'Orange for the microphone, green when the camera is on',
    });
    settings.bind('privacy-events', privacyRow, 'active', Gio.SettingsBindFlags.DEFAULT);
    sources.add(privacyRow);
    const wavesRow = new Adw.SwitchRow({
        title: 'Voice waves',
        subtitle: 'Bars that move when you talk. Measures loudness only, while another app already uses the microphone',
    });
    settings.bind('voice-waves', wavesRow, 'active', Gio.SettingsBindFlags.DEFAULT);
    settings.bind('privacy-events', wavesRow, 'sensitive', Gio.SettingsBindFlags.GET);
    sources.add(wavesRow);
    const clipboardRow = new Adw.SwitchRow({
        title: 'Copied',
        subtitle: 'A short bubble when you copy text, an image or files. Never shows what you copied',
    });
    settings.bind('clipboard-events', clipboardRow, 'active', Gio.SettingsBindFlags.DEFAULT);
    sources.add(clipboardRow);
    page.add(sources);

    // ---- media
    const media = new Adw.PreferencesGroup({
        title: 'Media',
        description: 'When several players are open at the same time.',
    });
    const modeRow = new Adw.ComboRow({
        title: 'Players to show',
        model: Gtk.StringList.new(['All players', 'One player']),
    });
    const preferredRow = new Adw.ComboRow({
        title: 'Preferred player',
        subtitle: 'Shown first whenever it is playing',
        model: new Gtk.StringList(),
    });
    let preferredKeys = [null];
    modeRow.connect('notify::selected', () => {
        if (syncing || !config)
            return;
        const next = JSON.parse(JSON.stringify(config));
        next.media.mode = modeRow.selected === 1 ? 'one' : 'all';
        setConfig(next);
    });
    preferredRow.connect('notify::selected', () => {
        if (syncing || !config)
            return;
        const next = JSON.parse(JSON.stringify(config));
        const key = preferredKeys[preferredRow.selected] ?? null;
        if (key)
            next.media.preferred_player = key;
        else
            delete next.media.preferred_player;
        setConfig(next);
    });
    media.add(modeRow);
    media.add(preferredRow);
    page.add(media);

    // ---- notifications
    const notifications = new Adw.PreferencesGroup({title: 'Notifications'});
    const bodyRow = new Adw.SwitchRow({
        title: 'Show notification text',
        subtitle: 'Off: only the title and app are shown',
    });
    bodyRow.connect('notify::active', () => {
        if (syncing || !config)
            return;
        const next = JSON.parse(JSON.stringify(config));
        next.notifications.show_body = bodyRow.active;
        setConfig(next);
    });
    const bannersRow = new Adw.SwitchRow({
        title: "Hide GNOME's notification banners",
        subtitle: 'So each notification appears once. Only while the island shows notifications.',
    });
    settings.bind('hide-gnome-banners', bannersRow, 'active', Gio.SettingsBindFlags.DEFAULT);
    notifications.add(bodyRow);
    notifications.add(bannersRow);
    page.add(notifications);

    // ---- apps (plugins)
    const apps = new Adw.PreferencesGroup({
        title: 'Apps',
        description: 'Apps that show their own events on the island. New apps ask first; they show nothing until allowed.',
    });
    const noApps = new Adw.ActionRow({title: 'No apps have asked yet'});
    apps.add(noApps);
    page.add(apps);
    let appRows = [];
    let plugins = [];

    function appSubtitle(plugin) {
        const parts = [];
        if (plugin.description)
            parts.push(plugin.description);
        if (!plugin.decided)
            parts.push('Waiting for your answer');
        else if (!plugin.running)
            parts.push('Not running');
        return parts.join(' · ');
    }

    function renderApps() {
        for (const row of appRows)
            apps.remove(row);
        appRows = plugins.map(plugin => {
            const row = new Adw.SwitchRow({title: plugin.name, subtitle: appSubtitle(plugin), active: plugin.allowed});
            row.connect('notify::active', () => {
                if (syncing || !config)
                    return;
                const next = JSON.parse(JSON.stringify(config));
                next.plugins = {...next.plugins ?? {}, [plugin.id]: row.active};
                setConfig(next);
            });
            apps.add(row);
            return row;
        });
        noApps.visible = appRows.length === 0;
    }

    // ---- syncing with the daemon
    const daemonRows = [...switches.values(), modeRow, preferredRow, bodyRow];

    function render() {
        syncing = true;
        const running = config !== null;
        serviceRow.subtitle = running
            ? 'Running'
            : 'Not running. Start it with: systemctl --user start selawi-islandd';
        for (const row of daemonRows)
            row.sensitive = running;
        if (running) {
            for (const [id, row] of switches) {
                row.active = config.integrations?.[id] ?? id !== 'health';
                const [, , description] = SOURCES.find(([sid]) => sid === id);
                const status = statusText(integrations.find(i => i.id === id));
                row.subtitle = status ? `${description} · ${status}` : description;
            }
            renderApps();
            modeRow.selected = config.media?.mode === 'one' ? 1 : 0;
            const preferred = config.media?.preferred_player ?? null;
            const known = new Map(players.map(p => [p.id, p.name]));
            if (preferred && !known.has(preferred))
                known.set(preferred, `${preferred} (not running)`);
            preferredKeys = [null, ...known.keys()];
            preferredRow.model = Gtk.StringList.new(['Automatic (most recently started)', ...known.values()]);
            preferredRow.selected = Math.max(0, preferredKeys.indexOf(preferred));
            preferredRow.sensitive = config.media?.mode === 'one';
            bodyRow.active = config.notifications?.show_body ?? true;
        }
        syncing = false;
    }

    async function refresh() {
        try {
            const [configJson] = await call(CONFIG_IFACE, 'Get');
            const [listJson] = await call(INTEGRATIONS_IFACE, 'List');
            const [playersJson] = await call(INTEGRATIONS_IFACE, 'MediaPlayers');
            config = JSON.parse(configJson);
            config.integrations ??= {};
            config.media ??= {mode: 'all'};
            config.notifications ??= {show_body: true};
            integrations = JSON.parse(listJson);
            players = JSON.parse(playersJson);
        } catch {
            config = null;
        }
        try {
            const [pluginsJson] = await call(PLUGINS_IFACE, 'List');
            plugins = JSON.parse(pluginsJson);
        } catch {
            plugins = [];
        }
        render();
    }

    async function setConfig(next) {
        try {
            await call(CONFIG_IFACE, 'Set', new GLib.Variant('(s)', [JSON.stringify(next)]));
        } catch (e) {
            console.warn(`Selawi Island: could not change settings: ${e.message}`);
        }
        await refresh();
    }

    const bus = Gio.DBus.session;
    const subscriptions = [CONFIG_IFACE, INTEGRATIONS_IFACE, PLUGINS_IFACE].map(iface =>
        bus.signal_subscribe(BUS_NAME, iface, 'Changed', PATH, null,
            Gio.DBusSignalFlags.NONE, () => refresh()));
    const watchId = Gio.bus_watch_name(Gio.BusType.SESSION, BUS_NAME,
        Gio.BusNameWatcherFlags.NONE, () => refresh(), () => {
            config = null;
            render();
        });
    cleanups.push(() => {
        subscriptions.forEach(id => bus.signal_unsubscribe(id));
        Gio.bus_unwatch_name(watchId);
    });

    render();
    return page;
}
