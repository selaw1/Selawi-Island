// The extension's preferences window (gnome-extensions prefs, or when the
// settings app is not installed). The same pages as the Selawi Island
// settings app, from ./settings/.

import {ExtensionPreferences} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

import {integrationsPage} from './settings/integrations-page.js';
import {islandPage} from './settings/island-page.js';
import {statusPage} from './settings/status-page.js';

export default class SelawiIslandPreferences extends ExtensionPreferences {
    async fillPreferencesWindow(window) {
        const settings = this.getSettings();
        window.add(islandPage(settings, window));
        window.add(integrationsPage(settings, window));
        window.add(statusPage(window));
    }
}
