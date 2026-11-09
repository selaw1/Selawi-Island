// The "Integrations" preferences page. Daemon settings are read and
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

const SOURCES = [
    ['mpris', 'Media players', 'Spotify, browsers, VLC and other players'],
    ['upower', 'Battery', 'Charger, full, low and critical battery'],
    ['bluez', 'Bluetooth', 'Devices connecting and disconnecting'],
    ['notifications', 'Notifications', 'App notifications'],
    ['network', 'Network', 'Wi-Fi, Ethernet, VPN and network sign-in'],
    ['storage', 'USB drives', 'Drives plugged in and removed'],
    ['health', 'System warnings', 'Low memory and high CPU temperature (checks every 15 s)'],
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
    const muteRow = new Adw.ComboRow({
        title: 'Mute button mutes',
        subtitle: 'They will not hear you, but the app still shows you as unmuted',
        model: Gtk.StringList.new(['Only the apps using the microphone', 'The whole microphone']),
    });
    const syncMute = () => {
        muteRow.selected = settings.get_string('mute-scope') === 'microphone' ? 1 : 0;
    };
    const muteId = settings.connect('changed::mute-scope', syncMute);
    cleanups.push(() => settings.disconnect(muteId));
    syncMute();
    muteRow.connect('notify::selected', () => {
        const value = muteRow.selected === 1 ? 'microphone' : 'apps';
        if (settings.get_string('mute-scope') !== value)
            settings.set_string('mute-scope', value);
    });
    settings.bind('privacy-events', muteRow, 'sensitive', Gio.SettingsBindFlags.GET);
    sources.add(muteRow);
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
    const subscriptions = [CONFIG_IFACE, INTEGRATIONS_IFACE].map(iface =>
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
