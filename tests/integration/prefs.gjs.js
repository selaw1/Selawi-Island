// Opens the real prefs.js window (GTK4 + libadwaita) and checks the Position
// controls. Run by tests/integration/prefs-check.sh.
import Adw from 'gi://Adw';
import Gtk from 'gi://Gtk?version=4.0';
const {default: SelawiIslandPreferences} = await import(`file://${ARGV[0]}/prefs.js`);
Adw.init();
const prefs = new SelawiIslandPreferences();
const settings = prefs.getSettings();
const window = new Adw.PreferencesWindow();
await prefs.fillPreferencesWindow(window);
const all = [];
const walk = w => { for (let c = w.get_first_child(); c; c = c.get_next_sibling()) { all.push(c); walk(c); } };
walk(window);
const combo = all.find(w => w instanceof Adw.ComboRow);
const btn = label => all.find(w => w instanceof Gtk.Button && w.label === label);
const ok = (c, m) => { if (!c) { printerr('FAIL: ' + m); imports.system.exit(1); } print('ok: ' + m); };
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
window.close();
