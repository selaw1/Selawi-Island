// Opens the settings app's window (GTK 4 + libadwaita) and checks its pages,
// the Status page against a running daemon, and that "Copy report" holds
// versions and states but none of the user's events.
// Run by tests/integration/prefs-check.sh.
import Adw from 'gi://Adw?version=1';
import Gdk from 'gi://Gdk?version=4.0';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk?version=4.0';
import System from 'system';

const loop = new GLib.MainLoop(null, false);
let failed = false;
main().catch(e => {
    printerr(`FAIL: ${e.message}`);
    failed = true;
}).finally(() => loop.quit());
loop.run();
System.exit(failed ? 1 : 0);

async function main() {
    const root = GLib.get_current_dir();
    const {buildWindow, extensionSettings, loadPages} = await import(`file://${root}/settings-app/window.js`);
    Adw.init();
    const ok = (c, m) => {
        if (!c)
            throw new Error(m);
        print(`ok: ${m}`);
    };
    const sleep = ms => new Promise(r => GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
        r();
        return GLib.SOURCE_REMOVE;
    }));
    const walk = w => {
        const all = [];
        const step = x => {
            for (let c = x.get_first_child(); c; c = c.get_next_sibling()) {
                all.push(c);
                step(c);
            }
        };
        step(w);
        return all;
    };

    // Something private on the island, which the report must not contain.
    Gio.DBus.session.call_sync('com.selawi.Island', '/com/selawi/Island', 'com.selawi.Island.Events1', 'Submit',
        new GLib.Variant('(s)', [JSON.stringify({id: 'secret', source: {app: 'Mail', integration: 'test'},
            category: 'notification', state: 'started', persistent: true,
            payload: {type: 'notification', title: 'Secret plans', body: 'do not share'}})]),
        null, Gio.DBusCallFlags.NONE, 2000, null);

    const pages = await loadPages();
    const settings = extensionSettings();
    ok(settings !== null, 'finds the extension settings');
    const app = new Adw.Application({application_id: 'com.selawi.Island.SettingsTest'});
    app.register(null);
    const window = buildWindow(app, settings, pages);
    window.present();
    await sleep(1200);
    let all = walk(window);
    const stack = all.find(w => w instanceof Adw.ViewStack);
    const names = [];
    for (let i = 0; i < stack.pages.get_n_items(); i++)
        names.push(stack.pages.get_item(i).title);
    ok(names.join() === 'Island,Integrations,Status', `three pages: ${names.join(', ')}`);

    stack.visible_child_name = 'status';
    await sleep(800);
    all = walk(stack.get_child_by_name('status'));
    const row = title => all.find(w => w instanceof Adw.ActionRow && w.title === title);
    ok(/^Running · version \d+\.\d+\.\d+ · \d+ event\(s\)$/.test(row('Service')?.subtitle ?? ''),
        `Status sees the running service (${row('Service')?.subtitle})`);
    ok(row('Island')?.subtitle === 'Not installed, or GNOME Shell is not running',
        'no GNOME Shell here: the extension row says so');
    ok(row('Media players') !== undefined && row('System warnings')?.subtitle === 'Off',
        'sources listed with their state');
    const button = label => all.find(w => w instanceof Gtk.Button && w.label === label);
    ok(row('Battery')?.subtitle.startsWith('Unavailable'), 'a source that cannot run here says why');
    ok(button('Restart') !== undefined, 'Restart button present');

    button('Copy').emit('clicked');
    await sleep(500);
    const clipboard = Gdk.Display.get_default().get_clipboard();
    const text = await new Promise((resolve, reject) => clipboard.read_text_async(null, (c, res) => {
        try {
            resolve(c.read_text_finish(res));
        } catch (e) {
            reject(e);
        }
    }));
    ok(/^Selawi Island report/.test(text) && /Service: running, version/.test(text) && /mpris: Active/.test(text),
        'report has versions and states');
    ok(!/Secret|share/.test(text), 'report has none of the events');

    // An app registers as a plugin (on its own connection): it shows under
    // Integrations → Apps, waiting for an answer; switching it on allows it.
    const address = GLib.getenv('DBUS_SESSION_BUS_ADDRESS');
    const pluginBus = Gio.DBusConnection.new_for_address_sync(address,
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, null, null);
    pluginBus.call_sync('com.selawi.Island', '/com/selawi/Island', 'com.selawi.Island.Plugins1', 'Register',
        new GLib.Variant('(s)', [JSON.stringify({manifest_version: 1, id: 'pomodoro', name: 'Pomodoro',
            description: 'Focus timer', categories: ['custom']})]), null, Gio.DBusCallFlags.NONE, 2000, null);
    await sleep(800);
    const integrations = walk(stack.get_child_by_name('integrations'));
    const appRow = integrations.find(w => w instanceof Adw.SwitchRow && w.title === 'Pomodoro');
    ok(appRow && !appRow.active && /Waiting for your answer/.test(appRow.subtitle),
        `a new app is listed, off, waiting (${appRow?.subtitle})`);
    appRow.active = true;
    await sleep(500);
    const config = JSON.parse(Gio.DBus.session.call_sync('com.selawi.Island', '/com/selawi/Island',
        'com.selawi.Island.Configuration1', 'Get', null, null, Gio.DBusCallFlags.NONE, 2000, null).deepUnpack()[0]);
    ok(config.plugins?.pomodoro === true, 'switching it on allows it');
    pluginBus.close_sync(null);

    const settingsLess = buildWindow(app, null, pages);
    all = walk(settingsLess);
    ok(all.some(w => w instanceof Adw.Banner && w.revealed), 'without the extension: a banner and only the status');
    window.close();
    settingsLess.close();
}
