// Runs the extension's real DaemonFeed against a running daemon.
// GNOME Shell's signals.js is replaced with a tiny local emitter by
// loading a patched copy into a temporary directory.
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import System from 'system';

const [cli] = ARGV;
const root = GLib.get_current_dir();
const dir = GLib.dir_make_tmp('selawi-gjs-XXXXXX');
const read = p => new TextDecoder().decode(GLib.file_get_contents(`${root}/${p}`)[1]);
GLib.file_set_contents(`${dir}/protocol.js`, read('gnome-extension/services/protocol.js'));
GLib.file_set_contents(`${dir}/daemon-feed.js`, read('gnome-extension/services/daemon-feed.js')
    .replace('resource:///org/gnome/shell/misc/signals.js', './signals.js'));
GLib.file_set_contents(`${dir}/signals.js`, `export class EventEmitter {
    connect(name, cb) { ((this._h ??= {})[name] ??= []).push(cb); }
    emit(name, ...args) { for (const h of this._h?.[name] ?? []) h(this, ...args); }
}`);

const {DaemonFeed} = await import(`file://${dir}/daemon-feed.js`);
const warnings = [];
const feed = new DaemonFeed({debug() {}, warn: m => warnings.push(m)});
const loop = new GLib.MainLoop(null, false);
const ids = () => feed.events().map(e => e.id).join(',');
const invoked = [];
feed.connect('action-invoked', (_f, id, actionId) => invoked.push(`${id}/${actionId}`));
const run = cmd => GLib.spawn_command_line_sync(cmd);
let failed = null;
const check = (cond, msg) => { if (!cond && !failed) failed = msg; };

const steps = [
    [400, () => check(feed.connected, 'feed did not connect')],
    [450, () => run(`${cli} notify Hello --id g1 --ttl 30`)],
    [800, () => { check(ids() === 'g1', `expected g1, got "${ids()}"`); feed.dismiss('g1'); }],
    [1100, () => { check(ids() === '', `dismiss failed, got "${ids()}"`); feed.invokeAction('g1', 'nope'); }],
    [1400, () => check(warnings.some(w => w.includes('UnknownAction')), 'unknown action not reported')],
    // An event submitted by the extension itself, with a button: pressing
    // it must come back as 'action-invoked' (this is how Mute works).
    [1450, () => feed.submit({id: 'privacy:capture', source: {app: 'Test', integration: 'gnome-privacy'},
        category: 'call', state: 'started', persistent: true,
        payload: {type: 'system', title: 'Microphone', detail: 'Test', severity: 'info'},
        actions: [{id: 'mute', label: 'Mute'}]})],
    [1800, () => {
        check(ids() === 'privacy:capture', `submitted event not shown, got "${ids()}"`);
        feed.invokeAction('privacy:capture', 'mute');
    }],
    [2100, () => check(invoked.includes('privacy:capture/mute'), 'Mute press did not come back as action-invoked')],
    [2150, () => { feed.destroy(); loop.quit(); }],
];
for (const [ms, fn] of steps)
    GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => { fn(); return GLib.SOURCE_REMOVE; });
loop.run();
Gio.File.new_for_path(dir).trash(null);
if (failed) {
    printerr(`DaemonFeed: ${failed}`);
    System.exit(1);
}
print('DaemonFeed check passed.');
