// The settings window: Island, Integrations and Status pages under a view
// switcher. The pages are shared with the extension's preferences window.

import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

export const APP_ID = 'com.selawi.Island.Settings';
const UUID = 'selawi-island@selawi';
const SCHEMA = 'org.gnome.shell.extensions.selawi-island';

const here = GLib.path_get_dirname(GLib.filename_from_uri(import.meta.url)[0]);

/**
 * The shared pages: next to this file when installed (pages/), or in the
 * extension's folder when run from the repository.
 */
export async function loadPages() {
    const installed = `${here}/pages`;
    const dir = GLib.file_test(`${installed}/pages.js`, GLib.FileTest.EXISTS)
        ? installed : `${here}/../gnome-extension/settings`;
    return import(`file://${dir}/pages.js`);
}

/**
 * The extension's settings, from its compiled schema (GNOME loads it from
 * the extension's folder too). Null when the extension is not installed.
 * SELAWI_ISLAND_SCHEMA_DIR overrides where to look (tests).
 */
export function extensionSettings() {
    const dataDirs = [GLib.get_user_data_dir(), ...GLib.get_system_data_dirs()];
    const candidates = [
        GLib.getenv('SELAWI_ISLAND_SCHEMA_DIR'),
        ...dataDirs.map(d => `${d}/gnome-shell/extensions/${UUID}/schemas`),
    ].filter(Boolean);
    for (const dir of candidates) {
        if (!GLib.file_test(`${dir}/gschemas.compiled`, GLib.FileTest.EXISTS))
            continue;
        try {
            const source = Gio.SettingsSchemaSource.new_from_directory(
                dir, Gio.SettingsSchemaSource.get_default(), false);
            const schema = source.lookup(SCHEMA, false);
            if (schema)
                return new Gio.Settings({settings_schema: schema});
        } catch {
            // Try the next place.
        }
    }
    return null;
}

/**
 * @param {Adw.Application} app
 * @param {Gio.Settings|null} settings - null: only the Status page
 * @param {object} pages - islandPage, integrationsPage, statusPage
 */
export function buildWindow(app, settings, pages) {
    const window = new Adw.ApplicationWindow({
        application: app,
        title: 'Selawi Island',
        default_width: 640,
        default_height: 720,
        width_request: 360,
        height_request: 400,
    });
    const stack = new Adw.ViewStack({vexpand: true});
    const add = (page, name) => {
        const item = stack.add_titled(page, name, page.title);
        item.icon_name = page.icon_name;
    };
    if (settings) {
        add(pages.islandPage(settings, window), 'island');
        add(pages.integrationsPage(settings, window), 'integrations');
    }
    add(pages.statusPage(window), 'status');
    if (!settings)
        stack.visible_child_name = 'status';

    const switcher = new Adw.ViewSwitcher({stack, policy: Adw.ViewSwitcherPolicy.WIDE});
    const header = new Adw.HeaderBar({title_widget: switcher});
    const bottom = new Adw.ViewSwitcherBar({stack});
    const view = new Adw.ToolbarView({content: stack});
    view.add_top_bar(header);
    view.add_bottom_bar(bottom);

    // Narrow windows: tabs move to the bottom.
    const narrow = new Adw.Breakpoint({condition: Adw.BreakpointCondition.parse('max-width: 500sp')});
    narrow.add_setter(switcher, 'visible', false);
    narrow.add_setter(bottom, 'reveal', true);
    window.add_breakpoint(narrow);

    if (!settings) {
        const banner = new Adw.Banner({
            title: 'The Selawi Island GNOME extension is not installed: only the status is shown.',
            revealed: true,
        });
        view.add_top_bar(banner);
    }
    window.content = view;
    return window;
}
