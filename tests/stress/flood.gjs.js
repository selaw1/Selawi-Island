// Sends many events to the daemon as fast as D-Bus allows and counts the
// Changed signals it emits. Prints JSON: {submits, seconds, changed}.
//   gjs -m tests/stress/flood.gjs.js COUNT
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import System from 'system';

const count = Number(ARGV[0] ?? 2000);
const bus = Gio.DBus.session;
let changed = 0;
bus.signal_subscribe('com.selawi.Island', 'com.selawi.Island.Events1', 'Changed', '/com/selawi/Island', null,
    Gio.DBusSignalFlags.NONE, () => changed++);

const submit = json => new Promise((resolve, reject) => bus.call('com.selawi.Island', '/com/selawi/Island',
    'com.selawi.Island.Events1', 'Submit', new GLib.Variant('(s)', [json]), null, Gio.DBusCallFlags.NONE, 10000, null,
    (b, res) => {
        try {
            b.call_finish(res);
            resolve();
        } catch (e) {
            reject(e);
        }
    }));

const loop = new GLib.MainLoop(null, false);
let result = null;
(async () => {
    const start = GLib.get_monotonic_time();
    const pending = [];
    for (let i = 0; i < count; i++) {
        // 100 distinct progress tasks, each updated many times (like busy
        // downloads or builds), all at once.
        const id = `stress-${i % 100}`;
        pending.push(submit(JSON.stringify({
            id, source: {app: 'Stress', integration: 'stress'}, category: 'transfer',
            state: i < 100 ? 'started' : 'updated', persistent: true,
            payload: {type: 'progress', title: `Task ${i % 100}`, percent: (i / count) * 100},
        })));
        if (pending.length >= 200)
            await Promise.all(pending.splice(0));
    }
    await Promise.all(pending);
    const seconds = (GLib.get_monotonic_time() - start) / 1e6;
    await new Promise(r => GLib.timeout_add(GLib.PRIORITY_DEFAULT, 500, () => r() ?? GLib.SOURCE_REMOVE));
    result = {submits: count, seconds: Number(seconds.toFixed(2)), changed};
})().catch(e => {
    printerr(e.message);
}).finally(() => loop.quit());
loop.run();
print(JSON.stringify(result));
System.exit(result ? 0 : 1);
