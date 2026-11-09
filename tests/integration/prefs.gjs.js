// Opens the real preferences window (GTK4 + libadwaita) and checks the
// Position controls and the Integrations page against a running daemon.
// Run by tests/integration/prefs-check.sh.
import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk?version=4.0';
import System from 'system';
// Everything runs inside one async function driven by a GLib main loop:
// GJS does not run promise continuations for a main loop nested inside a
// module that is still waiting on a top-level await.
const loop = new GLib.MainLoop(null, false);
let failed = false;
main().catch(e => {
    printerr(`FAIL: ${e.message}`);
    failed = true;
}).finally(() => loop.quit());
loop.run();
System.exit(failed ? 1 : 0);

async function main() {
    const {default: SelawiIslandPreferences} = await import(`file://${ARGV[0]}/prefs.js`);
    Adw.init();
    const prefs = new SelawiIslandPreferences();
    const settings = prefs.getSettings();
    const window = new Adw.PreferencesWindow();
    await prefs.fillPreferencesWindow(window);
    const all = [];
    const walk = w => { for (let c = w.get_first_child(); c; c = c.get_next_sibling()) { all.push(c); walk(c); } };
    walk(window);
    const btn = label => all.find(w => w instanceof Gtk.Button && w.label === label);
    const byTitle = (type, title) => all.find(w => w instanceof type && w.title === title);
    const ok = (c, m) => {
        if (!c)
            throw new Error(m);
        print(`ok: ${m}`);
    };
    const sleep = ms => new Promise(r => GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
        r();
        return GLib.SOURCE_REMOVE;
    }));
    const daemonConfig = () => {
        const reply = Gio.DBus.session.call_sync('com.selawi.Island', '/com/selawi/Island',
            'com.selawi.Island.Configuration1', 'Get', null, null, Gio.DBusCallFlags.NONE, 2000, null);
        return JSON.parse(reply.deepUnpack()[0]);
    };
    const combo = byTitle(Adw.ComboRow, 'Default position');
    ok(combo && combo.selected === 0, 'combo shows top-center');
    combo.selected = 6;
    ok(settings.get_string('default-anchor') === 'bottom-right', 'choosing Bottom right saves bottom-right');
    settings.set_string('default-anchor', 'center');
    ok(combo.selected === 3, 'setting change updates the combo');
    ok(!btn('Make this the default').sensitive && !btn('Reset').sensitive, 'buttons disabled at default position');
    settings.set_double('position-x', 0.3); settings.set_double('position-y', 0.7); settings.set_boolean('custom-position', true);
    ok(btn('Make this the default').sensitive, 'buttons enabled after a drag');
    btn('Make this the default').emit('clicked');
    ok(settings.get_string('default-anchor') === 'custom' && settings.get_double('default-x') === 0.3 &&
       settings.get_double('default-y') === 0.7 && !settings.get_boolean('custom-position'), 'Make this the default saves the spot');
    ok(combo.selected === 7, 'combo shows Custom');

    // ---- Integrations page (needs the daemon)
    await sleep(800);
    const service = byTitle(Adw.ActionRow, 'Selawi Island service');
    ok(service?.subtitle === 'Running', 'integrations page sees the running daemon');
    const bluetooth = byTitle(Adw.SwitchRow, 'Bluetooth');
    ok(bluetooth.active && bluetooth.sensitive, 'Bluetooth switch is on');
    ok(byTitle(Adw.SwitchRow, 'Network').active, 'Network switch is on');
    ok(!byTitle(Adw.SwitchRow, 'System warnings').active, 'System warnings switch is off by default');
    bluetooth.active = false;
    await sleep(500);
    ok(daemonConfig().integrations.bluez === false, 'turning Bluetooth off reaches the daemon');

    const mode = byTitle(Adw.ComboRow, 'Players to show');
    const preferred = byTitle(Adw.ComboRow, 'Preferred player');
    ok(!preferred.sensitive, 'preferred player disabled while showing all players');
    mode.selected = 1;
    await sleep(500);
    ok(daemonConfig().media.mode === 'one', 'choosing One player reaches the daemon');
    ok(preferred.sensitive, 'preferred player enabled in one-player mode');

    const body = byTitle(Adw.SwitchRow, 'Show notification text');
    body.active = false;
    await sleep(500);
    ok(daemonConfig().notifications.show_body === false, 'hiding notification text reaches the daemon');

    const audio = byTitle(Adw.SwitchRow, 'Audio devices');
    audio.active = false;
    ok(settings.get_boolean('audio-events') === false, 'audio switch saves the extension setting');
}
