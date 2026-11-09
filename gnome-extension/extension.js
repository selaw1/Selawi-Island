// Selawi Island GNOME Shell extension entry point.
//
// The extension draws the island and adds what only GNOME Shell can see
// (audio devices). Events come from a feed: the Selawi Island daemon over
// D-Bus, or the built-in demo simulator.

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {AudioSource} from './services/audio-source.js';
import {DaemonFeed} from './services/daemon-feed.js';
import {SimulatedFeed} from './services/simulated-feed.js';
import {BannerBlocker} from './ui/banner-blocker.js';
import {Island} from './ui/island.js';

export default class SelawiIslandExtension extends Extension {
    enable() {
        this._settings = this.getSettings();
        this._logger = this.getLogger();
        this._island = new Island(this._settings, this._logger, () => this.openPreferences());
        this._banners = new BannerBlocker();
        this._settings.connectObject(
            'changed::simulation-enabled', () => this._resetFeed(),
            'changed::audio-events', () => this._resetAudio(),
            'changed::hide-gnome-banners', () => this._updateBanners(),
            this);
        this._resetFeed();
    }

    disable() {
        this._settings.disconnectObject(this);
        this._audio?.destroy();
        this._audio = null;
        this._feed?.disconnectObject(this);
        this._feed?.destroy();
        this._feed = null;
        this._banners.destroy();
        this._banners = null;
        this._island.destroy();
        this._island = null;
        this._settings = null;
        this._logger = null;
    }

    _resetFeed() {
        this._audio?.destroy();
        this._audio = null;
        this._feed?.disconnectObject(this);
        this._feed?.destroy();
        this._feed = null;
        if (this._settings.get_boolean('simulation-enabled')) {
            const scenario = this.dir.get_child('data').get_child('demo-scenario.json');
            this._feed = new SimulatedFeed(scenario, this._logger);
            this._feed.start().catch(e => this._logger.error(`Simulator failed: ${e.message}`));
        } else {
            this._feed = new DaemonFeed(this._logger);
            this._feed.connectObject('integrations-changed', () => this._updateBanners(), this);
        }
        this._island.setFeed(this._feed);
        this._resetAudio();
        this._updateBanners();
    }

    _resetAudio() {
        this._audio?.destroy();
        this._audio = null;
        const feed = this._feed;
        if (feed instanceof DaemonFeed && this._settings.get_boolean('audio-events'))
            this._audio = new AudioSource(input => feed.submit(input));
    }

    // GNOME's banners are hidden only while the island really shows
    // notifications; if the daemon stops, GNOME's banners come back.
    _updateBanners() {
        const feed = this._feed;
        this._banners.active = this._settings.get_boolean('hide-gnome-banners') &&
            feed instanceof DaemonFeed && feed.integrationActive('notifications');
    }
}
