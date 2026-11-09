// Selawi Island GNOME Shell extension entry point.
//
// The extension only draws the island. Events come from a feed: for now
// the built-in simulator; from Phase 3 the Selawi Island daemon over D-Bus.

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {SimulatedFeed} from './services/simulated-feed.js';
import {Island} from './ui/island.js';

export default class SelawiIslandExtension extends Extension {
    enable() {
        this._settings = this.getSettings();
        this._logger = this.getLogger();
        this._island = new Island(this._settings, this._logger);
        this._settings.connectObject('changed::simulation-enabled', () => this._resetFeed(), this);
        this._resetFeed();
    }

    disable() {
        this._settings.disconnectObject(this);
        this._feed?.destroy();
        this._feed = null;
        this._island.destroy();
        this._island = null;
        this._settings = null;
        this._logger = null;
    }

    _resetFeed() {
        this._feed?.destroy();
        this._feed = null;
        if (this._settings.get_boolean('simulation-enabled')) {
            const scenario = this.dir.get_child('data').get_child('demo-scenario.json');
            this._feed = new SimulatedFeed(scenario, this._logger);
            this._feed.start().catch(e => this._logger.error(`Simulator failed: ${e.message}`));
        }
        this._island.setFeed(this._feed);
    }
}
