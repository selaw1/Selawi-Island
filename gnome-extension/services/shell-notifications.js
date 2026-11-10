// Notifications the daemon cannot see, passed to the island while it hides
// GNOME's banners (otherwise they would show nowhere):
// - GNOME Shell's own ("Screenshot captured", network sign-in, drives…),
//   which never go over D-Bus;
// - apps using GNOME's notification route (GNotification: Clocks, Files,
//   Software…), which may arrive through the notification portal.
// Notifications sent the common way (org.freedesktop.Notifications) are
// left to the daemon.

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {FdoNotificationDaemonSource, GtkNotificationDaemonAppSource} from 'resource:///org/gnome/shell/ui/notificationDaemon.js';

import {shellNotificationInput} from './shell-model.js';

export class ShellNotifications {
    /**
     * @param {(input: object) => void} submit - sends one event input
     * @param {(callback: (showBody: boolean) => void) => void} getShowBody
     */
    constructor(submit, getShowBody) {
        this._submit = submit;
        this._getShowBody = getShowBody;
        this._seq = 0;
        this._sources = new Set();
        Main.messageTray.connectObject('source-added', (_tray, source) => this._watch(source), this);
        for (const source of Main.messageTray.getSources())
            this._watch(source);
    }

    _watch(source) {
        if (source instanceof FdoNotificationDaemonSource)
            return;
        this._sources.add(source);
        source.connectObject(
            'notification-added', (_source, notification) => this._show(source, notification),
            'destroy', () => this._sources.delete(source),
            this);
    }

    _show(source, notification) {
        const seq = ++this._seq;
        this._getShowBody(showBody => {
            const input = shellNotificationInput({
                app: source.title,
                // GNOME's app sources know the app's id (for its logo).
                appId: source instanceof GtkNotificationDaemonAppSource ? source.policy?.id ?? null : null,
                title: notification.title,
                body: notification.body,
                urgency: notification.urgency,
                seq,
                showBody,
            });
            if (input)
                this._submit({...input, state: 'started'});
        });
    }

    destroy() {
        Main.messageTray.disconnectObject(this);
        for (const source of this._sources)
            source.disconnectObject(this);
        this._sources.clear();
    }
}
