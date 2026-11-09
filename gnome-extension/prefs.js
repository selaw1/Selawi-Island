// Quick preferences for the island. The full settings application
// (GTK4 + libadwaita, Rust) comes in Phase 10.

import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import Gtk from 'gi://Gtk';

import {ExtensionPreferences} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

function switchRow(settings, key, title, subtitle = '') {
    const row = new Adw.SwitchRow({title, subtitle});
    settings.bind(key, row, 'active', Gio.SettingsBindFlags.DEFAULT);
    return row;
}

function spinRow(settings, key, title, lower, upper, step = 1, digits = 0) {
    const row = new Adw.SpinRow({
        title,
        digits,
        adjustment: new Gtk.Adjustment({lower, upper, step_increment: step, page_increment: step * 10}),
    });
    settings.bind(key, row, 'value', Gio.SettingsBindFlags.DEFAULT);
    return row;
}

export default class SelawiIslandPreferences extends ExtensionPreferences {
    async fillPreferencesWindow(window) {
        const settings = this.getSettings();
        const page = new Adw.PreferencesPage({title: 'Island', icon_name: 'preferences-system-symbolic'});

        const behaviour = new Adw.PreferencesGroup({title: 'Behaviour'});
        behaviour.add(switchRow(settings, 'simulation-enabled', 'Demo events',
            'Play simulated calls, music, downloads and devices'));
        behaviour.add(switchRow(settings, 'auto-expand', 'Expand for new events',
            'Briefly expand when a new most-important event arrives'));
        behaviour.add(spinRow(settings, 'auto-expand-ms', 'Expansion time (ms)', 1000, 30000, 250));
        behaviour.add(switchRow(settings, 'show-when-idle', 'Show when idle'));
        page.add(behaviour);

        const position = new Adw.PreferencesGroup({title: 'Position'});
        position.add(switchRow(settings, 'below-top-bar', 'Below the top bar',
            'Turn off to place the island over the top bar (covers the clock)'));
        position.add(spinRow(settings, 'top-margin', 'Top margin', 0, 200));
        position.add(spinRow(settings, 'monitor-index', 'Monitor (-1 = primary)', -1, 16));
        page.add(position);

        const size = new Adw.PreferencesGroup({title: 'Size and motion'});
        size.add(spinRow(settings, 'idle-width', 'Idle width', 40, 400));
        size.add(spinRow(settings, 'compact-width', 'Compact width', 160, 800));
        size.add(spinRow(settings, 'compact-height', 'Height', 24, 80));
        size.add(spinRow(settings, 'expanded-width', 'Expanded width', 280, 1000));
        size.add(spinRow(settings, 'animation-duration-scale', 'Animation speed (× duration)', 0, 3, 0.05, 2));
        page.add(size);

        window.add(page);
    }
}
