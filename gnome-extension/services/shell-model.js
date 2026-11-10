// What the island shows for things only GNOME Shell sees: notifications
// from GNOME Shell itself ("Screenshot captured") and from apps using
// GNOME's notification route (Clocks, Files, Software…, also through the
// portal), and copying to the clipboard.
// Pure (no GNOME imports) so it is unit-tested with Node.

const MAX_TITLE = 256;
const MAX_BODY = 1024;

function clean(text, max) {
    return String(text ?? '').replace(/<[^>]*>/g, '').replace(/[\u0000-\u001f\u007f]/g, ' ')
        .replace(/\s+/g, ' ').trim().slice(0, max);
}

/**
 * A GNOME Shell notification as an event input (without `state`), or null.
 * Ids start with "notifications:" so the daemon drops them when the
 * Notifications integration is off.
 *
 * @param {object} n
 * @param {string} n.app - the notification source's title ("Screen Capture", "Clocks")
 * @param {string|null} [n.appId] - the app's id for apps ("org.gnome.clocks"): logo and grouping
 * @param {string} n.title
 * @param {string} [n.body]
 * @param {number} n.urgency - GNOME's 0 low … 3 critical
 * @param {number} n.seq - makes each id unique
 * @param {boolean} n.showBody - the "Show notification text" setting
 */
export function shellNotificationInput({app, appId = null, title, body, urgency, seq, showBody}) {
    const cleanTitle = clean(title, MAX_TITLE);
    if (!cleanTitle)
        return null;
    const cleanBody = showBody ? clean(body, MAX_BODY) : '';
    const payload = {type: 'notification', title: cleanTitle};
    if (cleanBody)
        payload.body = cleanBody;
    const validApp = typeof appId === 'string' && /^[A-Za-z0-9._-]{1,128}$/.test(appId) ? appId : null;
    const key = validApp ? validApp.split('.').pop().toLowerCase().replace(/[^a-z0-9_-]/g, '_') : 'gnome-shell';
    const source = {app: clean(app, 64) || 'GNOME', integration: 'gnome-shell'};
    if (validApp)
        source.icon = `app:${validApp}`;
    const input = {
        id: `notifications:gnome-shell:${seq}`,
        source,
        category: 'notification',
        persistent: false,
        ttl_ms: urgency >= 3 ? 15000 : 6000,
        group: `notifications:${key}`,
        payload,
        actions: [],
    };
    if (urgency >= 3)
        input.priority = 'critical';
    else if (urgency === 0)
        input.priority = 'reminder';
    return input;
}

export const CLIPBOARD_EVENT_ID = 'clipboard:copied';

/**
 * "Copied" for a new clipboard owner, from the kinds of data it offers
 * (never its content).
 *
 * @param {string[]} mimetypes
 */
export function clipboardInput(mimetypes) {
    const types = mimetypes ?? [];
    let title = 'Copied';
    if (types.some(t => t === 'x-special/gnome-copied-files' || t === 'text/uri-list'))
        title = 'Copied files';
    else if (types.some(t => t.startsWith('image/')))
        title = 'Copied image';
    return {
        id: CLIPBOARD_EVENT_ID,
        source: {app: 'Clipboard', integration: 'gnome-clipboard'},
        category: 'system',
        priority: 'notification',
        persistent: false,
        ttl_ms: 1500,
        payload: {type: 'system', title, severity: 'info'},
        actions: [],
    };
}

/** Events that should show without opening the island. */
export function isQuiet(event) {
    return event?.source?.integration === 'gnome-clipboard';
}
