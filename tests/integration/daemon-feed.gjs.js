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
    connect(_n, cb) { (this._h ??= []).push(cb); }
    emit() { for (const h of this._h ?? []) h(); }
}`);

const {DaemonFeed} = await import(`file://${dir}/daemon-feed.js`);
const warnings = [];
const feed = new DaemonFeed({debug() {}, warn: m => warnings.push(m)});
const loop = new GLib.MainLoop(null, false);
const ids = () => feed.events().map(e => e.id).join(',');
const run = cmd => GLib.spawn_command_line_sync(cmd);
let failed = null;
const check = (cond, msg) => { if (!cond && !failed) failed = msg; };

const steps = [
    [400, () => check(feed.connected, 'feed did not connect')],
    [450, () => run(`${cli} notify Hello --id g1 --ttl 30`)],
    [800, () => { check(ids() === 'g1', `expected g1, got "${ids()}"`); feed.dismiss('g1'); }],
    [1100, () => { check(ids() === '', `dismiss failed, got "${ids()}"`); feed.invokeAction('g1', 'nope'); }],
    [1400, () => check(warnings.some(w => w.includes('UnknownAction')), 'unknown action not reported')],
    [1450, () => { feed.destroy(); loop.quit(); }],
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
