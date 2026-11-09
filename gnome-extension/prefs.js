// Quick preferences for the island. The full settings application
// (GTK4 + libadwaita, Rust) comes in Phase 10.

import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import Gtk from 'gi://Gtk';

import {ExtensionPreferences} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

import {integrationsPage} from './prefs-integrations.js';

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

// Value stored in settings → label shown. Order is the dropdown order.
const ANCHOR_LABELS = [
    ['top-center', 'Top center'],
    ['top-left', 'Top left'],
    ['top-right', 'Top right'],
    ['center', 'Center of the screen'],
    ['bottom-center', 'Bottom center'],
    ['bottom-left', 'Bottom left'],
    ['bottom-right', 'Bottom right'],
    ['custom', 'Custom (saved from a drag)'],
];

function positionGroup(settings, window) {
    const group = new Adw.PreferencesGroup({
        title: 'Position',
        description: 'Drag the island to move it. Dropping it near its default position snaps it back.',
    });
    const handlers = [];
    const watch = (keys, fn) => {
        for (const key of keys)
            handlers.push(settings.connect(`changed::${key}`, fn));
        fn();
    };
    window.connect('close-request', () => handlers.forEach(id => settings.disconnect(id)));

    // Default position.
    const anchorRow = new Adw.ComboRow({
        title: 'Default position',
        subtitle: 'Where the island lives until you drag it',
        model: Gtk.StringList.new(ANCHOR_LABELS.map(([, label]) => label)),
    });
    let syncing = false;
    watch(['default-anchor'], () => {
        const index = ANCHOR_LABELS.findIndex(([value]) => value === settings.get_string('default-anchor'));
        syncing = true;
        anchorRow.selected = Math.max(0, index);
        syncing = false;
    });
    anchorRow.connect('notify::selected', () => {
        if (!syncing)
            settings.set_string('default-anchor', ANCHOR_LABELS[anchorRow.selected][0]);
    });
    group.add(anchorRow);

    // The current dragged position, with ways to keep it or drop it.
    const current = new Adw.ActionRow({title: 'Current position'});
    const makeDefault = new Gtk.Button({label: 'Make this the default', valign: Gtk.Align.CENTER});
    makeDefault.connect('clicked', () => {
        settings.set_double('default-x', settings.get_double('position-x'));
        settings.set_double('default-y', settings.get_double('position-y'));
        settings.set_string('default-anchor', 'custom');
        settings.set_boolean('custom-position', false);
    });
    const reset = new Gtk.Button({label: 'Reset', valign: Gtk.Align.CENTER});
    reset.connect('clicked', () => settings.set_boolean('custom-position', false));
    current.add_suffix(makeDefault);
    current.add_suffix(reset);
    watch(['custom-position'], () => {
        const dragged = settings.get_boolean('custom-position');
        current.subtitle = dragged ? 'Moved by dragging' : 'At the default position';
        makeDefault.sensitive = dragged;
        reset.sensitive = dragged;
    });
    group.add(current);

    group.add(switchRow(settings, 'allow-drag', 'Allow dragging'));
    group.add(switchRow(settings, 'below-top-bar', 'Below the top bar',
        'For top positions: turn off to place the island over the top bar'));
    group.add(spinRow(settings, 'top-margin', 'Edge margin', 0, 200));
    group.add(spinRow(settings, 'monitor-index', 'Monitor (-1 = primary)', -1, 16));
    return group;
}

export default class SelawiIslandPreferences extends ExtensionPreferences {
    async fillPreferencesWindow(window) {
        const settings = this.getSettings();
        const page = new Adw.PreferencesPage({title: 'Island', icon_name: 'preferences-system-symbolic'});

        const behaviour = new Adw.PreferencesGroup({title: 'Behaviour'});
        behaviour.add(switchRow(settings, 'simulation-enabled', 'Demo events',
            'Play simulated events instead of real ones from the Selawi Island service'));
        behaviour.add(switchRow(settings, 'auto-expand', 'Expand for new events',
            'Briefly expand when a new most-important event arrives'));
        behaviour.add(spinRow(settings, 'auto-expand-ms', 'Expansion time (ms)', 1000, 30000, 250));
        behaviour.add(switchRow(settings, 'show-when-idle', 'Show when idle'));
        page.add(behaviour);

        page.add(positionGroup(settings, window));

        const size = new Adw.PreferencesGroup({title: 'Size and motion'});
        size.add(switchRow(settings, 'idle-width-match-clock', 'Idle width matches the clock',
            'Turn off to choose the idle width below'));
        const idleWidth = spinRow(settings, 'idle-width', 'Idle width', 40, 400);
        settings.bind('idle-width-match-clock', idleWidth, 'sensitive', Gio.SettingsBindFlags.INVERT_BOOLEAN);
        size.add(idleWidth);
        size.add(spinRow(settings, 'idle-height', 'Idle height', 4, 80));
        size.add(spinRow(settings, 'compact-width', 'Compact width', 160, 800));
        size.add(spinRow(settings, 'compact-height', 'Height', 24, 80));
        size.add(spinRow(settings, 'expanded-width', 'Expanded width', 280, 1000));
        size.add(spinRow(settings, 'animation-duration-scale', 'Animation speed (× duration)', 0, 3, 0.05, 2));
        page.add(size);

        window.add(page);
        window.add(integrationsPage(settings, window));
    }
}
