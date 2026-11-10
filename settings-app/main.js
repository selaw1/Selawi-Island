// Selawi Island settings: a GNOME app (GTK 4 + libadwaita) with the same
// pages as the extension's preferences window. It also opens when the
// extension is off or broken. Run from the repository with
//   gjs -m settings-app/main.js
// (installed as selawi-island-settings).

import Adw from 'gi://Adw?version=1';
import Gio from 'gi://Gio';
import 'gi://Gtk?version=4.0';
import System from 'system';

import {APP_ID, buildWindow, extensionSettings, loadPages} from './window.js';

const pages = await loadPages();
const app = new Adw.Application({application_id: APP_ID, flags: Gio.ApplicationFlags.DEFAULT_FLAGS});
app.connect('activate', () => {
    (app.get_active_window() ?? buildWindow(app, extensionSettings(), pages)).present();
});
await app.runAsync([System.programInvocationName, ...ARGV]);
