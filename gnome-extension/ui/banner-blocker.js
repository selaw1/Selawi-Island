// Hides GNOME's own notification banners while the island shows
// notifications, so each notification appears once. Notifications still
// go to GNOME's notification list as usual.
//
// GNOME's top bar also sets bannerBlocked (true while the calendar menu is
// open, false when it closes), so after the menu closes it is set again.

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

export class BannerBlocker {
    constructor() {
        this._active = false;
        this._menu = Main.panel.statusArea.dateMenu?.menu ?? null;
        this._menu?.connectObject('open-state-changed', (_menu, open) => {
            if (!open && this._active)
                Main.messageTray.bannerBlocked = true;
        }, this);
    }

    set active(active) {
        if (active === this._active)
            return;
        this._active = active;
        Main.messageTray.bannerBlocked = active;
    }

    destroy() {
        this._menu?.disconnectObject(this);
        this._menu = null;
        this.active = false;
    }
}
